//! Round-trip checks use only generated PDFs and production export transactions.
use anyhow::{Result, ensure};
use pdf_application::{
    PageRenderer, RenderRequest, SourceId,
    export::{ExportControl, ExportPlan, ExportScope, finish_export, prepare_export},
    open_document,
};
use pdf_domain::{
    Command,
    annotation::{Annotation, MAX_SIGNATURES, SignatureImage},
};
use pdf_engine::PdfiumRenderer;
use pdf_platform::LocalOutput;
use qpdf_adapter::QpdfExporter;
use std::path::Path;
pub fn run(root: &Path, renderer: &PdfiumRenderer) -> Result<()> {
    for feature in ["plain", "cropped-signature"] {
        run_fixture(root, renderer, feature)?;
    }
    run_signature_limit_fixture(root, renderer)?;
    Ok(())
}

fn run_signature_limit_fixture(root: &Path, renderer: &PdfiumRenderer) -> Result<()> {
    let dir = tempfile::tempdir_in(root.join(".research"))?;
    let source = dir.path().join("source.pdf");
    let bytes = crate::synthetic::export_safety_pdf("plain");
    std::fs::write(&source, &bytes)?;
    let mut workspace = open_document(renderer, &source, SourceId(1))?;
    let page = workspace.pages()[0].id;
    let mut state = 1u32;
    let image = std::sync::Arc::new(SignatureImage {
        width: SignatureImage::MAX_SIDE,
        height: SignatureImage::MAX_PIXELS / SignatureImage::MAX_SIDE,
        rgba: (0..SignatureImage::MAX_PIXELS * 4)
            .map(|_| {
                state ^= state << 13;
                state ^= state >> 17;
                state ^= state << 5;
                state as u8
            })
            .collect(),
    });
    let mut signatures: Vec<_> = (1..=MAX_SIGNATURES)
        .map(|id| Annotation {
            id: id as u64,
            highlight: false,
            color: [0; 3],
            width: 1.,
            points: vec![[0.15, 0.75], [0.65, 0.9]],
            rectangles: false,
            signature: Some(image.clone()),
        })
        .collect();
    workspace.execute(Command::Annotate {
        page,
        annotations: signatures.clone(),
    })?;
    let exporter = QpdfExporter::new(&root.join("sidecars"));
    let control = ExportControl::default();
    let output = dir.path().join("large-signatures.pdf");
    let prepared = prepare_export(
        ExportPlan::capture(&workspace, 1, ExportScope::All)?,
        &output,
        &LocalOutput,
        &exporter,
        &control,
    )?;
    finish_export(prepared, true, &exporter, renderer, &control)?;
    let mut reopened = open_document(renderer, &output, SourceId(2))?;
    let reopened_page = reopened.pages()[0].id;
    ensure!(reopened.annotations(reopened_page) == signatures);
    signatures.pop();
    reopened.execute(Command::Annotate {
        page: reopened_page,
        annotations: signatures.clone(),
    })?;
    let edited = dir.path().join("large-signatures-edited.pdf");
    let prepared = prepare_export(
        ExportPlan::capture(&reopened, 2, ExportScope::All)?,
        &edited,
        &LocalOutput,
        &exporter,
        &control,
    )?;
    finish_export(prepared, true, &exporter, renderer, &control)?;
    let check = open_document(renderer, &edited, SourceId(3))?;
    ensure!(check.annotations(check.pages()[0].id) == signatures);
    ensure!(std::fs::read(&source)? == bytes, "source read-only");
    println!("PASS annotations: eight full-size high-entropy signatures save, reopen and edit");
    Ok(())
}

fn run_fixture(root: &Path, renderer: &PdfiumRenderer, feature: &str) -> Result<()> {
    let dir = tempfile::tempdir_in(root.join(".research"))?;
    let source = dir.path().join("source.pdf");
    let bytes = crate::synthetic::export_safety_pdf(feature);
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
        signature: None,
    };
    let ink = Annotation {
        id: 2,
        highlight: false,
        color: [220, 30, 30],
        width: 4.,
        points: vec![[0.2, 0.3], [0.6, 0.6], [0.3, 0.7]],
        rectangles: false,
        signature: None,
    };
    let signature = Annotation {
        id: 3,
        highlight: false,
        color: [0, 0, 0],
        width: 1.,
        rectangles: false,
        points: vec![[0.15, 0.75], [0.65, 0.9]],
        signature: Some(std::sync::Arc::new(
            pdf_domain::annotation::SignatureImage {
                width: 64,
                height: 24,
                rgba: (0..64 * 24)
                    .flat_map(|i| {
                        [
                            20,
                            50,
                            180,
                            if i % 64 > i / 64 && i % 64 < i / 64 + 20 {
                                180
                            } else {
                                0
                            },
                        ]
                    })
                    .collect(),
            },
        )),
    };
    w.execute(Command::Annotate {
        page,
        annotations: vec![highlight.clone(), ink.clone(), signature.clone()],
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
        reopened.annotations(reopened.pages()[0].id)
            == [highlight.clone(), ink.clone(), signature.clone()],
        "persisted editable annotations"
    );
    use pdf_application::{
        PdfInspector,
        text::{TextExtractor, TextRequest},
    };
    let info = renderer.inspect(&output)?;
    let text = renderer.extract_text(&TextRequest {
        path: output.clone(),
        page_index: 0,
        fingerprint: info.fingerprint,
    })?;
    ensure!(
        text.copy_range(0..text.chars.len()).contains("SAFE"),
        "signature preserves searchable source text"
    );
    let original = renderer.render(&source, 0)?;
    let native = renderer.render(&output, 0)?;
    let editor = renderer.render_page(&RenderRequest {
        path: source.clone(),
        page_index: 0,
        target_width: 1200,
        rotation_delta: 0,
        fingerprint: None,
        annotations: Some(vec![highlight.clone(), ink.clone(), signature.clone()]),
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
    // All downstream exporters must capture signature assets with the same snapshot.
    let optimized = dir.path().join("optimized.pdf");
    let prepared = prepare_export(
        ExportPlan::capture(&w, 4, ExportScope::Optimized)?,
        &optimized,
        &LocalOutput,
        &exporter,
        &control,
    )?;
    finish_export(prepared, true, &exporter, renderer, &control)?;
    ensure!(
        renderer.render(&optimized, 0)?.rgba == native.rgba,
        "Optimize preserves signature alpha and placement"
    );
    use pdf_application::split::{SplitPlan, SplitSpec, finish_split, prepare_split};
    let split = dir.path().join("split-signature");
    let prepared = prepare_split(
        SplitPlan::new(
            ExportPlan::capture(&w, 5, ExportScope::All)?,
            SplitSpec::Every(1),
        )?,
        &split,
        &pdf_platform::split::LocalSplitOutput,
        &exporter,
        &control,
    )?;
    finish_split(prepared, true, &exporter, renderer, &control, |_| {})?;
    let part = std::fs::read_dir(&split)?
        .next()
        .ok_or_else(|| anyhow::anyhow!("split empty"))??;
    ensure!(
        renderer.render(&part.path(), 0)?.rgba == native.rgba,
        "Split preserves signature appearance"
    );
    use pdf_application::png::{PngDpi, PngPlan, finish_png, prepare_png};
    let png = dir.path().join("png-signature");
    let prepared = prepare_png(
        PngPlan::new(
            ExportPlan::capture(&w, 6, ExportScope::All)?,
            PngDpi::Dpi150,
        )?,
        &png,
        &pdf_platform::png::LocalPngOutput,
        &exporter,
        &control,
    )?;
    finish_png(
        prepared,
        renderer,
        &pdf_platform::png::LocalPngWriter,
        &control,
        |_| {},
    )?;
    let image = std::fs::read_dir(&png)?
        .next()
        .ok_or_else(|| anyhow::anyhow!("PNG empty"))??;
    let decoded = image::open(image.path())?.to_rgba8();
    use pdf_application::{ExactPageRenderer, ExactRenderRequest};
    let expected = renderer.render_exact(&ExactRenderRequest {
        path: source.clone(),
        page_index: 0,
        width: decoded.width(),
        height: decoded.height(),
        fingerprint: w.source().fingerprint,
        rotation_delta: 0,
        annotations: Some(vec![highlight.clone(), ink.clone(), signature.clone()]),
    })?;
    ensure!(
        decoded.as_raw() == &expected.rgba,
        "PNG captures signature pixels"
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
        let editor_rotated = renderer.render_exact(&ExactRenderRequest {
            path: source.clone(),
            page_index: 0,
            width: native_rotated.width,
            height: native_rotated.height,
            rotation_delta: turn,
            fingerprint: w.source().fingerprint,
            annotations: Some(vec![highlight.clone(), ink.clone(), signature.clone()]),
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
        ensure!(check.annotations(check.pages()[0].id).len() == 3);
    }
    ensure!(std::fs::read(&source)? == bytes, "source read-only");
    println!(
        "PASS annotations: native appearance, persistence, reopen/delete, history/dirty, full Extract edits, rotations, render isolation, source bytes"
    );
    Ok(())
}
