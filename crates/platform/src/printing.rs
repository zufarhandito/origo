//! Launch only the embedded managed Windows print UI, never a PDF shell verb.
use pdf_application::export::{ExportControl, ExportError};
use std::{
    path::Path,
    process::{Command, Stdio},
    time::{Duration, Instant},
};
pub fn show(
    directory: &Path,
    script: &str,
    control: &ExportControl,
) -> Result<String, ExportError> {
    let helper = directory.join("print-helper.ps1");
    std::fs::write(&helper, script).map_err(|_| ExportError::Io)?;
    let windows = std::env::var_os("SystemRoot").ok_or(ExportError::Engine)?;
    let executable =
        std::path::PathBuf::from(windows).join("System32/WindowsPowerShell/v1.0/powershell.exe");
    let mut command = Command::new(executable);
    command
        .args([
            "-NoLogo",
            "-NoProfile",
            "-STA",
            "-ExecutionPolicy",
            "Bypass",
            "-File",
        ])
        .arg(&helper)
        .arg("-JobDirectory")
        .arg(directory)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    if let Ok(exe) = std::env::current_exe() {
        command.env("ORIGO_PRINT_EXECUTABLE", exe);
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000);
    }
    let mut child = command.spawn().map_err(|_| {
        ExportError::PrintBackend(
            "Komponen Windows PowerShell/.NET untuk dialog cetak tidak tersedia.".into(),
        )
    })?;
    let started = Instant::now();
    let mut cancelled = None;
    loop {
        if let Some(_status) = child.try_wait().map_err(|_| ExportError::Engine)? {
            break;
        }
        if control.checkpoint().is_err() || started.elapsed() > Duration::from_secs(1800) {
            let _ = std::fs::write(directory.join("cancel"), b"cancel");
            let since = cancelled.get_or_insert_with(Instant::now);
            if since.elapsed() > Duration::from_secs(30) {
                let _ = child.kill();
                let _ = child.wait();
                return Err(ExportError::Cancelled);
            }
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    control.checkpoint()?;
    let path = directory.join("result.json");
    if std::fs::metadata(&path)
        .map_err(|_| ExportError::PrintBackend("Helper cetak tidak menghasilkan status. Periksa komponen atau kebijakan perangkat Windows.".into()))?
        .len()
        > 65536
    {
        return Err(ExportError::ResourceLimit);
    }
    let result: serde_json::Value =
        serde_json::from_slice(&std::fs::read(path).map_err(|_| ExportError::Engine)?)
            .map_err(|_| ExportError::Engine)?;
    match result["status"].as_str() {
        Some("pdf") => Ok("pdf".into()),
        Some("printed") => Ok("printed".into()),
        Some("cancelled") => Err(ExportError::Cancelled),
        _ => Err(ExportError::PrintBackend(
            result["message"]
                .as_str()
                .unwrap_or("Dialog cetak tidak dapat diselesaikan.")
                .chars()
                .take(500)
                .collect(),
        )),
    }
}
