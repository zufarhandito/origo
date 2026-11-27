//! Window-level dialogs and background jobs for Flatten PDF and PDF Metadata / Properties editor.
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
    FlattenOptions, FlattenReport, MetadataReport, PdfMetadata, QpdfMetadata, RedactDpi,
};
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

#[derive(Clone)]
pub(super) struct FlattenDialog {
    pub snapshot: ExportPlan,
    pub source_path: PathBuf,
    pub dpi: RedactDpi,
    pub strip_metadata: bool,
    pub open_after: bool,
    pub custom_destination: Option<PathBuf>,
    pub picking: bool,
    pub error: Option<String>,
}

impl FlattenDialog {
    fn destination(&self) -> PathBuf {
        if let Some(custom) = &self.custom_destination {
            custom.clone()
        } else {
            suggested_tool_destination(&self.source_path, "flattened")
        }
    }
}

#[derive(Clone)]
pub(super) struct MetadataDialog {
    pub snapshot: ExportPlan,
    pub source_path: PathBuf,
    pub file_bytes: u64,
    pub loading: bool,
    pub title: Entity<InputState>,
    pub author: Entity<InputState>,
    pub subject: Entity<InputState>,
    pub keywords: Entity<InputState>,
    pub creator: Entity<InputState>,
    pub producer: Entity<InputState>,
    pub strip_xmp: bool,
    pub open_after: bool,
    pub custom_destination: Option<PathBuf>,
    pub picking: bool,
    pub error: Option<String>,
}

impl MetadataDialog {
    fn destination(&self) -> PathBuf {
        if let Some(custom) = &self.custom_destination {
            custom.clone()
        } else {
            suggested_tool_destination(&self.source_path, "metadata")
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum MetaFlattenKind {
    Flatten,
    Metadata,
}

pub(super) struct MetaFlattenJob {
    pub kind: MetaFlattenKind,
    pub total: usize,
    pub done: Arc<AtomicUsize>,
    pub control: ExportControl,
    pub open_after: bool,
    pub job_id: u64,
}

#[derive(Default)]
pub(super) struct MetaFlattenState {
    pub flatten_dialog: Option<FlattenDialog>,
    pub metadata_dialog: Option<MetadataDialog>,
    pub job: Option<MetaFlattenJob>,
    pub job_seq: u64,
}

impl MetaFlattenState {
    pub(super) fn has_dialog(&self) -> bool {
        self.flatten_dialog.is_some() || self.metadata_dialog.is_some()
    }

    pub(super) fn status_label(&self) -> String {
        let Some(job) = &self.job else {
            return String::new();
        };
        match job.kind {
            MetaFlattenKind::Flatten => {
                let done = job.done.load(Ordering::SeqCst);
                format!("Melakukan Flatten PDF: {done}/{} halaman…", job.total)
            }
            MetaFlattenKind::Metadata => "Menyimpan Properti & Metadata PDF…".into(),
        }
    }
}

fn suggested_tool_destination(source: &std::path::Path, suffix: &str) -> PathBuf {
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

fn format_size(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{bytes} B")
    } else if bytes < 1024 * 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else {
        format!("{:.2} MB", bytes as f64 / (1024.0 * 1024.0))
    }
}

impl WindowShell {
    pub(super) fn show_flatten_pdf(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.close.is_some()
            || self.modal(cx)
            || self.resources.export_owner.load(Ordering::SeqCst) != 0
            || self.meta_flatten.job.is_some()
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
                self.notice = Some(format!("PDF belum dapat di-flatten: {err}"));
                cx.notify();
                return;
            }
        };

        self.meta_flatten.flatten_dialog = Some(FlattenDialog {
            snapshot,
            source_path,
            dpi: RedactDpi::Dpi200,
            strip_metadata: false,
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
                .read_with(cx, |this, _| this.meta_flatten.flatten_dialog.clone())
                .ok()
                .flatten()
            else {
                return dialog;
            };
            let colors = cx.theme().colors;
            let total_pages = ui.snapshot.pages().len();
            let dest_display = ui.destination().display().to_string();

            let toggle_strip = weak.clone();
            let toggle_open = weak.clone();
            let pick_dest = weak.clone();
            let cancel = weak.clone();
            let submit = weak.clone();
            let closed = weak.clone();

            let mut dpi_row = gpui::div().flex().items_center().gap_2();
            for (idx, dpi_opt) in [RedactDpi::Dpi200, RedactDpi::Dpi300].into_iter().enumerate() {
                let pick = weak.clone();
                dpi_row = dpi_row.child(
                    Button::new(("flatten-dpi", idx))
                        .ghost()
                        .small()
                        .selected(ui.dpi == dpi_opt)
                        .label(dpi_opt.label())
                        .on_click(move |_, window, cx| {
                            let _ = pick.update(cx, |this, cx| {
                                if let Some(d) = &mut this.meta_flatten.flatten_dialog {
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
                .child(
                    gpui::div()
                        .px_3()
                        .py_2()
                        .rounded_md()
                        .border_1()
                        .border_color(colors.border)
                        .bg(colors.muted)
                        .text_xs()
                        .child(format!(
                            "Mengunci seluruh anotasi, tanda tangan, stempel, nomor halaman, watermark, dan isian form pada {total_pages} halaman menjadi konten statis yang tidak dapat digeser atau dihapus penerima."
                        )),
                )
                .child(
                    gpui::div()
                        .flex()
                        .flex_col()
                        .gap_1()
                        .child(gpui::div().text_xs().font_semibold().child("Resolusi halaman statis"))
                        .child(dpi_row),
                )
                .child(
                    gpui::div()
                        .flex()
                        .flex_col()
                        .gap_1p5()
                        .child(
                            Button::new("flatten-strip-meta")
                                .ghost()
                                .small()
                                .selected(ui.strip_metadata)
                                .label(if ui.strip_metadata {
                                    "✓ Bersihkan juga metadata tersembunyi (/Info & XMP)"
                                } else {
                                    "○ Bersihkan juga metadata tersembunyi (/Info & XMP)"
                                })
                                .on_click(move |_, window, cx| {
                                    let _ = toggle_strip.update(cx, |this, cx| {
                                        if let Some(d) = &mut this.meta_flatten.flatten_dialog {
                                            d.strip_metadata = !d.strip_metadata;
                                        }
                                        window.refresh();
                                        cx.notify();
                                    });
                                }),
                        )
                        .child(
                            Button::new("flatten-open-after")
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
                                        if let Some(d) = &mut this.meta_flatten.flatten_dialog {
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
                                    Button::new("flatten-pick-dest")
                                        .ghost()
                                        .small()
                                        .disabled(ui.picking)
                                        .label("Ubah lokasi…")
                                        .on_click(move |_, window, cx| {
                                            let _ = pick_dest.update(cx, |this, cx| {
                                                this.pick_flatten_destination(window, cx);
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

            dialog
                .w(px(540.))
                .title("Flatten PDF (Kunci Anotasi & Tanda Tangan)")
                .description(
                    "Simpan salinan PDF statis agar tanda tangan, catatan, dan form tidak dapat diubah lagi. File asli tetap utuh.",
                )
                .child(body)
                .on_close(move |_, _, cx| {
                    let _ = closed.update(cx, |this, cx| this.close_meta_flatten_dialogs(cx));
                })
                .footer(
                    DialogFooter::new()
                        .child(
                            Button::new("flatten-cancel")
                                .ghost()
                                .label("Batal")
                                .on_click(move |_, window, cx| {
                                    window.close_dialog(cx);
                                    let _ = cancel.update(cx, |this, cx| {
                                        this.close_meta_flatten_dialogs(cx);
                                    });
                                }),
                        )
                        .child(
                            Button::new("flatten-submit")
                                .primary()
                                .disabled(ui.picking)
                                .label("Flatten & Simpan PDF")
                                .on_click(move |_, window, cx| {
                                    let _ = submit.update(cx, |this, cx| {
                                        this.submit_flatten_pdf(window, cx);
                                    });
                                }),
                        ),
                )
        });
    }

    pub(super) fn show_metadata_editor(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.close.is_some()
            || self.modal(cx)
            || self.resources.export_owner.load(Ordering::SeqCst) != 0
            || self.meta_flatten.job.is_some()
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
                self.notice = Some(format!("Properti PDF belum dapat dibuka: {err}"));
                cx.notify();
                return;
            }
        };
        let file_bytes = snapshot
            .sources()
            .first()
            .map(|s| s.fingerprint.bytes)
            .unwrap_or(0);

        let title = cx.new(|cx| InputState::new(window, cx).placeholder("Judul dokumen"));
        let author =
            cx.new(|cx| InputState::new(window, cx).placeholder("Nama penulis / instansi"));
        let subject =
            cx.new(|cx| InputState::new(window, cx).placeholder("Subjek / deskripsi singkat"));
        let keywords =
            cx.new(|cx| InputState::new(window, cx).placeholder("contoh: kontrak, laporan, 2026"));
        let creator = cx.new(|cx| InputState::new(window, cx).placeholder("Aplikasi pembuat"));
        let producer = cx.new(|cx| InputState::new(window, cx).placeholder("Producer PDF"));

        for input in [&title, &author, &subject, &keywords, &creator, &producer] {
            cx.subscribe_in(input, window, |this, _, event: &InputEvent, window, cx| {
                if matches!(event, InputEvent::Change) {
                    if let Some(d) = &mut this.meta_flatten.metadata_dialog {
                        d.error = None;
                    }
                    window.refresh();
                    cx.notify();
                }
            })
            .detach();
        }

        self.meta_flatten.metadata_dialog = Some(MetadataDialog {
            snapshot,
            source_path: source_path.clone(),
            file_bytes,
            loading: true,
            title: title.clone(),
            author: author.clone(),
            subject: subject.clone(),
            keywords: keywords.clone(),
            creator: creator.clone(),
            producer: producer.clone(),
            strip_xmp: true,
            open_after: true,
            custom_destination: None,
            picking: false,
            error: None,
        });
        self.resources
            .modal_owner
            .store(SHELL_EXPORT_OWNER, Ordering::SeqCst);
        self.report(cx);

        // Load existing /Info metadata asynchronously via QPDF.
        let sidecars = self.sidecars.clone();
        let read_path = source_path;
        let handle = window.window_handle();
        let task = cx.background_executor().spawn(async move {
            let tool = QpdfMetadata::new(&sidecars);
            tool.read_metadata(&read_path, &ExportControl::default())
        });
        cx.spawn(async move |this, cx| {
            let result = task.await;
            let _ = handle.update(cx, |_, window, cx| {
                let _ = this.update(cx, |this, cx| {
                    if let Some(d) = &mut this.meta_flatten.metadata_dialog {
                        d.loading = false;
                        if let Ok(meta) = result {
                            d.title
                                .update(cx, |s, cx| s.set_value(meta.title, window, cx));
                            d.author
                                .update(cx, |s, cx| s.set_value(meta.author, window, cx));
                            d.subject
                                .update(cx, |s, cx| s.set_value(meta.subject, window, cx));
                            d.keywords
                                .update(cx, |s, cx| s.set_value(meta.keywords, window, cx));
                            d.creator
                                .update(cx, |s, cx| s.set_value(meta.creator, window, cx));
                            d.producer
                                .update(cx, |s, cx| s.set_value(meta.producer, window, cx));
                        }
                    }
                    window.refresh();
                    cx.notify();
                });
            });
        })
        .detach();

        let weak = cx.weak_entity();
        window.open_alert_dialog(cx, move |dialog, _, cx| {
            let Some(ui) = weak
                .read_with(cx, |this, _| this.meta_flatten.metadata_dialog.clone())
                .ok()
                .flatten()
            else {
                return dialog;
            };
            let colors = cx.theme().colors;
            let total_pages = ui.snapshot.pages().len();
            let size_text = format_size(ui.file_bytes);
            let dest_display = ui.destination().display().to_string();

            let clear_all = weak.clone();
            let toggle_xmp = weak.clone();
            let toggle_open = weak.clone();
            let pick_dest = weak.clone();
            let cancel = weak.clone();
            let submit = weak.clone();
            let closed = weak.clone();

            let field_row = |label: &'static str, state: &Entity<InputState>| {
                gpui::div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(
                        gpui::div()
                            .w(px(108.))
                            .flex_shrink_0()
                            .text_xs()
                            .font_semibold()
                            .child(label),
                    )
                    .child(gpui::div().flex_1().child(Input::new(state).small()))
            };

            let mut body = gpui::div()
                .flex()
                .flex_col()
                .gap_2p5()
                .child(
                    gpui::div()
                        .flex()
                        .items_center()
                        .justify_between()
                        .px_3()
                        .py_1p5()
                        .rounded_md()
                        .border_1()
                        .border_color(colors.border)
                        .bg(colors.muted)
                        .text_xs()
                        .child(if ui.loading {
                            format!("{total_pages} halaman · {size_text} · Memuat properti…")
                        } else {
                            format!("{total_pages} halaman · {size_text}")
                        })
                        .child(
                            Button::new("meta-clear-all")
                                .ghost()
                                .small()
                                .disabled(ui.loading)
                                .label("Kosongkan Semua")
                                .on_click(move |_, window, cx| {
                                    let _ = clear_all.update(cx, |this, cx| {
                                        if let Some(d) = &mut this.meta_flatten.metadata_dialog {
                                            for f in [
                                                &d.title,
                                                &d.author,
                                                &d.subject,
                                                &d.keywords,
                                                &d.creator,
                                                &d.producer,
                                            ] {
                                                f.update(cx, |s, cx| {
                                                    s.set_value(String::new(), window, cx)
                                                });
                                            }
                                            d.error = None;
                                        }
                                        window.refresh();
                                        cx.notify();
                                    });
                                }),
                        ),
                )
                .child(field_row("Judul (Title)", &ui.title))
                .child(field_row("Penulis (Author)", &ui.author))
                .child(field_row("Subjek (Subject)", &ui.subject))
                .child(field_row("Kata Kunci", &ui.keywords))
                .child(field_row("Creator", &ui.creator))
                .child(field_row("Producer", &ui.producer))
                .child(
                    gpui::div()
                        .flex()
                        .flex_col()
                        .gap_1()
                        .child(
                            Button::new("meta-toggle-xmp")
                                .ghost()
                                .small()
                                .selected(ui.strip_xmp)
                                .label(if ui.strip_xmp {
                                    "✓ Hapus metadata XMP lama agar properti selalu sinkron"
                                } else {
                                    "○ Hapus metadata XMP lama agar properti selalu sinkron"
                                })
                                .on_click(move |_, window, cx| {
                                    let _ = toggle_xmp.update(cx, |this, cx| {
                                        if let Some(d) = &mut this.meta_flatten.metadata_dialog {
                                            d.strip_xmp = !d.strip_xmp;
                                        }
                                        window.refresh();
                                        cx.notify();
                                    });
                                }),
                        )
                        .child(
                            Button::new("meta-toggle-open")
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
                                        if let Some(d) = &mut this.meta_flatten.metadata_dialog {
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
                                    Button::new("meta-pick-dest")
                                        .ghost()
                                        .small()
                                        .disabled(ui.picking)
                                        .label("Ubah lokasi…")
                                        .on_click(move |_, window, cx| {
                                            let _ = pick_dest.update(cx, |this, cx| {
                                                this.pick_metadata_destination(window, cx);
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

            dialog
                .w(px(560.))
                .title("Properti & Metadata PDF")
                .description(
                    "Lihat, ubah, atau kosongkan metadata dokumen (Title, Author, Subject, Keywords, Creator, Producer) ke file PDF baru.",
                )
                .child(body)
                .on_close(move |_, _, cx| {
                    let _ = closed.update(cx, |this, cx| this.close_meta_flatten_dialogs(cx));
                })
                .footer(
                    DialogFooter::new()
                        .child(
                            Button::new("meta-cancel")
                                .ghost()
                                .label("Batal")
                                .on_click(move |_, window, cx| {
                                    window.close_dialog(cx);
                                    let _ = cancel.update(cx, |this, cx| {
                                        this.close_meta_flatten_dialogs(cx);
                                    });
                                }),
                        )
                        .child(
                            Button::new("meta-submit")
                                .primary()
                                .disabled(ui.loading || ui.picking)
                                .label("Simpan Metadata PDF")
                                .on_click(move |_, window, cx| {
                                    let _ = submit.update(cx, |this, cx| {
                                        this.submit_metadata_edit(window, cx);
                                    });
                                }),
                        ),
                )
        });
    }

    fn close_meta_flatten_dialogs(&mut self, cx: &mut Context<Self>) {
        self.meta_flatten.flatten_dialog = None;
        self.meta_flatten.metadata_dialog = None;
        let _ = self.resources.modal_owner.compare_exchange(
            SHELL_EXPORT_OWNER,
            0,
            Ordering::SeqCst,
            Ordering::SeqCst,
        );
        self.report(cx);
        cx.notify();
    }

    fn pick_flatten_destination(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(d) = &mut self.meta_flatten.flatten_dialog else {
            return;
        };
        if d.picking {
            return;
        }
        d.picking = true;
        let dest = d.destination();
        let dir = dest
            .parent()
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("."));
        let name = dest
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("dokumen-flattened.pdf")
            .to_owned();
        let rx = cx.prompt_for_new_path(&dir, Some(&name));
        let handle = window.window_handle();
        cx.spawn(async move |this, cx| {
            let picked = rx.await.ok().and_then(Result::ok).flatten();
            let _ = handle.update(cx, |_, window, cx| {
                let _ = this.update(cx, |this, cx| {
                    if let Some(d) = &mut this.meta_flatten.flatten_dialog {
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

    fn pick_metadata_destination(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(d) = &mut self.meta_flatten.metadata_dialog else {
            return;
        };
        if d.picking {
            return;
        }
        d.picking = true;
        let dest = d.destination();
        let dir = dest
            .parent()
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("."));
        let name = dest
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("dokumen-metadata.pdf")
            .to_owned();
        let rx = cx.prompt_for_new_path(&dir, Some(&name));
        let handle = window.window_handle();
        cx.spawn(async move |this, cx| {
            let picked = rx.await.ok().and_then(Result::ok).flatten();
            let _ = handle.update(cx, |_, window, cx| {
                let _ = this.update(cx, |this, cx| {
                    if let Some(d) = &mut this.meta_flatten.metadata_dialog {
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

    fn submit_flatten_pdf(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(ui) = self.meta_flatten.flatten_dialog.clone() else {
            return;
        };
        let destination = ui.destination();
        if destination.exists() {
            if let Some(d) = &mut self.meta_flatten.flatten_dialog {
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
            if let Some(d) = &mut self.meta_flatten.flatten_dialog {
                d.error = Some("Ada proses ekspor lain yang sedang berjalan.".into());
            }
            window.refresh();
            cx.notify();
            return;
        }

        window.close_dialog(cx);
        self.close_meta_flatten_dialogs(cx);

        self.meta_flatten.job_seq += 1;
        let job_id = self.meta_flatten.job_seq;
        let total = ui.snapshot.pages().len();
        let done = Arc::new(AtomicUsize::new(0));
        let control = ExportControl::default();
        let open_after = ui.open_after;
        let options = FlattenOptions {
            dpi: ui.dpi,
            strip_metadata: ui.strip_metadata,
        };
        let plan = ui.snapshot;
        let sidecars = self.sidecars.clone();
        let renderer = self.renderer.clone();
        let inspector = self.renderer.clone();

        self.meta_flatten.job = Some(MetaFlattenJob {
            kind: MetaFlattenKind::Flatten,
            total,
            done: done.clone(),
            control: control.clone(),
            open_after,
            job_id,
        });
        cx.notify();

        let poll_done = done.clone();
        let (tx, rx) = std::sync::mpsc::sync_channel::<Result<FlattenReport, String>>(1);
        std::thread::spawn(move || {
            let tool = QpdfMetadata::new(&sidecars);
            let res = tool
                .flatten_pdf(
                    &plan,
                    &destination,
                    options,
                    &renderer,
                    &inspector,
                    &control,
                    &mut |count| {
                        poll_done.store(count, Ordering::SeqCst);
                    },
                )
                .map_err(|err| err.to_string());
            let _ = tx.send(res);
        });

        cx.spawn(async move |this, cx| {
            loop {
                match rx.try_recv() {
                    Ok(result) => {
                        let _ = this.update(cx, |this, cx| {
                            this.finish_flatten_job(job_id, result, cx);
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
                            this.finish_flatten_job(
                                job_id,
                                Err("Proses flatten berhenti tak terduga.".into()),
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

    fn submit_metadata_edit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(ui) = self.meta_flatten.metadata_dialog.clone() else {
            return;
        };
        let destination = ui.destination();
        if destination.exists() {
            if let Some(d) = &mut self.meta_flatten.metadata_dialog {
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
            if let Some(d) = &mut self.meta_flatten.metadata_dialog {
                d.error = Some("Ada proses ekspor lain yang sedang berjalan.".into());
            }
            window.refresh();
            cx.notify();
            return;
        }

        let metadata = PdfMetadata {
            title: ui.title.read(cx).value().to_string(),
            author: ui.author.read(cx).value().to_string(),
            subject: ui.subject.read(cx).value().to_string(),
            keywords: ui.keywords.read(cx).value().to_string(),
            creator: ui.creator.read(cx).value().to_string(),
            producer: ui.producer.read(cx).value().to_string(),
        };

        window.close_dialog(cx);
        self.close_meta_flatten_dialogs(cx);

        self.meta_flatten.job_seq += 1;
        let job_id = self.meta_flatten.job_seq;
        let total = ui.snapshot.pages().len();
        let done = Arc::new(AtomicUsize::new(0));
        let control = ExportControl::default();
        let open_after = ui.open_after;
        let strip_xmp = ui.strip_xmp;
        let plan = ui.snapshot;
        let sidecars = self.sidecars.clone();
        let inspector = self.renderer.clone();

        self.meta_flatten.job = Some(MetaFlattenJob {
            kind: MetaFlattenKind::Metadata,
            total,
            done,
            control: control.clone(),
            open_after,
            job_id,
        });
        cx.notify();

        let (tx, rx) = std::sync::mpsc::sync_channel::<Result<MetadataReport, String>>(1);
        std::thread::spawn(move || {
            let tool = QpdfMetadata::new(&sidecars);
            let res = tool
                .write_metadata(
                    &plan,
                    &destination,
                    &metadata,
                    strip_xmp,
                    &inspector,
                    &control,
                )
                .map_err(|err| err.to_string());
            let _ = tx.send(res);
        });

        cx.spawn(async move |this, cx| {
            loop {
                match rx.try_recv() {
                    Ok(result) => {
                        let _ = this.update(cx, |this, cx| {
                            this.finish_metadata_job(job_id, result, cx);
                        });
                        break;
                    }
                    Err(std::sync::mpsc::TryRecvError::Empty) => {
                        cx.background_executor()
                            .timer(std::time::Duration::from_millis(80))
                            .await;
                    }
                    Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                        let _ = this.update(cx, |this, cx| {
                            this.finish_metadata_job(
                                job_id,
                                Err("Proses simpan metadata berhenti tak terduga.".into()),
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

    pub(super) fn cancel_meta_flatten_job(&mut self, cx: &mut Context<Self>) {
        if let Some(job) = &self.meta_flatten.job {
            job.control.cancel();
            cx.notify();
        }
    }

    fn finish_flatten_job(
        &mut self,
        job_id: u64,
        result: Result<FlattenReport, String>,
        cx: &mut Context<Self>,
    ) {
        let Some(job) = self.meta_flatten.job.take() else {
            return;
        };
        if job.job_id != job_id {
            self.meta_flatten.job = Some(job);
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
                    .unwrap_or("hasil-flattened.pdf");
                self.notice = Some(format!(
                    "Flatten PDF selesai: {} halaman dikunci statis → {}",
                    report.page_count, name
                ));
                if job.open_after && self.tabs.len() < MAX_TABS {
                    self.open_paths(vec![report.output], cx);
                }
            }
            Err(err) => {
                self.notice = Some(format!("Flatten PDF gagal: {err}"));
            }
        }
        if self.close.is_some() {
            self.advance_close(cx);
        }
        self.report(cx);
        cx.notify();
    }

    fn finish_metadata_job(
        &mut self,
        job_id: u64,
        result: Result<MetadataReport, String>,
        cx: &mut Context<Self>,
    ) {
        let Some(job) = self.meta_flatten.job.take() else {
            return;
        };
        if job.job_id != job_id {
            self.meta_flatten.job = Some(job);
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
                    .unwrap_or("hasil-metadata.pdf");
                self.notice = Some(if report.cleared {
                    format!("Seluruh metadata PDF dihapus bersih → {name}")
                } else {
                    format!("Properti & Metadata PDF berhasil disimpan → {name}")
                });
                if job.open_after && self.tabs.len() < MAX_TABS {
                    self.open_paths(vec![report.output], cx);
                }
            }
            Err(err) => {
                self.notice = Some(format!("Gagal menyimpan metadata PDF: {err}"));
            }
        }
        if self.close.is_some() {
            self.advance_close(cx);
        }
        self.report(cx);
        cx.notify();
    }
}
