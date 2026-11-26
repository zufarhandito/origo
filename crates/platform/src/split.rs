//! One shared source snapshot and one private output directory for a split batch.
use super::*;
use pdf_application::split::{SplitError, SplitOutput, SplitTransaction, validate_folder};

pub struct LocalSplitOutput;
pub struct SafeSplit {
    directory: TempDir,
    snapshots: SafeOutput,
    ready: PathBuf,
    destination: PathBuf,
}
fn require_absent(path: &Path) -> Result<(), SplitError> {
    match fs::symlink_metadata(path) {
        Ok(_) => Err(SplitError::Exists),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(_) => Err(SplitError::Output),
    }
}
impl SplitOutput for LocalSplitOutput {
    type Transaction = SafeSplit;
    fn stage(
        &self,
        plan: &ExportPlan,
        destination: &Path,
        control: &ExportControl,
    ) -> Result<SafeSplit, SplitError> {
        stage_batch(plan, destination, control, ".pdf-studio-split-")
    }
}
pub(crate) fn stage_batch(
    plan: &ExportPlan,
    destination: &Path,
    control: &ExportControl,
    prefix: &str,
) -> Result<SafeSplit, SplitError> {
    control.checkpoint()?;
    let name = destination
        .file_name()
        .and_then(|s| s.to_str())
        .ok_or(SplitError::Folder)?;
    validate_folder(name)?;
    let parent = dunce::canonicalize(destination.parent().ok_or(SplitError::Output)?)
        .map_err(|_| SplitError::Output)?;
    let destination = parent.join(name);
    require_absent(&destination)?;
    let directory = tempfile::Builder::new()
        .prefix(prefix)
        .tempdir_in(&parent)
        .map_err(|_| SplitError::Output)?;
    let ready = directory.path().join("results");
    fs::create_dir(&ready).map_err(|_| SplitError::Output)?;
    let snapshots = LocalOutput.stage(plan, &directory.path().join("snapshot.pdf"), control)?;
    Ok(SafeSplit {
        directory,
        snapshots,
        ready,
        destination,
    })
}
impl SplitTransaction for SafeSplit {
    fn sources(&self) -> &[SourceDocument] {
        self.snapshots.sources()
    }
    fn output_directory(&self) -> &Path {
        &self.ready
    }
    fn work_directory(&self) -> &Path {
        self.directory.path()
    }
    fn verify_sources(&self) -> Result<(), ExportError> {
        self.snapshots.verify_sources()
    }
    fn commit(self, control: &ExportControl) -> Result<PathBuf, SplitError> {
        self.verify_sources()?;
        // Flush only the owned output files. Snapshot/work files are never published.
        let mut count = 0;
        for entry in fs::read_dir(&self.ready).map_err(|_| SplitError::Output)? {
            control.checkpoint()?;
            let path = entry.map_err(|_| SplitError::Output)?.path();
            let file = OpenOptions::new()
                .read(true)
                .write(true)
                .open(&path)
                .map_err(|_| SplitError::Output)?;
            let metadata = file.metadata().map_err(|_| SplitError::Output)?;
            if !metadata.is_file() || metadata.len() == 0 {
                return Err(SplitError::Output);
            }
            file.sync_all().map_err(|_| SplitError::Output)?;
            count += 1;
        }
        if count == 0 {
            return Err(ExportError::Empty.into());
        }
        self.verify_sources()?;
        require_absent(&self.destination)?;
        control.checkpoint()?;
        publish(&self.ready, &self.destination)?;
        Ok(self.destination.clone())
    }
}
/// Publish a staged directory to a non-existent destination without replacing existing entries.
pub fn publish_directory_no_clobber(source: &Path, destination: &Path) -> Result<(), SplitError> {
    require_absent(destination)?;
    publish(source, destination)
}
// Do not use std::fs::rename: it can replace an empty destination directory.
// The pinned safe wrapper calls MoveFileExW without REPLACE_EXISTING on Windows.
// Other platforms require their own validated no-replace directory primitive.
#[cfg(windows)]
fn publish(source: &Path, destination: &Path) -> Result<(), SplitError> {
    atomicwrites::move_atomic(source, destination).map_err(|_| {
        if fs::symlink_metadata(destination).is_ok() {
            SplitError::Exists
        } else {
            SplitError::Output
        }
    })
}
#[cfg(not(windows))]
fn publish(_: &Path, _: &Path) -> Result<(), SplitError> {
    Err(SplitError::Output)
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;
    #[test]
    fn directory_publication_never_replaces_existing_empty_directory_or_file()
    -> Result<(), Box<dyn std::error::Error>> {
        let dir = tempfile::tempdir()?;
        let source = dir.path().join("private");
        let target = dir.path().join("existing");
        fs::create_dir(&source)?;
        fs::write(source.join("result.pdf"), b"private output")?;
        fs::create_dir(&target)?;
        assert!(matches!(publish(&source, &target), Err(SplitError::Exists)));
        assert!(source.join("result.pdf").exists());
        assert_eq!(fs::read_dir(&target)?.count(), 0);
        let file = dir.path().join("existing-file");
        fs::write(&file, b"keep")?;
        assert!(matches!(publish(&source, &file), Err(SplitError::Exists)));
        assert_eq!(fs::read(&file)?, b"keep");
        let new = dir.path().join("new");
        publish(&source, &new)?;
        assert_eq!(fs::read(new.join("result.pdf"))?, b"private output");
        Ok(())
    }
}
