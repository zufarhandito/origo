//! One immutable page plan, exact raster sizes and a private batch publication.
use crate::export::{
    EXPORT_POLICY_VERSION, ExportControl, ExportError, ExportPhase, ExportPlan, ExportPreflight,
    resolve_source,
};
use crate::split::{SplitError, SplitOutput as BatchOutput, SplitTransaction as BatchTransaction};
use crate::{ExactPageRenderer, ExactRenderRequest, PageGeometry, RenderError, RenderedPage};
use std::{
    path::{Path, PathBuf},
    time::Instant,
};

pub const MAX_PNG_BYTES: u64 = 1024 * 1024 * 1024;
pub const MAX_RASTER_PIXELS: u64 = 16 * 1024 * 1024;
pub const MAX_RASTER_SIDE: u32 = 8192;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PngDpi {
    Dpi150,
    Dpi300,
}
impl PngDpi {
    pub fn value(self) -> u16 {
        match self {
            Self::Dpi150 => 150,
            Self::Dpi300 => 300,
        }
    }
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ImageExportFormat {
    #[default]
    Png,
    Jpeg {
        quality: u8,
    },
}
impl ImageExportFormat {
    pub fn extension(self) -> &'static str {
        match self {
            Self::Png => "png",
            Self::Jpeg { .. } => "jpg",
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            Self::Png => "PNG",
            Self::Jpeg { .. } => "JPEG",
        }
    }
    pub fn detail_label(self) -> String {
        match self {
            Self::Png => "PNG".into(),
            Self::Jpeg { quality } => format!("JPEG {}%", quality.clamp(10, 100)),
        }
    }
    pub fn filename(self, index: usize) -> String {
        format!("page-{:04}.{}", index + 1, self.extension())
    }
}
#[derive(Debug, thiserror::Error)]
pub enum PngError {
    #[error(
        "Halaman terlalu besar untuk DPI ini (maksimum 8192 piksel per sisi dan 16 Mi-piksel). Pilih DPI lebih rendah."
    )]
    Dimensions,
    #[error(
        "Total gambar melebihi batas output 1 GiB. Ekspor lebih sedikit halaman atau pilih DPI lebih rendah."
    )]
    OutputLimit,
    #[error("Gambar gagal ditulis atau divalidasi. Folder hasil tidak diterbitkan.")]
    Encoding,
    #[error(transparent)]
    Batch(#[from] SplitError),
    #[error(transparent)]
    Export(#[from] ExportError),
    #[error(transparent)]
    Render(#[from] RenderError),
}
pub fn dimensions(geometry: PageGeometry, dpi: PngDpi) -> Result<(u32, u32), PngError> {
    let side = |points: f32| {
        let pixels = (f64::from(points) * f64::from(dpi.value()) / 72.).round();
        if !points.is_finite() || points <= 0. || pixels < 1. || pixels > f64::from(MAX_RASTER_SIDE)
        {
            Err(PngError::Dimensions)
        } else {
            Ok(pixels as u32)
        }
    };
    let (width, height) = (side(geometry.width_points)?, side(geometry.height_points)?);
    if u64::from(width) * u64::from(height) > MAX_RASTER_PIXELS {
        return Err(PngError::Dimensions);
    }
    Ok((width, height))
}
#[derive(Clone)]
pub struct PngPlan {
    snapshot: ExportPlan,
    dpi: PngDpi,
    format: ImageExportFormat,
    sizes: Vec<(u32, u32)>,
    output_limit: u64,
}
impl PngPlan {
    pub fn new(snapshot: ExportPlan, dpi: PngDpi) -> Result<Self, PngError> {
        let sizes = snapshot
            .pages()
            .iter()
            .map(|p| dimensions(p.geometry, dpi))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self {
            snapshot,
            dpi,
            format: ImageExportFormat::Png,
            sizes,
            output_limit: MAX_PNG_BYTES,
        })
    }
    pub fn with_format(mut self, format: ImageExportFormat) -> Self {
        self.format = match format {
            ImageExportFormat::Png => ImageExportFormat::Png,
            ImageExportFormat::Jpeg { quality } => ImageExportFormat::Jpeg {
                quality: quality.clamp(10, 100),
            },
        };
        self
    }
    /// Allows tighter quotas in integrations/tests, never a higher production limit.
    pub fn with_output_limit(mut self, bytes: u64) -> Result<Self, PngError> {
        if bytes == 0 || bytes > MAX_PNG_BYTES {
            return Err(PngError::OutputLimit);
        }
        self.output_limit = bytes;
        Ok(self)
    }
    pub fn page_count(&self) -> usize {
        self.sizes.len()
    }
    pub fn dpi(&self) -> PngDpi {
        self.dpi
    }
    pub fn format(&self) -> ImageExportFormat {
        self.format
    }
    pub fn snapshot(&self) -> &ExportPlan {
        &self.snapshot
    }
    pub fn filename(index: usize) -> String {
        ImageExportFormat::Png.filename(index)
    }
    pub fn output_filename(&self, index: usize) -> String {
        self.format.filename(index)
    }
}
/// Encoder owns bounded streaming writes and complete decode validation of each file.
pub trait PngWriter {
    fn write_png(
        &self,
        raster: &RenderedPage,
        path: &Path,
        remaining: u64,
        control: &ExportControl,
    ) -> Result<u64, PngError>;
    fn write_image(
        &self,
        raster: &RenderedPage,
        path: &Path,
        format: ImageExportFormat,
        remaining: u64,
        control: &ExportControl,
    ) -> Result<u64, PngError> {
        match format {
            ImageExportFormat::Png => self.write_png(raster, path, remaining, control),
            ImageExportFormat::Jpeg { .. } => Err(PngError::Encoding),
        }
    }
}
pub struct PreparedPng<T> {
    plan: PngPlan,
    transaction: T,
}
pub struct PngReceipt {
    pub instance: u64,
    pub revision: u64,
    pub destination: PathBuf,
    pub pages: usize,
    pub bytes: u64,
    pub dpi: PngDpi,
    pub format: ImageExportFormat,
}
pub fn prepare_png<O: BatchOutput>(
    plan: PngPlan,
    destination: &Path,
    output: &O,
    preflight: &impl ExportPreflight,
    control: &ExportControl,
) -> Result<PreparedPng<O::Transaction>, PngError> {
    control.checkpoint()?;
    control.set_phase(ExportPhase::Snapshot);
    let transaction = output.stage(&plan.snapshot, destination, control)?;
    control.set_phase(ExportPhase::Preflight);
    // Deliberately use normal preflight, never tagged/whole-source Extract exceptions.
    // Image conversion explicitly omits metadata/bookmarks; interactive/complex inputs stay blocked.
    let report = preflight.preflight(transaction.sources(), control)?;
    if report.policy_version != EXPORT_POLICY_VERSION || report.tags_loss {
        return Err(ExportError::Inspection.into());
    }
    transaction.verify_sources()?;
    control.checkpoint()?;
    Ok(PreparedPng { plan, transaction })
}
pub fn finish_png<T: BatchTransaction>(
    prepared: PreparedPng<T>,
    renderer: &impl ExactPageRenderer,
    writer: &impl PngWriter,
    control: &ExportControl,
    mut progress: impl FnMut(usize),
) -> Result<PngReceipt, PngError> {
    let PreparedPng { plan, transaction } = prepared;
    let mut bytes = 0u64;
    for (index, (page, &(width, height))) in
        plan.snapshot.pages().iter().zip(&plan.sizes).enumerate()
    {
        control.checkpoint()?;
        transaction.verify_sources()?;
        control.set_phase(ExportPhase::Writing);
        let source = resolve_source(transaction.sources(), page.entry.source_id)?;
        let started = Instant::now();
        let result = renderer.render_exact(&ExactRenderRequest {
            annotations: Some(page.annotations.clone()),
            path: source.path.clone(),
            page_index: page.entry.source_page_index,
            rotation_delta: page.entry.rotation_delta,
            fingerprint: source.fingerprint,
            width,
            height,
        });
        control.charge(started.elapsed());
        control.remaining()?;
        let raster = result?;
        if raster.width != width
            || raster.height != height
            || raster.rgba.len() != width as usize * height as usize * 4
            || raster.rgba.as_chunks::<4>().0.iter().any(|p| p[3] != 255)
        {
            return Err(ExportError::Validation.into());
        }
        control.checkpoint()?;
        let path = transaction
            .output_directory()
            .join(plan.output_filename(index));
        let started = Instant::now();
        let result = writer.write_image(
            &raster,
            &path,
            plan.format,
            plan.output_limit - bytes,
            control,
        );
        control.charge(started.elapsed());
        control.remaining()?;
        let written = result?;
        bytes = bytes
            .checked_add(written)
            .filter(|b| *b <= plan.output_limit)
            .ok_or(PngError::OutputLimit)?;
        if written == 0 {
            return Err(PngError::Encoding);
        }
        drop(raster);
        progress(index + 1);
    }
    transaction.verify_sources()?;
    control.set_phase(ExportPhase::Commit);
    let destination = transaction.commit(control)?;
    control.set_phase(ExportPhase::Finished);
    Ok(PngReceipt {
        instance: plan.snapshot.instance(),
        revision: plan.snapshot.revision(),
        destination,
        pages: plan.page_count(),
        bytes,
        dpi: plan.dpi,
        format: plan.format,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::export::ExportScope;
    use crate::{ExportSupport, SourceDocument, SourceFingerprint, SourceId, Workspace};
    use pdf_domain::Command;

    #[test]
    fn dpi_dimensions_round_exactly_and_reject_instead_of_clamping() -> Result<(), PngError> {
        let g = PageGeometry {
            width_points: 595.,
            height_points: 842.,
            quarter_turns: 0,
        };
        assert_eq!(dimensions(g, PngDpi::Dpi150)?, (1240, 1754));
        assert_eq!(dimensions(g, PngDpi::Dpi300)?, (2479, 3508));
        for (w, h) in [
            (f32::NAN, 1.),
            (1., f32::INFINITY),
            (0., 1.),
            (-1., 1.),
            (4000., 10.),
            (2000., 2000.),
        ] {
            assert!(
                dimensions(
                    PageGeometry {
                        width_points: w,
                        height_points: h,
                        quarter_turns: 0
                    },
                    PngDpi::Dpi300
                )
                .is_err()
            );
        }
        // This sheet fits 150 DPI but must not silently shrink at 300 DPI.
        let g = PageGeometry {
            width_points: 1500.,
            height_points: 1500.,
            quarter_turns: 0,
        };
        assert!(dimensions(g, PngDpi::Dpi150).is_ok());
        assert!(dimensions(g, PngDpi::Dpi300).is_err());
        Ok(())
    }
    #[test]
    fn selected_snapshot_keeps_order_rotations_and_dirty_revision()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut w = Workspace::new(SourceDocument {
            id: SourceId(1),
            path: "source.pdf".into(),
            fingerprint: SourceFingerprint {
                bytes: 1,
                modified_ns: 1,
            },
            export_support: ExportSupport::NotPreflighted,
            annotations: vec![],
            geometries: vec![
                PageGeometry {
                    width_points: 72.,
                    height_points: 144.,
                    quarter_turns: 0
                };
                3
            ],
        })?;
        assert!(ExportPlan::capture(&w, 9, ExportScope::Selected).is_err());
        let ids: Vec<_> = w.pages().iter().map(|p| p.id).collect();
        w.execute(Command::Move {
            ids: vec![ids[2]],
            target: 0,
        })?;
        w.execute(Command::Rotate {
            ids: vec![ids[2]],
            turns: 1,
        })?;
        w.select([ids[0], ids[2]]);
        let plan = PngPlan::new(
            ExportPlan::capture(&w, 9, ExportScope::Selected)?,
            PngDpi::Dpi150,
        )?;
        assert_eq!(
            plan.snapshot
                .pages()
                .iter()
                .map(|p| p.entry.id)
                .collect::<Vec<_>>(),
            vec![ids[2], ids[0]]
        );
        assert_eq!(plan.sizes, vec![(300, 150), (150, 300)]);
        assert_eq!(plan.snapshot.instance(), 9);
        assert_eq!(plan.snapshot.revision(), w.revision());
        assert!(w.is_dirty());
        w.execute(Command::Delete { ids: vec![ids[2]] })?;
        assert_eq!(plan.page_count(), 2);
        assert_eq!(PngPlan::filename(0), "page-0001.png");
        assert_eq!(PngPlan::filename(1023), "page-1024.png");
        assert!(plan.clone().with_output_limit(0).is_err());
        assert!(plan.with_output_limit(MAX_PNG_BYTES + 1).is_err());
        Ok(())
    }
}
