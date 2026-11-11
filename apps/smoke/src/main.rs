use anyhow::{Context, Result, ensure};
use pdf_application::{
    PageRenderer, PdfInspector, RenderError, RenderRequest, SourceId, open_document,
};
use pdf_engine::PdfiumRenderer;
use qpdf_adapter::QpdfProbe;
use std::path::PathBuf;
mod books;
mod synthetic;
mod text;

fn m1_smoke(root: &std::path::Path, renderer: &PdfiumRenderer) -> Result<()> {
    let started = std::time::Instant::now();
    let temporary = tempfile::tempdir_in(root.join("tests/fixtures"))?;
    let path = temporary.path().join("100-pages.pdf");
    let bytes = synthetic::pdf(100, true);
    std::fs::write(&path, &bytes)?;
    let info = renderer.inspect(&path)?;
    ensure!(info.pages.len() == 100, "M1 inspected page count");
    let workspace = open_document(renderer, &path, SourceId(99))?;
    ensure!(
        workspace.pages()[99].source_page_index == 99 && !workspace.is_dirty(),
        "Workspace mapping"
    );
    ensure!(
        info.pages.iter().any(|g| g.quarter_turns == 1),
        "Rotated source detection"
    );
    ensure!(
        info.pages.iter().any(|g| g.width_points > g.height_points),
        "Mixed page orientation"
    );
    for (index, width) in [(0, 256), (99, 256), (25, 1600), (3, 8192)] {
        let rendered = renderer.render_page(&RenderRequest {
            path: path.clone(),
            page_index: index,
            target_width: width,
            rotation_delta: 0,
            fingerprint: Some(info.fingerprint),
        })?;
        ensure!(rendered.rgba.len() <= 64 * 1024 * 1024, "Raster memory cap");
        ensure!(
            rendered.width <= 8192 && rendered.height <= 8192,
            "Raster dimension cap"
        );
        let first = &rendered.rgba[..4];
        if index % 2 == 0 {
            ensure!(first[0] > first[2], "Source red page identity");
        } else {
            ensure!(first[2] > first[0], "Source blue page identity");
        }
    }
    let limited = renderer.clone().with_limits(pdf_engine::ResourceLimits {
        max_pages: 10,
        ..Default::default()
    })?;
    ensure!(
        matches!(limited.inspect(&path), Err(RenderError::ResourceLimit)),
        "Configurable page limit"
    );
    ensure!(
        matches!(renderer.render(&path, 100), Err(RenderError::InvalidPage)),
        "Out-of-range page rejected"
    );
    ensure!(
        std::fs::read(&path)? == bytes,
        "M1 viewer source bytes unchanged"
    );
    let mut modified = bytes.clone();
    modified.push(b'\n');
    std::fs::write(&path, modified)?;
    ensure!(
        matches!(
            renderer.render_page(&RenderRequest {
                path,
                page_index: 0,
                target_width: 256,
                rotation_delta: 0,
                fingerprint: Some(info.fingerprint)
            }),
            Err(RenderError::SourceChanged)
        ),
        "Changed source rejected"
    );
    println!(
        "PASS M1: 100-page inspection, workspace, mixed sizes/rotation, selected renders, resource limits, source-change rejection ({} ms)",
        started.elapsed().as_millis()
    );
    Ok(())
}

fn main() -> Result<()> {
    let root = dunce::canonicalize(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../.."))?;
    if std::env::args().nth(1).as_deref() == Some("--benchmark-books") {
        return books::run(&root);
    }
    if std::env::args().nth(1).as_deref() == Some("--write-fixtures") {
        std::fs::write(
            root.join("tests/fixtures/hundred-pages.pdf"),
            synthetic::pdf(100, false),
        )?;
        std::fs::write(
            root.join("tests/fixtures/mixed-pages.pdf"),
            synthetic::pdf(16, true),
        )?;
        std::fs::write(
            root.join("tests/fixtures/text-selection.pdf"),
            synthetic::selection_pdf(),
        )?;
        println!("Generated 100-page and mixed-size synthetic fixtures");
        return Ok(());
    }
    let sidecars = root.join("sidecars");
    let fixture = root.join("tests/fixtures/two-pages.pdf");
    let before =
        std::fs::read(&fixture).context("Generate fixture with scripts/new-fixture.ps1")?;
    let renderer = PdfiumRenderer::new(&sidecars);
    let a = renderer.render(&fixture, 0)?;
    let b = renderer.render(&fixture, 1)?;
    ensure!(a.page_count == 2 && b.page_count == 2, "Fixture page count");
    ensure!(a.rgba != b.rgba, "Synthetic pages must be distinguishable");
    let temp = tempfile::tempdir_in(root.join("tests/fixtures"))?;
    let output = temp.path().join("result.pdf");
    let second_source = temp.path().join("日本 & input.pdf");
    std::fs::write(&second_source, &before)?;
    let qpdf = QpdfProbe::new(&sidecars);
    qpdf.verify_version()?;
    qpdf.transform_fixture(&fixture, &second_source, &output)?;
    let first = renderer.render(&output, 0)?;
    let second = renderer.render(&output, 1)?;
    ensure!(
        first.page_count == 2 && second.page_count == 2,
        "Export count"
    );
    ensure!(first.clockwise_quarter_turns == 1, "Output rotation");
    ensure!(
        second.clockwise_quarter_turns == 0,
        "Second page orientation"
    );
    ensure!(second.rgba == a.rgba, "Output page 2 must be source page 1");
    // Solid page backgrounds allow comparison despite page-1 rotation.
    ensure!(
        first.rgba[..4] == b.rgba[..4],
        "Output page 1 must be source page 2"
    );
    ensure!(
        std::fs::read(&fixture)? == before,
        "Source fixture was modified"
    );
    ensure!(
        std::fs::read(&second_source)? == before,
        "Second source was modified"
    );
    println!("PASS: pinned QPDF, PDFium rendering, page order, rotation, source unchanged");
    m1_smoke(&root, &renderer)?;
    text::run(&root, &renderer)?;
    Ok(())
}
