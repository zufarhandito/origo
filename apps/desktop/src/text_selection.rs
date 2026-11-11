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
        }
    }
}
impl Viewer {
    pub(super) fn request_text(&mut self, index: usize, cx: &mut Context<Self>) {
        if self.opening
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
                match result {
                    Ok(mut page) => {
                        page.rotate(rotation);
                        let bytes = page.bytes();
                        this.text.pages.insert(index, Arc::new(page), bytes);
                    }
                    Err(error) => {
                        this.text.errors.insert(index, error.to_string());
                        if matches!(error, pdf_application::RenderError::SourceChanged) {
                            this.invalidate_work();
                            this.clear_bitmaps(cx);
                            this.text = TextState::default();
                            this.workspace = None;
                            this.navigation = Navigation::default();
                            this.error = Some(error.to_string());
                            this.status = error.to_string();
                        }
                    }
                }
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
            self.status =
                "Halaman ini tidak memiliki teks yang dapat dipilih. Scan gambar membutuhkan OCR."
                    .into();
        }
        self.report(cx);
        cx.notify();
    }
    pub(super) fn copy_text(&mut self, cx: &mut Context<Self>) {
        let Some(selection) = self.text.selection else {
            return;
        };
        let first = selection.anchor.page.min(selection.focus.page);
        let last = selection.anchor.page.max(selection.focus.page);
        let mut result = String::new();
        for index in first..=last {
            let Some(page) = self.text.pages.get(&index) else {
                self.status =
                    "Sebagian teks seleksi tidak ada di cache. Pilih rentang yang lebih kecil."
                        .into();
                cx.notify();
                return;
            };
            if !result.is_empty() {
                result.push('\n');
            }
            result.push_str(&page.copy_range(selection.range(index, page.chars.len())));
            if result.len() > 1024 * 1024 {
                self.status =
                    "Seleksi terlalu besar. Salin kurang dari 1 MiB teks sekaligus.".into();
                cx.notify();
                return;
            }
        }
        if !result.is_empty() {
            cx.write_to_clipboard(gpui::ClipboardItem::new_string(result));
            self.status = "Teks disalin ke clipboard".into();
            cx.notify();
        }
    }
    pub(super) fn clear_selection(&mut self, cx: &mut Context<Self>) {
        self.text.selection = None;
        self.text.dragging = false;
        self.text.pending = None;
        self.text.sentence_anchor = None;
        self.update_status();
        self.report(cx);
        cx.notify();
    }
}
