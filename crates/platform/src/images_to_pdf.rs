//! Streaming bounded PNG/JPEG-to-PDF writer with EXIF, DPI, PDFium validation, and no-clobber commit.
use super::*;
use flate2::{Compression, write::ZlibEncoder};
use image::{
    ColorType, DynamicImage, ImageDecoder, ImageFormat, ImageReader, metadata::Orientation,
};
use pdf_application::{
    PdfInspector,
    export::{ExportControl, ExportPhase},
    images_to_pdf::{
        ImageDimensions, ImagePdfWriter, ImagesToPdfError, ImagesToPdfPlan, ImagesToPdfReceipt,
        MAX_DECODE_BYTES, MAX_IMAGE_FILE_BYTES, MAX_IMAGE_PIXELS, MAX_IMAGE_SIDE,
        compute_page_layout,
    },
};
use std::{
    io::{BufWriter, Cursor, Read, Write},
    time::Instant,
};

#[derive(Clone, Copy, Default)]
pub struct LocalImagePdfWriter;

struct LimitedPdfWriter<'a> {
    writer: BufWriter<File>,
    control: &'a ExportControl,
    remaining: u64,
    written: u64,
    exceeded: bool,
}

impl<'a> LimitedPdfWriter<'a> {
    fn new(file: File, control: &'a ExportControl, limit: u64) -> Self {
        Self {
            writer: BufWriter::with_capacity(64 * 1024, file),
            control,
            remaining: limit,
            written: 0,
            exceeded: false,
        }
    }

    fn write_bytes(&mut self, bytes: &[u8]) -> Result<(), ImagesToPdfError> {
        self.control.checkpoint()?;
        let len = bytes.len() as u64;
        if len > self.remaining {
            self.exceeded = true;
            return Err(ImagesToPdfError::OutputLimit);
        }
        self.writer
            .write_all(bytes)
            .map_err(|_| ImagesToPdfError::Io)?;
        self.remaining -= len;
        self.written += len;
        Ok(())
    }

    fn offset(&self) -> u64 {
        self.written
    }

    fn finish(mut self) -> Result<u64, ImagesToPdfError> {
        self.writer.flush().map_err(|_| ImagesToPdfError::Io)?;
        let file = self.writer.into_inner().map_err(|_| ImagesToPdfError::Io)?;
        file.sync_all().map_err(|_| ImagesToPdfError::Io)?;
        Ok(self.written)
    }
}

enum EncodedStream {
    FlateRgb(Vec<u8>),
    FlateGray(Vec<u8>),
    DctRgb(Vec<u8>),
    DctGray(Vec<u8>),
}

impl EncodedStream {
    fn color_space(&self) -> &'static str {
        match self {
            Self::FlateRgb(_) | Self::DctRgb(_) => "/DeviceRGB",
            Self::FlateGray(_) | Self::DctGray(_) => "/DeviceGray",
        }
    }

    fn filter(&self) -> &'static str {
        match self {
            Self::FlateRgb(_) | Self::FlateGray(_) => "/FlateDecode",
            Self::DctRgb(_) | Self::DctGray(_) => "/DCTDecode",
        }
    }

    fn bytes(&self) -> &[u8] {
        match self {
            Self::FlateRgb(b) | Self::FlateGray(b) | Self::DctRgb(b) | Self::DctGray(b) => b,
        }
    }
}

struct PreparedImage {
    width: u32,
    height: u32,
    dpi: Option<(f32, f32)>,
    stream: EncodedStream,
}

fn display_name(path: &Path) -> String {
    path.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.display().to_string())
}

fn read_bounded_file(path: &Path) -> Result<Vec<u8>, ImagesToPdfError> {
    let name = display_name(path);
    let metadata =
        fs::metadata(path).map_err(|_| ImagesToPdfError::SourceUnreadable(name.clone()))?;
    if !metadata.is_file() {
        return Err(ImagesToPdfError::SourceUnreadable(name));
    }
    if metadata.len() > MAX_IMAGE_FILE_BYTES {
        return Err(ImagesToPdfError::ImageTooLarge(name));
    }
    let file = File::open(path).map_err(|_| ImagesToPdfError::SourceUnreadable(name.clone()))?;
    let mut raw = Vec::with_capacity(metadata.len().min(MAX_IMAGE_FILE_BYTES) as usize);
    file.take(MAX_IMAGE_FILE_BYTES + 1)
        .read_to_end(&mut raw)
        .map_err(|_| ImagesToPdfError::SourceUnreadable(name.clone()))?;
    if raw.len() as u64 > MAX_IMAGE_FILE_BYTES {
        return Err(ImagesToPdfError::ImageTooLarge(name));
    }
    Ok(raw)
}

/// Parses physical DPI from PNG `pHYs` chunk (meters) or JPEG `APP0 JFIF` segment.
fn parse_image_dpi(raw: &[u8], format: ImageFormat) -> Option<(f32, f32)> {
    match format {
        ImageFormat::Png => parse_png_phys_dpi(raw),
        ImageFormat::Jpeg => parse_jpeg_jfif_dpi(raw),
        _ => None,
    }
}

fn parse_png_phys_dpi(raw: &[u8]) -> Option<(f32, f32)> {
    if raw.len() < 8 || &raw[..8] != b"\x89PNG\r\n\x1a\n" {
        return None;
    }
    let mut pos = 8usize;
    while pos.saturating_add(12) <= raw.len() {
        let len = u32::from_be_bytes([raw[pos], raw[pos + 1], raw[pos + 2], raw[pos + 3]]) as usize;
        let kind = &raw[pos + 4..pos + 8];
        let data_start = pos + 8;
        let data_end = data_start.checked_add(len)?;
        let next_pos = data_end.checked_add(4)?;
        if next_pos > raw.len() {
            return None;
        }
        if kind == b"pHYs" && len == 9 {
            let ppu_x = u32::from_be_bytes([
                raw[data_start],
                raw[data_start + 1],
                raw[data_start + 2],
                raw[data_start + 3],
            ]);
            let ppu_y = u32::from_be_bytes([
                raw[data_start + 4],
                raw[data_start + 5],
                raw[data_start + 6],
                raw[data_start + 7],
            ]);
            let unit = raw[data_start + 8];
            if unit == 1 && ppu_x > 0 && ppu_y > 0 {
                return Some((ppu_x as f32 * 0.0254, ppu_y as f32 * 0.0254));
            }
            return None;
        }
        if kind == b"IDAT" || kind == b"IEND" {
            break;
        }
        pos = next_pos;
    }
    None
}

fn parse_jpeg_jfif_dpi(raw: &[u8]) -> Option<(f32, f32)> {
    if raw.len() < 4 || raw[0] != 0xFF || raw[1] != 0xD8 {
        return None;
    }
    let mut pos = 2usize;
    while pos + 4 <= raw.len() {
        if raw[pos] != 0xFF {
            return None;
        }
        while pos < raw.len() && raw[pos] == 0xFF {
            pos += 1;
        }
        if pos >= raw.len() {
            return None;
        }
        let marker = raw[pos];
        pos += 1;
        // Start of Scan or End of Image
        if marker == 0xDA || marker == 0xD9 {
            break;
        }
        // Standalone markers without length
        if marker == 0x01 || (0xD0..=0xD8).contains(&marker) {
            continue;
        }
        if pos + 2 > raw.len() {
            return None;
        }
        let seg_len = u16::from_be_bytes([raw[pos], raw[pos + 1]]) as usize;
        if seg_len < 2 || pos + seg_len > raw.len() {
            return None;
        }
        let payload = &raw[pos + 2..pos + seg_len];
        if marker == 0xE0 && payload.len() >= 12 && &payload[..5] == b"JFIF\0" {
            let units = payload[7];
            let x = u16::from_be_bytes([payload[8], payload[9]]);
            let y = u16::from_be_bytes([payload[10], payload[11]]);
            if x > 0 && y > 0 {
                return match units {
                    1 => Some((f32::from(x), f32::from(y))),
                    2 => Some((f32::from(x) * 2.54, f32::from(y) * 2.54)),
                    _ => None,
                };
            }
        }
        pos += seg_len;
    }
    None
}

/// Returns `Some(components)` (1 for Gray, 3 for RGB) when raw bytes are a clean 8-bit
/// baseline (`SOF0`) JPEG safe for direct `/DCTDecode` passthrough without recompression.
fn baseline_jpeg_passthrough_components(raw: &[u8]) -> Option<u8> {
    if raw.len() < 4 || raw[0] != 0xFF || raw[1] != 0xD8 {
        return None;
    }
    let mut pos = 2usize;
    let mut sof0_components: Option<u8> = None;
    let mut adobe_transform: Option<u8> = None;
    while pos + 2 <= raw.len() {
        if raw[pos] != 0xFF {
            return None;
        }
        while pos < raw.len() && raw[pos] == 0xFF {
            pos += 1;
        }
        if pos >= raw.len() {
            return None;
        }
        let marker = raw[pos];
        pos += 1;
        if marker == 0xDA || marker == 0xD9 {
            break;
        }
        if marker == 0x01 || (0xD0..=0xD8).contains(&marker) {
            continue;
        }
        if pos + 2 > raw.len() {
            return None;
        }
        let seg_len = u16::from_be_bytes([raw[pos], raw[pos + 1]]) as usize;
        if seg_len < 2 || pos + seg_len > raw.len() {
            return None;
        }
        let payload = &raw[pos + 2..pos + seg_len];
        match marker {
            // Baseline DCT (SOF0)
            0xC0 => {
                if payload.len() < 6 {
                    return None;
                }
                let precision = payload[0];
                let components = payload[5];
                if precision != 8 || !matches!(components, 1 | 3) {
                    return None;
                }
                sof0_components = Some(components);
            }
            // Any other SOF (progressive, lossless, arithmetic, etc.) -> re-encode
            0xC1..=0xC3 | 0xC5..=0xC7 | 0xC9..=0xCB | 0xCD..=0xCF => return None,
            // APP14 Adobe color transform: reject non-standard transforms (e.g. CMYK/YCCK/RGB-without-YCbCr)
            0xEE if payload.len() >= 12 && &payload[..5] == b"Adobe" => {
                adobe_transform = Some(payload[11]);
            }
            _ => {}
        }
        pos += seg_len;
    }
    // Reject Adobe RGB (transform=0 with 3 components) or YCCK (transform=2) to avoid color shift
    if let Some(transform) = adobe_transform
        && transform != 1
    {
        return None;
    }
    sof0_components
}

fn swaps_axes(orientation: Orientation) -> bool {
    matches!(
        orientation,
        Orientation::Rotate90
            | Orientation::Rotate270
            | Orientation::Rotate90FlipH
            | Orientation::Rotate270FlipH
    )
}

fn composite_rgba_on_white(image: &DynamicImage) -> Vec<u8> {
    let rgba = image.to_rgba8();
    let mut rgb = Vec::with_capacity(rgba.width() as usize * rgba.height() as usize * 3);
    for p in rgba.pixels() {
        let a = u16::from(p[3]);
        let inv = 255 - a;
        let blend = |c: u8| ((u16::from(c) * a + 255 * inv + 127) / 255) as u8;
        rgb.push(blend(p[0]));
        rgb.push(blend(p[1]));
        rgb.push(blend(p[2]));
    }
    rgb
}

fn flate_compress(data: &[u8]) -> Result<Vec<u8>, ImagesToPdfError> {
    let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(data).map_err(|_| ImagesToPdfError::Io)?;
    encoder.finish().map_err(|_| ImagesToPdfError::Io)
}

fn prepare_image(path: &Path) -> Result<PreparedImage, ImagesToPdfError> {
    let name = display_name(path);
    let raw = read_bounded_file(path)?;
    let mut reader = ImageReader::new(Cursor::new(&raw))
        .with_guessed_format()
        .map_err(|_| ImagesToPdfError::CorruptImage(name.clone()))?;
    let format = match reader.format() {
        Some(f @ (ImageFormat::Png | ImageFormat::Jpeg)) => f,
        _ => return Err(ImagesToPdfError::UnsupportedFormat(name)),
    };

    let mut limits = image::Limits::default();
    limits.max_image_width = Some(MAX_IMAGE_SIDE);
    limits.max_image_height = Some(MAX_IMAGE_SIDE);
    limits.max_alloc = Some(MAX_DECODE_BYTES);
    reader.limits(limits);

    let mut decoder = reader
        .into_decoder()
        .map_err(|_| ImagesToPdfError::CorruptImage(name.clone()))?;
    let (raw_w, raw_h) = decoder.dimensions();
    if raw_w == 0
        || raw_h == 0
        || raw_w > MAX_IMAGE_SIDE
        || raw_h > MAX_IMAGE_SIDE
        || u64::from(raw_w) * u64::from(raw_h) > MAX_IMAGE_PIXELS
        || decoder.total_bytes() > MAX_DECODE_BYTES
    {
        return Err(ImagesToPdfError::ImageTooLarge(name));
    }

    let orientation = decoder.orientation().unwrap_or(Orientation::NoTransforms);
    let mut image = DynamicImage::from_decoder(decoder)
        .map_err(|_| ImagesToPdfError::CorruptImage(name.clone()))?;
    image.apply_orientation(orientation);

    let width = image.width();
    let height = image.height();
    let mut dpi = parse_image_dpi(&raw, format);
    if swaps_axes(orientation) {
        dpi = dpi.map(|(x, y)| (y, x));
    }

    let stream = match format {
        ImageFormat::Jpeg => {
            let passthrough = if orientation == Orientation::NoTransforms {
                baseline_jpeg_passthrough_components(&raw)
            } else {
                None
            };
            match (passthrough, image.color()) {
                (Some(3), ColorType::Rgb8) => EncodedStream::DctRgb(raw),
                (Some(1), ColorType::L8) => EncodedStream::DctGray(raw),
                _ => {
                    let mut encoded = Vec::new();
                    let encoder =
                        image::codecs::jpeg::JpegEncoder::new_with_quality(&mut encoded, 92);
                    if image.color() == ColorType::L8 {
                        let gray = image.to_luma8();
                        gray.write_with_encoder(encoder)
                            .map_err(|_| ImagesToPdfError::Io)?;
                        EncodedStream::DctGray(encoded)
                    } else {
                        let rgb = image.to_rgb8();
                        rgb.write_with_encoder(encoder)
                            .map_err(|_| ImagesToPdfError::Io)?;
                        EncodedStream::DctRgb(encoded)
                    }
                }
            }
        }
        ImageFormat::Png => {
            if image.color() == ColorType::L8 {
                let gray = image.to_luma8();
                EncodedStream::FlateGray(flate_compress(gray.as_raw())?)
            } else if image.color().has_alpha() {
                let rgb = composite_rgba_on_white(&image);
                EncodedStream::FlateRgb(flate_compress(&rgb)?)
            } else {
                let rgb = image.to_rgb8();
                EncodedStream::FlateRgb(flate_compress(rgb.as_raw())?)
            }
        }
        _ => return Err(ImagesToPdfError::UnsupportedFormat(name)),
    };

    Ok(PreparedImage {
        width,
        height,
        dpi,
        stream,
    })
}

fn validate_destination(destination: &Path) -> Result<(PathBuf, PathBuf), ImagesToPdfError> {
    if !destination
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("pdf"))
    {
        return Err(ImagesToPdfError::Destination);
    }
    let name = destination
        .file_name()
        .ok_or(ImagesToPdfError::Destination)?;
    let parent = dunce::canonicalize(destination.parent().ok_or(ImagesToPdfError::Destination)?)
        .map_err(|_| ImagesToPdfError::Destination)?;
    if !parent.is_dir() {
        return Err(ImagesToPdfError::Destination);
    }
    let full = parent.join(name);
    match fs::symlink_metadata(&full) {
        Ok(_) => return Err(ImagesToPdfError::DestinationExists),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(_) => return Err(ImagesToPdfError::Destination),
    }
    Ok((parent, full))
}

impl ImagePdfWriter for LocalImagePdfWriter {
    fn write_pdf(
        &self,
        plan: &ImagesToPdfPlan,
        destination: &Path,
        inspector: &impl PdfInspector,
        control: &ExportControl,
        progress: &mut dyn FnMut(usize),
    ) -> Result<ImagesToPdfReceipt, ImagesToPdfError> {
        control.checkpoint()?;
        control.set_phase(ExportPhase::Snapshot);
        let (parent, canonical_destination) = validate_destination(destination)?;
        let dir = tempfile::Builder::new()
            .prefix(".pdf-studio-img2pdf-")
            .tempdir_in(&parent)
            .map_err(|_| ImagesToPdfError::Io)?;
        let temp_file = tempfile::Builder::new()
            .prefix("result-")
            .suffix(".pdf")
            .tempfile_in(dir.path())
            .map_err(|_| ImagesToPdfError::Io)?;
        let (file, temp_path) = temp_file.into_parts();

        let count = plan.images().len();
        let mut offsets: Vec<u64> = Vec::with_capacity(2 + count * 3);
        let mut placements = Vec::with_capacity(count);
        let mut writer = LimitedPdfWriter::new(file, control, plan.output_limit());

        control.set_phase(ExportPhase::Writing);
        writer.write_bytes(b"%PDF-1.4\n%\xE2\xE3\xCF\xD3\n")?;

        // Object 1: Catalog
        offsets.push(writer.offset());
        writer.write_bytes(b"1 0 obj\n<< /Type /Catalog /Pages 2 0 R >>\nendobj\n")?;

        // Object 2: Pages
        let kids = (0..count)
            .map(|i| format!("{} 0 R", 5 + i * 3))
            .collect::<Vec<_>>()
            .join(" ");
        offsets.push(writer.offset());
        writer.write_bytes(
            format!("2 0 obj\n<< /Type /Pages /Kids [{kids}] /Count {count} >>\nendobj\n")
                .as_bytes(),
        )?;

        for (index, image_path) in plan.images().iter().enumerate() {
            control.checkpoint()?;
            let started = Instant::now();
            let prepared = prepare_image(image_path)?;
            let placement = compute_page_layout(
                ImageDimensions {
                    width: prepared.width,
                    height: prepared.height,
                    dpi: prepared.dpi,
                },
                plan.options(),
            )?;

            let img_id = 3 + index * 3;
            let content_id = 4 + index * 3;
            let page_id = 5 + index * 3;

            // Image XObject
            offsets.push(writer.offset());
            let stream_bytes = prepared.stream.bytes();
            let img_header = format!(
                "{img_id} 0 obj\n<< /Type /XObject /Subtype /Image /Width {} /Height {} /ColorSpace {} /BitsPerComponent 8 /Filter {} /Interpolate true /Length {} >>\nstream\n",
                prepared.width,
                prepared.height,
                prepared.stream.color_space(),
                prepared.stream.filter(),
                stream_bytes.len()
            );
            writer.write_bytes(img_header.as_bytes())?;
            writer.write_bytes(stream_bytes)?;
            writer.write_bytes(b"\nendstream\nendobj\n")?;

            // Content stream
            let content = format!(
                "q\n{:.3} 0 0 {:.3} {:.3} {:.3} cm\n/Im0 Do\nQ\n",
                placement.image_width, placement.image_height, placement.image_x, placement.image_y
            );
            offsets.push(writer.offset());
            writer.write_bytes(
                format!(
                    "{content_id} 0 obj\n<< /Length {} >>\nstream\n{content}endstream\nendobj\n",
                    content.len()
                )
                .as_bytes(),
            )?;

            // Page object
            offsets.push(writer.offset());
            writer.write_bytes(
                format!(
                    "{page_id} 0 obj\n<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {:.3} {:.3}] /Resources << /XObject << /Im0 {img_id} 0 R >> >> /Contents {content_id} 0 R >>\nendobj\n",
                    placement.page_width, placement.page_height
                )
                .as_bytes(),
            )?;

            placements.push(placement);
            control.charge(started.elapsed());
            control.remaining()?;
            progress(index + 1);
        }

        // Cross-reference table & trailer (each xref entry must be exactly 20 bytes)
        let xref_offset = writer.offset();
        let total_entries = offsets.len() + 1;
        writer.write_bytes(format!("xref\n0 {total_entries}\n0000000000 65535 f \n").as_bytes())?;
        for offset in &offsets {
            writer.write_bytes(format!("{offset:010} 00000 n \n").as_bytes())?;
        }
        writer.write_bytes(
            format!(
                "trailer\n<< /Size {total_entries} /Root 1 0 R >>\nstartxref\n{xref_offset}\n%%EOF\n"
            )
            .as_bytes(),
        )?;
        let bytes = writer.finish()?;
        if bytes == 0 {
            return Err(ImagesToPdfError::Validation);
        }

        control.checkpoint()?;
        control.set_phase(ExportPhase::Validation);
        let started = Instant::now();
        let inspected = inspector
            .inspect_output(temp_path.as_ref())
            .map_err(|_| ImagesToPdfError::Validation)?;
        control.charge(started.elapsed());
        control.remaining()?;
        if inspected.pages.len() != placements.len() {
            return Err(ImagesToPdfError::Validation);
        }
        for (actual, expected) in inspected.pages.iter().zip(&placements) {
            if (actual.width_points - expected.page_width).abs() > 1.0
                || (actual.height_points - expected.page_height).abs() > 1.0
            {
                return Err(ImagesToPdfError::Validation);
            }
        }

        control.checkpoint()?;
        control.set_phase(ExportPhase::Commit);
        temp_path
            .persist_noclobber(&canonical_destination)
            .map_err(|e| {
                if e.error.kind() == std::io::ErrorKind::AlreadyExists {
                    ImagesToPdfError::DestinationExists
                } else {
                    ImagesToPdfError::Io
                }
            })?;

        Ok(ImagesToPdfReceipt {
            destination: canonical_destination,
            pages: count,
            bytes,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pdf_application::{
        InspectedDocument, PageGeometry, RenderError, SourceFingerprint,
        images_to_pdf::{
            ImageMargin, ImageOrientation, ImagePageSize, ImagesToPdfOptions, run_images_to_pdf,
        },
    };

    struct StubInspector {
        pages: Vec<(f32, f32)>,
    }

    impl PdfInspector for StubInspector {
        fn inspect(&self, source: &Path) -> Result<InspectedDocument, RenderError> {
            let bytes = fs::read(source).map_err(|_| RenderError::InvalidPdf)?;
            if !bytes.starts_with(b"%PDF-1.4\n") || !bytes.ends_with(b"%%EOF\n") {
                return Err(RenderError::InvalidPdf);
            }
            Ok(InspectedDocument {
                annotations: vec![vec![]; self.pages.len()],
                path: source.to_path_buf(),
                fingerprint: SourceFingerprint {
                    bytes: bytes.len() as u64,
                    modified_ns: 0,
                },
                pages: self
                    .pages
                    .iter()
                    .map(|&(w, h)| PageGeometry {
                        width_points: w,
                        height_points: h,
                        quarter_turns: 0,
                    })
                    .collect(),
            })
        }
    }

    fn write_test_png(path: &Path, width: u32, height: u32, rgba: [u8; 4]) {
        let mut img = image::RgbaImage::new(width, height);
        for p in img.pixels_mut() {
            *p = image::Rgba(rgba);
        }
        let saved = img.save_with_format(path, ImageFormat::Png);
        assert!(saved.is_ok());
    }

    fn write_test_jpeg(path: &Path, width: u32, height: u32, rgb: [u8; 3]) {
        let mut img = image::RgbImage::new(width, height);
        for p in img.pixels_mut() {
            *p = image::Rgb(rgb);
        }
        let saved = img.save_with_format(path, ImageFormat::Jpeg);
        assert!(saved.is_ok());
    }

    #[test]
    fn jpeg_passthrough_and_png_alpha_composite_to_valid_pdf() {
        let Ok(temp) = tempfile::tempdir() else {
            panic!("tempdir failed");
        };
        let jpg_path = temp.path().join("photo.jpg");
        let png_path = temp.path().join("alpha.png");
        write_test_jpeg(&jpg_path, 320, 200, [220, 40, 40]);
        write_test_png(&png_path, 160, 240, [0, 0, 255, 128]);
        let jpg_before = fs::read(&jpg_path).ok();
        let png_before = fs::read(&png_path).ok();

        let opts = ImagesToPdfOptions {
            page_size: ImagePageSize::A4,
            orientation: ImageOrientation::Auto,
            margin: ImageMargin::Small,
        };
        let Ok(plan) = ImagesToPdfPlan::new(vec![jpg_path.clone(), png_path.clone()], opts) else {
            panic!("plan failed");
        };
        let dest = temp.path().join("out.pdf");
        let inspector = StubInspector {
            // Page 1 landscape A4, Page 2 portrait A4
            pages: vec![(841.89, 595.28), (595.28, 841.89)],
        };
        let control = ExportControl::default();
        let mut ticks = Vec::new();
        let res = run_images_to_pdf(
            &plan,
            &dest,
            &LocalImagePdfWriter,
            &inspector,
            &control,
            |done| ticks.push(done),
        );
        let Ok(receipt) = res else {
            panic!("write_pdf failed: {res:?}");
        };
        assert_eq!(receipt.pages, 2);
        assert_eq!(ticks, vec![1, 2]);
        let Ok(pdf) = fs::read(&dest) else {
            panic!("read pdf failed");
        };
        let text = String::from_utf8_lossy(&pdf);
        // Page 1 used direct DCTDecode passthrough, Page 2 used FlateDecode
        assert!(text.contains("/Filter /DCTDecode"));
        assert!(text.contains("/Filter /FlateDecode"));
        // Source files unchanged
        assert_eq!(fs::read(&jpg_path).ok(), jpg_before);
        assert_eq!(fs::read(&png_path).ok(), png_before);
    }

    #[test]
    fn rejects_existing_destination_corrupt_and_unsupported_files() {
        let Ok(temp) = tempfile::tempdir() else {
            panic!("tempdir failed");
        };
        let img = temp.path().join("ok.png");
        write_test_png(&img, 40, 40, [10, 20, 30, 255]);
        let existing = temp.path().join("exists.pdf");
        assert!(fs::write(&existing, b"keep-me").is_ok());

        let Ok(plan) = ImagesToPdfPlan::new(vec![img], ImagesToPdfOptions::default()) else {
            panic!("plan failed");
        };
        let inspector = StubInspector {
            pages: vec![(595.28, 841.89)],
        };
        let err = run_images_to_pdf(
            &plan,
            &existing,
            &LocalImagePdfWriter,
            &inspector,
            &ExportControl::default(),
            |_| {},
        );
        assert!(matches!(err, Err(ImagesToPdfError::DestinationExists)));
        assert_eq!(
            fs::read(&existing).ok().as_deref(),
            Some(b"keep-me".as_slice())
        );

        let bad = temp.path().join("fake.txt");
        assert!(fs::write(&bad, b"not an image").is_ok());
        let Ok(bad_plan) = ImagesToPdfPlan::new(vec![bad], ImagesToPdfOptions::default()) else {
            panic!("plan failed");
        };
        let dest2 = temp.path().join("out2.pdf");
        let err2 = run_images_to_pdf(
            &bad_plan,
            &dest2,
            &LocalImagePdfWriter,
            &inspector,
            &ExportControl::default(),
            |_| {},
        );
        assert!(matches!(err2, Err(ImagesToPdfError::UnsupportedFormat(_))));
        assert!(!dest2.exists());
    }

    #[test]
    fn parses_png_phys_and_jpeg_jfif_dpi() {
        // Minimal PNG header + pHYs chunk with 5906 px/m (~150 DPI)
        let mut png = Vec::new();
        png.extend_from_slice(b"\x89PNG\r\n\x1a\n");
        png.extend_from_slice(&9u32.to_be_bytes());
        png.extend_from_slice(b"pHYs");
        png.extend_from_slice(&5906u32.to_be_bytes());
        png.extend_from_slice(&5906u32.to_be_bytes());
        png.push(1);
        png.extend_from_slice(&0u32.to_be_bytes());
        let dpi = parse_image_dpi(&png, ImageFormat::Png);
        assert!(dpi.is_some());
        if let Some((x, y)) = dpi {
            assert!((x - 150.0).abs() < 0.1 && (y - 150.0).abs() < 0.1);
        }

        // Minimal JPEG SOI + APP0 JFIF 300 DPI
        let jfif = [
            0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x10, b'J', b'F', b'I', b'F', 0x00, 0x01, 0x01, 0x01,
            0x01, 0x2C, 0x01, 0x2C, 0x00, 0x00, 0xFF, 0xD9,
        ];
        let jdpi = parse_image_dpi(&jfif, ImageFormat::Jpeg);
        assert_eq!(jdpi, Some((300.0, 300.0)));
    }

    #[test]
    fn enforces_output_limit_and_cancellation() {
        let Ok(temp) = tempfile::tempdir() else {
            panic!("tempdir failed");
        };
        let img = temp.path().join("big.png");
        write_test_png(&img, 200, 200, [100, 150, 200, 255]);
        let Ok(plan) = ImagesToPdfPlan::new(vec![img], ImagesToPdfOptions::default())
            .and_then(|p| p.with_output_limit(64))
        else {
            panic!("plan failed");
        };
        let dest = temp.path().join("capped.pdf");
        let inspector = StubInspector {
            pages: vec![(595.28, 841.89)],
        };
        let err = run_images_to_pdf(
            &plan,
            &dest,
            &LocalImagePdfWriter,
            &inspector,
            &ExportControl::default(),
            |_| {},
        );
        assert!(matches!(err, Err(ImagesToPdfError::OutputLimit)));
        assert!(!dest.exists());

        let control = ExportControl::default();
        control.cancel();
        let dest2 = temp.path().join("cancelled.pdf");
        let err2 = run_images_to_pdf(
            &plan,
            &dest2,
            &LocalImagePdfWriter,
            &inspector,
            &control,
            |_| {},
        );
        assert!(matches!(err2, Err(ImagesToPdfError::Cancelled)));
        assert!(!dest2.exists());
    }
}
