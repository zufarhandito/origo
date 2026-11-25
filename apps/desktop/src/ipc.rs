//! Local single-instance IPC for forwarding Explorer shell commands and CLI launches
//! to an already-running Origo window without spawning duplicate windows.
use std::{
    fs,
    io::{BufRead, BufReader, Read, Write},
    net::{IpAddr, Ipv4Addr, SocketAddr, TcpListener, TcpStream},
    path::{Path, PathBuf},
    sync::mpsc,
    time::Duration,
};

const ENDPOINT_FILE: &str = "ipc-endpoint.json";
const MAX_PAYLOAD_BYTES: usize = 256 * 1024;
const MAX_MANIFEST_BYTES: u64 = 1024 * 1024;
const CONNECT_TIMEOUT: Duration = Duration::from_millis(500);
const IO_TIMEOUT: Duration = Duration::from_millis(1500);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum PdfShellAction {
    Compress,
    Split,
    Png,
    Ocr,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum LaunchIntent {
    None,
    OpenPdf(PathBuf),
    PdfAction {
        path: PathBuf,
        action: PdfShellAction,
    },
    PdfCombineOrganize(Vec<PathBuf>),
    PdfCombineQuick(Vec<PathBuf>),
    ImagesToPdfDialog(Vec<PathBuf>),
    ImagesToPdfQuick(Vec<PathBuf>),
}

impl LaunchIntent {
    pub(super) fn is_none(&self) -> bool {
        matches!(self, Self::None)
    }

    pub(super) fn has_document_open(&self) -> bool {
        matches!(
            self,
            Self::OpenPdf(_) | Self::PdfAction { .. } | Self::PdfCombineOrganize(_)
        )
    }
}

pub(super) struct IpcGuard {
    endpoint_path: PathBuf,
    token: String,
}

impl Drop for IpcGuard {
    fn drop(&mut self) {
        if let Ok(content) = fs::read_to_string(&self.endpoint_path)
            && content.contains(&self.token)
        {
            let _ = fs::remove_file(&self.endpoint_path);
        }
    }
}

fn endpoint_path(state_dir: &Path) -> PathBuf {
    state_dir.join(ENDPOINT_FILE)
}

fn generate_token() -> Option<String> {
    let mut bytes = [0u8; 32];
    getrandom::fill(&mut bytes).ok()?;
    let mut hex = String::with_capacity(64);
    for byte in bytes {
        use std::fmt::Write as _;
        let _ = write!(&mut hex, "{byte:02x}");
    }
    Some(hex)
}

fn constant_time_eq(a: &str, b: &str) -> bool {
    let a = a.as_bytes();
    let b = b.as_bytes();
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for (x, y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

fn read_endpoint(state_dir: &Path) -> Option<(u16, String)> {
    let raw = fs::read_to_string(endpoint_path(state_dir)).ok()?;
    let value: serde_json::Value = serde_json::from_str(&raw).ok()?;
    let port = value.get("port")?.as_u64()?;
    let port = u16::try_from(port).ok()?;
    let token = value.get("token")?.as_str()?.to_owned();
    if port == 0 || token.len() < 32 {
        return None;
    }
    Some((port, token))
}

fn encode_paths(paths: &[PathBuf]) -> Vec<String> {
    paths
        .iter()
        .map(|p| p.to_string_lossy().into_owned())
        .collect()
}

fn encode_intent(token: &str, intent: &LaunchIntent) -> Option<String> {
    let (kind, paths): (&str, Vec<String>) = match intent {
        LaunchIntent::None => return None,
        LaunchIntent::OpenPdf(path) => ("open_pdf", vec![path.to_string_lossy().into_owned()]),
        LaunchIntent::PdfAction { path, action } => {
            let kind = match action {
                PdfShellAction::Compress => "pdf_compress",
                PdfShellAction::Split => "pdf_split",
                PdfShellAction::Png => "pdf_png",
                PdfShellAction::Ocr => "pdf_ocr",
            };
            (kind, vec![path.to_string_lossy().into_owned()])
        }
        LaunchIntent::PdfCombineOrganize(paths) => ("pdf_combine_organize", encode_paths(paths)),
        LaunchIntent::PdfCombineQuick(paths) => ("pdf_combine_quick", encode_paths(paths)),
        LaunchIntent::ImagesToPdfDialog(paths) => ("images_dialog", encode_paths(paths)),
        LaunchIntent::ImagesToPdfQuick(paths) => ("images_quick", encode_paths(paths)),
    };
    let payload = serde_json::json!({
        "token": token,
        "kind": kind,
        "paths": paths,
    });
    let mut line = serde_json::to_string(&payload).ok()?;
    line.push('\n');
    (line.len() <= MAX_PAYLOAD_BYTES).then_some(line)
}

fn decode_intent(line: &str, expected_token: &str) -> Option<LaunchIntent> {
    let value: serde_json::Value = serde_json::from_str(line.trim()).ok()?;
    let token = value.get("token")?.as_str()?;
    if !constant_time_eq(token, expected_token) {
        return None;
    }
    let kind = value.get("kind")?.as_str()?;
    let paths: Vec<PathBuf> = value
        .get("paths")?
        .as_array()?
        .iter()
        .filter_map(|item| item.as_str().map(PathBuf::from))
        .collect();
    match kind {
        "open_pdf" => paths.into_iter().next().map(LaunchIntent::OpenPdf),
        "pdf_compress" => paths.into_iter().next().map(|path| LaunchIntent::PdfAction {
            path,
            action: PdfShellAction::Compress,
        }),
        "pdf_split" => paths.into_iter().next().map(|path| LaunchIntent::PdfAction {
            path,
            action: PdfShellAction::Split,
        }),
        "pdf_png" => paths.into_iter().next().map(|path| LaunchIntent::PdfAction {
            path,
            action: PdfShellAction::Png,
        }),
        "pdf_ocr" => paths.into_iter().next().map(|path| LaunchIntent::PdfAction {
            path,
            action: PdfShellAction::Ocr,
        }),
        "pdf_combine_organize" if !paths.is_empty() => Some(LaunchIntent::PdfCombineOrganize(paths)),
        "pdf_combine_quick" if !paths.is_empty() => Some(LaunchIntent::PdfCombineQuick(paths)),
        "images_dialog" if !paths.is_empty() => Some(LaunchIntent::ImagesToPdfDialog(paths)),
        "images_quick" if !paths.is_empty() => Some(LaunchIntent::ImagesToPdfQuick(paths)),
        _ => None,
    }
}

pub(super) fn try_forward_to_running(state_dir: &Path, intent: &LaunchIntent) -> bool {
    if intent.is_none() {
        return false;
    }
    let Some((port, token)) = read_endpoint(state_dir) else {
        return false;
    };
    let Some(payload) = encode_intent(&token, intent) else {
        return false;
    };
    let addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), port);
    let Ok(mut stream) = TcpStream::connect_timeout(&addr, CONNECT_TIMEOUT) else {
        return false;
    };
    let _ = stream.set_read_timeout(Some(IO_TIMEOUT));
    let _ = stream.set_write_timeout(Some(IO_TIMEOUT));
    if stream.write_all(payload.as_bytes()).is_err() || stream.flush().is_err() {
        return false;
    }
    let mut response = String::new();
    let mut limited = stream.take(16);
    if limited.read_to_string(&mut response).is_err() {
        return false;
    }
    response.trim() == "OK"
}

pub(super) fn start_listener(state_dir: &Path) -> Option<(IpcGuard, mpsc::Receiver<LaunchIntent>)> {
    if fs::create_dir_all(state_dir).is_err() {
        return None;
    }
    let token = generate_token()?;
    let listener = TcpListener::bind(SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 0)).ok()?;
    let port = listener.local_addr().ok()?.port();
    let path = endpoint_path(state_dir);
    let record = serde_json::json!({
        "port": port,
        "token": token,
        "pid": std::process::id(),
    });
    let bytes = serde_json::to_vec(&record).ok()?;
    let temp = state_dir.join(format!("{ENDPOINT_FILE}.{}.tmp", std::process::id()));
    if fs::write(&temp, &bytes).is_err() {
        return None;
    }
    if fs::rename(&temp, &path).is_err() {
        let _ = fs::remove_file(&temp);
        if fs::write(&path, &bytes).is_err() {
            return None;
        }
    }
    let (tx, rx) = mpsc::channel();
    let thread_token = token.clone();
    let _ = std::thread::Builder::new()
        .name("origo-ipc".into())
        .spawn(move || {
            for incoming in listener.incoming() {
                let Ok(mut stream) = incoming else {
                    continue;
                };
                let _ = stream.set_read_timeout(Some(IO_TIMEOUT));
                let _ = stream.set_write_timeout(Some(IO_TIMEOUT));
                let mut line = String::new();
                let read_ok = {
                    let mut reader = BufReader::new((&stream).take(MAX_PAYLOAD_BYTES as u64));
                    reader.read_line(&mut line).is_ok()
                };
                if read_ok
                    && let Some(intent) = decode_intent(&line, &thread_token)
                    && tx.send(intent).is_ok()
                {
                    let _ = stream.write_all(b"OK\n");
                    let _ = stream.flush();
                } else {
                    let _ = stream.write_all(b"ERR\n");
                    let _ = stream.flush();
                }
            }
        });
    Some((
        IpcGuard {
            endpoint_path: path,
            token,
        },
        rx,
    ))
}

pub(super) fn parse_manifest_file(path: &Path, delete_after: bool) -> anyhow::Result<Vec<PathBuf>> {
    let result = (|| -> anyhow::Result<Vec<PathBuf>> {
        let meta = fs::metadata(path)?;
        anyhow::ensure!(
            meta.len() <= MAX_MANIFEST_BYTES,
            "File daftar gambar terlalu besar"
        );
        let raw = fs::read_to_string(path)?;
        let content = raw.strip_prefix('\u{FEFF}').unwrap_or(&raw);
        let paths = content
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .map(PathBuf::from)
            .collect();
        Ok(paths)
    })();
    if delete_after {
        let _ = fs::remove_file(path);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ipc_roundtrip_and_token_validation() {
        let dir = tempfile::tempdir().ok();
        assert!(dir.is_some());
        let Some(dir) = dir else { return };
        let started = start_listener(dir.path());
        assert!(started.is_some());
        let Some((_guard, rx)) = started else { return };

        let dialog_intent = LaunchIntent::ImagesToPdfDialog(vec![
            PathBuf::from("C:\\tmp\\a.png"),
            PathBuf::from("C:\\tmp\\b.jpg"),
        ]);
        assert!(try_forward_to_running(dir.path(), &dialog_intent));
        assert_eq!(
            rx.recv_timeout(Duration::from_secs(2)).ok(),
            Some(dialog_intent)
        );

        let quick_intent = LaunchIntent::ImagesToPdfQuick(vec![PathBuf::from("C:\\tmp\\c.jpeg")]);
        assert!(try_forward_to_running(dir.path(), &quick_intent));
        assert_eq!(
            rx.recv_timeout(Duration::from_secs(2)).ok(),
            Some(quick_intent)
        );

        let split_intent = LaunchIntent::PdfAction {
            path: PathBuf::from("C:\\tmp\\doc.pdf"),
            action: PdfShellAction::Split,
        };
        assert!(try_forward_to_running(dir.path(), &split_intent));
        assert_eq!(
            rx.recv_timeout(Duration::from_secs(2)).ok(),
            Some(split_intent)
        );

        let merge_intent = LaunchIntent::PdfCombineQuick(vec![
            PathBuf::from("C:\\tmp\\1.pdf"),
            PathBuf::from("C:\\tmp\\2.pdf"),
        ]);
        assert!(try_forward_to_running(dir.path(), &merge_intent));
        assert_eq!(
            rx.recv_timeout(Duration::from_secs(2)).ok(),
            Some(merge_intent)
        );

        // Bad token must be rejected.
        let Some((port, _)) = read_endpoint(dir.path()) else {
            return;
        };
        let addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), port);
        if let Ok(mut stream) = TcpStream::connect_timeout(&addr, CONNECT_TIMEOUT) {
            let _ = stream
                .write_all(b"{\"token\":\"bad\",\"kind\":\"open_pdf\",\"paths\":[\"a.pdf\"]}\n");
            let mut reply = String::new();
            let _ = stream.read_to_string(&mut reply);
            assert_eq!(reply.trim(), "ERR");
        }
    }

    #[test]
    fn manifest_parses_bom_and_deletes_when_requested() {
        let dir = tempfile::tempdir().ok();
        assert!(dir.is_some());
        let Some(dir) = dir else { return };
        let manifest = dir.path().join("list.txt");
        assert!(fs::write(&manifest, "\u{FEFF}C:\\one.png\r\n\r\nC:\\two.jpg\n").is_ok());
        let parsed = parse_manifest_file(&manifest, true).ok();
        assert_eq!(
            parsed,
            Some(vec![
                PathBuf::from("C:\\one.png"),
                PathBuf::from("C:\\two.jpg")
            ])
        );
        assert!(!manifest.exists());
    }
}
