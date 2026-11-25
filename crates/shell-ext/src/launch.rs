//! Safe manifest writer and detached Origo process launcher for Explorer commands.
use std::{
    fs,
    io::Write as _,
    path::{Path, PathBuf},
    process::Command,
    sync::atomic::{AtomicU64, Ordering},
};

#[cfg(windows)]
use std::os::windows::process::CommandExt as _;

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

static MANIFEST_SEQ: AtomicU64 = AtomicU64::new(1);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ActionMode {
    ImagesDialog,
    ImagesQuickConvert,
    PdfOpen,
    PdfCompress,
    PdfSplit,
    PdfPng,
    PdfOcr,
    PdfCombineOrganize,
    PdfCombineQuick,
}

impl ActionMode {
    pub fn cli_flag(self) -> &'static str {
        match self {
            Self::ImagesDialog => "--images-to-pdf-list",
            Self::ImagesQuickConvert => "--images-to-pdf-quick-list",
            Self::PdfOpen => "--open-pdf-list",
            Self::PdfCompress => "--pdf-compress-list",
            Self::PdfSplit => "--pdf-split-list",
            Self::PdfPng => "--pdf-png-list",
            Self::PdfOcr => "--pdf-ocr-list",
            Self::PdfCombineOrganize => "--merge-pdf-list",
            Self::PdfCombineQuick => "--merge-pdf-quick-list",
        }
    }
}

pub fn resolve_origo_exe(dll_dir: &Path) -> Option<PathBuf> {
    let pointer = dll_dir.join("origo-target.txt");
    if let Ok(raw) = fs::read_to_string(&pointer) {
        let target = PathBuf::from(raw.trim().trim_start_matches('\u{FEFF}'));
        if target.is_file() {
            return Some(target);
        }
    }
    let sibling = dll_dir.join("pdf-studio.exe");
    if sibling.is_file() {
        return Some(sibling);
    }
    let parent_exe = dll_dir.parent().map(|p| p.join("pdf-studio.exe"));
    if let Some(exe) = parent_exe
        && exe.is_file()
    {
        return Some(exe);
    }
    None
}

pub fn write_temp_manifest(paths: &[PathBuf]) -> std::io::Result<PathBuf> {
    let dir = std::env::temp_dir();
    let pid = std::process::id();
    let seq = MANIFEST_SEQ.fetch_add(1, Ordering::Relaxed);
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let manifest_path = dir.join(format!("origo-shell-{pid}-{seq}-{nanos}.txt"));
    let mut file = fs::File::create(&manifest_path)?;
    for path in paths {
        writeln!(file, "{}", path.to_string_lossy())?;
    }
    file.flush()?;
    Ok(manifest_path)
}

pub fn launch_origo(dll_dir: &Path, mode: ActionMode, paths: &[PathBuf]) -> std::io::Result<()> {
    if paths.is_empty() {
        return Ok(());
    }
    let exe = resolve_origo_exe(dll_dir).ok_or_else(|| {
        std::io::Error::new(std::io::ErrorKind::NotFound, "pdf-studio.exe not found")
    })?;
    let manifest = write_temp_manifest(paths)?;
    let mut cmd = Command::new(&exe);
    cmd.arg(mode.cli_flag()).arg(&manifest).arg("--delete-list");
    if let Some(parent) = exe.parent() {
        cmd.current_dir(parent);
    }
    #[cfg(windows)]
    cmd.creation_flags(CREATE_NO_WINDOW);
    if let Err(err) = cmd.spawn() {
        let _ = fs::remove_file(&manifest);
        return Err(err);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_target_pointer_and_writes_manifest() {
        let base = std::env::temp_dir().join(format!("origo-shell-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&base);
        assert!(fs::create_dir_all(&base).is_ok());
        let fake_exe = base.join("pdf-studio.exe");
        assert!(fs::write(&fake_exe, b"").is_ok());

        assert_eq!(resolve_origo_exe(&base), Some(fake_exe.clone()));

        let custom_exe = base.join("custom-origo.exe");
        assert!(fs::write(&custom_exe, b"").is_ok());
        assert!(
            fs::write(
                base.join("origo-target.txt"),
                custom_exe.to_string_lossy().as_bytes()
            )
            .is_ok()
        );
        assert_eq!(resolve_origo_exe(&base), Some(custom_exe));

        let manifest = write_temp_manifest(&[
            PathBuf::from("C:\\tmp\\1.png"),
            PathBuf::from("C:\\tmp\\2.jpg"),
        ])
        .ok();
        assert!(manifest.is_some());
        if let Some(manifest) = manifest {
            let text = fs::read_to_string(&manifest).unwrap_or_default();
            assert!(text.contains("C:\\tmp\\1.png"));
            assert!(text.contains("C:\\tmp\\2.jpg"));
            let _ = fs::remove_file(manifest);
        }
        let _ = fs::remove_dir_all(&base);
    }
}
