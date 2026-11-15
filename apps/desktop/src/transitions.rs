//! A single, cancellable intent to leave the current workspace.
use crate::*;
use gpui::component::{
    Disableable, WindowExt,
    button::{Button, ButtonVariants},
    dialog::DialogFooter,
};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) enum Phase {
    #[default]
    Idle,
    Choosing,
    Inspecting,
    WaitingExport,
    Prompt,
    Saving,
    Applying,
}
pub(super) enum Intent {
    Close,
    Open(Workspace),
}
#[derive(Default)]
pub(super) struct Transition {
    pub phase: Phase,
    pub token: u64,
    intent: Option<Intent>,
}
impl Transition {
    pub fn active(&self) -> bool {
        self.phase != Phase::Idle
    }
    fn begin(&mut self, phase: Phase) {
        self.token += 1;
        self.phase = phase;
    }
    fn clear(&mut self) {
        self.token += 1;
        self.phase = Phase::Idle;
        self.intent = None;
    }
    /// Failed/cancelled exports never authorize leaving. A save started for this
    /// intent additionally requires the exact full-export receipt to apply.
    fn exported(&mut self, success: bool, current: bool) -> bool {
        match self.phase {
            Phase::WaitingExport if success => true,
            Phase::Saving if success && current => true,
            Phase::WaitingExport | Phase::Saving => {
                self.clear();
                false
            }
            _ => false,
        }
    }
}
impl Viewer {
    pub(super) fn edits_locked(&self) -> bool {
        self.transition.active() || self.error_popup
    }
    pub(super) fn cancel_transition(&mut self, cx: &mut Context<Self>) {
        self.transition.clear();
        self.page_dragging = false;
        self.report(cx);
        cx.notify();
    }
    pub(super) fn request_close(&mut self, cx: &mut Context<Self>) -> bool {
        if self.transition.active() || self.error_popup || self.opening {
            return false;
        }
        if !self.export.running && !self.workspace.as_ref().is_some_and(Workspace::is_dirty) {
            return true;
        }
        self.transition.begin(Phase::WaitingExport);
        self.transition.intent = Some(Intent::Close);
        self.advance_transition(cx);
        false
    }
    pub(super) fn choose_open(&mut self, cx: &mut Context<Self>) {
        if self.opening || self.edits_locked() {
            return;
        }
        self.transition.begin(Phase::Choosing);
        let token = self.transition.token;
        let dialog = cx.prompt_for_paths(gpui::PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some("Pilih PDF".into()),
        });
        self.report(cx);
        cx.notify();
        cx.spawn(async move |this, cx| {
            let result = dialog.await;
            let _ = this.update(cx, |this, cx| {
                if this.transition.token != token {
                    return;
                }
                match result {
                    Ok(Ok(Some(paths))) => {
                        if let Some(path) = paths.into_iter().next() {
                            this.inspect_candidate(path, cx);
                        } else {
                            this.cancel_transition(cx);
                        }
                    }
                    Ok(Ok(None)) => this.cancel_transition(cx),
                    _ => {
                        this.cancel_transition(cx);
                        this.show_error_popup(
                            "PDF belum dapat dibuka",
                            "Dialog file gagal dibuka.".into(),
                            cx,
                        );
                    }
                }
            });
        })
        .detach();
    }
    pub(super) fn inspect_candidate(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        self.transition.begin(Phase::Inspecting);
        let token = self.transition.token;
        // Reserve an ID without invalidating the current workspace's text results.
        let id = pdf_domain::SourceId(self.source_sequence + 1);
        let renderer = self.renderer.clone();
        let task = cx
            .background_executor()
            .spawn(async move { pdf_application::open_document(&renderer, &path, id) });
        cx.spawn(async move |this, cx| {
            let result = task.await;
            let _ = this.update(cx, |this, cx| {
                if this.transition.token != token {
                    return;
                }
                match result {
                    Ok(candidate) => {
                        this.transition.intent = Some(Intent::Open(candidate));
                        this.advance_transition(cx);
                    }
                    Err(e) => {
                        this.cancel_transition(cx);
                        this.show_error_popup("PDF belum dapat dibuka", e.to_string(), cx);
                    }
                }
            });
        })
        .detach();
        self.report(cx);
        cx.notify();
    }
    pub(super) fn transition_export_finished(
        &mut self,
        success: bool,
        current: bool,
        cx: &mut Context<Self>,
    ) {
        if self.transition.exported(success, current) {
            self.advance_transition(cx);
        }
        self.report(cx);
        cx.notify();
    }
    fn advance_transition(&mut self, cx: &mut Context<Self>) {
        if self.export.running {
            self.transition.phase = Phase::WaitingExport;
            self.report(cx);
            cx.notify();
        } else if self.workspace.as_ref().is_some_and(Workspace::is_dirty) {
            self.prompt_unsaved(cx);
        } else {
            self.apply_transition(cx);
        }
    }
    fn prompt_unsaved(&mut self, cx: &mut Context<Self>) {
        if self.error_popup {
            self.cancel_transition(cx);
            return;
        }
        self.transition.phase = Phase::Prompt;
        self.error_popup = true; // Shared modal/input guard, including global canvas listeners.
        self.text.dragging = false;
        self.text.pending = None;
        self.text.sentence_anchor = None;
        self.page_dragging = false;
        let can_save = self
            .workspace
            .as_ref()
            .is_some_and(|w| !w.pages().is_empty());
        let token = self.transition.token;
        let handle = self.window_handle;
        let weak = cx.weak_entity();
        cx.defer(move |cx| {
            let _ = handle.update(cx, |_, window, cx| {
                let cancel_focus = cx.focus_handle();
                let focus = cancel_focus.clone();
                let focus_view = weak.clone();
                window.open_alert_dialog(cx, move |dialog, _, _| {
                    let cancel = weak.clone();
                    let discard = weak.clone();
                    let save = weak.clone();
                    let closed = weak.clone();
                    let description = if can_save {
                            "Perubahan halaman belum disimpan. Simpan sebagai PDF baru atau buang perubahan untuk melanjutkan."
                        } else {
                            "Perubahan belum disimpan. Minimal satu halaman diperlukan untuk menyimpan PDF. Pilih Batal untuk Undo atau impor halaman."
                        };
                    dialog.title(gpui::div().id("unsaved-title").role(gpui::Role::Group)
                            .aria_label("Simpan perubahan sebelum melanjutkan?")
                            .child("Simpan perubahan sebelum melanjutkan?"))
                        .description(gpui::div().id("unsaved-description").role(gpui::Role::Group).aria_label(description).child(description))
                        .centered().width(px(540.)).close_button(true)
                        .footer(DialogFooter::new()
                            .child(Button::new("unsaved-cancel").label("Batal").track_focus(&cancel_focus)
                                .on_click(move |_, window, cx| {
                                    window.close_dialog(cx);
                                    let _ = cancel.update(cx, |this, cx| this.unsaved_answer(token, None, cx));
                                }))
                            .child(Button::new("unsaved-discard").label("Buang perubahan")
                                .on_click(move |_, window, cx| {
                                    window.close_dialog(cx);
                                    let _ = discard.update(cx, |this, cx| this.unsaved_answer(token, Some(false), cx));
                                }))
                            .child(Button::new("unsaved-save").primary().label("Simpan sebagai")
                                .accessibility_label("Simpan perubahan sebagai PDF").disabled(!can_save)
                                .on_click(move |_, window, cx| {
                                    window.close_dialog(cx);
                                    let _ = save.update(cx, |this, cx| this.unsaved_answer(token, Some(true), cx));
                                })))
                        .on_close(move |_, _, cx| {
                            let _ = closed.update(cx, |this, cx| this.unsaved_answer(token, None, cx));
                        })
                });
                window.on_next_frame(move |window, cx| {
                    if focus_view.read_with(cx, |this, _| this.transition.token == token && this.transition.phase == Phase::Prompt).unwrap_or(false) {
                        focus.focus(window, cx);
                    }
                });
            });
        });
        self.report(cx);
        cx.notify();
    }
    fn unsaved_answer(&mut self, token: u64, save: Option<bool>, cx: &mut Context<Self>) {
        if self.transition.token != token || self.transition.phase != Phase::Prompt {
            return;
        }
        self.error_popup = false;
        match save {
            None => self.cancel_transition(cx),
            Some(false) => self.apply_transition(cx),
            Some(true) => {
                self.transition.phase = Phase::Saving;
                self.start_transition_export(cx);
            }
        }
    }
    fn apply_transition(&mut self, cx: &mut Context<Self>) {
        self.transition.phase = Phase::Applying;
        match self.transition.intent.take() {
            Some(Intent::Close) => {
                let handle = self.window_handle;
                cx.defer(move |cx| {
                    let _ = handle.update(cx, |_, window, _| window.remove_window());
                });
            }
            Some(Intent::Open(candidate)) => {
                let token = self.transition.token;
                let path = candidate.source().path.clone();
                let expected = candidate.source().fingerprint;
                let task = cx.background_executor().spawn(async move {
                    pdf_platform::fingerprint(&path).is_ok_and(|actual| actual == expected)
                });
                cx.spawn(async move |this, cx| {
                    let unchanged = task.await;
                    let _ = this.update(cx, |this, cx| {
                        if this.transition.token != token {
                            return;
                        }
                        if unchanged {
                            this.install_workspace(candidate, cx);
                        }
                        this.cancel_transition(cx);
                        if !unchanged {
                            this.show_error_popup(
                                "PDF belum dapat dibuka",
                                "PDF tujuan berubah atau tidak tersedia. Pilih ulang PDF tersebut."
                                    .into(),
                                cx,
                            );
                        }
                    });
                })
                .detach();
            }
            None => self.cancel_transition(cx),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn leaving_requires_success_and_saving_requires_current_receipt() {
        for phase in [Phase::WaitingExport, Phase::Saving] {
            for success in [false, true] {
                for current in [false, true] {
                    let mut state = Transition::default();
                    state.begin(phase);
                    state.intent = Some(Intent::Close);
                    let ready = state.exported(success, current);
                    assert_eq!(ready, success && (phase == Phase::WaitingExport || current));
                    if !ready {
                        assert!(!state.active());
                        assert!(state.intent.is_none());
                    }
                }
            }
        }
    }
    #[test]
    fn cancellation_invalidates_async_callbacks_and_allows_new_intent() {
        let mut state = Transition::default();
        state.begin(Phase::Choosing);
        let old = state.token;
        state.clear();
        state.begin(Phase::Inspecting);
        assert_ne!(old, state.token);
        assert!(state.active());
        assert!(!state.exported(true, true));
        assert_eq!(state.phase, Phase::Inspecting);
    }
}
