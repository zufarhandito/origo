//! Snapshot preparation and Windows print UI. Never applies a saved receipt.
use crate::*;
use gpui::component::{WindowExt, button::Button, dialog::DialogFooter, input::Input};
use pdf_application::{
    ExactRenderRequest, PdfInspector,
    export::{
        AtomicOutput, ExportControl, ExportError, ExportPhase, ExportPlan, ExportScope,
        OutputTransaction,
    },
    png::{PngDpi, PngWriter, dimensions},
    printing::PrintPlan,
};
use std::sync::atomic::Ordering;
#[derive(Clone)]
pub(super) struct PrintDialog {
    snapshot: ExportPlan,
    range: Entity<InputState>,
    current: usize,
    error: Option<String>,
}
impl DocumentSession {
    pub(super) fn show_print(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let current = usize::from(self.navigation.current);
        if let Some(owner) = &self.document_owner {
            let _ = owner.update(cx, |s, cx| s.show_print_at(current, window, cx));
            return;
        }
        self.show_print_at(current, window, cx);
    }
    fn show_print_at(&mut self, current: usize, window: &mut Window, cx: &mut Context<Self>) {
        if self.edits_locked()
            || self.opening
            || self.resources.export_owner.load(Ordering::SeqCst) != 0
        {
            return;
        }
        let snapshot = self
            .workspace
            .as_ref()
            .and_then(|w| ExportPlan::capture(&w, self.workspace_instance, ExportScope::All).ok());
        let Some(snapshot) = snapshot else {
            return;
        };
        let range = cx
            .new(|cx| InputState::new(window, cx).placeholder("Semua halaman, atau contoh: 1-3,5"));
        cx.subscribe_in(&range, window, |s, _, event: &InputEvent, window, cx| {
            if matches!(event, InputEvent::Change) {
                if let Some(ui) = &mut s.print_dialog {
                    ui.error = None;
                }
                window.refresh();
                cx.notify();
            }
        })
        .detach();
        self.print_dialog = Some(PrintDialog {
            snapshot,
            range,
            current,
            error: None,
        });
        self.close_annotation_popups(cx);
        self.error_popup = true;
        self.resources
            .modal_owner
            .store(self.tab_id.0, Ordering::SeqCst);
        let weak = cx.weak_entity();
        window.open_alert_dialog(cx,move |dialog,_,cx|{
            let Some(ui)=weak.read_with(cx,|s,_|s.print_dialog.clone()).ok().flatten()else{return dialog;};
            let all=weak.clone();let current=weak.clone();let start=weak.clone();let close=weak.clone();
            dialog.title("Cetak PDF").description("Cetak susunan halaman saat ini beserta anotasi dan tanda tangan. Printer, kertas, orientasi, salinan dan skala dipilih di pratinjau berikutnya. Kualitas raster 150 DPI.")
                .centered().width(px(560.))
                .child(gpui::div().flex().gap_2()
                    .child(Button::new("print-all").label("Semua halaman").on_click(move |_,window,cx|{let _=all.update(cx,|s,cx|{if let Some(ui)=&s.print_dialog{ui.range.update(cx,|i,cx|i.set_value("",window,cx));}window.refresh();});}))
                    .child(Button::new("print-current").label("Halaman ini").on_click(move |_,window,cx|{let _=current.update(cx,|s,cx|{if let Some(ui)=&s.print_dialog{ui.range.update(cx,|i,cx|i.set_value((ui.current+1).to_string(),window,cx));}window.refresh();});})))
                .child(Input::new(&ui.range).aria_label("Rentang halaman cetak").w_full())
                .child(ui.error.unwrap_or_default())
                .footer(DialogFooter::new().child(Button::new("print-preview").label("Pratinjau cetak").on_click(move |_,window,cx|{let _=start.update(cx,|s,cx|s.start_print(window,cx));})))
                .on_close(move |_,_,cx|{let _=close.update(cx,|s,cx|{s.print_dialog=None;s.clear_modal();cx.notify();});})
        });
        cx.notify();
    }
    fn start_print(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(ui) = self.print_dialog.clone() else {
            return;
        };
        let plan = match PrintPlan::new(ui.snapshot, &ui.range.read(cx).value()) {
            Ok(p) => p,
            Err(_) => {
                if let Some(ui) = &mut self.print_dialog {
                    ui.error =
                        Some("Rentang tidak valid. Gunakan nomor halaman, misalnya 1-3,5.".into());
                }
                window.refresh();
                cx.notify();
                return;
            }
        };
        window.close_dialog(cx);
        self.print_dialog = None;
        self.clear_modal();
        let control = ExportControl::with_budget(std::time::Duration::from_secs(1800));
        self.start_export_job(control.clone(), cx);
        self.export.printing = true;
        self.export.pages = plan.indices.len();
        self.export.png_total = plan.indices.len();
        let renderer = self.renderer.clone();
        let progress = self.export.png_done.clone();
        cx.spawn(async move |this, cx| {
            let render = renderer.clone();
            let task_control = control.clone();
            let captured = plan.clone();
            let task = cx.background_executor().spawn(async move {
                let dir = prepare_print(&captured, &render, &task_control, |done| {
                    progress.store(done, Ordering::SeqCst);
                })?;
                let status = pdf_platform::printing::show(
                    dir.path(),
                    include_str!("../resources/print-helper.ps1"),
                    &task_control,
                )?;
                Ok::<_, ExportError>((dir, status))
            });
            let result = task.await;
            let result = match result {
                Ok((dir, status)) if status == "pdf" => {
                    let parent = plan.snapshot.sources()[0]
                        .path
                        .parent()
                        .unwrap_or_else(|| std::path::Path::new("."))
                        .to_path_buf();
                    let picker = this.update(cx, |_, cx| {
                        cx.prompt_for_new_path(&parent, Some("Origo-cetak.pdf"))
                    });
                    let destination = match picker {
                        Ok(picker) => match picker.await {
                            Ok(Ok(Some(p))) => Some(p),
                            _ => None,
                        },
                        Err(_) => None,
                    };
                    if let Some(destination) = destination {
                        let task_control = control.clone();
                        let snapshot = plan.snapshot.clone();
                        let count = plan.indices.len();
                        cx.background_executor()
                            .spawn(async move {
                                publish_print_pdf(
                                    &dir.path().join("output.pdf"),
                                    &destination,
                                    &snapshot,
                                    count,
                                    &renderer,
                                    &task_control,
                                )
                            })
                            .await
                            .map(|_| "PDF hasil cetak disimpan ke file baru.".to_string())
                    } else {
                        Err(ExportError::Cancelled)
                    }
                }
                Ok((_dir, _)) => Ok("Dikirim ke antrean printer.".into()),
                Err(error) => Err(error),
            };
            let _ = this.update(cx, |s, cx| {
                s.resources.export_owner.store(0, Ordering::SeqCst);
                s.export.running = false;
                s.export.printing = false;
                s.export.control = None;
                s.export.completed += 1;
                let success = result.is_ok();
                s.export.success = success;
                match result {
                    Ok(message) => s.export.notice = Some(message),
                    Err(ExportError::Cancelled) => {
                        s.export.notice = Some(
                            "Cetak dibatalkan. Halaman yang sudah dikirim mungkin tetap tercetak."
                                .into(),
                        )
                    }
                    Err(error) => s.show_error_popup("Cetak belum selesai", error.to_string(), cx),
                }
                s.transition_export_finished(success, false, cx);
                s.report(cx);
                cx.notify();
            });
        })
        .detach();
    }
}
pub(super) fn prepare_print(
    plan: &PrintPlan,
    renderer: &PdfiumRenderer,
    control: &ExportControl,
    mut progress: impl FnMut(usize),
) -> Result<tempfile::TempDir, ExportError> {
    let dir = tempfile::tempdir().map_err(|_| ExportError::Io)?;
    let mut pages = Vec::new();
    let mut remaining = 512 * 1024 * 1024;
    control.set_phase(ExportPhase::Writing);
    for (position, index) in plan.indices.iter().enumerate() {
        control.checkpoint()?;
        let page = &plan.snapshot.pages()[*index];
        let source = plan
            .snapshot
            .sources()
            .iter()
            .find(|s| s.id == page.entry.source_id)
            .ok_or(ExportError::InvalidPlan)?;
        let (width, height) =
            dimensions(page.geometry, PngDpi::Dpi150).map_err(|_| ExportError::ResourceLimit)?;
        let raster = renderer
            .render_print(&ExactRenderRequest {
                path: source.path.clone(),
                page_index: page.entry.source_page_index,
                rotation_delta: page.entry.rotation_delta,
                fingerprint: source.fingerprint,
                width,
                height,
                annotations: Some(page.annotations.clone()),
            })
            .map_err(|_| ExportError::Validation)?;
        let file = format!("page-{position:04}.png");
        let size = pdf_platform::png::LocalPngWriter
            .write_png(&raster, &dir.path().join(&file), remaining, control)
            .map_err(|_| ExportError::Io)?;
        remaining -= size;
        pages.push(serde_json::json!({"file":file,"width":page.geometry.width_points,"height":page.geometry.height_points,"label":index+1}));
        progress(position + 1);
    }
    for source in plan.snapshot.sources() {
        if pdf_platform::fingerprint(&source.path)? != source.fingerprint {
            return Err(ExportError::SourceChanged);
        }
    }
    std::fs::write(
        dir.path().join("job.json"),
        serde_json::to_vec(&serde_json::json!({"pages":pages})).map_err(|_| ExportError::Io)?,
    )
    .map_err(|_| ExportError::Io)?;
    Ok(dir)
}
fn publish_print_pdf(
    source: &std::path::Path,
    destination: &std::path::Path,
    snapshot: &ExportPlan,
    count: usize,
    renderer: &PdfiumRenderer,
    control: &ExportControl,
) -> Result<(), ExportError> {
    control.checkpoint()?;
    if std::fs::metadata(source)
        .map_err(|_| ExportError::Io)?
        .len()
        > 128 * 1024 * 1024
    {
        return Err(ExportError::ResourceLimit);
    }
    let transaction = pdf_platform::LocalOutput.stage(snapshot, destination, control)?;
    std::fs::copy(source, transaction.output()).map_err(|_| ExportError::Io)?;
    let info = renderer
        .inspect_output(transaction.output())
        .map_err(|_| ExportError::Validation)?;
    if info.pages.len() != count {
        return Err(ExportError::Validation);
    }
    transaction.verify_sources()?;
    transaction.commit(control)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn virtual_print_publication_never_replaces_source_or_existing_file() -> anyhow::Result<()> {
        let dir = tempfile::tempdir()?;
        let source = dir.path().join("source.pdf");
        std::fs::write(&source, b"source bytes")?;
        let output = dir.path().join("printed.pdf");
        std::fs::write(&output, b"generated bytes")?;
        let w = pdf_domain::Workspace::new(pdf_domain::SourceDocument {
            id: pdf_domain::SourceId(1),
            path: source.clone(),
            fingerprint: pdf_platform::fingerprint(&source)?,
            geometries: vec![pdf_domain::PageGeometry {
                width_points: 300.,
                height_points: 420.,
                quarter_turns: 0,
            }],
            annotations: vec![],
            export_support: pdf_domain::ExportSupport::NotPreflighted,
        })?;
        let plan = ExportPlan::capture(&w, 1, ExportScope::All)?;
        let renderer = PdfiumRenderer::new(dir.path());
        let control = ExportControl::default();
        assert!(publish_print_pdf(&output, &source, &plan, 1, &renderer, &control).is_err());
        assert!(publish_print_pdf(&output, &output, &plan, 1, &renderer, &control).is_err());
        assert_eq!(std::fs::read(&source)?, b"source bytes");
        assert_eq!(std::fs::read(&output)?, b"generated bytes");
        Ok(())
    }
}
