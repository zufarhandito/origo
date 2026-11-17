//! Local UI preferences, never written into a source PDF.
use std::{
    collections::BTreeSet,
    io::{Read, Write},
    path::{Path, PathBuf},
};

const MAX_BYTES: u64 = 1024 * 1024;
const MAX_FAVORITES: usize = 4096;

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
    std::fs::create_dir_all(dir)?;
    let mut file = tempfile::NamedTempFile::new_in(dir)?;
    file.write_all(&bytes)?;
    file.as_file().sync_all()?;
    file.persist(dir.join("favorites.json"))?;
    Ok(())
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
}
