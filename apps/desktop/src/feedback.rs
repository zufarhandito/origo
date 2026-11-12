//! Modal error feedback shared by file, organizer and export operations.
use crate::*;
use gpui::component::{WindowExt, dialog::DialogButtonProps};

impl Viewer {
    pub(super) fn show_error_popup(
        &mut self,
        title: &'static str,
        message: String,
        cx: &mut Context<Self>,
    ) {
        // Concurrent background failures must not stack modal dialogs.
        if self.error_popup {
            return;
        }
        self.error_popup = true;
        self.text.dragging = false;
        self.text.pending = None;
        self.text.sentence_anchor = None;
        let handle = self.window_handle;
        let weak = cx.weak_entity();
        cx.defer(move |cx| {
            let _ = handle.update(cx, |_, window, cx| {
                window.open_alert_dialog(cx, move |dialog, _, _| {
                    let weak = weak.clone();
                    dialog
                        .title(
                            gpui::div()
                                .id("operation-error-title")
                                .role(gpui::Role::Group)
                                .aria_label(title)
                                .child(title),
                        )
                        .icon(gpui::component::Icon::new(
                            gpui::assets::IconName::TriangleAlert,
                        ))
                        .description(
                            gpui::div()
                                .id("operation-error-message")
                                .role(gpui::Role::Group)
                                .aria_label(message.clone())
                                .child(message.clone()),
                        )
                        .centered()
                        .width(px(520.))
                        .button_props(DialogButtonProps::default().ok_text("Tutup"))
                        .on_close(move |_, _, cx| {
                            let _ = weak.update(cx, |this, cx| {
                                this.error_popup = false;
                                cx.notify();
                            });
                        })
                });
            });
        });
    }
}
