use anyhow::{Result, ensure};
use pdf_application::{
    PageRenderer, PdfInspector, RenderRequest, SourceId, Workspace,
    export::{
        ExportControl, ExportError, ExportPlan, ExportScope, PdfExporter, finish_export,
        prepare_export,
    },
    open_document,
};
use pdf_domain::Command;
use pdf_engine::PdfiumRenderer;
use pdf_platform::LocalOutput;
use qpdf_adapter::QpdfExporter;
use std::path::Path;

struct Interrupt<'a> {
    inner: &'a QpdfExporter,
    cancel: bool,
}
impl PdfExporter for Interrupt<'_> {
    fn write(
        &self,
        plan: &ExportPlan,
        sources: &[pdf_application::SourceDocument],
        path: &Path,
        work: &Path,
        control: &ExportControl,
    ) -> Result<(), ExportError> {
        self.inner.write(plan, sources, path, work, control)?;
        if self.cancel {
            control.cancel();
            Ok(())
        } else {
            Err(ExportError::Engine)
        }
    }
    fn check(&self, path: &Path, control: &ExportControl) -> Result<(), ExportError> {
        self.inner.check(path, control)
    }
}
fn roundtrip(
    w: &mut Workspace,
    dir: &Path,
    exporter: &QpdfExporter,
    renderer: &PdfiumRenderer,
) -> Result<()> {
    let revision = w.revision();
    let dirty = w.is_dirty();
    let baseline = dir.join("reference.pdf");
    let output = dir.join("optimized.pdf");
    let c = ExportControl::default();
    let prepared = prepare_export(
        ExportPlan::capture(w, 41, ExportScope::All)?,
        &baseline,
        &LocalOutput,
        exporter,
        &c,
    )?;
    finish_export(prepared, false, exporter, renderer, &c)?;
    let plan = ExportPlan::capture(w, 41, ExportScope::Optimized)?;
    let c = ExportControl::default();
    let prepared = prepare_export(plan.clone(), &output, &LocalOutput, exporter, &c)?;
    let receipt = finish_export(prepared, false, exporter, renderer, &c)?;
    let sizes = receipt
        .optimization
        .ok_or_else(|| anyhow::anyhow!("Missing optimization sizes"))?;
    ensure!(
        sizes.baseline_bytes == std::fs::metadata(&baseline)?.len(),
        "Before size must describe the same current plan, not original source totals"
    );
    ensure!(
        sizes.output_bytes == std::fs::metadata(&output)?.len(),
        "After size matches published bytes"
    );
    ensure!(
        !receipt.apply(41, w) && w.revision() == revision && w.is_dirty() == dirty,
        "Optimize must not change saved baseline/history"
    );
    super::exporting::verify(&plan, &output, renderer)?;
    let before = renderer.inspect(&baseline)?;
    let after = renderer.inspect(&output)?;
    for index in 0..plan.pages().len() {
        let a = renderer.render_page(&RenderRequest {
            path: baseline.clone(),
            page_index: index as u16,
            target_width: 512,
            rotation_delta: 0,
            fingerprint: Some(before.fingerprint),
        })?;
        let b = renderer.render_page(&RenderRequest {
            path: output.clone(),
            page_index: index as u16,
            target_width: 512,
            rotation_delta: 0,
            fingerprint: Some(after.fingerprint),
        })?;
        ensure!(
            a.width == b.width && a.height == b.height && a.rgba == b.rgba,
            "Full raster must match the unoptimized page plan"
        );
    }
    ensure!(
        std::fs::read(&output)?
            .windows(b"/ObjStm".len())
            .any(|s| s == b"/ObjStm"),
        "Object-stream optimization applied"
    );
    ensure!(
        std::fs::read_dir(dir)?.count() == 2,
        "Only reference and final PDFs remain; staging/baseline artifacts cleaned"
    );
    Ok(())
}
pub fn run(root: &Path, renderer: &PdfiumRenderer) -> Result<()> {
    let dir = tempfile::tempdir_in(root.join("tests/fixtures"))?;
    let a = dir.path().join("sumber A & 日本.pdf");
    let b = dir.path().join("sumber B.pdf");
    let bytes_a = super::synthetic::selection_pdf();
    let bytes_b = super::synthetic::selection_pdf_named("SOURCE B");
    std::fs::write(&a, &bytes_a)?;
    std::fs::write(&b, &bytes_b)?;
    let exporter = QpdfExporter::new(&root.join("sidecars"));
    let mut w = open_document(renderer, &a, SourceId(8001))?;
    w.execute(Command::Import(
        open_document(renderer, &b, SourceId(8002))?
            .source()
            .clone(),
    ))?;
    let moved = vec![w.pages()[1].id, w.pages()[5].id];
    w.execute(Command::Move {
        ids: moved.clone(),
        target: 0,
    })?;
    w.execute(Command::Rotate {
        ids: moved,
        turns: 1,
    })?;
    w.execute(Command::Delete {
        ids: vec![w.pages()[7].id],
    })?;
    let output_dir = dir.path().join("multi-source");
    std::fs::create_dir(&output_dir)?;
    roundtrip(&mut w, &output_dir, &exporter, renderer)?;
    let plan = ExportPlan::capture(&w, 41, ExportScope::Optimized)?;

    for cancel in [false, true] {
        let output = dir.path().join(if cancel {
            "cancelled.pdf"
        } else {
            "failed.pdf"
        });
        let c = ExportControl::default();
        let prepared = prepare_export(plan.clone(), &output, &LocalOutput, &exporter, &c)?;
        let result = finish_export(
            prepared,
            false,
            &Interrupt {
                inner: &exporter,
                cancel,
            },
            renderer,
            &c,
        );
        ensure!(
            result.is_err() && !output.exists(),
            "Failure/cancel after writing must not publish staged files"
        );
    }
    let output = dir.path().join("raced.pdf");
    let c = ExportControl::default();
    let prepared = prepare_export(plan.clone(), &output, &LocalOutput, &exporter, &c)?;
    std::fs::write(&output, b"keep existing file")?;
    ensure!(
        matches!(
            finish_export(prepared, false, &exporter, renderer, &c),
            Err(ExportError::DestinationExists)
        ),
        "Destination race refused"
    );
    ensure!(
        std::fs::read(&output)? == b"keep existing file",
        "Existing destination unchanged"
    );

    for feature in ["info", "bookmark-info"] {
        let source = dir.path().join(format!("{feature}.pdf"));
        let bytes = super::synthetic::export_safety_pdf(feature);
        std::fs::write(&source, &bytes)?;
        let w = open_document(renderer, &source, SourceId(8003))?;
        let plan = ExportPlan::capture(&w, 41, ExportScope::Optimized)?;
        let output = dir.path().join(format!("{feature}-optimized.pdf"));
        let c = ExportControl::default();
        let prepared = prepare_export(plan.clone(), &output, &LocalOutput, &exporter, &c)?;
        ensure!(
            prepared.report.requires_acknowledgement(),
            "Loss consent retained for Optimize"
        );
        ensure!(
            matches!(
                finish_export(prepared, false, &exporter, renderer, &c),
                Err(ExportError::Acknowledgement)
            ),
            "Consent cannot be bypassed"
        );
        ensure!(!output.exists(), "Declined Optimize publishes nothing");
        let c = ExportControl::default();
        let prepared = prepare_export(plan.clone(), &output, &LocalOutput, &exporter, &c)?;
        let receipt = finish_export(prepared, true, &exporter, renderer, &c)?;
        ensure!(
            receipt.optimization.is_some(),
            "Consented Optimize reports sizes"
        );
        super::exporting::verify(&plan, &output, renderer)?;
        ensure!(
            std::fs::read(&source)? == bytes,
            "Metadata/bookmark source preserved"
        );
    }
    for feature in [
        "form",
        "signature",
        "tagged",
        "annotation",
        "javascript",
        "unknown",
    ] {
        let source = dir.path().join(format!("blocked-{feature}.pdf"));
        std::fs::write(&source, super::synthetic::export_safety_pdf(feature))?;
        let w = open_document(renderer, &source, SourceId(8004))?;
        let plan = ExportPlan::capture(&w, 41, ExportScope::Optimized)?;
        let output = dir.path().join(format!("blocked-{feature}-optimized.pdf"));
        ensure!(
            prepare_export(
                plan,
                &output,
                &LocalOutput,
                &exporter,
                &ExportControl::default()
            )
            .is_err(),
            "Conservative Optimize preflight: {feature}"
        );
        ensure!(!output.exists(), "Blocked Optimize publishes nothing");
    }
    for (name, bytes) in [
        ("blank", super::synthetic::blank_pdf()),
        ("scan", super::synthetic::scanned_book(2)),
        ("jpeg", super::synthetic::jpeg_pdf()),
    ] {
        let source = dir.path().join(format!("{name}.pdf"));
        std::fs::write(&source, &bytes)?;
        let mut w = open_document(renderer, &source, SourceId(8005))?;
        let out = dir.path().join(name);
        std::fs::create_dir(&out)?;
        roundtrip(&mut w, &out, &exporter, renderer)?;
        if name == "jpeg" {
            let jpeg = include_bytes!("../../../tests/fixtures/optimize-photo.jpg");
            ensure!(
                std::fs::read(out.join("optimized.pdf"))?
                    .windows(jpeg.len())
                    .any(|s| s == jpeg),
                "Original JPEG encoded bytes must be retained without re-encoding"
            );
        }
        ensure!(std::fs::read(&source)? == bytes, "Image source preserved");
    }
    let output = dir.path().join("changed-source.pdf");
    let c = ExportControl::default();
    let prepared = prepare_export(plan, &output, &LocalOutput, &exporter, &c)?;
    let mut changed = bytes_a.clone();
    changed.push(b'\n');
    std::fs::write(&a, changed)?;
    ensure!(
        matches!(
            finish_export(prepared, false, &exporter, renderer, &c),
            Err(ExportError::SourceChanged)
        ),
        "Changed originals rejected before publication"
    );
    ensure!(!output.exists(), "Changed-source output absent");
    std::fs::write(&a, &bytes_a)?;
    ensure!(
        std::fs::read(&a)? == bytes_a && std::fs::read(&b)? == bytes_b,
        "Original bytes unchanged"
    );
    ensure!(
        !std::fs::read_dir(dir.path())?
            .filter_map(Result::ok)
            .any(|e| e
                .file_name()
                .to_string_lossy()
                .starts_with(".pdf-studio-export-")),
        "Owned staging and baselines cleaned"
    );
    println!(
        "PASS Optimize: current multi-source plan, exact baseline sizes, full-raster/text preservation, original JPEG bytes, scan, consent/blockers, dirty baseline, cancel/failure/race/source checks and cleanup"
    );
    Ok(())
}
