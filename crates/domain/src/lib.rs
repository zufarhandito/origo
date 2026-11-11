//! Pure document identity and invariants. No native engines, UI or filesystem access.
use std::path::PathBuf;

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
}

#[derive(Debug, thiserror::Error)]
pub enum DomainError {
    #[error("Dokumen tidak memiliki halaman yang valid.")]
    InvalidPages,
    #[error("Referensi halaman tidak valid.")]
    InvalidReference,
}

pub struct Workspace {
    source: SourceDocument,
    pages: Vec<PageEntry>,
}
impl Workspace {
    pub fn new(source: SourceDocument) -> Result<Self, DomainError> {
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
        let pages = (0..source.geometries.len())
            .map(|index| PageEntry {
                id: PageId {
                    source: source.id,
                    serial: index as u32,
                },
                source_id: source.id,
                source_page_index: index as u16,
            })
            .collect();
        Ok(Self { source, pages })
    }
    pub fn source(&self) -> &SourceDocument {
        &self.source
    }
    pub fn pages(&self) -> &[PageEntry] {
        &self.pages
    }
    pub fn geometry(&self, index: usize) -> Result<PageGeometry, DomainError> {
        let entry = self.pages.get(index).ok_or(DomainError::InvalidReference)?;
        if entry.source_id != self.source.id {
            return Err(DomainError::InvalidReference);
        }
        self.source
            .geometries
            .get(usize::from(entry.source_page_index))
            .copied()
            .ok_or(DomainError::InvalidReference)
    }
    /// Viewer actions never change document contents.
    pub fn is_dirty(&self) -> bool {
        false
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
