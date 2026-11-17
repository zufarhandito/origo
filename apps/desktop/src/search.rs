//! Find belongs to a document entity; only one page is scanned at a time.
use crate::*;
use gpui::assets::IconName;
use gpui::component::{
    ActiveTheme, Disableable, Sizable,
    button::{Button, ButtonVariants},
    input::Input,
    menu::{DropdownMenu, PopupMenuItem},
};
use pdf_application::{
    search::{MATCH_LIMIT, QUERY_LIMIT, SearchOptions, SearchQuery, next_match},
    text::{TextExtractor, TextRequest},
};

pub(super) struct Match {
    pub page: usize,
    pub range: Range<usize>,
}
#[derive(Default)]
pub(super) struct SearchState {
    pub open: bool,
    pub options: SearchOptions,
    pub matches: Vec<Match>,
    pub current: Option<usize>,
    pub scanned: usize,
    pub errors: usize,
    pub has_text: bool,
    pub running: bool,
    pub capped: bool,
    pub wrapped: bool,
    pub token: u64,
    pending_reveal: bool,
    pub preserve_reading: bool,
    busy: bool,
    delayed: bool,
    query: Option<SearchQuery>,
    debounce: Option<gpui::Task<()>>,
}
impl SearchState {
    pub(super) fn pause(&mut self) {
        self.token = self.token.wrapping_add(1);
        self.debounce = None;
        self.delayed = false;
    }
}

impl DocumentSession {
    pub(super) fn show_search(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.workspace.is_none() || self.opening || self.edits_locked() {
            return;
        }
        let was_open = self.search.open;
        self.search.open = true;
        self.search_input.update(cx, |input, cx| {
            input.focus(window, cx);
            input.select_all(window, cx);
        });
        if !was_open {
            self.restart_search(cx);
        }
        cx.notify();
    }
    pub(super) fn close_search(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.search.open = false;
        self.search.pause();
        self.search.running = false;
        self.search.matches.clear();
        self.search.current = None;
        self.focus.focus(window, cx);
        self.report(cx);
        cx.notify();
    }
    pub(super) fn restart_search(&mut self, cx: &mut Context<Self>) {
        self.search.pause();
        self.search.matches.clear();
        self.search.current = None;
        self.search.scanned = 0;
        self.search.errors = 0;
        self.search.has_text = false;
        self.search.capped = false;
        self.search.wrapped = false;
        self.search.pending_reveal = false;
        self.search.preserve_reading = false;
        self.search.query = SearchQuery::new(
            self.search_input.read(cx).value().as_str(),
            self.search.options,
        );
        self.search.running =
            self.search.open && self.search.query.is_some() && self.workspace.is_some();
        if self.search.running && self.visible_for_render {
            self.search.delayed = true;
            let token = self.search.token;
            let timer = cx
                .background_executor()
                .timer(std::time::Duration::from_millis(200));
            self.search.debounce = Some(cx.spawn(async move |this, cx| {
                timer.await;
                let _ = this.update(cx, |this, cx| {
                    if this.search.token == token {
                        this.search.delayed = false;
                        this.pump_search(cx);
                    }
                });
            }));
        }
        self.report(cx);
        cx.notify();
    }
    pub(super) fn pump_search(&mut self, cx: &mut Context<Self>) {
        if !self.visible_for_render
            || !self.search.open
            || !self.search.running
            || self.search.busy
            || self.search.delayed
        {
            return;
        }
        let Some(workspace) = self.workspace.as_ref() else {
            self.search.running = false;
            return;
        };
        let index = self.search.scanned;
        let Some(page) = workspace.pages().get(index) else {
            self.search.running = false;
            self.report(cx);
            cx.notify();
            return;
        };
        let Some(source) = workspace.resolve(page.source_id) else {
            return;
        };
        let request = TextRequest {
            path: source.path.clone(),
            page_index: page.source_page_index,
            fingerprint: source.fingerprint,
        };
        let Some(query) = self.search.query.clone() else {
            return;
        };
        let limit = MATCH_LIMIT.saturating_sub(self.search.matches.len());
        let token = self.search.token;
        let source_sequence = self.source_sequence;
        let renderer = self.renderer.clone();
        self.search.busy = true;
        let task = cx.background_executor().spawn(async move {
            renderer.extract_text(&request).map(|page| {
                let has_text = page.chars.iter().any(|ch| !ch.unicode.is_whitespace());
                (query.find(&page, limit), has_text)
            })
        });
        cx.spawn(async move |this, cx| {
            let result = task.await;
            let _ = this.update(cx, |this, cx| {
                this.search.busy = false;
                if this.search.token == token
                    && this.source_sequence == source_sequence
                    && this.visible_for_render
                    && this.search.open
                {
                    this.search.scanned += 1;
                    match result {
                        Ok((ranges, has_text)) => {
                            this.search.has_text |= has_text;
                            this.search.matches.extend(
                                ranges.into_iter().map(|range| Match { page: index, range }),
                            );
                            if this.search.current.is_none() && !this.search.matches.is_empty() {
                                this.search.current = Some(0);
                                if !this.edits_locked() && !this.search.preserve_reading {
                                    this.reveal_search_match(cx);
                                }
                            }
                            if this.search.matches.len() >= MATCH_LIMIT {
                                this.search.capped = true;
                                this.search.running = false;
                            }
                        }
                        Err(pdf_application::RenderError::SourceChanged) => {
                            this.search.pause();
                            this.search.running = false;
                            this.search.matches.clear();
                            this.search.current = None;
                            this.invalidate_work();
                            this.clear_bitmaps(cx);
                            this.text = text_selection::TextState::new(this.text_budget);
                            this.workspace.clear();
                            this.navigation = Navigation::default();
                            this.source_sequence += 1;
                            this.error =
                                Some(pdf_application::RenderError::SourceChanged.to_string());
                            this.status = pdf_application::RenderError::SourceChanged.to_string();
                        }
                        Err(_) => this.search.errors += 1,
                    }
                    this.report(cx);
                    cx.notify();
                }
                this.pump_search(cx);
            });
        })
        .detach();
    }
    pub(super) fn move_search(&mut self, previous: bool, cx: &mut Context<Self>) {
        if self.edits_locked() || !self.search.open {
            return;
        }
        let old = self.search.current;
        let next = next_match(old, self.search.matches.len(), previous);
        self.search.wrapped =
            matches!((old, next), (Some(a), Some(b)) if if previous { b >= a } else { b <= a });
        self.search.current = next;
        self.reveal_search_match(cx);
        self.report(cx);
        cx.notify();
    }
    fn reveal_search_match(&mut self, cx: &mut Context<Self>) {
        let Some(hit) = self.search.current.and_then(|i| self.search.matches.get(i)) else {
            return;
        };
        let page = hit.page;
        self.search.pending_reveal = true;
        self.go_to(page as u16, cx);
        self.request_text(page, cx);
        self.align_search_match(cx);
    }
    pub(super) fn align_search_match(&mut self, cx: &mut Context<Self>) {
        if !self.search.pending_reveal {
            return;
        }
        let Some(hit) = self.search.current.and_then(|i| self.search.matches.get(i)) else {
            return;
        };
        if !self.search.open || usize::from(self.navigation.current) != hit.page {
            return;
        }
        let Some(text) = self.text.pages.get(&hit.page) else {
            return;
        };
        let Some(rect) = text.highlights(hit.range.clone()).first().copied() else {
            return;
        };
        let Some(geometry) = self
            .workspace
            .as_ref()
            .and_then(|w| w.geometry(hit.page).ok())
        else {
            return;
        };
        let (width, height) = self.zoom.size(geometry, self.viewport);
        self.search.pending_reveal = false;
        let top = (32. + height * rect.top - self.viewport.1 * 0.3).max(0.);
        self.page_list.scroll_to(gpui::ListOffset {
            item_ix: hit.page,
            offset_in_item: px(top),
        });
        let x = (width * rect.left + 32. - self.viewport.0 * 0.3).max(0.);
        let max_x = (self.content_width() - self.viewport.0).max(0.);
        self.horizontal
            .set_offset(gpui::point(px(-x.min(max_x)), px(0.)));
        cx.notify();
    }
    pub(super) fn search_bar(&self, cx: &mut Context<Self>) -> gpui::AnyElement {
        let count = self.search.matches.len();
        let current = self.search.current.map_or(0, |i| i + 1);
        let label = if self.search_input.read(cx).value().chars().count() > QUERY_LIMIT {
            format!("Maksimal {QUERY_LIMIT} karakter")
        } else if self.search.query.is_none() {
            "Cari kata atau frasa".into()
        } else if self.search.capped {
            format!(
                "{current}/{count}+ · batas hasil{}",
                if self.search.errors > 0 {
                    format!(" · {} halaman gagal dibaca", self.search.errors)
                } else {
                    String::new()
                }
            )
        } else if self.search.running {
            format!(
                "{current}/{count} · Mencari… {}/{}",
                self.search.scanned, self.navigation.count
            )
        } else if self.search.errors > 0 {
            format!(
                "{current}/{count} · {} halaman gagal dibaca",
                self.search.errors
            )
        } else if !self.search.has_text {
            "Tidak ada teks yang dapat dicari; OCR diperlukan".into()
        } else if count == 0 {
            "Tidak ditemukan".into()
        } else if self.search.wrapped {
            format!(
                "{current}/{count} · Kembali ke {}",
                if current == 1 { "awal" } else { "akhir" }
            )
        } else {
            format!("{current}/{count}")
        };
        let options = self.search.options;
        let weak = cx.weak_entity();
        let weak2 = weak.clone();
        gpui::div()
            .id("pdf-search-bar")
            .key_context("PdfSearch")
            .on_action(cx.listener(|this, _: &ClearTextSelection, window, cx| {
                if !this.edits_locked() {
                    this.close_search(window, cx);
                }
                cx.stop_propagation();
            }))
            .flex()
            .when(self.split_presentation, |el| el.flex_wrap())
            .items_center()
            .gap_2()
            .px_3()
            .py_1()
            .flex_shrink_0()
            .border_b_1()
            .border_color(cx.theme().border)
            .child(
                gpui::div().w(px(240.)).max_w_full().flex_shrink_0().child(
                    Input::new(&self.search_input)
                        .small()
                        .aria_label("Cari dalam PDF"),
                ),
            )
            .child(
                gpui::div()
                    .id("search-count")
                    .role(gpui::Role::Group)
                    .aria_label(label.clone())
                    .flex_1()
                    .min_w_0()
                    .when(self.split_presentation, |el| {
                        el.min_w(px(50.)).flex_shrink_0()
                    })
                    .text_xs()
                    .child(label),
            )
            .child(
                Button::new("search-previous")
                    .ghost()
                    .icon(IconName::ChevronUp)
                    .accessibility_label("Hasil sebelumnya")
                    .tooltip("Hasil sebelumnya · Shift+Enter / Shift+F3")
                    .disabled(count == 0)
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.move_search(true, cx);
                        this.search_input
                            .update(cx, |input, cx| input.focus(window, cx));
                    })),
            )
            .child(
                Button::new("search-next")
                    .ghost()
                    .icon(IconName::ChevronDown)
                    .accessibility_label("Hasil berikutnya")
                    .tooltip("Hasil berikutnya · Enter / F3")
                    .disabled(count == 0)
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.move_search(false, cx);
                        this.search_input
                            .update(cx, |input, cx| input.focus(window, cx));
                    })),
            )
            .child(
                Button::new("search-options")
                    .ghost()
                    .icon(IconName::Ellipsis)
                    .accessibility_label("Opsi pencarian")
                    .dropdown_menu(move |menu, _, _| {
                        let weak = weak.clone();
                        let weak2 = weak2.clone();
                        menu.item(
                            PopupMenuItem::new("Bedakan huruf besar-kecil")
                                .checked(options.case_sensitive)
                                .on_click(move |_, _, cx| {
                                    let _ = weak.update(cx, |this, cx| {
                                        this.search.options.case_sensitive =
                                            !options.case_sensitive;
                                        this.restart_search(cx);
                                    });
                                }),
                        )
                        .item(
                            PopupMenuItem::new("Kata utuh")
                                .checked(options.whole_words)
                                .on_click(move |_, _, cx| {
                                    let _ = weak2.update(cx, |this, cx| {
                                        this.search.options.whole_words = !options.whole_words;
                                        this.restart_search(cx);
                                    });
                                }),
                        )
                    }),
            )
            .child(
                Button::new("search-close")
                    .ghost()
                    .icon(IconName::X)
                    .accessibility_label("Tutup pencarian")
                    .tooltip("Tutup pencarian · Esc")
                    .on_click(cx.listener(|this, _, window, cx| this.close_search(window, cx))),
            )
            .into_any_element()
    }
}
