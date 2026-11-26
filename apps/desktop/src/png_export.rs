//! PNG configuration and an origin-owned background job; never saves PDF edits.
use crate::*;
use gpui::component::{
    Disableable, Selectable, WindowExt,
    button::{Button, ButtonVariants},
    dialog::DialogFooter,
    input::Input,
};
use pdf_application::{
    export::{ExportControl, ExportError, ExportPlan, ExportScope},
    png::{ImageExportFormat, PngDpi, PngError, PngPlan, PngReceipt, finish_png, prepare_png},
    split::{SplitError, validate_folder},
};
use pdf_platform::png::{LocalPngOutput, LocalPngWriter};
use std::sync::atomic::Ordering;

#[derive(Clone)]
pub(super) struct PngDialog {
    all: ExportPlan,
    selected: Option<ExportPlan>,
    selected_only: bool,
    dpi: PngDpi,
    format: ImageExportFormat,
    jpeg_quality: u8,
    folder: Entity<InputState>,
    error: Option<String>,
}
impl PngDialog {
    fn plan(&self) -> Result<PngPlan, PngError> {
        let snapshot = if self.selected_only {
            self.selected.as_ref().ok_or(ExportError::Empty)?
        } else {
            &self.all
        };
        Ok(PngPlan::new(snapshot.clone(), self.dpi)?.with_format(self.format))
    }
}
impl DocumentSession {
    pub(super) fn show_png(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(owner) = &self.document_owner {
            let _ = owner.update(cx, |s, cx| s.show_png(window, cx));
            return;
        }
        if self.edits_locked()
            || self.opening
            || self.resources.export_owner.load(Ordering::SeqCst) != 0
        {
            return;
        }
        let Some(workspace) = self.workspace.as_ref() else {
            return;
        };
        let all = match ExportPlan::capture(&workspace, self.workspace_instance, ExportScope::All) {
            Ok(plan) => plan,
            Err(error) => {
                drop(workspace);
                self.show_error_popup("Gambar belum dapat diekspor", error.to_string(), cx);
                return;
            }
        };
        let selected =
            ExportPlan::capture(&workspace, self.workspace_instance, ExportScope::Selected).ok();
        drop(workspace);
        let folder = cx.new(|cx| {
            let mut input = InputState::new(window, cx);
            input.set_value("hasil-png", window, cx);
            input
        });
        cx.subscribe_in(
            &folder,
            window,
            |this, _, event: &InputEvent, window, cx| {
                if matches!(event, InputEvent::Change) {
                    if let Some(dialog) = &mut this.png_dialog {
                        dialog.error = None;
                    }
                    window.refresh();
                    cx.notify();
                }
            },
        )
        .detach();
        self.png_dialog = Some(PngDialog {
            all,
            selected,
            selected_only: false,
            dpi: PngDpi::Dpi150,
            format: ImageExportFormat::Png,
            jpeg_quality: 90,
            folder,
            error: None,
        });
        self.error_popup = true;
        self.resources
            .modal_owner
            .store(self.tab_id.0, Ordering::SeqCst);
        self.page_dragging = false;
        self.text.dragging = false;
        self.text.pending = None;
        self.text.sentence_anchor = None;
        let weak = cx.weak_entity();
        window.open_alert_dialog(cx, move |dialog, _, cx| {
            let Some(ui) = weak
                .read_with(cx, |this, _| this.png_dialog.clone())
                .ok()
                .flatten()
            else {
                return dialog;
            };
            let plan = ui.plan();
            let preview = match &plan {
                Ok(plan) => format!(
                    "{} {} · {} DPI · {} dan seterusnya",
                    plan.page_count(),
                    plan.format().detail_label(),
                    plan.dpi().value(),
                    plan.output_filename(0)
                ),
                Err(error) => error.to_string(),
            };
            let is_jpeg = matches!(ui.format, ImageExportFormat::Jpeg { .. });
            let all = weak.clone();
            let selected = weak.clone();
            let fmt_png = weak.clone();
            let fmt_jpeg = weak.clone();
            let q80 = weak.clone();
            let q90 = weak.clone();
            let q95 = weak.clone();
            let normal = weak.clone();
            let high = weak.clone();
            let cancel = weak.clone();
            let start = weak.clone();
            let closed = weak.clone();
            dialog
                .title(
                    gpui::div()
                        .id("png-title")
                        .role(gpui::Role::Group)
                        .aria_label("Ekspor Gambar (PNG / JPEG)")
                        .child("Ekspor Gambar (PNG / JPEG)"),
                )
                .description("Satu gambar berlatar putih per halaman sesuai susunan dan rotasi saat ini. Teks, tautan, dan bookmark tidak interaktif pada hasil gambar. File sumber tetap utuh.")
                .centered()
                .width(px(600.))
                .close_button(true)
                .child(
                    gpui::div()
                        .flex()
                        .gap_2()
                        .child(
                            Button::new("png-all")
                                .label("Semua halaman")
                                .selected(!ui.selected_only)
                                .on_click(move |_, window, cx| {
                                    let _ = all.update(cx, |this, cx| {
                                        this.png_options(Some(false), None, None, window, cx)
                                    });
                                }),
                        )
                        .child(
                            Button::new("png-selected")
                                .label("Halaman terpilih")
                                .selected(ui.selected_only)
                                .disabled(ui.selected.is_none())
                                .on_click(move |_, window, cx| {
                                    let _ = selected.update(cx, |this, cx| {
                                        this.png_options(Some(true), None, None, window, cx)
                                    });
                                }),
                        ),
                )
                .child(
                    gpui::div()
                        .flex()
                        .gap_2()
                        .mt_3()
                        .child(
                            Button::new("img-fmt-png")
                                .label("PNG (Lossless)")
                                .selected(!is_jpeg)
                                .on_click(move |_, window, cx| {
                                    let _ = fmt_png.update(cx, |this, cx| {
                                        this.png_options(
                                            None,
                                            None,
                                            Some(ImageExportFormat::Png),
                                            window,
                                            cx,
                                        )
                                    });
                                }),
                        )
                        .child(
                            Button::new("img-fmt-jpeg")
                                .label("JPEG (Hemat ukuran)")
                                .selected(is_jpeg)
                                .on_click(move |_, window, cx| {
                                    let _ = fmt_jpeg.update(cx, |this, cx| {
                                        let q = this
                                            .png_dialog
                                            .as_ref()
                                            .map(|d| d.jpeg_quality)
                                            .unwrap_or(90);
                                        this.png_options(
                                            None,
                                            None,
                                            Some(ImageExportFormat::Jpeg { quality: q }),
                                            window,
                                            cx,
                                        )
                                    });
                                }),
                        ),
                )
                .when(is_jpeg, |el| {
                    el.child(
                        gpui::div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .mt_2()
                            .child(gpui::div().text_sm().child("Kualitas JPEG:"))
                            .child(
                                Button::new("jpeg-q-80")
                                    .label("80%")
                                    .selected(ui.jpeg_quality == 80)
                                    .on_click(move |_, window, cx| {
                                        let _ = q80.update(cx, |this, cx| {
                                            this.png_options(
                                                None,
                                                None,
                                                Some(ImageExportFormat::Jpeg { quality: 80 }),
                                                window,
                                                cx,
                                            )
                                        });
                                    }),
                            )
                            .child(
                                Button::new("jpeg-q-90")
                                    .label("90%")
                                    .selected(ui.jpeg_quality == 90)
                                    .on_click(move |_, window, cx| {
                                        let _ = q90.update(cx, |this, cx| {
                                            this.png_options(
                                                None,
                                                None,
                                                Some(ImageExportFormat::Jpeg { quality: 90 }),
                                                window,
                                                cx,
                                            )
                                        });
                                    }),
                            )
                            .child(
                                Button::new("jpeg-q-95")
                                    .label("95%")
                                    .selected(ui.jpeg_quality == 95)
                                    .on_click(move |_, window, cx| {
                                        let _ = q95.update(cx, |this, cx| {
                                            this.png_options(
                                                None,
                                                None,
                                                Some(ImageExportFormat::Jpeg { quality: 95 }),
                                                window,
                                                cx,
                                            )
                                        });
                                    }),
                            ),
                    )
                })
                .child(
                    gpui::div()
                        .flex()
                        .gap_2()
                        .mt_3()
                        .child(
                            Button::new("png-150")
                                .label("150 DPI")
                                .selected(ui.dpi == PngDpi::Dpi150)
                                .on_click(move |_, window, cx| {
                                    let _ = normal.update(cx, |this, cx| {
                                        this.png_options(None, Some(PngDpi::Dpi150), None, window, cx)
                                    });
                                }),
                        )
                        .child(
                            Button::new("png-300")
                                .label("300 DPI")
                                .selected(ui.dpi == PngDpi::Dpi300)
                                .on_click(move |_, window, cx| {
                                    let _ = high.update(cx, |this, cx| {
                                        this.png_options(None, Some(PngDpi::Dpi300), None, window, cx)
                                    });
                                }),
                        ),
                )
                .child(
                    gpui::div()
                        .flex()
                        .flex_col()
                        .gap_2()
                        .mt_3()
                        .child("Nama folder baru (pilih folder induk setelah ini)")
                        .child(Input::new(&ui.folder).aria_label("Nama folder gambar"))
                        .child(
                            gpui::div()
                                .id("png-preview")
                                .role(gpui::Role::Group)
                                .aria_label(preview.clone())
                                .text_sm()
                                .child(preview),
                        )
                        .when_some(ui.error.clone(), |el, error| {
                            el.child(
                                gpui::div()
                                    .id("png-error")
                                    .role(gpui::Role::Group)
                                    .aria_label(error.clone())
                                    .child(error),
                            )
                        }),
                )
                .footer(
                    DialogFooter::new()
                        .child(Button::new("png-cancel").label("Batal").on_click(
                            move |_, window, cx| {
                                window.close_dialog(cx);
                                let _ = cancel.update(cx, |this, cx| this.close_png(cx));
                            },
                        ))
                        .child(
                            Button::new("png-start")
                                .primary()
                                .label("Pilih folder induk")
                                .disabled(plan.is_err())
                                .on_click(move |_, window, cx| {
                                    let _ = start.update(cx, |this, cx| this.submit_png(window, cx));
                                }),
                        ),
                )
                .on_close(move |_, _, cx| {
                    let _ = closed.update(cx, |this, cx| this.close_png(cx));
                })
        });
        cx.notify();
    }
    fn png_options(
        &mut self,
        selected: Option<bool>,
        dpi: Option<PngDpi>,
        format: Option<ImageExportFormat>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let mut folder_swap: Option<(Entity<InputState>, &'static str)> = None;
        if let Some(ui) = &mut self.png_dialog {
            if let Some(selected) = selected {
                ui.selected_only = selected && ui.selected.is_some();
            }
            if let Some(dpi) = dpi {
                ui.dpi = dpi;
            }
            if let Some(fmt) = format {
                ui.format = fmt;
                if let ImageExportFormat::Jpeg { quality } = fmt {
                    ui.jpeg_quality = quality;
                }
                let current_name = ui.folder.read(cx).value().to_string();
                let next_default = match fmt {
                    ImageExportFormat::Png if current_name == "hasil-jpeg" => Some("hasil-png"),
                    ImageExportFormat::Jpeg { .. } if current_name == "hasil-png" => {
                        Some("hasil-jpeg")
                    }
                    _ => None,
                };
                if let Some(next_name) = next_default {
                    folder_swap = Some((ui.folder.clone(), next_name));
                }
            }
            ui.error = None;
        }
        if let Some((folder, next_name)) = folder_swap {
            folder.update(cx, |input, cx| input.set_value(next_name, window, cx));
        }
        window.refresh();
        cx.notify();
    }
    fn close_png(&mut self, cx: &mut Context<Self>) {
        self.png_dialog = None;
        self.clear_modal();
        self.restore_visible_focus(cx);
        cx.notify();
    }
    fn submit_png(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(ui) = self.png_dialog.as_ref() else {
            return;
        };
        let name = ui.folder.read(cx).value().to_string();
        let plan = validate_folder(&name)
            .map_err(PngError::from)
            .and_then(|_| ui.plan());
        let plan = match plan {
            Ok(plan) => plan,
            Err(error) => {
                if let Some(ui) = &mut self.png_dialog {
                    ui.error = Some(error.to_string());
                }
                window.refresh();
                cx.notify();
                return;
            }
        };
        window.close_dialog(cx);
        self.close_png(cx);
        self.start_png(plan, name, cx);
    }
    fn start_png(&mut self, plan: PngPlan, name: String, cx: &mut Context<Self>) {
        if self.resources.export_owner.load(Ordering::SeqCst) != 0 {
            return;
        }
        let picker = cx.prompt_for_paths(gpui::PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: Some(format!("Pilih folder induk hasil {}", plan.format().label()).into()),
        });
        let control = ExportControl::with_budget(std::time::Duration::from_secs(1800));
        self.start_export_job(control.clone(), cx);
        self.export.png = true;
        self.export.png_total = plan.page_count();
        let progress = self.export.png_done.clone();
        let preflight = self.exporter.clone();
        let renderer = self.renderer.clone();
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
                    let _ = this.update(cx, |this, cx| this.png_finished(Err(error.into()), cx));
                    return;
                }
            };
            let result = cx
                .background_executor()
                .spawn(async move {
                    let prepared =
                        prepare_png(plan, &destination, &LocalPngOutput, &preflight, &control)?;
                    finish_png(prepared, &renderer, &LocalPngWriter, &control, |count| {
                        progress.store(count, Ordering::SeqCst);
                    })
                })
                .await;
            let _ = this.update(cx, |this, cx| this.png_finished(result, cx));
        })
        .detach();
    }
    fn png_finished(&mut self, result: Result<PngReceipt, PngError>, cx: &mut Context<Self>) {
        self.resources.export_owner.store(0, Ordering::SeqCst);
        self.export.running = false;
        self.export.control = None;
        self.export.completed += 1;
        self.export.success = result.is_ok();
        self.export.pages = 0;
        let success = result.is_ok();
        match result {
            Ok(receipt) => {
                self.export.pages = receipt.pages;
                self.export.notice = Some(format!(
                    "{} {} ({} DPI) tersimpan di {}. Status tersimpan PDF tidak berubah.",
                    receipt.pages,
                    receipt.format.detail_label(),
                    receipt.dpi.value(),
                    receipt.destination.display()
                ));
            }
            Err(
                PngError::Export(ExportError::Cancelled)
                | PngError::Batch(SplitError::Export(ExportError::Cancelled)),
            ) => {
                self.export.notice =
                    Some("Ekspor gambar dibatalkan. Folder hasil tidak diterbitkan.".into());
            }
            Err(error) => {
                self.export.notice = None;
                self.show_error_popup("Gambar belum dapat diekspor", error.to_string(), cx);
            }
        }
        self.transition_export_finished(success, false, cx);
        self.report(cx);
        cx.notify();
    }
}
