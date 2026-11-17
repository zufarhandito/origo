//! Split configuration and background batch execution; never marks the PDF saved.
use crate::*;
use gpui::component::{
    Selectable, WindowExt,
    button::{Button, ButtonVariants},
    dialog::DialogFooter,
    input::Input,
};
use pdf_application::{
    export::{ExportControl, ExportError, ExportPhase, ExportPlan, ExportScope},
    split::{
        SplitError, SplitPlan, SplitReceipt, SplitSpec, finish_split, prepare_split,
        validate_folder,
    },
};
use pdf_platform::split::LocalSplitOutput;
use std::sync::atomic::Ordering;

#[derive(Clone)]
pub(super) struct SplitDialog {
    plan: Arc<ExportPlan>,
    manual: bool,
    spec: Entity<InputState>,
    folder: Entity<InputState>,
    error: Option<String>,
}
impl SplitDialog {
    fn with_spec<T>(
        &self,
        cx: &App,
        consume: impl FnOnce(SplitSpec<'_>) -> Result<T, SplitError>,
    ) -> Result<T, SplitError> {
        let text = self.spec.read(cx).value().to_string();
        let spec = if self.manual {
            SplitSpec::Ranges(&text)
        } else {
            if !text.trim().bytes().all(|b| b.is_ascii_digit()) {
                return Err(SplitError::Count);
            }
            SplitSpec::Every(text.trim().parse().map_err(|_| SplitError::Count)?)
        };
        consume(spec)
    }
    fn split_plan(&self, cx: &App) -> Result<SplitPlan, SplitError> {
        self.with_spec(cx, |spec| SplitPlan::new((*self.plan).clone(), spec))
    }
    fn summary(&self, cx: &App) -> Result<(usize, usize), SplitError> {
        self.with_spec(cx, |spec| SplitPlan::preview(self.plan.pages().len(), spec))
    }
}
impl DocumentSession {
    pub(super) fn show_split(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(owner) = &self.document_owner {
            let _ = owner.update(cx, |s, cx| s.show_split(window, cx));
            return;
        }
        if self.edits_locked()
            || self.opening
            || self
                .resources
                .export_owner
                .load(std::sync::atomic::Ordering::SeqCst)
                != 0
        {
            return;
        }
        let Some(w) = self.workspace.as_ref() else {
            return;
        };
        let plan = match ExportPlan::capture(&w, self.workspace_instance, ExportScope::All) {
            Ok(p) => p,
            Err(e) => {
                drop(w);
                self.show_error_popup("PDF belum dapat dibagi", e.to_string(), cx);
                return;
            }
        };
        drop(w);
        let count = plan.pages().len();
        let spec = cx.new(|cx| {
            let mut s = InputState::new(window, cx);
            s.set_value(count.min(10).to_string(), window, cx);
            s
        });
        let folder = cx.new(|cx| {
            let mut s = InputState::new(window, cx);
            s.set_value("hasil-split", window, cx);
            s
        });
        for input in [&spec, &folder] {
            cx.subscribe_in(input, window, |this, _, event: &InputEvent, window, cx| {
                if !matches!(event, InputEvent::Change) {
                    return;
                }
                if let Some(ui) = &mut this.split_dialog {
                    ui.error = None;
                }
                window.refresh();
                cx.notify();
            })
            .detach();
        }
        self.split_dialog = Some(SplitDialog {
            plan: Arc::new(plan),
            manual: false,
            spec,
            folder,
            error: None,
        });
        self.error_popup = true;
        self.resources
            .modal_owner
            .store(self.tab_id.0, std::sync::atomic::Ordering::SeqCst);
        self.page_dragging = false;
        self.text.dragging = false;
        self.text.pending = None;
        self.text.sentence_anchor = None;
        let weak = cx.weak_entity();
        window.open_alert_dialog(cx, move |dialog, _, cx| {
            let Some(ui) = weak.read_with(cx, |this, _| this.split_dialog.clone()).ok().flatten() else { return dialog; };
            let preview = match ui.summary(cx) {
                Ok((files, pages)) => format!("{files} file PDF · {pages} dari {} halaman workspace. Halaman yang tidak dicantumkan tidak diekspor.", ui.plan.pages().len()),
                Err(e) => e.to_string(),
            };
            let every = weak.clone(); let manual = weak.clone(); let cancel = weak.clone();
            let submit = weak.clone(); let closed = weak.clone();
            dialog.title(gpui::div().id("split-title").role(gpui::Role::Group).aria_label("Split PDF").child("Split PDF"))
                .description("Bagi susunan halaman saat ini ke folder baru. File sumber dan status tersimpan workspace tetap utuh.")
                .centered().width(px(580.)).close_button(true)
                .child(gpui::div().flex().gap_2()
                    .child(Button::new("split-every").label("Setiap N halaman").selected(!ui.manual)
                        .on_click(move |_, window, cx| { let _ = every.update(cx, |this, cx| this.split_mode(false, window, cx)); }))
                    .child(Button::new("split-manual").label("Rentang manual").selected(ui.manual)
                        .on_click(move |_, window, cx| { let _ = manual.update(cx, |this, cx| this.split_mode(true, window, cx)); })))
                .child(gpui::div().flex().flex_col().gap_2().mt_3()
                    .child(if ui.manual { "Rentang naik; titik koma memisahkan file. Contoh: 1-3,5; 6-8" } else { "Jumlah halaman per file" })
                    .child(Input::new(&ui.spec).aria_label("Aturan pembagian"))
                    .child("Nama folder baru (pilih folder induk setelah ini)")
                    .child(Input::new(&ui.folder).aria_label("Nama folder Split"))
                    .child(gpui::div().id("split-preview").role(gpui::Role::Group).aria_label(preview.clone()).text_sm().child(preview.clone()))
                    .when_some(ui.error.clone().filter(|error| *error != preview), |el, error| el.child(gpui::div().id("split-error").role(gpui::Role::Group).aria_label(error.clone()).child(error))))
                .footer(DialogFooter::new()
                    .child(Button::new("split-cancel").label("Batal").on_click(move |_, window, cx| {
                        window.close_dialog(cx); let _ = cancel.update(cx, |this, cx| this.close_split(cx));
                    }))
                    .child(Button::new("split-start").primary().label("Pilih folder induk")
                        .on_click(move |_, window, cx| { let _ = submit.update(cx, |this, cx| this.submit_split(window, cx)); })))
                .on_close(move |_, _, cx| { let _ = closed.update(cx, |this, cx| this.close_split(cx)); })
        });
        cx.notify();
    }
    fn split_mode(&mut self, manual: bool, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(ui) = &mut self.split_dialog {
            if ui.manual == manual {
                return;
            }
            ui.manual = manual;
            ui.error = None;
            let value = if manual {
                format!("1-{}", ui.plan.pages().len())
            } else {
                ui.plan.pages().len().min(10).to_string()
            };
            ui.spec.update(cx, |s, cx| s.set_value(value, window, cx));
        }
        window.refresh();
        cx.notify();
    }
    fn close_split(&mut self, cx: &mut Context<Self>) {
        self.split_dialog = None;
        self.clear_modal();
        self.restore_visible_focus(cx);
        cx.notify();
    }
    fn submit_split(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(ui) = self.split_dialog.as_ref() else {
            return;
        };
        let name = ui.folder.read(cx).value().to_string();
        let result = validate_folder(&name).and_then(|_| ui.split_plan(cx));
        let plan = match result {
            Ok(p) => p,
            Err(e) => {
                if let Some(ui) = &mut self.split_dialog {
                    ui.error = Some(e.to_string());
                }
                window.refresh();
                cx.notify();
                return;
            }
        };
        window.close_dialog(cx);
        self.close_split(cx);
        self.start_split(plan, name, cx);
    }
    fn split_finished(&mut self, result: Result<SplitReceipt, SplitError>, cx: &mut Context<Self>) {
        self.resources
            .export_owner
            .store(0, std::sync::atomic::Ordering::SeqCst);
        self.export.running = false;
        self.export.control = None;
        self.export.completed += 1;
        let success = result.is_ok();
        self.export.success = success;
        self.export.pages = 0;
        match result {
            Ok(receipt) => {
                self.export.pages = receipt.pages;
                self.export.notice = Some(format!(
                    "Split selesai: {} file PDF, {} halaman · {}",
                    receipt.files,
                    receipt.pages,
                    receipt
                        .destination
                        .file_name()
                        .unwrap_or_default()
                        .to_string_lossy()
                ));
            }
            Err(SplitError::Export(ExportError::Cancelled)) => {
                self.export.notice =
                    Some("Split dibatalkan. Folder hasil tidak diterbitkan.".into())
            }
            Err(e) => {
                self.export.notice = None;
                self.show_error_popup("PDF belum dapat dibagi", e.to_string(), cx);
            }
        }
        self.transition_export_finished(success, false, cx);
        self.report(cx);
        cx.notify();
    }
    fn start_split(&mut self, plan: SplitPlan, name: String, cx: &mut Context<Self>) {
        if self
            .resources
            .export_owner
            .load(std::sync::atomic::Ordering::SeqCst)
            != 0
        {
            return;
        }
        let picker = cx.prompt_for_paths(gpui::PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: Some("Pilih folder induk hasil Split".into()),
        });
        let control = ExportControl::default();
        self.start_export_job(control.clone(), cx);
        self.export.split_total = plan.parts().len();
        let total = self.export.split_total;
        let progress = self.export.split_done.clone();
        let exporter = self.exporter.clone();
        let renderer = self.renderer.clone();
        let handle = self.window_handle;
        self.report(cx);
        cx.notify();
        cx.spawn(async move |this, cx| {
            let destination = match picker.await {
                Ok(Ok(Some(paths))) if paths.len() == 1 => paths[0].join(name),
                other => {
                    let error = if matches!(other, Ok(Ok(None))) {
                        ExportError::Cancelled
                    } else {
                        ExportError::Destination
                    };
                    let _ = this.update(cx, |this, cx| this.split_finished(Err(error.into()), cx));
                    return;
                }
            };
            let preflight = exporter.clone();
            let c = control.clone();
            let prepared = cx
                .background_executor()
                .spawn(async move {
                    prepare_split(plan, &destination, &LocalSplitOutput, &preflight, &c)
                })
                .await;
            let prepared = match prepared {
                Ok(p) => p,
                Err(e) => {
                    let _ = this.update(cx, |this, cx| this.split_finished(Err(e), cx));
                    return;
                }
            };
            let acknowledged = if prepared.report.requires_acknowledgement() {
                control.set_phase(ExportPhase::Confirmation);
                let report = prepared.report.clone();
                let prompt = handle.update(cx, |_, window, cx| {
                    let detail = format!(
                        "Persetujuan ini berlaku untuk seluruh batch Split ({total} file). {}",
                        report.confirmation_detail()
                    );
                    window.prompt(
                        gpui::PromptLevel::Warning,
                        report.confirmation_title(),
                        Some(&detail),
                        &["Batal", report.confirmation_action()],
                        cx,
                    )
                });
                match prompt {
                    Ok(answer) => answer.await == Ok(1),
                    Err(_) => false,
                }
            } else {
                false
            };
            if prepared.report.requires_acknowledgement() && !acknowledged {
                drop(prepared);
                let _ = this.update(cx, |this, cx| {
                    this.split_finished(Err(ExportError::Cancelled.into()), cx)
                });
                return;
            }
            let result = cx
                .background_executor()
                .spawn(async move {
                    finish_split(
                        prepared,
                        acknowledged,
                        &exporter,
                        &renderer,
                        &control,
                        |n| {
                            progress.store(n, Ordering::SeqCst);
                        },
                    )
                })
                .await;
            let _ = this.update(cx, |this, cx| this.split_finished(result, cx));
        })
        .detach();
    }
}
