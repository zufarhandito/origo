//! Round-trip checks use only generated PDFs and production export transactions.
use anyhow::{Result, ensure};
use pdf_application::{
    PageRenderer, RenderRequest, SourceId,
    export::{ExportControl, ExportPlan, ExportScope, finish_export, prepare_export},
    open_document,
};
use pdf_domain::{Command, annotation::Annotation};
use pdf_engine::PdfiumRenderer;
use pdf_platform::LocalOutput;
use qpdf_adapter::QpdfExporter;
use std::path::Path;
pub fn run(root: &Path, renderer: &PdfiumRenderer) -> Result<()> {
    let dir = tempfile::tempdir_in(root.join(".research"))?;
    let source = dir.path().join("source.pdf");
    let bytes = crate::synthetic::export_safety_pdf("plain");
    std::fs::write(&source, &bytes)?;
    let mut w = open_document(renderer, &source, SourceId(1))?;
    let page = w.pages()[0].id;
    let highlight = Annotation {
        id: 1,
        highlight: true,
        color: [255, 210, 0],
        width: 1.,
        points: vec![[0.1, 0.1], [0.8, 0.2]],
        rectangles: true,
    };
    let ink = Annotation {
        id: 2,
        highlight: false,
        color: [220, 30, 30],
        width: 4.,
        points: vec![[0.2, 0.3], [0.6, 0.6], [0.3, 0.7]],
        rectangles: false,
    };
    w.execute(Command::Annotate {
        page,
        annotations: vec![highlight.clone(), ink.clone()],
    })?;
    ensure!(w.is_dirty());
    ensure!(w.undo());
    ensure!(!w.is_dirty());
    ensure!(w.redo());
    let exporter = QpdfExporter::new(&root.join("sidecars"));
    let output = dir.path().join("annotated.pdf");
    let control = ExportControl::default();
    let plan = ExportPlan::capture(&w, 1, ExportScope::All)?;
    let prepared = prepare_export(plan, &output, &LocalOutput, &exporter, &control)?;
    let receipt = finish_export(prepared, true, &exporter, renderer, &control)?;
    ensure!(receipt.apply(1, &mut w));
    ensure!(!w.is_dirty());
    let reopened = open_document(renderer, &output, SourceId(2))?;
    ensure!(
        reopened.annotations(reopened.pages()[0].id) == [highlight.clone(), ink.clone()],
        "persisted editable annotations"
    );
    let original = renderer.render(&source, 0)?;
    let native = renderer.render(&output, 0)?;
    let editor = renderer.render_page(&RenderRequest {
        path: source.clone(),
        page_index: 0,
        target_width: 1200,
        rotation_delta: 0,
        fingerprint: None,
        annotations: Some(vec![highlight.clone(), ink.clone()]),
    })?;
    let difference: u64 = editor
        .rgba
        .iter()
        .zip(&native.rgba)
        .map(|(a, b)| u64::from(a.abs_diff(*b)))
        .sum();
    ensure!(
        difference as f64 / (native.rgba.len() as f64) < 0.75,
        "editor/export raster difference {}",
        difference as f64 / native.rgba.len() as f64
    );
    ensure!(
        original.rgba != native.rgba,
        "native PDFium must paint actual PDF annotations without editor overlays"
    );
    let hidden = renderer.render_page(&RenderRequest {
        path: output.clone(),
        page_index: 0,
        target_width: 1200,
        rotation_delta: 0,
        fingerprint: None,
        annotations: Some(vec![]),
    })?;
    ensure!(
        hidden.rgba == original.rgba,
        "deleting annotations restores base page exactly"
    );
    let native_again = renderer.render(&output, 0)?;
    ensure!(
        native_again.rgba == native.rgba,
        "temporary render flags restored"
    );
    let mut reopened = reopened;
    let p = reopened.pages()[0].id;
    reopened.execute(Command::Annotate {
        page: p,
        annotations: vec![],
    })?;
    reopened.select(reopened.pages().iter().map(|p| p.id).collect::<Vec<_>>());
    let plan = ExportPlan::capture(&reopened, 2, ExportScope::Selected)?;
    ensure!(
        !plan.is_whole_source_extract(),
        "copy optimization cannot lose deletion"
    );
    let clean = dir.path().join("removed.pdf");
    let prepared = prepare_export(plan, &clean, &LocalOutput, &exporter, &control)?;
    finish_export(prepared, true, &exporter, renderer, &control)?;
    let clean_w = open_document(renderer, &clean, SourceId(3))?;
    ensure!(clean_w.annotations(clean_w.pages()[0].id).is_empty());
    ensure!(
        renderer.render(&clean, 0)?.rgba == original.rgba,
        "removed after reopen/save"
    );
    for turn in 1..4 {
        let mut rotated = open_document(renderer, &output, SourceId(10 + u64::from(turn)))?;
        rotated.execute(Command::Rotate {
            ids: vec![rotated.pages()[0].id],
            turns: turn,
        })?;
        let out = dir.path().join(format!("rotated-{turn}.pdf"));
        let prepared = prepare_export(
            ExportPlan::capture(&rotated, 3, ExportScope::All)?,
            &out,
            &LocalOutput,
            &exporter,
            &control,
        )?;
        finish_export(prepared, true, &exporter, renderer, &control)?;
        let native_rotated = renderer.render(&out, 0)?;
        let editor_rotated = renderer.render_page(&RenderRequest {
            path: source.clone(),
            page_index: 0,
            target_width: 1200,
            rotation_delta: turn,
            fingerprint: None,
            annotations: Some(vec![highlight.clone(), ink.clone()]),
        })?;
        ensure!(
            native_rotated.width == editor_rotated.width
                && native_rotated.height == editor_rotated.height
        );
        let difference: u64 = native_rotated
            .rgba
            .iter()
            .zip(&editor_rotated.rgba)
            .map(|(a, b)| u64::from(a.abs_diff(*b)))
            .sum();
        ensure!(
            difference as f64 / (native_rotated.rgba.len() as f64) < 0.75,
            "rotated editor/export appearance differs for rotation {turn}"
        );
        let check = open_document(renderer, &out, SourceId(20))?;
        ensure!(check.annotations(check.pages()[0].id).len() == 2);
    }
    ensure!(std::fs::read(&source)? == bytes, "source read-only");
    println!(
        "PASS annotations: native appearance, persistence, reopen/delete, history/dirty, full Extract edits, rotations, render isolation, source bytes"
    );
    Ok(())
}
