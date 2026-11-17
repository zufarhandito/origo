//! Synchronize shared document changes without sharing reading state or native jobs.
use crate::*;

impl DocumentSession {
    pub(super) fn dismiss_export_notice(&mut self, cx: &mut Context<Self>) {
        if let Some(owner) = &self.document_owner {
            let _ = owner.update(cx, |s, cx| s.dismiss_export_notice(cx));
        } else {
            self.export.notice = None;
            cx.notify();
        }
    }
    pub(super) fn remember_view_page(&mut self) {
        self.current_page_id = self.workspace.as_ref().and_then(|w| {
            w.pages()
                .get(usize::from(self.navigation.current))
                .map(|p| p.id)
        });
    }
    pub(super) fn sync_shared_model(&mut self, cx: &mut Context<Self>) {
        let signature = self.workspace.signature();
        if signature == self.observed_model {
            return;
        }
        let revision_changed = signature.revision != self.observed_model.revision;
        self.observed_model = signature;
        if self.workspace.is_none() {
            self.search.pause();
            self.search.running = false;
            self.search.matches.clear();
            self.search.current = None;
            self.text = text_selection::TextState::new(self.text_budget);
            self.invalidate_work();
            self.clear_bitmaps(cx);
            self.navigation = Navigation::default();
            self.pending_reading = None;
            self.current_page_id = None;
            self.error = Some("Sumber PDF berubah. Buka kembali dokumen untuk melanjutkan.".into());
            self.status = self.error.clone().unwrap_or_default();
        } else if revision_changed {
            self.refresh_after_edit(false, cx);
            return;
        }
        self.update_status();
        self.report(cx);
        cx.notify();
    }
    // Native dialogs can belong to the hidden actor. Restore keyboard focus to
    // the currently displayed view after returning the actor's entity borrow.
    pub(super) fn restore_visible_focus(&self, cx: &mut Context<Self>) {
        let owner = self.owner.clone();
        let handle = self.window_handle;
        cx.defer(move |cx| {
            let _ = handle.update(cx, |_, window, cx| {
                let _ = owner.update(cx, |shell, cx| shell.restore_document_focus(window, cx));
            });
        });
    }
}
