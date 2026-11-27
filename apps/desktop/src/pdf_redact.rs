//! Interactive True Redaction marking mode on DocumentSession + confirmation dialog
//! and background QPDF/PDFium execution on WindowShell.
use crate::images_to_pdf::SHELL_EXPORT_OWNER;
use crate::shell::{MAX_TABS, WindowShell};
use crate::*;
use gpui::component::{
    ActiveTheme, Disableable, Selectable, Sizable, StyledExt, WindowExt,
    button::{Button, ButtonVariants},
    dialog::DialogFooter,
};
use gpui_kit::assets::IconName;
use pdf_application::export::{ExportControl, ExportPlan, ExportScope};
use pdf_domain::{PageId, annotation::rotate};
use qpdf_adapter::{QpdfRedactor, RedactDpi, RedactOptions, RedactRect, RedactReport};
use std::{
    collections::{BTreeMap, HashMap},
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};

const REDACT_FILL_PRESETS: [(&str, [u8; 3]); 3] = [
    ("Hitam", [0, 0, 0]),
    ("Putih", [255, 255, 255]),
    ("Abu Gelap", [38, 38, 42]),
];

#[derive(Default, Clone)]
pub(super) struct RedactionMarksState {
    pub active: bool,
    pub boxes: HashMap<PageId, Vec<RedactRect>>,
    pub history: Vec<PageId>,
    pub draft: Option<(PageId, [f32; 2], [f32; 2])>,
}

impl RedactionMarksState {
    pub(super) fn total_boxes(&self) -> usize {
        self.boxes.values().map(Vec::len).sum()
    }

    pub(super) fn page_count(&self) -> usize {
        self.boxes.values().filter(|v| !v.is_empty()).count()
    }

    pub(super) fn clear(&mut self) {
        self.boxes.clear();
        self.history.clear();
        self.draft = None;
    }

    pub(super) fn undo_last(&mut self) -> bool {
        while let Some(page_id) = self.history.pop() {
            if let Some(list) = self.boxes.get_mut(&page_id)
                && list.pop().is_some()
            {
                if list.is_empty() {
                    self.boxes.remove(&page_id);
                }
                return true;
            }
        }
        false
    }

    pub(super) fn map_for_plan(&self, plan: &ExportPlan) -> BTreeMap<usize, Vec<RedactRect>> {
        let mut out = BTreeMap::new();
        for (index, page) in plan.pages().iter().enumerate() {
            if let Some(list) = self.boxes.get(&page.entry.id)
                && !list.is_empty()
            {
                out.insert(index, list.clone());
            }
        }
        out
    }
}

pub(super) fn paint_redaction_overlays(
    boxes: &[RedactRect],
    draft: Option<RedactRect>,
    quarter_turns: u8,
    bounds: Bounds<gpui::Pixels>,
    window: &mut Window,
) {
    let paint_box = |rect: RedactRect, is_draft: bool, window: &mut Window| {
        let [left, top, right, bottom] = rect.visual_bounds(quarter_turns);
        let box_bounds = Bounds::new(
            bounds.origin + gpui::point(bounds.size.width * left, bounds.size.height * top),
            gpui::size(
                bounds.size.width * (right - left),
                bounds.size.height * (bottom - top),
            ),
        );
        let fill_color = if is_draft {
            gpui::rgba(0xdc262638)
        } else {
            gpui::rgba(0xdc262666)
        };
        let border_color = gpui::rgba(0xdc2626ff);
        window.paint_quad(gpui::quad(
            box_bounds,
            gpui::Corners::default(),
            fill_color,
            gpui::Edges::all(px(1.5)),
            border_color,
            gpui::BorderStyle::Solid,
        ));
        // Diagonal cross indicator inside marked redaction boxes so they look distinct from shapes.
        if !is_draft && box_bounds.size.width >= px(10.) && box_bounds.size.height >= px(10.) {
            let mut p1 = gpui::PathBuilder::stroke(px(1.0));
            p1.move_to(box_bounds.origin);
            p1.line_to(
                box_bounds.origin + gpui::point(box_bounds.size.width, box_bounds.size.height),
            );
            if let Ok(path) = p1.build() {
                window.paint_path(path, gpui::rgba(0xdc2626b0));
            }
            let mut p2 = gpui::PathBuilder::stroke(px(1.0));
            p2.move_to(box_bounds.origin + gpui::point(box_bounds.size.width, px(0.)));
            p2.line_to(box_bounds.origin + gpui::point(px(0.), box_bounds.size.height));
            if let Ok(path) = p2.build() {
                window.paint_path(path, gpui::rgba(0xdc2626b0));
            }
        }
    };

    for rect in boxes {
        paint_box(*rect, false, window);
    }
    if let Some(rect) = draft {
        paint_box(rect, true, window);
    }
}

impl DocumentSession {
    pub(super) fn enter_redaction_mode(&mut self, cx: &mut Context<Self>) {
        if self.edits_locked() || self.workspace.is_none() {
            return;
        }
        self.close_annotation_popups(cx);
        self.annotation.draft = None;
        self.annotation.selected = None;
        self.annotation.tool = annotations::Tool::Select;
        self.redaction.active = true;
        self.status =
            "Mode Sensor Permanen: seret kotak di area sensitif, atau klik kotak untuk menghapus."
                .into();
        cx.notify();
    }

    pub(super) fn exit_redaction_mode(&mut self, cx: &mut Context<Self>) {
        self.redaction.active = false;
        self.redaction.draft = None;
        self.update_status();
        cx.notify();
    }

    pub(super) fn toggle_redaction_mode(&mut self, cx: &mut Context<Self>) {
        if self.redaction.active {
            self.exit_redaction_mode(cx);
        } else {
            self.enter_redaction_mode(cx);
        }
    }

    pub(super) fn clear_redaction_marks(&mut self, cx: &mut Context<Self>) {
        self.redaction.clear();
        self.redaction.active = false;
        self.update_status();
        cx.notify();
    }

    pub(super) fn redaction_banner(&mut self, cx: &mut Context<Self>) -> Option<gpui::AnyElement> {
        if !self.redaction.active || self.workspace.is_none() || self.split_presentation {
            return None;
        }
        let total = self.redaction.total_boxes();
        let pages = self.redaction.page_count();
        let summary = if total == 0 {
            "Seret kotak pada teks/gambar sensitif untuk ditandai. Klik kotak merah untuk menghapus.".to_owned()
        } else {
            format!(
                "{total} area ditandai di {pages} halaman — klik kotak untuk menghapus, lalu Terapkan."
            )
        };
        let owner = self.owner.clone();
        Some(
            gpui::div()
                .id("redaction-banner")
                .flex()
                .flex_wrap()
                .items_center()
                .gap_2()
                .px_3()
                .py_1p5()
                .border_b_1()
                .border_color(gpui::rgba(0xdc262655))
                .bg(gpui::rgba(0xdc262614))
                .child(
                    gpui::component::Icon::new(IconName::EyeOff)
                        .size(px(15.))
                        .text_color(gpui::rgb(0xdc2626)),
                )
                .child(
                    gpui::div()
                        .text_xs()
                        .font_semibold()
                        .text_color(gpui::rgb(0xdc2626))
                        .child("Mode Sensor Permanen"),
                )
                .child(gpui::div().flex_1().text_xs().child(summary))
                .child(
                    Button::new("redaction-undo")
                        .ghost()
                        .small()
                        .label("Hapus Terakhir")
                        .disabled(total == 0)
                        .on_click(cx.listener(|this, _, _, cx| {
                            if this.redaction.undo_last() {
                                cx.notify();
                            }
                        })),
                )
                .child(
                    Button::new("redaction-clear")
                        .ghost()
                        .small()
                        .label("Bersihkan")
                        .disabled(total == 0)
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.redaction.clear();
                            cx.notify();
                        })),
                )
                .child(
                    Button::new("redaction-apply")
                        .danger()
                        .small()
                        .label("Terapkan Sensor Permanen…")
                        .on_click(move |_, window, cx| {
                            let _ = owner.update(cx, |shell, cx| shell.show_redact_pdf(window, cx));
                        }),
                )
                .child(
                    Button::new("redaction-done")
                        .ghost()
                        .small()
                        .label("Selesai")
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.exit_redaction_mode(cx);
                        })),
                )
                .into_any_element(),
        )
    }

    pub(super) fn redaction_pointer(
        &mut self,
        index: usize,
        bounds: Bounds<gpui::Pixels>,
        position: gpui::Point<gpui::Pixels>,
        down: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        if !self.redaction.active || self.mode != ViewerMode::Read || self.edits_locked() {
            return false;
        }
        let Some(workspace) = self.workspace.as_ref() else {
            return false;
        };
        let (Some(page), Ok(geometry)) = (workspace.pages().get(index), workspace.geometry(index))
        else {
            return false;
        };
        let page_id = page.id;
        let width = f32::from(bounds.size.width);
        let height = f32::from(bounds.size.height);
        if width <= 0. || height <= 0. {
            return false;
        }
        let vx = ((f32::from(position.x - bounds.origin.x)) / width).clamp(0.0, 1.0);
        let vy = ((f32::from(position.y - bounds.origin.y)) / height).clamp(0.0, 1.0);
        let unrotated = rotate([vx, vy], (4 - geometry.quarter_turns % 4) % 4);

        if down {
            self.focus.focus(window, cx);
            // Clicking inside an existing marked box deletes it directly.
            if let Some(list) = self.redaction.boxes.get_mut(&page_id)
                && let Some(pos) = list.iter().rposition(|r| {
                    unrotated[0] >= r.left
                        && unrotated[0] <= r.right
                        && unrotated[1] >= r.top
                        && unrotated[1] <= r.bottom
                })
            {
                list.remove(pos);
                if list.is_empty() {
                    self.redaction.boxes.remove(&page_id);
                }
                cx.notify();
                return true;
            }
            self.redaction.draft = Some((page_id, unrotated, unrotated));
            cx.notify();
            return true;
        }

        if let Some((draft_page, start, end)) = &mut self.redaction.draft
            && *draft_page == page_id
        {
            *end = unrotated;
            let _ = start;
            cx.notify();
            return true;
        }
        false
    }

    pub(super) fn finish_redaction_drag(&mut self, cx: &mut Context<Self>) {
        if let Some((page_id, start, end)) = self.redaction.draft.take() {
            if let Some(rect) = RedactRect::from_corners(start, end) {
                self.redaction.boxes.entry(page_id).or_default().push(rect);
                self.redaction.history.push(page_id);
            }
            cx.notify();
        }
    }
}

#[derive(Clone)]
pub(super) struct RedactDialog {
    pub snapshot: ExportPlan,
    pub source_path: PathBuf,
    pub boxes_by_page: BTreeMap<usize, Vec<RedactRect>>,
    pub fill_index: usize,
    pub dpi: RedactDpi,
    pub sanitize_metadata: bool,
    pub open_after: bool,
    pub custom_destination: Option<PathBuf>,
    pub picking: bool,
    pub error: Option<String>,
}

impl RedactDialog {
    fn total_boxes(&self) -> usize {
        self.boxes_by_page.values().map(Vec::len).sum()
    }

    fn redacted_pages(&self) -> usize {
        self.boxes_by_page
            .values()
            .filter(|v| !v.is_empty())
            .count()
    }

    fn destination(&self) -> PathBuf {
        if let Some(custom) = &self.custom_destination {
            custom.clone()
        } else {
            let suffix = if self.total_boxes() > 0 {
                "redacted"
            } else {
                "sanitized"
            };
            suggested_redact_destination(&self.source_path, suffix)
        }
    }
}

pub(super) struct RedactJob {
    pub total: usize,
    pub done: Arc<AtomicUsize>,
    pub control: ExportControl,
    pub open_after: bool,
    pub job_id: u64,
    pub source_session: gpui::WeakEntity<DocumentSession>,
}

#[derive(Default)]
pub(super) struct RedactState {
    pub dialog: Option<RedactDialog>,
    pub job: Option<RedactJob>,
    pub job_seq: u64,
}

impl RedactState {
    pub(super) fn status_label(&self) -> String {
        let Some(job) = &self.job else {
            return String::new();
        };
        let done = job.done.load(Ordering::SeqCst);
        format!("Menerapkan Sensor Permanen: {done}/{} halaman…", job.total)
    }
}

fn suggested_redact_destination(source: &std::path::Path, suffix: &str) -> PathBuf {
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

impl WindowShell {
    pub(super) fn show_redact_pdf(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.close.is_some()
            || self.modal(cx)
            || self.resources.export_owner.load(Ordering::SeqCst) != 0
            || self.redact.job.is_some()
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
            ExportPlan::capture(&workspace, s.workspace_instance, ExportScope::All).map(|plan| {
                let mapped = s.redaction.map_for_plan(&plan);
                (plan, mapped)
            })
        };
        let (snapshot, boxes_by_page) = match captured {
            Ok(pair) => pair,
            Err(err) => {
                self.notice = Some(format!("Dokumen belum dapat disensor: {err}"));
                cx.notify();
                return;
            }
        };

        self.redact.dialog = Some(RedactDialog {
            snapshot,
            source_path,
            boxes_by_page,
            fill_index: 0,
            dpi: RedactDpi::Dpi200,
            sanitize_metadata: true,
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
                .read_with(cx, |this, _| this.redact.dialog.clone())
                .ok()
                .flatten()
            else {
                return dialog;
            };
            let colors = cx.theme().colors;
            let total_boxes = ui.total_boxes();
            let redacted_pages = ui.redacted_pages();
            let total_pages = ui.snapshot.pages().len();
            let dest_display = ui.destination().display().to_string();

            let mark_on_page = weak.clone();
            let pick_dest = weak.clone();
            let toggle_sanitize = weak.clone();
            let toggle_open = weak.clone();
            let cancel = weak.clone();
            let submit = weak.clone();
            let closed = weak.clone();

            let summary_box = gpui::div()
                .flex()
                .items_center()
                .justify_between()
                .gap_2()
                .px_3()
                .py_2()
                .rounded_md()
                .border_1()
                .border_color(if total_boxes > 0 {
                    gpui::rgba(0xdc262666).into()
                } else {
                    colors.border
                })
                .bg(if total_boxes > 0 {
                    gpui::rgba(0xdc262612).into()
                } else {
                    colors.muted
                })
                .child(
                    gpui::div()
                        .flex()
                        .flex_col()
                        .gap_0p5()
                        .child(
                            gpui::div()
                                .text_xs()
                                .font_semibold()
                                .child(if total_boxes > 0 {
                                    format!(
                                        "{total_boxes} area sensor ditandai di {redacted_pages} dari {total_pages} halaman"
                                    )
                                } else {
                                    "Belum ada area kotak sensor ditandai".to_owned()
                                }),
                        )
                        .child(
                            gpui::div()
                                .text_xs()
                                .text_color(colors.muted_foreground)
                                .child(if total_boxes > 0 {
                                    "Halaman bertanda dirasterisasi permanen; halaman lain tetap vektor asli."
                                } else {
                                    "Tandai area di halaman terlebih dahulu, atau lanjutkan untuk membersihkan metadata saja."
                                }),
                        ),
                )
                .child(
                    Button::new("redact-mark-on-page")
                        .ghost()
                        .small()
                        .icon(IconName::EyeOff)
                        .label(if total_boxes > 0 {
                            "Edit Tanda"
                        } else {
                            "Tandai Area"
                        })
                        .on_click(move |_, window, cx| {
                            window.close_dialog(cx);
                            let _ = mark_on_page.update(cx, |this, cx| {
                                this.close_redact_dialog(cx);
                                if let Some(session) = this.session() {
                                    session.update(cx, |s, cx| s.enter_redaction_mode(cx));
                                }
                            });
                        }),
                );

            let mut color_row = gpui::div().flex().items_center().gap_2();
            for (idx, (label, rgb)) in REDACT_FILL_PRESETS.iter().enumerate() {
                let pick = weak.clone();
                let swatch = gpui::rgb(
                    (u32::from(rgb[0]) << 16) | (u32::from(rgb[1]) << 8) | u32::from(rgb[2]),
                );
                color_row = color_row.child(
                    Button::new(("redact-fill", idx))
                        .ghost()
                        .small()
                        .selected(ui.fill_index == idx)
                        .disabled(total_boxes == 0)
                        .child(
                            gpui::div()
                                .flex()
                                .items_center()
                                .gap_1p5()
                                .child(
                                    gpui::div()
                                        .w(px(12.))
                                        .h(px(12.))
                                        .rounded_sm()
                                        .border_1()
                                        .border_color(colors.border)
                                        .bg(swatch),
                                )
                                .child(*label),
                        )
                        .on_click(move |_, window, cx| {
                            let _ = pick.update(cx, |this, cx| {
                                if let Some(d) = &mut this.redact.dialog {
                                    d.fill_index = idx;
                                    d.error = None;
                                }
                                window.refresh();
                                cx.notify();
                            });
                        }),
                );
            }

            let mut dpi_row = gpui::div().flex().items_center().gap_2();
            for (idx, dpi_opt) in [RedactDpi::Dpi200, RedactDpi::Dpi300].into_iter().enumerate() {
                let pick = weak.clone();
                dpi_row = dpi_row.child(
                    Button::new(("redact-dpi", idx))
                        .ghost()
                        .small()
                        .selected(ui.dpi == dpi_opt)
                        .disabled(total_boxes == 0)
                        .label(dpi_opt.label())
                        .on_click(move |_, window, cx| {
                            let _ = pick.update(cx, |this, cx| {
                                if let Some(d) = &mut this.redact.dialog {
                                    d.dpi = dpi_opt;
                                    d.error = None;
                                }
                                window.refresh();
                                cx.notify();
                            });
                        }),
                );
            }

            let mut body = gpui::div()
                .flex()
                .flex_col()
                .gap_3()
                .child(summary_box)
                .child(
                    gpui::div()
                        .flex()
                        .flex_col()
                        .gap_1()
                        .child(
                            gpui::div()
                                .text_xs()
                                .font_semibold()
                                .child("Warna blok sensor"),
                        )
                        .child(color_row),
                )
                .child(
                    gpui::div()
                        .flex()
                        .flex_col()
                        .gap_1()
                        .child(
                            gpui::div()
                                .text_xs()
                                .font_semibold()
                                .child("Resolusi halaman tersensor"),
                        )
                        .child(dpi_row),
                )
                .child(
                    gpui::div()
                        .flex()
                        .flex_col()
                        .gap_1p5()
                        .child(
                            Button::new("redact-toggle-sanitize")
                                .ghost()
                                .small()
                                .selected(ui.sanitize_metadata)
                                .label(if ui.sanitize_metadata {
                                    "✓ Bersihkan semua metadata tersembunyi (Author, Title, Info & XMP)"
                                } else {
                                    "○ Bersihkan semua metadata tersembunyi (Author, Title, Info & XMP)"
                                })
                                .on_click(move |_, window, cx| {
                                    let _ = toggle_sanitize.update(cx, |this, cx| {
                                        if let Some(d) = &mut this.redact.dialog {
                                            d.sanitize_metadata = !d.sanitize_metadata;
                                            d.error = None;
                                        }
                                        window.refresh();
                                        cx.notify();
                                    });
                                }),
                        )
                        .child(
                            Button::new("redact-toggle-open")
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
                                        if let Some(d) = &mut this.redact.dialog {
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
                                    Button::new("redact-pick-dest")
                                        .ghost()
                                        .small()
                                        .disabled(ui.picking)
                                        .label("Ubah lokasi…")
                                        .on_click(move |_, window, cx| {
                                            let _ = pick_dest.update(cx, |this, cx| {
                                                this.pick_redact_destination(window, cx);
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

            let can_run = (total_boxes > 0 || ui.sanitize_metadata) && !ui.picking;
            let submit_label = if total_boxes > 0 {
                "Terapkan Sensor Permanen"
            } else {
                "Bersihkan Metadata & Simpan"
            };

            dialog
                .w(px(560.))
                .title("Sensor Permanen (True Redaction) & Sanitize Metadata")
                .description(
                    "Menghapus teks/gambar sensitif secara permanen ke file PDF baru. File asli tetap utuh.",
                )
                .child(body)
                .on_close(move |_, _, cx| {
                    let _ = closed.update(cx, |this, cx| this.close_redact_dialog(cx));
                })
                .footer(
                    DialogFooter::new()
                        .child(
                            Button::new("redact-cancel")
                                .ghost()
                                .label("Batal")
                                .on_click(move |_, window, cx| {
                                    window.close_dialog(cx);
                                    let _ = cancel.update(cx, |this, cx| {
                                        this.close_redact_dialog(cx);
                                    });
                                }),
                        )
                        .child(
                            Button::new("redact-submit")
                                .danger()
                                .disabled(!can_run)
                                .label(submit_label)
                                .on_click(move |_, window, cx| {
                                    let _ = submit.update(cx, |this, cx| {
                                        this.submit_redact_pdf(window, cx);
                                    });
                                }),
                        ),
                )
        });
    }

    fn close_redact_dialog(&mut self, cx: &mut Context<Self>) {
        self.redact.dialog = None;
        let _ = self.resources.modal_owner.compare_exchange(
            SHELL_EXPORT_OWNER,
            0,
            Ordering::SeqCst,
            Ordering::SeqCst,
        );
        self.report(cx);
        cx.notify();
    }

    fn pick_redact_destination(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(dialog) = &mut self.redact.dialog else {
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
            .unwrap_or("dokumen-redacted.pdf")
            .to_owned();
        let rx = cx.prompt_for_new_path(&dir, Some(&name));
        let handle = window.window_handle();
        cx.spawn(async move |this, cx| {
            let picked = rx.await.ok().and_then(Result::ok).flatten();
            let _ = handle.update(cx, |_, window, cx| {
                let _ = this.update(cx, |this, cx| {
                    if let Some(d) = &mut this.redact.dialog {
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

    fn submit_redact_pdf(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(ui) = self.redact.dialog.clone() else {
            return;
        };
        if ui.total_boxes() == 0 && !ui.sanitize_metadata {
            if let Some(d) = &mut self.redact.dialog {
                d.error =
                    Some("Tandai minimal 1 area sensor atau aktifkan pembersihan metadata.".into());
            }
            window.refresh();
            cx.notify();
            return;
        }
        let destination = ui.destination();
        if destination.exists() {
            if let Some(d) = &mut self.redact.dialog {
                d.error = Some(
                    "File tujuan sudah ada. Klik 'Ubah lokasi…' untuk memilih nama file baru."
                        .into(),
                );
            }
            window.refresh();
            cx.notify();
            return;
        }
        let Some(session) = self.session() else {
            return;
        };
        if self
            .resources
            .export_owner
            .compare_exchange(0, SHELL_EXPORT_OWNER, Ordering::SeqCst, Ordering::SeqCst)
            .is_err()
        {
            if let Some(d) = &mut self.redact.dialog {
                d.error = Some("Ada proses ekspor lain yang sedang berjalan.".into());
            }
            window.refresh();
            cx.notify();
            return;
        }

        window.close_dialog(cx);
        self.close_redact_dialog(cx);

        self.redact.job_seq += 1;
        let job_id = self.redact.job_seq;
        let total = ui.snapshot.pages().len();
        let done = Arc::new(AtomicUsize::new(0));
        let control = ExportControl::default();
        let open_after = ui.open_after;
        let fill_color = REDACT_FILL_PRESETS
            .get(ui.fill_index)
            .map(|(_, c)| *c)
            .unwrap_or([0, 0, 0]);
        let options = RedactOptions {
            boxes_by_page: ui.boxes_by_page,
            fill_color,
            dpi: ui.dpi,
            sanitize_metadata: ui.sanitize_metadata,
        };
        let plan = ui.snapshot;
        let sidecars = self.sidecars.clone();
        let renderer = self.renderer.clone();
        let inspector = self.renderer.clone();
        let source_session = session.downgrade();

        self.redact.job = Some(RedactJob {
            total,
            done: done.clone(),
            control: control.clone(),
            open_after,
            job_id,
            source_session: source_session.clone(),
        });
        cx.notify();

        let poll_done = done.clone();
        let (tx, rx) = std::sync::mpsc::sync_channel::<Result<RedactReport, String>>(1);
        std::thread::spawn(move || {
            let redactor = QpdfRedactor::new(&sidecars);
            let result = redactor
                .redact_pdf(
                    &plan,
                    &destination,
                    &options,
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
                            this.finish_redact_job(job_id, result, cx);
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
                            this.finish_redact_job(
                                job_id,
                                Err("Proses sensor permanen berhenti tak terduga.".into()),
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

    pub(super) fn cancel_redact_job(&mut self, cx: &mut Context<Self>) {
        if let Some(job) = &self.redact.job {
            job.control.cancel();
            cx.notify();
        }
    }

    fn finish_redact_job(
        &mut self,
        job_id: u64,
        result: Result<RedactReport, String>,
        cx: &mut Context<Self>,
    ) {
        let Some(job) = self.redact.job.take() else {
            return;
        };
        if job.job_id != job_id {
            self.redact.job = Some(job);
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
                let _ = job.source_session.update(cx, |s, cx| {
                    s.clear_redaction_marks(cx);
                });
                let name = report
                    .output
                    .file_name()
                    .and_then(|s| s.to_str())
                    .unwrap_or("hasil-redacted.pdf");
                let summary = if report.redacted_boxes > 0 && report.metadata_sanitized {
                    format!(
                        "Sensor permanen selesai: {} area di {} halaman dihapus permanen & metadata dibersihkan → {}",
                        report.redacted_boxes, report.redacted_pages, name
                    )
                } else if report.redacted_boxes > 0 {
                    format!(
                        "Sensor permanen selesai: {} area di {} halaman dihapus permanen → {}",
                        report.redacted_boxes, report.redacted_pages, name
                    )
                } else {
                    format!("Metadata PDF dibersihkan secara permanen → {name}")
                };
                self.notice = Some(summary);
                if job.open_after && self.tabs.len() < MAX_TABS {
                    self.open_paths(vec![report.output], cx);
                }
            }
            Err(err) => {
                self.notice = Some(format!("Sensor permanen gagal: {err}"));
            }
        }
        if self.close.is_some() {
            self.advance_close(cx);
        }
        self.report(cx);
        cx.notify();
    }
}
