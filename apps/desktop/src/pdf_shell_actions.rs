//! Explorer shell actions for `.pdf` files: single-PDF tools, multi-PDF combine
//! (Organizer & 1-click no-clobber), and Windows 11 shell extension management.
use crate::images_to_pdf::SHELL_EXPORT_OWNER;
use crate::shell::{MAX_TABS, Tab, TabId, ViewId, WindowShell};
use crate::*;
use pdf_application::{
    PdfInspector, SourceDocument,
    export::{
        ExportControl, ExportError, ExportPhase, ExportPlan, ExportReceipt, ExportScope,
        finish_export, prepare_export,
    },
};
use pdf_domain::{Command, SourceId};
use pdf_platform::LocalOutput;
use std::sync::atomic::Ordering;

#[cfg(windows)]
use gpui::component::{
    ActiveTheme, Disableable, StyledExt, WindowExt,
    button::{Button, ButtonVariants},
    dialog::DialogFooter,
};

pub(super) struct PdfMergeJob {
    pub files: usize,
    pub control: ExportControl,
    pub job_id: u64,
}

#[derive(Default)]
pub(super) struct PdfMergeState {
    pub job: Option<PdfMergeJob>,
    pub job_seq: u64,
    pub completed: u64,
    pub success: bool,
    pub pages: usize,
}

impl PdfMergeState {
    pub(super) fn phase_label(&self) -> String {
        let Some(job) = &self.job else {
            return String::new();
        };
        let phase = job.control.phase();
        match phase {
            ExportPhase::Choosing => format!("Membaca {} PDF sumber…", job.files),
            ExportPhase::Snapshot => "Menyiapkan salinan PDF sumber…".into(),
            ExportPhase::Preflight => "Memeriksa keamanan struktur PDF…".into(),
            ExportPhase::Writing => format!("Menggabungkan {} PDF…", job.files),
            ExportPhase::Validation => "Memvalidasi PDF hasil gabungan…".into(),
            ExportPhase::Commit => "Menyimpan file PDF gabungan…".into(),
            _ => phase.label().into(),
        }
    }
}

#[cfg(windows)]
#[derive(Clone, Default)]
pub(super) struct ShellExtDialog {
    pub loading: bool,
    pub busy: bool,
    pub installed: bool,
    pub version: Option<String>,
    pub message: Option<String>,
}

fn is_pdf_extension(path: &std::path::Path) -> bool {
    path.extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("pdf"))
}

fn sanitize_stem(raw: &str) -> String {
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
        "gabungan".into()
    } else {
        cleaned.to_owned()
    }
}

pub(super) fn compute_quick_merge_destination(pdfs: &[PathBuf]) -> Option<PathBuf> {
    let parent = pdfs.first()?.parent()?;
    if parent.as_os_str().is_empty() {
        return None;
    }
    let folder = parent
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("gabungan");
    let stem = sanitize_stem(folder);
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

fn build_combined_workspace(
    renderer: &PdfiumRenderer,
    raw_paths: &[PathBuf],
    base_source_id: u64,
    control: Option<&ExportControl>,
) -> Result<(Workspace, PathBuf, usize), String> {
    let mut canonical = Vec::with_capacity(raw_paths.len());
    for p in raw_paths {
        if let Some(ctrl) = control
            && ctrl.checkpoint().is_err()
        {
            return Err(ExportError::Cancelled.to_string());
        }
        let path = dunce::canonicalize(p).map_err(|e| format!("{}: {e}", p.display()))?;
        canonical.push(path);
    }
    let Some((first, rest)) = canonical.split_first() else {
        return Err("Tidak ada file PDF untuk digabungkan.".into());
    };
    let mut workspace =
        pdf_application::open_document(renderer, first, SourceId(base_source_id))
            .map_err(|e| format!("{}: {e}", first.display()))?;
    let actual_first = pdf_platform::fingerprint(first)
        .map_err(|_| pdf_application::RenderError::SourceChanged.to_string())?;
    if actual_first != workspace.source().fingerprint {
        return Err(pdf_application::RenderError::SourceChanged.to_string());
    }
    for (offset, path) in rest.iter().enumerate() {
        if let Some(ctrl) = control
            && ctrl.checkpoint().is_err()
        {
            return Err(ExportError::Cancelled.to_string());
        }
        let info = renderer
            .inspect(path)
            .map_err(|e| format!("{}: {e}", path.display()))?;
        let actual = pdf_platform::fingerprint(&info.path)
            .map_err(|_| pdf_application::RenderError::SourceChanged.to_string())?;
        if actual != info.fingerprint {
            return Err(pdf_application::RenderError::SourceChanged.to_string());
        }
        let source = SourceDocument {
            id: SourceId(base_source_id.saturating_add((offset as u64) + 1)),
            path: info.path,
            fingerprint: info.fingerprint,
            geometries: info.pages,
            export_support: pdf_application::ExportSupport::NotPreflighted,
            annotations: info.annotations,
        };
        workspace
            .execute(Command::Import(source))
            .map_err(|e| format!("{}: {e}", path.display()))?;
    }
    let count = canonical.len();
    Ok((workspace, first.clone(), count))
}

impl WindowShell {
    pub(super) fn open_pdf_with_action(
        &mut self,
        path: PathBuf,
        action: ipc::PdfShellAction,
        cx: &mut Context<Self>,
    ) {
        if self.opening || self.close.is_some() || self.modal(cx) {
            self.notice = Some(
                "Selesaikan dialog atau proses ekspor yang sedang berjalan terlebih dahulu.".into(),
            );
            cx.notify();
            return;
        }
        self.pending_pdf_action = Some(action);
        self.open_paths(vec![path], cx);
    }

    pub(super) fn trigger_pending_pdf_action(&mut self, cx: &mut Context<Self>) {
        let Some(action) = self.pending_pdf_action.take() else {
            return;
        };
        let handle = self.handle;
        let weak = cx.weak_entity();
        cx.defer(move |cx| {
            let _ = handle.update(cx, |_, window, cx| {
                let _ = weak.update(cx, |this, cx| {
                    let Some(session) = this.session() else {
                        return;
                    };
                    session.update(cx, |s, cx| match action {
                        ipc::PdfShellAction::Compress => {
                            s.start_export(ExportScope::Optimized, cx);
                        }
                        ipc::PdfShellAction::Split => {
                            s.show_split(window, cx);
                        }
                        ipc::PdfShellAction::Png => {
                            s.show_png(window, cx);
                        }
                        ipc::PdfShellAction::Ocr => {
                            s.start_ocr_batch(cx);
                        }
                    });
                });
            });
        });
    }

    pub(super) fn combine_pdfs_in_organizer(
        &mut self,
        raw_paths: Vec<PathBuf>,
        cx: &mut Context<Self>,
    ) {
        if self.opening || self.close.is_some() || self.modal(cx) {
            self.notice = Some(
                "Selesaikan dialog atau proses ekspor yang sedang berjalan terlebih dahulu.".into(),
            );
            cx.notify();
            return;
        }
        let pdfs: Vec<PathBuf> = raw_paths
            .into_iter()
            .filter(|p| is_pdf_extension(p))
            .collect();
        if pdfs.is_empty() {
            self.notice = Some("Tidak ada file PDF yang dipilih.".into());
            cx.notify();
            return;
        }
        if self.tabs.len() >= MAX_TABS {
            self.notice = Some(
                "Maksimal 8 tab. Tutup salah satu tab terlebih dahulu untuk menggabungkan PDF."
                    .into(),
            );
            cx.notify();
            return;
        }
        self.session_offer = None;
        self.session_offer_dismissed = true;
        self.opening = true;
        let id = TabId(self.next_id);
        self.next_id += 1;
        let placeholder_path = pdfs[0].clone();
        self.tabs.push(Tab {
            id,
            path: placeholder_path,
            session: None,
            secondary: None,
            preferred_secondary: false,
        });
        self.resources
            .tab_count
            .store(self.tabs.len(), Ordering::SeqCst);
        let renderer = self.renderer.clone();
        let base_source_id = id.0 << 32;
        let task = cx
            .background_executor()
            .spawn(async move { build_combined_workspace(&renderer, &pdfs, base_source_id, None) });
        let handle = self.handle;
        cx.spawn(async move |this, cx| {
            let result = task.await;
            let _ = handle.update(cx, |_, window, cx| {
                let _ = this.update(cx, |this, cx| {
                    this.opening = false;
                    match result {
                        Ok((workspace, first_path, count)) => {
                            let pages = workspace.pages().len();
                            let owner = cx.weak_entity();
                            let session = DocumentSession::new(
                                window,
                                cx,
                                &this.sidecars,
                                this.renderer.clone(),
                                this.page_budget,
                                this.thumbnail_budget,
                                this.metrics.clone(),
                                owner,
                                this.resources.clone(),
                                this.ocr.clone(),
                                id,
                                ViewId(id.0 * 2),
                            );
                            session.update(cx, |s, cx| {
                                s.install_workspace(workspace, cx);
                                s.set_mode(ViewerMode::Organize, window, cx);
                            });
                            this.observe_document_view(&session, id, true, cx);
                            if let Some(tab) = this.tabs.iter_mut().find(|t| t.id == id) {
                                tab.path = first_path;
                                tab.session = Some(session);
                            }
                            this.resources
                                .tab_count
                                .store(this.tabs.len(), Ordering::SeqCst);
                            this.activate(id, cx);
                            this.notice = Some(format!(
                                "{count} PDF siap digabungkan di mode Atur ({pages} halaman). Pilih Simpan sebagai… untuk menyimpan file baru."
                            ));
                        }
                        Err(error) => {
                            this.tabs.retain(|t| t.id != id);
                            this.resources
                                .tab_count
                                .store(this.tabs.len(), Ordering::SeqCst);
                            this.notice = Some(format!("Gabung PDF gagal: {error}"));
                        }
                    }
                    this.report(cx);
                    cx.notify();
                });
            });
        })
        .detach();
        cx.notify();
    }

    pub(super) fn quick_combine_pdfs(&mut self, raw_paths: Vec<PathBuf>, cx: &mut Context<Self>) {
        if !self.can_start_images_to_pdf(cx) {
            self.notice = Some(
                "Selesaikan dialog atau proses ekspor yang sedang berjalan terlebih dahulu.".into(),
            );
            cx.notify();
            return;
        }
        let pdfs: Vec<PathBuf> = raw_paths
            .into_iter()
            .filter(|p| is_pdf_extension(p))
            .collect();
        if pdfs.len() < 2 {
            self.notice = Some("Pilih minimal 2 file PDF untuk digabungkan.".into());
            cx.notify();
            return;
        }
        let Some(destination) = compute_quick_merge_destination(&pdfs) else {
            self.notice = Some(
                "Tidak dapat menentukan file PDF tujuan yang bebas konflik di folder sumber."
                    .into(),
            );
            cx.notify();
            return;
        };
        if self
            .resources
            .export_owner
            .compare_exchange(0, SHELL_EXPORT_OWNER, Ordering::SeqCst, Ordering::SeqCst)
            .is_err()
        {
            return;
        }
        let control = ExportControl::with_budget(std::time::Duration::from_secs(600));
        self.pdf_merge.job_seq += 1;
        let job_id = self.pdf_merge.job_seq;
        self.pdf_merge.job = Some(PdfMergeJob {
            files: pdfs.len(),
            control: control.clone(),
            job_id,
        });
        self.pdf_merge.success = false;
        self.pdf_merge.pages = 0;
        self.notice = None;
        self.report(cx);
        cx.notify();

        cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(std::time::Duration::from_millis(150))
                    .await;
                let active = this
                    .update(cx, |this, cx| {
                        let same = this
                            .pdf_merge
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
        let exporter = qpdf_adapter::QpdfExporter::new(&self.sidecars);
        cx.spawn(async move |this, cx| {
            let result: Result<ExportReceipt, String> = cx
                .background_executor()
                .spawn(async move {
                    let (workspace, _, _) =
                        build_combined_workspace(&renderer, &pdfs, 1u64 << 32, Some(&control))?;
                    let plan = ExportPlan::capture(&workspace, 0, ExportScope::All)
                        .map_err(|e| e.to_string())?;
                    let prepared =
                        prepare_export(plan, &destination, &LocalOutput, &exporter, &control)
                            .map_err(|e| e.to_string())?;
                    finish_export(prepared, true, &exporter, &renderer, &control)
                        .map_err(|e| e.to_string())
                })
                .await;
            let _ = this.update(cx, |this, cx| this.finish_quick_combine_pdfs(result, cx));
        })
        .detach();
    }

    pub(super) fn cancel_quick_combine_pdfs(&mut self, cx: &mut Context<Self>) {
        if let Some(job) = &self.pdf_merge.job {
            job.control.cancel();
        }
        cx.notify();
    }

    fn finish_quick_combine_pdfs(
        &mut self,
        result: Result<ExportReceipt, String>,
        cx: &mut Context<Self>,
    ) {
        self.pdf_merge.job = None;
        let _ = self.resources.export_owner.compare_exchange(
            SHELL_EXPORT_OWNER,
            0,
            Ordering::SeqCst,
            Ordering::SeqCst,
        );
        self.pdf_merge.completed += 1;
        self.pdf_merge.success = result.is_ok();
        match result {
            Ok(receipt) => {
                self.pdf_merge.pages = receipt.page_count;
                self.notice = Some(format!(
                    "PDF gabungan ({} halaman) tersimpan di {}.",
                    receipt.page_count,
                    receipt.destination.display()
                ));
                if self.close.is_none() && self.tabs.len() < MAX_TABS {
                    self.open_paths(vec![receipt.destination], cx);
                }
            }
            Err(error) => {
                self.notice = Some(format!("Gabung PDF gagal: {error}"));
            }
        }
        if self.close.is_some() {
            self.advance_close(cx);
        }
        self.report(cx);
        cx.notify();
    }
}

#[cfg(windows)]
fn run_powershell(args: &[&str]) -> Result<String, String> {
    use std::os::windows::process::CommandExt as _;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    let output = std::process::Command::new("powershell.exe")
        .args(["-NoProfile", "-ExecutionPolicy", "Bypass"])
        .args(args)
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .map_err(|e| format!("Gagal menjalankan PowerShell: {e}"))?;
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_owned())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_owned();
        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_owned();
        let detail = if !stderr.is_empty() { stderr } else { stdout };
        Err(if detail.is_empty() {
            format!("Perintah gagal dengan kode {:?}", output.status.code())
        } else {
            detail
        })
    }
}

#[cfg(windows)]
fn query_shell_extension_status() -> (bool, Option<String>) {
    let script = "$p = Get-AppxPackage -Name 'Origo.ShellExtension' -ErrorAction SilentlyContinue; if ($p) { $p.Version }";
    match run_powershell(&["-Command", script]) {
        Ok(ver) if !ver.is_empty() => (true, Some(ver)),
        _ => (false, None),
    }
}

#[cfg(windows)]
fn find_dev_register_script(exe: &std::path::Path) -> Option<PathBuf> {
    let mut cur = exe.parent();
    for _ in 0..4 {
        if let Some(dir) = cur {
            let candidate = dir.join("scripts").join("register-shell-ext-dev.ps1");
            if candidate.is_file() {
                return Some(candidate);
            }
            cur = dir.parent();
        } else {
            break;
        }
    }
    None
}

#[cfg(windows)]
fn install_shell_extension() -> Result<String, String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let exe_dir = exe
        .parent()
        .ok_or_else(|| "Folder aplikasi Origo tidak ditemukan.".to_owned())?;
    // Prefer bundled signed MSIX next to pdf-studio.exe if available.
    let bundled_msix = exe_dir.join("OrigoShellExt.msix");
    if bundled_msix.is_file() {
        let msix = bundled_msix.to_string_lossy().replace('\'', "''");
        let ext_loc = exe_dir.to_string_lossy().replace('\'', "''");
        let cmd = format!(
            "$e = Get-AppxPackage -Name 'Origo.ShellExtension' -ErrorAction SilentlyContinue; if ($e) {{ $e | Remove-AppxPackage }}; Add-AppxPackage -Path '{msix}' -ExternalLocation '{ext_loc}'"
        );
        run_powershell(&["-Command", &cmd])?;
        return Ok("Menu klik kanan Windows 11 Explorer berhasil dipasang.".into());
    }
    // Fallback for development builds running inside the repository.
    if let Some(script) = find_dev_register_script(&exe) {
        let profile = if exe_dir
            .file_name()
            .is_some_and(|n| n.eq_ignore_ascii_case("release"))
        {
            "release"
        } else {
            "debug"
        };
        let script_str = script.to_string_lossy().into_owned();
        let exe_str = exe.to_string_lossy().into_owned();
        run_powershell(&[
            "-File",
            &script_str,
            "-Profile",
            profile,
            "-TargetExe",
            &exe_str,
            "-SkipBuild",
        ])?;
        return Ok("Menu klik kanan Windows 11 Explorer (mode dev) berhasil diperbarui.".into());
    }
    Err(
        "Paket ekstensi shell (OrigoShellExt.msix) tidak ditemukan di folder instalasi ini."
            .into(),
    )
}

#[cfg(windows)]
fn uninstall_shell_extension() -> Result<String, String> {
    let cmd = "$e = Get-AppxPackage -Name 'Origo.ShellExtension' -ErrorAction SilentlyContinue; if ($e) { $e | Remove-AppxPackage }";
    run_powershell(&["-Command", cmd])?;
    Ok("Menu klik kanan Windows 11 Explorer berhasil dicopot.".into())
}

#[cfg(windows)]
pub(crate) fn unregister_shell_extension_silently() {
    let _ = uninstall_shell_extension();
}

#[cfg(windows)]
impl WindowShell {
    pub(super) fn show_shell_integration_dialog(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.opening || self.close.is_some() || self.modal(cx) {
            return;
        }
        self.tab_picker_open = false;
        self.shell_ext_dialog = Some(ShellExtDialog {
            loading: true,
            busy: false,
            installed: false,
            version: None,
            message: None,
        });
        self.resources
            .modal_owner
            .store(SHELL_EXPORT_OWNER, Ordering::SeqCst);

        let task = cx
            .background_executor()
            .spawn(async move { query_shell_extension_status() });
        let handle = self.handle;
        cx.spawn(async move |this, cx| {
            let (installed, version) = task.await;
            let _ = handle.update(cx, |_, window, cx| {
                let _ = this.update(cx, |this, cx| {
                    if let Some(dlg) = &mut this.shell_ext_dialog {
                        dlg.loading = false;
                        dlg.installed = installed;
                        dlg.version = version;
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
                .read_with(cx, |this, _| this.shell_ext_dialog.clone())
                .ok()
                .flatten()
            else {
                return dialog;
            };
            let colors = cx.theme().colors;
            let status_text = if ui.loading {
                "Memeriksa status menu klik kanan Explorer…".to_owned()
            } else if ui.installed {
                match &ui.version {
                    Some(v) => format!("Aktif di Windows 11 File Explorer (v{v})"),
                    None => "Aktif di Windows 11 File Explorer".to_owned(),
                }
            } else {
                "Belum terpasang di Windows 11 File Explorer".to_owned()
            };

            let install_weak = weak.clone();
            let remove_weak = weak.clone();
            let close_weak = weak.clone();
            let closed_weak = weak.clone();

            let mut body = gpui::div()
                .flex()
                .flex_col()
                .gap_3()
                .child(
                    gpui::div()
                        .rounded_md()
                        .border_1()
                        .border_color(colors.border)
                        .bg(colors.secondary)
                        .p_3()
                        .flex()
                        .flex_col()
                        .gap_1()
                        .child(gpui::div().text_sm().font_semibold().child(status_text))
                        .child(
                            gpui::div()
                                .text_xs()
                                .text_color(colors.muted_foreground)
                                .child(
                                    "Mendukung klik kanan file gambar (PNG/JPEG) dan PDF (Buka, Kompres, Pisahkan, Ekspor PNG, OCR, dan Gabungkan 2+ PDF).",
                                ),
                        ),
                );
            if let Some(msg) = &ui.message {
                body = body.child(
                    gpui::div()
                        .text_xs()
                        .text_color(colors.foreground)
                        .child(msg.clone()),
                );
            }

            dialog
                .title(
                    gpui::div()
                        .id("shell-ext-title")
                        .role(gpui::Role::Group)
                        .aria_label("Integrasi Menu Explorer")
                        .child("Integrasi Menu Explorer"),
                )
                .description(
                    "Kelola submenu klik kanan Origo pada Windows 11 File Explorer tanpa perlu restart Windows.",
                )
                .centered()
                .width(px(520.))
                .close_button(true)
                .child(body)
                .on_close(move |_, _, cx| {
                    let _ = closed_weak.update(cx, |this, cx| this.close_shell_ext_dialog(cx));
                })
                .footer(
                    DialogFooter::new()
                        .child(
                            Button::new("shell-ext-close")
                                .label("Tutup")
                                .disabled(ui.busy)
                                .on_click(move |_, window, cx| {
                                    window.close_dialog(cx);
                                    let _ = close_weak
                                        .update(cx, |this, cx| this.close_shell_ext_dialog(cx));
                                }),
                        )
                        .when(ui.installed, |footer| {
                            footer.child(
                                Button::new("shell-ext-remove")
                                    .ghost()
                                    .label("Copot dari Explorer")
                                    .disabled(ui.loading || ui.busy)
                                    .on_click(move |_, window, cx| {
                                        let _ = remove_weak.update(cx, |this, cx| {
                                            this.run_shell_ext_action(false, window, cx)
                                        });
                                    }),
                            )
                        })
                        .child(
                            Button::new("shell-ext-install")
                                .primary()
                                .label(if ui.installed {
                                    "Perbarui registrasi"
                                } else {
                                    "Pasang ke Explorer"
                                })
                                .disabled(ui.loading || ui.busy)
                                .on_click(move |_, window, cx| {
                                    let _ = install_weak.update(cx, |this, cx| {
                                        this.run_shell_ext_action(true, window, cx)
                                    });
                                }),
                        ),
                )
        });
        cx.notify();
    }

    fn run_shell_ext_action(&mut self, install: bool, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(dlg) = &mut self.shell_ext_dialog {
            if dlg.busy {
                return;
            }
            dlg.busy = true;
            dlg.message = Some(if install {
                "Memasang ekstensi menu klik kanan…".into()
            } else {
                "Mencopot ekstensi menu klik kanan…".into()
            });
        }
        window.refresh();
        cx.notify();

        let task = cx.background_executor().spawn(async move {
            let outcome = if install {
                install_shell_extension()
            } else {
                uninstall_shell_extension()
            };
            let status = query_shell_extension_status();
            (outcome, status)
        });
        let handle = self.handle;
        cx.spawn(async move |this, cx| {
            let (outcome, (installed, version)) = task.await;
            let _ = handle.update(cx, |_, window, cx| {
                let _ = this.update(cx, |this, cx| {
                    if let Some(dlg) = &mut this.shell_ext_dialog {
                        dlg.busy = false;
                        dlg.loading = false;
                        dlg.installed = installed;
                        dlg.version = version;
                        dlg.message = Some(match outcome {
                            Ok(msg) => msg,
                            Err(err) => format!("Gagal: {err}"),
                        });
                    }
                    window.refresh();
                    cx.notify();
                });
            });
        })
        .detach();
    }

    fn close_shell_ext_dialog(&mut self, cx: &mut Context<Self>) {
        self.shell_ext_dialog = None;
        let _ = self.resources.modal_owner.compare_exchange(
            SHELL_EXPORT_OWNER,
            0,
            Ordering::SeqCst,
            Ordering::SeqCst,
        );
        self.report(cx);
        cx.notify();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quick_merge_destination_picks_non_colliding_names() {
        let dir = tempfile::tempdir().ok();
        assert!(dir.is_some());
        let Some(dir) = dir else { return };
        let sub = dir.path().join("Laporan Q4");
        assert!(std::fs::create_dir_all(&sub).is_ok());

        let pdfs = vec![sub.join("01.pdf"), sub.join("02.pdf")];
        assert_eq!(
            compute_quick_merge_destination(&pdfs),
            Some(sub.join("Laporan Q4.pdf"))
        );

        assert!(std::fs::write(sub.join("Laporan Q4.pdf"), b"%PDF-1.4").is_ok());
        assert_eq!(
            compute_quick_merge_destination(&pdfs),
            Some(sub.join("Laporan Q4-2.pdf"))
        );
    }
}
