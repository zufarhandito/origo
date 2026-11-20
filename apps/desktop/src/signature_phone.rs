//! Short-lived, single-use LAN drawing sessions. No PDF bytes or external services.
use std::{
    io::{Read, Write},
    net::{TcpListener, TcpStream, UdpSocket},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    time::{Duration, Instant},
};
pub(super) type Strokes = Vec<Vec<[f32; 2]>>;
pub(super) struct PhoneSession {
    pub url: String,
    pub qr: pdf_domain::annotation::SignatureImage,
    stop: Arc<AtomicBool>,
    receiver: mpsc::Receiver<Result<Strokes, String>>,
}
impl Drop for PhoneSession {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
    }
}
impl PhoneSession {
    pub fn poll(&self) -> Option<Result<Strokes, String>> {
        self.receiver.try_recv().ok()
    }
    pub fn start() -> Result<Self, String> {
        let route = UdpSocket::bind("0.0.0.0:0").map_err(|e| e.to_string())?;
        // UDP connect only selects the local route; no packet is sent.
        route
            .connect("192.0.2.1:80")
            .map_err(|_| "Jaringan LAN tidak tersedia.")?;
        let ip = route.local_addr().map_err(|e| e.to_string())?.ip();
        if ip.is_loopback() || ip.is_unspecified() {
            return Err("Alamat LAN tidak ditemukan.".into());
        }
        Self::bind(ip)
    }
    fn bind(ip: std::net::IpAddr) -> Result<Self, String> {
        Self::bind_with_ttl(ip, Duration::from_secs(300))
    }
    fn bind_with_ttl(ip: std::net::IpAddr, ttl: Duration) -> Result<Self, String> {
        let listener =
            TcpListener::bind((ip, 0)).map_err(|e| format!("Listener LAN gagal: {e}"))?;
        listener.set_nonblocking(true).map_err(|e| e.to_string())?;
        let host = listener
            .local_addr()
            .map_err(|e| e.to_string())?
            .to_string();
        let mut random = [0u8; 24];
        getrandom::fill(&mut random).map_err(|e| e.to_string())?;
        let token = random
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>();
        let path = format!("/draw/{token}");
        let url = format!("http://{host}{path}");
        let qr = qr_image(&url)?;
        let stop = Arc::new(AtomicBool::new(false));
        let stopped = stop.clone();
        let (sender, receiver) = mpsc::channel();
        std::thread::Builder::new()
            .name("origo-phone-signature".into())
            .spawn(move || {
                let deadline = Instant::now() + ttl;
                while !stopped.load(Ordering::SeqCst) && Instant::now() < deadline {
                    match listener.accept() {
                        Ok((mut stream, _)) => {
                            // Windows accepted sockets can inherit the listener's nonblocking mode.
                            if stream.set_nonblocking(false).is_err() {
                                continue;
                            }

                            let _ = stream.set_read_timeout(Some(Duration::from_secs(2)));
                            let _ = stream.set_write_timeout(Some(Duration::from_secs(2)));
                            match request(&mut stream, &host, &path, &stopped, deadline) {
                                Ok(Some(strokes)) => {
                                    let _ = sender.send(Ok(strokes));
                                    return;
                                }
                                Ok(None) => {}
                                Err(_) => {
                                    let _ = respond(
                                        &mut stream,
                                        "400 Bad Request",
                                        "text/plain",
                                        b"Invalid or expired request",
                                    );
                                }
                            }
                        }
                        Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                            std::thread::sleep(Duration::from_millis(40))
                        }
                        Err(e) => {
                            let _ = sender.send(Err(e.to_string()));
                            return;
                        }
                    }
                }
                if !stopped.load(Ordering::SeqCst) {
                    let _ = sender.send(Err("QR kedaluwarsa. Buat QR baru.".into()));
                }
            })
            .map_err(|e| e.to_string())?;
        Ok(Self {
            url,
            qr,
            stop,
            receiver,
        })
    }
}
fn qr_image(url: &str) -> Result<pdf_domain::annotation::SignatureImage, String> {
    let qr = qrcode::QrCode::new(url.as_bytes()).map_err(|e| e.to_string())?;
    let side = qr.width() + 8;
    let size = side * 4;
    let mut rgba = vec![255; size * size * 4];
    for y in 0..qr.width() {
        for x in 0..qr.width() {
            if qr[(x, y)] == qrcode::Color::Dark {
                for dy in 0..4 {
                    for dx in 0..4 {
                        let i = (((y + 4) * 4 + dy) * size + (x + 4) * 4 + dx) * 4;
                        rgba[i..i + 3].fill(0);
                    }
                }
            }
        }
    }
    Ok(pdf_domain::annotation::SignatureImage {
        width: size as u32,
        height: size as u32,
        rgba,
    })
}
fn respond(s: &mut TcpStream, status: &str, kind: &str, body: &[u8]) -> std::io::Result<()> {
    write!(
        s,
        "HTTP/1.1 {status}\r\nContent-Type: {kind}\r\nContent-Length: {}\r\nConnection: close\r\nCache-Control: no-store\r\nReferrer-Policy: no-referrer\r\nX-Content-Type-Options: nosniff\r\nContent-Security-Policy: default-src 'none'; script-src 'unsafe-inline'; style-src 'unsafe-inline'; connect-src 'self'; frame-ancestors 'none'; base-uri 'none'\r\n\r\n",
        body.len()
    )?;
    s.write_all(body)
}
fn request(
    s: &mut TcpStream,
    host: &str,
    path: &str,
    stop: &AtomicBool,
    deadline: Instant,
) -> anyhow::Result<Option<Strokes>> {
    let mut bytes = Vec::new();
    let mut part = [0; 2048];
    let head_end = loop {
        anyhow::ensure!(
            !stop.load(Ordering::SeqCst) && Instant::now() < deadline,
            "expired"
        );
        let n = s.read(&mut part)?;
        anyhow::ensure!(n > 0, "closed");
        bytes.extend_from_slice(&part[..n]);
        if let Some(p) = bytes.windows(4).position(|p| p == b"\r\n\r\n") {
            break p + 4;
        }
        anyhow::ensure!(bytes.len() <= 8192, "header too large");
    };
    anyhow::ensure!(head_end <= 8192, "header too large");
    let head = std::str::from_utf8(&bytes[..head_end])?;
    let mut lines = head.split("\r\n");
    let first = lines
        .next()
        .unwrap_or_default()
        .split_whitespace()
        .collect::<Vec<_>>();
    anyhow::ensure!(first.len() == 3, "request");
    let method = first[0].to_owned();
    let target = first[1].to_owned();
    let mut headers = std::collections::HashMap::new();
    for line in lines.filter(|l| !l.is_empty()) {
        let (k, v) = line
            .split_once(':')
            .ok_or_else(|| anyhow::anyhow!("header"))?;
        anyhow::ensure!(
            headers
                .insert(k.trim().to_ascii_lowercase(), v.trim().to_owned())
                .is_none(),
            "duplicate header"
        );
    }
    anyhow::ensure!(headers.get("host").is_some_and(|v| v == host), "host");
    anyhow::ensure!(
        !stop.load(Ordering::SeqCst) && Instant::now() < deadline,
        "expired"
    );
    if method == "GET" && target == path {
        respond(
            s,
            "200 OK",
            "text/html; charset=utf-8",
            include_bytes!("../assets/phone-signature.html"),
        )?;
        return Ok(None);
    }
    anyhow::ensure!(
        method == "POST" && target == format!("{path}/submit"),
        "route"
    );
    anyhow::ensure!(
        headers
            .get("origin")
            .is_some_and(|v| v == &format!("http://{host}")),
        "origin"
    );
    anyhow::ensure!(
        !headers.contains_key("transfer-encoding")
            && headers
                .get("content-type")
                .is_some_and(|v| v == "application/json"),
        "type"
    );
    let count = headers
        .get("content-length")
        .and_then(|v| v.parse::<usize>().ok())
        .filter(|v| *v <= 512 * 1024 && *v > 0)
        .ok_or_else(|| anyhow::anyhow!("body limit"))?;
    while bytes.len() - head_end < count {
        anyhow::ensure!(
            !stop.load(Ordering::SeqCst) && Instant::now() < deadline,
            "expired"
        );
        let n = s.read(&mut part)?;
        anyhow::ensure!(n > 0, "closed");
        bytes.extend_from_slice(&part[..n]);
        anyhow::ensure!(bytes.len() <= head_end + 512 * 1024, "body limit");
    }
    anyhow::ensure!(bytes.len() == head_end + count, "extra body");
    let strokes: Strokes = serde_json::from_slice(&bytes[head_end..])?;
    validate(&strokes)?;
    anyhow::ensure!(
        !stop.load(Ordering::SeqCst) && Instant::now() < deadline,
        "expired"
    );
    respond(s, "200 OK", "text/plain", b"Signature received")?;
    Ok(Some(strokes))
}
fn validate(strokes: &Strokes) -> anyhow::Result<()> {
    anyhow::ensure!(
        !strokes.is_empty()
            && strokes.len() <= 128
            && strokes.iter().map(Vec::len).sum::<usize>() <= 16384,
        "stroke limit"
    );
    anyhow::ensure!(
        strokes.iter().any(|s| s.len() > 1)
            && strokes.iter().all(|s| !s.is_empty()
                && s.len() <= 1024
                && s.iter()
                    .flatten()
                    .all(|v| v.is_finite() && (0.0..=1.0).contains(v))),
        "invalid points"
    );
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    fn exchange(url: &str, path: &str, origin: bool, body: &str) -> String {
        let host = url
            .trim_start_matches("http://")
            .split('/')
            .next()
            .unwrap_or_default();
        let mut s = TcpStream::connect(host).unwrap_or_else(|e| panic!("connect: {e}"));
        let origin = if origin {
            format!("Origin: http://{host}\r\n")
        } else {
            String::new()
        };
        let req = format!(
            "POST {path} HTTP/1.1\r\nHost: {host}\r\n{origin}Content-Type: application/json\r\nContent-Length: {}\r\n\r\n{body}",
            body.len()
        );
        let _ = s.write_all(req.as_bytes());
        let mut out = String::new();
        let _ = s.read_to_string(&mut out);
        out
    }
    #[test]
    fn fragmented_http_body_waits_for_later_packet() -> anyhow::Result<()> {
        let session = PhoneSession::bind("127.0.0.1".parse()?).map_err(anyhow::Error::msg)?;
        let host = session
            .url
            .trim_start_matches("http://")
            .split('/')
            .next()
            .unwrap_or_default();
        let path = format!("/{}", session.url.splitn(4, '/').nth(3).unwrap_or_default());
        let body = "[[[0.1,0.2],[0.8,0.7]]]";
        let mut socket = TcpStream::connect(host)?;
        write!(
            socket,
            "POST {path}/submit HTTP/1.1\r\nHost: {host}\r\nOrigin: http://{host}\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n",
            body.len()
        )?;
        std::thread::sleep(Duration::from_millis(100));
        socket.write_all(body.as_bytes())?;
        let mut reply = String::new();
        socket.read_to_string(&mut reply)?;
        assert!(reply.contains("200 OK"));
        Ok(())
    }
    #[test]
    fn expiry_and_cancellation_release_listener() -> anyhow::Result<()> {
        let session = PhoneSession::bind_with_ttl("127.0.0.1".parse()?, Duration::from_millis(30))
            .map_err(anyhow::Error::msg)?;
        std::thread::sleep(Duration::from_millis(120));
        assert!(session.poll().is_some_and(|v| v.is_err()));
        let session = PhoneSession::bind("127.0.0.1".parse()?).map_err(anyhow::Error::msg)?;
        let host = session
            .url
            .trim_start_matches("http://")
            .split('/')
            .next()
            .unwrap_or_default()
            .to_owned();
        drop(session);
        std::thread::sleep(Duration::from_millis(120));
        assert!(TcpStream::connect(host).is_err());
        Ok(())
    }
    #[test]
    fn lan_pairing_rejects_wrong_token_origin_and_payload_then_receives_once() -> anyhow::Result<()>
    {
        let session = PhoneSession::bind("127.0.0.1".parse()?).map_err(anyhow::Error::msg)?;
        let path = format!("/{}", session.url.splitn(4, '/').nth(3).unwrap_or_default());
        let submit = format!("{path}/submit");
        assert!(exchange(&session.url, "/wrong", true, "[]").contains("400"));
        assert!(exchange(&session.url, &submit, false, "[]").contains("400"));
        assert!(exchange(&session.url, &submit, true, "[[[2,0],[0,1]]]").contains("400"));
        assert!(exchange(&session.url, &submit, true, "[[[0.1,0.2],[0.8,0.7]]]").contains("200"));
        let end = Instant::now() + Duration::from_secs(2);
        loop {
            if let Some(result) = session.poll() {
                assert_eq!(result.map_err(anyhow::Error::msg)?.len(), 1);
                break;
            }
            anyhow::ensure!(Instant::now() < end, "receive timeout");
            std::thread::sleep(Duration::from_millis(10));
        }
        assert!(session.poll().is_none());
        Ok(())
    }
}
