use crate::*;
use pdf_domain::{Command, PageId, SourceDocument};

pub(super) struct PageDrag(pub Vec<PageId>);
pub(super) struct DragLabel;
impl gpui::Render for DragLabel {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl gpui::IntoElement {
        gpui::div()
            .bg(gpui::rgb(0xdddddd))
            .p_2()
            .child("Pindahkan halaman")
    }
}
impl Viewer {
    pub(super) fn select_page(
        &mut self,
        index: usize,
        toggle: bool,
        extend: bool,
        cx: &mut Context<Self>,
    ) {
        if self.mode != ViewerMode::Organize {
            return;
        }
        let Some(w) = self.workspace.as_mut() else {
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
        self.go_to(index as u16, cx);
        cx.notify();
    }
    pub(super) fn edit(&mut self, command: Command, cx: &mut Context<Self>) {
        let importing = matches!(command, Command::Import(_));
        if self.opening || (!importing && self.mode != ViewerMode::Organize) {
            return;
        }
        let Some(w) = self.workspace.as_mut() else {
            return;
        };
        match w.execute(command) {
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
                cx.notify();
            }
        }
    }
    pub(super) fn edited(&mut self, cx: &mut Context<Self>) {
        self.page_dragging = false;
        if self.mode == ViewerMode::Read
            && let Some(w) = &mut self.workspace
        {
            w.select([]);
        }
        self.source_sequence += 1; // Reject positional text results from before this edit.
        self.text = text_selection::TextState::default();
        self.invalidate_work();
        self.clear_bitmaps(cx);
        if let Some(w) = &self.workspace {
            self.navigation.count = w.pages().len() as u16;
            self.navigation.current = self
                .navigation
                .current
                .min(self.navigation.count.saturating_sub(1));
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
        if self.opening || self.mode == ViewerMode::Convert {
            return;
        }
        if let Some(w) = &mut self.workspace {
            let changed = if redo { w.redo() } else { w.undo() };
            if changed {
                self.edited(cx);
            }
        }
    }
    pub(super) fn move_selection(&mut self, later: bool, cx: &mut Context<Self>) {
        let Some(w) = &self.workspace else {
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
        if self.opening || self.workspace.is_none() {
            return;
        }
        let dialog = cx.prompt_for_paths(gpui::PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some("Impor PDF (tambahkan halaman)".into()),
        });
        cx.spawn(async move |this, cx| {
            if let Ok(Ok(Some(paths))) = dialog.await
                && let Some(path) = paths.into_iter().next()
            {
                let _ = this.update(cx, |this, cx| this.import_path(path, cx));
            }
        })
        .detach();
    }
    fn import_path(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        if self.opening || self.workspace.is_none() {
            return;
        }
        self.opening = true;
        self.source_sequence += 1;
        let id = pdf_domain::SourceId(self.source_sequence);
        let renderer = self.renderer.clone();
        let task = cx.background_executor().spawn(async move {
            use pdf_application::PdfInspector;
            renderer.inspect(&path).map(|info| SourceDocument {
                id,
                path: info.path,
                fingerprint: info.fingerprint,
                geometries: info.pages,
                export_support: pdf_application::ExportSupport::NotPreflighted,
            })
        });
        cx.spawn(async move |this, cx| {
            let result = task.await;
            let _ = this.update(cx, |this, cx| {
                this.opening = false;
                // An earlier positional text request may have completed during inspection.
                this.text = text_selection::TextState::default();
                match result {
                    Ok(source) => this.edit(Command::Import(source), cx),
                    Err(e) => {
                        this.status = e.to_string();
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
