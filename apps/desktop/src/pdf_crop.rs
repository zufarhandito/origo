//! Window-level dialog and background QPDF job for lossless PDF margin cropping
//! and proportional paper-size standardization (A4 / Letter / F4 / Legal).
use crate::images_to_pdf::SHELL_EXPORT_OWNER;
use crate::shell::{MAX_TABS, WindowShell};
use crate::*;
use gpui::component::{
    ActiveTheme, Disableable, Selectable, Sizable, StyledExt, WindowExt,
    button::{Button, ButtonVariants},
    dialog::DialogFooter,
    input::Input,
};
use pdf_application::export::{ExportControl, ExportPlan, ExportScope};
use qpdf_adapter::{
    CropMargins, PageBoxOperation, PageBoxReport, PaperOrientation, PaperSize, QpdfPageBox,
    parse_page_selection,
};
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub(super) enum PageBoxMode {
    #[default]
    Crop,
    StandardizePaper,
}

#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub(super) enum CropUnit {
    #[default]
    Mm,
    Pt,
}

#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub(super) enum PageTargetScope {
    #[default]
    All,
    Current,
    Range,
}

#[derive(Clone)]
pub(super) struct PageBoxDialog {
    pub snapshot: ExportPlan,
    pub source_path: PathBuf,
    pub current_page_index: usize,
    pub mode: PageBoxMode,
    pub unit: CropUnit,
    pub top: Entity<InputState>,
    pub bottom: Entity<InputState>,
    pub left: Entity<InputState>,
    pub right: Entity<InputState>,
    pub paper: PaperSize,
    pub orientation: PaperOrientation,
    pub scope: PageTargetScope,
    pub range_input: Entity<InputState>,
    pub open_after: bool,
    pub custom_destination: Option<PathBuf>,
    pub picking: bool,
    pub error: Option<String>,
}

impl PageBoxDialog {
    fn destination(&self) -> PathBuf {
        if let Some(custom) = &self.custom_destination {
            custom.clone()
        } else {
            let suffix = match self.mode {
                PageBoxMode::Crop => "cropped",
                PageBoxMode::StandardizePaper => self.paper.suffix(),
            };
            suggested_page_box_destination(&self.source_path, suffix)
        }
    }
}

pub(super) struct PageBoxJob {
    pub mode: PageBoxMode,
    pub total: usize,
    pub done: Arc<AtomicUsize>,
    pub control: ExportControl,
    pub open_after: bool,
    pub job_id: u64,
}

#[derive(Default)]
pub(super) struct PageBoxState {
    pub dialog: Option<PageBoxDialog>,
    pub job: Option<PageBoxJob>,
    pub job_seq: u64,
}

impl PageBoxState {
    pub(super) fn status_label(&self) -> String {
        let Some(job) = &self.job else {
            return String::new();
        };
        let done = job.done.load(Ordering::SeqCst);
        match job.mode {
            PageBoxMode::Crop => format!("Memotong margin halaman: {done}/{} halaman…", job.total),
            PageBoxMode::StandardizePaper => {
                format!("Menyesuaikan ukuran kertas: {done}/{} halaman…", job.total)
            }
        }
    }
}

fn suggested_page_box_destination(source: &std::path::Path, suffix: &str) -> PathBuf {
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
    let base = format!("{stem}-{suffix}");
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

fn parse_margin_value(raw: &str) -> Result<f32, &'static str> {
    let text = raw.trim().replace(',', ".");
    if text.is_empty() {
        return Ok(0.0);
    }
    let val: f32 = text
        .parse()
        .map_err(|_| "Nilai margin harus berupa angka valid (contoh: 15 atau 12.5).")?;
    if !val.is_finite() || val < 0.0 || val > 1000.0 {
        return Err("Nilai margin harus antara 0 dan 1000.");
    }
    Ok(val)
}

impl WindowShell {
    pub(super) fn show_crop_paper_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.close.is_some()
            || self.modal(cx)
            || self.resources.export_owner.load(Ordering::SeqCst) != 0
            || self.page_box.job.is_some()
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
            let current = usize::from(s.navigation.current);
            ExportPlan::capture(&workspace, s.workspace_instance, ExportScope::All)
                .map(|plan| (plan, current))
        };
        let (snapshot, current_page_index) = match captured {
            Ok(pair) => pair,
            Err(err) => {
                self.notice = Some(format!("Halaman belum dapat diproses: {err}"));
                cx.notify();
                return;
            }
        };

        let make_margin_input = |default_val: &str, window: &mut Window, cx: &mut Context<Self>| {
            let val = default_val.to_owned();
            cx.new(|cx| {
                let mut s = InputState::new(window, cx).placeholder("0");
                s.set_value(val, window, cx);
                s
            })
        };
        let top = make_margin_input("15", window, cx);
        let bottom = make_margin_input("15", window, cx);
        let left = make_margin_input("15", window, cx);
        let right = make_margin_input("15", window, cx);
        let range_input = cx.new(|cx| {
            let mut s = InputState::new(window, cx).placeholder("contoh: 1-3, 5");
            s.set_value(format!("{}", current_page_index + 1), window, cx);
            s
        });

        for input in [&top, &bottom, &left, &right, &range_input] {
            cx.subscribe_in(input, window, |this, _, event: &InputEvent, window, cx| {
                if matches!(event, InputEvent::Change) {
                    if let Some(d) = &mut this.page_box.dialog {
                        d.error = None;
                    }
                    window.refresh();
                    cx.notify();
                }
            })
            .detach();
        }

        self.page_box.dialog = Some(PageBoxDialog {
            snapshot,
            source_path,
            current_page_index,
            mode: PageBoxMode::Crop,
            unit: CropUnit::Mm,
            top,
            bottom,
            left,
            right,
            paper: PaperSize::A4,
            orientation: PaperOrientation::Auto,
            scope: PageTargetScope::All,
            range_input,
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
                .read_with(cx, |this, _| this.page_box.dialog.clone())
                .ok()
                .flatten()
            else {
                return dialog;
            };
            let colors = cx.theme().colors;
            let total_pages = ui.snapshot.pages().len();
            let current_one_based = (ui.current_page_index + 1).min(total_pages.max(1));
            let dest_display = ui.destination().display().to_string();

            let mode_crop = weak.clone();
            let mode_paper = weak.clone();
            let toggle_open = weak.clone();
            let pick_dest = weak.clone();
            let cancel = weak.clone();
            let submit = weak.clone();
            let closed = weak.clone();

            let mode_tabs = gpui::div()
                .flex()
                .items_center()
                .gap_2()
                .child(
                    Button::new("pagebox-mode-crop")
                        .ghost()
                        .small()
                        .selected(ui.mode == PageBoxMode::Crop)
                        .label("Potong Margin (Crop)")
                        .on_click(move |_, window, cx| {
                            let _ = mode_crop.update(cx, |this, cx| {
                                if let Some(d) = &mut this.page_box.dialog {
                                    d.mode = PageBoxMode::Crop;
                                    d.error = None;
                                }
                                window.refresh();
                                cx.notify();
                            });
                        }),
                )
                .child(
                    Button::new("pagebox-mode-paper")
                        .ghost()
                        .small()
                        .selected(ui.mode == PageBoxMode::StandardizePaper)
                        .label("Ukuran Kertas (A4 / Letter / F4 / Legal)")
                        .on_click(move |_, window, cx| {
                            let _ = mode_paper.update(cx, |this, cx| {
                                if let Some(d) = &mut this.page_box.dialog {
                                    d.mode = PageBoxMode::StandardizePaper;
                                    d.error = None;
                                }
                                window.refresh();
                                cx.notify();
                            });
                        }),
                );

            let mode_section = if ui.mode == PageBoxMode::Crop {
                let unit_label = if ui.unit == CropUnit::Mm { "mm" } else { "pt" };
                let mut preset_row = gpui::div().flex().flex_wrap().items_center().gap_1p5();
                let presets: [(&str, &str); 3] = if ui.unit == CropUnit::Mm {
                    [("Tipis (10 mm)", "10"), ("Sedang (20 mm)", "20"), ("Besar (30 mm)", "30")]
                } else {
                    [("Tipis (28 pt)", "28"), ("Sedang (56 pt)", "56"), ("Besar (85 pt)", "85")]
                };
                for (idx, (label, val)) in presets.into_iter().enumerate() {
                    let pick = weak.clone();
                    let val_str = val.to_owned();
                    preset_row = preset_row.child(
                        Button::new(("crop-preset", idx))
                            .ghost()
                            .small()
                            .label(label)
                            .on_click(move |_, window, cx| {
                                let v = val_str.clone();
                                let _ = pick.update(cx, |this, cx| {
                                    if let Some(d) = &mut this.page_box.dialog {
                                        for field in [&d.top, &d.bottom, &d.left, &d.right] {
                                            field.update(cx, |s, cx| s.set_value(v.clone(), window, cx));
                                        }
                                        d.error = None;
                                    }
                                    window.refresh();
                                    cx.notify();
                                });
                            }),
                    );
                }

                let unit_mm = weak.clone();
                let unit_pt = weak.clone();
                preset_row = preset_row
                    .child(gpui::div().flex_1())
                    .child(
                        Button::new("crop-unit-mm")
                            .ghost()
                            .small()
                            .selected(ui.unit == CropUnit::Mm)
                            .label("mm")
                            .on_click(move |_, window, cx| {
                                let _ = unit_mm.update(cx, |this, cx| {
                                    if let Some(d) = &mut this.page_box.dialog {
                                        d.unit = CropUnit::Mm;
                                        d.error = None;
                                    }
                                    window.refresh();
                                    cx.notify();
                                });
                            }),
                    )
                    .child(
                        Button::new("crop-unit-pt")
                            .ghost()
                            .small()
                            .selected(ui.unit == CropUnit::Pt)
                            .label("pt")
                            .on_click(move |_, window, cx| {
                                let _ = unit_pt.update(cx, |this, cx| {
                                    if let Some(d) = &mut this.page_box.dialog {
                                        d.unit = CropUnit::Pt;
                                        d.error = None;
                                    }
                                    window.refresh();
                                    cx.notify();
                                });
                            }),
                    );

                let field_box = |label: String, state: &Entity<InputState>| {
                    gpui::div()
                        .flex_1()
                        .flex()
                        .flex_col()
                        .gap_1()
                        .child(gpui::div().text_xs().text_color(colors.muted_foreground).child(label))
                        .child(Input::new(state).small())
                };

                gpui::div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(
                        gpui::div()
                            .text_xs()
                            .font_semibold()
                            .child("Preset & satuan margin"),
                    )
                    .child(preset_row)
                    .child(
                        gpui::div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .child(field_box(format!("Atas ({unit_label})"), &ui.top))
                            .child(field_box(format!("Bawah ({unit_label})"), &ui.bottom))
                            .child(field_box(format!("Kiri ({unit_label})"), &ui.left))
                            .child(field_box(format!("Kanan ({unit_label})"), &ui.right)),
                    )
                    .child(
                        gpui::div()
                            .text_xs()
                            .text_color(colors.muted_foreground)
                            .child("Crop margin 100% lossless (vektor & teks tetap tajam; rotasi halaman diperhitungkan otomatis)."),
                    )
            } else {
                let mut paper_row = gpui::div().flex().flex_wrap().items_center().gap_1p5();
                for (idx, paper) in [
                    PaperSize::A4,
                    PaperSize::Letter,
                    PaperSize::F4,
                    PaperSize::Legal,
                ]
                .into_iter()
                .enumerate()
                {
                    let pick = weak.clone();
                    paper_row = paper_row.child(
                        Button::new(("paper-size", idx))
                            .ghost()
                            .small()
                            .selected(ui.paper == paper)
                            .label(paper.label())
                            .on_click(move |_, window, cx| {
                                let _ = pick.update(cx, |this, cx| {
                                    if let Some(d) = &mut this.page_box.dialog {
                                        d.paper = paper;
                                        d.error = None;
                                    }
                                    window.refresh();
                                    cx.notify();
                                });
                            }),
                    );
                }

                let mut orient_row = gpui::div().flex().flex_wrap().items_center().gap_1p5();
                for (idx, orient) in [
                    PaperOrientation::Auto,
                    PaperOrientation::Portrait,
                    PaperOrientation::Landscape,
                ]
                .into_iter()
                .enumerate()
                {
                    let pick = weak.clone();
                    orient_row = orient_row.child(
                        Button::new(("paper-orient", idx))
                            .ghost()
                            .small()
                            .selected(ui.orientation == orient)
                            .label(orient.label())
                            .on_click(move |_, window, cx| {
                                let _ = pick.update(cx, |this, cx| {
                                    if let Some(d) = &mut this.page_box.dialog {
                                        d.orientation = orient;
                                        d.error = None;
                                    }
                                    window.refresh();
                                    cx.notify();
                                });
                            }),
                    );
                }

                gpui::div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(gpui::div().text_xs().font_semibold().child("Target ukuran kertas"))
                    .child(paper_row)
                    .child(gpui::div().text_xs().font_semibold().child("Orientasi"))
                    .child(orient_row)
                    .child(
                        gpui::div()
                            .text_xs()
                            .text_color(colors.muted_foreground)
                            .child("Konten diskalakan proporsional & dipusatkan tanpa distorsi. Halaman yang sudah sesuai tetap vektor asli."),
                    )
            };

            let scope_all = weak.clone();
            let scope_cur = weak.clone();
            let scope_rng = weak.clone();
            let mut scope_section = gpui::div()
                .flex()
                .flex_col()
                .gap_1p5()
                .child(gpui::div().text_xs().font_semibold().child("Cakupan halaman"))
                .child(
                    gpui::div()
                        .flex()
                        .flex_wrap()
                        .items_center()
                        .gap_1p5()
                        .child(
                            Button::new("pagebox-scope-all")
                                .ghost()
                                .small()
                                .selected(ui.scope == PageTargetScope::All)
                                .label(format!("Semua ({total_pages} hal.)"))
                                .on_click(move |_, window, cx| {
                                    let _ = scope_all.update(cx, |this, cx| {
                                        if let Some(d) = &mut this.page_box.dialog {
                                            d.scope = PageTargetScope::All;
                                            d.error = None;
                                        }
                                        window.refresh();
                                        cx.notify();
                                    });
                                }),
                        )
                        .child(
                            Button::new("pagebox-scope-current")
                                .ghost()
                                .small()
                                .selected(ui.scope == PageTargetScope::Current)
                                .label(format!("Halaman aktif (Hal. {current_one_based})"))
                                .on_click(move |_, window, cx| {
                                    let _ = scope_cur.update(cx, |this, cx| {
                                        if let Some(d) = &mut this.page_box.dialog {
                                            d.scope = PageTargetScope::Current;
                                            d.error = None;
                                        }
                                        window.refresh();
                                        cx.notify();
                                    });
                                }),
                        )
                        .child(
                            Button::new("pagebox-scope-range")
                                .ghost()
                                .small()
                                .selected(ui.scope == PageTargetScope::Range)
                                .label("Rentang halaman")
                                .on_click(move |_, window, cx| {
                                    let _ = scope_rng.update(cx, |this, cx| {
                                        if let Some(d) = &mut this.page_box.dialog {
                                            d.scope = PageTargetScope::Range;
                                            d.error = None;
                                        }
                                        window.refresh();
                                        cx.notify();
                                    });
                                }),
                        ),
                );
            if ui.scope == PageTargetScope::Range {
                scope_section = scope_section.child(Input::new(&ui.range_input).small());
            }

            let mut body = gpui::div()
                .flex()
                .flex_col()
                .gap_3()
                .child(mode_tabs)
                .child(mode_section)
                .child(scope_section)
                .child(
                    Button::new("pagebox-toggle-open")
                        .ghost()
                        .small()
                        .selected(ui.open_after)
                        .label(if ui.open_after {
                            "✓ Buka hasil PDF di tab baru"
                        } else {
                            "○ Buka hasil PDF di tab baru"
                        })
                        .on_click(move |_, window, cx| {
                            let _ = toggle_open.update(cx, |this, cx| {
                                if let Some(d) = &mut this.page_box.dialog {
                                    d.open_after = !d.open_after;
                                }
                                window.refresh();
                                cx.notify();
                            });
                        }),
                )
                .child(
                    gpui::div()
                        .flex()
                        .flex_col()
                        .gap_1()
                        .child(gpui::div().text_xs().font_semibold().child("Lokasi simpan PDF baru"))
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
                                        .child(dest_display),
                                )
                                .child(
                                    Button::new("pagebox-pick-dest")
                                        .ghost()
                                        .small()
                                        .disabled(ui.picking)
                                        .label("Ubah lokasi…")
                                        .on_click(move |_, window, cx| {
                                            let _ = pick_dest.update(cx, |this, cx| {
                                                this.pick_page_box_destination(window, cx);
                                            });
                                        }),
                                ),
                        ),
                );

            if let Some(err) = &ui.error {
                body = body.child(
                    gpui::div()
                        .px_2p5()
                        .py_1p5()
                        .rounded_md()
                        .bg(gpui::rgba(0xdc262618))
                        .text_xs()
                        .text_color(gpui::rgb(0xdc2626))
                        .child(err.clone()),
                );
            }

            let submit_label = if ui.mode == PageBoxMode::Crop {
                "Potong Margin & Simpan"
            } else {
                "Standardisasi Kertas & Simpan"
            };

            dialog
                .w(px(580.))
                .title("Crop Halaman & Standardisasi Ukuran Kertas")
                .description(
                    "Potong margin halaman secara lossless atau seragamkan ukuran kertas (A4, Letter, F4, Legal) ke file PDF baru.",
                )
                .child(body)
                .on_close(move |_, _, cx| {
                    let _ = closed.update(cx, |this, cx| this.close_page_box_dialog(cx));
                })
                .footer(
                    DialogFooter::new()
                        .child(
                            Button::new("pagebox-cancel")
                                .ghost()
                                .label("Batal")
                                .on_click(move |_, window, cx| {
                                    window.close_dialog(cx);
                                    let _ = cancel.update(cx, |this, cx| {
                                        this.close_page_box_dialog(cx);
                                    });
                                }),
                        )
                        .child(
                            Button::new("pagebox-submit")
                                .primary()
                                .disabled(ui.picking)
                                .label(submit_label)
                                .on_click(move |_, window, cx| {
                                    let _ = submit.update(cx, |this, cx| {
                                        this.submit_page_box(window, cx);
                                    });
                                }),
                        ),
                )
        });
    }

    fn close_page_box_dialog(&mut self, cx: &mut Context<Self>) {
        self.page_box.dialog = None;
        let _ = self.resources.modal_owner.compare_exchange(
            SHELL_EXPORT_OWNER,
            0,
            Ordering::SeqCst,
            Ordering::SeqCst,
        );
        self.report(cx);
        cx.notify();
    }

    fn pick_page_box_destination(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(dialog) = &mut self.page_box.dialog else {
            return;
        };
        if dialog.picking {
            return;
        }
        dialog.picking = true;
        let default_dest = dialog.destination();
        let dir = default_dest
            .parent()
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("."));
        let name = default_dest
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("dokumen-cropped.pdf")
            .to_owned();
        let rx = cx.prompt_for_new_path(&dir, Some(&name));
        let handle = window.window_handle();
        cx.spawn(async move |this, cx| {
            let picked = rx.await.ok().and_then(Result::ok).flatten();
            let _ = handle.update(cx, |_, window, cx| {
                let _ = this.update(cx, |this, cx| {
                    if let Some(d) = &mut this.page_box.dialog {
                        d.picking = false;
                        if let Some(mut path) = picked {
                            if path
                                .extension()
                                .and_then(|e| e.to_str())
                                .is_none_or(|e| !e.eq_ignore_ascii_case("pdf"))
                            {
                                path.set_extension("pdf");
                            }
                            d.custom_destination = Some(path);
                            d.error = None;
                        }
                    }
                    window.refresh();
                    cx.notify();
                });
            });
        })
        .detach();
    }

    fn submit_page_box(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(ui) = self.page_box.dialog.clone() else {
            return;
        };
        let total = ui.snapshot.pages().len();
        let pages: Vec<usize> = match ui.scope {
            PageTargetScope::All => Vec::new(),
            PageTargetScope::Current => vec![ui.current_page_index.min(total.saturating_sub(1))],
            PageTargetScope::Range => {
                let raw = ui.range_input.read(cx).value().to_string();
                match parse_page_selection(&raw, total) {
                    Ok(indices) => indices,
                    Err(msg) => {
                        if let Some(d) = &mut self.page_box.dialog {
                            d.error = Some(msg.into());
                        }
                        window.refresh();
                        cx.notify();
                        return;
                    }
                }
            }
        };

        let operation = match ui.mode {
            PageBoxMode::Crop => {
                let top = match parse_margin_value(&ui.top.read(cx).value()) {
                    Ok(v) => v,
                    Err(msg) => {
                        if let Some(d) = &mut self.page_box.dialog {
                            d.error = Some(msg.into());
                        }
                        window.refresh();
                        cx.notify();
                        return;
                    }
                };
                let bottom = match parse_margin_value(&ui.bottom.read(cx).value()) {
                    Ok(v) => v,
                    Err(msg) => {
                        if let Some(d) = &mut self.page_box.dialog {
                            d.error = Some(msg.into());
                        }
                        window.refresh();
                        cx.notify();
                        return;
                    }
                };
                let left = match parse_margin_value(&ui.left.read(cx).value()) {
                    Ok(v) => v,
                    Err(msg) => {
                        if let Some(d) = &mut self.page_box.dialog {
                            d.error = Some(msg.into());
                        }
                        window.refresh();
                        cx.notify();
                        return;
                    }
                };
                let right = match parse_margin_value(&ui.right.read(cx).value()) {
                    Ok(v) => v,
                    Err(msg) => {
                        if let Some(d) = &mut self.page_box.dialog {
                            d.error = Some(msg.into());
                        }
                        window.refresh();
                        cx.notify();
                        return;
                    }
                };
                let margins = match ui.unit {
                    CropUnit::Mm => CropMargins::from_mm(top, bottom, left, right),
                    CropUnit::Pt => CropMargins {
                        top_pt: top,
                        bottom_pt: bottom,
                        left_pt: left,
                        right_pt: right,
                    },
                };
                if !margins.valid() {
                    if let Some(d) = &mut self.page_box.dialog {
                        d.error = Some("Masukkan minimal satu nilai margin lebih dari 0.".into());
                    }
                    window.refresh();
                    cx.notify();
                    return;
                }
                PageBoxOperation::Crop { margins, pages }
            }
            PageBoxMode::StandardizePaper => PageBoxOperation::StandardizePaper {
                paper: ui.paper,
                orientation: ui.orientation,
                pages,
            },
        };

        let destination = ui.destination();
        if destination.exists() {
            if let Some(d) = &mut self.page_box.dialog {
                d.error = Some(
                    "File tujuan sudah ada. Klik 'Ubah lokasi…' untuk memilih nama file baru."
                        .into(),
                );
            }
            window.refresh();
            cx.notify();
            return;
        }
        if self
            .resources
            .export_owner
            .compare_exchange(0, SHELL_EXPORT_OWNER, Ordering::SeqCst, Ordering::SeqCst)
            .is_err()
        {
            if let Some(d) = &mut self.page_box.dialog {
                d.error = Some("Ada proses ekspor lain yang sedang berjalan.".into());
            }
            window.refresh();
            cx.notify();
            return;
        }

        window.close_dialog(cx);
        self.close_page_box_dialog(cx);

        self.page_box.job_seq += 1;
        let job_id = self.page_box.job_seq;
        let done = Arc::new(AtomicUsize::new(0));
        let control = ExportControl::default();
        let mode = ui.mode;
        let open_after = ui.open_after;
        let plan = ui.snapshot;
        let sidecars = self.sidecars.clone();
        let renderer = self.renderer.clone();
        let inspector = self.renderer.clone();

        self.page_box.job = Some(PageBoxJob {
            mode,
            total,
            done: done.clone(),
            control: control.clone(),
            open_after,
            job_id,
        });
        cx.notify();

        let poll_done = done.clone();
        let (tx, rx) = std::sync::mpsc::sync_channel::<Result<PageBoxReport, String>>(1);
        std::thread::spawn(move || {
            let tool = QpdfPageBox::new(&sidecars);
            let result = tool
                .apply(
                    &plan,
                    &destination,
                    &operation,
                    &renderer,
                    &inspector,
                    &control,
                    &mut |count| {
                        poll_done.store(count, Ordering::SeqCst);
                    },
                )
                .map_err(|err| err.to_string());
            let _ = tx.send(result);
        });

        cx.spawn(async move |this, cx| {
            loop {
                match rx.try_recv() {
                    Ok(result) => {
                        let _ = this.update(cx, |this, cx| {
                            this.finish_page_box_job(job_id, result, cx);
                        });
                        break;
                    }
                    Err(std::sync::mpsc::TryRecvError::Empty) => {
                        cx.background_executor()
                            .timer(std::time::Duration::from_millis(80))
                            .await;
                        let _ = this.update(cx, |_, cx| cx.notify());
                    }
                    Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                        let _ = this.update(cx, |this, cx| {
                            this.finish_page_box_job(
                                job_id,
                                Err("Proses ukuran halaman berhenti tak terduga.".into()),
                                cx,
                            );
                        });
                        break;
                    }
                }
            }
        })
        .detach();
    }

    pub(super) fn cancel_page_box_job(&mut self, cx: &mut Context<Self>) {
        if let Some(job) = &self.page_box.job {
            job.control.cancel();
            cx.notify();
        }
    }

    fn finish_page_box_job(
        &mut self,
        job_id: u64,
        result: Result<PageBoxReport, String>,
        cx: &mut Context<Self>,
    ) {
        let Some(job) = self.page_box.job.take() else {
            return;
        };
        if job.job_id != job_id {
            self.page_box.job = Some(job);
            return;
        }
        let _ = self.resources.export_owner.compare_exchange(
            SHELL_EXPORT_OWNER,
            0,
            Ordering::SeqCst,
            Ordering::SeqCst,
        );
        match result {
            Ok(report) => {
                let name = report
                    .output
                    .file_name()
                    .and_then(|s| s.to_str())
                    .unwrap_or("hasil.pdf");
                let msg = match job.mode {
                    PageBoxMode::Crop => format!(
                        "Crop halaman selesai: {} dari {} halaman dipotong → {}",
                        report.modified_pages, report.page_count, name
                    ),
                    PageBoxMode::StandardizePaper => format!(
                        "Standardisasi kertas selesai: {} halaman disesuaikan (total {} hal.) → {}",
                        report.modified_pages, report.page_count, name
                    ),
                };
                self.notice = Some(msg);
                if job.open_after && self.tabs.len() < MAX_TABS {
                    self.open_paths(vec![report.output], cx);
                }
            }
            Err(err) => {
                self.notice = Some(format!("Gagal memproses ukuran halaman: {err}"));
            }
        }
        if self.close.is_some() {
            self.advance_close(cx);
        }
        self.report(cx);
        cx.notify();
    }
}
