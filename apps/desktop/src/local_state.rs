//! Local UI preferences, never written into a source PDF.
use std::{
    collections::BTreeSet,
    io::{Read, Write},
    path::{Path, PathBuf},
};

const MAX_BYTES: u64 = 1024 * 1024;
const MAX_FAVORITES: usize = 4096;

#[derive(Clone, Debug, Default, PartialEq)]
pub(super) struct SessionSnapshot {
    pub documents: Vec<SavedDocument>,
    pub active: Option<usize>,
}
#[derive(Clone, Debug, PartialEq)]
pub(super) struct SavedDocument {
    pub path: PathBuf,
    pub fingerprint: pdf_domain::SourceFingerprint,
    pub position: crate::reading_position::ReadingPosition,
}

pub(super) fn load_session(dir: &Path) -> anyhow::Result<Option<SessionSnapshot>> {
    let file = match std::fs::File::open(dir.join("session.json")) {
        Ok(file) => file,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e.into()),
    };
    let mut bytes = Vec::new();
    file.take(MAX_BYTES + 1).read_to_end(&mut bytes)?;
    anyhow::ensure!(bytes.len() as u64 <= MAX_BYTES, "Sesi terlalu besar");
    parse_session(&bytes).map(Some)
}

fn parse_session(bytes: &[u8]) -> anyhow::Result<SessionSnapshot> {
    use pdf_application::viewer::Zoom;
    let value: serde_json::Value = serde_json::from_slice(bytes)?;
    anyhow::ensure!(value["version"] == 1, "Versi sesi tidak didukung");
    let docs = value["documents"]
        .as_array()
        .ok_or_else(|| anyhow::anyhow!("Daftar sesi tidak valid"))?;
    anyhow::ensure!(docs.len() <= 8, "Terlalu banyak tab");
    let mut documents = Vec::new();
    let mut paths = BTreeSet::new();
    for doc in docs {
        let path = PathBuf::from(
            doc["path"]
                .as_str()
                .ok_or_else(|| anyhow::anyhow!("Path tidak valid"))?,
        );
        anyhow::ensure!(
            path.is_absolute()
                && paths.insert(path.to_string_lossy().replace('/', "\\").to_lowercase()),
            "Path sesi tidak valid atau duplikat"
        );
        let page = doc["page"]
            .as_u64()
            .filter(|v| *v < 1024)
            .ok_or_else(|| anyhow::anyhow!("Halaman tidak valid"))? as u16;
        let number = |key: &str, low: f64, high: f64| -> anyhow::Result<f32> {
            Ok(doc[key]
                .as_f64()
                .filter(|v| v.is_finite() && (low..=high).contains(v))
                .ok_or_else(|| anyhow::anyhow!("Posisi tidak valid"))? as f32)
        };
        let zoom = match doc["zoom"].as_str() {
            Some("width") => Zoom::FitWidth,
            Some("page") => Zoom::FitPage,
            Some("percent") => Zoom::Percent(
                doc["percent"]
                    .as_u64()
                    .filter(|p| (50..=400).contains(p))
                    .ok_or_else(|| anyhow::anyhow!("Zoom tidak valid"))? as u16,
            ),
            _ => anyhow::bail!("Mode zoom tidak valid"),
        };
        let fingerprint = pdf_domain::SourceFingerprint {
            bytes: doc["bytes"]
                .as_u64()
                .ok_or_else(|| anyhow::anyhow!("Ukuran sumber tidak valid"))?,
            modified_ns: doc["modifiedNs"]
                .as_str()
                .ok_or_else(|| anyhow::anyhow!("Waktu sumber tidak valid"))?
                .parse()?,
        };
        documents.push(SavedDocument {
            path,
            fingerprint,
            position: crate::reading_position::ReadingPosition {
                page,
                zoom,
                offset: number("offset", -1., 1.)?,
                pan: number("pan", -1., 0.)?,
            },
        });
    }
    let active = if value["active"].is_null() {
        None
    } else {
        Some(
            value["active"]
                .as_u64()
                .filter(|i| (*i as usize) < documents.len())
                .ok_or_else(|| anyhow::anyhow!("Tab aktif tidak valid"))? as usize,
        )
    };
    Ok(SessionSnapshot { documents, active })
}

pub(super) fn save_session(dir: &Path, snapshot: &SessionSnapshot) -> anyhow::Result<()> {
    use pdf_application::viewer::Zoom;
    let docs: Vec<_> = snapshot.documents.iter().map(|doc| {
        let (zoom, percent) = match doc.position.zoom { Zoom::FitWidth => ("width", 0), Zoom::FitPage => ("page", 0), Zoom::Percent(p) => ("percent", p) };
        serde_json::json!({"path": doc.path.to_string_lossy(), "bytes": doc.fingerprint.bytes, "modifiedNs": doc.fingerprint.modified_ns.to_string(), "page": doc.position.page, "zoom": zoom, "percent": percent, "offset": doc.position.offset, "pan": doc.position.pan})
    }).collect();
    let bytes = serde_json::to_vec(
        &serde_json::json!({"version": 1, "documents": docs, "active": snapshot.active}),
    )?;
    anyhow::ensure!(bytes.len() as u64 <= MAX_BYTES, "Sesi terlalu besar");
    parse_session(&bytes)?;
    write_atomic(dir, "session.json", &bytes)
}

pub(super) fn write_atomic(dir: &Path, name: &str, bytes: &[u8]) -> anyhow::Result<()> {
    std::fs::create_dir_all(dir)?;
    let mut file = tempfile::NamedTempFile::new_in(dir)?;
    file.write_all(bytes)?;
    file.as_file().sync_all()?;
    file.persist(dir.join(name))?;
    Ok(())
}

pub(super) fn directory() -> Option<PathBuf> {
    std::env::var_os("PDF_STUDIO_STATE_DIR")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .or_else(|| std::env::var_os("LOCALAPPDATA").map(|p| PathBuf::from(p).join("PDF Studio")))
}

pub(super) fn load_favorites(dir: &Path) -> anyhow::Result<BTreeSet<String>> {
    let file = match std::fs::File::open(dir.join("favorites.json")) {
        Ok(file) => file,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(BTreeSet::new()),
        Err(e) => return Err(e.into()),
    };
    let mut bytes = Vec::new();
    file.take(MAX_BYTES + 1).read_to_end(&mut bytes)?;
    anyhow::ensure!(bytes.len() as u64 <= MAX_BYTES, "Preferensi terlalu besar");
    let value: serde_json::Value = serde_json::from_slice(&bytes)?;
    anyhow::ensure!(value["version"] == 1, "Versi preferensi tidak didukung");
    let stars = value["favorites"]
        .as_array()
        .ok_or_else(|| anyhow::anyhow!("Daftar favorit tidak valid"))?;
    anyhow::ensure!(stars.len() <= MAX_FAVORITES, "Terlalu banyak favorit");
    stars
        .iter()
        .map(|v| {
            v.as_str()
                .map(str::to_owned)
                .ok_or_else(|| anyhow::anyhow!("Favorit tidak valid"))
        })
        .collect()
}

pub(super) fn save_favorites(dir: &Path, favorites: &BTreeSet<String>) -> anyhow::Result<()> {
    anyhow::ensure!(favorites.len() <= MAX_FAVORITES, "Terlalu banyak favorit");
    let bytes = serde_json::to_vec(&serde_json::json!({"version": 1, "favorites": favorites}))?;
    anyhow::ensure!(bytes.len() as u64 <= MAX_BYTES, "Preferensi terlalu besar");
    write_atomic(dir, "favorites.json", &bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn favorites_atomic_replace_and_corrupt_input() -> anyhow::Result<()> {
        let dir = tempfile::tempdir()?;
        assert!(load_favorites(dir.path())?.is_empty());
        let stars = BTreeSet::from(["d:\\buku\\研究.pdf".into()]);
        save_favorites(dir.path(), &stars)?;
        assert_eq!(load_favorites(dir.path())?, stars);
        save_favorites(dir.path(), &BTreeSet::new())?;
        assert!(load_favorites(dir.path())?.is_empty());
        std::fs::write(dir.path().join("favorites.json"), b"broken")?;
        assert!(load_favorites(dir.path()).is_err());
        Ok(())
    }
    #[test]
    fn session_roundtrip_bounds_and_atomic_failure_preserve_previous_metadata() -> anyhow::Result<()>
    {
        use pdf_application::viewer::Zoom;
        let dir = tempfile::tempdir()?;
        assert!(load_session(dir.path())?.is_none());
        let documents = [Zoom::FitWidth, Zoom::FitPage, Zoom::Percent(400)]
            .into_iter()
            .enumerate()
            .map(|(i, zoom)| SavedDocument {
                path: dir.path().join(format!("研究-{i}.pdf")),
                fingerprint: pdf_domain::SourceFingerprint {
                    bytes: 123,
                    modified_ns: u128::MAX,
                },
                position: crate::reading_position::ReadingPosition {
                    page: 1023,
                    zoom,
                    offset: 0.25,
                    pan: -0.5,
                },
            })
            .collect();
        let snapshot = SessionSnapshot {
            documents,
            active: Some(2),
        };
        save_session(dir.path(), &snapshot)?;
        assert_eq!(load_session(dir.path())?, Some(snapshot.clone()));
        let previous = std::fs::read(dir.path().join("session.json"))?;
        let mut invalid = snapshot.clone();
        invalid.documents[0].position.page = 1024;
        assert!(save_session(dir.path(), &invalid).is_err());
        assert_eq!(std::fs::read(dir.path().join("session.json"))?, previous);
        let base: serde_json::Value = serde_json::from_slice(&previous)?;
        let mut too_many = base.clone();
        too_many["documents"] = serde_json::json!(vec![base["documents"][0].clone(); 9]);
        assert!(parse_session(&serde_json::to_vec(&too_many)?).is_err());
        let blocked = dir.path().join("blocked");
        std::fs::create_dir_all(blocked.join("session.json"))?;
        assert!(save_session(&blocked, &snapshot).is_err());
        assert!(blocked.join("session.json").is_dir());
        assert_eq!(std::fs::read_dir(&blocked)?.count(), 1);
        for (pointer, bad) in [
            ("/version", serde_json::json!(2)),
            ("/active", serde_json::json!(3)),
            ("/documents/0/path", serde_json::json!("relative.pdf")),
            ("/documents/0/offset", serde_json::json!(2.)),
            ("/documents/0/zoom", serde_json::json!("other")),
            ("/documents/0/modifiedNs", serde_json::json!("bad")),
        ] {
            let mut value = base.clone();
            *value
                .pointer_mut(pointer)
                .ok_or_else(|| anyhow::anyhow!("Missing test pointer"))? = bad;
            assert!(
                parse_session(&serde_json::to_vec(&value)?).is_err(),
                "{pointer}"
            );
        }
        save_session(dir.path(), &SessionSnapshot::default())?;
        assert_eq!(load_session(dir.path())?, Some(SessionSnapshot::default()));
        std::fs::write(
            dir.path().join("session.json"),
            vec![b' '; MAX_BYTES as usize + 1],
        )?;
        assert!(load_session(dir.path()).is_err());
        Ok(())
    }
}

pub(super) fn load_recents(dir: &Path) -> anyhow::Result<Vec<PathBuf>> {
    let file = match std::fs::File::open(dir.join("recents.json")) {
        Ok(f) => f,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(vec![]),
        Err(e) => return Err(e.into()),
    };
    let mut bytes = Vec::new();
    file.take(MAX_BYTES + 1).read_to_end(&mut bytes)?;
    anyhow::ensure!(bytes.len() as u64 <= MAX_BYTES, "Riwayat terlalu besar");
    let value: serde_json::Value = serde_json::from_slice(&bytes)?;
    anyhow::ensure!(value["version"] == 1, "Versi riwayat tidak didukung");
    let values = value["paths"]
        .as_array()
        .ok_or_else(|| anyhow::anyhow!("Riwayat tidak valid"))?;
    anyhow::ensure!(values.len() <= 20, "Riwayat terlalu panjang");
    let mut seen = BTreeSet::new();
    values
        .iter()
        .map(|v| {
            let raw = v
                .as_str()
                .ok_or_else(|| anyhow::anyhow!("Path tidak valid"))?;
            let path = PathBuf::from(raw);
            anyhow::ensure!(
                raw.len() <= 32768
                    && path.is_absolute()
                    && seen.insert(raw.replace('/', "\\").to_lowercase()),
                "Path riwayat tidak valid"
            );
            Ok(path)
        })
        .collect()
}
pub(super) fn save_recents(dir: &Path, paths: &[PathBuf]) -> anyhow::Result<()> {
    anyhow::ensure!(
        paths.len() <= 20
            && paths
                .iter()
                .all(|p| p.is_absolute() && p.to_string_lossy().len() <= 32768),
        "Riwayat tidak valid"
    );
    let bytes = serde_json::to_vec(&serde_json::json!({"version":1,"paths":paths}))?;
    anyhow::ensure!(bytes.len() as u64 <= MAX_BYTES, "Riwayat terlalu besar");
    write_atomic(dir, "recents.json", &bytes)
}
#[cfg(test)]
mod recent_tests {
    use super::*;
    #[test]
    fn recents_roundtrip_clear_and_reject_invalid_metadata() -> anyhow::Result<()> {
        let dir = tempfile::tempdir()?;
        let paths = vec![dir.path().join("B.pdf"), dir.path().join("A.pdf")];
        assert!(load_recents(dir.path())?.is_empty());
        save_recents(dir.path(), &paths)?;
        assert_eq!(load_recents(dir.path())?, paths);
        save_recents(dir.path(), &[])?;
        assert!(load_recents(dir.path())?.is_empty());
        std::fs::write(
            dir.path().join("recents.json"),
            br#"{"version":1,"paths":["relative.pdf"]}"#,
        )?;
        assert!(load_recents(dir.path()).is_err());
        std::fs::write(dir.path().join("recents.json"), b"broken")?;
        assert!(load_recents(dir.path()).is_err());
        Ok(())
    }
}
