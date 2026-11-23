//! Installed Windows builds check their packaged feed outside the UI thread.
use anyhow::Context as _;
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
        state_dir: preview.then(|| root.join("UserData")),
    }))
}

impl Installed {
    pub fn version(&self) -> String {
        self.manager.get_current_version_as_string()
    }

    pub fn check_in_background(&self) {
        let manager = self.manager.clone();
        let _ = std::thread::Builder::new()
            .name("origo-update".into())
            .spawn(move || {
                loop {
                    if manager.get_update_pending_restart().is_none()
                        && let Ok(UpdateCheck::UpdateAvailable(update)) =
                            manager.check_for_updates()
                    {
                        let _ = manager.download_updates(&update, None);
                    }
                    std::thread::sleep(Duration::from_secs(30 * 60));
                }
            });
    }
}
