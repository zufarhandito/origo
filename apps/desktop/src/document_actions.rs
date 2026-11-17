use crate::*;
impl DocumentSession {
    pub(super) fn document_actions(element: gpui::Div, cx: &mut Context<Self>) -> gpui::Div {
        element
            .key_context("PdfViewer")
            .on_action(cx.listener(|this, _: &FindText, window, cx| this.show_search(window, cx)))
            .on_action(cx.listener(|this, _: &FindNext, window, cx| {
                if this.search.open {
                    this.move_search(false, cx);
                } else {
                    this.show_search(window, cx);
                }
            }))
            .on_action(cx.listener(|this, _: &FindPrevious, window, cx| {
                if this.search.open {
                    this.move_search(true, cx);
                } else {
                    this.show_search(window, cx);
                }
            }))
            .on_action(cx.listener(|this, _: &CopyText, _, cx| {
                if this.mode != ViewerMode::Convert {
                    this.copy_text(cx);
                }
            }))
            .on_action(cx.listener(|this, _: &ClearTextSelection, window, cx| {
                if this.edits_locked() {
                    return;
                }
                if this.search.open {
                    this.close_search(window, cx);
                    return;
                }
                if this.mode == ViewerMode::Convert {
                    this.set_mode(ViewerMode::Read, window, cx);
                    return;
                }
                cx.stop_active_drag(window);
                this.page_dragging = false;
                if this.mode == ViewerMode::Organize {
                    this.set_mode(ViewerMode::Read, window, cx);
                } else {
                    this.clear_selection(cx);
                }
            }))
            .on_mouse_up(
                gpui::MouseButton::Left,
                cx.listener(|this, _, _, cx| {
                    if this.page_dragging {
                        this.page_dragging = false;
                        cx.notify();
                    }
                }),
            )
            .on_action(cx.listener(Self::open))
            .on_action(cx.listener(|this, _: &SaveAs, _, cx| {
                this.start_export(pdf_application::export::ExportScope::All, cx)
            }))
            .on_action(cx.listener(|this, _: &ExtractPages, _, cx| {
                this.start_export(pdf_application::export::ExportScope::Selected, cx)
            }))
            .on_action(cx.listener(|this, _: &ImportPdf, _, cx| this.import_pdf(cx)))
            .on_action(cx.listener(|this, _: &UndoEdit, _, cx| this.history(false, cx)))
            .on_action(cx.listener(|this, _: &RedoEdit, _, cx| this.history(true, cx)))
            .on_action(
                cx.listener(|this, _: &FirstPage, _, cx| this.navigate(Direction::First, cx)),
            )
            .on_action(
                cx.listener(|this, _: &PreviousPage, _, cx| this.navigate(Direction::Previous, cx)),
            )
            .on_action(cx.listener(|this, _: &NextPage, _, cx| this.navigate(Direction::Next, cx)))
            .on_action(cx.listener(|this, _: &LastPage, _, cx| this.navigate(Direction::Last, cx)))
            .on_action(cx.listener(|this, _: &ZoomIn, _, cx| this.zoom_step(25, cx)))
            .on_action(cx.listener(|this, _: &ZoomOut, _, cx| this.zoom_step(-25, cx)))
            .on_action(cx.listener(|this, _: &FitWidth, _, cx| this.set_zoom(Zoom::FitWidth, cx)))
            .on_action(cx.listener(|this, _: &FitPage, _, cx| this.set_zoom(Zoom::FitPage, cx)))
            .on_action(cx.listener(|this, _: &ToggleSidebar, _, cx| {
                this.sidebar = !this.sidebar;
                cx.notify();
            }))
            .on_action(cx.listener(|this, _: &FocusPage, window, cx| {
                if this.mode == ViewerMode::Convert {
                    this.set_mode(ViewerMode::Read, window, cx);
                }
                this.jump.update(cx, |state, cx| {
                    state.set_value("", window, cx);
                    state.focus(window, cx);
                });
            }))
    }
}
