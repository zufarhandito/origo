use crate::*;
use gpui::component::{
    Disableable,
    button::{Button, ButtonVariants},
};
impl DocumentSession {
    pub(super) fn signature_phone_panel(&self, cx: &mut Context<Self>) -> gpui::AnyElement {
        let state = &self.annotation.signature;
        let mut panel = gpui::div().flex().flex_col().gap_2().child(
            Button::new("signature-from-phone")
                .ghost()
                .label(if state.phone.is_some() {
                    "Batalkan QR"
                } else {
                    "Gambar dari HP"
                })
                .disabled(self.opening)
                .on_click(cx.listener(|s, _, _, cx| {
                    if s.annotation.signature.phone.is_some() {
                        s.annotation.signature.phone = None;
                        s.annotation.signature.phone_qr = None;
                        cx.notify();
                    } else {
                        s.begin_signature_phone(cx);
                    }
                })),
        );
        if let Some(phone) = &state.phone {
            panel = panel
                .child("Scan QR dengan HP di jaringan yang sama. Berlaku 5 menit, sekali kirim.")
                .child(
                    gpui::div()
                        .id("signature-phone-link")
                        .role(gpui::Role::Group)
                        .aria_label(format!("Alamat gambar HP: {}", phone.url))
                        .text_xs()
                        .child(phone.url.clone()),
                );
            if let Some(qr) = &state.phone_qr {
                panel = panel.child(gpui::img(qr.clone()).size(px(phone.qr.width as f32)));
            }
            panel=panel.child("Menunggu gambar dari HP. Gunakan LAN tepercaya; koneksi lokal HTTP tanpa enkripsi. Jika gagal terhubung, periksa izin firewall jaringan privat.");
        }
        if let Some(status) = &state.phone_status {
            panel = panel.child(status.clone());
        }
        panel.into_any_element()
    }
    fn begin_signature_phone(&mut self, cx: &mut Context<Self>) {
        if self.edits_locked() || self.opening {
            return;
        }
        self.annotation.signature.phone_status = None;
        match crate::signature_phone::PhoneSession::start() {
            Ok(phone) => {
                self.annotation.signature.phone_qr = crate::signatures::asset_preview(&phone.qr);
                self.annotation.signature.phone = Some(phone);
                self.annotation.signature.phone_revision += 1;
                let revision = self.annotation.signature.phone_revision;
                let instance = self.workspace_instance;
                cx.spawn(async move|this,cx|{loop {
                    cx.background_executor().timer(std::time::Duration::from_millis(250)).await;
                    let keep=this.update(cx,|s,cx|{
                        let state=&mut s.annotation.signature;
                        if state.phone_revision!=revision{return false;}
                        if !state.open||!s.active||s.workspace_instance!=instance||state.method!=crate::signatures::SignatureMethod::Draw {state.phone=None;state.phone_qr=None;return false;}
                        let result=state.phone.as_ref().and_then(|phone|phone.poll());
                        if let Some(result)=result {
                            state.phone=None;state.phone_qr=None;
                            match result {
                                Ok(strokes)=>{
                                    state.strokes=strokes;
                                    state.asset=crate::signatures::draw_asset_with(&state.strokes,crate::signatures::signature_color(state.color),state.thickness).map(Arc::new);
                                    state.preview=state.asset.as_deref().and_then(crate::signatures::asset_preview);
                                    state.error=None;state.phone_status=Some("Gambar diterima dari HP. Periksa preview lalu pilih Gunakan tanda tangan.".into());
                                },Err(error)=>state.phone_status=Some(error),
                            }
                            cx.notify();return false;
                        }
                        state.phone.is_some()
                    });
                    if !matches!(keep,Ok(true)){break;}
                }}).detach();
            }
            Err(error) => self.annotation.signature.phone_status = Some(error),
        }
        cx.notify();
    }
}
