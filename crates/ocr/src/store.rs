use crate::{ENGINE_ID, Language, Page};
use flate2::{Compression, read::ZlibDecoder, write::ZlibEncoder};
use pdf_application::text::TextRequest;
use rusqlite::{Connection, OptionalExtension, TransactionBehavior, params};
use serde::{Deserialize, Serialize};
use std::{
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};
#[path = "snapshot.rs"]
mod snapshot;
pub use snapshot::FrozenResults;

const MAX_RECORD: usize = 2 * 1024 * 1024;
const MAX_ENTRIES: i64 = 8192;
const DATA_BUDGET: i64 = 192 * 1024 * 1024;
const MAX_DB_PAGES: u32 = 65_536;

#[derive(Debug, thiserror::Error)]
pub enum CacheError {
    #[error("Penyimpanan OCR lokal tidak dapat dibaca atau ditulis.")]
    Storage,
    #[error("Hasil OCR melebihi batas penyimpanan.")]
    Limit,
}

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct PageKey(String);

impl PageKey {
    pub fn new(request: &TextRequest, language: Language) -> Self {
        // A path is supplied by PDFium inspection, normalized the same way as
        // desktop tabs on Windows. Length-prefix the path to keep the key unique.
        let path = request.path.to_string_lossy();
        #[cfg(windows)]
        let path = path.replace('/', "\\").to_lowercase();
        Self(format!(
            "{ENGINE_ID}:{}:{}:{path}:{}:{}:{}",
            language.code(),
            path.len(),
            request.fingerprint.bytes,
            request.fingerprint.modified_ns,
            request.page_index,
        ))
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Recognition {
    pub page: Page,
    /// Clockwise rotation applied to the source-normalized frame for recognition.
    pub rotation: u8,
}

#[derive(Clone, Debug)]
pub enum Cached {
    Text(Recognition),
    Native,
    Failed,
}

#[derive(Clone)]
pub struct Store {
    path: Option<PathBuf>,
    connection: Arc<Mutex<Option<Connection>>>,
}

impl Store {
    pub fn new(state: Option<&Path>) -> Self {
        Self {
            path: state.map(|s| s.join("ocr-v1.sqlite3")),
            connection: Arc::default(),
        }
    }

    fn with_connection<T>(
        &self,
        operation: impl FnOnce(&mut Connection) -> Result<T, rusqlite::Error>,
    ) -> Result<T, CacheError> {
        let mut guard = self.connection.lock().map_err(|_| CacheError::Storage)?;
        if guard.is_none() {
            let connection = if let Some(path) = &self.path {
                if let Some(parent) = path.parent() {
                    std::fs::create_dir_all(parent).map_err(|_| CacheError::Storage)?;
                }
                Connection::open(path)
            } else {
                Connection::open_in_memory()
            }
            .map_err(|_| CacheError::Storage)?;
            connection
                .execute_batch(
                    "PRAGMA page_size=4096;
                     PRAGMA auto_vacuum=INCREMENTAL;
                     PRAGMA synchronous=FULL;
                     PRAGMA busy_timeout=2000;
                     PRAGMA secure_delete=ON;
                     CREATE TABLE IF NOT EXISTS ocr_result (
                         id INTEGER PRIMARY KEY,
                         page_key TEXT NOT NULL UNIQUE,
                         kind INTEGER NOT NULL,
                         data BLOB NOT NULL
                     );
                     CREATE TABLE IF NOT EXISTS ocr_checkpoint (
                         id INTEGER PRIMARY KEY,
                         source_key TEXT NOT NULL UNIQUE
                     );",
                )
                .map_err(|_| CacheError::Storage)?;
            connection
                .pragma_update(None, "max_page_count", MAX_DB_PAGES)
                .map_err(|_| CacheError::Storage)?;
            *guard = Some(connection);
        }
        operation(guard.as_mut().ok_or(CacheError::Storage)?).map_err(|_| CacheError::Storage)
    }

    pub fn get(&self, key: &PageKey) -> Result<Option<Cached>, CacheError> {
        if self.path.as_ref().is_some_and(|path| !path.exists()) {
            return Ok(None);
        }
        let record: Option<(i64, Vec<u8>)> = self.with_connection(|db| {
            db.query_row(
                "SELECT kind, data FROM ocr_result WHERE page_key=?1 AND length(data)<=?2",
                params![key.0, MAX_RECORD as i64],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()
        })?;
        Ok(match record {
            Some((0, bytes)) => decode(&bytes).map(Cached::Text),
            Some((1, _)) => Some(Cached::Native),
            Some((2, _)) => Some(Cached::Failed),
            _ => None,
        })
    }

    pub fn put(&self, key: &PageKey, value: &Cached) -> Result<(), CacheError> {
        let (kind, data) = match value {
            Cached::Text(result) => (0, encode(result)?),
            Cached::Native => (1, vec![]),
            Cached::Failed => (2, vec![]),
        };
        self.with_connection(|db| {
            let tx = db.transaction_with_behavior(TransactionBehavior::Immediate)?;
            tx.execute(
                "INSERT OR REPLACE INTO ocr_result(page_key,kind,data) VALUES (?1,?2,?3)",
                params![key.0, kind, data],
            )?;
            loop {
                let (count, bytes): (i64, i64) = tx.query_row(
                    "SELECT COUNT(*), COALESCE(SUM(length(data)),0) FROM ocr_result",
                    [],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )?;
                if count <= MAX_ENTRIES && bytes <= DATA_BUDGET {
                    break;
                }
                tx.execute(
                    "DELETE FROM ocr_result WHERE id IN
                     (SELECT id FROM ocr_result ORDER BY id LIMIT 32)",
                    [],
                )?;
            }
            tx.commit()
        })
    }

    pub fn clear(&self) -> Result<(), CacheError> {
        self.with_connection(|db| {
            let tx = db.transaction_with_behavior(TransactionBehavior::Immediate)?;
            tx.execute("DELETE FROM ocr_result", [])?;
            tx.execute("DELETE FROM ocr_checkpoint", [])?;
            tx.commit()?;
            db.execute_batch("PRAGMA incremental_vacuum")
        })
    }

    pub fn has_checkpoint(&self, document: &PageKey) -> Result<bool, CacheError> {
        if self.path.as_ref().is_some_and(|path| !path.exists()) {
            return Ok(false);
        }
        self.with_connection(|db| {
            db.query_row(
                "SELECT EXISTS(SELECT 1 FROM ocr_checkpoint WHERE source_key=?1)",
                [&document.0],
                |row| row.get(0),
            )
        })
    }

    pub fn set_checkpoint(&self, document: &PageKey, pending: bool) -> Result<(), CacheError> {
        self.with_connection(|db| {
            if pending {
                let tx = db.transaction_with_behavior(TransactionBehavior::Immediate)?;
                tx.execute(
                    "INSERT OR REPLACE INTO ocr_checkpoint(source_key) VALUES (?1)",
                    [&document.0],
                )?;
                tx.execute(
                    "DELETE FROM ocr_checkpoint WHERE id IN
                     (SELECT id FROM ocr_checkpoint ORDER BY id DESC LIMIT -1 OFFSET 256)",
                    [],
                )?;
                tx.commit()
            } else {
                db.execute(
                    "DELETE FROM ocr_checkpoint WHERE source_key=?1",
                    [&document.0],
                )?;
                Ok(())
            }
        })
    }
}

fn encode(result: &Recognition) -> Result<Vec<u8>, CacheError> {
    if result.rotation > 3 || !result.page.valid() {
        return Err(CacheError::Limit);
    }
    let bytes = serde_json::to_vec(result).map_err(|_| CacheError::Storage)?;
    if bytes.len() > MAX_RECORD {
        return Err(CacheError::Limit);
    }
    let mut encoder = ZlibEncoder::new(Vec::new(), Compression::fast());
    encoder.write_all(&bytes).map_err(|_| CacheError::Storage)?;
    encoder.finish().map_err(|_| CacheError::Storage)
}

fn decode(bytes: &[u8]) -> Option<Recognition> {
    let mut text = Vec::new();
    ZlibDecoder::new(bytes)
        .take(MAX_RECORD as u64 + 1)
        .read_to_end(&mut text)
        .ok()?;
    if text.len() > MAX_RECORD {
        return None;
    }
    let result: Recognition = serde_json::from_slice(&text).ok()?;
    (result.rotation <= 3 && result.page.valid()).then_some(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use pdf_application::SourceFingerprint;

    #[test]
    fn cache_survives_reopen_and_keeps_results_separate() -> Result<(), Box<dyn std::error::Error>>
    {
        let directory = tempfile::tempdir()?;
        let mut view = TextRequest {
            path: directory.path().join("bukü.pdf"),
            page_index: 2,
            fingerprint: SourceFingerprint {
                bytes: 1,
                modified_ns: 2,
            },
        };
        let key = PageKey::new(&view, Language::IndonesianEnglish);
        let store = Store::new(Some(directory.path()));
        store.put(
            &key,
            &Cached::Text(Recognition {
                page: Page::default(),
                rotation: 3,
            }),
        )?;
        drop(store);
        let store = Store::new(Some(directory.path()));
        assert!(matches!(
            store.get(&key)?,
            Some(Cached::Text(Recognition { rotation: 3, .. }))
        ));
        assert!(
            store
                .get(&PageKey::new(&view, Language::English))?
                .is_none()
        );
        view.fingerprint.modified_ns += 1;
        assert!(
            store
                .get(&PageKey::new(&view, Language::IndonesianEnglish))?
                .is_none()
        );
        store.with_connection(|db| db.execute("UPDATE ocr_result SET data=X'FFFF'", []))?;
        assert!(store.get(&key)?.is_none());
        store.put(&key, &Cached::Native)?;
        assert!(matches!(store.get(&key)?, Some(Cached::Native)));
        store.clear()?;
        assert!(store.get(&key)?.is_none());
        Ok(())
    }

    #[test]
    fn full_database_keeps_committed_results_and_checkpoint()
    -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempfile::tempdir()?;
        let key = PageKey("existing".into());
        let oversized = PageKey("x".repeat(32 * 1024));
        let store = Store::new(Some(directory.path()));
        store.put(&key, &Cached::Native)?;
        store.set_checkpoint(&key, true)?;
        store.with_connection(|db| {
            let pages = db.pragma_query_value(None, "page_count", |row| row.get::<_, u32>(0))?;
            db.pragma_update(None, "max_page_count", pages)
        })?;

        assert!(matches!(
            store.put(&oversized, &Cached::Native),
            Err(CacheError::Storage)
        ));
        drop(store);
        let resumed = Store::new(Some(directory.path()));
        assert!(matches!(resumed.get(&key)?, Some(Cached::Native)));
        assert!(resumed.has_checkpoint(&key)?);
        assert!(resumed.get(&oversized)?.is_none());
        Ok(())
    }
}
