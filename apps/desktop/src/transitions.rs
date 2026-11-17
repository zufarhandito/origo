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

    WaitingExport,
    Prompt,
    Saving,
}
pub(super) enum Intent {
    Close,
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
impl DocumentSession {
    pub(super) fn edits_locked(&self) -> bool {
        self.workspace.importing()
            || self.transition.active()
            || self.error_popup
            || self
                .resources
                .closing
                .load(std::sync::atomic::Ordering::SeqCst)
            || self
                .resources
                .modal_owner
                .load(std::sync::atomic::Ordering::SeqCst)
                != 0
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
        if !self.export.running && !self.workspace.as_ref().is_some_and(|w| w.is_dirty()) {
            return true;
        }
        self.transition.begin(Phase::WaitingExport);
        self.transition.intent = Some(Intent::Close);
        self.advance_transition(cx);
        false
    }
    pub(super) fn choose_open(&mut self, cx: &mut Context<Self>) {
        let owner = self.owner.clone();
        cx.defer(move |cx| {
            let _ = owner.update(cx, |shell, cx| shell.choose_open(cx));
        });
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
        } else if self.workspace.as_ref().is_some_and(|w| w.is_dirty()) {
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
        self.resources
            .modal_owner
            .store(self.tab_id.0, std::sync::atomic::Ordering::SeqCst);
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
        let document_name = self
            .workspace
            .as_ref()
            .map(|w| {
                w.source()
                    .path
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .into_owned()
            })
            .unwrap_or_default();
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
                    let description = format!("{document_name}\n{description}");
                    dialog.title(gpui::div().id("unsaved-title").role(gpui::Role::Group)
                            .aria_label("Simpan perubahan sebelum melanjutkan?")
                            .child("Simpan perubahan sebelum melanjutkan?"))
                        .description(gpui::div().id("unsaved-description").role(gpui::Role::Group).aria_label(description.clone()).child(description))
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
        self.clear_modal();
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
        match self.transition.intent.take() {
            Some(Intent::Close) => {
                self.close_approved = true;
                self.transition.clear();
                cx.notify();
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
        state.begin(Phase::Prompt);
        let old = state.token;
        state.clear();
        state.begin(Phase::Prompt);
        assert_ne!(old, state.token);
        assert!(state.active());
        assert!(!state.exported(true, true));
        assert_eq!(state.phase, Phase::Prompt);
    }
}
