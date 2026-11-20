//! Local template actions, serialized off the UI thread.
use crate::signature_library::Entry;
use crate::signatures::{SignatureKind, SignatureMethod};
use crate::*;
use gpui::component::{
    Disableable,
    button::{Button, ButtonVariants},
    input::Input,
};
pub(super) enum LibraryEdit {
    Add(Entry),
    Rename(String, String),
    Delete(String),
}
impl DocumentSession {
    pub(super) fn signature_library_panel(&mut self, cx: &mut Context<Self>) -> gpui::AnyElement {
        let state = &self.annotation.signature;
        let mut panel = gpui::div()
            .flex()
            .flex_col()
            .gap_2()
            .child("Template lokal (opsional)");
        if state.library_loading {
            panel = panel.child("Memuat pustaka...");
        }
        panel = panel.children(state.library.iter().enumerate().map(|(index, entry)| {
            let entry = entry.clone();
            Button::new(("signature-template", index))
                .ghost()
                .label(entry.label.clone())
                .accessibility_label(format!("Pakai template: {}", entry.label))
                .on_click(cx.listener(move |s, _, window, cx| {
                    s.annotation.signature.kind = match entry.kind {
                        1 => SignatureKind::Initials,
                        2 => SignatureKind::Stamp,
                        _ => SignatureKind::Signature,
                    };
                    s.annotation.signature.phone = None;
                    s.annotation.signature.phone_qr = None;
                    s.annotation.signature.method = SignatureMethod::Image;
                    s.annotation.signature.remove_white = false;
                    s.annotation.signature.asset = Some(entry.asset.clone());
                    s.annotation.signature.imported = Some(entry.asset.clone());
                    s.annotation.signature.strokes.clear();
                    s.annotation.signature.error = None;
                    s.annotation.signature.preview = crate::signatures::asset_preview(&entry.asset);
                    s.annotation.signature.library_selected = Some(entry.label.clone());
                    if let Some(input) = s.annotation.signature.library_label.clone() {
                        input.update(cx, |i, cx| i.set_value(entry.label.clone(), window, cx));
                    }
                    cx.notify();
                }))
        }));
        if let Some(input) = &state.library_label {
            panel = panel.child(Input::new(input).aria_label("Nama template tanda tangan"));
        }
        panel = panel.child(
            gpui::div()
                .flex()
                .gap_2()
                .child(
                    Button::new("signature-template-save")
                        .ghost()
                        .label("Simpan template")
                        .disabled(
                            !state.library_ready
                                || self.opening
                                || state.asset.is_none()
                                || state.library.len() >= 8,
                        )
                        .on_click(cx.listener(|s, _, _, cx| {
                            let Some(asset) = s.annotation.signature.asset.clone() else {
                                return;
                            };
                            let label = s
                                .annotation
                                .signature
                                .library_label
                                .as_ref()
                                .map(|i| i.read(cx).value().trim().to_owned())
                                .unwrap_or_default();
                            let entry = Entry {
                                label,
                                kind: s.annotation.signature.kind as usize,
                                asset,
                            };
                            s.edit_signature_library(LibraryEdit::Add(entry), cx);
                        })),
                )
                .child(
                    Button::new("signature-template-rename")
                        .ghost()
                        .label("Ubah nama template")
                        .disabled(
                            self.opening
                                || state.library_selected.is_none()
                                || !state.library_ready,
                        )
                        .on_click(cx.listener(|s, _, _, cx| {
                            if let Some(old) = s.annotation.signature.library_selected.clone() {
                                let new = s
                                    .annotation
                                    .signature
                                    .library_label
                                    .as_ref()
                                    .map(|i| i.read(cx).value().trim().to_owned())
                                    .unwrap_or_default();
                                s.edit_signature_library(LibraryEdit::Rename(old, new), cx);
                            }
                        })),
                )
                .child(
                    Button::new("signature-template-delete")
                        .ghost()
                        .label("Hapus template")
                        .disabled(
                            self.opening
                                || state.library_selected.is_none()
                                || !state.library_ready,
                        )
                        .on_click(cx.listener(|s, _, _, cx| {
                            if let Some(label) = s.annotation.signature.library_selected.clone() {
                                s.edit_signature_library(LibraryEdit::Delete(label), cx);
                            }
                        })),
                ),
        );
        panel.into_any_element()
    }
    pub(super) fn load_signature_library(&mut self, cx: &mut Context<Self>) {
        if self.annotation.signature.library_loading {
            return;
        }
        let Some(dir) = self.resources.templates_dir.clone() else {
            return;
        };
        let lock = self.resources.templates_lock.clone();
        self.annotation.signature.library_loading = true;
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_executor()
                .spawn(async move {
                    let _guard = lock
                        .lock()
                        .map_err(|_| anyhow::anyhow!("Pustaka terkunci"))?;
                    crate::signature_library::load(&dir)
                })
                .await;
            let _ = this.update(cx, |s, cx| {
                s.annotation.signature.library_loading = false;
                match result {
                    Ok(entries) => {
                        s.annotation.signature.library = entries;
                        s.annotation.signature.library_ready = true;
                    }
                    Err(e) => {
                        s.annotation.signature.library_ready = false;
                        s.annotation.signature.error = Some(format!(
                            "Pustaka tidak dapat dibaca; file lama dipertahankan: {e}"
                        ));
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }
    pub(super) fn edit_signature_library(&mut self, edit: LibraryEdit, cx: &mut Context<Self>) {
        if self.opening || !self.annotation.signature.library_ready {
            return;
        }
        let Some(dir) = self.resources.templates_dir.clone() else {
            return;
        };
        let lock = self.resources.templates_lock.clone();
        self.opening = true;
        cx.notify();
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_executor()
                .spawn(async move {
                    let _guard = lock
                        .lock()
                        .map_err(|_| anyhow::anyhow!("Pustaka terkunci"))?;
                    let mut entries = crate::signature_library::load(&dir)?;
                    match edit {
                        LibraryEdit::Add(entry) => entries.push(entry),
                        LibraryEdit::Rename(old, new) => {
                            let entry =
                                entries.iter_mut().find(|e| e.label == old).ok_or_else(|| {
                                    anyhow::anyhow!("Template sudah berubah. Buka editor kembali.")
                                })?;
                            entry.label = new;
                        }
                        LibraryEdit::Delete(label) => entries.retain(|e| e.label != label),
                    }
                    crate::signature_library::save(&dir, &entries)?;
                    Ok::<_, anyhow::Error>(entries)
                })
                .await;
            let _ = this.update(cx, |s, cx| {
                s.opening = false;
                match result {
                    Ok(entries) => {
                        s.annotation.signature.library = entries;
                        s.annotation.signature.library_selected = None;
                        s.annotation.signature.error = None;
                    }
                    Err(e) => {
                        s.annotation.signature.error = Some(format!("Pustaka belum disimpan: {e}"))
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }
}
