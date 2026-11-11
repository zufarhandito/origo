//! Owned output transactions and bounded child processes. No UI or PDF parsing.
use pdf_application::{
    SourceDocument, SourceFingerprint,
    export::{AtomicOutput, ExportControl, ExportError, ExportPlan, OutputTransaction},
};
use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant, UNIX_EPOCH},
};
use tempfile::{TempDir, TempPath};

pub fn fingerprint(path: &Path) -> Result<SourceFingerprint, ExportError> {
    let m = fs::metadata(path).map_err(|_| ExportError::SourceChanged)?;
    if !m.is_file() {
        return Err(ExportError::SourceChanged);
    }
    Ok(SourceFingerprint {
        bytes: m.len(),
        modified_ns: m
            .modified()
            .map_err(|_| ExportError::SourceChanged)?
            .duration_since(UNIX_EPOCH)
            .map_err(|_| ExportError::SourceChanged)?
            .as_nanos(),
    })
}
#[derive(Clone, Copy)]
pub struct LocalOutput;
pub struct SafeOutput {
    dir: TempDir,
    output: TempPath,
    destination: PathBuf,
    originals: Vec<SourceDocument>,
    sources: Vec<SourceDocument>,
}
impl AtomicOutput for LocalOutput {
    type Transaction = SafeOutput;
    fn stage(
        &self,
        plan: &ExportPlan,
        destination: &Path,
        control: &ExportControl,
    ) -> Result<SafeOutput, ExportError> {
        control.checkpoint()?;
        if !destination
            .extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("pdf"))
        {
            return Err(ExportError::Destination);
        }
        let name = destination.file_name().ok_or(ExportError::Destination)?;
        let parent = dunce::canonicalize(destination.parent().ok_or(ExportError::Destination)?)
            .map_err(|_| ExportError::Destination)?;
        if !parent.is_dir() {
            return Err(ExportError::Destination);
        }
        let destination = parent.join(name);
        match fs::symlink_metadata(&destination) {
            Ok(_) => return Err(ExportError::DestinationExists),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(_) => return Err(ExportError::Destination),
        }
        let dir = tempfile::Builder::new()
            .prefix(".pdf-studio-export-")
            .tempdir_in(&parent)
            .map_err(|_| ExportError::Io)?;
        let mut sources = Vec::new();
        let mut total = 0u64;
        for (index, original) in plan.sources().iter().enumerate() {
            control.checkpoint()?;
            if fingerprint(&original.path)? != original.fingerprint {
                return Err(ExportError::SourceChanged);
            }
            let mut input = File::open(&original.path).map_err(|_| ExportError::SourceChanged)?;
            let path = dir.path().join(format!("source-{index}.pdf"));
            let mut output = OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&path)
                .map_err(|_| ExportError::Io)?;
            let mut buffer = [0; 65536];
            let mut copied = 0u64;
            loop {
                control.checkpoint()?;
                let size = input
                    .read(&mut buffer)
                    .map_err(|_| ExportError::SourceChanged)?;
                if size == 0 {
                    break;
                }
                copied += size as u64;
                total += size as u64;
                if total > 128 * 1024 * 1024 || copied > original.fingerprint.bytes {
                    return Err(ExportError::ResourceLimit);
                }
                output
                    .write_all(&buffer[..size])
                    .map_err(|_| ExportError::Io)?;
            }
            output.sync_all().map_err(|_| ExportError::Io)?;
            drop(output);
            if copied != original.fingerprint.bytes
                || fingerprint(&original.path)? != original.fingerprint
            {
                return Err(ExportError::SourceChanged);
            }
            let mut source = original.clone();
            source.path = path;
            source.fingerprint = fingerprint(&source.path)?;
            sources.push(source);
        }
        let output = tempfile::Builder::new()
            .prefix("result-")
            .suffix(".pdf")
            .tempfile_in(dir.path())
            .map_err(|_| ExportError::Io)?
            .into_temp_path();
        Ok(SafeOutput {
            dir,
            output,
            destination,
            originals: plan.sources().to_vec(),
            sources,
        })
    }
}
impl OutputTransaction for SafeOutput {
    fn sources(&self) -> &[SourceDocument] {
        &self.sources
    }
    fn directory(&self) -> &Path {
        self.dir.path()
    }
    fn output(&self) -> &Path {
        self.output.as_ref()
    }
    fn verify_sources(&self) -> Result<(), ExportError> {
        for source in self.originals.iter().chain(&self.sources) {
            if fingerprint(&source.path)? != source.fingerprint {
                return Err(ExportError::SourceChanged);
            }
        }
        Ok(())
    }
    fn commit(self, control: &ExportControl) -> Result<PathBuf, ExportError> {
        self.verify_sources()?;
        control.checkpoint()?;
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .open(&self.output)
            .map_err(|_| ExportError::Io)?;
        if file.metadata().map_err(|_| ExportError::Io)?.len() == 0 {
            return Err(ExportError::Validation);
        }
        file.sync_all().map_err(|_| ExportError::Io)?;
        drop(file);
        control.checkpoint()?;
        self.output
            .persist_noclobber(&self.destination)
            .map_err(|e| {
                if e.error.kind() == std::io::ErrorKind::AlreadyExists {
                    ExportError::DestinationExists
                } else {
                    ExportError::Io
                }
            })?;
        Ok(self.destination)
    }
}
pub struct ProcessOutput {
    pub code: Option<i32>,
    pub stdout: Vec<u8>,
}
fn capture(
    mut reader: impl Read,
    limit: usize,
    overflow: Arc<AtomicBool>,
) -> Result<Vec<u8>, ExportError> {
    let mut result = Vec::new();
    let mut buffer = [0; 8192];
    loop {
        let size = reader.read(&mut buffer).map_err(|_| ExportError::Io)?;
        if size == 0 {
            break;
        }
        let keep = size.min(limit.saturating_sub(result.len()));
        result.extend_from_slice(&buffer[..keep]);
        if keep < size {
            overflow.store(true, Ordering::SeqCst);
        }
    }
    Ok(result)
}
/// QPDF budget measures process time, excluding user file/confirmation dialogs.
pub fn run(
    mut command: Command,
    stdout_limit: usize,
    control: &ExportControl,
) -> Result<ProcessOutput, ExportError> {
    let budget = control.remaining()?;
    let start = Instant::now();
    command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000);
    }
    let mut child = command.spawn().map_err(|_| ExportError::Dependency)?;
    let stdout = child.stdout.take().ok_or(ExportError::Engine)?;
    let stderr = child.stderr.take().ok_or(ExportError::Engine)?;
    let overflow = Arc::new(AtomicBool::new(false));
    let out_flag = overflow.clone();
    let err_flag = overflow.clone();
    let out = std::thread::spawn(move || capture(stdout, stdout_limit, out_flag));
    let err = std::thread::spawn(move || capture(stderr, 65536, err_flag));
    let result = loop {
        if let Err(e) = control.checkpoint() {
            break Err(e);
        }
        if overflow.load(Ordering::SeqCst) {
            break Err(ExportError::ResourceLimit);
        }
        if start.elapsed() >= budget {
            break Err(ExportError::Timeout);
        }
        match child.try_wait() {
            Ok(Some(status)) => break Ok(status.code()),
            Ok(None) => std::thread::sleep(Duration::from_millis(10)),
            Err(_) => break Err(ExportError::Engine),
        }
    };
    if result.is_err() {
        let _ = child.kill();
        let _ = child.wait();
    }
    let stdout = out.join().map_err(|_| ExportError::Engine)?;
    let stderr = err.join().map_err(|_| ExportError::Engine)?;
    control.charge(start.elapsed());
    let code = result?;
    let stdout = stdout?;
    let _ = stderr?;
    if overflow.load(Ordering::SeqCst) {
        return Err(ExportError::ResourceLimit);
    }
    Ok(ProcessOutput { code, stdout })
}

#[cfg(test)]
mod tests {
    use super::*;
    use pdf_application::{ExportSupport, PageGeometry, SourceId, Workspace, export::ExportScope};
    fn plan(dir: &Path) -> Result<ExportPlan, Box<dyn std::error::Error>> {
        let path = dir.join("input.pdf");
        fs::write(&path, b"fixture")?;
        let w = Workspace::new(SourceDocument {
            id: SourceId(1),
            fingerprint: fingerprint(&path)?,
            path,
            geometries: vec![PageGeometry {
                width_points: 300.,
                height_points: 420.,
                quarter_turns: 0,
            }],
            export_support: ExportSupport::NotPreflighted,
        })?;
        Ok(ExportPlan::capture(&w, 1, ExportScope::All)?)
    }
    #[test]
    fn no_clobber_covers_existing_hardlink_races_and_cleans_owned_stage()
    -> Result<(), Box<dyn std::error::Error>> {
        let dir = tempfile::tempdir()?;
        let plan = plan(dir.path())?;
        let c = ExportControl::default();
        let dest = dir.path().join("out.pdf");
        fs::hard_link(&plan.sources()[0].path, &dest)?;
        assert!(matches!(
            LocalOutput.stage(&plan, &dest, &c),
            Err(ExportError::DestinationExists)
        ));
        fs::remove_file(&dest)?;
        let stage = LocalOutput.stage(&plan, &dest, &c)?;
        let owned = stage.directory().to_path_buf();
        fs::write(stage.output(), b"output")?;
        fs::write(&dest, b"winner")?;
        assert!(matches!(
            stage.commit(&c),
            Err(ExportError::DestinationExists)
        ));
        assert_eq!(fs::read(&dest)?, b"winner");
        assert!(!owned.exists());
        fs::remove_file(&dest)?;
        let stage = LocalOutput.stage(&plan, &dest, &c)?;
        let owned = stage.directory().to_path_buf();
        fs::write(stage.output(), b"output")?;
        stage.commit(&c)?;
        assert_eq!(fs::read(&dest)?, b"output");
        assert!(!owned.exists());
        assert_eq!(fs::read(&plan.sources()[0].path)?, b"fixture");
        Ok(())
    }
    #[test]
    fn source_change_and_cancel_leave_destination_absent() -> Result<(), Box<dyn std::error::Error>>
    {
        let dir = tempfile::tempdir()?;
        let plan = plan(dir.path())?;
        let dest = dir.path().join("out.pdf");
        let c = ExportControl::default();
        let stage = LocalOutput.stage(&plan, &dest, &c)?;
        let owned = stage.directory().to_path_buf();
        fs::write(stage.output(), b"output")?;
        fs::write(&plan.sources()[0].path, b"changed-size")?;
        assert!(matches!(stage.commit(&c), Err(ExportError::SourceChanged)));
        assert!(!dest.exists());
        assert!(!owned.exists());
        let c = ExportControl::default();
        c.cancel();
        assert!(matches!(
            LocalOutput.stage(&plan, &dest, &c),
            Err(ExportError::Cancelled)
        ));
        Ok(())
    }
    #[test]
    fn child_fixture() {
        if let Ok(mode) = std::env::var("PDF_STUDIO_PROCESS_TEST") {
            match mode.as_str() {
                "overflow" => {
                    let _ = std::io::stdout().write_all(&vec![b'x'; 100000]);
                }
                "sleep" => std::thread::sleep(Duration::from_secs(10)),
                "fail" => std::process::exit(7),
                _ => {}
            }
        }
    }
    fn command(mode: &str) -> Result<Command, std::io::Error> {
        let mut c = Command::new(std::env::current_exe()?);
        c.args(["--exact", "tests::child_fixture", "--nocapture"])
            .env("PDF_STUDIO_PROCESS_TEST", mode);
        Ok(c)
    }
    #[test]
    fn process_limits_timeout_cancellation_and_status_are_real()
    -> Result<(), Box<dyn std::error::Error>> {
        assert!(matches!(
            run(command("overflow")?, 100, &ExportControl::default()),
            Err(ExportError::ResourceLimit)
        ));
        assert!(matches!(
            run(
                command("sleep")?,
                1000,
                &ExportControl::with_budget(Duration::from_millis(100))
            ),
            Err(ExportError::Timeout)
        ));
        let c = ExportControl::default();
        let cancel = c.clone();
        let sender = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(100));
            cancel.cancel();
        });
        assert!(matches!(
            run(command("sleep")?, 1000, &c),
            Err(ExportError::Cancelled)
        ));
        sender.join().map_err(|_| "cancel thread")?;
        assert_eq!(
            run(command("fail")?, 1000, &ExportControl::default())?.code,
            Some(7)
        );
        Ok(())
    }
}
