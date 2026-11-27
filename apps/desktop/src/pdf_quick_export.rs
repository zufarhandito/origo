//! Window-level dialogs and background jobs for quick exports:
//! - PDF -> UTF-8 Plain Text (.txt) in current ExportPlan page order
//! - Extract original embedded images (native JPEG & lossless PNG) via QPDF
use crate::images_to_pdf::SHELL_EXPORT_OWNER;
use crate::shell::WindowShell;
use crate::*;
use gpui::{
    component::{
        ActiveTheme, Disableable, Selectable, Sizable, WindowExt,
        button::{Button, ButtonVariants},
        dialog::DialogFooter,
        input::Input,
    },
    div,
};
use pdf_application::{
    export::{ExportControl, ExportError, ExportPlan, ExportScope},
    split::{SplitError, validate_folder},
    text_export::{TextExportOptions, extract_plan_text},
};
use pdf_platform::write_text_file_no_clobber;
use qpdf_adapter::{
    ExtractImagesError, ExtractImagesOptions, ExtractImagesReport, QpdfImageExtractor,
};
use std::{
    path::Path,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};

#[derive(Clone)]
pub(super) struct TxtExportDialog {
    pub all: ExportPlan,
    pub selected: Option<ExportPlan>,
    pub selected_only: bool,
    pub include_page_headers: bool,
    pub source_path: PathBuf,
    pub custom_destination: Option<PathBuf>,
    pub picking: bool,
    pub error: Option<String>,
}

impl TxtExportDialog {
    fn active_plan(&self) -> Result<&ExportPlan, ExportError> {
        if self.selected_only {
            self.selected.as_ref().ok_or(ExportError::Empty)
        } else {
            Ok(&self.all)
        }
    }

    fn destination(&self) -> PathBuf {
        if let Some(custom) = &self.custom_destination {
            custom.clone()
        } else {
            suggested_txt_destination(&self.source_path)
        }
    }
}

#[derive(Clone)]
pub(super) struct ExtractImagesDialog {
    pub source_path: PathBuf,
    pub skip_small: bool,
    pub skip_soft_masks: bool,
    pub folder: Entity<InputState>,
    pub error: Option<String>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum QuickExportKind {
    Txt,
    ExtractImages,
}

pub(super) struct QuickExportJob {
    pub kind: QuickExportKind,
    pub total: usize,
    pub done: Arc<AtomicUsize>,
    pub control: ExportControl,
    pub job_id: u64,
}

#[derive(Default)]
pub(super) struct QuickExportState {
    pub txt_dialog: Option<TxtExportDialog>,
    pub extract_images_dialog: Option<ExtractImagesDialog>,
    pub job: Option<QuickExportJob>,
    pub job_seq: u64,
}

impl QuickExportState {
    pub(super) fn has_dialog(&self) -> bool {
        self.txt_dialog.is_some() || self.extract_images_dialog.is_some()
    }

    pub(super) fn status_label(&self) -> String {
        let Some(job) = &self.job else {
            return String::new();
        };
        let done = job.done.load(Ordering::SeqCst);
        match job.kind {
            QuickExportKind::Txt => {
                if job.total > 0 {
                    format!("Mengekspor teks (TXT): halaman {done} / {}…", job.total)
                } else {
                    "Mengekspor teks (TXT)…".into()
                }
            }
            QuickExportKind::ExtractImages => {
                if done > 0 && job.total > 0 {
                    format!("Mengekstrak gambar asli: {done} / {} objek…", job.total)
                } else {
                    "Membaca gambar embedded dari PDF…".into()
                }
            }
        }
    }
}

struct TxtExportReceipt {
    destination: PathBuf,
    pages: usize,
    pages_with_text: usize,
    chars: usize,
    bytes: u64,
}

pub(crate) fn suggested_txt_destination(source: &Path) -> PathBuf {
    let parent = source
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let stem = source
        .file_stem()
        .and_then(|s| s.to_str())
        .filter(|s| !s.trim().is_empty())
        .unwrap_or("dokumen");
    let first = parent.join(format!("{stem}.txt"));
    if std::fs::symlink_metadata(&first).is_err() {
        return first;
    }
    for n in 2..=9999u32 {
        let candidate = parent.join(format!("{stem} ({n}).txt"));
        if std::fs::symlink_metadata(&candidate).is_err() {
            return candidate;
        }
    }
    parent.join(format!("{stem}-teks.txt"))
}

impl WindowShell {
    pub(super) fn can_start_quick_export(&self, cx: &App) -> bool {
        if self.quick_export.job.is_some()
            || self.quick_export.has_dialog()
            || self.resources.export_owner.load(Ordering::SeqCst) != 0
            || self.modal(cx)
        {
            return false;
        }
        let Some(session) = self.session() else {
            return false;
        };
        if self.active_tab().is_none() {
            return false;
        }
        let s = session.read(cx);
        !s.opening && !s.edits_locked() && s.workspace.is_some()
    }

    pub(super) fn show_txt_export_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.can_start_quick_export(cx) {
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
            let Some(ws) = s.workspace.as_ref() else {
                return;
            };
            let all = ExportPlan::capture(&ws, s.workspace_instance, ExportScope::All);
            let selected =
                ExportPlan::capture(&ws, s.workspace_instance, ExportScope::Selected).ok();
            (all, selected)
        };
        let (all, selected) = captured;
        let all = match all {
            Ok(plan) => plan,
            Err(error) => {
                self.notice = Some(format!("Teks belum dapat diekspor: {error}"));
                cx.notify();
                return;
            }
        };
        self.quick_export.txt_dialog = Some(TxtExportDialog {
            all,
            selected,
            selected_only: false,
            include_page_headers: true,
            source_path,
            custom_destination: None,
            picking: false,
            error: None,
        });
        self.resources
            .modal_owner
            .store(SHELL_EXPORT_OWNER, Ordering::SeqCst);

        let weak = cx.weak_entity();
        window.open_alert_dialog(cx, move |dialog, _, cx| {
            let Some(ui) = weak
                .read_with(cx, |this, _| this.quick_export.txt_dialog.clone())
                .ok()
                .flatten()
            else {
                return dialog;
            };
            let theme = cx.theme();
            let active_count = ui.active_plan().map(|p| p.pages().len()).unwrap_or(0);
            let dest_display = ui.destination().display().to_string();
            let scope_all = weak.clone();
            let scope_sel = weak.clone();
            let toggle_hdr = weak.clone();
            let pick_dest = weak.clone();
            let cancel = weak.clone();
            let start = weak.clone();
            let closed = weak.clone();

            dialog
                .title(
                    div()
                        .id("txt-export-title")
                        .role(gpui::Role::Group)
                        .aria_label("Ekspor Teks (TXT)")
                        .child("Ekspor Teks (TXT)"),
                )
                .description(
                    "Mengekstrak seluruh teks PDF ke file teks polos UTF-8 (.txt) sesuai urutan halaman saat ini. Untuk PDF hasil scan gambar, jalankan OCR terlebih dahulu.",
                )
                .centered()
                .width(px(580.))
                .close_button(true)
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_3()
                        .child(
                            div()
                                .flex()
                                .gap_2()
                                .child(
                                    Button::new("txt-scope-all")
                                        .label(format!("Semua halaman ({})", ui.all.pages().len()))
                                        .selected(!ui.selected_only)
                                        .on_click(move |_, window, cx| {
                                            let _ = scope_all.update(cx, |this, cx| {
                                                if let Some(d) = &mut this.quick_export.txt_dialog {
                                                    d.selected_only = false;
                                                    d.error = None;
                                                }
                                                window.refresh();
                                                cx.notify();
                                            });
                                        }),
                                )
                                .child(
                                    Button::new("txt-scope-sel")
                                        .label(match &ui.selected {
                                            Some(sel) => {
                                                format!("Halaman terpilih ({})", sel.pages().len())
                                            }
                                            None => "Halaman terpilih".into(),
                                        })
                                        .selected(ui.selected_only)
                                        .disabled(ui.selected.is_none())
                                        .on_click(move |_, window, cx| {
                                            let _ = scope_sel.update(cx, |this, cx| {
                                                if let Some(d) = &mut this.quick_export.txt_dialog
                                                    && d.selected.is_some()
                                                {
                                                    d.selected_only = true;
                                                    d.error = None;
                                                }
                                                window.refresh();
                                                cx.notify();
                                            });
                                        }),
                                ),
                        )
                        .child(
                            div().flex().items_center().gap_2().child(
                                Button::new("txt-toggle-headers")
                                    .small()
                                    .label(if ui.include_page_headers {
                                        "☑ Sertakan pemisah halaman (=== Halaman N ===)"
                                    } else {
                                        "☐ Sertakan pemisah halaman (=== Halaman N ===)"
                                    })
                                    .selected(ui.include_page_headers)
                                    .on_click(move |_, window, cx| {
                                        let _ = toggle_hdr.update(cx, |this, cx| {
                                            if let Some(d) = &mut this.quick_export.txt_dialog {
                                                d.include_page_headers = !d.include_page_headers;
                                            }
                                            window.refresh();
                                            cx.notify();
                                        });
                                    }),
                            ),
                        )
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .gap_1()
                                .p_2()
                                .rounded_md()
                                .border_1()
                                .border_color(theme.border)
                                .child(
                                    div()
                                        .flex()
                                        .items_center()
                                        .justify_between()
                                        .gap_2()
                                        .child(
                                            div()
                                                .text_xs()
                                                .text_color(theme.muted_foreground)
                                                .child(format!(
                                                    "File tujuan ({active_count} halaman · UTF-8):"
                                                )),
                                        )
                                        .child(
                                            Button::new("txt-pick-dest")
                                                .small()
                                                .ghost()
                                                .label("Ubah tujuan…")
                                                .disabled(ui.picking)
                                                .on_click(move |_, window, cx| {
                                                    let _ = pick_dest.update(cx, |this, cx| {
                                                        this.pick_txt_destination(window, cx);
                                                    });
                                                }),
                                        ),
                                )
                                .child(div().text_xs().child(dest_display)),
                        )
                        .when_some(ui.error.clone(), |el, err| {
                            el.child(div().text_sm().text_color(theme.danger).child(err))
                        }),
                )
                .footer(
                    DialogFooter::new()
                        .child(Button::new("txt-cancel").label("Batal").on_click(
                            move |_, window, cx| {
                                window.close_dialog(cx);
                                let _ = cancel.update(cx, |this, cx| {
                                    this.close_quick_export_dialogs(cx);
                                });
                            },
                        ))
                        .child(
                            Button::new("txt-start")
                                .primary()
                                .label("Ekspor ke TXT")
                                .disabled(ui.picking || active_count == 0)
                                .on_click(move |_, window, cx| {
                                    let _ = start.update(cx, |this, cx| {
                                        this.submit_txt_export(window, cx);
                                    });
                                }),
                        ),
                )
                .on_close(move |_, _, cx| {
                    let _ = closed.update(cx, |this, cx| {
                        this.close_quick_export_dialogs(cx);
                    });
                })
        });
        cx.notify();
    }

    fn pick_txt_destination(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(ui) = &mut self.quick_export.txt_dialog else {
            return;
        };
        if ui.picking {
            return;
        }
        let suggested = ui.destination();
        let dir = suggested
            .parent()
            .filter(|p| p.is_dir())
            .unwrap_or_else(|| Path::new("."))
            .to_path_buf();
        let suggested_name = suggested
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("dokumen.txt");
        ui.picking = true;
        let picker = cx.prompt_for_new_path(&dir, Some(suggested_name));
        let window_handle = window.window_handle();
        cx.notify();
        cx.spawn(async move |this, cx| {
            let picked = picker.await;
            let _ = this.update(cx, |this, cx| {
                if let Some(ui) = &mut this.quick_export.txt_dialog {
                    ui.picking = false;
                    if let Ok(Ok(Some(mut path))) = picked {
                        if !path
                            .extension()
                            .is_some_and(|e| e.eq_ignore_ascii_case("txt"))
                        {
                            path.set_extension("txt");
                        }
                        ui.custom_destination = Some(path);
                        ui.error = None;
                    }
                }
                let _ = cx.update_window(window_handle, |_, w, _| w.refresh());
                cx.notify();
            });
        })
        .detach();
    }

    fn submit_txt_export(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(ui) = self.quick_export.txt_dialog.clone() else {
            return;
        };
        let plan = match ui.active_plan() {
            Ok(p) => p.clone(),
            Err(error) => {
                if let Some(d) = &mut self.quick_export.txt_dialog {
                    d.error = Some(error.to_string());
                }
                window.refresh();
                cx.notify();
                return;
            }
        };
        let destination = ui.destination();
        if std::fs::symlink_metadata(&destination).is_ok() {
            if let Some(d) = &mut self.quick_export.txt_dialog {
                d.error = Some(
                    "File tujuan sudah ada. Pilih nama file lain agar file lama tidak tertimpa."
                        .into(),
                );
            }
            window.refresh();
            cx.notify();
            return;
        }
        window.close_dialog(cx);
        self.close_quick_export_dialogs(cx);

        let options = TextExportOptions {
            include_page_headers: ui.include_page_headers,
        };
        self.start_txt_export_job(plan, destination, options, cx);
    }

    fn start_txt_export_job(
        &mut self,
        plan: ExportPlan,
        destination: PathBuf,
        options: TextExportOptions,
        cx: &mut Context<Self>,
    ) {
        if self.resources.export_owner.load(Ordering::SeqCst) != 0 {
            return;
        }
        let total = plan.pages().len();
        let control = ExportControl::with_budget(std::time::Duration::from_secs(600));
        let done = Arc::new(AtomicUsize::new(0));
        self.quick_export.job_seq = self.quick_export.job_seq.wrapping_add(1);
        let job_id = self.quick_export.job_seq;
        self.resources
            .export_owner
            .store(SHELL_EXPORT_OWNER, Ordering::SeqCst);
        self.quick_export.job = Some(QuickExportJob {
            kind: QuickExportKind::Txt,
            total,
            done: done.clone(),
            control: control.clone(),
            job_id,
        });
        let renderer = self.renderer.clone();
        cx.notify();

        let progress_counter = done.clone();
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_executor()
                .spawn(async move {
                    let extracted =
                        extract_plan_text(&plan, &renderer, options, &control, |count, _total| {
                            progress_counter.store(count, Ordering::SeqCst);
                        })?;
                    let (final_dest, bytes) =
                        write_text_file_no_clobber(&destination, &extracted.content, &control)?;
                    Ok::<TxtExportReceipt, ExportError>(TxtExportReceipt {
                        destination: final_dest,
                        pages: extracted.pages,
                        pages_with_text: extracted.pages_with_text,
                        chars: extracted.chars,
                        bytes,
                    })
                })
                .await;
            let _ = this.update(cx, |this, cx| {
                this.finish_txt_export_job(job_id, result, cx);
            });
        })
        .detach();
    }

    fn finish_txt_export_job(
        &mut self,
        job_id: u64,
        result: Result<TxtExportReceipt, ExportError>,
        cx: &mut Context<Self>,
    ) {
        if self.quick_export.job.as_ref().map(|j| j.job_id) != Some(job_id) {
            return;
        }
        self.quick_export.job = None;
        self.resources.export_owner.store(0, Ordering::SeqCst);
        match result {
            Ok(receipt) => {
                if receipt.pages_with_text == 0 {
                    self.notice = Some(format!(
                        "TXT tersimpan di {}, namun tidak ada teks ditemukan ({} halaman). Untuk PDF hasil scan, jalankan OCR terlebih dahulu.",
                        receipt.destination.display(),
                        receipt.pages,
                    ));
                } else {
                    self.notice = Some(format!(
                        "Teks berhasil diekspor: {} dari {} halaman ({} karakter, {}) → {}",
                        receipt.pages_with_text,
                        receipt.pages,
                        receipt.chars,
                        format_quick_bytes(receipt.bytes),
                        receipt.destination.display(),
                    ));
                }
            }
            Err(ExportError::Cancelled) => {
                self.notice = Some("Ekspor teks (TXT) dibatalkan.".into());
            }
            Err(error) => {
                self.notice = Some(format!("Gagal mengekspor teks: {error}"));
            }
        }
        cx.notify();
    }

    pub(super) fn show_extract_images_dialog(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.can_start_quick_export(cx) {
            return;
        }
        let Some(tab) = self.active_tab() else {
            return;
        };
        let source_path = tab.path.clone();
        let default_folder = source_path
            .file_stem()
            .and_then(|s| s.to_str())
            .filter(|s| !s.trim().is_empty())
            .map(|stem| {
                let candidate = format!("{stem}-gambar");
                if validate_folder(&candidate).is_ok() {
                    candidate
                } else {
                    "hasil-gambar".into()
                }
            })
            .unwrap_or_else(|| "hasil-gambar".into());
        let folder = cx.new(|cx| {
            let mut input = InputState::new(window, cx);
            input.set_value(&default_folder, window, cx);
            input
        });
        cx.subscribe_in(
            &folder,
            window,
            |this, _, event: &InputEvent, window, cx| {
                if matches!(event, InputEvent::Change) {
                    if let Some(d) = &mut this.quick_export.extract_images_dialog {
                        d.error = None;
                    }
                    window.refresh();
                    cx.notify();
                }
            },
        )
        .detach();
        self.quick_export.extract_images_dialog = Some(ExtractImagesDialog {
            source_path,
            skip_small: true,
            skip_soft_masks: true,
            folder,
            error: None,
        });
        self.resources
            .modal_owner
            .store(SHELL_EXPORT_OWNER, Ordering::SeqCst);

        let weak = cx.weak_entity();
        window.open_alert_dialog(cx, move |dialog, _, cx| {
            let Some(ui) = weak
                .read_with(cx, |this, _| {
                    this.quick_export.extract_images_dialog.clone()
                })
                .ok()
                .flatten()
            else {
                return dialog;
            };
            let theme = cx.theme();
            let toggle_small = weak.clone();
            let toggle_mask = weak.clone();
            let cancel = weak.clone();
            let start = weak.clone();
            let closed = weak.clone();

            dialog
                .title(
                    div()
                        .id("extract-images-title")
                        .role(gpui::Role::Group)
                        .aria_label("Ekstrak Gambar Asli")
                        .child("Ekstrak Gambar Asli"),
                )
                .description(
                    "Mengekstrak gambar embedded asli (JPEG langsung tanpa re-kompresi & gambar lossless ke PNG) dari file sumber PDF ke folder baru. File PDF sumber tetap utuh.",
                )
                .centered()
                .width(px(580.))
                .close_button(true)
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_3()
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .gap_2()
                                .child(
                                    Button::new("extract-img-skip-small")
                                        .small()
                                        .label(if ui.skip_small {
                                            "☑ Abaikan gambar sangat kecil (< 32 px)"
                                        } else {
                                            "☐ Abaikan gambar sangat kecil (< 32 px)"
                                        })
                                        .selected(ui.skip_small)
                                        .on_click(move |_, window, cx| {
                                            let _ = toggle_small.update(cx, |this, cx| {
                                                if let Some(d) =
                                                    &mut this.quick_export.extract_images_dialog
                                                {
                                                    d.skip_small = !d.skip_small;
                                                }
                                                window.refresh();
                                                cx.notify();
                                            });
                                        }),
                                )
                                .child(
                                    Button::new("extract-img-skip-masks")
                                        .small()
                                        .label(if ui.skip_soft_masks {
                                            "☑ Gabungkan & lewati soft-mask terpisah (Disarankan)"
                                        } else {
                                            "☐ Gabungkan & lewati soft-mask terpisah (Disarankan)"
                                        })
                                        .selected(ui.skip_soft_masks)
                                        .on_click(move |_, window, cx| {
                                            let _ = toggle_mask.update(cx, |this, cx| {
                                                if let Some(d) =
                                                    &mut this.quick_export.extract_images_dialog
                                                {
                                                    d.skip_soft_masks = !d.skip_soft_masks;
                                                }
                                                window.refresh();
                                                cx.notify();
                                            });
                                        }),
                                ),
                        )
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .gap_1()
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(theme.muted_foreground)
                                        .child("Nama folder baru (pilih folder induk setelah ini):"),
                                )
                                .child(Input::new(&ui.folder).aria_label("Nama folder gambar asli")),
                        )
                        .when_some(ui.error.clone(), |el, err| {
                            el.child(div().text_sm().text_color(theme.danger).child(err))
                        }),
                )
                .footer(
                    DialogFooter::new()
                        .child(Button::new("extract-img-cancel").label("Batal").on_click(
                            move |_, window, cx| {
                                window.close_dialog(cx);
                                let _ = cancel.update(cx, |this, cx| {
                                    this.close_quick_export_dialogs(cx);
                                });
                            },
                        ))
                        .child(
                            Button::new("extract-img-start")
                                .primary()
                                .label("Pilih folder induk")
                                .on_click(move |_, window, cx| {
                                    let _ = start.update(cx, |this, cx| {
                                        this.submit_extract_images(window, cx);
                                    });
                                }),
                        ),
                )
                .on_close(move |_, _, cx| {
                    let _ = closed.update(cx, |this, cx| {
                        this.close_quick_export_dialogs(cx);
                    });
                })
        });
        cx.notify();
    }

    fn submit_extract_images(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(ui) = self.quick_export.extract_images_dialog.clone() else {
            return;
        };
        let folder_name = ui.folder.read(cx).value().to_string();
        if let Err(error) = validate_folder(&folder_name) {
            if let Some(d) = &mut self.quick_export.extract_images_dialog {
                d.error = Some(error.to_string());
            }
            window.refresh();
            cx.notify();
            return;
        }
        window.close_dialog(cx);
        self.close_quick_export_dialogs(cx);

        let options = ExtractImagesOptions {
            min_dimension: if ui.skip_small { 32 } else { 1 },
            skip_soft_masks: ui.skip_soft_masks,
        };
        self.start_extract_images_job(ui.source_path, folder_name, options, cx);
    }

    fn start_extract_images_job(
        &mut self,
        source_path: PathBuf,
        folder_name: String,
        options: ExtractImagesOptions,
        cx: &mut Context<Self>,
    ) {
        if self.resources.export_owner.load(Ordering::SeqCst) != 0 {
            return;
        }
        let picker = cx.prompt_for_paths(gpui::PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: Some("Pilih folder induk hasil ekstraksi gambar".into()),
        });
        let control = ExportControl::with_budget(std::time::Duration::from_secs(600));
        let done = Arc::new(AtomicUsize::new(0));
        self.quick_export.job_seq = self.quick_export.job_seq.wrapping_add(1);
        let job_id = self.quick_export.job_seq;
        self.resources
            .export_owner
            .store(SHELL_EXPORT_OWNER, Ordering::SeqCst);
        self.quick_export.job = Some(QuickExportJob {
            kind: QuickExportKind::ExtractImages,
            total: 0,
            done: done.clone(),
            control: control.clone(),
            job_id,
        });
        let extractor = QpdfImageExtractor::new(&self.sidecars);
        cx.notify();

        let progress_counter = done.clone();
        cx.spawn(async move |this, cx| {
            let destination = match picker.await {
                Ok(Ok(Some(paths))) if paths.len() == 1 => paths[0].join(folder_name),
                other => {
                    let error = if matches!(other, Ok(Ok(None))) {
                        ExtractImagesError::Export(ExportError::Cancelled)
                    } else {
                        ExtractImagesError::Batch(SplitError::Output)
                    };
                    let _ = this.update(cx, |this, cx| {
                        this.finish_extract_images_job(job_id, Err(error), cx);
                    });
                    return;
                }
            };
            let result = cx
                .background_executor()
                .spawn(async move {
                    extractor.extract_images(
                        &source_path,
                        &destination,
                        options,
                        &control,
                        |count, _total| {
                            progress_counter.store(count, Ordering::SeqCst);
                        },
                    )
                })
                .await;
            let _ = this.update(cx, |this, cx| {
                this.finish_extract_images_job(job_id, result, cx);
            });
        })
        .detach();
    }

    fn finish_extract_images_job(
        &mut self,
        job_id: u64,
        result: Result<ExtractImagesReport, ExtractImagesError>,
        cx: &mut Context<Self>,
    ) {
        if self.quick_export.job.as_ref().map(|j| j.job_id) != Some(job_id) {
            return;
        }
        self.quick_export.job = None;
        self.resources.export_owner.store(0, Ordering::SeqCst);
        match result {
            Ok(report) => {
                let mut detail_parts = Vec::new();
                if report.jpeg_count > 0 {
                    detail_parts.push(format!("{} JPEG", report.jpeg_count));
                }
                if report.png_count > 0 {
                    detail_parts.push(format!("{} PNG", report.png_count));
                }
                let detail = detail_parts.join(", ");
                let skipped_note = if report.skipped > 0 {
                    format!(
                        " ({} objek mask/kecil/tidak didukung dilewati)",
                        report.skipped
                    )
                } else {
                    String::new()
                };
                self.notice = Some(format!(
                    "{} gambar asli diekstrak ({detail} · {}){skipped_note} → {}",
                    report.extracted,
                    format_quick_bytes(report.total_bytes),
                    report.destination.display(),
                ));
            }
            Err(
                ExtractImagesError::Export(ExportError::Cancelled)
                | ExtractImagesError::Batch(SplitError::Export(ExportError::Cancelled)),
            ) => {
                self.notice = Some(
                    "Ekstraksi gambar asli dibatalkan. Folder hasil tidak diterbitkan.".into(),
                );
            }
            Err(error) => {
                self.notice = Some(format!("Gagal mengekstrak gambar asli: {error}"));
            }
        }
        cx.notify();
    }

    pub(super) fn cancel_quick_export_job(&mut self, cx: &mut Context<Self>) {
        if let Some(job) = &self.quick_export.job {
            job.control.cancel();
            cx.notify();
        }
    }

    fn close_quick_export_dialogs(&mut self, cx: &mut Context<Self>) {
        self.quick_export.txt_dialog = None;
        self.quick_export.extract_images_dialog = None;
        if self.resources.modal_owner.load(Ordering::SeqCst) == SHELL_EXPORT_OWNER {
            self.resources.modal_owner.store(0, Ordering::SeqCst);
        }
        cx.notify();
    }
}

fn format_quick_bytes(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{bytes} B")
    } else if bytes < 1024 * 1024 {
        format!("{:.1} KiB", bytes as f64 / 1024.0)
    } else {
        format!("{:.2} MiB", bytes as f64 / (1024.0 * 1024.0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn suggested_txt_destination_avoids_existing_files() -> Result<(), Box<dyn std::error::Error>> {
        let dir = tempfile::tempdir()?;
        let src = dir.path().join("laporan.pdf");
        std::fs::write(&src, b"pdf")?;
        let first = suggested_txt_destination(&src);
        assert_eq!(first, dir.path().join("laporan.txt"));
        std::fs::write(&first, b"existing")?;
        let second = suggested_txt_destination(&src);
        assert_eq!(second, dir.path().join("laporan (2).txt"));
        Ok(())
    }
}
