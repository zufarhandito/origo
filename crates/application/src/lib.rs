//! M0 rendering boundary. No native pointers or GPUI objects cross this port.
pub use pdf_domain::{
    ExportSupport, PageGeometry, SourceDocument, SourceFingerprint, SourceId, Workspace,
};
use std::path::{Path, PathBuf};
pub mod export;
pub mod text;
pub mod viewer;

#[derive(Debug, thiserror::Error)]
pub enum RenderError {
    #[error("PDFium tidak tersedia: {0}. Periksa sidecar yang dipin.")]
    DependencyUnavailable(String),
    #[error("PDF tidak dapat dibuka atau dirender.")]
    InvalidPdf,
    #[error("Dokumen atau bitmap melebihi batas sumber daya yang dikonfigurasi.")]
    ResourceLimit,
    #[error("Halaman tidak ditemukan.")]
    InvalidPage,
    #[error("File sumber berubah. Buka ulang PDF agar pratinjau sesuai dengan sumber terbaru.")]
    SourceChanged,
}

pub struct InspectedDocument {
    pub path: PathBuf,
    pub fingerprint: SourceFingerprint,
    pub pages: Vec<PageGeometry>,
}
pub trait PdfInspector {
    fn inspect(&self, source: &Path) -> Result<InspectedDocument, RenderError>;
}

pub fn open_document(
    inspector: &impl PdfInspector,
    path: &Path,
    id: SourceId,
) -> Result<Workspace, RenderError> {
    let info = inspector.inspect(path)?;
    Workspace::new(SourceDocument {
        id,
        path: info.path,
        fingerprint: info.fingerprint,
        geometries: info.pages,
        export_support: ExportSupport::NotPreflighted,
    })
    .map_err(|_| RenderError::InvalidPdf)
}

#[derive(Clone, Debug)]
pub struct RenderRequest {
    pub path: PathBuf,
    pub page_index: u16,
    pub target_width: u32,
    pub rotation_delta: u8,
    pub fingerprint: Option<SourceFingerprint>,
}

pub struct RenderedPage {
    pub width: u32,
    pub height: u32,
    /// Tightly packed RGBA8, independent of PDFium's native stride/channel order.
    pub rgba: Vec<u8>,
    pub page_count: u16,
    pub clockwise_quarter_turns: u8,
}

pub trait PageRenderer {
    fn render_page(&self, request: &RenderRequest) -> Result<RenderedPage, RenderError>;
    fn render(&self, source: &Path, page: u16) -> Result<RenderedPage, RenderError> {
        self.render_page(&RenderRequest {
            path: source.into(),
            page_index: page,
            target_width: 1200,
            rotation_delta: 0,
            fingerprint: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Fake;
    impl PdfInspector for Fake {
        fn inspect(&self, source: &Path) -> Result<InspectedDocument, RenderError> {
            Ok(InspectedDocument {
                path: source.into(),
                fingerprint: SourceFingerprint {
                    bytes: 123,
                    modified_ns: 42,
                },
                pages: vec![
                    PageGeometry {
                        width_points: 612.,
                        height_points: 792.,
                        quarter_turns: 0
                    };
                    100
                ],
            })
        }
    }
    #[test]
    fn open_uses_metadata_without_rasterizing_and_preserves_mapping() -> Result<(), RenderError> {
        let workspace = open_document(&Fake, Path::new("input.pdf"), SourceId(8))?;
        assert_eq!(workspace.pages().len(), 100);
        assert_eq!(workspace.pages()[45].source_page_index, 45);
        assert_eq!(workspace.source().fingerprint.bytes, 123);
        assert!(!workspace.is_dirty());
        assert_eq!(
            workspace.source().export_support,
            ExportSupport::NotPreflighted
        );
        Ok(())
    }
}
