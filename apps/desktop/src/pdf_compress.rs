//! Window-level dialog and background job for Smart PDF Compression (DPI/quality presets + grayscale).
use crate::images_to_pdf::SHELL_EXPORT_OWNER;
use crate::shell::{MAX_TABS, WindowShell};
use crate::*;
use gpui::component::{
    ActiveTheme, Disableable, Selectable, Sizable, StyledExt, WindowExt,
    button::{Button, ButtonVariants},
    dialog::DialogFooter,
};
use pdf_application::{
    compress::{
        CompressError, CompressOptions, CompressPlan, CompressPreset, CompressReceipt,
        format_bytes, run_compress_pdf,
    },
    export::{ExportControl, ExportPlan, ExportScope},
};
use pdf_platform::compress::LocalPdfCompressor;
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

#[derive(Clone)]
pub(super) struct CompressDialog {
    pub snapshot: ExportPlan,
    pub source_path: PathBuf,
    pub baseline_bytes: u64,
    pub preset: CompressPreset,
    pub grayscale: bool,
    pub open_after: bool,
    pub custom_destination: Option<PathBuf>,
    pub picking: bool,
    pub error: Option<String>,
}

impl CompressDialog {
    fn destination(&self) -> PathBuf {
        if let Some(custom) = &self.custom_destination {
            custom.clone()
        } else {
            suggested_compress_destination(&self.source_path, self.preset, self.grayscale)
        }
    }
}

pub(super) struct CompressJob {
    pub preset: CompressPreset,
    pub grayscale: bool,
    pub total: usize,
    pub done: Arc<AtomicUsize>,
    pub control: ExportControl,
    pub open_after: bool,
    pub job_id: u64,
}

#[derive(Default)]
pub(super) struct CompressState {
    pub dialog: Option<CompressDialog>,
    pub job: Option<CompressJob>,
    pub job_seq: u64,
    pub completed: u64,
    pub success: bool,
    pub last_before_bytes: u64,
    pub last_after_bytes: u64,
}

impl CompressState {
    pub(super) fn status_label(&self) -> String {
        let Some(job) = &self.job else {
            return String::new();
        };
        let done = job.done.load(Ordering::SeqCst);
        let mode = if job.grayscale {
            format!(
                "{}, {} DPI, Grayscale",
                job.preset.short_label(),
                job.preset.dpi()
            )
        } else {
            format!("{}, {} DPI", job.preset.short_label(), job.preset.dpi())
        };
        format!("Mengompres PDF ({mode}): {done}/{} halaman…", job.total)
    }
}

fn suggested_compress_name(
    source: &std::path::Path,
    preset: CompressPreset,
    grayscale: bool,
) -> String {
    let stem = source
        .file_stem()
        .and_then(|s| s.to_str())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or("dokumen");
    let suffix = preset.suffix();
    if grayscale {
        format!("{stem}-{suffix}-bw.pdf")
    } else {
        format!("{stem}-{suffix}.pdf")
    }
}

fn suggested_compress_destination(
    source: &std::path::Path,
    preset: CompressPreset,
    grayscale: bool,
) -> PathBuf {
    let dir = source
        .parent()
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    let stem = source
        .file_stem()
        .and_then(|s| s.to_str())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or("dokumen");
    let base = if grayscale {
        format!("{stem}-{}-bw", preset.suffix())
    } else {
        format!("{stem}-{}", preset.suffix())
    };
    let candidate = dir.join(format!("{base}.pdf"));
    if !candidate.exists() {
        return candidate;
    }
    for n in 2..=9999u32 {
        let next = dir.join(format!("{base} ({n}).pdf"));
        if !next.exists() {
            return next;
        }
    }
    candidate
}

impl WindowShell {
    pub(super) fn show_compress_pdf(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.close.is_some()
            || self.modal(cx)
            || self.resources.export_owner.load(Ordering::SeqCst) != 0
            || self.compress.job.is_some()
        {
            return;
        }
        let Some(tab) = self.active_tab() else {
            return;
        };
        let source_path = tab.path.clone();
        let Some(session) = self.session() else {
            return;
        };
        let captured = {
            let s = session.read(cx);
            let Some(workspace) = s.workspace.as_ref() else {
                return;
            };
            ExportPlan::capture(&workspace, s.workspace_instance, ExportScope::All)
        };
        let snapshot = match captured {
            Ok(plan) => plan,
            Err(err) => {
                self.notice = Some(format!("PDF belum dapat dikompres: {err}"));
                cx.notify();
                return;
            }
        };
        let baseline_bytes = snapshot
            .sources()
            .iter()
            .map(|src| src.fingerprint.bytes)
            .fold(0u64, u64::saturating_add);

        self.compress.dialog = Some(CompressDialog {
            snapshot,
            source_path,
            baseline_bytes,
            preset: CompressPreset::Balanced,
            grayscale: false,
            open_after: true,
            custom_destination: None,
            picking: false,
            error: None,
        });
        self.resources
            .modal_owner
            .store(SHELL_EXPORT_OWNER, Ordering::SeqCst);
        self.report(cx);

        let weak = cx.weak_entity();
        window.open_alert_dialog(cx, move |dialog, _, cx| {
            let Some(ui) = weak
                .read_with(cx, |this, _| this.compress.dialog.clone())
                .ok()
                .flatten()
            else {
                return dialog;
            };
            let colors = cx.theme().colors;
            let source_name = ui
                .source_path
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_else(|| ui.source_path.display().to_string());
            let page_count = ui.snapshot.pages().len();
            let dest_label = ui.destination().display().to_string();

            let toggle_gray = weak.clone();
            let toggle_open = weak.clone();
            let pick_dest = weak.clone();
            let cancel = weak.clone();
            let submit = weak.clone();
            let closed = weak.clone();

            let mut preset_list = gpui::div().flex().flex_col().gap_2();
            for preset in CompressPreset::ALL {
                let selected = ui.preset == preset;
                let choose = weak.clone();
                let id = match preset {
                    CompressPreset::High => "compress-preset-high",
                    CompressPreset::Balanced => "compress-preset-balanced",
                    CompressPreset::Compact => "compress-preset-compact",
                };
                preset_list = preset_list.child(
                    gpui::div()
                        .id(id)
                        .role(gpui::Role::Button)
                        .aria_label(preset.label())
                        .cursor_pointer()
                        .px_3()
                        .py_2()
                        .rounded_md()
                        .border_1()
                        .border_color(if selected {
                            colors.primary
                        } else {
                            colors.border
                        })
                        .bg(if selected {
                            colors.secondary
                        } else {
                            colors.background
                        })
                        .flex()
                        .flex_col()
                        .gap_0p5()
                        .on_click(move |_, window, cx| {
                            let _ = choose.update(cx, |this, cx| {
                                if let Some(d) = &mut this.compress.dialog {
                                    d.preset = preset;
                                    d.error = None;
                                }
                                window.refresh();
                                cx.notify();
                            });
                        })
                        .child(
                            gpui::div()
                                .flex()
                                .items_center()
                                .justify_between()
                                .child(gpui::div().text_sm().font_semibold().child(preset.label()))
                                .when(selected, |el| {
                                    el.child(
                                        gpui::div()
                                            .text_xs()
                                            .font_semibold()
                                            .text_color(colors.primary)
                                            .child("Dipilih"),
                                    )
                                }),
                        )
                        .child(
                            gpui::div()
                                .text_xs()
                                .text_color(colors.muted_foreground)
                                .child(preset.description()),
                        ),
                );
            }

            let mut body = gpui::div()
                .flex()
                .flex_col()
                .gap_3()
                .child(
                    gpui::div()
                        .px_2p5()
                        .py_1p5()
                        .rounded_md()
                        .border_1()
                        .border_color(colors.border)
                        .bg(colors.muted)
                        .text_xs()
                        .child(format!(
                            "{source_name} · {page_count} halaman · Ukuran saat ini: {}",
                            format_bytes(ui.baseline_bytes)
                        )),
                )
                .child(
                    gpui::div()
                        .flex()
                        .flex_col()
                        .gap_1p5()
                        .child(
                            gpui::div()
                                .text_xs()
                                .font_semibold()
                                .child("Pilih tingkat kompresi"),
                        )
                        .child(preset_list),
                )
                .child(
                    gpui::div()
                        .flex()
                        .flex_wrap()
                        .gap_2()
                        .child(
                            Button::new("compress-grayscale")
                                .small()
                                .label(if ui.grayscale {
                                    "Skala abu-abu (Grayscale): Aktif"
                                } else {
                                    "Skala abu-abu (Grayscale): Nonaktif"
                                })
                                .selected(ui.grayscale)
                                .on_click(move |_, window, cx| {
                                    let _ = toggle_gray.update(cx, |this, cx| {
                                        if let Some(d) = &mut this.compress.dialog {
                                            d.grayscale = !d.grayscale;
                                            d.error = None;
                                        }
                                        window.refresh();
                                        cx.notify();
                                    });
                                }),
                        )
                        .child(
                            Button::new("compress-open-after")
                                .small()
                                .label(if ui.open_after {
                                    "Buka PDF hasil di Origo: Ya"
                                } else {
                                    "Buka PDF hasil di Origo: Tidak"
                                })
                                .selected(ui.open_after)
                                .on_click(move |_, window, cx| {
                                    let _ = toggle_open.update(cx, |this, cx| {
                                        if let Some(d) = &mut this.compress.dialog {
                                            d.open_after = !d.open_after;
                                        }
                                        window.refresh();
                                        cx.notify();
                                    });
                                }),
                        ),
                )
                .child(
                    gpui::div()
                        .flex()
                        .flex_col()
                        .gap_1()
                        .child(gpui::div().text_xs().font_semibold().child("Lokasi simpan"))
                        .child(
                            gpui::div()
                                .flex()
                                .items_center()
                                .gap_2()
                                .child(
                                    gpui::div()
                                        .flex_1()
                                        .px_2p5()
                                        .py_1p5()
                                        .rounded_md()
                                        .border_1()
                                        .border_color(colors.border)
                                        .bg(colors.muted)
                                        .text_xs()
                                        .overflow_hidden()
                                        .child(dest_label),
                                )
                                .child(
                                    Button::new("compress-pick-dest")
                                        .outline()
                                        .small()
                                        .label("Ubah Lokasi…")
                                        .disabled(ui.picking)
                                        .on_click(move |_, _, cx| {
                                            let _ = pick_dest.update(cx, |this, cx| {
                                                this.pick_compress_destination(cx);
                                            });
                                        }),
                                ),
                        ),
                )
                .child(
                    gpui::div()
                        .text_xs()
                        .text_color(colors.muted_foreground)
                        .child(
                            "Catatan: Kompresi ukuran merasterisasi halaman menjadi gambar terkompresi (sangat efektif untuk scan/foto; teks tidak terseleksi lagi). Untuk menjaga teks vektor asli, gunakan Optimize PDF (lossless).",
                        ),
                );

            if let Some(err) = ui.error.clone() {
                body = body.child(
                    gpui::div()
                        .id("compress-error")
                        .role(gpui::Role::Group)
                        .aria_label(err.clone())
                        .text_xs()
                        .text_color(cx.theme().danger)
                        .child(err),
                );
            }

            dialog
                .title(
                    gpui::div()
                        .id("compress-dialog-title")
                        .role(gpui::Role::Group)
                        .aria_label("Kompres PDF (Ukuran)")
                        .child("Kompres PDF (Ukuran)"),
                )
                .description(
                    "Perkecil ukuran PDF secara lokal (offline) ke file baru. File asli tetap utuh.",
                )
                .centered()
                .width(px(560.))
                .close_button(true)
                .child(body)
                .footer(
                    DialogFooter::new()
                        .child(Button::new("compress-cancel").label("Batal").on_click(
                            move |_, window, cx| {
                                window.close_dialog(cx);
                                let _ = cancel.update(cx, |this, cx| {
                                    this.close_compress_dialog(cx);
                                });
                            },
                        ))
                        .child(
                            Button::new("compress-submit")
                                .primary()
                                .disabled(ui.picking)
                                .label("Kompres PDF")
                                .on_click(move |_, window, cx| {
                                    let _ = submit.update(cx, |this, cx| {
                                        this.submit_compress_dialog(window, cx);
                                    });
                                }),
                        ),
                )
                .on_close(move |_, _, cx| {
                    let _ = closed.update(cx, |this, cx| this.close_compress_dialog(cx));
                })
        });
        cx.notify();
    }

    fn pick_compress_destination(&mut self, cx: &mut Context<Self>) {
        let Some(dialog) = &mut self.compress.dialog else {
            return;
        };
        if dialog.picking {
            return;
        }
        dialog.picking = true;
        dialog.error = None;
        let suggested =
            suggested_compress_name(&dialog.source_path, dialog.preset, dialog.grayscale);
        let start_dir = dialog
            .custom_destination
            .as_ref()
            .and_then(|p| p.parent())
            .or_else(|| dialog.source_path.parent())
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("."));
        let prompt = cx.prompt_for_new_path(&start_dir, Some(&suggested));
        let handle = self.handle;
        cx.spawn(async move |this, cx| {
            let reply = prompt.await;
            cx.update(|cx| {
                let _ = handle.update(cx, |_, window, cx| {
                    let _ = this.update(cx, |this, cx| {
                        if let Some(dialog) = &mut this.compress.dialog {
                            dialog.picking = false;
                            if let Ok(Ok(Some(path))) = reply {
                                let dest = if path
                                    .extension()
                                    .is_some_and(|e| e.eq_ignore_ascii_case("pdf"))
                                {
                                    path
                                } else {
                                    path.with_extension("pdf")
                                };
                                dialog.custom_destination = Some(dest);
                                dialog.error = None;
                            }
                        }
                        window.refresh();
                        cx.notify();
                    });
                });
            });
        })
        .detach();
        cx.notify();
    }

    fn close_compress_dialog(&mut self, cx: &mut Context<Self>) {
        self.compress.dialog = None;
        let _ = self.resources.modal_owner.compare_exchange(
            SHELL_EXPORT_OWNER,
            0,
            Ordering::SeqCst,
            Ordering::SeqCst,
        );
        self.report(cx);
        cx.notify();
    }

    fn submit_compress_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(dialog) = self.compress.dialog.clone() else {
            return;
        };
        let destination = dialog.destination();
        let options = CompressOptions {
            preset: dialog.preset,
            grayscale: dialog.grayscale,
        };
        let plan = match CompressPlan::new(dialog.snapshot.clone(), options) {
            Ok(plan) => plan,
            Err(err) => {
                if let Some(d) = &mut self.compress.dialog {
                    d.error = Some(err.to_string());
                }
                window.refresh();
                cx.notify();
                return;
            }
        };

        window.close_dialog(cx);
        self.close_compress_dialog(cx);

        if self
            .resources
            .export_owner
            .compare_exchange(0, SHELL_EXPORT_OWNER, Ordering::SeqCst, Ordering::SeqCst)
            .is_err()
        {
            self.notice = Some("Tunggu proses ekspor lain selesai sebelum mengompres PDF.".into());
            cx.notify();
            return;
        }

        let total = plan.page_count();
        let done = Arc::new(AtomicUsize::new(0));
        let control = ExportControl::default();
        self.compress.job_seq += 1;
        let job_id = self.compress.job_seq;
        self.compress.job = Some(CompressJob {
            preset: options.preset,
            grayscale: options.grayscale,
            total,
            done: done.clone(),
            control: control.clone(),
            open_after: dialog.open_after,
            job_id,
        });
        self.notice = None;
        self.report(cx);
        cx.notify();

        let renderer = self.renderer.clone();
        let done_bg = done.clone();
        let control_bg = control.clone();
        let task = cx.background_executor().spawn(async move {
            run_compress_pdf(
                &plan,
                &destination,
                &renderer,
                &renderer,
                &LocalPdfCompressor,
                &control_bg,
                |completed| {
                    done_bg.store(completed, Ordering::SeqCst);
                },
            )
        });

        let poll_weak = cx.weak_entity();
        cx.spawn(async move |this, cx| {
            // Periodic UI progress updates while compressing
            let poller = cx.spawn(async move |cx| {
                loop {
                    cx.background_executor()
                        .timer(std::time::Duration::from_millis(120))
                        .await;
                    let running = poll_weak
                        .update(cx, |this, cx| {
                            if this
                                .compress
                                .job
                                .as_ref()
                                .is_some_and(|j| j.job_id == job_id)
                            {
                                cx.notify();
                                true
                            } else {
                                false
                            }
                        })
                        .unwrap_or(false);
                    if !running {
                        break;
                    }
                }
            });
            let result = task.await;
            drop(poller);
            let _ = this.update(cx, |this, cx| {
                this.finish_compress_job(job_id, result, cx);
            });
        })
        .detach();
    }

    pub(super) fn cancel_compress_job(&mut self, cx: &mut Context<Self>) {
        if let Some(job) = &self.compress.job {
            job.control.cancel();
            cx.notify();
        }
    }

    fn finish_compress_job(
        &mut self,
        job_id: u64,
        result: Result<CompressReceipt, CompressError>,
        cx: &mut Context<Self>,
    ) {
        let Some(job) = self.compress.job.take() else {
            return;
        };
        if job.job_id != job_id {
            self.compress.job = Some(job);
            return;
        }
        let _ = self.resources.export_owner.compare_exchange(
            SHELL_EXPORT_OWNER,
            0,
            Ordering::SeqCst,
            Ordering::SeqCst,
        );
        self.compress.completed += 1;
        match result {
            Ok(receipt) => {
                self.compress.success = true;
                self.compress.last_before_bytes = receipt.baseline_bytes;
                self.compress.last_after_bytes = receipt.output_bytes;
                self.notice = Some(receipt.summary_message());
                if job.open_after && self.tabs.len() < MAX_TABS {
                    self.open_paths(vec![receipt.destination], cx);
                }
            }
            Err(err) => {
                self.compress.success = false;
                self.notice = Some(err.to_string());
            }
        }
        self.report(cx);
        cx.notify();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn suggested_compress_destination_avoids_overwriting_existing_files()
    -> Result<(), Box<dyn std::error::Error>> {
        let dir = tempfile::tempdir()?;
        let src = dir.path().join("laporan.pdf");
        std::fs::write(&src, b"dummy")?;

        let first = suggested_compress_destination(&src, CompressPreset::Balanced, false);
        assert_eq!(first, dir.path().join("laporan-kompres.pdf"));

        std::fs::write(&first, b"existing")?;
        let second = suggested_compress_destination(&src, CompressPreset::Balanced, false);
        assert_eq!(second, dir.path().join("laporan-kompres (2).pdf"));

        let bw = suggested_compress_destination(&src, CompressPreset::Compact, true);
        assert_eq!(bw, dir.path().join("laporan-kompres-minimal-bw.pdf"));
        Ok(())
    }
}
