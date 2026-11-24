//! Installed Windows builds check their packaged feed outside the UI thread
//! and prompt the user before downloading any update package.
use anyhow::Context as _;
use gpui_kit::{
    AnyWindowHandle, App, AppContext, Context, Entity, InteractiveElement, IntoElement,
    ParentElement, Render, StatefulInteractiveElement, Styled, Subscription, WeakEntity, Window,
    component::{
        Sizable, WindowExt,
        button::{Button, ButtonVariants},
        notification::{Notification, NotificationType},
    },
    div,
};
use std::{
    fs::{File, OpenOptions, TryLockError},
    path::{Path, PathBuf},
    sync::mpsc::TryRecvError,
    time::Duration,
};
use velopack::{
    UpdateCheck, UpdateInfo, UpdateManager, VelopackApp,
    sources::{FileSource, GithubSource, HttpSource, NoneSource},
};

const PREVIEW_ID: &str = "Origo.Preview";
const POLL_INTERVAL: Duration = Duration::from_secs(30 * 60);
const PROGRESS_POLL_INTERVAL: Duration = Duration::from_millis(150);

struct UpdateNoticeId;
struct ActiveVersionNoticeId;

pub(super) enum Startup {
    Unmanaged,
    Installed(Installed),
    Exit,
}

pub(super) struct Installed {
    manager: UpdateManager,
    _instance: File,
    restarted: bool,
    pub state_dir: Option<PathBuf>,
}

pub(super) fn start(executable: &Path) -> anyhow::Result<Startup> {
    let mut restarted = false;
    VelopackApp::build()
        .set_auto_apply_on_startup(false)
        .on_restarted(|_| restarted = true)
        .run();
    let Some(bin) = executable.parent() else {
        return Ok(Startup::Unmanaged);
    };
    let feed = match std::fs::read_to_string(bin.join("origo-update-feed.txt")) {
        Ok(feed) => feed,
        Err(_) => return Ok(Startup::Unmanaged),
    };
    let Ok(probe) = UpdateManager::new(NoneSource {}, None, None) else {
        return Ok(Startup::Unmanaged);
    };
    let id = probe.get_app_id();
    let preview = id == PREVIEW_ID || id == "Origo.Preview.Fixture";
    if probe.get_is_portable() {
        return Ok(Startup::Unmanaged);
    }
    let Some(target) = classify_feed(&feed, preview) else {
        return Ok(Startup::Unmanaged);
    };
    let manager = match target {
        FeedTarget::GithubRepo(url) => {
            UpdateManager::new(GithubSource::new(url, None, preview), None, None)
        }
        FeedTarget::Https(url) => UpdateManager::new(HttpSource::new(url), None, None),
        FeedTarget::LocalDir(path) => UpdateManager::new(FileSource::new(path), None, None),
    };
    let Ok(manager) = manager else {
        return Ok(Startup::Unmanaged);
    };
    let root = bin
        .parent()
        .context("Folder instalasi Origo tidak ditemukan")?;
    let instance = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(false)
        .open(root.join("origo-instance.lock"))?;
    match instance.try_lock() {
        Ok(()) => {}
        Err(TryLockError::WouldBlock) => return Ok(Startup::Exit),
        Err(TryLockError::Error(error)) => return Err(error.into()),
    }

    // The previous instance must have exited before any pending update can be
    // applied. No document, export, or OCR job exists in this new process yet.
    if !restarted
        && let Some(asset) = manager.get_update_pending_restart()
        && manager
            .wait_exit_then_apply_updates(asset, true, true, std::env::args_os().skip(1))
            .is_ok()
    {
        return Ok(Startup::Exit);
    }
    Ok(Startup::Installed(Installed {
        manager,
        _instance: instance,
        restarted,
        state_dir: preview.then(|| root.join("UserData")),
    }))
}

impl Installed {
    pub fn version(&self) -> String {
        self.manager.get_current_version_as_string()
    }

    pub fn updater(&self) -> impl FnOnce(AnyWindowHandle, &mut App) -> Entity<Updater> + use<> {
        let manager = self.manager.clone();
        let restarted = self.restarted;
        move |window, cx| Updater::start(manager, restarted, window, cx)
    }
}

enum Phase {
    Idle,
    Available {
        update: Box<UpdateInfo>,
    },
    Downloading {
        version: String,
        progress: Option<u8>,
    },
    Ready {
        version: String,
    },
    Failed {
        update: Box<UpdateInfo>,
    },
}

enum CheckOutcome {
    PendingRestart(String),
    Available(Box<UpdateInfo>),
    UpToDate,
    Error,
}

pub(super) struct Updater {
    manager: UpdateManager,
    window: AnyWindowHandle,
    checking: bool,
    phase: Phase,
    dismissed_version: Option<String>,
}

impl Updater {
    fn start(
        manager: UpdateManager,
        restarted: bool,
        window: AnyWindowHandle,
        cx: &mut App,
    ) -> Entity<Self> {
        cx.new(|cx| {
            let updater = Self {
                manager: manager.clone(),
                window,
                checking: false,
                phase: Phase::Idle,
                dismissed_version: None,
            };
            if restarted {
                let message = format!("Origo {} sekarang aktif.", manager.get_current_version());
                let _ = window.update(cx, |_, window, cx| {
                    window.push_notification(active_version_notice(message), cx);
                });
            }
            cx.spawn(async move |weak: WeakEntity<Self>, cx| {
                loop {
                    let keep_polling = weak
                        .update(cx, |this, cx| {
                            if matches!(this.phase, Phase::Ready { .. }) {
                                return false;
                            }
                            this.check_for_updates(false, cx);
                            true
                        })
                        .unwrap_or(false);
                    if !keep_polling {
                        break;
                    }
                    cx.background_executor().timer(POLL_INTERVAL).await;
                }
            })
            .detach();
            updater
        })
    }

    pub(super) fn check_manually(&mut self, cx: &mut Context<Self>) {
        self.check_for_updates(true, cx);
    }

    fn check_for_updates(&mut self, manual: bool, cx: &mut Context<Self>) {
        if let Phase::Ready { version } = &self.phase {
            if manual {
                self.push_ready_notice(version.clone(), cx);
            }
            return;
        }
        if matches!(self.phase, Phase::Downloading { .. }) {
            if manual {
                self.push_downloading_notice(cx);
            }
            return;
        }
        if self.checking {
            if manual {
                self.push_status_notice(
                    "Memeriksa pembaruan…".into(),
                    NotificationType::Info,
                    false,
                    cx,
                );
            }
            return;
        }

        self.checking = true;
        if manual {
            self.push_status_notice(
                "Memeriksa pembaruan…".into(),
                NotificationType::Info,
                false,
                cx,
            );
        }

        let manager = self.manager.clone();
        cx.spawn(async move |weak: WeakEntity<Self>, cx| {
            let outcome = cx
                .background_executor()
                .spawn(async move {
                    if let Some(asset) = manager.get_update_pending_restart() {
                        return CheckOutcome::PendingRestart(asset.Version);
                    }
                    match manager.check_for_updates() {
                        Ok(UpdateCheck::UpdateAvailable(update)) => CheckOutcome::Available(update),
                        Ok(UpdateCheck::NoUpdateAvailable | UpdateCheck::RemoteIsEmpty) => {
                            CheckOutcome::UpToDate
                        }
                        Err(_) => CheckOutcome::Error,
                    }
                })
                .await;
            let _ = weak.update(cx, |this, cx| {
                this.finish_check(outcome, manual, cx);
            });
        })
        .detach();
    }

    fn finish_check(&mut self, outcome: CheckOutcome, manual: bool, cx: &mut Context<Self>) {
        self.checking = false;
        if matches!(self.phase, Phase::Downloading { .. } | Phase::Ready { .. }) {
            return;
        }
        match outcome {
            CheckOutcome::PendingRestart(version) => {
                self.dismissed_version = None;
                self.phase = Phase::Ready {
                    version: version.clone(),
                };
                self.push_ready_notice(version, cx);
            }
            CheckOutcome::Available(update) => {
                let version = update.TargetFullRelease.Version.clone();
                let prompted_version = match &self.phase {
                    Phase::Available { update } => Some(update.TargetFullRelease.Version.as_str()),
                    _ => None,
                };
                if !should_prompt_for_version(
                    &version,
                    self.dismissed_version.as_deref(),
                    prompted_version,
                    manual,
                ) {
                    return;
                }
                if manual {
                    self.dismissed_version = None;
                }
                let message = available_message(&version, download_size_bytes(&update));
                self.phase = Phase::Available { update };
                self.push_prompt_notice(version, message, "Unduh", NotificationType::Info, cx);
            }
            CheckOutcome::UpToDate => {
                if matches!(self.phase, Phase::Available { .. } | Phase::Failed { .. }) {
                    self.phase = Phase::Idle;
                    if !manual {
                        self.remove_update_notice(cx);
                    }
                }
                if manual {
                    let message = format!(
                        "Origo {} sudah versi terbaru.",
                        self.manager.get_current_version()
                    );
                    self.push_status_notice(message, NotificationType::Info, true, cx);
                }
            }
            CheckOutcome::Error => {
                if manual {
                    self.push_status_notice(
                        "Gagal memeriksa pembaruan.".into(),
                        NotificationType::Warning,
                        true,
                        cx,
                    );
                }
            }
        }
    }

    fn start_download(&mut self, cx: &mut Context<Self>) {
        let update = match &self.phase {
            Phase::Available { update } | Phase::Failed { update } => update.clone(),
            _ => return,
        };
        let version = update.TargetFullRelease.Version.clone();
        self.dismissed_version = None;
        self.phase = Phase::Downloading {
            version: version.clone(),
            progress: None,
        };
        cx.notify();
        self.push_downloading_notice(cx);

        let manager = self.manager.clone();
        cx.spawn(async move |weak: WeakEntity<Self>, cx| {
            let (tx, rx) = std::sync::mpsc::channel::<i16>();
            let progress_weak = weak.clone();
            cx.spawn(async move |cx| {
                loop {
                    let mut latest = None;
                    let mut disconnected = false;
                    loop {
                        match rx.try_recv() {
                            Ok(percent) => latest = Some(percent.clamp(0, 100) as u8),
                            Err(TryRecvError::Empty) => break,
                            Err(TryRecvError::Disconnected) => {
                                disconnected = true;
                                break;
                            }
                        }
                    }
                    if let Some(percent) = latest {
                        let _ = progress_weak.update(cx, |this, cx| {
                            this.set_progress(percent, cx);
                        });
                    }
                    if disconnected {
                        break;
                    }
                    cx.background_executor().timer(PROGRESS_POLL_INTERVAL).await;
                }
            })
            .detach();

            let download_update = update.clone();
            let result = cx
                .background_executor()
                .spawn(async move { manager.download_updates(&download_update, Some(tx)) })
                .await;
            let _ = weak.update(cx, |this, cx| match result {
                Ok(()) => this.finish_download_ok(version, cx),
                Err(_) => this.finish_download_err(update, cx),
            });
        })
        .detach();
    }

    fn set_progress(&mut self, percent: u8, cx: &mut Context<Self>) {
        if let Phase::Downloading { progress, .. } = &mut self.phase
            && *progress != Some(percent)
        {
            *progress = Some(percent);
            cx.notify();
        }
    }

    fn finish_download_ok(&mut self, version: String, cx: &mut Context<Self>) {
        self.phase = Phase::Ready {
            version: version.clone(),
        };
        cx.notify();
        self.push_ready_notice(version, cx);
    }

    fn finish_download_err(&mut self, update: Box<UpdateInfo>, cx: &mut Context<Self>) {
        let version = update.TargetFullRelease.Version.clone();
        let message = format!("Gagal mengunduh Origo {version}.");
        self.phase = Phase::Failed { update };
        cx.notify();
        self.push_prompt_notice(version, message, "Coba lagi", NotificationType::Warning, cx);
    }

    fn dismiss_prompt(&mut self, version: &str) {
        let matches_version = match &self.phase {
            Phase::Available { update } | Phase::Failed { update } => {
                update.TargetFullRelease.Version == version
            }
            _ => false,
        };
        if matches_version {
            self.dismissed_version = Some(version.to_owned());
            self.phase = Phase::Idle;
        }
    }

    fn downloading_message(&self) -> Option<String> {
        match &self.phase {
            Phase::Downloading { version, progress } => {
                Some(downloading_message(version, *progress))
            }
            _ => None,
        }
    }

    fn push_prompt_notice(
        &self,
        version: String,
        message: String,
        confirm_label: &'static str,
        kind: NotificationType,
        cx: &mut Context<Self>,
    ) {
        let window = self.window;
        let weak = cx.weak_entity();
        cx.defer(move |cx| {
            let _ = window.update(cx, |_, window, cx| {
                let on_confirm = weak.clone();
                let on_defer = weak.clone();
                let on_close = weak.clone();
                let defer_version = version.clone();
                let close_version = version.clone();
                let notification = Notification::new()
                    .id::<UpdateNoticeId>()
                    .with_type(kind)
                    .placement(gpui_kit::Anchor::BottomRight)
                    .autohide(false)
                    .on_close(move |_, cx| {
                        let _ = on_close.update(cx, |this, _| this.dismiss_prompt(&close_version));
                    })
                    .content(move |_, _, _| {
                        let on_confirm = on_confirm.clone();
                        let on_defer = on_defer.clone();
                        let defer_version = defer_version.clone();
                        div()
                            .flex()
                            .flex_col()
                            .gap_2()
                            .child(
                                div()
                                    .id("update-notification")
                                    .role(gpui_kit::Role::Alert)
                                    .aria_label(message.clone())
                                    .text_sm()
                                    .child(message.clone()),
                            )
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap_2()
                                    .child(
                                        Button::new("update-confirm-download")
                                            .small()
                                            .primary()
                                            .label(confirm_label)
                                            .on_click(move |_, _, cx| {
                                                let _ = on_confirm.update(cx, |this, cx| {
                                                    this.start_download(cx);
                                                });
                                            }),
                                    )
                                    .child(
                                        Button::new("update-defer-download")
                                            .small()
                                            .ghost()
                                            .label("Nanti")
                                            .on_click(move |_, window, cx| {
                                                let _ = on_defer.update(cx, |this, _| {
                                                    this.dismiss_prompt(&defer_version);
                                                });
                                                window.remove_notification::<UpdateNoticeId>(cx);
                                            }),
                                    ),
                            )
                            .into_any_element()
                    });
                window.push_notification(notification, cx);
            });
        });
    }

    fn push_downloading_notice(&self, cx: &mut Context<Self>) {
        let window = self.window;
        let updater = cx.entity();
        cx.defer(move |cx| {
            let _ = window.update(cx, |_, window, cx| {
                let progress_view = cx.new(|cx| DownloadProgressView::new(updater, cx));
                let notification = Notification::new()
                    .id::<UpdateNoticeId>()
                    .with_type(NotificationType::Info)
                    .placement(gpui_kit::Anchor::BottomRight)
                    .autohide(false)
                    .content(move |_, _, _| progress_view.clone().into_any_element());
                window.push_notification(notification, cx);
            });
        });
    }

    fn push_ready_notice(&self, version: String, cx: &mut Context<Self>) {
        let message =
            format!("Origo {version} siap dipasang. Keluar dan buka lagi untuk memperbarui.");
        self.push_status_notice(message, NotificationType::Info, false, cx);
    }

    fn push_status_notice(
        &self,
        message: String,
        kind: NotificationType,
        autohide: bool,
        cx: &mut Context<Self>,
    ) {
        let window = self.window;
        cx.defer(move |cx| {
            let _ = window.update(cx, |_, window, cx| {
                window.push_notification(status_notice(message, kind, autohide), cx);
            });
        });
    }

    fn remove_update_notice(&self, cx: &mut Context<Self>) {
        let window = self.window;
        cx.defer(move |cx| {
            let _ = window.update(cx, |_, window, cx| {
                window.remove_notification::<UpdateNoticeId>(cx);
            });
        });
    }
}

struct DownloadProgressView {
    updater: Entity<Updater>,
    _observe: Subscription,
}

impl DownloadProgressView {
    fn new(updater: Entity<Updater>, cx: &mut Context<Self>) -> Self {
        let observe = cx.observe(&updater, |_, _, cx| cx.notify());
        Self {
            updater,
            _observe: observe,
        }
    }
}

impl Render for DownloadProgressView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let message = self
            .updater
            .read(cx)
            .downloading_message()
            .unwrap_or_else(|| "Mengunduh pembaruan…".into());
        div()
            .id("update-notification")
            .role(gpui_kit::Role::Alert)
            .aria_label(message.clone())
            .text_sm()
            .child(message)
    }
}

fn active_version_notice(message: String) -> Notification {
    Notification::new()
        .id::<ActiveVersionNoticeId>()
        .with_type(NotificationType::Success)
        .placement(gpui_kit::Anchor::BottomRight)
        .autohide(true)
        .content(move |_, _, _| {
            div()
                .id("update-active-notification")
                .role(gpui_kit::Role::Alert)
                .aria_label(message.clone())
                .text_sm()
                .child(message.clone())
                .into_any_element()
        })
}

fn status_notice(message: String, kind: NotificationType, autohide: bool) -> Notification {
    Notification::new()
        .id::<UpdateNoticeId>()
        .with_type(kind)
        .placement(gpui_kit::Anchor::BottomRight)
        .autohide(autohide)
        .content(move |_, _, _| {
            div()
                .id("update-notification")
                .role(gpui_kit::Role::Alert)
                .aria_label(message.clone())
                .text_sm()
                .child(message.clone())
                .into_any_element()
        })
}

fn download_size_bytes(update: &UpdateInfo) -> u64 {
    if update.BaseRelease.is_some() && !update.DeltasToTarget.is_empty() {
        let delta: u64 = update.DeltasToTarget.iter().map(|asset| asset.Size).sum();
        if delta > 0 {
            return delta;
        }
    }
    update.TargetFullRelease.Size
}

fn format_download_size(bytes: u64) -> Option<String> {
    if bytes == 0 {
        return None;
    }
    if bytes < 1024 {
        Some(format!("{bytes} B"))
    } else if bytes < 1024 * 1024 {
        Some(format!("{:.1} KiB", bytes as f64 / 1024.))
    } else {
        Some(format!("{:.1} MiB", bytes as f64 / (1024. * 1024.)))
    }
}

fn available_message(version: &str, bytes: u64) -> String {
    match format_download_size(bytes) {
        Some(size) => format!("Origo {version} tersedia ({size}). Unduh pembaruan sekarang?"),
        None => format!("Origo {version} tersedia. Unduh pembaruan sekarang?"),
    }
}

fn downloading_message(version: &str, progress: Option<u8>) -> String {
    match progress {
        Some(percent) if percent > 0 => format!("Mengunduh Origo {version}… {percent}%"),
        _ => format!("Mengunduh Origo {version}…"),
    }
}

fn should_prompt_for_version(
    version: &str,
    dismissed: Option<&str>,
    already_prompted: Option<&str>,
    manual: bool,
) -> bool {
    if manual {
        return true;
    }
    dismissed != Some(version) && already_prompted != Some(version)
}

#[derive(Debug, PartialEq, Eq)]
enum FeedTarget<'a> {
    GithubRepo(&'a str),
    Https(&'a str),
    LocalDir(&'a str),
}

fn classify_feed(feed: &str, allow_local: bool) -> Option<FeedTarget<'_>> {
    let feed = feed.trim();
    if feed.is_empty() {
        return None;
    }
    if let Some(rest) = feed.strip_prefix("https://") {
        let trimmed = rest.trim_end_matches('/');
        if trimmed.is_empty() || trimmed.contains(char::is_whitespace) {
            return None;
        }
        let authority = trimmed.split('/').next().unwrap_or("");
        if authority.is_empty() || authority.contains('@') {
            return None;
        }
        if is_github_repo_path(trimmed) {
            return Some(FeedTarget::GithubRepo(feed.trim_end_matches('/')));
        }
        return Some(FeedTarget::Https(feed));
    }
    if allow_local && Path::new(feed).is_absolute() {
        return Some(FeedTarget::LocalDir(feed));
    }
    None
}

fn is_github_repo_path(https_rest_trimmed: &str) -> bool {
    if https_rest_trimmed.contains('?') || https_rest_trimmed.contains('#') {
        return false;
    }
    let mut segments = https_rest_trimmed.split('/');
    let Some(host) = segments.next() else {
        return false;
    };
    if !host.eq_ignore_ascii_case("github.com") {
        return false;
    }
    let (Some(owner), Some(repo), None) = (segments.next(), segments.next(), segments.next())
    else {
        return false;
    };
    is_github_segment(owner) && is_github_segment(repo) && !repo.ends_with(".git")
}

fn is_github_segment(segment: &str) -> bool {
    !segment.is_empty()
        && segment != "."
        && segment != ".."
        && segment
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.'))
}

#[cfg(test)]
mod tests {
    use super::*;
    use velopack::VelopackAsset;

    fn asset_with_size(version: &str, size: u64) -> VelopackAsset {
        VelopackAsset {
            Version: version.into(),
            Size: size,
            ..Default::default()
        }
    }

    #[test]
    fn classifies_github_https_and_local_update_feeds() {
        assert_eq!(
            classify_feed("https://github.com/zufarhandito/origo", true),
            Some(FeedTarget::GithubRepo(
                "https://github.com/zufarhandito/origo"
            ))
        );
        assert_eq!(
            classify_feed("  https://github.com/zufarhandito/origo/ \n", false),
            Some(FeedTarget::GithubRepo(
                "https://github.com/zufarhandito/origo"
            ))
        );
        assert_eq!(
            classify_feed("https://updates.example.com/origo-preview/", false),
            Some(FeedTarget::Https(
                "https://updates.example.com/origo-preview/"
            ))
        );
        assert_eq!(
            classify_feed(
                "https://github.com/zufarhandito/origo/releases/latest/download",
                false
            ),
            Some(FeedTarget::Https(
                "https://github.com/zufarhandito/origo/releases/latest/download"
            ))
        );
        assert_eq!(
            classify_feed(r"D:\projects\bpdf\dist\origo-preview", true),
            Some(FeedTarget::LocalDir(r"D:\projects\bpdf\dist\origo-preview"))
        );
        assert_eq!(
            classify_feed(r"D:\projects\bpdf\dist\origo-preview", false),
            None
        );
        assert_eq!(classify_feed("http://updates.example.com/feed", true), None);
        assert_eq!(
            classify_feed("https://user@github.com/zufarhandito/origo", true),
            None
        );
        assert_eq!(
            classify_feed("https://github.com/./origo", true),
            Some(FeedTarget::Https("https://github.com/./origo"))
        );
        assert_eq!(
            classify_feed("https://github.com/zufarhandito/..", true),
            Some(FeedTarget::Https("https://github.com/zufarhandito/.."))
        );
        assert_eq!(classify_feed("relative/feed", true), None);
        assert_eq!(classify_feed("   ", true), None);
    }

    #[test]
    fn download_size_prefers_deltas_when_present() {
        let full = UpdateInfo {
            TargetFullRelease: asset_with_size("0.1.4", 50 * 1024 * 1024),
            BaseRelease: None,
            DeltasToTarget: Vec::new(),
            IsDowngrade: false,
        };
        assert_eq!(download_size_bytes(&full), 50 * 1024 * 1024);

        let delta = UpdateInfo {
            TargetFullRelease: asset_with_size("0.1.4", 50 * 1024 * 1024),
            BaseRelease: Some(asset_with_size("0.1.3", 48 * 1024 * 1024)),
            DeltasToTarget: vec![
                asset_with_size("0.1.4", 3 * 1024 * 1024),
                asset_with_size("0.1.4", 512 * 1024),
            ],
            IsDowngrade: false,
        };
        assert_eq!(download_size_bytes(&delta), 3 * 1024 * 1024 + 512 * 1024);
    }

    #[test]
    fn formats_update_messages_and_sizes() {
        assert_eq!(format_download_size(0), None);
        assert_eq!(format_download_size(512).as_deref(), Some("512 B"));
        assert_eq!(format_download_size(1536).as_deref(), Some("1.5 KiB"));
        assert_eq!(
            format_download_size(47_729_471).as_deref(),
            Some("45.5 MiB")
        );

        assert_eq!(
            available_message("0.1.4", 0),
            "Origo 0.1.4 tersedia. Unduh pembaruan sekarang?"
        );
        assert_eq!(
            available_message("0.1.4", 47_729_471),
            "Origo 0.1.4 tersedia (45.5 MiB). Unduh pembaruan sekarang?"
        );
        assert_eq!(downloading_message("0.1.4", None), "Mengunduh Origo 0.1.4…");
        assert_eq!(
            downloading_message("0.1.4", Some(0)),
            "Mengunduh Origo 0.1.4…"
        );
        assert_eq!(
            downloading_message("0.1.4", Some(45)),
            "Mengunduh Origo 0.1.4… 45%"
        );
    }

    #[test]
    fn respects_dismissed_and_prompted_versions_until_manual_or_newer() {
        assert!(should_prompt_for_version("0.1.4", None, None, false));
        assert!(!should_prompt_for_version(
            "0.1.4",
            Some("0.1.4"),
            None,
            false
        ));
        assert!(!should_prompt_for_version(
            "0.1.4",
            None,
            Some("0.1.4"),
            false
        ));
        assert!(should_prompt_for_version(
            "0.1.5",
            Some("0.1.4"),
            None,
            false
        ));
        assert!(should_prompt_for_version(
            "0.1.5",
            None,
            Some("0.1.4"),
            false
        ));
        assert!(should_prompt_for_version(
            "0.1.4",
            Some("0.1.4"),
            None,
            true
        ));
        assert!(should_prompt_for_version(
            "0.1.4",
            None,
            Some("0.1.4"),
            true
        ));
    }
}
