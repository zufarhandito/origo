//! Installed Windows builds check their packaged feed outside the UI thread.
use anyhow::Context as _;
use gpui_kit::{
    AnyWindowHandle, App, InteractiveElement, IntoElement, ParentElement,
    StatefulInteractiveElement, Styled,
    component::{WindowExt, notification::Notification},
    div,
};
use std::{
    fs::{File, OpenOptions, TryLockError},
    path::{Path, PathBuf},
    time::Duration,
};
use velopack::{
    UpdateCheck, UpdateManager, VelopackApp,
    sources::{FileSource, HttpSource},
};

const PREVIEW_ID: &str = "Origo.Preview";

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
    let feed = feed.trim();
    let manager = if feed.starts_with("https://") {
        UpdateManager::new(HttpSource::new(feed), None, None)
    } else if Path::new(feed).is_absolute() {
        UpdateManager::new(FileSource::new(feed), None, None)
    } else {
        return Ok(Startup::Unmanaged);
    };
    let Ok(manager) = manager else {
        return Ok(Startup::Unmanaged);
    };
    let id = manager.get_app_id();
    let preview = id == PREVIEW_ID || id == "Origo.Preview.Fixture";
    if manager.get_is_portable() || (!preview && !feed.starts_with("https://")) {
        return Ok(Startup::Unmanaged);
    }
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

    pub fn notifications(&self) -> impl FnOnce(AnyWindowHandle, &mut App) + use<> {
        let manager = self.manager.clone();
        let restarted = self.restarted;
        move |window, cx| {
            cx.spawn(async move |cx| {
                if restarted {
                    let message =
                        format!("Origo {} sekarang aktif.", manager.get_current_version());
                    let _ = window.update(cx, |_, window, cx| {
                        window.push_notification(update_notice(message, false), cx);
                    });
                }
                loop {
                    let check = manager.clone();
                    let ready = cx
                        .background_executor()
                        .spawn(async move {
                            if let Some(asset) = check.get_update_pending_restart() {
                                return Some(asset.Version);
                            }
                            let Ok(UpdateCheck::UpdateAvailable(update)) =
                                check.check_for_updates()
                            else {
                                return None;
                            };
                            check.download_updates(&update, None).ok()?;
                            Some(update.TargetFullRelease.Version)
                        })
                        .await;
                    if let Some(version) = ready {
                        let message = format!(
                            "Origo {version} siap dipasang. Keluar dan buka lagi untuk memperbarui."
                        );
                        let _ = window.update(cx, |_, window, cx| {
                            window.push_notification(update_notice(message, true), cx);
                        });
                        break;
                    }
                    cx.background_executor()
                        .timer(Duration::from_secs(30 * 60))
                        .await;
                }
            })
            .detach();
        }
    }
}

fn update_notice(message: String, ready: bool) -> Notification {
    use gpui_kit::component::notification::NotificationType;
    Notification::new()
        .with_type(if ready {
            NotificationType::Info
        } else {
            NotificationType::Success
        })
        .placement(gpui_kit::Anchor::BottomRight)
        .autohide(!ready)
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
