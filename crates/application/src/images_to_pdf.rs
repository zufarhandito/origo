//! Bounded PNG/JPEG-to-PDF plan, page layout math, and writer contract.
use crate::{
    PdfInspector,
    export::{ExportControl, ExportError},
};
use std::path::{Path, PathBuf};

pub const MAX_IMAGES: usize = 200;
pub const MAX_IMAGE_FILE_BYTES: u64 = 32 * 1024 * 1024;
pub const MAX_IMAGE_SIDE: u32 = 8192;
pub const MAX_IMAGE_PIXELS: u64 = 32 * 1024 * 1024;
pub const MAX_DECODE_BYTES: u64 = 128 * 1024 * 1024;
pub const MAX_PDF_OUTPUT_BYTES: u64 = 512 * 1024 * 1024;
pub const DEFAULT_IMAGE_DPI: f32 = 96.0;
pub const MIN_PDF_PAGE_POINTS: f32 = 36.0;
pub const MAX_PDF_PAGE_POINTS: f32 = 14_400.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum ImagePageSize {
    #[default]
    A4,
    Letter,
    FitImage,
}

impl ImagePageSize {
    pub fn label(self) -> &'static str {
        match self {
            Self::A4 => "A4",
            Self::Letter => "Letter",
            Self::FitImage => "Sesuai gambar",
        }
    }

    pub fn portrait_points(self) -> Option<(f32, f32)> {
        match self {
            Self::A4 => Some((595.28, 841.89)),
            Self::Letter => Some((612.0, 792.0)),
            Self::FitImage => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum ImageOrientation {
    #[default]
    Auto,
    Portrait,
    Landscape,
}

impl ImageOrientation {
    pub fn label(self) -> &'static str {
        match self {
            Self::Auto => "Otomatis",
            Self::Portrait => "Potret",
            Self::Landscape => "Lanskap",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum ImageMargin {
    None,
    #[default]
    Small,
    Normal,
}

impl ImageMargin {
    pub fn label(self) -> &'static str {
        match self {
            Self::None => "Tanpa margin",
            Self::Small => "Kecil",
            Self::Normal => "Normal",
        }
    }

    pub fn points(self) -> f32 {
        match self {
            Self::None => 0.0,
            Self::Small => 18.0,
            Self::Normal => 36.0,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct ImagesToPdfOptions {
    pub page_size: ImagePageSize,
    pub orientation: ImageOrientation,
    pub margin: ImageMargin,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ImageDimensions {
    pub width: u32,
    pub height: u32,
    pub dpi: Option<(f32, f32)>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PagePlacement {
    pub page_width: f32,
    pub page_height: f32,
    pub image_x: f32,
    pub image_y: f32,
    pub image_width: f32,
    pub image_height: f32,
}

#[derive(Debug, thiserror::Error)]
pub enum ImagesToPdfError {
    #[error("Pilih minimal satu gambar PNG atau JPEG.")]
    Empty,
    #[error("Maksimal {MAX_IMAGES} gambar per PDF.")]
    TooManyImages,
    #[error("Tujuan penyimpanan harus berupa file .pdf baru di folder yang valid.")]
    Destination,
    #[error("File tujuan sudah ada. Pilih nama file PDF baru agar tidak menimpa file lama.")]
    DestinationExists,
    #[error("Gambar tidak dapat dibaca: {0}")]
    SourceUnreadable(String),
    #[error("Format tidak didukung (hanya PNG atau JPEG): {0}")]
    UnsupportedFormat(String),
    #[error(
        "Gambar melebihi batas (maks. 32 MiB, 8192 piksel per sisi, 32 Mi-piksel, decode 128 MiB): {0}"
    )]
    ImageTooLarge(String),
    #[error("Gambar rusak atau tidak valid: {0}")]
    CorruptImage(String),
    #[error("Ukuran PDF melebihi batas 512 MiB. Kurangi jumlah atau ukuran gambar.")]
    OutputLimit,
    #[error("Konversi Gambar ke PDF dibatalkan. File tujuan tidak dibuat.")]
    Cancelled,
    #[error("Batas waktu konversi tercapai. Coba dengan lebih sedikit gambar.")]
    BudgetExceeded,
    #[error("PDF hasil gagal divalidasi. File tujuan tidak diterbitkan.")]
    Validation,
    #[error("Gagal membaca atau menulis file ke disk.")]
    Io,
}

impl From<ExportError> for ImagesToPdfError {
    fn from(value: ExportError) -> Self {
        match value {
            ExportError::Cancelled => Self::Cancelled,
            ExportError::Timeout => Self::BudgetExceeded,
            ExportError::Destination => Self::Destination,
            ExportError::DestinationExists => Self::DestinationExists,
            ExportError::Validation => Self::Validation,
            ExportError::ResourceLimit => Self::OutputLimit,
            _ => Self::Io,
        }
    }
}

pub fn compute_page_layout(
    dims: ImageDimensions,
    options: ImagesToPdfOptions,
) -> Result<PagePlacement, ImagesToPdfError> {
    if dims.width == 0
        || dims.height == 0
        || dims.width > MAX_IMAGE_SIDE
        || dims.height > MAX_IMAGE_SIDE
        || u64::from(dims.width) * u64::from(dims.height) > MAX_IMAGE_PIXELS
    {
        return Err(ImagesToPdfError::ImageTooLarge(format!(
            "{}x{}",
            dims.width, dims.height
        )));
    }
    let margin = options.margin.points();
    if let Some((base_w, base_h)) = options.page_size.portrait_points() {
        let landscape = match options.orientation {
            ImageOrientation::Auto => dims.width > dims.height,
            ImageOrientation::Portrait => false,
            ImageOrientation::Landscape => true,
        };
        let (page_width, page_height) = if landscape {
            (base_h, base_w)
        } else {
            (base_w, base_h)
        };
        let avail_w = (page_width - 2.0 * margin).max(1.0);
        let avail_h = (page_height - 2.0 * margin).max(1.0);
        let w = dims.width as f32;
        let h = dims.height as f32;
        let scale = (avail_w / w).min(avail_h / h);
        let image_width = (w * scale).max(1.0);
        let image_height = (h * scale).max(1.0);
        let image_x = (page_width - image_width) * 0.5;
        let image_y = (page_height - image_height) * 0.5;
        return Ok(PagePlacement {
            page_width,
            page_height,
            image_x,
            image_y,
            image_width,
            image_height,
        });
    }

    let valid_dpi = |v: f32| v.is_finite() && (24.0..=2400.0).contains(&v);
    let (dpi_x, dpi_y) = match dims.dpi {
        Some((x, y)) if valid_dpi(x) && valid_dpi(y) => (x, y),
        _ => (DEFAULT_IMAGE_DPI, DEFAULT_IMAGE_DPI),
    };
    let mut image_width = dims.width as f32 * 72.0 / dpi_x;
    let mut image_height = dims.height as f32 * 72.0 / dpi_y;
    let max_content = (MAX_PDF_PAGE_POINTS - 2.0 * margin).max(1.0);
    if image_width > max_content || image_height > max_content {
        let scale = (max_content / image_width).min(max_content / image_height);
        image_width *= scale;
        image_height *= scale;
    }
    let page_width = (image_width + 2.0 * margin).clamp(MIN_PDF_PAGE_POINTS, MAX_PDF_PAGE_POINTS);
    let page_height = (image_height + 2.0 * margin).clamp(MIN_PDF_PAGE_POINTS, MAX_PDF_PAGE_POINTS);
    let image_x = (page_width - image_width) * 0.5;
    let image_y = (page_height - image_height) * 0.5;
    Ok(PagePlacement {
        page_width,
        page_height,
        image_x,
        image_y,
        image_width,
        image_height,
    })
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ImagesToPdfPlan {
    images: Vec<PathBuf>,
    options: ImagesToPdfOptions,
    output_limit: u64,
}

impl ImagesToPdfPlan {
    pub fn new(
        images: Vec<PathBuf>,
        options: ImagesToPdfOptions,
    ) -> Result<Self, ImagesToPdfError> {
        if images.is_empty() {
            return Err(ImagesToPdfError::Empty);
        }
        if images.len() > MAX_IMAGES {
            return Err(ImagesToPdfError::TooManyImages);
        }
        Ok(Self {
            images,
            options,
            output_limit: MAX_PDF_OUTPUT_BYTES,
        })
    }

    /// Allows tighter quotas in tests, never a higher production limit.
    pub fn with_output_limit(mut self, bytes: u64) -> Result<Self, ImagesToPdfError> {
        if bytes == 0 || bytes > MAX_PDF_OUTPUT_BYTES {
            return Err(ImagesToPdfError::OutputLimit);
        }
        self.output_limit = bytes;
        Ok(self)
    }

    pub fn images(&self) -> &[PathBuf] {
        &self.images
    }

    pub fn options(&self) -> ImagesToPdfOptions {
        self.options
    }

    pub fn output_limit(&self) -> u64 {
        self.output_limit
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ImagesToPdfReceipt {
    pub destination: PathBuf,
    pub pages: usize,
    pub bytes: u64,
}

pub trait ImagePdfWriter {
    fn write_pdf(
        &self,
        plan: &ImagesToPdfPlan,
        destination: &Path,
        inspector: &impl PdfInspector,
        control: &ExportControl,
        progress: &mut dyn FnMut(usize),
    ) -> Result<ImagesToPdfReceipt, ImagesToPdfError>;
}

pub fn run_images_to_pdf(
    plan: &ImagesToPdfPlan,
    destination: &Path,
    writer: &impl ImagePdfWriter,
    inspector: &impl PdfInspector,
    control: &ExportControl,
    mut progress: impl FnMut(usize),
) -> Result<ImagesToPdfReceipt, ImagesToPdfError> {
    control.checkpoint()?;
    writer.write_pdf(plan, destination, inspector, control, &mut progress)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plan_rejects_empty_and_over_limit() {
        assert!(matches!(
            ImagesToPdfPlan::new(vec![], ImagesToPdfOptions::default()),
            Err(ImagesToPdfError::Empty)
        ));
        let too_many: Vec<PathBuf> = (0..=MAX_IMAGES)
            .map(|i| PathBuf::from(format!("img-{i}.png")))
            .collect();
        assert!(matches!(
            ImagesToPdfPlan::new(too_many, ImagesToPdfOptions::default()),
            Err(ImagesToPdfError::TooManyImages)
        ));
        let ok = ImagesToPdfPlan::new(vec![PathBuf::from("a.png")], ImagesToPdfOptions::default());
        assert!(ok.is_ok());
        if let Ok(plan) = ok {
            assert!(plan.clone().with_output_limit(0).is_err());
            assert!(
                plan.clone()
                    .with_output_limit(MAX_PDF_OUTPUT_BYTES + 1)
                    .is_err()
            );
            assert!(plan.with_output_limit(1024).is_ok());
        }
    }

    #[test]
    fn a4_auto_orientation_and_margins_preserve_aspect_ratio() {
        let opts = ImagesToPdfOptions {
            page_size: ImagePageSize::A4,
            orientation: ImageOrientation::Auto,
            margin: ImageMargin::Small,
        };
        let Ok(portrait) = compute_page_layout(
            ImageDimensions {
                width: 1000,
                height: 2000,
                dpi: None,
            },
            opts,
        ) else {
            panic!("portrait layout failed");
        };
        assert!((portrait.page_width - 595.28).abs() < 0.01);
        assert!((portrait.page_height - 841.89).abs() < 0.01);
        assert!(portrait.image_x >= 18.0 - 0.01);
        assert!(portrait.image_y >= 18.0 - 0.01);
        assert!(((portrait.image_width / portrait.image_height) - 0.5).abs() < 0.001);

        let Ok(landscape) = compute_page_layout(
            ImageDimensions {
                width: 2000,
                height: 1000,
                dpi: None,
            },
            opts,
        ) else {
            panic!("landscape layout failed");
        };
        assert!((landscape.page_width - 841.89).abs() < 0.01);
        assert!((landscape.page_height - 595.28).abs() < 0.01);
        assert!(((landscape.image_width / landscape.image_height) - 2.0).abs() < 0.001);
    }

    #[test]
    fn fit_image_uses_physical_dpi_and_falls_back_to_96_dpi() {
        let opts = ImagesToPdfOptions {
            page_size: ImagePageSize::FitImage,
            orientation: ImageOrientation::Auto,
            margin: ImageMargin::None,
        };
        // 1240x1754 at 150 DPI is ~A4 (595.2 x 841.92 pt)
        let Ok(a4_scan) = compute_page_layout(
            ImageDimensions {
                width: 1240,
                height: 1754,
                dpi: Some((150.0, 150.0)),
            },
            opts,
        ) else {
            panic!("dpi layout failed");
        };
        assert!((a4_scan.page_width - 595.2).abs() < 0.1);
        assert!((a4_scan.page_height - 841.92).abs() < 0.1);
        assert!(a4_scan.image_x.abs() < 0.01);
        assert!(a4_scan.image_y.abs() < 0.01);

        // No DPI -> 96 DPI default: 960x480 -> 720x360 pt
        let Ok(fallback) = compute_page_layout(
            ImageDimensions {
                width: 960,
                height: 480,
                dpi: None,
            },
            opts,
        ) else {
            panic!("fallback layout failed");
        };
        assert!((fallback.page_width - 720.0).abs() < 0.1);
        assert!((fallback.page_height - 360.0).abs() < 0.1);
    }

    #[test]
    fn fit_image_clamps_tiny_and_rejects_oversized() {
        let opts = ImagesToPdfOptions {
            page_size: ImagePageSize::FitImage,
            orientation: ImageOrientation::Auto,
            margin: ImageMargin::None,
        };
        let Ok(tiny) = compute_page_layout(
            ImageDimensions {
                width: 16,
                height: 16,
                dpi: None,
            },
            opts,
        ) else {
            panic!("tiny layout failed");
        };
        assert!((tiny.page_width - MIN_PDF_PAGE_POINTS).abs() < 0.01);
        assert!((tiny.image_width - 12.0).abs() < 0.01);
        assert!((tiny.image_x - 12.0).abs() < 0.01);

        assert!(matches!(
            compute_page_layout(
                ImageDimensions {
                    width: MAX_IMAGE_SIDE + 1,
                    height: 100,
                    dpi: None,
                },
                opts,
            ),
            Err(ImagesToPdfError::ImageTooLarge(_))
        ));
    }
}
