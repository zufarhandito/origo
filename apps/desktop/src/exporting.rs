//! Native dialogs and async composition of the safe export use case.
use crate::*;
use pdf_application::export::{
    ExportControl, ExportError, ExportPlan, ExportReceipt, ExportScope, finish_export,
    prepare_export,
};
use pdf_platform::LocalOutput;
#[derive(Default)]
pub(super) struct ExportState {
    pub running: bool,
    job_id: u64,
    pub control: Option<ExportControl>,
    pub notice: Option<String>,
    pub success: bool,
    pub completed: u64,
    pub pages: usize,
    pub close_after: bool,
}
impl Viewer {
    pub(super) fn cancel_export(&mut self, cx: &mut Context<Self>) {
        if let Some(c) = &self.export.control {
            c.cancel();
        }
        cx.notify();
    }
    fn export_finished(
        &mut self,
        result: Result<ExportReceipt, ExportError>,
        cx: &mut Context<Self>,
    ) {
        self.export.running = false;
        self.export.control = None;
        self.export.completed += 1;
        match result {
            Ok(receipt) => {
                let current = self
                    .workspace
                    .as_mut()
                    .is_some_and(|w| receipt.apply(self.workspace_instance, w));
                self.export.success = true;
                self.export.pages = receipt.page_count;
                let name = receipt
                    .destination
                    .file_name()
                    .map(|s| s.to_string_lossy().into_owned())
                    .unwrap_or_default();
                self.export.notice = Some(if receipt.scope == ExportScope::Selected {
                    format!("Halaman terpilih tersimpan: {name}")
                } else if current {
                    format!("PDF baru tersimpan: {name}")
                } else {
                    format!(
                        "Snapshot sebelumnya tersimpan: {name}. Workspace saat ini tidak ditandai tersimpan."
                    )
                });
            }
            Err(e) => {
                self.export.success = false;
                self.export.pages = 0;
                self.export.notice = Some(e.to_string());
            }
        }
        self.report(cx);
        cx.notify();
        if self.export.close_after {
            let handle = self.window_handle;
            cx.defer(move |cx| {
                let _ = handle.update(cx, |_, window, _| window.remove_window());
            });
        }
    }
    pub(super) fn start_export(&mut self, scope: ExportScope, cx: &mut Context<Self>) {
        if self.export.running || self.opening {
            return;
        }
        let Some(w) = &self.workspace else {
            return;
        };
        let plan = match ExportPlan::capture(w, self.workspace_instance, scope) {
            Ok(p) => p,
            Err(e) => {
                self.export.notice = Some(e.to_string());
                cx.notify();
                return;
            }
        };
        let parent = w
            .source()
            .path
            .parent()
            .map(PathBuf::from)
            .unwrap_or_default();
        let stem = w
            .source()
            .path
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| "document".into());
        let suggested = format!(
            "{stem}-{}.pdf",
            if scope == ExportScope::All {
                "edited"
            } else {
                "pages"
            }
        );
        let dialog = cx.prompt_for_new_path(&parent, Some(&suggested));
        let control = ExportControl::default();
        self.export.job_id += 1;
        let poll_id = self.export.job_id;
        self.export.running = true;
        self.export.control = Some(control.clone());
        self.export.notice = None;
        self.export.success = false;
        self.report(cx);
        cx.notify();
        // A timer only projects phase state; all native work is off the UI thread.
        cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(std::time::Duration::from_millis(150))
                    .await;
                let active = this
                    .update(cx, |this, cx| {
                        if this.export.running && this.export.job_id == poll_id {
                            this.report(cx);
                            cx.notify();
                        }
                        this.export.running && this.export.job_id == poll_id
                    })
                    .unwrap_or(false);
                if !active {
                    break;
                }
            }
        })
        .detach();
        let exporter = self.exporter.clone();
        let renderer = self.renderer.clone();
        let handle = self.window_handle;
        cx.spawn(async move |this, cx| {
            let path = match dialog.await {
                Ok(Ok(Some(path))) => {
                    if path.extension().is_none() {
                        path.with_extension("pdf")
                    } else {
                        path
                    }
                }
                result => {
                    let error = if matches!(result, Ok(Ok(None))) {
                        ExportError::Cancelled
                    } else {
                        ExportError::Destination
                    };
                    let _ = this.update(cx, |this, cx| this.export_finished(Err(error), cx));
                    return;
                }
            };
            let preflight = exporter.clone();
            let c = control.clone();
            let task = cx.background_executor().spawn(async move {
                prepare_export(plan, &path, &LocalOutput, &preflight, &c)
            });
            let prepared = match task.await {
                Ok(p) => p,
                Err(e) => {
                    let _ = this.update(cx, |this, cx| this.export_finished(Err(e), cx));
                    return;
                }
            };
            let acknowledged = if prepared.report.metadata_loss {
                control.set_phase(pdf_application::export::ExportPhase::Confirmation);
                let prompt = handle.update(cx, |_, window, cx| {
                    window.prompt(
                        gpui::PromptLevel::Warning,
                        "Metadata dokumen dapat tidak terbawa",
                        Some("Ekspor ini menyusun PDF baru. Metadata seperti judul, penulis, atau informasi XML dapat hilang. Lanjutkan ekspor snapshot ini?"),
                        &["Batal", "Lanjutkan"],
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
            if prepared.report.metadata_loss && !acknowledged {
                drop(prepared);
                let _ = this.update(cx, |this, cx| {
                    this.export_finished(Err(ExportError::Cancelled), cx)
                });
                return;
            }
            let task = cx.background_executor().spawn(async move {
                finish_export(prepared, acknowledged, &exporter, &renderer, &control)
            });
            let result = task.await;
            let _ = this.update(cx, |this, cx| this.export_finished(result, cx));
        })
        .detach();
    }
}
