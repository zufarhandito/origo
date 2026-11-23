use anyhow::{Context, Result, ensure};
use pdf_application::{
    PageRenderer, PdfInspector, RenderRequest, SourceId,
    export::{ExportControl, ExportError, ExportPlan, ExportScope, finish_export, prepare_export},
    open_document,
    text::{TextExtractor, TextRequest},
};
use pdf_engine::PdfiumRenderer;
use pdf_ocr::{Cached, Store, Tesseract};
use pdf_platform::LocalOutput;
use qpdf_adapter::{QpdfExporter, SearchableExporter};
use std::path::Path;

pub fn run(root: &Path, fixtures: &Path) -> Result<()> {
    let sidecars = root.join("sidecars");
    let renderer = PdfiumRenderer::new(&sidecars);
    let engine = Tesseract::new(&sidecars);
    let dir = tempfile::Builder::new()
        .prefix("ocr-export-")
        .tempdir_in(fixtures)?;
    let source = dir.path().join("Scan 日本.pdf");
    let bytes = super::ocr::make_book(fixtures, 3, true)?;
    std::fs::write(&source, &bytes)?;
    let mut workspace = open_document(&renderer, &source, SourceId(1))?;
    let page = super::ocr::request_for(&workspace, 0)?;
    let store = Store::new(Some(dir.path()));
    let control = ExportControl::default();
    let Cached::Text(recognition) = page.recognize(&renderer, &engine, &store, &control, false)?
    else {
        anyhow::bail!("expected scan text");
    };
    let mut expected = recognition.text_page();
    let page_id = workspace.pages()[0].id;
    workspace.execute(pdf_domain::Command::Rotate {
        ids: vec![page_id],
        turns: 1,
    })?;
    expected.rotate(1);

    let keys = (0..3)
        .map(|i| super::ocr::request_for(&workspace, i).map(|p| p.key()))
        .collect::<Result<Vec<_>>>()?;
    let frozen = store.freeze(&keys, &control)?;
    ensure!(frozen.pages() == 3 && frozen.recognized() == 1);
    store.clear()?;
    let plan = ExportPlan::capture(&workspace, 1, ExportScope::Searchable)?;
    let base = QpdfExporter::new(&sidecars);
    let output = dir.path().join("Searchable.pdf");
    let prepared = prepare_export(plan.clone(), &output, &LocalOutput, &base, &control)?;
    let exporter = SearchableExporter::new(base.clone(), frozen, renderer.clone());
    let receipt = finish_export(prepared, true, &exporter, &renderer, &control)?;
    ensure!(!receipt.apply(1, &mut workspace) && workspace.is_dirty());
    let info = renderer.inspect_output(&output)?;
    let text = |page_index| {
        renderer.extract_text(&TextRequest {
            path: output.clone(),
            page_index,
            fingerprint: info.fingerprint,
        })
    };
    let scan = text(0)?;
    ensure!(
        scan.copy_range(0..scan.chars.len())
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
            == "Buku ini memuat 123 halaman.",
        "OCR word spacing or reading order lost"
    );
    let actual = scan
        .chars
        .iter()
        .find(|ch| ch.unicode == 'B')
        .and_then(|ch| ch.bounds)
        .context("missing output glyph bounds")?;
    let expected = expected.chars[0]
        .bounds
        .context("missing source glyph bounds")?;
    ensure!(
        (actual.left - expected.left).abs() < 0.015 && (actual.top - expected.top).abs() < 0.015,
        "OCR text detached from the rotated scan"
    );
    ensure!(!pdf_ocr::has_text(&text(1)?));
    let native = text(2)?;
    ensure!(
        native
            .copy_range(0..native.chars.len())
            .contains("Native text remains searchable.")
    );
    let baseline = dir.path().join("Normal.pdf");
    let normal_plan = ExportPlan::capture(&workspace, 1, ExportScope::All)?;
    let plain = prepare_export(
        normal_plan,
        &baseline,
        &LocalOutput,
        &base,
        &ExportControl::default(),
    )?;
    finish_export(plain, true, &base, &renderer, &ExportControl::default())?;
    let baseline_info = renderer.inspect_output(&baseline)?;
    for index in 0..3 {
        let request = RenderRequest {
            annotations: None,
            path: baseline.clone(),
            page_index: index,
            target_width: 640,
            rotation_delta: 0,
            fingerprint: Some(baseline_info.fingerprint),
        };
        let before = renderer.render_page(&request)?;
        let after = renderer.render_page(&RenderRequest {
            path: output.clone(),
            fingerprint: Some(info.fingerprint),
            rotation_delta: 0,
            ..request
        })?;
        ensure!(
            (before.width, before.height, before.rgba) == (after.width, after.height, after.rgba),
            "OCR layer changed page {} pixels",
            index + 1
        );
    }
    renderer.inspect_output(&baseline)?;
    renderer.inspect_output(&output)?;
    ensure!(std::fs::read(&source)? == bytes);
    ensure!(matches!(
        prepare_export(
            plan.clone(),
            &output,
            &LocalOutput,
            &base,
            &ExportControl::default()
        ),
        Err(ExportError::DestinationExists)
    ));
    let cancelled = ExportControl::default();
    let cancelled_path = dir.path().join("Cancelled.pdf");
    let staged = prepare_export(plan, &cancelled_path, &LocalOutput, &base, &cancelled)?;
    cancelled.cancel();
    ensure!(
        matches!(
            finish_export(staged, true, &exporter, &renderer, &cancelled),
            Err(ExportError::Cancelled)
        ) && !cancelled_path.exists()
    );
    let evidence = dir.keep();
    println!(
        "PASS searchable PDF: frozen coverage, rotated text, original rendering and bytes, native text, no overwrite and cancel"
    );
    println!(
        "Synthetic searchable PDF: {}",
        evidence.join("Searchable.pdf").display()
    );
    Ok(())
}
