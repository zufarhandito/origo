//! Opt-in local template library; never contains PDF documents.
use pdf_domain::annotation::{Annotation, SignatureImage};
use std::{io::Read, path::Path, sync::Arc};
#[derive(Clone)]
pub(super) struct Entry {
    pub label: String,
    pub kind: usize,
    pub asset: Arc<SignatureImage>,
}
// Up to eight 2 MiB RGBA assets, plus base64 and JSON overhead.
const LIMIT: u64 = 22 * 1024 * 1024;
pub(super) fn load(dir: &Path) -> anyhow::Result<Vec<Entry>> {
    let file = match std::fs::File::open(dir.join("signature-library.json")) {
        Ok(f) => f,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(vec![]),
        Err(e) => return Err(e.into()),
    };
    anyhow::ensure!(file.metadata()?.len() <= LIMIT, "Pustaka terlalu besar");
    let mut bytes = Vec::new();
    file.take(LIMIT + 1).read_to_end(&mut bytes)?;
    anyhow::ensure!(bytes.len() as u64 <= LIMIT, "Pustaka terlalu besar");
    let value: serde_json::Value = serde_json::from_slice(&bytes)?;
    anyhow::ensure!(value["version"] == 1, "Versi pustaka tidak didukung");
    let entries = value["entries"]
        .as_array()
        .ok_or_else(|| anyhow::anyhow!("Pustaka tidak valid"))?;
    anyhow::ensure!(entries.len() <= 8, "Maksimal delapan template");
    let mut labels = std::collections::BTreeSet::new();
    entries
        .iter()
        .map(|entry| {
            let label = entry["label"]
                .as_str()
                .ok_or_else(|| anyhow::anyhow!("Nama template tidak valid"))?
                .to_owned();
            valid_label(&label)?;
            anyhow::ensure!(labels.insert(label.clone()), "Nama duplikat");
            let kind = entry["kind"]
                .as_u64()
                .filter(|v| *v <= 2)
                .ok_or_else(|| anyhow::anyhow!("Jenis template tidak valid"))?
                as usize;
            let annotation = entry["asset"]
                .as_str()
                .and_then(Annotation::from_name)
                .ok_or_else(|| anyhow::anyhow!("Aset pustaka tidak valid"))?;
            Ok(Entry {
                label,
                kind,
                asset: annotation
                    .signature
                    .ok_or_else(|| anyhow::anyhow!("Bukan aset tanda tangan"))?,
            })
        })
        .collect()
}
pub(super) fn valid_label(label: &str) -> anyhow::Result<()> {
    anyhow::ensure!(
        !label.trim().is_empty()
            && label.chars().count() <= 40
            && !label.chars().any(char::is_control),
        "Nama template harus 1-40 karakter"
    );
    Ok(())
}
pub(super) fn save(dir: &Path, entries: &[Entry]) -> anyhow::Result<()> {
    anyhow::ensure!(entries.len() <= 8, "Maksimal delapan template");
    let mut labels = std::collections::BTreeSet::new();
    let mut values = Vec::new();
    for e in entries {
        valid_label(&e.label)?;
        anyhow::ensure!(
            labels.insert(e.label.clone()) && e.kind <= 2 && e.asset.valid(),
            "Template tidak valid/duplikat"
        );
        let annotation = Annotation {
            id: 1,
            highlight: false,
            color: [0; 3],
            width: 1.,
            points: vec![[0., 0.], [1., 1.]],
            rectangles: false,
            signature: Some(e.asset.clone()),
        };
        values.push(serde_json::json!({"label":e.label,"kind":e.kind,"asset":annotation.name()}));
    }
    let bytes = serde_json::to_vec(&serde_json::json!({"version":1,"entries":values}))?;
    anyhow::ensure!(bytes.len() as u64 <= LIMIT, "Pustaka terlalu besar");
    crate::local_state::write_atomic(dir, "signature-library.json", &bytes)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn library_roundtrip_and_corrupt_input_are_bounded() -> anyhow::Result<()> {
        let dir = tempfile::tempdir()?;
        assert!(load(dir.path())?.is_empty());
        let entry = Entry {
            label: "Pribadi".into(),
            kind: 1,
            asset: Arc::new(SignatureImage {
                width: 1,
                height: 1,
                rgba: vec![10, 20, 30, 128],
            }),
        };
        save(dir.path(), std::slice::from_ref(&entry))?;
        let loaded = load(dir.path())?;
        assert_eq!(loaded[0].asset, entry.asset);
        assert!(save(dir.path(), &vec![entry.clone(); 9]).is_err());
        assert!(save(dir.path(), &[entry.clone(), entry]).is_err());
        std::fs::write(dir.path().join("signature-library.json"), b"broken")?;
        assert!(load(dir.path()).is_err());
        assert!(valid_label(" ").is_err());
        Ok(())
    }
}
