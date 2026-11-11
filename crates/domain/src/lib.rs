//! Pure document identity and invariants. No native engines, UI or filesystem access.
use std::{
    collections::{HashSet, VecDeque},
    path::PathBuf,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SourceId(pub u64);
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct PageId {
    pub source: SourceId,
    pub serial: u32,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SourceFingerprint {
    pub bytes: u64,
    pub modified_ns: u128,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PageGeometry {
    pub width_points: f32,
    pub height_points: f32,
    pub quarter_turns: u8,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExportSupport {
    NotPreflighted,
}

#[derive(Clone, Debug)]
pub struct SourceDocument {
    pub id: SourceId,
    pub path: PathBuf,
    pub fingerprint: SourceFingerprint,
    pub geometries: Vec<PageGeometry>,
    pub export_support: ExportSupport,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PageEntry {
    pub id: PageId,
    pub source_id: SourceId,
    pub source_page_index: u16,
    pub rotation_delta: u8,
}

#[derive(Debug, thiserror::Error)]
pub enum DomainError {
    #[error("Dokumen tidak memiliki halaman yang valid.")]
    InvalidPages,
    #[error("Referensi halaman tidak valid.")]
    InvalidReference,
    #[error("Batas workspace: maksimum 1024 halaman sumber dan total 128 MiB.")]
    ResourceLimit,
    #[error("PDF ini sudah ada di workspace. Duplikasi halaman belum didukung.")]
    DuplicateSource,
}

#[derive(Clone, Debug)]
pub enum Command {
    Rotate {
        ids: Vec<PageId>,
        turns: u8,
    },
    Delete {
        ids: Vec<PageId>,
    },
    /// Insertion boundary in the original list, before removal of moved pages.
    Move {
        ids: Vec<PageId>,
        target: usize,
    },
    Import(SourceDocument),
}
struct Change {
    before: Vec<(usize, PageEntry)>,
    after: Vec<(usize, PageEntry)>,
    selection_before: HashSet<PageId>,
    selection_after: HashSet<PageId>,
    revision_before: u64,
    revision_after: u64,
    source: Option<SourceDocument>,
}
pub struct Workspace {
    sources: Vec<SourceDocument>,
    pages: Vec<PageEntry>,
    selected: HashSet<PageId>,
    undo: VecDeque<Change>,
    redo: Vec<Change>,
    revision: u64,
    next_revision: u64,
    clean_revision: u64,
}
impl Workspace {
    fn validate_source(source: &SourceDocument) -> Result<(), DomainError> {
        if source.geometries.is_empty()
            || source.geometries.len() > u16::MAX as usize
            || source.geometries.iter().any(|g| {
                !g.width_points.is_finite()
                    || !g.height_points.is_finite()
                    || g.width_points <= 0.
                    || g.height_points <= 0.
                    || g.quarter_turns > 3
            })
        {
            return Err(DomainError::InvalidPages);
        }
        Ok(())
    }
    fn entries(source: &SourceDocument) -> Vec<PageEntry> {
        (0..source.geometries.len())
            .map(|index| PageEntry {
                id: PageId {
                    source: source.id,
                    serial: index as u32,
                },
                source_id: source.id,
                source_page_index: index as u16,
                rotation_delta: 0,
            })
            .collect()
    }
    pub fn new(source: SourceDocument) -> Result<Self, DomainError> {
        Self::validate_source(&source)?;
        let pages = Self::entries(&source);
        Ok(Self {
            sources: vec![source],
            pages,
            selected: HashSet::new(),
            undo: VecDeque::new(),
            redo: Vec::new(),
            revision: 0,
            next_revision: 1,
            clean_revision: 0,
        })
    }
    pub fn source(&self) -> &SourceDocument {
        &self.sources[0]
    }
    pub fn sources(&self) -> &[SourceDocument] {
        &self.sources
    }
    pub fn resolve(&self, id: SourceId) -> Option<&SourceDocument> {
        self.sources.iter().find(|s| s.id == id)
    }
    pub fn pages(&self) -> &[PageEntry] {
        &self.pages
    }
    pub fn selected(&self) -> &HashSet<PageId> {
        &self.selected
    }
    pub fn select(&mut self, ids: impl IntoIterator<Item = PageId>) {
        let valid: HashSet<_> = self.pages.iter().map(|p| p.id).collect();
        self.selected = ids.into_iter().filter(|id| valid.contains(id)).collect();
    }
    pub fn geometry(&self, index: usize) -> Result<PageGeometry, DomainError> {
        let p = self.pages.get(index).ok_or(DomainError::InvalidReference)?;
        let mut g = *self
            .resolve(p.source_id)
            .and_then(|s| s.geometries.get(usize::from(p.source_page_index)))
            .ok_or(DomainError::InvalidReference)?;
        if p.rotation_delta % 2 == 1 {
            std::mem::swap(&mut g.width_points, &mut g.height_points);
        }
        g.quarter_turns = (g.quarter_turns + p.rotation_delta) % 4;
        Ok(g)
    }
    pub fn is_dirty(&self) -> bool {
        self.revision != self.clean_revision
    }
    pub fn mark_exported(&mut self, revision: u64) -> bool {
        if self.revision != revision {
            return false;
        }
        self.clean_revision = revision;
        true
    }
    pub fn revision(&self) -> u64 {
        self.revision
    }
    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }
    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }
    pub fn execute(&mut self, command: Command) -> Result<bool, DomainError> {
        let mut source = None;
        let (ids, target, turns, delete) = match command {
            Command::Import(s) => {
                Self::validate_source(&s)?;
                if self
                    .sources
                    .iter()
                    .any(|v| v.id == s.id || v.path == s.path)
                {
                    return Err(DomainError::DuplicateSource);
                }
                if self
                    .sources
                    .iter()
                    .map(|v| v.geometries.len())
                    .sum::<usize>()
                    + s.geometries.len()
                    > 1024
                    || self
                        .sources
                        .iter()
                        .fold(0u64, |bytes, v| bytes.saturating_add(v.fingerprint.bytes))
                        .saturating_add(s.fingerprint.bytes)
                        > 128 * 1024 * 1024
                {
                    return Err(DomainError::ResourceLimit);
                }
                source = Some(s);
                (Vec::new(), None, 0, false)
            }
            Command::Move { ids, target } => {
                if target > self.pages.len() {
                    return Err(DomainError::InvalidReference);
                }
                (ids, Some(target), 0, false)
            }
            Command::Rotate { ids, turns } => {
                if turns > 3 {
                    return Err(DomainError::InvalidReference);
                }
                (ids, None, turns, false)
            }
            Command::Delete { ids } => (ids, None, 0, true),
        };
        let wanted: HashSet<_> = ids.iter().copied().collect();
        if wanted.len() != ids.len()
            || wanted
                .iter()
                .any(|id| !self.pages.iter().any(|p| p.id == *id))
        {
            return Err(DomainError::InvalidReference);
        }
        let before: Vec<_> = self
            .pages
            .iter()
            .copied()
            .enumerate()
            .filter(|(_, p)| wanted.contains(&p.id))
            .collect();
        let after = if let Some(s) = &source {
            Self::entries(s)
                .into_iter()
                .enumerate()
                .map(|(i, p)| (self.pages.len() + i, p))
                .collect()
        } else if delete {
            Vec::new()
        } else if let Some(t) = target {
            let insertion = t - before.iter().filter(|(i, _)| *i < t).count();
            before
                .iter()
                .enumerate()
                .map(|(offset, (_, p))| (insertion + offset, *p))
                .collect()
        } else {
            before
                .iter()
                .map(|(i, p)| {
                    let mut p = *p;
                    p.rotation_delta = (p.rotation_delta + turns) % 4;
                    (*i, p)
                })
                .collect()
        };
        if before == after {
            return Ok(false);
        }
        let selection_after = if delete {
            self.selected.difference(&wanted).copied().collect()
        } else if source.is_some() || target.is_some() {
            after.iter().map(|(_, p)| p.id).collect()
        } else {
            self.selected.clone()
        };
        let change = Change {
            before,
            after,
            selection_before: self.selected.clone(),
            selection_after,
            revision_before: self.revision,
            revision_after: self.next_revision,
            source,
        };
        self.next_revision += 1;
        self.apply(&change, true);
        self.redo.clear();
        self.undo.push_back(change);
        if self.undo.len() > 128 {
            self.undo.pop_front();
        }
        Ok(true)
    }
    fn apply(&mut self, c: &Change, forward: bool) {
        let (remove, insert) = if forward {
            (&c.before, &c.after)
        } else {
            (&c.after, &c.before)
        };
        let ids: HashSet<_> = remove.iter().map(|(_, p)| p.id).collect();
        self.pages.retain(|p| !ids.contains(&p.id));
        for (i, p) in insert {
            self.pages.insert(*i, *p);
        }
        if let Some(s) = &c.source {
            if forward {
                self.sources.push(s.clone());
            } else {
                self.sources.retain(|v| v.id != s.id);
            }
        }
        self.selected = if forward {
            c.selection_after.clone()
        } else {
            c.selection_before.clone()
        };
        self.revision = if forward {
            c.revision_after
        } else {
            c.revision_before
        };
    }
    pub fn undo(&mut self) -> bool {
        if let Some(c) = self.undo.pop_back() {
            self.apply(&c, false);
            self.redo.push(c);
            true
        } else {
            false
        }
    }
    pub fn redo(&mut self) -> bool {
        if let Some(c) = self.redo.pop() {
            self.apply(&c, true);
            self.undo.push_back(c);
            true
        } else {
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn source(id: u64, geometry: PageGeometry) -> SourceDocument {
        SourceDocument {
            id: SourceId(id),
            path: "synthetic.pdf".into(),
            fingerprint: SourceFingerprint {
                bytes: 50,
                modified_ns: 1,
            },
            geometries: vec![geometry; 100],
            export_support: ExportSupport::NotPreflighted,
        }
    }
    fn small(id: u64) -> SourceDocument {
        let mut s = source(
            id,
            PageGeometry {
                width_points: 600.,
                height_points: 800.,
                quarter_turns: 0,
            },
        );
        s.path = format!("source-{id}.pdf").into();
        s.geometries.truncate(5);
        s
    }
    #[test]
    fn edit_roundtrip_and_branch() -> Result<(), DomainError> {
        let mut w = Workspace::new(small(1))?;
        let original = w.pages.clone();
        let ids = vec![original[1].id, original[3].id];
        w.select(ids.clone());
        w.execute(Command::Move {
            ids: ids.clone(),
            target: 5,
        })?;
        assert_eq!(
            w.pages
                .iter()
                .map(|p| p.source_page_index)
                .collect::<Vec<_>>(),
            vec![0, 2, 4, 1, 3]
        );
        w.execute(Command::Rotate {
            ids: ids.clone(),
            turns: 1,
        })?;
        assert_eq!(w.geometry(3)?.width_points, 800.);
        w.execute(Command::Delete { ids: ids.clone() })?;
        assert!(w.selected.is_empty());
        assert!(w.is_dirty());
        for _ in 0..3 {
            assert!(w.undo());
        }
        assert_eq!(w.pages, original);
        assert_eq!(w.selected.len(), 2);
        assert!(!w.is_dirty());
        for _ in 0..3 {
            assert!(w.redo());
        }
        assert_eq!(w.pages.len(), 3);
        assert!(w.undo());
        w.execute(Command::Rotate { ids, turns: 2 })?;
        assert!(!w.can_redo());
        Ok(())
    }
    #[test]
    fn import_empty_and_atomic_validation() -> Result<(), DomainError> {
        let mut w = Workspace::new(small(1))?;
        assert!(w.execute(Command::Import(small(1))).is_err());
        w.execute(Command::Import(small(2)))?;
        assert_eq!(w.resolve(SourceId(2)).map(|s| s.geometries.len()), Some(5));
        let before = w.pages.clone();
        assert!(
            w.execute(Command::Move {
                ids: vec![before[0].id],
                target: 11
            })
            .is_err()
        );
        assert_eq!(w.pages, before);
        let all = w.pages.iter().map(|p| p.id).collect();
        w.execute(Command::Delete { ids: all })?;
        assert!(w.pages.is_empty());
        assert!(w.undo());
        assert_eq!(w.pages, before);
        assert!(w.undo());
        assert!(w.resolve(SourceId(2)).is_none());
        assert!(w.redo());
        assert_eq!(w.pages, before);
        Ok(())
    }
    #[test]
    fn seeded_sequences_preserve_invariants_and_undo() -> Result<(), DomainError> {
        let mut w = Workspace::new(small(7))?;
        let original = w.pages.clone();
        let mut seed = 42u64;
        let mut count = 0;
        for _ in 0..100 {
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
            let i = (seed as usize) % w.pages.len();
            let id = w.pages[i].id;
            let command = if seed & 1 == 0 {
                Command::Rotate {
                    ids: vec![id],
                    turns: 1,
                }
            } else {
                Command::Move {
                    ids: vec![id],
                    target: (seed as usize / 7) % (w.pages.len() + 1),
                }
            };
            if w.execute(command)? {
                count += 1;
            }
            let unique: HashSet<_> = w.pages.iter().map(|p| p.id).collect();
            assert_eq!(unique.len(), 5);
            for i in 0..5 {
                w.geometry(i)?;
            }
        }
        for _ in 0..count {
            assert!(w.undo());
        }
        assert_eq!(w.pages, original);
        assert!(!w.is_dirty());
        for _ in 0..4 {
            w.execute(Command::Rotate {
                ids: vec![original[0].id],
                turns: 1,
            })?;
        }
        assert_eq!(w.pages, original);
        Ok(())
    }
    #[test]
    fn seeded_mixed_edits_restore_exact_state() -> Result<(), DomainError> {
        let mut w = Workspace::new(small(1))?;
        let mut seed = 8125u64;
        for step in 0..100 {
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
            let before = w.pages.clone();
            let sources = w.sources.len();
            let revision = w.revision;
            let selection = w.selected.clone();
            let command = if w.pages.is_empty() || seed.is_multiple_of(5) {
                Command::Import(small(step + 2))
            } else {
                let id = w.pages[(seed as usize / 11) % w.pages.len()].id;
                match seed % 5 {
                    1 => Command::Delete { ids: vec![id] },
                    2 => Command::Move {
                        ids: vec![id],
                        target: (seed as usize / 7) % (w.pages.len() + 1),
                    },
                    _ => Command::Rotate {
                        ids: vec![id],
                        turns: 1,
                    },
                }
            };
            if w.execute(command)? {
                let after = w.pages.clone();
                assert!(w.undo());
                assert_eq!(w.pages, before);
                assert_eq!(w.sources.len(), sources);
                assert_eq!(w.revision, revision);
                assert_eq!(w.selected, selection);
                assert!(w.redo());
                assert_eq!(w.pages, after);
            }
            for (i, p) in w.pages.iter().enumerate() {
                assert!(w.resolve(p.source_id).is_some());
                assert!(p.rotation_delta < 4);
                w.geometry(i)?;
            }
            assert!(
                w.selected
                    .iter()
                    .all(|id| w.pages.iter().any(|p| p.id == *id))
            );
        }
        Ok(())
    }
    #[test]
    fn noops_limits_and_bounded_history() -> Result<(), DomainError> {
        let mut w = Workspace::new(small(1))?;
        let id = w.pages[0].id;
        assert!(!w.execute(Command::Rotate {
            ids: vec![id],
            turns: 0
        })?);
        assert!(!w.is_dirty());
        for _ in 0..200 {
            w.execute(Command::Rotate {
                ids: vec![id],
                turns: 1,
            })?;
        }
        assert_eq!(w.undo.len(), 128);
        let mut huge = small(2);
        huge.geometries.resize(1024, huge.geometries[0]);
        assert!(w.execute(Command::Import(huge)).is_err());
        assert_eq!(w.pages.len(), 5);
        Ok(())
    }
    #[test]
    fn stable_identity_is_distinct_from_position_and_source() -> Result<(), DomainError> {
        let g = PageGeometry {
            width_points: 600.,
            height_points: 800.,
            quarter_turns: 0,
        };
        let a = Workspace::new(source(10, g))?;
        let b = Workspace::new(source(11, g))?;
        assert_ne!(a.pages()[0].id, b.pages()[0].id);
        assert_ne!(a.pages()[0].id, a.pages()[1].id);
        for _ in 0..100 {
            assert_eq!(a.geometry(99)?, g);
        }
        assert_eq!(a.pages()[99].source_page_index, 99);
        assert!(!a.is_dirty());
        assert!(a.geometry(100).is_err());
        Ok(())
    }
    #[test]
    fn invalid_geometry_and_empty_documents_are_rejected() {
        let g = PageGeometry {
            width_points: f32::NAN,
            height_points: 800.,
            quarter_turns: 0,
        };
        assert!(Workspace::new(source(1, g)).is_err());
        let mut empty = source(2, g);
        empty.geometries.clear();
        assert!(Workspace::new(empty).is_err());
    }
}
