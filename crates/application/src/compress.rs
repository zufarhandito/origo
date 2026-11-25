//! Smart PDF size-compression planning (quality/DPI presets + optional grayscale).
use crate::export::{ExportControl, ExportError, ExportPlan};
use crate::{ExactPageRenderer, PageGeometry, PdfInspector, RenderError};
use std::path::{Path, PathBuf};

/// Default maximum compressed output size (512 MiB).
pub const MAX_COMPRESS_OUTPUT_BYTES: u64 = 512 * 1024 * 1024;
/// Maximum raster dimension per side during compression (4096 px).
pub const MAX_COMPRESS_SIDE: u32 = 4096;
/// Maximum total pixels per page during compression (12 Mi-pixels).
pub const MAX_COMPRESS_PIXELS: u64 = 12 * 1024 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum CompressPreset {
    /// Sharp print/archive quality (200 DPI, JPEG Q85).
    High,
    /// Balanced quality and size for email & scans (150 DPI, JPEG Q75).
    #[default]
    Balanced,
    /// Minimum size for strict upload limits / WhatsApp (100 DPI, JPEG Q60).
    Compact,
}

impl CompressPreset {
    pub const ALL: [Self; 3] = [Self::High, Self::Balanced, Self::Compact];

    pub fn dpi(self) -> u16 {
        match self {
            Self::High => 200,
            Self::Balanced => 150,
            Self::Compact => 100,
        }
    }

    pub fn jpeg_quality(self) -> u8 {
        match self {
            Self::High => 85,
            Self::Balanced => 75,
            Self::Compact => 60,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::High => "Tinggi (Cetak / Arsip)",
            Self::Balanced => "Seimbang (Email / Scan)",
            Self::Compact => "Ukuran Minimal (Upload)",
        }
    }

    pub fn short_label(self) -> &'static str {
        match self {
            Self::High => "Tinggi",
            Self::Balanced => "Seimbang",
            Self::Compact => "Minimal",
        }
    }

    pub fn description(self) -> &'static str {
        match self {
            Self::High => "200 DPI · Kualitas 85% — teks & foto tetap tajam untuk cetak.",
            Self::Balanced => "150 DPI · Kualitas 75% — ideal untuk dokumen scan & lampiran email.",
            Self::Compact => "100 DPI · Kualitas 60% — ukuran terkecil untuk unggah formulir/chat.",
        }
    }

    pub fn suffix(self) -> &'static str {
        match self {
            Self::High => "kompres-tinggi",
            Self::Balanced => "kompres",
            Self::Compact => "kompres-minimal",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct CompressOptions {
    pub preset: CompressPreset,
    pub grayscale: bool,
}

#[derive(Debug, thiserror::Error)]
pub enum CompressError {
    #[error("Tidak ada halaman untuk dikompres.")]
    Empty,
    #[error("Tujuan harus berupa file PDF baru di folder yang dapat ditulis.")]
    Destination,
    #[error("File tujuan sudah ada. Pilih nama PDF baru; file lama tidak ditimpa.")]
    DestinationExists,
    #[error("Ukuran hasil kompresi melebihi batas output.")]
    OutputLimit,
    #[error("Gagal menyandikan halaman menjadi gambar terkompresi.")]
    Encoding,
    #[error("PDF hasil kompresi gagal divalidasi; file tujuan tidak diterbitkan.")]
    Validation,
    #[error("Kompresi PDF dibatalkan.")]
    Cancelled,
    #[error("Gagal membaca atau menulis berkas kompresi.")]
    Io,
    #[error(transparent)]
    Export(#[from] ExportError),
    #[error(transparent)]
    Render(#[from] RenderError),
}

/// Computes bounded raster dimensions for a page at `dpi`, clamping oversized pages
/// proportionally to fit within [`MAX_COMPRESS_SIDE`] and [`MAX_COMPRESS_PIXELS`]
/// while keeping the original PDF MediaBox intact.
pub fn compress_dimensions(geometry: PageGeometry, dpi: u16) -> Result<(u32, u32), CompressError> {
    if !geometry.width_points.is_finite()
        || !geometry.height_points.is_finite()
        || geometry.width_points <= 0.0
        || geometry.height_points <= 0.0
        || dpi == 0
    {
        return Err(CompressError::Encoding);
    }
    let mut w = f64::from(geometry.width_points) * f64::from(dpi) / 72.0;
    let mut h = f64::from(geometry.height_points) * f64::from(dpi) / 72.0;
    let max_side = f64::from(MAX_COMPRESS_SIDE);
    if w > max_side || h > max_side {
        let scale = (max_side / w.max(h)).min(1.0);
        w *= scale;
        h *= scale;
    }
    let pixels = w * h;
    let max_pixels = MAX_COMPRESS_PIXELS as f64;
    if pixels > max_pixels {
        let scale = (max_pixels / pixels).sqrt();
        w *= scale;
        h *= scale;
    }
    let width = (w.round() as u32).clamp(1, MAX_COMPRESS_SIDE);
    let height = (h.round() as u32).clamp(1, MAX_COMPRESS_SIDE);
    Ok((width, height))
}

#[derive(Clone)]
pub struct CompressPlan {
    snapshot: ExportPlan,
    options: CompressOptions,
    sizes: Vec<(u32, u32)>,
    baseline_bytes: u64,
    output_limit: u64,
}

impl CompressPlan {
    pub fn new(snapshot: ExportPlan, options: CompressOptions) -> Result<Self, CompressError> {
        if snapshot.pages().is_empty() {
            return Err(CompressError::Empty);
        }
        let dpi = options.preset.dpi();
        let sizes = snapshot
            .pages()
            .iter()
            .map(|p| compress_dimensions(p.geometry, dpi))
            .collect::<Result<Vec<_>, _>>()?;
        let baseline_bytes = snapshot
            .sources()
            .iter()
            .map(|s| s.fingerprint.bytes)
            .fold(0u64, u64::saturating_add);
        Ok(Self {
            snapshot,
            options,
            sizes,
            baseline_bytes,
            output_limit: MAX_COMPRESS_OUTPUT_BYTES,
        })
    }

    /// Allows tighter quotas in tests, never a higher production limit.
    pub fn with_output_limit(mut self, bytes: u64) -> Result<Self, CompressError> {
        if bytes == 0 || bytes > MAX_COMPRESS_OUTPUT_BYTES {
            return Err(CompressError::OutputLimit);
        }
        self.output_limit = bytes;
        Ok(self)
    }

    pub fn snapshot(&self) -> &ExportPlan {
        &self.snapshot
    }

    pub fn options(&self) -> CompressOptions {
        self.options
    }

    pub fn sizes(&self) -> &[(u32, u32)] {
        &self.sizes
    }

    pub fn page_count(&self) -> usize {
        self.sizes.len()
    }

    pub fn baseline_bytes(&self) -> u64 {
        self.baseline_bytes
    }

    pub fn output_limit(&self) -> u64 {
        self.output_limit
    }
}

#[derive(Clone, Debug)]
pub struct CompressReceipt {
    pub instance: u64,
    pub revision: u64,
    pub destination: PathBuf,
    pub pages: usize,
    pub baseline_bytes: u64,
    pub output_bytes: u64,
    pub preset: CompressPreset,
    pub grayscale: bool,
}

impl CompressReceipt {
    /// Positive when output is smaller than baseline, negative when larger.
    pub fn saved_percent(&self) -> f64 {
        if self.baseline_bytes == 0 {
            return 0.0;
        }
        let diff = self.baseline_bytes as f64 - self.output_bytes as f64;
        (diff / self.baseline_bytes as f64) * 100.0
    }

    pub fn summary_message(&self) -> String {
        let name = self
            .destination
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| self.destination.display().to_string());
        let mode = if self.grayscale {
            format!("{}, {} DPI, Grayscale", self.preset.short_label(), self.preset.dpi())
        } else {
            format!("{}, {} DPI", self.preset.short_label(), self.preset.dpi())
        };
        let before = format_bytes(self.baseline_bytes);
        let after = format_bytes(self.output_bytes);
        let pct = self.saved_percent();
        if self.output_bytes <= self.baseline_bytes {
            format!(
                "PDF berhasil dikompres ({mode}): {name} — {before} → {after} (-{pct:.1}%)."
            )
        } else {
            let growth = -pct;
            format!(
                "PDF disimpan ({mode}): {name} — {before} → {after} (+{growth:.1}%; file asal sudah sangat ringan — gunakan Optimize PDF lossless untuk menjaga teks asli)."
            )
        }
    }
}

pub fn format_bytes(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{bytes} B")
    } else if bytes < 1024 * 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else {
        format!("{:.2} MB", bytes as f64 / (1024.0 * 1024.0))
    }
}

pub trait PdfCompressor {
    fn compress_pdf(
        &self,
        plan: &CompressPlan,
        destination: &Path,
        renderer: &impl ExactPageRenderer,
        inspector: &impl PdfInspector,
        control: &ExportControl,
        progress: &mut dyn FnMut(usize),
    ) -> Result<CompressReceipt, CompressError>;
}

pub fn run_compress_pdf(
    plan: &CompressPlan,
    destination: &Path,
    renderer: &impl ExactPageRenderer,
    inspector: &impl PdfInspector,
    compressor: &impl PdfCompressor,
    control: &ExportControl,
    mut progress: impl FnMut(usize),
) -> Result<CompressReceipt, CompressError> {
    compressor.compress_pdf(plan, destination, renderer, inspector, control, &mut progress)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dimensions_match_dpi_and_clamp_oversized_posters_proportionally() -> Result<(), CompressError>
    {
        // Standard A4 at 150 DPI
        let a4 = PageGeometry {
            width_points: 595.0,
            height_points: 842.0,
            quarter_turns: 0,
        };
        assert_eq!(compress_dimensions(a4, 150)?, (1240, 1754));
        assert_eq!(compress_dimensions(a4, 100)?, (826, 1169));
        assert_eq!(compress_dimensions(a4, 200)?, (1653, 2339));

        // Oversized architectural poster (100x50 inches at 200 DPI = 20000x10000 px)
        let poster = PageGeometry {
            width_points: 7200.0,
            height_points: 3600.0,
            quarter_turns: 0,
        };
        let (w, h) = compress_dimensions(poster, 200)?;
        assert!(w <= MAX_COMPRESS_SIDE && h <= MAX_COMPRESS_SIDE);
        assert!(u64::from(w) * u64::from(h) <= MAX_COMPRESS_PIXELS);
        assert!(((w as f64 / h as f64) - 2.0).abs() < 0.02);
        Ok(())
    }

    #[test]
    fn receipt_reports_savings_and_growth_honestly() {
        let smaller = CompressReceipt {
            instance: 1,
            revision: 1,
            destination: PathBuf::from("D:/out/scan-kompres.pdf"),
            pages: 5,
            baseline_bytes: 2_000_000,
            output_bytes: 400_000,
            preset: CompressPreset::Balanced,
            grayscale: false,
        };
        assert!((smaller.saved_percent() - 80.0).abs() < 0.01);
        let msg = smaller.summary_message();
        assert!(msg.contains("-80.0%"));
        assert!(msg.contains("Seimbang, 150 DPI"));

        let bigger = CompressReceipt {
            instance: 1,
            revision: 1,
            destination: PathBuf::from("D:/out/tiny-kompres.pdf"),
            pages: 1,
            baseline_bytes: 10_000,
            output_bytes: 25_000,
            preset: CompressPreset::Compact,
            grayscale: true,
        };
        assert!(bigger.saved_percent() < 0.0);
        let msg = bigger.summary_message();
        assert!(msg.contains("Grayscale"));
        assert!(msg.contains("Optimize PDF lossless"));
    }
}
