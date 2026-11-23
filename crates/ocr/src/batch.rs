use crate::{Cached, DocumentError, DocumentPage, OcrError, PageKey, Store, document::verify};
use std::collections::{HashMap, HashSet, VecDeque};

pub const MAX_BATCH_PAGES: usize = 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PageStatus {
    Pending,
    Working,
    Ready,
    Empty,
    Failed,
}

impl From<&Cached> for PageStatus {
    fn from(value: &Cached) -> Self {
        match value {
            Cached::Native => Self::Ready,
            Cached::Text(result) if result.page.words.is_empty() => Self::Empty,
            Cached::Text(_) => Self::Ready,
            Cached::Failed => Self::Failed,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Progress {
    pub ready: usize,
    pub empty: usize,
    pub failed: usize,
    pub total: usize,
}

impl Progress {
    pub fn remaining(self) -> usize {
        self.total - self.ready - self.empty - self.failed
    }
}

struct Entry {
    page: DocumentPage,
    positions: Vec<usize>,
    status: PageStatus,
}

pub struct BatchPlan {
    entries: Vec<Entry>,
    by_key: HashMap<PageKey, usize>,
    queue: VecDeque<usize>,
    progress: Progress,
}

impl BatchPlan {
    pub fn load(pages: Vec<DocumentPage>, store: &Store) -> Result<Self, DocumentError> {
        if pages.is_empty() || pages.len() > MAX_BATCH_PAGES {
            return Err(OcrError::Limit.into());
        }
        let mut plan = Self {
            entries: Vec::new(),
            by_key: HashMap::new(),
            queue: VecDeque::new(),
            progress: Progress {
                total: pages.len(),
                ..Progress::default()
            },
        };
        let mut sources = HashSet::new();
        for (position, page) in pages.into_iter().enumerate() {
            if sources.insert((page.request.path.clone(), page.request.fingerprint)) {
                verify(&page.request)?;
            }
            let key = page.key();
            if let Some(&id) = plan.by_key.get(&key) {
                let entry = &mut plan.entries[id];
                entry.positions.push(position + 1);
                Self::count(&mut plan.progress, entry.status, 1, true);
                continue;
            }
            let status = store
                .get(&key)?
                .as_ref()
                .map_or(PageStatus::Pending, PageStatus::from);
            let id = plan.entries.len();
            if status == PageStatus::Pending {
                plan.queue.push_back(id);
            }
            Self::count(&mut plan.progress, status, 1, true);
            plan.by_key.insert(key, id);
            plan.entries.push(Entry {
                page,
                positions: vec![position + 1],
                status,
            });
        }
        Ok(plan)
    }

    fn count(progress: &mut Progress, status: PageStatus, positions: usize, add: bool) {
        let counter = match status {
            PageStatus::Ready => &mut progress.ready,
            PageStatus::Empty => &mut progress.empty,
            PageStatus::Failed => &mut progress.failed,
            _ => return,
        };
        if add {
            *counter += positions;
        } else {
            *counter -= positions;
        }
    }

    pub fn progress(&self) -> Progress {
        self.progress
    }

    pub fn failed_pages(&self) -> Vec<usize> {
        let mut pages: Vec<_> = self
            .entries
            .iter()
            .filter(|entry| entry.status == PageStatus::Failed)
            .flat_map(|entry| entry.positions.iter().copied())
            .collect();
        pages.sort_unstable();
        pages
    }

    pub fn claim_next(&mut self) -> Option<(usize, DocumentPage)> {
        let id = self.queue.pop_front()?;
        let entry = &mut self.entries[id];
        entry.status = PageStatus::Working;
        Some((id, entry.page.clone()))
    }

    pub fn claim_priority(&mut self, key: &PageKey) -> Option<usize> {
        let id = *self.by_key.get(key)?;
        let entry = &mut self.entries[id];
        if !matches!(
            entry.status,
            PageStatus::Pending | PageStatus::Failed | PageStatus::Empty
        ) {
            return None;
        }
        self.queue.retain(|pending| *pending != id);
        Self::count(
            &mut self.progress,
            entry.status,
            entry.positions.len(),
            false,
        );
        entry.status = PageStatus::Working;
        Some(id)
    }

    pub fn finish(&mut self, id: usize, status: PageStatus) {
        let Some(entry) = self
            .entries
            .get_mut(id)
            .filter(|entry| entry.status == PageStatus::Working)
        else {
            return;
        };
        entry.status = status;
        if status == PageStatus::Pending {
            self.queue.push_front(id);
        } else {
            Self::count(&mut self.progress, status, entry.positions.len(), true);
        }
    }

    pub fn retry_failed(&mut self) {
        for (id, entry) in self.entries.iter_mut().enumerate() {
            if entry.status == PageStatus::Failed {
                Self::count(
                    &mut self.progress,
                    entry.status,
                    entry.positions.len(),
                    false,
                );
                entry.status = PageStatus::Pending;
                self.queue.push_back(id);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pdf_application::{PageGeometry, text::TextRequest};

    #[test]
    fn thousand_page_checkpoint_priority_failure_and_resume()
    -> Result<(), Box<dyn std::error::Error>> {
        let dir = tempfile::tempdir()?;
        let path = dir.path().join("synthetic.pdf");
        std::fs::write(&path, b"synthetic")?;
        let fingerprint = pdf_platform::fingerprint(&path)?;
        let pages: Vec<_> = (0..1000)
            .map(|page_index| DocumentPage {
                request: TextRequest {
                    path: path.clone(),
                    page_index,
                    fingerprint,
                },
                geometry: PageGeometry {
                    width_points: 595.,
                    height_points: 842.,
                    quarter_turns: 0,
                },
                rotation: 0,
                language: Default::default(),
            })
            .collect();
        let store = Store::new(Some(dir.path()));
        let mut batch = BatchPlan::load(pages.clone(), &store)?;
        let priority = batch
            .claim_priority(&pages[999].key())
            .ok_or("priority lost")?;
        batch.finish(priority, PageStatus::Ready);
        store.put(&pages[999].key(), &Cached::Native)?;
        let (first, page) = batch.claim_next().ok_or("first page missing")?;
        assert_eq!(page.request.page_index, 0);
        batch.finish(first, PageStatus::Failed);
        store.put(&page.key(), &Cached::Failed)?;
        batch.claim_next().ok_or("in-flight page missing")?;
        drop(batch);
        drop(store);
        let store = Store::new(Some(dir.path()));
        let mut resumed = BatchPlan::load(pages, &store)?;
        assert_eq!(
            resumed.progress(),
            Progress {
                ready: 1,
                failed: 1,
                empty: 0,
                total: 1000
            }
        );
        assert_eq!(resumed.failed_pages(), [1]);
        assert_eq!(
            resumed
                .claim_next()
                .ok_or("interrupted page missing")?
                .1
                .request
                .page_index,
            1
        );
        resumed.retry_failed();
        assert!(resumed.failed_pages().is_empty());
        assert_eq!(resumed.progress().remaining(), 999);
        Ok(())
    }
}
