//! Native dialogs and async composition of the safe export use case.
use crate::*;
use pdf_application::export::{
    ExportControl, ExportError, ExportPlan, ExportReceipt, ExportScope, OptimizationSizes,
    finish_export, prepare_export,
};
use pdf_platform::LocalOutput;
#[derive(Clone, Default)]
pub(super) struct ExportState {
    pub printing: bool,
    pub png: bool,
    pub png_total: usize,
    pub png_done: Arc<std::sync::atomic::AtomicUsize>,
    pub optimizing: bool,
    pub optimization: Option<OptimizationSizes>,
    pub split_total: usize,
    pub split_done: Arc<std::sync::atomic::AtomicUsize>,
    pub running: bool,
    job_id: u64,
    pub control: Option<ExportControl>,
    pub notice: Option<String>,
    pub success: bool,
    pub completed: u64,
    pub pages: usize,
}
impl ExportState {
    pub(super) fn phase_label(&self) -> &'static str {
        use pdf_application::export::ExportPhase;
        let phase = self
            .control
            .as_ref()
            .map_or(ExportPhase::Choosing, |c| c.phase());
        if self.printing {
            return "Menyiapkan cetak / dialog printer";
        }
        if self.png {
            match phase {
                ExportPhase::Writing => "Merender dan memvalidasi PNG",
                ExportPhase::Validation => "Memvalidasi PNG",
                ExportPhase::Commit => "Menyimpan folder PNG baru",
                _ => phase.label(),
            }
        } else {
            phase.label()
        }
    }
}
impl DocumentSession {
    pub(super) fn start_export_job(&mut self, control: ExportControl, cx: &mut Context<Self>) {
        self.resources
            .export_owner
            .store(self.tab_id.0, std::sync::atomic::Ordering::SeqCst);
        self.export.printing = false;
        self.export.optimizing = false;
        self.export.png = false;
        self.export.png_total = 0;
        self.export
            .png_done
            .store(0, std::sync::atomic::Ordering::SeqCst);
        self.export.optimization = None;
        self.export.split_total = 0;
        self.export
            .split_done
            .store(0, std::sync::atomic::Ordering::SeqCst);
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
    }

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
        self.resources
            .export_owner
            .store(0, std::sync::atomic::Ordering::SeqCst);
        self.export.running = false;
        self.export.control = None;
        self.export.completed += 1;
        let success = result.is_ok();
        let mut current = false;
        match result {
            Ok(receipt) => {
                current = self
                    .workspace
                    .as_mut()
                    .is_some_and(|mut w| receipt.apply(self.workspace_instance, &mut w));
                self.export.success = true;
                self.export.pages = receipt.page_count;
                self.export.optimization = receipt.optimization;
                let name = receipt
                    .destination
                    .file_name()
                    .map(|s| s.to_string_lossy().into_owned())
                    .unwrap_or_default();
                self.export.notice = Some(
                    if let (ExportScope::Optimized, Some(sizes)) =
                        (receipt.scope, receipt.optimization)
                    {
                        optimization_notice(&name, sizes)
                    } else if receipt.scope == ExportScope::Selected {
                        format!("Halaman terpilih tersimpan: {name}")
                    } else if current {
                        format!("PDF baru tersimpan: {name}")
                    } else {
                        format!(
                            "Snapshot sebelumnya tersimpan: {name}. Workspace saat ini tidak ditandai tersimpan."
                        )
                    },
                );
            }
            Err(e) => {
                self.export.success = false;
                self.export.pages = 0;
                if matches!(e, ExportError::Cancelled) {
                    self.export.notice = Some(e.to_string());
                } else {
                    self.export.notice = None;
                    self.show_error_popup("PDF belum dapat diekspor", e.to_string(), cx);
                }
            }
        }
        self.report(cx);
        cx.notify();
        self.transition_export_finished(success, current, cx);
    }
    pub(super) fn start_export(&mut self, scope: ExportScope, cx: &mut Context<Self>) {
        if let Some(owner) = &self.document_owner {
            let _ = owner.update(cx, |s, cx| s.start_export(scope, cx));
            return;
        }
        if self.edits_locked() {
            return;
        }
        self.begin_export(scope, cx);
    }
    pub(super) fn start_transition_export(&mut self, cx: &mut Context<Self>) {
        self.begin_export(ExportScope::All, cx);
    }
    fn begin_export(&mut self, scope: ExportScope, cx: &mut Context<Self>) {
        if self
            .resources
            .export_owner
            .load(std::sync::atomic::Ordering::SeqCst)
            != 0
            || self.export.running
            || self.opening
            || self.error_popup
        {
            return;
        }
        let Some(w) = self.workspace.as_ref() else {
            // A source-change result may have invalidated the workspace while
            // an unsaved-change dialog was open. Never strand its saving intent.
            self.cancel_transition(cx);
            return;
        };
        let plan = match ExportPlan::capture(&w, self.workspace_instance, scope) {
            Ok(p) => p,
            Err(e) => {
                drop(w);
                self.cancel_transition(cx);
                self.show_error_popup("PDF belum dapat diekspor", e.to_string(), cx);
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
            match scope {
                ExportScope::All => "edited",
                ExportScope::Selected => "pages",
                ExportScope::Optimized => "optimized",
            }
        );
        let dialog = cx.prompt_for_new_path(&parent, Some(&suggested));
        let document_name = w
            .source()
            .path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned();
        drop(w);
        let control = ExportControl::default();
        self.start_export_job(control.clone(), cx);
        self.export.optimizing = scope == ExportScope::Optimized;
        self.report(cx);
        cx.notify();
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
            let task = cx
                .background_executor()
                .spawn(async move { prepare_export(plan, &path, &LocalOutput, &preflight, &c) });
            let prepared = match task.await {
                Ok(p) => p,
                Err(e) => {
                    let _ = this.update(cx, |this, cx| this.export_finished(Err(e), cx));
                    return;
                }
            };
            let acknowledged = if prepared.report.requires_acknowledgement() {
                control.set_phase(pdf_application::export::ExportPhase::Confirmation);
                let prompt = handle.update(cx, |_, window, cx| {
                    window.prompt(
                        gpui::PromptLevel::Warning,
                        prepared.report.confirmation_title(),
                        Some(&format!(
                            "{document_name}\n{}",
                            prepared.report.confirmation_detail()
                        )),
                        &["Batal", prepared.report.confirmation_action()],
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
fn format_size(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{bytes} B")
    } else if bytes < 1024 * 1024 {
        format!("{:.1} KiB ({bytes} B)", bytes as f64 / 1024.)
    } else {
        format!("{:.2} MiB ({bytes} B)", bytes as f64 / (1024. * 1024.))
    }
}
fn optimization_notice(name: &str, sizes: OptimizationSizes) -> String {
    let before = sizes.baseline_bytes;
    let after = sizes.output_bytes;
    let change = if after < before {
        format!(
            "Ukuran berkurang {:.1}%.",
            (before - after) as f64 * 100. / before as f64
        )
    } else if after > before {
        format!(
            "Ukuran bertambah {:.1}%.",
            (after - before) as f64 * 100. / before as f64
        )
    } else {
        "Ukuran tetap sama.".into()
    };
    format!(
        "Optimize selesai: {name}. Susunan sebelum optimasi: {}; hasil: {}. {change} Status tersimpan workspace tidak berubah.",
        format_size(before),
        format_size(after)
    )
}
