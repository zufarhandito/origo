//! Document-level dialogs for automatic Page Numbers and diagonal Text Watermarks.
//! Both apply atomically across targeted pages in a single Undo/Redo step via `Command::AnnotateMany`.
use crate::*;
use gpui::component::{
    ActiveTheme, Disableable, Selectable, Sizable, StyledExt, WindowExt,
    button::{Button, ButtonVariants},
    dialog::DialogFooter,
    input::Input,
};
use pdf_application::annotation::{
    MAX_WATERMARK_CHARS, PageNumberFormat, PageNumberPosition, format_page_number,
    page_number_rect, parse_page_targets, rasterize_watermark, validate_text, watermark_rect,
};
use pdf_domain::{
    Command,
    annotation::{Annotation, MAX_PAGE_ANNOTATIONS, TextAnnotation, within_signature_budget},
};
use std::sync::atomic::Ordering;

const PAGE_NUMBER_SIZES: [f32; 4] = [10., 12., 14., 16.];
const PAGE_NUMBER_COLORS: [(&str, [u8; 3]); 3] = [
    ("Hitam", [24, 24, 27]),
    ("Abu-abu", [100, 116, 139]),
    ("Biru", [30, 64, 175]),
];

const WATERMARK_PRESETS: [&str; 4] = ["RAHASIA", "DRAFT", "SALINAN", "LUNAS"];
const WATERMARK_ANGLES: [(&str, i16); 3] = [
    ("-45° Diagonal", -45),
    ("0° Mendatar", 0),
    ("45° Diagonal", 45),
];
const WATERMARK_OPACITIES: [u8; 4] = [15, 25, 40, 60];
const WATERMARK_SIZES: [(&str, f32); 3] = [("Kecil", 0.40), ("Sedang", 0.62), ("Besar", 0.82)];
const WATERMARK_COLORS: [(&str, [u8; 3]); 4] = [
    ("Abu-abu", [100, 116, 139]),
    ("Merah", [185, 28, 28]),
    ("Biru", [30, 64, 175]),
    ("Hitam", [24, 24, 27]),
];

#[derive(Clone)]
pub(super) struct PageNumbersDialog {
    pub format: PageNumberFormat,
    pub position: PageNumberPosition,
    pub all_pages: bool,
    pub range: Entity<InputState>,
    pub start_number: Entity<InputState>,
    pub font_size: f32,
    pub color_index: usize,
    pub error: Option<String>,
}

#[derive(Clone)]
pub(super) struct WatermarkDialog {
    pub text: Entity<InputState>,
    pub angle: i16,
    pub opacity: u8,
    pub size_index: usize,
    pub color_index: usize,
    pub all_pages: bool,
    pub range: Entity<InputState>,
    pub error: Option<String>,
}

fn parse_start_number(raw: &str) -> Result<usize, String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() || !trimmed.bytes().all(|b| b.is_ascii_digit()) {
        return Err("Nomor awal harus berupa angka bulat positif (misalnya 1).".into());
    }
    let value = trimmed
        .parse::<usize>()
        .map_err(|_| "Nomor awal terlalu besar.".to_owned())?;
    if value == 0 || value > 99_999 {
        return Err("Nomor awal harus antara 1 dan 99999.".into());
    }
    Ok(value)
}

impl DocumentSession {
    pub(super) fn show_page_numbers(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(owner) = &self.document_owner {
            let _ = owner.update(cx, |s, cx| s.show_page_numbers(window, cx));
            return;
        }
        if self.edits_locked()
            || self.opening
            || self.resources.modal_owner.load(Ordering::SeqCst) != 0
        {
            return;
        }
        let Some(page_count) = self.workspace.as_ref().map(|w| w.pages().len()) else {
            return;
        };
        if page_count == 0 {
            return;
        }
        let range = cx.new(|cx| {
            let mut s = InputState::new(window, cx).placeholder("Contoh: 2-10 atau 1-3, 5");
            s.set_value(format!("1-{page_count}"), window, cx);
            s
        });
        let start_number = cx.new(|cx| {
            let mut s = InputState::new(window, cx).placeholder("1");
            s.set_value("1", window, cx);
            s
        });
        for input in [&range, &start_number] {
            cx.subscribe_in(input, window, |this, _, event: &InputEvent, window, cx| {
                if matches!(event, InputEvent::Change) {
                    if let Some(ui) = &mut this.page_numbers_dialog {
                        ui.error = None;
                    }
                    window.refresh();
                    cx.notify();
                }
            })
            .detach();
        }
        self.page_numbers_dialog = Some(PageNumbersDialog {
            format: PageNumberFormat::SlashTotal,
            position: PageNumberPosition::BottomCenter,
            all_pages: true,
            range,
            start_number,
            font_size: 12.,
            color_index: 0,
            error: None,
        });
        self.error_popup = true;
        self.resources
            .modal_owner
            .store(self.tab_id.0, Ordering::SeqCst);
        self.report(cx);

        let weak = cx.weak_entity();
        window.open_alert_dialog(cx, move |dialog, _, cx| {
            let Some(ui) = weak
                .read_with(cx, |this, _| this.page_numbers_dialog.clone())
                .ok()
                .flatten()
            else {
                return dialog;
            };
            let total_pages = weak
                .read_with(cx, |this, _| {
                    this.workspace.as_ref().map_or(0, |w| w.pages().len())
                })
                .unwrap_or(0);
            let preview_result = weak
                .read_with(cx, |this, cx| this.preview_page_numbers(&ui, cx))
                .unwrap_or_else(|_| Err("Dokumen tidak tersedia.".into()));
            let colors = cx.theme().colors;

            let format_buttons = gpui::div().flex().flex_wrap().gap_2().children(
                [
                    (PageNumberFormat::NumberOnly, "1"),
                    (PageNumberFormat::SlashTotal, "1 / N"),
                    (PageNumberFormat::IndonesianFull, "Halaman 1 dari N"),
                ]
                .into_iter()
                .enumerate()
                .map(|(idx, (fmt, label))| {
                    let w = weak.clone();
                    Button::new(("pn-fmt", idx))
                        .small()
                        .label(label)
                        .selected(ui.format == fmt)
                        .on_click(move |_, window, cx| {
                            let _ = w.update(cx, |this, cx| {
                                if let Some(d) = &mut this.page_numbers_dialog {
                                    d.format = fmt;
                                    d.error = None;
                                }
                                window.refresh();
                                cx.notify();
                            });
                        })
                }),
            );

            let top_positions = [
                (PageNumberPosition::TopLeft, "Kiri Atas"),
                (PageNumberPosition::TopCenter, "Tengah Atas"),
                (PageNumberPosition::TopRight, "Kanan Atas"),
            ];
            let bottom_positions = [
                (PageNumberPosition::BottomLeft, "Kiri Bawah"),
                (PageNumberPosition::BottomCenter, "Tengah Bawah"),
                (PageNumberPosition::BottomRight, "Kanan Bawah"),
            ];
            let pos_row = |offset: usize, items: [(PageNumberPosition, &str); 3]| {
                gpui::div().flex().gap_2().children(
                    items.into_iter().enumerate().map(|(i, (pos, label))| {
                        let w = weak.clone();
                        Button::new(("pn-pos", offset + i))
                            .flex_1()
                            .small()
                            .label(label)
                            .selected(ui.position == pos)
                            .on_click(move |_, window, cx| {
                                let _ = w.update(cx, |this, cx| {
                                    if let Some(d) = &mut this.page_numbers_dialog {
                                        d.position = pos;
                                        d.error = None;
                                    }
                                    window.refresh();
                                    cx.notify();
                                });
                            })
                    }),
                )
            };

            let w_all = weak.clone();
            let w_range = weak.clone();
            let cancel = weak.clone();
            let submit = weak.clone();
            let closed = weak.clone();

            let mut body = gpui::div()
                .flex()
                .flex_col()
                .gap_3()
                .child(
                    gpui::div()
                        .flex()
                        .flex_col()
                        .gap_1()
                        .child(gpui::div().text_xs().font_semibold().child("Format nomor"))
                        .child(format_buttons),
                )
                .child(
                    gpui::div()
                        .flex()
                        .flex_col()
                        .gap_1p5()
                        .child(gpui::div().text_xs().font_semibold().child("Posisi pada halaman"))
                        .child(pos_row(0, top_positions))
                        .child(pos_row(3, bottom_positions)),
                )
                .child(
                    gpui::div()
                        .flex()
                        .flex_col()
                        .gap_1p5()
                        .child(gpui::div().text_xs().font_semibold().child("Halaman target"))
                        .child(
                            gpui::div()
                                .flex()
                                .gap_2()
                                .child(
                                    Button::new("pn-all")
                                        .small()
                                        .label(format!("Semua ({total_pages} halaman)"))
                                        .selected(ui.all_pages)
                                        .on_click(move |_, window, cx| {
                                            let _ = w_all.update(cx, |this, cx| {
                                                if let Some(d) = &mut this.page_numbers_dialog {
                                                    d.all_pages = true;
                                                    d.error = None;
                                                }
                                                window.refresh();
                                                cx.notify();
                                            });
                                        }),
                                )
                                .child(
                                    Button::new("pn-range-mode")
                                        .small()
                                        .label("Rentang khusus")
                                        .selected(!ui.all_pages)
                                        .on_click(move |_, window, cx| {
                                            let _ = w_range.update(cx, |this, cx| {
                                                if let Some(d) = &mut this.page_numbers_dialog {
                                                    d.all_pages = false;
                                                    d.error = None;
                                                }
                                                window.refresh();
                                                cx.notify();
                                            });
                                        }),
                                ),
                        )
                        .when(!ui.all_pages, |el| {
                            el.child(Input::new(&ui.range).aria_label("Rentang halaman penomoran"))
                        }),
                )
                .child(
                    gpui::div()
                        .flex()
                        .items_center()
                        .gap_3()
                        .child(
                            gpui::div()
                                .w(px(110.))
                                .flex()
                                .flex_col()
                                .gap_1()
                                .child(gpui::div().text_xs().font_semibold().child("Mulai dari"))
                                .child(
                                    Input::new(&ui.start_number)
                                        .small()
                                        .aria_label("Nomor awal halaman"),
                                ),
                        )
                        .child(
                            gpui::div()
                                .flex_1()
                                .flex()
                                .flex_col()
                                .gap_1()
                                .child(gpui::div().text_xs().font_semibold().child("Ukuran font"))
                                .child(gpui::div().flex().gap_1p5().children(
                                    PAGE_NUMBER_SIZES.into_iter().enumerate().map(|(i, sz)| {
                                        let w = weak.clone();
                                        Button::new(("pn-sz", i))
                                            .small()
                                            .label(format!("{} pt", sz as u32))
                                            .selected((ui.font_size - sz).abs() < 0.1)
                                            .on_click(move |_, window, cx| {
                                                let _ = w.update(cx, |this, cx| {
                                                    if let Some(d) = &mut this.page_numbers_dialog {
                                                        d.font_size = sz;
                                                    }
                                                    window.refresh();
                                                    cx.notify();
                                                });
                                            })
                                    }),
                                )),
                        ),
                )
                .child(
                    gpui::div()
                        .flex()
                        .flex_col()
                        .gap_1()
                        .child(gpui::div().text_xs().font_semibold().child("Warna"))
                        .child(gpui::div().flex().gap_2().children(
                            PAGE_NUMBER_COLORS.into_iter().enumerate().map(|(i, (label, _))| {
                                let w = weak.clone();
                                Button::new(("pn-clr", i))
                                    .small()
                                    .label(label)
                                    .selected(ui.color_index == i)
                                    .on_click(move |_, window, cx| {
                                        let _ = w.update(cx, |this, cx| {
                                            if let Some(d) = &mut this.page_numbers_dialog {
                                                d.color_index = i;
                                            }
                                            window.refresh();
                                            cx.notify();
                                        });
                                    })
                            }),
                        )),
                );

            let summary_text = match &preview_result {
                Ok(text) => text.clone(),
                Err(err) => err.clone(),
            };
            body = body
                .child(
                    gpui::div()
                        .id("page-numbers-preview")
                        .role(gpui::Role::Group)
                        .aria_label(summary_text.clone())
                        .text_xs()
                        .text_color(if preview_result.is_ok() {
                            colors.muted_foreground
                        } else {
                            cx.theme().danger
                        })
                        .child(summary_text),
                )
                .when_some(ui.error.clone(), |el, err| {
                    el.child(
                        gpui::div()
                            .id("page-numbers-error")
                            .text_xs()
                            .text_color(cx.theme().danger)
                            .child(err),
                    )
                });

            dialog
                .title(
                    gpui::div()
                        .id("page-numbers-title")
                        .role(gpui::Role::Group)
                        .aria_label("Nomor Halaman")
                        .child("Tambah Nomor Halaman"),
                )
                .description(
                    "Sisipkan nomor halaman pasif di seluruh atau sebagian halaman. Bisa dibatalkan sekaligus dengan Ctrl+Z.",
                )
                .centered()
                .width(px(560.))
                .close_button(true)
                .child(body)
                .footer(
                    DialogFooter::new()
                        .child(Button::new("pn-cancel").label("Batal").on_click(
                            move |_, window, cx| {
                                window.close_dialog(cx);
                                let _ = cancel.update(cx, |this, cx| {
                                    this.close_page_numbers(cx);
                                });
                            },
                        ))
                        .child(
                            Button::new("pn-apply")
                                .primary()
                                .disabled(preview_result.is_err())
                                .label("Terapkan Nomor Halaman")
                                .on_click(move |_, window, cx| {
                                    let _ = submit.update(cx, |this, cx| {
                                        this.apply_page_numbers(window, cx);
                                    });
                                }),
                        ),
                )
                .on_close(move |_, _, cx| {
                    let _ = closed.update(cx, |this, cx| this.close_page_numbers(cx));
                })
        });
        cx.notify();
    }

    fn resolve_page_number_targets(
        &self,
        ui: &PageNumbersDialog,
        cx: &App,
    ) -> Result<(Vec<usize>, usize), String> {
        let Some(w) = self.workspace.as_ref() else {
            return Err("Dokumen tidak tersedia.".into());
        };
        let page_count = w.pages().len();
        let targets = if ui.all_pages {
            (0..page_count).collect::<Vec<_>>()
        } else {
            let spec = ui.range.read(cx).value().to_string();
            parse_page_targets(&spec, page_count, None)?
        };
        if targets.is_empty() {
            return Err("Pilih minimal 1 halaman target.".into());
        }
        let start = parse_start_number(&ui.start_number.read(cx).value())?;
        for &idx in &targets {
            let page_id = w.pages()[idx].id;
            if w.annotations(page_id).len() >= MAX_PAGE_ANNOTATIONS {
                return Err(format!(
                    "Halaman {} sudah mencapai batas {MAX_PAGE_ANNOTATIONS} anotasi.",
                    idx + 1
                ));
            }
        }
        Ok((targets, start))
    }

    fn preview_page_numbers(&self, ui: &PageNumbersDialog, cx: &App) -> Result<String, String> {
        let (targets, start) = self.resolve_page_number_targets(ui, cx)?;
        let total = start.saturating_add(targets.len().saturating_sub(1));
        let sample = format_page_number(ui.format, start, total);
        Ok(format!(
            "Contoh halaman pertama target: \"{sample}\" · {} halaman target · 1 langkah Undo.",
            targets.len()
        ))
    }

    fn close_page_numbers(&mut self, cx: &mut Context<Self>) {
        self.page_numbers_dialog = None;
        self.error_popup = false;
        let _ = self.resources.modal_owner.compare_exchange(
            self.tab_id.0,
            0,
            Ordering::SeqCst,
            Ordering::SeqCst,
        );
        self.report(cx);
        cx.notify();
    }

    fn apply_page_numbers(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(ui) = self.page_numbers_dialog.clone() else {
            return;
        };
        let (targets, start) = match self.resolve_page_number_targets(&ui, cx) {
            Ok(v) => v,
            Err(err) => {
                if let Some(d) = &mut self.page_numbers_dialog {
                    d.error = Some(err);
                }
                window.refresh();
                cx.notify();
                return;
            }
        };
        let color = PAGE_NUMBER_COLORS
            .get(ui.color_index)
            .map(|(_, c)| *c)
            .unwrap_or([24, 24, 27]);
        let total_display = start.saturating_add(targets.len().saturating_sub(1));
        let Some(mut w) = self.workspace.as_mut() else {
            return;
        };
        let mut changes = Vec::with_capacity(targets.len());
        for (offset, &page_index) in targets.iter().enumerate() {
            let Ok(geometry) = w.geometry(page_index) else {
                continue;
            };
            let [page_w, page_h] = crate::signatures::unrotated_page_size(geometry);
            let label = format_page_number(ui.format, start + offset, total_display);
            let Some([p0, p1]) =
                page_number_rect(&label, ui.font_size, page_w, page_h, ui.position)
            else {
                continue;
            };
            let page_id = w.pages()[page_index].id;
            let mut all = w.annotations(page_id).to_vec();
            let id = w.allocate_annotation_id();
            all.push(Annotation {
                id,
                highlight: false,
                color,
                width: 1.,
                points: vec![p0, p1],
                rectangles: false,
                signature: None,
                text: Some(TextAnnotation {
                    content: label,
                    font_size: ui.font_size,
                }),
                extra: None,
            });
            changes.push((page_id, all));
        }
        let applied = changes.len();
        let result = w.execute(Command::AnnotateMany(changes));
        drop(w);
        window.close_dialog(cx);
        self.close_page_numbers(cx);
        match result {
            Ok(true) => {
                self.status = format!(
                    "Nomor halaman ditambahkan ke {applied} halaman. Ctrl+Z untuk membatalkan."
                );
                self.edited(cx);
            }
            Err(err) => {
                self.status = err.to_string();
                cx.notify();
            }
            Ok(false) => {}
        }
        self.focus.focus(window, cx);
    }

    pub(super) fn show_watermark(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(owner) = &self.document_owner {
            let _ = owner.update(cx, |s, cx| s.show_watermark(window, cx));
            return;
        }
        if self.edits_locked()
            || self.opening
            || self.resources.modal_owner.load(Ordering::SeqCst) != 0
        {
            return;
        }
        let Some(page_count) = self.workspace.as_ref().map(|w| w.pages().len()) else {
            return;
        };
        if page_count == 0 {
            return;
        }
        let text = cx.new(|cx| {
            let mut s = InputState::new(window, cx).placeholder("Contoh: RAHASIA, DRAFT, SALINAN");
            s.set_value("RAHASIA", window, cx);
            s
        });
        let range = cx.new(|cx| {
            let mut s = InputState::new(window, cx).placeholder("Contoh: 1-3, 5");
            s.set_value(format!("1-{page_count}"), window, cx);
            s
        });
        for input in [&text, &range] {
            cx.subscribe_in(input, window, |this, _, event: &InputEvent, window, cx| {
                if matches!(event, InputEvent::Change) {
                    if let Some(ui) = &mut this.watermark_dialog {
                        ui.error = None;
                    }
                    window.refresh();
                    cx.notify();
                }
            })
            .detach();
        }
        self.watermark_dialog = Some(WatermarkDialog {
            text,
            angle: -45,
            opacity: 25,
            size_index: 1,
            color_index: 0,
            all_pages: true,
            range,
            error: None,
        });
        self.error_popup = true;
        self.resources
            .modal_owner
            .store(self.tab_id.0, Ordering::SeqCst);
        self.report(cx);

        let weak = cx.weak_entity();
        window.open_alert_dialog(cx, move |dialog, _, cx| {
            let Some(ui) = weak
                .read_with(cx, |this, _| this.watermark_dialog.clone())
                .ok()
                .flatten()
            else {
                return dialog;
            };
            let total_pages = weak
                .read_with(cx, |this, _| {
                    this.workspace.as_ref().map_or(0, |w| w.pages().len())
                })
                .unwrap_or(0);
            let preview_result = weak
                .read_with(cx, |this, cx| this.preview_watermark(&ui, cx))
                .unwrap_or_else(|_| Err("Dokumen tidak tersedia.".into()));
            let colors = cx.theme().colors;

            let presets = gpui::div().flex().flex_wrap().gap_1p5().children(
                WATERMARK_PRESETS.into_iter().enumerate().map(|(i, label)| {
                    let text_entity = ui.text.clone();
                    let w = weak.clone();
                    Button::new(("wm-preset", i))
                        .outline()
                        .small()
                        .label(label)
                        .on_click(move |_, window, cx| {
                            text_entity.update(cx, |s, cx| s.set_value(label, window, cx));
                            let _ = w.update(cx, |this, cx| {
                                if let Some(d) = &mut this.watermark_dialog {
                                    d.error = None;
                                }
                                window.refresh();
                                cx.notify();
                            });
                        })
                }),
            );

            let angle_row = gpui::div().flex().gap_2().children(
                WATERMARK_ANGLES.into_iter().enumerate().map(|(i, (label, deg))| {
                    let w = weak.clone();
                    Button::new(("wm-angle", i))
                        .small()
                        .label(label)
                        .selected(ui.angle == deg)
                        .on_click(move |_, window, cx| {
                            let _ = w.update(cx, |this, cx| {
                                if let Some(d) = &mut this.watermark_dialog {
                                    d.angle = deg;
                                }
                                window.refresh();
                                cx.notify();
                            });
                        })
                }),
            );

            let opacity_row = gpui::div().flex().gap_1p5().children(
                WATERMARK_OPACITIES.into_iter().enumerate().map(|(i, op)| {
                    let w = weak.clone();
                    Button::new(("wm-op", i))
                        .small()
                        .label(format!("{op}%"))
                        .selected(ui.opacity == op)
                        .on_click(move |_, window, cx| {
                            let _ = w.update(cx, |this, cx| {
                                if let Some(d) = &mut this.watermark_dialog {
                                    d.opacity = op;
                                }
                                window.refresh();
                                cx.notify();
                            });
                        })
                }),
            );

            let size_row = gpui::div().flex().gap_1p5().children(
                WATERMARK_SIZES.into_iter().enumerate().map(|(i, (label, _))| {
                    let w = weak.clone();
                    Button::new(("wm-sz", i))
                        .small()
                        .label(label)
                        .selected(ui.size_index == i)
                        .on_click(move |_, window, cx| {
                            let _ = w.update(cx, |this, cx| {
                                if let Some(d) = &mut this.watermark_dialog {
                                    d.size_index = i;
                                }
                                window.refresh();
                                cx.notify();
                            });
                        })
                }),
            );

            let color_row = gpui::div().flex().gap_2().children(
                WATERMARK_COLORS.into_iter().enumerate().map(|(i, (label, _))| {
                    let w = weak.clone();
                    Button::new(("wm-clr", i))
                        .small()
                        .label(label)
                        .selected(ui.color_index == i)
                        .on_click(move |_, window, cx| {
                            let _ = w.update(cx, |this, cx| {
                                if let Some(d) = &mut this.watermark_dialog {
                                    d.color_index = i;
                                }
                                window.refresh();
                                cx.notify();
                            });
                        })
                }),
            );

            let w_all = weak.clone();
            let w_range = weak.clone();
            let cancel = weak.clone();
            let submit = weak.clone();
            let closed = weak.clone();

            let summary_text = match &preview_result {
                Ok(text) => text.clone(),
                Err(err) => err.clone(),
            };

            let body = gpui::div()
                .flex()
                .flex_col()
                .gap_3()
                .child(
                    gpui::div()
                        .flex()
                        .flex_col()
                        .gap_1p5()
                        .child(gpui::div().text_xs().font_semibold().child("Teks watermark"))
                        .child(Input::new(&ui.text).aria_label("Teks watermark"))
                        .child(presets),
                )
                .child(
                    gpui::div()
                        .flex()
                        .flex_col()
                        .gap_1()
                        .child(gpui::div().text_xs().font_semibold().child("Sudut kemiringan"))
                        .child(angle_row),
                )
                .child(
                    gpui::div()
                        .flex()
                        .gap_4()
                        .child(
                            gpui::div()
                                .flex_1()
                                .flex()
                                .flex_col()
                                .gap_1()
                                .child(gpui::div().text_xs().font_semibold().child("Opasitas"))
                                .child(opacity_row),
                        )
                        .child(
                            gpui::div()
                                .flex_1()
                                .flex()
                                .flex_col()
                                .gap_1()
                                .child(gpui::div().text_xs().font_semibold().child("Ukuran"))
                                .child(size_row),
                        ),
                )
                .child(
                    gpui::div()
                        .flex()
                        .flex_col()
                        .gap_1()
                        .child(gpui::div().text_xs().font_semibold().child("Warna"))
                        .child(color_row),
                )
                .child(
                    gpui::div()
                        .flex()
                        .flex_col()
                        .gap_1p5()
                        .child(gpui::div().text_xs().font_semibold().child("Halaman target"))
                        .child(
                            gpui::div()
                                .flex()
                                .gap_2()
                                .child(
                                    Button::new("wm-all")
                                        .small()
                                        .label(format!("Semua ({total_pages} halaman)"))
                                        .selected(ui.all_pages)
                                        .on_click(move |_, window, cx| {
                                            let _ = w_all.update(cx, |this, cx| {
                                                if let Some(d) = &mut this.watermark_dialog {
                                                    d.all_pages = true;
                                                    d.error = None;
                                                }
                                                window.refresh();
                                                cx.notify();
                                            });
                                        }),
                                )
                                .child(
                                    Button::new("wm-range-mode")
                                        .small()
                                        .label("Rentang khusus")
                                        .selected(!ui.all_pages)
                                        .on_click(move |_, window, cx| {
                                            let _ = w_range.update(cx, |this, cx| {
                                                if let Some(d) = &mut this.watermark_dialog {
                                                    d.all_pages = false;
                                                    d.error = None;
                                                }
                                                window.refresh();
                                                cx.notify();
                                            });
                                        }),
                                ),
                        )
                        .when(!ui.all_pages, |el| {
                            el.child(Input::new(&ui.range).aria_label("Rentang halaman watermark"))
                        }),
                )
                .child(
                    gpui::div()
                        .id("watermark-preview")
                        .role(gpui::Role::Group)
                        .aria_label(summary_text.clone())
                        .text_xs()
                        .text_color(if preview_result.is_ok() {
                            colors.muted_foreground
                        } else {
                            cx.theme().danger
                        })
                        .child(summary_text),
                )
                .when_some(ui.error.clone(), |el, err| {
                    el.child(
                        gpui::div()
                            .id("watermark-error")
                            .text_xs()
                            .text_color(cx.theme().danger)
                            .child(err),
                    )
                });

            dialog
                .title(
                    gpui::div()
                        .id("watermark-title")
                        .role(gpui::Role::Group)
                        .aria_label("Watermark")
                        .child("Tambah Watermark Teks"),
                )
                .description(
                    "Tambahkan watermark semi-transparan ke halaman PDF. Menggunakan 1 aset bersama yang efisien dan bisa di-Undo sekaligus.",
                )
                .centered()
                .width(px(580.))
                .close_button(true)
                .child(body)
                .footer(
                    DialogFooter::new()
                        .child(Button::new("wm-cancel").label("Batal").on_click(
                            move |_, window, cx| {
                                window.close_dialog(cx);
                                let _ = cancel.update(cx, |this, cx| {
                                    this.close_watermark(cx);
                                });
                            },
                        ))
                        .child(
                            Button::new("wm-apply")
                                .primary()
                                .disabled(preview_result.is_err())
                                .label("Terapkan Watermark")
                                .on_click(move |_, window, cx| {
                                    let _ = submit.update(cx, |this, cx| {
                                        this.apply_watermark(window, cx);
                                    });
                                }),
                        ),
                )
                .on_close(move |_, _, cx| {
                    let _ = closed.update(cx, |this, cx| this.close_watermark(cx));
                })
        });
        cx.notify();
    }

    fn resolve_watermark_targets(
        &self,
        ui: &WatermarkDialog,
        cx: &App,
    ) -> Result<(String, Vec<usize>), String> {
        let Some(w) = self.workspace.as_ref() else {
            return Err("Dokumen tidak tersedia.".into());
        };
        let raw = ui.text.read(cx).value().to_string();
        let trimmed = raw.trim().to_owned();
        if trimmed.is_empty() || trimmed.chars().count() > MAX_WATERMARK_CHARS {
            return Err(format!(
                "Isi teks watermark 1-{MAX_WATERMARK_CHARS} karakter dalam satu baris."
            ));
        }
        validate_text(&trimmed)?;
        let page_count = w.pages().len();
        let targets = if ui.all_pages {
            (0..page_count).collect::<Vec<_>>()
        } else {
            let spec = ui.range.read(cx).value().to_string();
            parse_page_targets(&spec, page_count, None)?
        };
        if targets.is_empty() {
            return Err("Pilih minimal 1 halaman target.".into());
        }
        for &idx in &targets {
            let page_id = w.pages()[idx].id;
            if w.annotations(page_id).len() >= MAX_PAGE_ANNOTATIONS {
                return Err(format!(
                    "Halaman {} sudah mencapai batas {MAX_PAGE_ANNOTATIONS} anotasi.",
                    idx + 1
                ));
            }
        }
        Ok((trimmed, targets))
    }

    fn preview_watermark(&self, ui: &WatermarkDialog, cx: &App) -> Result<String, String> {
        let (text, targets) = self.resolve_watermark_targets(ui, cx)?;
        Ok(format!(
            "\"{text}\" · {}° · opasitas {}% · {} halaman · 1 langkah Undo.",
            ui.angle,
            ui.opacity,
            targets.len()
        ))
    }

    fn close_watermark(&mut self, cx: &mut Context<Self>) {
        self.watermark_dialog = None;
        self.error_popup = false;
        let _ = self.resources.modal_owner.compare_exchange(
            self.tab_id.0,
            0,
            Ordering::SeqCst,
            Ordering::SeqCst,
        );
        self.report(cx);
        cx.notify();
    }

    fn apply_watermark(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(ui) = self.watermark_dialog.clone() else {
            return;
        };
        let (text, targets) = match self.resolve_watermark_targets(&ui, cx) {
            Ok(v) => v,
            Err(err) => {
                if let Some(d) = &mut self.watermark_dialog {
                    d.error = Some(err);
                }
                window.refresh();
                cx.notify();
                return;
            }
        };
        let color = WATERMARK_COLORS
            .get(ui.color_index)
            .map(|(_, c)| *c)
            .unwrap_or([100, 116, 139]);
        let coverage = WATERMARK_SIZES
            .get(ui.size_index)
            .map(|(_, cov)| *cov)
            .unwrap_or(0.62);
        let Some(image) = rasterize_watermark(&text, color, ui.opacity, ui.angle) else {
            if let Some(d) = &mut self.watermark_dialog {
                d.error = Some("Gagal merender teks watermark. Coba teks lebih singkat.".into());
            }
            window.refresh();
            cx.notify();
            return;
        };
        let shared = Arc::new(image);
        let Some(mut w) = self.workspace.as_mut() else {
            return;
        };

        let sample = Annotation {
            id: 1,
            highlight: false,
            color,
            width: 1.,
            points: vec![[0.1, 0.1], [0.9, 0.9]],
            rectangles: false,
            signature: Some(shared.clone()),
            text: None,
            extra: None,
        };
        let simulated = w
            .pages()
            .iter()
            .flat_map(|p| w.annotations(p.id).iter())
            .chain(std::iter::repeat_n(&sample, targets.len()));
        if !within_signature_budget(simulated) {
            drop(w);
            if let Some(d) = &mut self.watermark_dialog {
                d.error = Some(
                    "Melebihi batas aset stempel/watermark pada dokumen ini (maks 8 aset unik / 1024 penempatan)."
                        .into(),
                );
            }
            window.refresh();
            cx.notify();
            return;
        }

        let mut changes = Vec::with_capacity(targets.len());
        for &page_index in &targets {
            let Ok(geometry) = w.geometry(page_index) else {
                continue;
            };
            let [page_w, page_h] = crate::signatures::unrotated_page_size(geometry);
            let Some([p0, p1]) =
                watermark_rect(shared.width, shared.height, page_w, page_h, coverage)
            else {
                continue;
            };
            let page_id = w.pages()[page_index].id;
            let mut all = w.annotations(page_id).to_vec();
            let id = w.allocate_annotation_id();
            all.push(Annotation {
                id,
                highlight: false,
                color,
                width: 1.,
                points: vec![p0, p1],
                rectangles: false,
                signature: Some(shared.clone()),
                text: None,
                extra: None,
            });
            changes.push((page_id, all));
        }
        let applied = changes.len();
        let result = w.execute(Command::AnnotateMany(changes));
        drop(w);
        window.close_dialog(cx);
        self.close_watermark(cx);
        match result {
            Ok(true) => {
                self.status =
                    format!("Watermark diterapkan ke {applied} halaman. Ctrl+Z untuk membatalkan.");
                self.edited(cx);
            }
            Err(err) => {
                self.status = err.to_string();
                cx.notify();
            }
            Ok(false) => {}
        }
        self.focus.focus(window, cx);
    }
}
