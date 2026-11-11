//! M0 trusted synthetic-fixture probe only; not a production user-document exporter.
use anyhow::{Context, Result, bail};
use std::{
    ffi::OsString,
    io::Read,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::{Duration, Instant},
};

pub struct QpdfProbe {
    executable: PathBuf,
}

impl QpdfProbe {
    pub fn new(sidecars: &Path) -> Self {
        Self {
            executable: sidecars.join("qpdf/bin/qpdf.exe"),
        }
    }

    pub fn verify_version(&self) -> Result<()> {
        let output = self.run(&["--version".into()])?;
        let first = output.lines().next().unwrap_or_default();
        anyhow::ensure!(
            first == "qpdf version 12.4.2",
            "QPDF sidecar version mismatch"
        );
        Ok(())
    }

    /// Called only for our generated fixture and a new path owned by a tempdir.
    pub fn transform_fixture(
        &self,
        source: &Path,
        second_source: &Path,
        output: &Path,
    ) -> Result<()> {
        anyhow::ensure!(!output.exists(), "Destination already exists");
        // QPDF treats the verbatim Windows prefix as special syntax. Keep normal
        // absolute paths where Windows permits it, retaining UNC semantics safely.
        let source = dunce::canonicalize(source).context("Synthetic fixture missing")?;
        let second_source = dunce::canonicalize(second_source).context("Second fixture missing")?;
        self.run(&fixture_args(&source, &second_source, output))?;
        self.run(&["--check".into(), output.as_os_str().to_owned()])?;
        anyhow::ensure!(output.metadata()?.len() > 0, "QPDF output is empty");
        Ok(())
    }

    fn run(&self, args: &[OsString]) -> Result<String> {
        let mut command = Command::new(&self.executable);
        command
            .args(args)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            command.creation_flags(0x08000000); // CREATE_NO_WINDOW
        }
        let mut child = command
            .spawn()
            .context("Bundled QPDF unavailable; run fetch-sidecars.ps1")?;
        let stdout = child.stdout.take().context("Missing stdout pipe")?;
        let stderr = child.stderr.take().context("Missing stderr pipe")?;
        let reader = std::thread::spawn(move || capture(stdout));
        let errors = std::thread::spawn(move || capture(stderr));
        let deadline = Instant::now() + Duration::from_secs(30);
        let result = loop {
            match child.try_wait() {
                Ok(Some(status)) => break Ok(status),
                Ok(None) if Instant::now() < deadline => {
                    std::thread::sleep(Duration::from_millis(20))
                }
                Ok(None) => break Err(anyhow::anyhow!("QPDF timed out")),
                Err(error) => break Err(error.into()),
            }
        };
        if result.is_err() {
            let _ = child.kill();
            let _ = child.wait();
        }
        let captured = reader
            .join()
            .map_err(|_| anyhow::anyhow!("QPDF output reader failed"))??;
        let diagnostic = errors
            .join()
            .map_err(|_| anyhow::anyhow!("QPDF stderr reader failed"))??;
        let status = result?;
        if !status.success() {
            // Probe uses only synthetic data; never expose this raw diagnostic for user PDFs.
            bail!(
                "QPDF synthetic probe failed ({status}): {}",
                String::from_utf8_lossy(&diagnostic)
            );
        }
        Ok(String::from_utf8_lossy(&captured).into_owned())
    }
}

fn capture(mut stream: impl Read) -> std::io::Result<Vec<u8>> {
    let mut captured = Vec::new();
    let mut buffer = [0u8; 4096];
    loop {
        let read = stream.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        let keep = read.min(65536usize.saturating_sub(captured.len()));
        captured.extend_from_slice(&buffer[..keep]);
    }
    Ok(captured)
}

fn fixture_args(source: &Path, second_source: &Path, output: &Path) -> Vec<OsString> {
    // Select page 2 from source A and page 1 from source B, then rotate output page 1.
    vec![
        "--empty".into(),
        "--rotate=+90:1".into(),
        "--pages".into(),
        source.as_os_str().to_owned(),
        "2".into(),
        second_source.as_os_str().to_owned(),
        "1".into(),
        "--".into(),
        output.as_os_str().to_owned(),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn paths_are_single_tokens_and_page_order_is_explicit() {
        let source = Path::new("C:/contoh spasi/日本 & file.pdf");
        let output = Path::new("C:/output/out.pdf");
        let second_source = Path::new("C:/contoh spasi/-input.pdf");
        let args = fixture_args(source, second_source, output);
        assert_eq!(args[3], source.as_os_str());
        assert_eq!(args[4], "2");
        assert_eq!(args[5], second_source.as_os_str());
        assert_eq!(args[6], "1");
        assert_eq!(args[7], "--");
        assert_eq!(args[8], output.as_os_str());
    }
}
