//! Window-level Images (PNG/JPEG) to PDF dialog and bounded background job.
use crate::shell::{MAX_TABS, WindowShell};
use crate::*;
use gpui::component::{
    ActiveTheme, Disableable, Selectable, StyledExt, WindowExt,
    button::{Button, ButtonVariants},
    dialog::DialogFooter,
};
use pdf_application::{
    export::{ExportControl, ExportPhase},
    images_to_pdf::{
        ImageMargin, ImageOrientation, ImagePageSize, ImagesToPdfError, ImagesToPdfOptions,
        ImagesToPdfPlan, ImagesToPdfReceipt, MAX_IMAGES, run_images_to_pdf,
    },
};
use pdf_platform::images_to_pdf::LocalImagePdfWriter;
use std::sync::atomic::{AtomicUsize, Ordering};

pub(super) const SHELL_EXPORT_OWNER: u64 = u64::MAX;

#[derive(Clone)]
pub(super) struct ImagesPdfDialog {
    pub images: Vec<PathBuf>,
    pub options: ImagesToPdfOptions,
    pub open_after: bool,
    pub picking: bool,
    pub error: Option<String>,
}

pub(super) struct ImagesPdfJob {
    pub total: usize,
    pub done: Arc<AtomicUsize>,
    pub control: ExportControl,
    pub open_after: bool,
    pub job_id: u64,
}

#[derive(Default)]
pub(super) struct ImagesPdfState {
    pub dialog: Option<ImagesPdfDialog>,
    pub job: Option<ImagesPdfJob>,
    pub job_seq: u64,
    pub completed: u64,
    pub success: bool,
    pub pages: usize,
}

impl ImagesPdfState {
    pub(super) fn phase_label(&self) -> String {
        let Some(job) = &self.job else {
            return String::new();
        };
        let phase = job.control.phase();
        let done = job.done.load(Ordering::SeqCst);
        match phase {
            ExportPhase::Choosing => "Memilih file PDF tujuan…".into(),
            ExportPhase::Snapshot => "Menyiapkan folder tujuan…".into(),
            ExportPhase::Writing => {
                format!("Memproses gambar ({done}/{})…", job.total)
            }
            ExportPhase::Validation => "Memvalidasi PDF hasil…".into(),
            ExportPhase::Commit => "Menyimpan file PDF…".into(),
            _ => phase.label().into(),
        }
    }
}

fn is_supported_image_extension(path: &std::path::Path) -> bool {
    path.extension().is_some_and(|e| {
        e.eq_ignore_ascii_case("png")
            || e.eq_ignore_ascii_case("jpg")
            || e.eq_ignore_ascii_case("jpeg")
    })
}

fn suggested_pdf_name(images: &[PathBuf]) -> String {
    if images.len() == 1
        && let Some(stem) = images[0].file_stem().and_then(|s| s.to_str())
        && !stem.trim().is_empty()
    {
        return format!("{stem}.pdf");
    }
    if images.len() > 1
        && let Some(folder) = images[0]
            .parent()
            .and_then(|p| p.file_name())
            .and_then(|n| n.to_str())
        && !folder.trim().is_empty()
    {
        return format!("{folder}.pdf");
    }
    "gambar-ke-pdf.pdf".into()
}

fn quick_base_stem(images: &[PathBuf]) -> String {
    let raw = if images.len() == 1 {
        images[0]
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("gambar-ke-pdf")
    } else {
        images
            .first()
            .and_then(|p| p.parent())
            .and_then(|p| p.file_name())
            .and_then(|n| n.to_str())
            .unwrap_or("gambar-ke-pdf")
    };
    let cleaned: String = raw
        .trim()
        .chars()
        .map(|c| {
            if matches!(c, '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*') || c.is_control() {
                '-'
            } else {
                c
            }
        })
        .collect();
    let cleaned = cleaned.trim_matches(|c| c == '.' || c == ' ');
    if cleaned.is_empty() {
        "gambar-ke-pdf".into()
    } else {
        cleaned.to_owned()
    }
}

pub(super) fn compute_quick_destination(images: &[PathBuf]) -> Option<PathBuf> {
    let parent = images.first()?.parent()?;
    if parent.as_os_str().is_empty() {
        return None;
    }
    let stem = quick_base_stem(images);
    let candidate = parent.join(format!("{stem}.pdf"));
    if !candidate.exists() {
        return Some(candidate);
    }
    for n in 2..=999 {
        let next = parent.join(format!("{stem}-{n}.pdf"));
        if !next.exists() {
            return Some(next);
        }
    }
    None
}

fn filter_incoming_images(paths: Vec<PathBuf>) -> (Vec<PathBuf>, Option<String>) {
    let mut accepted = Vec::new();
    let mut rejected = 0usize;
    let mut truncated = false;
    for path in paths {
        if !is_supported_image_extension(&path) {
            rejected += 1;
            continue;
        }
        if accepted.len() >= MAX_IMAGES {
            truncated = true;
            break;
        }
        accepted.push(path);
    }
    let note = if truncated {
        Some(format!(
            "Maksimal {MAX_IMAGES} gambar per PDF. Sebagian file dilewati."
        ))
    } else if rejected > 0 {
        Some(format!(
            "{rejected} file dilewati karena bukan PNG atau JPEG."
        ))
    } else {
        None
    };
    (accepted, note)
}

fn summary_text(ui: &ImagesPdfDialog) -> String {
    if ui.images.is_empty() {
        return "Belum ada gambar dipilih".into();
    }
    if ui.options.page_size == ImagePageSize::FitImage {
        format!(
            "{} gambar · {} · Margin {}",
            ui.images.len(),
            ui.options.page_size.label(),
            ui.options.margin.label()
        )
    } else {
        format!(
            "{} gambar · {} · {} · Margin {}",
            ui.images.len(),
            ui.options.page_size.label(),
            ui.options.orientation.label(),
            ui.options.margin.label()
        )
    }
}

impl WindowShell {
    pub(super) fn can_start_images_to_pdf(&self, cx: &App) -> bool {
        !self.opening
            && self.close.is_none()
            && !self.modal(cx)
            && self.resources.export_owner.load(Ordering::SeqCst) == 0
    }

    pub(super) fn show_images_to_pdf(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.show_images_to_pdf_with_paths(Vec::new(), window, cx);
    }

    pub(super) fn show_images_to_pdf_with_paths(
        &mut self,
        initial_images: Vec<PathBuf>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.can_start_images_to_pdf(cx) {
            if !initial_images.is_empty() {
                self.notice = Some(
                    "Selesaikan dialog atau proses ekspor yang sedang berjalan terlebih dahulu."
                        .into(),
                );
                cx.notify();
            }
            return;
        }
        let (images, error) = filter_incoming_images(initial_images);
        self.tab_picker_open = false;
        self.images_pdf.dialog = Some(ImagesPdfDialog {
            images,
            options: ImagesToPdfOptions::default(),
            open_after: true,
            picking: false,
            error,
        });
        self.resources
            .modal_owner
            .store(SHELL_EXPORT_OWNER, Ordering::SeqCst);
        let weak = cx.weak_entity();
        window.open_alert_dialog(cx, move |dialog, _, cx| {
            let Some(ui) = weak
                .read_with(cx, |this, _| this.images_pdf.dialog.clone())
                .ok()
                .flatten()
            else {
                return dialog;
            };
            let colors = cx.theme().colors;
            let count = ui.images.len();
            let preview = summary_text(&ui);
            let fit_image = ui.options.page_size == ImagePageSize::FitImage;

            let add = weak.clone();
            let clear = weak.clone();
            let size_a4 = weak.clone();
            let size_letter = weak.clone();
            let size_fit = weak.clone();
            let orient_auto = weak.clone();
            let orient_portrait = weak.clone();
            let orient_landscape = weak.clone();
            let margin_none = weak.clone();
            let margin_small = weak.clone();
            let margin_normal = weak.clone();
            let toggle_open = weak.clone();
            let cancel = weak.clone();
            let start = weak.clone();
            let closed = weak.clone();

            let mut list_box = gpui::div()
                .id("img2pdf-list")
                .w_full()
                .max_h(px(180.))
                .overflow_y_scroll()
                .rounded_md()
                .border_1()
                .border_color(colors.border)
                .bg(colors.background)
                .p_2()
                .flex()
                .flex_col()
                .gap_1();

            if ui.images.is_empty() {
                list_box = list_box.child(
                    gpui::div()
                        .text_sm()
                        .text_color(colors.muted_foreground)
                        .py_3()
                        .child(
                            "Belum ada gambar dipilih. Klik Tambah gambar… untuk memilih PNG atau JPEG.",
                        ),
                );
            } else {
                for (index, path) in ui.images.iter().enumerate() {
                    let filename = path
                        .file_name()
                        .map(|n| n.to_string_lossy().into_owned())
                        .unwrap_or_else(|| path.display().to_string());
                    let row_label = format!("{}. {filename}", index + 1);
                    let up = weak.clone();
                    let down = weak.clone();
                    let remove = weak.clone();
                    list_box = list_box.child(
                        gpui::div()
                            .id(("img2pdf-row", index))
                            .role(gpui::Role::Group)
                            .aria_label(row_label.clone())
                            .flex()
                            .items_center()
                            .gap_2()
                            .px_2()
                            .py_1()
                            .rounded_sm()
                            .bg(colors.secondary)
                            .child(
                                gpui::div()
                                    .flex_1()
                                    .min_w_0()
                                    .text_sm()
                                    .truncate()
                                    .child(row_label),
                            )
                            .child(
                                Button::new(("img2pdf-up", index))
                                    .ghost()
                                    .label("↑")
                                    .accessibility_label(format!("Naikkan gambar {}", index + 1))
                                    .disabled(index == 0 || ui.picking)
                                    .on_click(move |_, window, cx| {
                                        let _ = up.update(cx, |this, cx| {
                                            this.move_image_pdf_item(index, -1, window, cx)
                                        });
                                    }),
                            )
                            .child(
                                Button::new(("img2pdf-down", index))
                                    .ghost()
                                    .label("↓")
                                    .accessibility_label(format!("Turunkan gambar {}", index + 1))
                                    .disabled(index + 1 == count || ui.picking)
                                    .on_click(move |_, window, cx| {
                                        let _ = down.update(cx, |this, cx| {
                                            this.move_image_pdf_item(index, 1, window, cx)
                                        });
                                    }),
                            )
                            .child(
                                Button::new(("img2pdf-remove", index))
                                    .ghost()
                                    .label("✕")
                                    .accessibility_label(format!("Hapus gambar {}", index + 1))
                                    .disabled(ui.picking)
                                    .on_click(move |_, window, cx| {
                                        let _ = remove.update(cx, |this, cx| {
                                            this.remove_image_pdf_item(index, window, cx)
                                        });
                                    }),
                            ),
                    );
                }
            }

            dialog
                .title(
                    gpui::div()
                        .id("img2pdf-title")
                        .role(gpui::Role::Group)
                        .aria_label("Gambar ke PDF")
                        .child("Gambar ke PDF"),
                )
                .description(
                    "Gabungkan satu atau banyak gambar PNG/JPEG menjadi satu file PDF baru. File gambar asli tetap utuh.",
                )
                .centered()
                .width(px(620.))
                .close_button(true)
                .child(
                    gpui::div()
                        .flex()
                        .flex_col()
                        .gap_3()
                        .child(
                            gpui::div()
                                .flex()
                                .items_center()
                                .gap_2()
                                .child(
                                    Button::new("img2pdf-add")
                                        .label("Tambah gambar…")
                                        .disabled(ui.picking || count >= MAX_IMAGES)
                                        .on_click(move |_, _, cx| {
                                            let _ = add.update(cx, |this, cx| {
                                                this.pick_images_for_pdf(cx)
                                            });
                                        }),
                                )
                                .child(
                                    Button::new("img2pdf-clear")
                                        .ghost()
                                        .label("Kosongkan")
                                        .disabled(ui.images.is_empty() || ui.picking)
                                        .on_click(move |_, window, cx| {
                                            let _ = clear.update(cx, |this, cx| {
                                                this.clear_images_for_pdf(window, cx)
                                            });
                                        }),
                                ),
                        )
                        .child(list_box)
                        .child(
                            gpui::div()
                                .flex()
                                .flex_col()
                                .gap_1()
                                .child(gpui::div().text_xs().font_semibold().child("Ukuran halaman"))
                                .child(
                                    gpui::div()
                                        .flex()
                                        .gap_2()
                                        .child(
                                            Button::new("img2pdf-size-a4")
                                                .label("A4")
                                                .selected(ui.options.page_size == ImagePageSize::A4)
                                                .on_click(move |_, window, cx| {
                                                    let _ = size_a4.update(cx, |this, cx| {
                                                        this.update_images_pdf_options(
                                                            Some(ImagePageSize::A4),
                                                            None,
                                                            None,
                                                            None,
                                                            window,
                                                            cx,
                                                        )
                                                    });
                                                }),
                                        )
                                        .child(
                                            Button::new("img2pdf-size-letter")
                                                .label("Letter")
                                                .selected(
                                                    ui.options.page_size == ImagePageSize::Letter,
                                                )
                                                .on_click(move |_, window, cx| {
                                                    let _ = size_letter.update(cx, |this, cx| {
                                                        this.update_images_pdf_options(
                                                            Some(ImagePageSize::Letter),
                                                            None,
                                                            None,
                                                            None,
                                                            window,
                                                            cx,
                                                        )
                                                    });
                                                }),
                                        )
                                        .child(
                                            Button::new("img2pdf-size-fit")
                                                .label("Sesuai gambar")
                                                .selected(fit_image)
                                                .on_click(move |_, window, cx| {
                                                    let _ = size_fit.update(cx, |this, cx| {
                                                        this.update_images_pdf_options(
                                                            Some(ImagePageSize::FitImage),
                                                            None,
                                                            None,
                                                            None,
                                                            window,
                                                            cx,
                                                        )
                                                    });
                                                }),
                                        ),
                                ),
                        )
                        .child(
                            gpui::div()
                                .flex()
                                .flex_col()
                                .gap_1()
                                .child(gpui::div().text_xs().font_semibold().child("Orientasi"))
                                .child(
                                    gpui::div()
                                        .flex()
                                        .gap_2()
                                        .child(
                                            Button::new("img2pdf-orient-auto")
                                                .label("Otomatis")
                                                .selected(
                                                    !fit_image
                                                        && ui.options.orientation
                                                            == ImageOrientation::Auto,
                                                )
                                                .disabled(fit_image)
                                                .on_click(move |_, window, cx| {
                                                    let _ = orient_auto.update(cx, |this, cx| {
                                                        this.update_images_pdf_options(
                                                            None,
                                                            Some(ImageOrientation::Auto),
                                                            None,
                                                            None,
                                                            window,
                                                            cx,
                                                        )
                                                    });
                                                }),
                                        )
                                        .child(
                                            Button::new("img2pdf-orient-portrait")
                                                .label("Potret")
                                                .selected(
                                                    !fit_image
                                                        && ui.options.orientation
                                                            == ImageOrientation::Portrait,
                                                )
                                                .disabled(fit_image)
                                                .on_click(move |_, window, cx| {
                                                    let _ = orient_portrait.update(cx, |this, cx| {
                                                        this.update_images_pdf_options(
                                                            None,
                                                            Some(ImageOrientation::Portrait),
                                                            None,
                                                            None,
                                                            window,
                                                            cx,
                                                        )
                                                    });
                                                }),
                                        )
                                        .child(
                                            Button::new("img2pdf-orient-landscape")
                                                .label("Lanskap")
                                                .selected(
                                                    !fit_image
                                                        && ui.options.orientation
                                                            == ImageOrientation::Landscape,
                                                )
                                                .disabled(fit_image)
                                                .on_click(move |_, window, cx| {
                                                    let _ = orient_landscape.update(cx, |this, cx| {
                                                        this.update_images_pdf_options(
                                                            None,
                                                            Some(ImageOrientation::Landscape),
                                                            None,
                                                            None,
                                                            window,
                                                            cx,
                                                        )
                                                    });
                                                }),
                                        ),
                                ),
                        )
                        .child(
                            gpui::div()
                                .flex()
                                .flex_col()
                                .gap_1()
                                .child(gpui::div().text_xs().font_semibold().child("Margin"))
                                .child(
                                    gpui::div()
                                        .flex()
                                        .gap_2()
                                        .child(
                                            Button::new("img2pdf-margin-none")
                                                .label("Tanpa margin")
                                                .selected(ui.options.margin == ImageMargin::None)
                                                .on_click(move |_, window, cx| {
                                                    let _ = margin_none.update(cx, |this, cx| {
                                                        this.update_images_pdf_options(
                                                            None,
                                                            None,
                                                            Some(ImageMargin::None),
                                                            None,
                                                            window,
                                                            cx,
                                                        )
                                                    });
                                                }),
                                        )
                                        .child(
                                            Button::new("img2pdf-margin-small")
                                                .label("Kecil")
                                                .selected(ui.options.margin == ImageMargin::Small)
                                                .on_click(move |_, window, cx| {
                                                    let _ = margin_small.update(cx, |this, cx| {
                                                        this.update_images_pdf_options(
                                                            None,
                                                            None,
                                                            Some(ImageMargin::Small),
                                                            None,
                                                            window,
                                                            cx,
                                                        )
                                                    });
                                                }),
                                        )
                                        .child(
                                            Button::new("img2pdf-margin-normal")
                                                .label("Normal")
                                                .selected(ui.options.margin == ImageMargin::Normal)
                                                .on_click(move |_, window, cx| {
                                                    let _ = margin_normal.update(cx, |this, cx| {
                                                        this.update_images_pdf_options(
                                                            None,
                                                            None,
                                                            Some(ImageMargin::Normal),
                                                            None,
                                                            window,
                                                            cx,
                                                        )
                                                    });
                                                }),
                                        ),
                                ),
                        )
                        .child(
                            gpui::div().flex().items_center().gap_2().child(
                                Button::new("img2pdf-open-after")
                                    .label(if ui.open_after {
                                        "✓ Buka PDF setelah selesai"
                                    } else {
                                        "Buka PDF setelah selesai"
                                    })
                                    .accessibility_label("Buka PDF setelah selesai")
                                    .selected(ui.open_after)
                                    .on_click(move |_, window, cx| {
                                        let _ = toggle_open.update(cx, |this, cx| {
                                            this.update_images_pdf_options(
                                                None,
                                                None,
                                                None,
                                                Some(!ui.open_after),
                                                window,
                                                cx,
                                            )
                                        });
                                    }),
                            ),
                        )
                        .child(
                            gpui::div()
                                .id("img2pdf-preview")
                                .role(gpui::Role::Group)
                                .aria_label(preview.clone())
                                .text_sm()
                                .text_color(colors.muted_foreground)
                                .child(preview),
                        )
                        .when_some(ui.error.clone(), |el, error| {
                            el.child(
                                gpui::div()
                                    .id("img2pdf-error")
                                    .role(gpui::Role::Group)
                                    .aria_label(error.clone())
                                    .text_sm()
                                    .text_color(colors.danger)
                                    .child(error),
                            )
                        }),
                )
                .footer(
                    DialogFooter::new()
                        .child(Button::new("img2pdf-cancel").label("Batal").on_click(
                            move |_, window, cx| {
                                window.close_dialog(cx);
                                let _ = cancel.update(cx, |this, cx| this.close_images_to_pdf(cx));
                            },
                        ))
                        .child(
                            Button::new("img2pdf-start")
                                .primary()
                                .label("Simpan PDF…")
                                .disabled(ui.images.is_empty() || ui.picking)
                                .on_click(move |_, window, cx| {
                                    let _ = start.update(cx, |this, cx| {
                                        this.submit_images_to_pdf(window, cx)
                                    });
                                }),
                        ),
                )
                .on_close(move |_, _, cx| {
                    let _ = closed.update(cx, |this, cx| this.close_images_to_pdf(cx));
                })
        });
        self.report(cx);
        cx.notify();
    }

    fn pick_images_for_pdf(&mut self, cx: &mut Context<Self>) {
        let Some(ui) = &mut self.images_pdf.dialog else {
            return;
        };
        if ui.picking {
            return;
        }
        ui.picking = true;
        ui.error = None;
        let picker = cx.prompt_for_paths(gpui::PathPromptOptions {
            files: true,
            directories: false,
            multiple: true,
            prompt: Some("Pilih gambar PNG atau JPEG".into()),
        });
        let handle = self.handle;
        cx.spawn(async move |this, cx| {
            let picked = picker.await;
            let _ = this.update(cx, |this, cx| {
                if let Some(ui) = &mut this.images_pdf.dialog {
                    ui.picking = false;
                    match picked {
                        Ok(Ok(Some(paths))) => {
                            let mut rejected_format = 0usize;
                            let mut truncated = false;
                            for path in paths {
                                if !is_supported_image_extension(&path) {
                                    rejected_format += 1;
                                    continue;
                                }
                                if ui.images.len() >= MAX_IMAGES {
                                    truncated = true;
                                    break;
                                }
                                ui.images.push(path);
                            }
                            if truncated {
                                ui.error = Some(format!(
                                    "Maksimal {MAX_IMAGES} gambar per PDF. Sebagian file dilewati."
                                ));
                            } else if rejected_format > 0 {
                                ui.error = Some(format!(
                                    "{rejected_format} file dilewati karena bukan PNG atau JPEG."
                                ));
                            }
                        }
                        Ok(Ok(None)) => {}
                        _ => {
                            ui.error = Some("Dialog pilih gambar gagal dibuka.".into());
                        }
                    }
                }
                this.report(cx);
                cx.notify();
            });
            let _ = handle.update(cx, |_, window, _| window.refresh());
        })
        .detach();
        cx.notify();
    }

    fn move_image_pdf_item(
        &mut self,
        index: usize,
        delta: isize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(ui) = &mut self.images_pdf.dialog {
            let target = index as isize + delta;
            if target >= 0 && (target as usize) < ui.images.len() {
                ui.images.swap(index, target as usize);
                ui.error = None;
            }
        }
        window.refresh();
        cx.notify();
    }

    fn remove_image_pdf_item(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(ui) = &mut self.images_pdf.dialog
            && index < ui.images.len()
        {
            ui.images.remove(index);
            ui.error = None;
        }
        window.refresh();
        cx.notify();
    }

    fn clear_images_for_pdf(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(ui) = &mut self.images_pdf.dialog {
            ui.images.clear();
            ui.error = None;
        }
        window.refresh();
        cx.notify();
    }

    fn update_images_pdf_options(
        &mut self,
        page_size: Option<ImagePageSize>,
        orientation: Option<ImageOrientation>,
        margin: Option<ImageMargin>,
        open_after: Option<bool>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(ui) = &mut self.images_pdf.dialog {
            if let Some(page_size) = page_size {
                ui.options.page_size = page_size;
            }
            if let Some(orientation) = orientation {
                ui.options.orientation = orientation;
            }
            if let Some(margin) = margin {
                ui.options.margin = margin;
            }
            if let Some(open_after) = open_after {
                ui.open_after = open_after;
            }
            ui.error = None;
        }
        window.refresh();
        cx.notify();
    }

    fn close_images_to_pdf(&mut self, cx: &mut Context<Self>) {
        self.images_pdf.dialog = None;
        let _ = self.resources.modal_owner.compare_exchange(
            SHELL_EXPORT_OWNER,
            0,
            Ordering::SeqCst,
            Ordering::SeqCst,
        );
        self.report(cx);
        cx.notify();
    }

    fn submit_images_to_pdf(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(ui) = self.images_pdf.dialog.clone() else {
            return;
        };
        let plan = match ImagesToPdfPlan::new(ui.images.clone(), ui.options) {
            Ok(plan) => plan,
            Err(error) => {
                if let Some(dialog) = &mut self.images_pdf.dialog {
                    dialog.error = Some(error.to_string());
                }
                window.refresh();
                cx.notify();
                return;
            }
        };
        window.close_dialog(cx);
        self.close_images_to_pdf(cx);
        self.start_images_to_pdf(plan, ui.open_after, cx);
    }

    pub(super) fn quick_convert_images_to_pdf(
        &mut self,
        raw_images: Vec<PathBuf>,
        cx: &mut Context<Self>,
    ) {
        if !self.can_start_images_to_pdf(cx) {
            self.notice = Some(
                "Selesaikan dialog atau proses ekspor yang sedang berjalan terlebih dahulu.".into(),
            );
            cx.notify();
            return;
        }
        let (images, _) = filter_incoming_images(raw_images);
        let plan = match ImagesToPdfPlan::new(images, ImagesToPdfOptions::default()) {
            Ok(plan) => plan,
            Err(error) => {
                self.notice = Some(format!("Gambar ke PDF gagal: {error}"));
                cx.notify();
                return;
            }
        };
        let Some(destination) = compute_quick_destination(plan.images()) else {
            self.notice = Some(
                "Tidak dapat menentukan file PDF tujuan yang bebas konflik di folder sumber."
                    .into(),
            );
            cx.notify();
            return;
        };
        self.start_images_to_pdf_with_destination(plan, Some(destination), true, cx);
    }

    fn start_images_to_pdf(
        &mut self,
        plan: ImagesToPdfPlan,
        open_after: bool,
        cx: &mut Context<Self>,
    ) {
        self.start_images_to_pdf_with_destination(plan, None, open_after, cx);
    }

    fn start_images_to_pdf_with_destination(
        &mut self,
        plan: ImagesToPdfPlan,
        fixed_destination: Option<PathBuf>,
        open_after: bool,
        cx: &mut Context<Self>,
    ) {
        if self
            .resources
            .export_owner
            .compare_exchange(0, SHELL_EXPORT_OWNER, Ordering::SeqCst, Ordering::SeqCst)
            .is_err()
        {
            return;
        }
        let dialog = if fixed_destination.is_none() {
            let parent = plan
                .images()
                .first()
                .and_then(|p| p.parent())
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from("."));
            let suggested = suggested_pdf_name(plan.images());
            Some(cx.prompt_for_new_path(&parent, Some(&suggested)))
        } else {
            None
        };
        let control = ExportControl::with_budget(std::time::Duration::from_secs(600));
        let done = Arc::new(AtomicUsize::new(0));
        self.images_pdf.job_seq += 1;
        let job_id = self.images_pdf.job_seq;
        self.images_pdf.job = Some(ImagesPdfJob {
            total: plan.images().len(),
            done: done.clone(),
            control: control.clone(),
            open_after,
            job_id,
        });
        self.images_pdf.success = false;
        self.images_pdf.pages = 0;
        self.notice = None;
        self.report(cx);
        cx.notify();

        // Poll progress off-UI-thread work for banner/diagnostics updates.
        cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(std::time::Duration::from_millis(150))
                    .await;
                let active = this
                    .update(cx, |this, cx| {
                        let same = this
                            .images_pdf
                            .job
                            .as_ref()
                            .is_some_and(|j| j.job_id == job_id);
                        if same {
                            this.report(cx);
                            cx.notify();
                        }
                        same
                    })
                    .unwrap_or(false);
                if !active {
                    break;
                }
            }
        })
        .detach();

        let renderer = self.renderer.clone();
        cx.spawn(async move |this, cx| {
            let destination = match fixed_destination {
                Some(path) => path,
                None => match dialog {
                    Some(dialog) => match dialog.await {
                        Ok(Ok(Some(path))) => {
                            if path.extension().is_none() {
                                path.with_extension("pdf")
                            } else {
                                path
                            }
                        }
                        Ok(Ok(None)) => {
                            let _ = this.update(cx, |this, cx| {
                                this.finish_images_to_pdf(Err(ImagesToPdfError::Cancelled), cx)
                            });
                            return;
                        }
                        _ => {
                            let _ = this.update(cx, |this, cx| {
                                this.finish_images_to_pdf(Err(ImagesToPdfError::Destination), cx)
                            });
                            return;
                        }
                    },
                    None => {
                        let _ = this.update(cx, |this, cx| {
                            this.finish_images_to_pdf(Err(ImagesToPdfError::Destination), cx)
                        });
                        return;
                    }
                },
            };
            let result = cx
                .background_executor()
                .spawn(async move {
                    run_images_to_pdf(
                        &plan,
                        &destination,
                        &LocalImagePdfWriter,
                        &renderer,
                        &control,
                        |count| {
                            done.store(count, Ordering::SeqCst);
                        },
                    )
                })
                .await;
            let _ = this.update(cx, |this, cx| this.finish_images_to_pdf(result, cx));
        })
        .detach();
    }

    pub(super) fn cancel_images_to_pdf(&mut self, cx: &mut Context<Self>) {
        if let Some(job) = &self.images_pdf.job {
            job.control.cancel();
        }
        cx.notify();
    }

    fn finish_images_to_pdf(
        &mut self,
        result: Result<ImagesToPdfReceipt, ImagesToPdfError>,
        cx: &mut Context<Self>,
    ) {
        let open_after = self.images_pdf.job.take().is_some_and(|job| job.open_after);
        let _ = self.resources.export_owner.compare_exchange(
            SHELL_EXPORT_OWNER,
            0,
            Ordering::SeqCst,
            Ordering::SeqCst,
        );
        self.images_pdf.completed += 1;
        self.images_pdf.success = result.is_ok();
        match result {
            Ok(receipt) => {
                self.images_pdf.pages = receipt.pages;
                self.notice = Some(format!(
                    "{} halaman PDF tersimpan di {}.",
                    receipt.pages,
                    receipt.destination.display()
                ));
                if open_after && self.close.is_none() && self.tabs.len() < MAX_TABS {
                    self.open_paths(vec![receipt.destination], cx);
                }
            }
            Err(ImagesToPdfError::Cancelled) => {
                self.notice =
                    Some("Konversi Gambar ke PDF dibatalkan. File tujuan tidak dibuat.".into());
            }
            Err(error) => {
                self.notice = Some(format!("Gambar ke PDF gagal: {error}"));
            }
        }
        if self.close.is_some() {
            self.advance_close(cx);
        }
        self.report(cx);
        cx.notify();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quick_destination_picks_non_colliding_names() {
        let dir = tempfile::tempdir().ok();
        assert!(dir.is_some());
        let Some(dir) = dir else { return };
        let sub = dir.path().join("Foto Liburan");
        assert!(std::fs::create_dir_all(&sub).is_ok());

        let one = vec![sub.join("nota.png")];
        assert_eq!(compute_quick_destination(&one), Some(sub.join("nota.pdf")));

        assert!(std::fs::write(sub.join("nota.pdf"), b"%PDF-1.4").is_ok());
        assert_eq!(
            compute_quick_destination(&one),
            Some(sub.join("nota-2.pdf"))
        );

        let many = vec![sub.join("a.png"), sub.join("b.jpg")];
        assert_eq!(
            compute_quick_destination(&many),
            Some(sub.join("Foto Liburan.pdf"))
        );
        assert!(std::fs::write(sub.join("Foto Liburan.pdf"), b"%PDF-1.4").is_ok());
        assert_eq!(
            compute_quick_destination(&many),
            Some(sub.join("Foto Liburan-2.pdf"))
        );
    }
}
