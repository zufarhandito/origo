use crate::*;
use pdf_domain::{Command, PageId, SourceDocument};

pub(super) struct PageDrag(pub shell::TabId, pub Vec<PageId>);
pub(super) struct DragLabel;
impl gpui::Render for DragLabel {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl gpui::IntoElement {
        gpui::div()
            .bg(gpui::rgb(0xdddddd))
            .p_2()
            .child("Pindahkan halaman")
    }
}
impl DocumentSession {
    pub(super) fn select_page(
        &mut self,
        index: usize,
        toggle: bool,
        extend: bool,
        cx: &mut Context<Self>,
    ) {
        if self.edits_locked() || self.mode != ViewerMode::Organize {
            return;
        }
        let Some(mut w) = self.workspace.as_mut() else {
            return;
        };
        let Some(page) = w.pages().get(index).copied() else {
            return;
        };
        let mut ids = if toggle {
            w.selected().clone()
        } else {
            Default::default()
        };
        if extend {
            let anchor = self
                .page_anchor
                .and_then(|id| w.pages().iter().position(|p| p.id == id))
                .unwrap_or(index);
            ids.extend(
                w.pages()[anchor.min(index)..=anchor.max(index)]
                    .iter()
                    .map(|p| p.id),
            );
        } else {
            if !toggle || !ids.remove(&page.id) {
                ids.insert(page.id);
            }
            self.page_anchor = Some(page.id);
        }
        w.select(ids);
        drop(w);
        self.go_to(index as u16, cx);
        cx.notify();
    }
    pub(super) fn edit(&mut self, command: Command, cx: &mut Context<Self>) {
        let importing = matches!(command, Command::Import(_));
        if self.edits_locked() || self.opening || (!importing && self.mode != ViewerMode::Organize)
        {
            return;
        }
        let Some(mut w) = self.workspace.as_mut() else {
            return;
        };
        let result = w.execute(command);
        drop(w);
        match result {
            Ok(true) => {
                if importing {
                    self.mode = ViewerMode::Organize;
                    self.sidebar = true;
                }
                self.edited(cx);
            }
            Ok(false) => {}
            Err(e) => {
                self.status = e.to_string();
                self.show_error_popup(
                    if importing {
                        "PDF belum dapat diimpor"
                    } else {
                        "Halaman belum dapat diubah"
                    },
                    e.to_string(),
                    cx,
                );
                cx.notify();
            }
        }
    }
    pub(super) fn edited(&mut self, cx: &mut Context<Self>) {
        self.refresh_after_edit(true, cx);
    }
    pub(super) fn refresh_after_edit(
        &mut self,
        clear_read_selection: bool,
        cx: &mut Context<Self>,
    ) {
        self.annotation.draft = None;
        self.annotation.signature.gesture = None;
        let top = self.page_list.logical_scroll_top();
        let old_height = self
            .page_list
            .bounds_for_item(top.item_ix)
            .map(|b| f32::from(b.size.height));
        let fraction = old_height
            .filter(|h| *h > 0.)
            .map_or(0., |h| f32::from(top.offset_in_item) / h);
        let pan = f32::from(self.horizontal.offset().x)
            / (self.content_width() - self.viewport.0).max(1.);
        self.pending_reading = None;
        self.page_dragging = false;
        if clear_read_selection
            && self.mode == ViewerMode::Read
            && let Some(mut w) = self.workspace.as_mut()
        {
            w.select([]);
        }
        self.source_sequence += 1; // Reject positional text results from before this edit.
        self.text = text_selection::TextState::new(self.text_budget);
        self.restart_search(cx);
        self.search.preserve_reading = true;
        self.invalidate_work();
        self.clear_bitmaps(cx);
        if let Some(w) = self.workspace.as_ref() {
            self.navigation.count = w.pages().len() as u16;
            self.navigation.current = self
                .current_page_id
                .and_then(|id| w.pages().iter().position(|p| p.id == id))
                .map(|i| i as u16)
                .unwrap_or_else(|| {
                    self.navigation
                        .current
                        .min(self.navigation.count.saturating_sub(1))
                });
            if self
                .page_anchor
                .is_some_and(|id| !w.pages().iter().any(|p| p.id == id))
            {
                self.page_anchor = None;
            }
        }
        self.visible = if self.navigation.count == 0 {
            0..0
        } else {
            let i = usize::from(self.navigation.current);
            i..i + 1
        };
        self.reset_layout();
        let index = usize::from(self.navigation.current);
        if let Some(height) = self
            .workspace
            .as_ref()
            .and_then(|w| w.geometry(index).ok())
            .map(|g| self.zoom.size(g, self.viewport).1 + 48.)
        {
            self.page_list.scroll_to(gpui::ListOffset {
                item_ix: index,
                offset_in_item: px(fraction.clamp(-1., 1.) * height),
            });
        }
        self.remember_view_page();
        self.horizontal.set_offset(gpui::point(
            px(pan.clamp(-1., 0.) * (self.content_width() - self.viewport.0).max(0.)),
            px(0.),
        ));
        self.observed_model = self.workspace.signature();
        self.schedule_current(cx);
        self.update_status();
        self.report(cx);
        cx.notify();
    }
    pub(super) fn selected_ids(&self) -> Vec<PageId> {
        self.workspace
            .as_ref()
            .map(|w| {
                w.pages()
                    .iter()
                    .filter(|p| w.selected().contains(&p.id))
                    .map(|p| p.id)
                    .collect()
            })
            .unwrap_or_default()
    }
    pub(super) fn history(&mut self, redo: bool, cx: &mut Context<Self>) {
        if self.edits_locked() || self.opening || self.mode == ViewerMode::Convert {
            return;
        }
        let changed = self
            .workspace
            .as_mut()
            .is_some_and(|mut w| if redo { w.redo() } else { w.undo() });
        if changed {
            self.edited(cx);
        }
    }
    pub(super) fn move_selection(&mut self, later: bool, cx: &mut Context<Self>) {
        let Some(w) = self.workspace.as_ref() else {
            return;
        };
        let positions: Vec<_> = w
            .pages()
            .iter()
            .enumerate()
            .filter(|(_, p)| w.selected().contains(&p.id))
            .map(|(i, _)| i)
            .collect();
        if let (Some(first), Some(last)) = (positions.first(), positions.last()) {
            let target = if later {
                (last + 2).min(w.pages().len())
            } else {
                first.saturating_sub(1)
            };
            drop(w);
            self.edit(
                Command::Move {
                    ids: self.selected_ids(),
                    target,
                },
                cx,
            );
        }
    }
    pub(super) fn page_step(&mut self, next: bool, extend: bool, cx: &mut Context<Self>) {
        if let Some(i) = self.navigation.target(if next {
            Direction::Next
        } else {
            Direction::Previous
        }) {
            self.select_page(usize::from(i), false, extend, cx);
        }
    }
    pub(super) fn import_pdf(&mut self, cx: &mut Context<Self>) {
        if self.edits_locked() || self.opening || self.workspace.is_none() {
            return;
        }
        let dialog = cx.prompt_for_paths(gpui::PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some("Impor PDF (tambahkan halaman)".into()),
        });
        self.opening = true;
        cx.notify();
        cx.spawn(async move |this, cx| {
            let result = dialog.await;
            let _ = this.update(cx, |this, cx| {
                this.opening = false;
                if let Ok(Ok(Some(paths))) = result
                    && let Some(path) = paths.into_iter().next()
                {
                    this.import_path(path, cx);
                }
                this.report(cx);
                cx.notify();
            });
        })
        .detach();
    }
    fn import_path(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        if self.edits_locked() || self.opening || self.workspace.is_none() {
            return;
        }
        self.opening = true;
        self.source_sequence += 1;
        let Some(id) = self.workspace.next_source() else {
            self.opening = false;
            self.show_error_popup(
                "PDF belum dapat diimpor",
                "Identitas sumber habis.".into(),
                cx,
            );
            return;
        };
        self.workspace.set_importing(true);
        let model = self.workspace.clone();
        let renderer = self.renderer.clone();
        let task = cx.background_executor().spawn(async move {
            use pdf_application::PdfInspector;
            renderer.inspect(&path).map(|info| SourceDocument {
                id,
                path: info.path,
                fingerprint: info.fingerprint,
                geometries: info.pages,
                export_support: pdf_application::ExportSupport::NotPreflighted,
                annotations: info.annotations,
            })
        });
        cx.spawn(async move |this, cx| {
            let result = task.await;
            model.set_importing(false);
            let _ = this.update(cx, |this, cx| {
                this.opening = false;
                // An earlier positional text request may have completed during inspection.
                this.text = text_selection::TextState::new(this.text_budget);
                match result {
                    Ok(source) => this.edit(Command::Import(source), cx),
                    Err(e) => {
                        this.status = e.to_string();
                        this.show_error_popup("PDF belum dapat diimpor", e.to_string(), cx);
                    }
                }
                this.schedule_current(cx);
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }
}
