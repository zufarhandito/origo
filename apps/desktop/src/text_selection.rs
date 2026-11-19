use crate::*;
use pdf_application::text::{TextExtractor, TextPage, TextPosition, TextRequest, TextSelection};

pub(super) struct TextState {
    pub pages: ByteCache<usize, Arc<TextPage>>,
    pub selection: Option<TextSelection>,
    pub dragging: bool,
    pub pending: Option<(TextPosition, gpui::Point<gpui::Pixels>)>,
    pub sentence_anchor: Option<TextSelection>,
    pub busy: bool,
    pub errors: HashMap<usize, String>,
    pub loads: u64,
    copy: Option<CopyJob>,
}
const COPY_LIMIT: usize = 1024 * 1024;
struct CopyJob {
    selection: TextSelection,
    token: u64,
    next: usize,
    last: usize,
    result: String,
}
impl CopyJob {
    fn append(&mut self, page: &TextPage) -> bool {
        let part = page.copy_range(self.selection.range(self.next, page.chars.len()));
        let separator = usize::from(!self.result.is_empty());
        if self
            .result
            .len()
            .saturating_add(separator)
            .saturating_add(part.len())
            > COPY_LIMIT
        {
            return false;
        }
        if separator != 0 {
            self.result.push('\n');
        }
        self.result.push_str(&part);
        self.next += 1;
        true
    }
}
impl TextState {
    pub(super) fn new(budget: usize) -> Self {
        Self {
            pages: ByteCache::new(budget),
            ..Self::default()
        }
    }
    pub(super) fn copying(&self) -> bool {
        self.copy.is_some()
    }
    pub(super) fn copy_bytes(&self) -> usize {
        self.copy.as_ref().map_or(0, |j| j.result.len())
    }
    pub(super) fn cancel_copy(&mut self) {
        self.copy = None;
    }
}
impl Default for TextState {
    fn default() -> Self {
        Self {
            pages: ByteCache::new(16 * 1024 * 1024),
            selection: None,
            dragging: false,
            pending: None,
            sentence_anchor: None,
            busy: false,
            errors: HashMap::new(),
            loads: 0,
            copy: None,
        }
    }
}
impl DocumentSession {
    pub(super) fn request_text(&mut self, index: usize, cx: &mut Context<Self>) {
        if !self.visible_for_render
            || self.opening
            || self.text.busy
            || self.text.pages.contains(&index)
            || self.text.errors.contains_key(&index)
        {
            return;
        }
        let Some(workspace) = self.workspace.as_ref() else {
            return;
        };
        let Some(page) = workspace.pages().get(index) else {
            return;
        };
        let id = self.source_sequence;
        let rotation = page.rotation_delta;
        let Some(source) = workspace.resolve(page.source_id) else {
            return;
        };
        let request = TextRequest {
            path: source.path.clone(),
            page_index: page.source_page_index,
            fingerprint: source.fingerprint,
        };
        self.text.busy = true;
        self.text.loads += 1;
        let renderer = self.renderer.clone();
        let task = cx
            .background_executor()
            .spawn(async move { renderer.extract_text(&request) });
        cx.spawn(async move |this, cx| {
            let result = task.await;
            let _ = this.update(cx, |this, cx| {
                if this.source_sequence != id {
                    return;
                }
                this.text.busy = false;
                if !this.visible_for_render {
                    return;
                }
                match result {
                    Ok(mut page) => {
                        page.rotate(rotation);
                        let bytes = page.bytes();
                        this.text.pages.insert(index, Arc::new(page), bytes);
                        this.align_search_match(cx);
                    }
                    Err(error) => {
                        this.text.errors.insert(index, error.to_string());
                        if matches!(error, pdf_application::RenderError::SourceChanged) {
                            this.search.pause();
                            this.search.running = false;
                            this.search.matches.clear();
                            this.search.current = None;
                            this.invalidate_work();
                            this.clear_bitmaps(cx);
                            this.text = TextState::new(this.text_budget);
                            this.workspace.clear();
                            this.navigation = Navigation::default();
                            this.error = Some(error.to_string());
                            this.status = error.to_string();
                        }
                    }
                }
                this.resume_copy(cx);
                let next = this.visible.clone().find(|index| {
                    !this.text.pages.contains(index) && !this.text.errors.contains_key(index)
                });
                if let Some(index) = next {
                    this.request_text(index, cx);
                }
                this.report(cx);
                cx.notify();
            });
        })
        .detach();
    }
    pub(super) fn select_text(
        &mut self,
        page: usize,
        bounds: Bounds<gpui::Pixels>,
        position: gpui::Point<gpui::Pixels>,
        clicks: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let start = clicks > 0;
        if start {
            self.text.cancel_copy();
            self.text.selection = None;
            self.text.dragging = false;
            self.text.pending = None;
            self.text.sentence_anchor = None;
            self.focus.focus(window, cx);
            self.update_status();
        }
        let Some(text) = self.text.pages.get(&page) else {
            if start {
                self.status =
                    self.text.errors.get(&page).cloned().unwrap_or_else(|| {
                        "Menyiapkan teks halaman… Coba pilih lagi sebentar.".into()
                    });
                self.request_text(page, cx);
                cx.notify();
            }
            return;
        };
        let x = f32::from(position.x - bounds.origin.x) / f32::from(bounds.size.width);
        let y = f32::from(position.y - bounds.origin.y) / f32::from(bounds.size.height);
        if let Some(character) = text.hit(x, y, start) {
            let focus = TextPosition { page, character };
            if start {
                self.text.dragging = true;
                self.text.pending = Some((focus, position));
                if clicks >= 2 {
                    let range = text.sentence_range(character);
                    if !range.is_empty() {
                        let selection = TextSelection {
                            anchor: TextPosition {
                                page,
                                character: range.start,
                            },
                            focus: TextPosition {
                                page,
                                character: range.end - 1,
                            },
                        };
                        self.text.selection = Some(selection);
                        self.text.sentence_anchor = Some(selection);
                    }
                }
            } else if let Some(sentence) = self.text.sentence_anchor {
                let range = text.sentence_range(character);
                if !range.is_empty() {
                    self.text.selection = Some(if focus < sentence.anchor {
                        TextSelection {
                            anchor: sentence.focus,
                            focus: TextPosition {
                                page,
                                character: range.start,
                            },
                        }
                    } else {
                        TextSelection {
                            anchor: sentence.anchor,
                            focus: TextPosition {
                                page,
                                character: range.end - 1,
                            },
                        }
                    });
                }
            } else if let Some((anchor, origin)) = self.text.pending {
                let dx = f32::from(position.x - origin.x);
                let dy = f32::from(position.y - origin.y);
                if self.text.selection.is_some() || (dx * dx + dy * dy >= 16. && focus != anchor) {
                    self.text.selection = Some(TextSelection { anchor, focus });
                }
            }
            if self.text.selection.is_some() {
                self.status =
                    "Teks dipilih · Ctrl+C atau Salin untuk menyalin · Esc untuk batal".into();
            } else {
                self.update_status();
            }
        } else if start && text.chars.is_empty() {
            self.update_status();
        }
        self.report(cx);
        cx.notify();
    }
    pub(super) fn copy_text(&mut self, cx: &mut Context<Self>) {
        if !self.active || self.edits_locked() || self.text.copy.is_some() {
            return;
        }
        let Some(selection) = self.text.selection else {
            return;
        };
        let first = selection.anchor.page.min(selection.focus.page);
        let last = selection.anchor.page.max(selection.focus.page);
        self.text.copy = Some(CopyJob {
            selection,
            token: self.source_sequence,
            next: first,
            last,
            result: String::new(),
        });
        self.resume_copy(cx);
    }
    fn resume_copy(&mut self, cx: &mut Context<Self>) {
        let Some(mut job) = self.text.copy.take() else {
            return;
        };
        if !self.active
            || self.edits_locked()
            || job.token != self.source_sequence
            || !self
                .text
                .selection
                .is_some_and(|s| s.anchor == job.selection.anchor && s.focus == job.selection.focus)
        {
            return;
        }
        while job.next <= job.last {
            let index = job.next;
            let Some(page) = self.text.pages.get(&index) else {
                if let Some(error) = self.text.errors.get(&index) {
                    self.status = error.clone();
                    cx.notify();
                    return;
                }
                self.text.copy = Some(job);
                self.status = "Menyiapkan teks untuk disalin…".into();
                self.request_text(index, cx);
                self.report(cx);
                cx.notify();
                return;
            };
            if !job.append(&page) {
                self.status =
                    "Seleksi terlalu besar. Salin kurang dari 1 MiB teks sekaligus.".into();
                cx.notify();
                return;
            }
        }
        if !job.result.is_empty() {
            cx.write_to_clipboard(gpui::ClipboardItem::new_string(job.result));
            self.status = "Teks disalin ke clipboard".into();
        }
        self.report(cx);
        cx.notify();
    }
    pub(super) fn clear_selection(&mut self, cx: &mut Context<Self>) {
        self.text.cancel_copy();
        self.text.selection = None;
        self.text.dragging = false;
        self.text.pending = None;
        self.text.sentence_anchor = None;
        self.update_status();
        self.report(cx);
        cx.notify();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pdf_application::text::TextChar;
    fn page(text: &str) -> TextPage {
        TextPage {
            chars: text
                .chars()
                .map(|unicode| TextChar {
                    unicode,
                    bounds: None,
                })
                .collect(),
        }
    }
    fn job() -> CopyJob {
        CopyJob {
            selection: TextSelection {
                anchor: TextPosition {
                    page: 0,
                    character: 0,
                },
                focus: TextPosition {
                    page: 2,
                    character: 0,
                },
            },
            token: 1,
            next: 0,
            last: 2,
            result: String::new(),
        }
    }
    #[test]
    fn streaming_copy_preserves_unicode_order_and_bounds_bytes_including_separators() {
        let mut copy = job();
        assert!(copy.append(&page("é")));
        assert!(copy.append(&page("界")));
        assert!(copy.append(&page("!")));
        assert_eq!(copy.result, "é\n界\n!");
        let mut exact = job();
        exact.next = 1;
        exact.result = "x".repeat(COPY_LIMIT - 3);
        assert!(exact.append(&page("é")));
        assert_eq!(exact.result.len(), COPY_LIMIT);
        let mut overflow = job();
        overflow.next = 1;
        overflow.result = "x".repeat(COPY_LIMIT - 2);
        assert!(!overflow.append(&page("é")));
        assert_eq!(overflow.result.len(), COPY_LIMIT - 2);
        assert_eq!(overflow.next, 1);
    }
}
