//! Streaming bounded JPEG-recompression PDF writer with PDFium validation and atomic no-clobber commit.
use super::*;
use image::{ExtendedColorType, codecs::jpeg::JpegEncoder};
use pdf_application::{
    ExactPageRenderer, ExactRenderRequest, PdfInspector,
    compress::{CompressError, CompressPlan, CompressReceipt, PdfCompressor},
    export::{ExportControl, ExportError, ExportPhase, resolve_source},
};
use std::io::BufWriter;

#[derive(Clone, Copy, Default)]
pub struct LocalPdfCompressor;

struct BoundedWriter<'a> {
    writer: BufWriter<File>,
    control: &'a ExportControl,
    remaining: u64,
    written: u64,
}

impl<'a> BoundedWriter<'a> {
    fn new(file: File, control: &'a ExportControl, limit: u64) -> Self {
        Self {
            writer: BufWriter::with_capacity(64 * 1024, file),
            control,
            remaining: limit,
            written: 0,
        }
    }

    fn write_bytes(&mut self, bytes: &[u8]) -> Result<(), CompressError> {
        self.control.checkpoint().map_err(map_export_err)?;
        let len = bytes.len() as u64;
        if len > self.remaining {
            return Err(CompressError::OutputLimit);
        }
        self.writer
            .write_all(bytes)
            .map_err(|_| CompressError::Io)?;
        self.remaining -= len;
        self.written += len;
        Ok(())
    }

    fn offset(&self) -> u64 {
        self.written
    }

    fn finish(mut self) -> Result<u64, CompressError> {
        self.writer.flush().map_err(|_| CompressError::Io)?;
        let file = self.writer.into_inner().map_err(|_| CompressError::Io)?;
        file.sync_all().map_err(|_| CompressError::Io)?;
        Ok(self.written)
    }
}

fn map_export_err(err: ExportError) -> CompressError {
    match err {
        ExportError::Cancelled => CompressError::Cancelled,
        ExportError::Destination => CompressError::Destination,
        ExportError::DestinationExists => CompressError::DestinationExists,
        ExportError::Validation => CompressError::Validation,
        ExportError::Io => CompressError::Io,
        other => CompressError::Export(other),
    }
}

fn encode_rgba_as_jpeg(
    rgba: &[u8],
    width: u32,
    height: u32,
    quality: u8,
    grayscale: bool,
) -> Result<Vec<u8>, CompressError> {
    let pixel_count = usize::try_from(width)
        .ok()
        .and_then(|w| usize::try_from(height).ok().and_then(|h| w.checked_mul(h)))
        .ok_or(CompressError::Encoding)?;
    if rgba.len() != pixel_count.saturating_mul(4) {
        return Err(CompressError::Encoding);
    }

    let mut jpeg = Vec::with_capacity(pixel_count / 4);
    let mut encoder = JpegEncoder::new_with_quality(&mut jpeg, quality);
    if grayscale {
        let mut luma = Vec::with_capacity(pixel_count);
        for px in rgba.as_chunks::<4>().0 {
            let a = u32::from(px[3]);
            let (r, g, b) = if a == 255 {
                (u32::from(px[0]), u32::from(px[1]), u32::from(px[2]))
            } else {
                let inv = 255 - a;
                (
                    (u32::from(px[0]) * a + 255 * inv + 127) / 255,
                    (u32::from(px[1]) * a + 255 * inv + 127) / 255,
                    (u32::from(px[2]) * a + 255 * inv + 127) / 255,
                )
            };
            let y = ((77 * r + 150 * g + 29 * b + 128) >> 8) as u8;
            luma.push(y);
        }
        encoder
            .encode(&luma, width, height, ExtendedColorType::L8)
            .map_err(|_| CompressError::Encoding)?;
    } else {
        let mut rgb = Vec::with_capacity(pixel_count.saturating_mul(3));
        for px in rgba.as_chunks::<4>().0 {
            let a = u32::from(px[3]);
            if a == 255 {
                rgb.push(px[0]);
                rgb.push(px[1]);
                rgb.push(px[2]);
            } else {
                let inv = 255 - a;
                rgb.push(((u32::from(px[0]) * a + 255 * inv + 127) / 255) as u8);
                rgb.push(((u32::from(px[1]) * a + 255 * inv + 127) / 255) as u8);
                rgb.push(((u32::from(px[2]) * a + 255 * inv + 127) / 255) as u8);
            }
        }
        encoder
            .encode(&rgb, width, height, ExtendedColorType::Rgb8)
            .map_err(|_| CompressError::Encoding)?;
    }
    if jpeg.len() < 4 || jpeg[0] != 0xFF || jpeg[1] != 0xD8 {
        return Err(CompressError::Encoding);
    }
    Ok(jpeg)
}

/// Write a standalone single-page clean PDF containing only the rasterized image (no text,
/// no annotations, no metadata). Used by True Redaction, Flatten PDF, and page standardization.
#[allow(clippy::too_many_arguments)]
pub fn write_single_page_raster_pdf(
    path: &Path,
    rgba: &[u8],
    width: u32,
    height: u32,
    page_w: f32,
    page_h: f32,
    quality: u8,
    control: &ExportControl,
) -> Result<(), ExportError> {
    write_single_page_raster_pdf_placed(
        path,
        rgba,
        width,
        height,
        page_w,
        page_h,
        [0.0, 0.0, page_w, page_h],
        quality,
        control,
    )
}

/// Write a single-page clean PDF placing the raster inside `draw_rect = [x, y, w, h]` (points)
/// on a canvas of `(page_w, page_h)` points. Enables both full-page replacement and centered
/// paper-size standardization (A4/Letter/F4/Legal).
#[allow(clippy::too_many_arguments)]
pub fn write_single_page_raster_pdf_placed(
    path: &Path,
    rgba: &[u8],
    width: u32,
    height: u32,
    page_w: f32,
    page_h: f32,
    draw_rect: [f32; 4],
    quality: u8,
    control: &ExportControl,
) -> Result<(), ExportError> {
    control.checkpoint()?;
    let jpeg = encode_rgba_as_jpeg(rgba, width, height, quality, false).map_err(|e| match e {
        CompressError::Cancelled => ExportError::Cancelled,
        _ => ExportError::Engine,
    })?;
    let file = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(path)
        .map_err(|_| ExportError::Io)?;
    let mut writer = BoundedWriter::new(file, control, 128 * 1024 * 1024);
    let mut offsets: Vec<u64> = Vec::with_capacity(5);
    let map_err = |e: CompressError| match e {
        CompressError::Cancelled => ExportError::Cancelled,
        CompressError::OutputLimit => ExportError::ResourceLimit,
        _ => ExportError::Io,
    };

    writer
        .write_bytes(b"%PDF-1.4\n%\xE2\xE3\xCF\xD3\n")
        .map_err(map_err)?;
    offsets.push(writer.offset());
    writer
        .write_bytes(b"1 0 obj\n<< /Type /Catalog /Pages 2 0 R >>\nendobj\n")
        .map_err(map_err)?;
    offsets.push(writer.offset());
    writer
        .write_bytes(b"2 0 obj\n<< /Type /Pages /Kids [5 0 R] /Count 1 >>\nendobj\n")
        .map_err(map_err)?;
    offsets.push(writer.offset());
    let img_header = format!(
        "3 0 obj\n<< /Type /XObject /Subtype /Image /Width {width} /Height {height} /ColorSpace /DeviceRGB /BitsPerComponent 8 /Filter /DCTDecode /Interpolate true /Length {} >>\nstream\n",
        jpeg.len()
    );
    writer.write_bytes(img_header.as_bytes()).map_err(map_err)?;
    writer.write_bytes(&jpeg).map_err(map_err)?;
    writer
        .write_bytes(b"\nendstream\nendobj\n")
        .map_err(map_err)?;
    let [dx, dy, dw, dh] = draw_rect;
    let content = format!("q\n{dw:.3} 0 0 {dh:.3} {dx:.3} {dy:.3} cm\n/Im0 Do\nQ\n");
    offsets.push(writer.offset());
    writer
        .write_bytes(
            format!(
                "4 0 obj\n<< /Length {} >>\nstream\n{content}endstream\nendobj\n",
                content.len()
            )
            .as_bytes(),
        )
        .map_err(map_err)?;
    offsets.push(writer.offset());
    writer
        .write_bytes(
            format!(
                "5 0 obj\n<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {page_w:.3} {page_h:.3}] /Resources << /XObject << /Im0 3 0 R >> >> /Contents 4 0 R >>\nendobj\n"
            )
            .as_bytes(),
        )
        .map_err(map_err)?;

    let xref_offset = writer.offset();
    let total_entries = offsets.len() + 1;
    writer
        .write_bytes(format!("xref\n0 {total_entries}\n0000000000 65535 f \n").as_bytes())
        .map_err(map_err)?;
    for offset in &offsets {
        writer
            .write_bytes(format!("{offset:010} 00000 n \n").as_bytes())
            .map_err(map_err)?;
    }
    writer
        .write_bytes(
            format!(
                "trailer\n<< /Size {total_entries} /Root 1 0 R >>\nstartxref\n{xref_offset}\n%%EOF\n"
            )
            .as_bytes(),
        )
        .map_err(map_err)?;
    let written = writer.finish().map_err(map_err)?;
    if written == 0 {
        return Err(ExportError::Validation);
    }
    Ok(())
}

fn verify_plan_sources(plan: &CompressPlan) -> Result<(), CompressError> {
    for source in plan.snapshot().sources() {
        let actual = fingerprint(&source.path).map_err(map_export_err)?;
        if actual != source.fingerprint {
            return Err(CompressError::Export(ExportError::SourceChanged));
        }
    }
    Ok(())
}

impl PdfCompressor for LocalPdfCompressor {
    fn compress_pdf(
        &self,
        plan: &CompressPlan,
        destination: &Path,
        renderer: &impl ExactPageRenderer,
        inspector: &impl PdfInspector,
        control: &ExportControl,
        progress: &mut dyn FnMut(usize),
    ) -> Result<CompressReceipt, CompressError> {
        control.checkpoint().map_err(map_export_err)?;
        control.set_phase(ExportPhase::Snapshot);
        verify_plan_sources(plan)?;
        let staged = StagedPdfOutput::stage(destination, control).map_err(map_export_err)?;
        let file = OpenOptions::new()
            .write(true)
            .truncate(true)
            .open(staged.output_path())
            .map_err(|_| CompressError::Io)?;

        let count = plan.page_count();
        let options = plan.options();
        let quality = options.preset.jpeg_quality();
        let color_space = if options.grayscale {
            "/DeviceGray"
        } else {
            "/DeviceRGB"
        };

        let mut offsets: Vec<u64> = Vec::with_capacity(2 + count * 3);
        let mut expected_boxes: Vec<(f32, f32)> = Vec::with_capacity(count);
        let mut writer = BoundedWriter::new(file, control, plan.output_limit());

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

        for (index, (page, &(width, height))) in
            plan.snapshot().pages().iter().zip(plan.sizes()).enumerate()
        {
            control.checkpoint().map_err(map_export_err)?;
            let source = resolve_source(plan.snapshot().sources(), page.entry.source_id)
                .map_err(map_export_err)?;

            let started = Instant::now();
            let raster = renderer.render_exact(&ExactRenderRequest {
                annotations: Some(page.annotations.clone()),
                path: source.path.clone(),
                page_index: page.entry.source_page_index,
                rotation_delta: page.entry.rotation_delta,
                fingerprint: source.fingerprint,
                width,
                height,
            })?;
            control.charge(started.elapsed());
            control.remaining().map_err(map_export_err)?;

            if raster.width != width
                || raster.height != height
                || raster.rgba.len() != width as usize * height as usize * 4
            {
                return Err(CompressError::Validation);
            }

            let encode_started = Instant::now();
            let jpeg =
                encode_rgba_as_jpeg(&raster.rgba, width, height, quality, options.grayscale)?;
            drop(raster);
            control.charge(encode_started.elapsed());
            control.remaining().map_err(map_export_err)?;

            let img_id = 3 + index * 3;
            let content_id = 4 + index * 3;
            let page_id = 5 + index * 3;
            let page_w = page.geometry.width_points;
            let page_h = page.geometry.height_points;

            // Image XObject
            offsets.push(writer.offset());
            let img_header = format!(
                "{img_id} 0 obj\n<< /Type /XObject /Subtype /Image /Width {width} /Height {height} /ColorSpace {color_space} /BitsPerComponent 8 /Filter /DCTDecode /Interpolate true /Length {} >>\nstream\n",
                jpeg.len()
            );
            writer.write_bytes(img_header.as_bytes())?;
            writer.write_bytes(&jpeg)?;
            writer.write_bytes(b"\nendstream\nendobj\n")?;
            drop(jpeg);

            // Content stream fitting image cleanly to the page MediaBox
            let content = format!("q\n{page_w:.3} 0 0 {page_h:.3} 0 0 cm\n/Im0 Do\nQ\n");
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
                    "{page_id} 0 obj\n<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {page_w:.3} {page_h:.3}] /Resources << /XObject << /Im0 {img_id} 0 R >> >> /Contents {content_id} 0 R >>\nendobj\n"
                )
                .as_bytes(),
            )?;

            expected_boxes.push((page_w, page_h));
            progress(index + 1);
        }

        // Cross-reference table & trailer
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
        let written = writer.finish()?;
        if written == 0 {
            return Err(CompressError::Validation);
        }

        control.checkpoint().map_err(map_export_err)?;
        control.set_phase(ExportPhase::Validation);
        verify_plan_sources(plan)?;
        let started = Instant::now();
        let inspected = inspector
            .inspect_output(staged.output_path())
            .map_err(|_| CompressError::Validation)?;
        control.charge(started.elapsed());
        control.remaining().map_err(map_export_err)?;

        if inspected.pages.len() != expected_boxes.len() {
            return Err(CompressError::Validation);
        }
        for (actual, &(exp_w, exp_h)) in inspected.pages.iter().zip(&expected_boxes) {
            if (actual.width_points - exp_w).abs() > 1.0
                || (actual.height_points - exp_h).abs() > 1.0
            {
                return Err(CompressError::Validation);
            }
        }

        control.checkpoint().map_err(map_export_err)?;
        control.set_phase(ExportPhase::Commit);
        let (committed, output_bytes) = staged.commit(control).map_err(map_export_err)?;
        control.set_phase(ExportPhase::Finished);

        Ok(CompressReceipt {
            instance: plan.snapshot().instance(),
            revision: plan.snapshot().revision(),
            destination: committed,
            pages: count,
            baseline_bytes: plan.baseline_bytes(),
            output_bytes,
            preset: options.preset,
            grayscale: options.grayscale,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pdf_application::{
        ExportSupport, InspectedDocument, PageGeometry, RenderError, RenderedPage, SourceId,
        Workspace,
        compress::{CompressOptions, CompressPreset, run_compress_pdf},
        export::{ExportPlan, ExportScope},
    };

    struct FakeRenderer;
    impl ExactPageRenderer for FakeRenderer {
        fn render_exact(&self, request: &ExactRenderRequest) -> Result<RenderedPage, RenderError> {
            let mut rgba = vec![255u8; request.width as usize * request.height as usize * 4];
            // Draw a soft horizontal gradient so JPEG compression has realistic content.
            for y in 0..request.height as usize {
                for x in 0..request.width as usize {
                    let i = (y * request.width as usize + x) * 4;
                    rgba[i] = ((x * 255) / (request.width as usize).max(1)) as u8;
                    rgba[i + 1] = ((y * 255) / (request.height as usize).max(1)) as u8;
                    rgba[i + 2] = 200;
                    rgba[i + 3] = 255;
                }
            }
            Ok(RenderedPage {
                width: request.width,
                height: request.height,
                rgba,
                page_count: 1,
                clockwise_quarter_turns: 0,
            })
        }
    }

    struct FakeInspector(Vec<PageGeometry>);
    impl PdfInspector for FakeInspector {
        fn inspect(&self, source: &Path) -> Result<InspectedDocument, RenderError> {
            let bytes = fs::read(source).map_err(|_| RenderError::InvalidPdf)?;
            if !bytes.starts_with(b"%PDF-1.4") || !bytes.ends_with(b"%%EOF\n") {
                return Err(RenderError::InvalidPdf);
            }
            Ok(InspectedDocument {
                annotations: vec![],
                path: source.into(),
                fingerprint: fingerprint(source).map_err(|_| RenderError::InvalidPdf)?,
                pages: self.0.clone(),
            })
        }
    }

    #[test]
    fn compresses_rgb_and_grayscale_and_respects_noclobber()
    -> Result<(), Box<dyn std::error::Error>> {
        let dir = tempfile::tempdir()?;
        let src = dir.path().join("source.pdf");
        // Write a 256 KB synthetic source placeholder to verify baseline/savings math.
        fs::write(&src, vec![b'A'; 256 * 1024])?;
        let fp = fingerprint(&src)?;
        let geom = PageGeometry {
            width_points: 595.0,
            height_points: 842.0,
            quarter_turns: 0,
        };
        let workspace = Workspace::new(SourceDocument {
            id: SourceId(1),
            path: src,
            fingerprint: fp,
            geometries: vec![geom, geom],
            export_support: ExportSupport::NotPreflighted,
            annotations: vec![vec![], vec![]],
        })?;
        let snapshot = ExportPlan::capture(&workspace, 1, ExportScope::All)?;
        let inspector = FakeInspector(vec![geom, geom]);
        let control = ExportControl::default();

        // 1. RGB Compact preset
        let plan_rgb = CompressPlan::new(
            snapshot.clone(),
            CompressOptions {
                preset: CompressPreset::Compact,
                grayscale: false,
            },
        )?;
        let out_rgb = dir.path().join("out-rgb.pdf");
        let mut ticks = 0usize;
        let receipt_rgb = run_compress_pdf(
            &plan_rgb,
            &out_rgb,
            &FakeRenderer,
            &inspector,
            &LocalPdfCompressor,
            &control,
            |done| ticks = done,
        )?;
        assert_eq!(ticks, 2);
        assert_eq!(receipt_rgb.pages, 2);
        assert!(receipt_rgb.output_bytes < receipt_rgb.baseline_bytes);
        let raw_rgb = fs::read(&out_rgb)?;
        assert!(raw_rgb.windows(10).any(|w| w == b"/DeviceRGB"));
        assert!(raw_rgb.windows(10).any(|w| w == b"/DCTDecode"));

        // 2. Grayscale Compact preset should be even smaller than RGB
        let plan_gray = CompressPlan::new(
            snapshot,
            CompressOptions {
                preset: CompressPreset::Compact,
                grayscale: true,
            },
        )?;
        let out_gray = dir.path().join("out-gray.pdf");
        let receipt_gray = run_compress_pdf(
            &plan_gray,
            &out_gray,
            &FakeRenderer,
            &inspector,
            &LocalPdfCompressor,
            &control,
            |_| {},
        )?;
        assert!(receipt_gray.output_bytes < receipt_rgb.output_bytes);
        let raw_gray = fs::read(&out_gray)?;
        assert!(raw_gray.windows(11).any(|w| w == b"/DeviceGray"));

        // 3. No-clobber: writing to existing out_rgb must fail with DestinationExists
        let err = run_compress_pdf(
            &plan_rgb,
            &out_rgb,
            &FakeRenderer,
            &inspector,
            &LocalPdfCompressor,
            &control,
            |_| {},
        );
        assert!(matches!(err, Err(CompressError::DestinationExists)));

        // 4. Cancellation leaves destination absent
        let cancelled = ExportControl::default();
        cancelled.cancel();
        let out_cancel = dir.path().join("cancelled.pdf");
        let err = run_compress_pdf(
            &plan_rgb,
            &out_cancel,
            &FakeRenderer,
            &inspector,
            &LocalPdfCompressor,
            &cancelled,
            |_| {},
        );
        assert!(matches!(err, Err(CompressError::Cancelled)));
        assert!(!out_cancel.exists());
        Ok(())
    }
}
