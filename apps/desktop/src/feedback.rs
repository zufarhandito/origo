//! Modal error feedback shared by file, organizer and export operations.
use crate::*;
use gpui::component::{
    WindowExt,
    button::{Button, ButtonVariants},
    dialog::DialogFooter,
};

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
                    let close_view = weak.clone();
                    let copy_message = message.clone();
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
                                .child(gpui::base::SelectableText::new(
                                    "selectable-error-message",
                                    message.clone(),
                                )),
                        )
                        .centered()
                        .width(px(520.))
                        .footer(
                            DialogFooter::new()
                                .child(
                                    Button::new("copy-error-message")
                                        .ghost()
                                        .label("Salin pesan")
                                        .on_click(move |_, _, cx| {
                                            cx.write_to_clipboard(gpui::ClipboardItem::new_string(
                                                copy_message.clone(),
                                            ));
                                        }),
                                )
                                .child(
                                    Button::new("close-error-message")
                                        .primary()
                                        .label("Tutup")
                                        .on_click(move |_, window, cx| {
                                            window.close_dialog(cx);
                                            let _ = close_view.update(cx, |this, cx| {
                                                this.error_popup = false;
                                                cx.notify();
                                            });
                                        }),
                                ),
                        )
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
