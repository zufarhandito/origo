//! Round-trip checks use only generated PDFs and production export transactions.
use anyhow::{Result, ensure};
use pdf_application::{
    PageRenderer, RenderRequest, SourceId,
    export::{ExportControl, ExportPlan, ExportScope, finish_export, prepare_export},
    open_document,
};
use pdf_domain::{
    Command,
    annotation::{Annotation, MAX_SIGNATURES, SignatureImage, TextAnnotation},
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
    run_text_annotation_fixture(root, renderer)?;
    run_multipage_stamp_fixture(root, renderer)?;
    run_extra_annotations_fixture(root, renderer)?;
    Ok(())
}

fn qpdf_json(root: &Path, pdf: &Path) -> Result<serde_json::Value> {
    let qpdf = root
        .join("sidecars")
        .join("qpdf")
        .join("bin")
        .join("qpdf.exe");
    let output = std::process::Command::new(qpdf)
        .args(["--json=2", "--json-stream-data=none"])
        .arg(pdf)
        .output()?;
    ensure!(
        output.status.success(),
        "qpdf json failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    Ok(serde_json::from_slice(&output.stdout)?)
}

fn run_text_annotation_fixture(root: &Path, renderer: &PdfiumRenderer) -> Result<()> {
    let dir = tempfile::tempdir_in(root.join(".research"))?;
    let source = dir.path().join("source.pdf");
    let bytes = crate::synthetic::export_safety_pdf("plain");
    std::fs::write(&source, &bytes)?;
    let mut w = open_document(renderer, &source, SourceId(1))?;
    let page = w.pages()[0].id;
    let date_note = Annotation {
        id: 1,
        highlight: false,
        color: [25, 25, 25],
        width: 1.,
        points: vec![[0.1, 0.15], [0.42, 0.20]],
        rectangles: false,
        signature: None,
        text: Some(TextAnnotation {
            content: "24 November 2026".into(),
            font_size: 14.,
        }),
        extra: None,
    };
    let name_note = Annotation {
        id: 2,
        highlight: false,
        color: [30, 80, 180],
        width: 1.,
        points: vec![[0.1, 0.25], [0.38, 0.29]],
        rectangles: false,
        signature: None,
        text: Some(TextAnnotation {
            content: "Budi Santoso".into(),
            font_size: 12.,
        }),
        extra: None,
    };
    w.execute(Command::Annotate {
        page,
        annotations: vec![date_note.clone(), name_note.clone()],
    })?;
    ensure!(w.is_dirty());
    ensure!(w.undo() && !w.is_dirty());
    ensure!(w.redo() && w.is_dirty());

    let exporter = QpdfExporter::new(&root.join("sidecars"));
    let control = ExportControl::default();
    let output = dir.path().join("text-annotations.pdf");
    let prepared = prepare_export(
        ExportPlan::capture(&w, 1, ExportScope::All)?,
        &output,
        &LocalOutput,
        &exporter,
        &control,
    )?;
    finish_export(prepared, true, &exporter, renderer, &control)?;

    // Verify passive /FreeText structure and absence of AcroForm fields.
    let json = qpdf_json(root, &output)?;
    ensure!(
        json.pointer("/acroform/hasacroform")
            .and_then(serde_json::Value::as_bool)
            == Some(false),
        "passive text annotations must not create AcroForm"
    );
    let objects = json
        .pointer("/qpdf/1")
        .and_then(serde_json::Value::as_object)
        .ok_or_else(|| anyhow::anyhow!("qpdf objects missing"))?;
    let freetext_contents: Vec<_> = objects
        .values()
        .filter_map(|o| o.get("value").and_then(serde_json::Value::as_object))
        .filter(|d| d.get("/Subtype").and_then(serde_json::Value::as_str) == Some("/FreeText"))
        .filter_map(|d| d.get("/Contents").and_then(serde_json::Value::as_str))
        .collect();
    ensure!(
        freetext_contents.len() == 2
            && freetext_contents.contains(&"u:24 November 2026")
            && freetext_contents.contains(&"u:Budi Santoso"),
        "expected passive /FreeText annotations with /Contents, got {freetext_contents:?}"
    );

    let mut reopened = open_document(renderer, &output, SourceId(2))?;
    let reopened_page = reopened.pages()[0].id;
    ensure!(
        reopened.annotations(reopened_page) == [date_note.clone(), name_note.clone()],
        "reopened passive text annotations must match"
    );

    let original = renderer.render(&source, 0)?;
    let native = renderer.render(&output, 0)?;
    ensure!(
        original.rgba != native.rgba,
        "native text annotation must paint"
    );
    let editor = renderer.render_page(&RenderRequest {
        path: source.clone(),
        page_index: 0,
        target_width: 1200,
        rotation_delta: 0,
        fingerprint: None,
        annotations: Some(vec![date_note.clone(), name_note.clone()]),
    })?;
    let diff: u64 = editor
        .rgba
        .iter()
        .zip(&native.rgba)
        .map(|(a, b)| u64::from(a.abs_diff(*b)))
        .sum();
    ensure!(
        diff as f64 / (native.rgba.len() as f64) < 0.75,
        "text annotation editor/export diff {}",
        diff as f64 / (native.rgba.len() as f64)
    );

    // Check rotated parity across turns 1..4.
    use pdf_application::{ExactPageRenderer, ExactRenderRequest};
    for turn in 1..4 {
        let mut rotated = open_document(renderer, &output, SourceId(30 + u64::from(turn)))?;
        rotated.execute(Command::Rotate {
            ids: vec![rotated.pages()[0].id],
            turns: turn,
        })?;
        let out = dir.path().join(format!("text-rotated-{turn}.pdf"));
        let prepared = prepare_export(
            ExportPlan::capture(&rotated, 2, ExportScope::All)?,
            &out,
            &LocalOutput,
            &exporter,
            &control,
        )?;
        finish_export(prepared, true, &exporter, renderer, &control)?;
        let native_rot = renderer.render(&out, 0)?;
        let editor_rot = renderer.render_exact(&ExactRenderRequest {
            path: source.clone(),
            page_index: 0,
            width: native_rot.width,
            height: native_rot.height,
            rotation_delta: turn,
            fingerprint: w.source().fingerprint,
            annotations: Some(vec![date_note.clone(), name_note.clone()]),
        })?;
        let rot_diff: u64 = native_rot
            .rgba
            .iter()
            .zip(&editor_rot.rgba)
            .map(|(a, b)| u64::from(a.abs_diff(*b)))
            .sum();
        ensure!(
            rot_diff as f64 / (native_rot.rgba.len() as f64) < 0.75,
            "rotated text diff {} for turn {turn}",
            rot_diff as f64 / (native_rot.rgba.len() as f64)
        );
    }

    // Re-edit after reopen, save, then delete and verify base page restores cleanly.
    let edited_note = Annotation {
        id: 1,
        highlight: false,
        color: [180, 35, 45],
        width: 1.,
        points: vec![[0.12, 0.16], [0.48, 0.23]],
        rectangles: false,
        signature: None,
        text: Some(TextAnnotation {
            content: "25 November 2026".into(),
            font_size: 18.,
        }),
        extra: None,
    };
    reopened.execute(Command::Annotate {
        page: reopened_page,
        annotations: vec![edited_note.clone()],
    })?;
    let edited_path = dir.path().join("text-edited.pdf");
    let prepared = prepare_export(
        ExportPlan::capture(&reopened, 3, ExportScope::All)?,
        &edited_path,
        &LocalOutput,
        &exporter,
        &control,
    )?;
    finish_export(prepared, true, &exporter, renderer, &control)?;
    let mut reopened2 = open_document(renderer, &edited_path, SourceId(3))?;
    let p2 = reopened2.pages()[0].id;
    ensure!(reopened2.annotations(p2) == [edited_note]);
    reopened2.execute(Command::Annotate {
        page: p2,
        annotations: vec![],
    })?;
    let clean = dir.path().join("text-removed.pdf");
    let prepared = prepare_export(
        ExportPlan::capture(&reopened2, 4, ExportScope::All)?,
        &clean,
        &LocalOutput,
        &exporter,
        &control,
    )?;
    finish_export(prepared, true, &exporter, renderer, &control)?;
    ensure!(
        renderer.render(&clean, 0)?.rgba == original.rgba,
        "deleting text annotations restores base page"
    );
    ensure!(std::fs::read(&source)? == bytes, "source read-only");
    println!(
        "PASS annotations: passive /FreeText save, reopen, edit, delete, rotation parity, no AcroForm"
    );
    Ok(())
}

fn run_extra_annotations_fixture(root: &Path, renderer: &PdfiumRenderer) -> Result<()> {
    use pdf_domain::annotation::{AnnotationExtra, FillSymbol, ShapeKind};
    let dir = tempfile::tempdir_in(root.join(".research"))?;
    let source = dir.path().join("source.pdf");
    let bytes = crate::synthetic::export_safety_pdf("plain");
    std::fs::write(&source, &bytes)?;
    let mut w = open_document(renderer, &source, SourceId(1))?;
    let page = w.pages()[0].id;
    let items = vec![
        Annotation {
            id: 1,
            highlight: false,
            color: [40, 110, 220],
            width: 1.5,
            points: vec![[0.1, 0.12], [0.5, 0.15]],
            rectangles: true,
            signature: None,
            text: None,
            extra: Some(AnnotationExtra::Underline),
        },
        Annotation {
            id: 2,
            highlight: false,
            color: [230, 65, 75],
            width: 1.5,
            points: vec![[0.1, 0.18], [0.5, 0.21]],
            rectangles: true,
            signature: None,
            text: None,
            extra: Some(AnnotationExtra::StrikeOut),
        },
        Annotation {
            id: 3,
            highlight: false,
            color: [230, 65, 75],
            width: 2.5,
            points: vec![[0.1, 0.28], [0.45, 0.42]],
            rectangles: false,
            signature: None,
            text: None,
            extra: Some(AnnotationExtra::Shape(ShapeKind::Rectangle)),
        },
        Annotation {
            id: 4,
            highlight: false,
            color: [30, 170, 95],
            width: 2.5,
            points: vec![[0.52, 0.28], [0.85, 0.42]],
            rectangles: false,
            signature: None,
            text: None,
            extra: Some(AnnotationExtra::Shape(ShapeKind::Ellipse)),
        },
        Annotation {
            id: 5,
            highlight: false,
            color: [40, 110, 220],
            width: 2.5,
            points: vec![[0.1, 0.50], [0.45, 0.62]],
            rectangles: false,
            signature: None,
            text: None,
            extra: Some(AnnotationExtra::Shape(ShapeKind::Arrow)),
        },
        Annotation {
            id: 6,
            highlight: false,
            color: [255, 210, 0],
            width: 1.0,
            points: vec![[0.55, 0.52], [0.60, 0.56]],
            rectangles: false,
            signature: None,
            text: None,
            extra: Some(AnnotationExtra::Note("Cek pasal 4 ayat 2".into())),
        },
        Annotation {
            id: 7,
            highlight: false,
            color: [25, 25, 25],
            width: 2.0,
            points: vec![[0.70, 0.52], [0.74, 0.55]],
            rectangles: false,
            signature: None,
            text: None,
            extra: Some(AnnotationExtra::Symbol(FillSymbol::Check)),
        },
    ];
    w.execute(Command::Annotate {
        page,
        annotations: items.clone(),
    })?;
    let exporter = QpdfExporter::new(&root.join("sidecars"));
    let control = ExportControl::default();
    let output = dir.path().join("extra-annotations.pdf");
    let prepared = prepare_export(
        ExportPlan::capture(&w, 1, ExportScope::All)?,
        &output,
        &LocalOutput,
        &exporter,
        &control,
    )?;
    finish_export(prepared, true, &exporter, renderer, &control)?;
    let reopened = open_document(renderer, &output, SourceId(2))?;
    ensure!(
        reopened.annotations(reopened.pages()[0].id) == items,
        "extra annotations must roundtrip cleanly"
    );
    let original = renderer.render(&source, 0)?;
    let native = renderer.render(&output, 0)?;
    ensure!(
        original.rgba != native.rgba,
        "extra annotations must paint visibly in PDFium"
    );
    println!("PASS annotations: underline, strikeout, shapes, sticky note, fill symbol roundtrip");
    Ok(())
}

fn run_multipage_stamp_fixture(root: &Path, renderer: &PdfiumRenderer) -> Result<()> {
    let dir = tempfile::tempdir_in(root.join(".research"))?;
    let source = dir.path().join("twelve-pages.pdf");
    let bytes = crate::synthetic::pdf(12, false);
    std::fs::write(&source, &bytes)?;
    let mut w = open_document(renderer, &source, SourceId(1))?;
    ensure!(w.pages().len() == 12);

    let stamp_image = std::sync::Arc::new(SignatureImage {
        width: 48,
        height: 20,
        rgba: (0..48 * 20)
            .flat_map(|i| {
                [
                    180,
                    35,
                    45,
                    if (i % 48) % 3 == 0 || (i / 48) % 3 == 0 {
                        220
                    } else {
                        0
                    },
                ]
            })
            .collect(),
    });
    let first_page = w.pages()[0].id;
    let first_stamp = Annotation {
        id: w.allocate_annotation_id(),
        highlight: false,
        color: [0; 3],
        width: 1.,
        points: vec![[0.65, 0.80], [0.90, 0.90]],
        rectangles: false,
        signature: Some(stamp_image.clone()),
        text: None,
        extra: None,
    };
    w.execute(Command::Annotate {
        page: first_page,
        annotations: vec![first_stamp.clone()],
    })?;

    // Apply to remaining 11 pages in 1 atomic Undo/Redo step (12 total placements > old limit 8).
    let target_pages: Vec<_> = w.pages()[1..].iter().map(|p| p.id).collect();
    let mut changes = Vec::with_capacity(target_pages.len());
    for page_id in &target_pages {
        let mut copy = first_stamp.clone();
        copy.id = w.allocate_annotation_id();
        changes.push((*page_id, vec![copy]));
    }
    w.execute(Command::AnnotateMany(changes))?;
    ensure!(w.pages().iter().all(|p| w.annotations(p.id).len() == 1));
    // 1 Undo must revert all 11 applied pages at once.
    ensure!(w.undo());
    ensure!(w.annotations(first_page).len() == 1);
    ensure!(target_pages.iter().all(|p| w.annotations(*p).is_empty()));
    // 1 Redo must restore all 11 applied pages at once.
    ensure!(w.redo());
    ensure!(w.pages().iter().all(|p| w.annotations(p.id).len() == 1));

    let exporter = QpdfExporter::new(&root.join("sidecars"));
    let control = ExportControl::default();
    let output = dir.path().join("stamped-12.pdf");
    let prepared = prepare_export(
        ExportPlan::capture(&w, 1, ExportScope::All)?,
        &output,
        &LocalOutput,
        &exporter,
        &control,
    )?;
    finish_export(prepared, true, &exporter, renderer, &control)?;

    // Inspect PDF objects to verify XObject and /NM deduplication.
    let json = qpdf_json(root, &output)?;
    let objects = json
        .pointer("/qpdf/1")
        .and_then(serde_json::Value::as_object)
        .ok_or_else(|| anyhow::anyhow!("qpdf objects missing"))?;
    let image_streams = objects
        .values()
        .filter_map(|o| {
            o.pointer("/stream/dict")
                .and_then(serde_json::Value::as_object)
        })
        .filter(|d| d.get("/Subtype").and_then(serde_json::Value::as_str) == Some("/Image"))
        .count();
    ensure!(
        image_streams == 2,
        "expected 1 RGB + 1 SMask deduplicated image stream across 12 pages, got {image_streams}"
    );
    let ref_names = objects
        .values()
        .filter_map(|o| o.get("value").and_then(serde_json::Value::as_object))
        .filter(|d| d.get("/Subtype").and_then(serde_json::Value::as_str) == Some("/Stamp"))
        .filter_map(|d| d.get("/NM").and_then(serde_json::Value::as_str))
        .filter(|nm| nm.contains("\"ref:"))
        .count();
    ensure!(
        ref_names == 11,
        "expected 11 deduplicated ref:<id> /NM entries, got {ref_names}"
    );

    // Reopen and verify all 12 pages share the same Arc<SignatureImage> in RAM.
    let reopened = open_document(renderer, &output, SourceId(2))?;
    ensure!(reopened.pages().len() == 12);
    let first_arc = reopened.annotations(reopened.pages()[0].id)[0]
        .signature
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("missing reopened stamp"))?;
    ensure!(first_arc.as_ref() == stamp_image.as_ref());
    for page in reopened.pages() {
        let annots = reopened.annotations(page.id);
        ensure!(annots.len() == 1);
        let sig = annots[0]
            .signature
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("missing stamp on page"))?;
        ensure!(
            std::sync::Arc::ptr_eq(first_arc, sig),
            "reopened multi-page stamps should share Arc<SignatureImage> in RAM"
        );
    }

    // Split Every(1) must keep every split page self-contained and render identically.
    use pdf_application::split::{SplitPlan, SplitSpec, finish_split, prepare_split};
    let split_dir = dir.path().join("split-12");
    let prepared = prepare_split(
        SplitPlan::new(
            ExportPlan::capture(&reopened, 2, ExportScope::All)?,
            SplitSpec::Every(1),
        )?,
        &split_dir,
        &pdf_platform::split::LocalSplitOutput,
        &exporter,
        &control,
    )?;
    finish_split(prepared, true, &exporter, renderer, &control, |_| {})?;
    let mut split_files: Vec<_> = std::fs::read_dir(&split_dir)?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .collect();
    split_files.sort();
    ensure!(split_files.len() == 12);
    for (index, part_path) in split_files.iter().enumerate() {
        let part_doc = open_document(renderer, part_path, SourceId(100 + index as u64))?;
        ensure!(
            part_doc.annotations(part_doc.pages()[0].id).len() == 1,
            "split part {} lost self-contained stamp",
            index + 1
        );
        ensure!(
            renderer.render(part_path, 0)?.rgba == renderer.render(&output, index as u16)?.rgba,
            "split part {} appearance differs from source page",
            index + 1
        );
    }

    ensure!(std::fs::read(&source)? == bytes, "source read-only");
    println!(
        "PASS annotations: 12-page atomic stamp apply, 1-step Undo/Redo, XObject+NM dedup, shared Arc, self-contained Split"
    );
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
            text: None,
            extra: None,
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
        text: None,
        extra: None,
    };
    let ink = Annotation {
        id: 2,
        highlight: false,
        color: [220, 30, 30],
        width: 4.,
        points: vec![[0.2, 0.3], [0.6, 0.6], [0.3, 0.7]],
        rectangles: false,
        signature: None,
        text: None,
        extra: None,
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
        text: None,
        extra: None,
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
