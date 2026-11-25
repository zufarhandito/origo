//! Native PDFium + QPDF smoke tests for Images (PNG/JPEG) to PDF conversion.
use anyhow::{Result, ensure};
use image::{ImageFormat, Rgb, RgbImage, Rgba, RgbaImage};
use pdf_application::{
    PageRenderer, PdfInspector, SourceDocument, SourceId,
    export::{ExportControl, ExportPreflight},
    images_to_pdf::{
        ImageMargin, ImageOrientation, ImagePageSize, ImagesToPdfError, ImagesToPdfOptions,
        ImagesToPdfPlan, run_images_to_pdf,
    },
};
use pdf_engine::PdfiumRenderer;
use pdf_platform::images_to_pdf::LocalImagePdfWriter;
use qpdf_adapter::QpdfExporter;
use std::path::Path;

fn make_png_with_phys_150_dpi(width: u32, height: u32) -> Result<Vec<u8>> {
    let mut img = RgbaImage::new(width, height);
    for y in 0..height {
        for x in 0..width {
            let color = if y < height / 2 {
                // Semi-transparent red -> blends with white to pinkish-red (255, 128, 128)
                Rgba([255, 0, 0, 128])
            } else {
                // Solid blue
                Rgba([0, 40, 220, 255])
            };
            img.put_pixel(x, y, color);
        }
    }
    let mut raw = Vec::new();
    img.write_to(&mut std::io::Cursor::new(&mut raw), ImageFormat::Png)?;
    // Insert pHYs chunk right after IHDR (byte offset 8 + 4 + 4 + 13 + 4 = 33)
    let mut phys_data = Vec::with_capacity(9);
    // 150 DPI = 5906 px/m
    phys_data.extend_from_slice(&5906u32.to_be_bytes());
    phys_data.extend_from_slice(&5906u32.to_be_bytes());
    phys_data.push(1);
    let mut crc_input = Vec::with_capacity(13);
    crc_input.extend_from_slice(b"pHYs");
    crc_input.extend_from_slice(&phys_data);
    let crc = png_crc32(&crc_input);

    let mut out = Vec::with_capacity(raw.len() + 21);
    out.extend_from_slice(&raw[..33]);
    out.extend_from_slice(&9u32.to_be_bytes());
    out.extend_from_slice(&crc_input);
    out.extend_from_slice(&crc.to_be_bytes());
    out.extend_from_slice(&raw[33..]);
    Ok(out)
}

fn png_crc32(data: &[u8]) -> u32 {
    let mut crc = 0xFFFF_FFFFu32;
    for &byte in data {
        crc ^= u32::from(byte);
        for _ in 0..8 {
            if crc & 1 != 0 {
                crc = (crc >> 1) ^ 0xEDB8_8320;
            } else {
                crc >>= 1;
            }
        }
    }
    !crc
}

fn make_landscape_jpeg(width: u32, height: u32) -> Result<Vec<u8>> {
    let mut img = RgbImage::new(width, height);
    for y in 0..height {
        for x in 0..width {
            let color = if x < width / 2 {
                Rgb([20, 180, 40])
            } else {
                Rgb([240, 210, 20])
            };
            img.put_pixel(x, y, color);
        }
    }
    let mut raw = Vec::new();
    let encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut raw, 95);
    img.write_with_encoder(encoder)?;
    Ok(raw)
}

/// Generates a 320x160 JPEG (red left, blue right) tagged with EXIF Orientation=6 (90° CW).
/// After applying EXIF orientation, it becomes 160x320 portrait with red on TOP and blue on BOTTOM.
fn make_exif_rotated_jpeg() -> Result<Vec<u8>> {
    let mut img = RgbImage::new(320, 160);
    for y in 0..160 {
        for x in 0..320 {
            let color = if x < 160 {
                Rgb([230, 20, 20])
            } else {
                Rgb([20, 40, 230])
            };
            img.put_pixel(x, y, color);
        }
    }
    let mut raw = Vec::new();
    let encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut raw, 95);
    img.write_with_encoder(encoder)?;
    ensure!(raw.starts_with(&[0xFF, 0xD8]), "JPEG SOI");

    // Minimal little-endian TIFF Exif APP1 segment with Orientation = 6 (90° CW)
    let app1: [u8; 36] = [
        0xFF, 0xE1, // APP1 marker
        0x00, 0x22, // length = 34 (2 + 6 + 8 + 2 + 12 + 4)
        b'E', b'x', b'i', b'f', 0x00, 0x00, // Exif header
        b'I', b'I', 0x2A, 0x00, // Little-endian TIFF
        0x08, 0x00, 0x00, 0x00, // IFD0 offset = 8
        0x01, 0x00, // 1 entry
        0x12, 0x01, // Tag 0x0112 (Orientation)
        0x03, 0x00, // Type SHORT (3)
        0x01, 0x00, 0x00, 0x00, // Count = 1
        0x06, 0x00, 0x00, 0x00, // Value = 6 (Rotate90 CW)
        0x00, 0x00, 0x00, 0x00, // Next IFD = 0
    ];
    let mut out = Vec::with_capacity(raw.len() + app1.len());
    out.extend_from_slice(&raw[..2]);
    out.extend_from_slice(&app1);
    out.extend_from_slice(&raw[2..]);
    Ok(out)
}

fn sample_rgb(rendered: &pdf_application::RenderedPage, nx: f32, ny: f32) -> [u8; 3] {
    let x = ((rendered.width as f32 * nx) as u32).min(rendered.width - 1);
    let y = ((rendered.height as f32 * ny) as u32).min(rendered.height - 1);
    let idx = (y as usize * rendered.width as usize + x as usize) * 4;
    [
        rendered.rgba[idx],
        rendered.rgba[idx + 1],
        rendered.rgba[idx + 2],
    ]
}

pub fn run(root: &Path, renderer: &PdfiumRenderer) -> Result<()> {
    let started = std::time::Instant::now();
    let temp = tempfile::tempdir_in(root.join("tests/fixtures"))?;

    let png_bytes = make_png_with_phys_150_dpi(300, 600)?;
    let jpg_bytes = make_landscape_jpeg(480, 320)?;
    let exif_bytes = make_exif_rotated_jpeg()?;

    let png_path = temp.path().join("01-alpha-150dpi.png");
    let jpg_path = temp.path().join("02-landscape.jpg");
    let exif_path = temp.path().join("03-exif-rot90.jpg");
    std::fs::write(&png_path, &png_bytes)?;
    std::fs::write(&jpg_path, &jpg_bytes)?;
    std::fs::write(&exif_path, &exif_bytes)?;

    // 1. Multi-image A4 Auto + Small margin
    let out_a4 = temp.path().join("combined-a4.pdf");
    let plan = ImagesToPdfPlan::new(
        vec![png_path.clone(), jpg_path.clone(), exif_path.clone()],
        ImagesToPdfOptions {
            page_size: ImagePageSize::A4,
            orientation: ImageOrientation::Auto,
            margin: ImageMargin::Small,
        },
    )?;
    let control = ExportControl::default();
    let mut ticks = Vec::new();
    let receipt = run_images_to_pdf(
        &plan,
        &out_a4,
        &LocalImagePdfWriter,
        renderer,
        &control,
        |done| ticks.push(done),
    )?;
    ensure!(
        receipt.pages == 3 && ticks == vec![1, 2, 3],
        "3 pages progress"
    );

    let inspected = renderer.inspect(&out_a4)?;
    ensure!(inspected.pages.len() == 3, "PDFium page count");
    // Page 1: portrait A4
    ensure!(
        (inspected.pages[0].width_points - 595.28).abs() < 1.0
            && (inspected.pages[0].height_points - 841.89).abs() < 1.0,
        "Page 1 should be portrait A4"
    );
    // Page 2: landscape A4
    ensure!(
        (inspected.pages[1].width_points - 841.89).abs() < 1.0
            && (inspected.pages[1].height_points - 595.28).abs() < 1.0,
        "Page 2 should be landscape A4"
    );
    // Page 3: EXIF-rotated 320x160 -> portrait 160x320 -> portrait A4
    ensure!(
        (inspected.pages[2].width_points - 595.28).abs() < 1.0
            && (inspected.pages[2].height_points - 841.89).abs() < 1.0,
        "Page 3 (EXIF Rotate90) should be portrait A4"
    );

    // Verify rendered pixels across all 3 pages
    let p1 = renderer.render(&out_a4, 0)?;
    let p1_top = sample_rgb(&p1, 0.5, 0.25);
    let p1_bot = sample_rgb(&p1, 0.5, 0.75);
    // Semi-transparent red over white -> R~255, G~127, B~127
    ensure!(
        p1_top[0] > 230 && (90..=165).contains(&p1_top[1]) && (90..=165).contains(&p1_top[2]),
        "Page 1 alpha composite over white failed: {p1_top:?}"
    );
    ensure!(
        p1_bot[2] > 180 && p1_bot[0] < 40,
        "Page 1 bottom blue failed: {p1_bot:?}"
    );

    let p2 = renderer.render(&out_a4, 1)?;
    let p2_left = sample_rgb(&p2, 0.25, 0.5);
    let p2_right = sample_rgb(&p2, 0.75, 0.5);
    ensure!(
        p2_left[1] > 140 && p2_left[0] < 70,
        "Page 2 left green failed: {p2_left:?}"
    );
    ensure!(
        p2_right[0] > 200 && p2_right[1] > 170 && p2_right[2] < 70,
        "Page 2 right yellow failed: {p2_right:?}"
    );

    let p3 = renderer.render(&out_a4, 2)?;
    let p3_top = sample_rgb(&p3, 0.5, 0.25);
    let p3_bot = sample_rgb(&p3, 0.5, 0.75);
    ensure!(
        p3_top[0] > 180 && p3_top[2] < 60,
        "Page 3 EXIF top red failed: {p3_top:?}"
    );
    ensure!(
        p3_bot[2] > 180 && p3_bot[0] < 60,
        "Page 3 EXIF bottom blue failed: {p3_bot:?}"
    );

    // Verify generated PDF passes Origo's QPDF preflight cleanly
    let qpdf = QpdfExporter::new(&root.join("sidecars"));
    let preflight_doc = SourceDocument {
        id: SourceId(1),
        path: out_a4.clone(),
        fingerprint: inspected.fingerprint,
        geometries: inspected.pages.clone(),
        export_support: pdf_application::ExportSupport::NotPreflighted,
        annotations: inspected.annotations.clone(),
    };
    let report = qpdf.preflight(&[preflight_doc], &ExportControl::default())?;
    ensure!(
        !report.requires_acknowledgement(),
        "Generated PDF should be clean in QPDF preflight"
    );

    // 2. FitImage mode with PNG pHYs 150 DPI (300x600 -> 144x288 pt)
    let out_fit = temp.path().join("fit-dpi.pdf");
    let fit_plan = ImagesToPdfPlan::new(
        vec![png_path.clone()],
        ImagesToPdfOptions {
            page_size: ImagePageSize::FitImage,
            orientation: ImageOrientation::Auto,
            margin: ImageMargin::None,
        },
    )?;
    run_images_to_pdf(
        &fit_plan,
        &out_fit,
        &LocalImagePdfWriter,
        renderer,
        &ExportControl::default(),
        |_| {},
    )?;
    let fit_info = renderer.inspect(&out_fit)?;
    ensure!(
        (fit_info.pages[0].width_points - 144.0).abs() < 1.0
            && (fit_info.pages[0].height_points - 288.0).abs() < 1.0,
        "FitImage pHYs 150 DPI page size mismatch: {:?}",
        fit_info.pages[0]
    );

    // 3. No-clobber existing destination & corrupt image cleanup
    let existing_err = run_images_to_pdf(
        &fit_plan,
        &out_fit,
        &LocalImagePdfWriter,
        renderer,
        &ExportControl::default(),
        |_| {},
    );
    ensure!(
        matches!(existing_err, Err(ImagesToPdfError::DestinationExists)),
        "Existing destination must be rejected"
    );

    let corrupt_path = temp.path().join("corrupt.jpg");
    std::fs::write(&corrupt_path, b"\xFF\xD8\xFF\xE0broken")?;
    let corrupt_dest = temp.path().join("should-not-exist.pdf");
    let corrupt_plan = ImagesToPdfPlan::new(vec![corrupt_path], ImagesToPdfOptions::default())?;
    let corrupt_err = run_images_to_pdf(
        &corrupt_plan,
        &corrupt_dest,
        &LocalImagePdfWriter,
        renderer,
        &ExportControl::default(),
        |_| {},
    );
    ensure!(
        matches!(corrupt_err, Err(ImagesToPdfError::CorruptImage(_))) && !corrupt_dest.exists(),
        "Corrupt image must fail cleanly without output"
    );

    // 4. Verify all source images remain byte-identical
    ensure!(std::fs::read(&png_path)? == png_bytes, "PNG source changed");
    ensure!(
        std::fs::read(&jpg_path)? == jpg_bytes,
        "JPEG source changed"
    );
    ensure!(
        std::fs::read(&exif_path)? == exif_bytes,
        "EXIF JPEG source changed"
    );

    println!(
        "PASS images-to-pdf: PNG alpha+150DPI, JPEG DCTDecode, EXIF 90CW, QPDF preflight, FitImage, no-clobber, corrupt cleanup ({} ms)",
        started.elapsed().as_millis()
    );
    Ok(())
}
