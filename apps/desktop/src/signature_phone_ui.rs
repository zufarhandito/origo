use crate::*;
use gpui::assets::IconName;
use gpui::component::{
    ActiveTheme, Disableable, Icon, Sizable, StyledExt, button::Button, spinner::Spinner,
};
use std::time::Duration;

impl DocumentSession {
    pub(super) fn signature_phone_button(&self, cx: &mut Context<Self>) -> Button {
        Button::new("signature-from-phone")
            .outline()
            .small()
            .icon(IconName::Smartphone)
            .label("Gambar dari HP")
            .disabled(self.opening)
            .on_click(cx.listener(|s, _, _, cx| s.begin_signature_phone(cx)))
    }

    pub(super) fn signature_phone_panel(&self, cx: &mut Context<Self>) -> gpui::AnyElement {
        let state = &self.annotation.signature;
        let Some(phone) = &state.phone else {
            return gpui::div().into_any_element();
        };
        let colors = cx.theme().colors;
        gpui::div()
            .flex()
            .flex_col()
            .gap_3()
            .child(
                gpui::div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap_2()
                    .child(
                        gpui::div()
                            .id("signature-phone-title")
                            .role(gpui::Role::Group)
                            .aria_label("Pindai QR dengan kamera HP")
                            .text_sm()
                            .font_semibold()
                            .child("Pindai QR dengan kamera HP"),
                    )
                    .child(
                        Button::new("signature-phone-cancel")
                            .outline()
                            .small()
                            .icon(IconName::X)
                            .label("Batalkan QR")
                            .on_click(cx.listener(|s, _, _, cx| {
                                s.cancel_signature_phone();
                                cx.notify();
                            })),
                    ),
            )
            .child(
                gpui::div()
                    .flex()
                    .flex_col()
                    .items_center()
                    .gap_2()
                    .when_some(state.phone_qr.clone(), |el, qr| {
                        el.child(
                            gpui::div()
                                .id("signature-phone-qr")
                                .role(gpui::Role::Group)
                                .aria_label("Kode QR gambar dari HP")
                                .rounded_lg()
                                .border_1()
                                .border_color(colors.border)
                                .bg(gpui::white())
                                .overflow_hidden()
                                .child(gpui::img(qr).size(px(phone.qr.width as f32))),
                        )
                    })
                    .child(
                        Button::new("signature-phone-copy-link")
                            .outline()
                            .small()
                            .icon(IconName::Copy)
                            .label("Salin tautan")
                            .on_click(cx.listener(|s, _, _, cx| {
                                let state = &mut s.annotation.signature;
                                if let Some(phone) = &state.phone {
                                    cx.write_to_clipboard(gpui::ClipboardItem::new_string(
                                        phone.url.clone(),
                                    ));
                                    state.phone_status = Some("Tautan disalin.".into());
                                    cx.notify();
                                }
                            })),
                    )
                    .when_some(state.phone_status.clone(), |el, status| {
                        el.child(
                            gpui::div()
                                .id("signature-phone-feedback")
                                .role(gpui::Role::Group)
                                .aria_label(status.clone())
                                .text_xs()
                                .text_color(colors.muted_foreground)
                                .child(status),
                        )
                    })
                    .child(
                        gpui::div()
                            .id("signature-phone-waiting")
                            .role(gpui::Role::Group)
                            .aria_label("Menunggu gambar dari HP...")
                            .flex()
                            .items_center()
                            .gap_2()
                            .text_sm()
                            .text_color(colors.muted_foreground)
                            .child(Spinner::new().xsmall().color(colors.muted_foreground))
                            .child("Menunggu gambar dari HP..."),
                    ),
            )
            .child(
                gpui::div()
                    .flex()
                    .items_start()
                    .gap_2()
                    .p_3()
                    .rounded_lg()
                    .bg(colors.muted)
                    .text_xs()
                    .text_color(colors.muted_foreground)
                    .child(Icon::new(IconName::Info).small().flex_shrink_0())
                    .child(gpui::div().flex_1().min_w_0().whitespace_normal().child(
                        "Berlaku 5 menit untuk sekali kirim. Gunakan jaringan Wi-Fi atau LAN tepercaya yang sama. Periksa firewall jika gagal terhubung.",
                    )),
            )
            .into_any_element()
    }

    pub(super) fn cancel_signature_phone(&mut self) {
        let state = &mut self.annotation.signature;
        state.phone = None;
        state.phone_qr = None;
        state.phone_status = None;
        state.phone_revision = state.phone_revision.wrapping_add(1);
    }

    fn begin_signature_phone(&mut self, cx: &mut Context<Self>) {
        if self.edits_locked() || self.opening {
            return;
        }
        let phone = match crate::signature_phone::PhoneSession::start() {
            Ok(phone) => phone,
            Err(error) => {
                self.annotation.signature.phone_status = Some(error);
                cx.notify();
                return;
            }
        };
        self.cancel_signature_phone();
        let state = &mut self.annotation.signature;
        state.phone_qr = crate::signatures::asset_preview(&phone.qr);
        state.phone = Some(phone);
        let revision = state.phone_revision;
        let instance = self.workspace_instance;
        cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(250))
                    .await;
                let keep = this.update(cx, |s, cx| s.poll_signature_phone(instance, revision, cx));
                if !matches!(keep, Ok(true)) {
                    break;
                }
            }
        })
        .detach();
        cx.notify();
    }

    fn poll_signature_phone(
        &mut self,
        instance: u64,
        revision: u64,
        cx: &mut Context<Self>,
    ) -> bool {
        let state = &self.annotation.signature;
        if state.phone_revision != revision || state.phone.is_none() {
            return false;
        }
        if !state.open
            || !self.active
            || self.workspace_instance != instance
            || state.method != crate::signatures::SignatureMethod::Draw
        {
            self.cancel_signature_phone();
            cx.notify();
            return false;
        }
        let Some(result) = state.phone.as_ref().and_then(|phone| phone.poll()) else {
            return true;
        };

        self.cancel_signature_phone();
        let state = &mut self.annotation.signature;
        state.phone_status = Some(match result {
            Ok(strokes) => {
                state.strokes = strokes;
                state.asset = crate::signatures::draw_asset_with(
                    &state.strokes,
                    crate::signatures::signature_color(state.color),
                    state.thickness,
                )
                .map(Arc::new);
                state.preview = state
                    .asset
                    .as_deref()
                    .and_then(crate::signatures::asset_preview);
                state.error = None;
                "Gambar diterima dari HP. Periksa hasilnya lalu pilih Gunakan Tanda Tangan.".into()
            }
            Err(error) => error,
        });
        cx.notify();
        false
    }
}
