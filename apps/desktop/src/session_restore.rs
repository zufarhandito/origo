//! Reading session persistence, separate from unsaved edits and export receipts.
use super::*;
use crate::local_state::{SavedDocument, SessionSnapshot};
use gpui::component::{Sizable, StyledExt};

impl WindowShell {
    pub(super) fn initialize_session_restore(
        &mut self,
        explicit_file: bool,
        cx: &mut Context<Self>,
    ) {
        let directory = self.state_dir.clone();
        cx.spawn(async move |this, cx| {
            let result = cx.background_executor().spawn(async move {
                let dir = directory.ok_or_else(|| anyhow::anyhow!("Direktori sesi tidak tersedia"))?;
                crate::local_state::load_session(&dir)
            }).await;
            let _ = this.update(cx, |this, cx| {
                if this.session_cleared { return; }
                match result {
                    Ok(snapshot) => {
                        this.session_ready = true;
                        this.session_last = snapshot.clone();
                        if !explicit_file && !this.session_offer_dismissed && !this.opening && this.tabs.is_empty() {
                            this.session_offer = snapshot.filter(|s| !s.documents.is_empty());
                        }
                    }
                    Err(_) => this.notice = Some("Sesi tersimpan tidak dapat dibaca. Pemulihan dilewati; penyimpanan sesi dinonaktifkan agar metadata lama tetap aman. Hapus riwayat sesi untuk mengaktifkannya kembali.".into()),
                }
                cx.notify();
            });
        }).detach();
        cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(std::time::Duration::from_secs(1))
                    .await;
                if this
                    .update(cx, |this, cx| this.checkpoint_session(cx))
                    .is_err()
                {
                    break;
                }
            }
        })
        .detach();
    }
    pub(super) fn capture_session(&self, cx: &App) -> SessionSnapshot {
        if self.session_cleared {
            return SessionSnapshot::default();
        }
        let mut snapshot = SessionSnapshot::default();
        for tab in &self.tabs {
            let Some(session) = tab.preferred_session() else {
                continue;
            };
            let session = session.read(cx);
            let Some(workspace) = session.workspace.as_ref() else {
                continue;
            };
            if Some(tab.id) == self.active {
                snapshot.active = Some(snapshot.documents.len());
            }
            snapshot.documents.push(SavedDocument {
                path: tab.path.clone(),
                fingerprint: workspace.source().fingerprint,
                position: session.reading_position(),
            });
        }
        snapshot
    }
    pub(super) fn checkpoint_session(&mut self, cx: &mut Context<Self>) {
        if !self.session_ready
            || self.opening
            || self.close.is_some()
            || self.session_offer.is_some()
            || self.restoring_session.is_some()
            || self.session_cleared
        {
            return;
        }
        self.persist_session(self.capture_session(cx), cx);
    }
    pub(super) fn persist_session(&mut self, snapshot: SessionSnapshot, cx: &mut Context<Self>) {
        if !self.session_ready || self.session_last.as_ref() == Some(&snapshot) {
            return;
        }
        self.session_last = Some(snapshot.clone());
        if self.session_writing {
            self.session_queued = Some(snapshot);
            return;
        }
        self.write_session(snapshot, cx);
    }
    fn write_session(&mut self, snapshot: SessionSnapshot, cx: &mut Context<Self>) {
        self.session_writing = true;
        self.pending_preferences += 1;
        let directory = self.state_dir.clone();
        cx.spawn(async move |this, cx| {
            let result = cx.background_executor().spawn(async move {
                let dir = directory.ok_or_else(|| anyhow::anyhow!("Direktori sesi tidak tersedia"))?;
                crate::local_state::save_session(&dir, &snapshot)?;
                Ok::<_, anyhow::Error>(())
            }).await;
            let _ = this.update(cx, |this, cx| {
                this.pending_preferences = this.pending_preferences.saturating_sub(1);
                this.session_writing = false;
                if result.is_err() && this.session_queued.is_none() {
                    if this.close.as_ref().is_some_and(|b| b.window) { this.cancel_close(cx); }
                    this.notice = Some("Sesi baca gagal disimpan. PDF tidak berubah. Tutup lagi untuk keluar tanpa menyimpan sesi, atau ubah posisi baca untuk mencoba lagi.".into());
                }
                if result.is_ok() && this.session_cleared && this.session_queued.is_none() {
                    this.notice = Some("Riwayat sesi dihapus. Sesi baca tidak direkam lagi sampai aplikasi dibuka kembali; PDF dan favorit tetap tersedia.".into());
                }
                if let Some(queued) = this.session_queued.take() { this.write_session(queued, cx); }
                this.advance_close(cx);
                cx.notify();
            });
        }).detach();
    }
    pub(super) fn clear_session_history(&mut self, cx: &mut Context<Self>) {
        if self.close.is_some() || self.modal(cx) {
            return;
        }
        self.session_ready = true;
        self.session_offer = None;
        self.session_cleared = true;
        self.session_last = None;
        self.persist_session(SessionSnapshot::default(), cx);
        self.notice = Some("Penyimpanan sesi dinonaktifkan sampai aplikasi dibuka kembali. Penghapusan riwayat sedang diproses.".into());
        cx.notify();
    }
    fn restore_session(&mut self, cx: &mut Context<Self>) {
        if self.opening || self.close.is_some() || self.modal(cx) {
            return;
        }
        let Some(snapshot) = self.session_offer.take() else {
            return;
        };
        let paths = snapshot.documents.iter().map(|d| d.path.clone()).collect();
        self.restoring_session = Some(snapshot);
        self.open_paths(paths, cx);
    }
    pub(super) fn apply_session_position(
        &mut self,
        id: TabId,
        session: &Entity<DocumentSession>,
        cx: &mut Context<Self>,
    ) {
        let Some(snapshot) = &self.restoring_session else {
            return;
        };
        let Some(tab) = self.tabs.iter().find(|t| t.id == id) else {
            return;
        };
        let Some(saved) = snapshot
            .documents
            .iter()
            .find(|d| path_key(&d.path) == path_key(&tab.path))
        else {
            return;
        };
        if session
            .read(cx)
            .workspace
            .as_ref()
            .is_some_and(|w| w.source().fingerprint == saved.fingerprint)
        {
            session.update(cx, |s, cx| {
                s.queue_reading_position(saved.position.clone(), cx)
            });
        }
    }
    pub(super) fn finish_session_restore(&mut self, cx: &mut Context<Self>) {
        let Some(snapshot) = self.restoring_session.take() else {
            return;
        };
        let active = snapshot
            .active
            .and_then(|i| snapshot.documents.get(i))
            .and_then(|d| {
                self.tabs
                    .iter()
                    .find(|t| path_key(&t.path) == path_key(&d.path))
            })
            .map(|t| t.id);
        if let Some(id) = active {
            self.activate(id, cx);
        }
        self.checkpoint_session(cx);
    }
    fn dismiss_session_offer(&mut self, cx: &mut Context<Self>) {
        if self.opening || self.close.is_some() || self.modal(cx) {
            return;
        }
        self.session_offer_dismissed = true;
        self.session_offer = None;
        self.persist_session(SessionSnapshot::default(), cx);
        cx.notify();
    }
    pub(super) fn render_session_offer(&self, cx: &mut Context<Self>) -> AnyElement {
        let count = self.session_offer.as_ref().map_or(0, |s| s.documents.len());
        let blocked = self.opening || self.close.is_some() || self.modal(cx);
        div().id("restore-session-offer").flex().items_center().flex_shrink_0()
            .gap_3().px_3().py_2().border_b_1()
            .bg(cx.theme().warning.opacity(0.15)).border_color(cx.theme().warning.opacity(0.45))
            .role(gpui::Role::Group)
            .aria_label(format!("Lanjutkan sesi baca terakhir ({count} PDF)? PDF asli dibuka kembali; perubahan belum disimpan tidak dipulihkan."))
            .child(gpui::component::Icon::new(IconName::RotateCcw).size(px(16.)).flex_shrink_0().text_color(cx.theme().warning))
            .child(div().flex_1().min_w_0().flex().flex_col().gap_1().text_xs()
                .child(div().font_medium().child(format!("Lanjutkan sesi baca terakhir ({count} PDF)?")))
                .child(div().text_color(cx.theme().muted_foreground).child("PDF asli dibuka kembali; perubahan belum disimpan tidak dipulihkan.")))
            .child(Button::new("restore-session").primary().small().label("Pulihkan Sesi").disabled(blocked)
                .on_click(cx.listener(|s,_,_,cx|s.restore_session(cx))))
            .child(Button::new("dismiss-session-offer").ghost().icon(IconName::X)
                .accessibility_label("Tutup tawaran sesi").tooltip("Tutup")
                .w(px(28.)).h(px(28.)).disabled(blocked)
                .on_click(cx.listener(|s,_,_,cx|s.dismiss_session_offer(cx))))
            .into_any_element()
    }
}
