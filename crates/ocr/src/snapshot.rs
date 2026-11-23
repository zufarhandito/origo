//! An export freezes OCR coverage without keeping all page text in RAM.
use crate::{
    CacheError, PageKey, Recognition, Store,
    store::{MAX_RECORD, decode},
};
use pdf_application::export::{ExportControl, ExportError};
use rusqlite::{OptionalExtension, params};
use std::{
    collections::HashMap,
    fs::File,
    io::{Read, Seek, SeekFrom, Write},
    sync::Mutex,
};

#[derive(Clone, Copy)]
struct Span {
    offset: u64,
    length: usize,
}

pub struct FrozenResults {
    data: Mutex<File>,
    pages: Vec<Option<Span>>,
    recognized: usize,
}

impl FrozenResults {
    pub fn pages(&self) -> usize {
        self.pages.len()
    }

    pub fn recognized(&self) -> usize {
        self.recognized
    }

    pub fn get(&self, index: usize) -> Result<Option<Recognition>, CacheError> {
        let Some(span) = self.pages.get(index).and_then(|s| *s) else {
            return Ok(None);
        };
        let mut file = self.data.lock().map_err(|_| CacheError::Storage)?;
        file.seek(SeekFrom::Start(span.offset))
            .map_err(|_| CacheError::Storage)?;
        let mut bytes = vec![0; span.length];
        file.read_exact(&mut bytes)
            .map_err(|_| CacheError::Storage)?;
        decode(&bytes).map(Some).ok_or(CacheError::Storage)
    }
}

impl Store {
    pub fn freeze(
        &self,
        keys: &[PageKey],
        control: &ExportControl,
    ) -> Result<FrozenResults, ExportError> {
        control.checkpoint()?;
        if keys.len() > crate::MAX_BATCH_PAGES {
            return Err(ExportError::ResourceLimit);
        }
        let mut file = self
            .path
            .as_deref()
            .and_then(|p| p.parent())
            .map_or_else(tempfile::tempfile, |parent| {
                std::fs::create_dir_all(parent)?;
                tempfile::tempfile_in(parent)
            })
            .map_err(|_| ExportError::Io)?;
        let mut seen = HashMap::new();
        let mut pages = Vec::with_capacity(keys.len());
        let mut offset = 0;
        let mut recognized = 0;
        let snapshot = self.with_connection(|db| {
            let tx = db.transaction()?;
            let mut statement = tx.prepare(
                "SELECT data FROM ocr_result WHERE page_key=?1 AND kind=0 AND length(data)<=?2",
            )?;
            for key in keys {
                if control.checkpoint().is_err() {
                    break;
                }
                let span = if let Some(span) = seen.get(key) {
                    *span
                } else {
                    let bytes: Option<Vec<u8>> = statement
                        .query_row(params![key.0, MAX_RECORD as i64], |row| row.get(0))
                        .optional()?;
                    let span = bytes
                        .and_then(|bytes| {
                            let content = decode(&bytes)?;
                            (!content.page.words.is_empty()).then_some(bytes)
                        })
                        .map(|bytes| {
                            if offset + bytes.len() as u64 > 192 * 1024 * 1024 {
                                return Err(rusqlite::Error::InvalidQuery);
                            }
                            file.write_all(&bytes).map_err(|e| {
                                rusqlite::Error::ToSqlConversionFailure(Box::new(e))
                            })?;
                            let span = Span {
                                offset,
                                length: bytes.len(),
                            };
                            offset += bytes.len() as u64;
                            Ok(span)
                        })
                        .transpose()?;
                    seen.insert(key, span);
                    span
                };
                recognized += usize::from(span.is_some());
                pages.push(span);
            }
            Ok(())
        });
        control.checkpoint()?;
        snapshot.map_err(|_| ExportError::Io)?;
        Ok(FrozenResults {
            data: Mutex::new(file),
            pages,
            recognized,
        })
    }
}
