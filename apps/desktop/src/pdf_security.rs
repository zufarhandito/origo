//! Window-level dialogs and background QPDF jobs for PDF password protection (AES-256)
//! and PDF unlocking. Works both with an open PDF tab and with zero tabs open.
use crate::images_to_pdf::SHELL_EXPORT_OWNER;
use crate::shell::{MAX_TABS, WindowShell};
use crate::*;
use gpui::component::{
    ActiveTheme, Disableable, Selectable, Sizable, StyledExt, WindowExt,
    button::{Button, ButtonVariants},
    dialog::DialogFooter,
    input::Input,
};
use pdf_application::export::ExportControl;
use qpdf_adapter::{EncryptOptions, EncryptPermissions, QpdfSecurity, SecurityReport};
use std::sync::atomic::Ordering;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum SecurityMode {
    Encrypt,
    Decrypt,
}

#[derive(Clone)]
pub(super) struct SecurityDialog {
    pub mode: SecurityMode,
    pub source: Option<PathBuf>,
    pub active_dirty_hint: bool,
    pub password: Entity<InputState>,
    pub confirm_password: Entity<InputState>,
    pub owner_password: Entity<InputState>,
    pub permissions: EncryptPermissions,
    pub open_after: bool,
    pub picking: bool,
    pub error: Option<String>,
}

pub(super) struct SecurityJob {
    pub mode: SecurityMode,
    pub control: ExportControl,
    pub open_after: bool,
    pub job_id: u64,
}

#[derive(Default)]
pub(super) struct SecurityState {
    pub dialog: Option<SecurityDialog>,
    pub job: Option<SecurityJob>,
    pub job_seq: u64,
    pub completed: u64,
    pub success: bool,
}

impl SecurityState {
    pub(super) fn status_label(&self) -> &'static str {
        match self.job.as_ref().map(|j| j.mode) {
            Some(SecurityMode::Encrypt) => "Mengenkripsi PDF (AES-256)…",
            Some(SecurityMode::Decrypt) => "Membuka kunci PDF…",
            None => "",
        }
    }
}

fn suggested_security_name(source: &std::path::Path, mode: SecurityMode) -> String {
    let stem = source
        .file_stem()
        .and_then(|s| s.to_str())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or("dokumen");
    match mode {
        SecurityMode::Encrypt => format!("{stem}-terlindungi.pdf"),
        SecurityMode::Decrypt => format!("{stem}-terbuka.pdf"),
    }
}

fn format_bytes(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{bytes} B")
    } else if bytes < 1024 * 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else {
        format!("{:.2} MB", bytes as f64 / (1024.0 * 1024.0))
    }
}

impl WindowShell {
    pub(super) fn show_encrypt_pdf(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.show_security_dialog(SecurityMode::Encrypt, window, cx);
    }

    pub(super) fn show_decrypt_pdf(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.show_security_dialog(SecurityMode::Decrypt, window, cx);
    }

    fn show_security_dialog(
        &mut self,
        mode: SecurityMode,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.close.is_some()
            || self.modal(cx)
            || self.resources.export_owner.load(Ordering::SeqCst) != 0
            || self.security.job.is_some()
        {
            return;
        }
        let (source, active_dirty_hint) = if mode == SecurityMode::Encrypt {
            let active_path = self.active_tab().map(|tab| tab.path.clone());
            let dirty = self.session().is_some_and(|session| {
                session
                    .read(cx)
                    .workspace
                    .as_ref()
                    .is_some_and(|w| w.is_dirty())
            });
            (active_path, dirty)
        } else {
            (None, false)
        };

        let password = cx.new(|cx| {
            let mut s = InputState::new(window, cx).placeholder(if mode == SecurityMode::Encrypt {
                "Masukkan password untuk membuka PDF"
            } else {
                "Masukkan password PDF saat ini"
            });
            s.set_masked(true, window, cx);
            s
        });
        let confirm_password = cx.new(|cx| {
            let mut s = InputState::new(window, cx).placeholder("Ulangi password buka PDF");
            s.set_masked(true, window, cx);
            s
        });
        let owner_password = cx.new(|cx| {
            let mut s = InputState::new(window, cx)
                .placeholder("Opsional (kosongkan agar sama dengan password buka)");
            s.set_masked(true, window, cx);
            s
        });

        for input in [&password, &confirm_password, &owner_password] {
            cx.subscribe_in(input, window, |this, _, event: &InputEvent, window, cx| {
                if matches!(event, InputEvent::Change) {
                    if let Some(dialog) = &mut this.security.dialog {
                        dialog.error = None;
                    }
                    window.refresh();
                    cx.notify();
                }
            })
            .detach();
        }

        self.security.dialog = Some(SecurityDialog {
            mode,
            source,
            active_dirty_hint,
            password,
            confirm_password,
            owner_password,
            permissions: EncryptPermissions::default(),
            open_after: mode == SecurityMode::Decrypt,
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
                .read_with(cx, |this, _| this.security.dialog.clone())
                .ok()
                .flatten()
            else {
                return dialog;
            };
            let is_encrypt = ui.mode == SecurityMode::Encrypt;
            let title = if is_encrypt {
                "Lindungi PDF dengan Password (AES-256)"
            } else {
                "Buka Kunci PDF Terenkripsi"
            };
            let desc = if is_encrypt {
                "Buat salinan PDF baru terenkripsi AES-256 secara lokal (offline). File asli tidak diubah."
            } else {
                "Hapus proteksi password ke file PDF baru secara lokal (offline). File asli tetap utuh."
            };
            let source_label = ui
                .source
                .as_ref()
                .map(|p| p.display().to_string())
                .unwrap_or_else(|| "Belum ada file PDF dipilih.".into());
            let colors = cx.theme().colors;

            let pick = weak.clone();
            let cancel = weak.clone();
            let submit = weak.clone();
            let closed = weak.clone();
            let toggle_print = weak.clone();
            let toggle_copy = weak.clone();
            let toggle_modify = weak.clone();
            let toggle_open = weak.clone();

            let mut body = gpui::div()
                .flex()
                .flex_col()
                .gap_3()
                .child(
                    gpui::div()
                        .flex()
                        .flex_col()
                        .gap_1()
                        .child(gpui::div().text_xs().font_semibold().child("File PDF sumber"))
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
                                        .child(source_label),
                                )
                                .child(
                                    Button::new("security-pick-source")
                                        .outline()
                                        .small()
                                        .label("Pilih PDF…")
                                        .disabled(ui.picking)
                                        .on_click(move |_, _, cx| {
                                            let _ = pick.update(cx, |this, cx| {
                                                this.pick_security_source(cx);
                                            });
                                        }),
                                ),
                        )
                        .when(is_encrypt && ui.active_dirty_hint, |el| {
                            el.child(
                                gpui::div()
                                    .text_xs()
                                    .text_color(colors.muted_foreground)
                                    .child(
                                        "Catatan: Tab aktif punya perubahan belum disimpan. Simpan (Ctrl+Shift+S) dulu jika ingin editan terbaru ikut terenkripsi.",
                                    ),
                            )
                        }),
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
                                .child(if is_encrypt {
                                    "Password buka PDF"
                                } else {
                                    "Password PDF"
                                }),
                        )
                        .child(
                            Input::new(&ui.password)
                                .mask_toggle()
                                .aria_label("Password PDF"),
                        ),
                );

            if is_encrypt {
                body = body
                    .child(
                        gpui::div()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .child(
                                gpui::div()
                                    .text_xs()
                                    .font_semibold()
                                    .child("Konfirmasi password buka PDF"),
                            )
                            .child(
                                Input::new(&ui.confirm_password)
                                    .mask_toggle()
                                    .aria_label("Konfirmasi password PDF"),
                            ),
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
                                    .child("Password izin/pemilik (opsional)"),
                            )
                            .child(
                                Input::new(&ui.owner_password)
                                    .mask_toggle()
                                    .aria_label("Password pemilik PDF"),
                            ),
                    )
                    .child(
                        gpui::div()
                            .flex()
                            .flex_col()
                            .gap_1p5()
                            .child(gpui::div().text_xs().font_semibold().child("Izin dokumen"))
                            .child(
                                gpui::div()
                                    .flex()
                                    .flex_wrap()
                                    .gap_2()
                                    .child(
                                        Button::new("sec-perm-print")
                                            .small()
                                            .label(if ui.permissions.allow_print {
                                                "Cetak: Diizinkan"
                                            } else {
                                                "Cetak: Dilarang"
                                            })
                                            .selected(ui.permissions.allow_print)
                                            .on_click(move |_, window, cx| {
                                                let _ = toggle_print.update(cx, |this, cx| {
                                                    if let Some(d) = &mut this.security.dialog {
                                                        d.permissions.allow_print =
                                                            !d.permissions.allow_print;
                                                    }
                                                    window.refresh();
                                                    cx.notify();
                                                });
                                            }),
                                    )
                                    .child(
                                        Button::new("sec-perm-copy")
                                            .small()
                                            .label(if ui.permissions.allow_copy {
                                                "Salin teks: Diizinkan"
                                            } else {
                                                "Salin teks: Dilarang"
                                            })
                                            .selected(ui.permissions.allow_copy)
                                            .on_click(move |_, window, cx| {
                                                let _ = toggle_copy.update(cx, |this, cx| {
                                                    if let Some(d) = &mut this.security.dialog {
                                                        d.permissions.allow_copy =
                                                            !d.permissions.allow_copy;
                                                    }
                                                    window.refresh();
                                                    cx.notify();
                                                });
                                            }),
                                    )
                                    .child(
                                        Button::new("sec-perm-modify")
                                            .small()
                                            .label(if ui.permissions.allow_modify {
                                                "Ubah: Diizinkan"
                                            } else {
                                                "Ubah: Dilarang"
                                            })
                                            .selected(ui.permissions.allow_modify)
                                            .on_click(move |_, window, cx| {
                                                let _ = toggle_modify.update(cx, |this, cx| {
                                                    if let Some(d) = &mut this.security.dialog {
                                                        d.permissions.allow_modify =
                                                            !d.permissions.allow_modify;
                                                    }
                                                    window.refresh();
                                                    cx.notify();
                                                });
                                            }),
                                    ),
                            ),
                    );
            } else {
                body = body.child(
                    gpui::div().flex().items_center().gap_2().child(
                        Button::new("sec-open-after")
                            .small()
                            .label(if ui.open_after {
                                "Buka PDF hasil di Origo: Ya"
                            } else {
                                "Buka PDF hasil di Origo: Tidak"
                            })
                            .selected(ui.open_after)
                            .on_click(move |_, window, cx| {
                                let _ = toggle_open.update(cx, |this, cx| {
                                    if let Some(d) = &mut this.security.dialog {
                                        d.open_after = !d.open_after;
                                    }
                                    window.refresh();
                                    cx.notify();
                                });
                            }),
                    ),
                );
            }

            if let Some(err) = ui.error.clone() {
                body = body.child(
                    gpui::div()
                        .id("security-error")
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
                        .id("security-dialog-title")
                        .role(gpui::Role::Group)
                        .aria_label(title)
                        .child(title),
                )
                .description(desc)
                .centered()
                .width(px(560.))
                .close_button(true)
                .child(body)
                .footer(
                    DialogFooter::new()
                        .child(Button::new("security-cancel").label("Batal").on_click(
                            move |_, window, cx| {
                                window.close_dialog(cx);
                                let _ = cancel.update(cx, |this, cx| {
                                    this.close_security_dialog(cx);
                                });
                            },
                        ))
                        .child(
                            Button::new("security-submit")
                                .primary()
                                .disabled(ui.picking)
                                .label(if is_encrypt {
                                    "Simpan PDF Terlindungi…"
                                } else {
                                    "Simpan PDF Terbuka…"
                                })
                                .on_click(move |_, window, cx| {
                                    let _ = submit.update(cx, |this, cx| {
                                        this.submit_security_dialog(window, cx);
                                    });
                                }),
                        ),
                )
                .on_close(move |_, _, cx| {
                    let _ = closed.update(cx, |this, cx| this.close_security_dialog(cx));
                })
        });
        cx.notify();
    }

    fn pick_security_source(&mut self, cx: &mut Context<Self>) {
        let Some(dialog) = &mut self.security.dialog else {
            return;
        };
        if dialog.picking {
            return;
        }
        dialog.picking = true;
        dialog.error = None;
        let prompt = cx.prompt_for_paths(gpui::PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some("Pilih file PDF".into()),
        });
        let handle = self.handle;
        cx.spawn(async move |this, cx| {
            let reply = prompt.await;
            cx.update(|cx| {
                let _ = handle.update(cx, |_, window, cx| {
                    let _ = this.update(cx, |this, cx| {
                        if let Some(dialog) = &mut this.security.dialog {
                            dialog.picking = false;
                            if let Ok(Ok(Some(mut paths))) = reply
                                && let Some(path) = paths.pop()
                            {
                                if path
                                    .extension()
                                    .is_some_and(|e| e.eq_ignore_ascii_case("pdf"))
                                {
                                    dialog.source = Some(path);
                                    dialog.active_dirty_hint = false;
                                    dialog.error = None;
                                } else {
                                    dialog.error = Some("Pilih file berekstensi .pdf.".into());
                                }
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

    fn close_security_dialog(&mut self, cx: &mut Context<Self>) {
        self.security.dialog = None;
        let _ = self.resources.modal_owner.compare_exchange(
            SHELL_EXPORT_OWNER,
            0,
            Ordering::SeqCst,
            Ordering::SeqCst,
        );
        self.report(cx);
        cx.notify();
    }

    fn submit_security_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(dialog) = self.security.dialog.clone() else {
            return;
        };
        let Some(source) = dialog.source.clone() else {
            if let Some(d) = &mut self.security.dialog {
                d.error = Some("Pilih file PDF sumber terlebih dahulu.".into());
            }
            window.refresh();
            cx.notify();
            return;
        };
        let pw = dialog.password.read(cx).value().to_string();
        if dialog.mode == SecurityMode::Encrypt {
            let confirm = dialog.confirm_password.read(cx).value().to_string();
            if pw.is_empty() {
                if let Some(d) = &mut self.security.dialog {
                    d.error = Some("Masukkan password minimal 1 karakter.".into());
                }
                window.refresh();
                cx.notify();
                return;
            }
            if pw != confirm {
                if let Some(d) = &mut self.security.dialog {
                    d.error = Some("Konfirmasi password tidak cocok.".into());
                }
                window.refresh();
                cx.notify();
                return;
            }
        }
        let owner_pw = dialog.owner_password.read(cx).value().to_string();
        let suggested = suggested_security_name(&source, dialog.mode);
        let start_dir = source
            .parent()
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("."));
        let mode = dialog.mode;
        let permissions = dialog.permissions;
        let open_after = dialog.open_after;

        window.close_dialog(cx);
        self.close_security_dialog(cx);

        if self
            .resources
            .export_owner
            .compare_exchange(0, SHELL_EXPORT_OWNER, Ordering::SeqCst, Ordering::SeqCst)
            .is_err()
        {
            self.notice = Some("Tunggu proses ekspor lain selesai sebelum memproses keamanan PDF.".into());
            cx.notify();
            return;
        }

        let control = ExportControl::default();
        self.security.job_seq += 1;
        let job_id = self.security.job_seq;
        self.security.job = Some(SecurityJob {
            mode,
            control: control.clone(),
            open_after,
            job_id,
        });
        self.notice = None;
        self.report(cx);
        cx.notify();

        let prompt = cx.prompt_for_new_path(&start_dir, Some(&suggested));
        let sidecars = self.sidecars.clone();
        cx.spawn(async move |this, cx| {
            let chosen = prompt.await;
            let destination = match chosen {
                Ok(Ok(Some(path))) => {
                    if path.extension().is_some_and(|e| e.eq_ignore_ascii_case("pdf")) {
                        path
                    } else {
                        path.with_extension("pdf")
                    }
                }
                _ => {
                    let _ = this.update(cx, |this, cx| {
                        this.finish_security_job(job_id, None, cx);
                    });
                    return;
                }
            };
            let result = cx
                .background_executor()
                .spawn(async move {
                    let engine = QpdfSecurity::new(&sidecars);
                    match mode {
                        SecurityMode::Encrypt => {
                            let options = EncryptOptions {
                                user_password: pw,
                                owner_password: owner_pw,
                                permissions,
                            };
                            engine.encrypt_pdf(&source, &destination, &options, &control)
                        }
                        SecurityMode::Decrypt => {
                            engine.decrypt_pdf(&source, &destination, &pw, &control)
                        }
                    }
                })
                .await;
            let _ = this.update(cx, |this, cx| {
                this.finish_security_job(job_id, Some(result), cx);
            });
        })
        .detach();
    }

    pub(super) fn cancel_security_job(&mut self, cx: &mut Context<Self>) {
        if let Some(job) = &self.security.job {
            job.control.cancel();
            cx.notify();
        }
    }

    fn finish_security_job(
        &mut self,
        job_id: u64,
        result: Option<Result<SecurityReport, qpdf_adapter::SecurityError>>,
        cx: &mut Context<Self>,
    ) {
        let Some(job) = self.security.job.take() else {
            return;
        };
        if job.job_id != job_id {
            self.security.job = Some(job);
            return;
        }
        let _ = self.resources.export_owner.compare_exchange(
            SHELL_EXPORT_OWNER,
            0,
            Ordering::SeqCst,
            Ordering::SeqCst,
        );
        match result {
            None => {}
            Some(Ok(report)) => {
                self.security.completed += 1;
                self.security.success = true;
                let file_name = report
                    .output
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_else(|| report.output.display().to_string());
                self.notice = Some(if report.encrypted {
                    format!(
                        "PDF berhasil dilindungi (AES-256): {file_name} ({}).",
                        format_bytes(report.bytes)
                    )
                } else {
                    format!(
                        "Password PDF berhasil dihapus: {file_name} ({}).",
                        format_bytes(report.bytes)
                    )
                });
                if !report.encrypted && job.open_after && self.tabs.len() < MAX_TABS {
                    self.open_paths(vec![report.output], cx);
                }
            }
            Some(Err(err)) => {
                self.security.completed += 1;
                self.security.success = false;
                self.notice = Some(err.to_string());
            }
        }
        self.report(cx);
        cx.notify();
    }
}
