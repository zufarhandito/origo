use anyhow::{Context, Result, ensure};
use pdf_application::{
    SourceId,
    export::{ExportControl, ExportError, ExportPlan, ExportScope, PdfExporter},
    open_document,
    split::{SplitError, SplitPlan, SplitSpec, finish_split, prepare_split},
};
use pdf_domain::Command;
use pdf_engine::PdfiumRenderer;
use pdf_platform::split::LocalSplitOutput;
use qpdf_adapter::QpdfExporter;
use std::{
    path::Path,
    sync::atomic::{AtomicUsize, Ordering},
};

struct FailSecond<'a> {
    inner: &'a QpdfExporter,
    count: AtomicUsize,
}
impl PdfExporter for FailSecond<'_> {
    fn write(
        &self,
        plan: &ExportPlan,
        sources: &[pdf_application::SourceDocument],
        path: &Path,
        work: &Path,
        control: &ExportControl,
    ) -> Result<(), ExportError> {
        if self.count.fetch_add(1, Ordering::SeqCst) == 1 {
            return Err(ExportError::Engine);
        }
        self.inner.write(plan, sources, path, work, control)
    }
    fn check(&self, path: &Path, control: &ExportControl) -> Result<(), ExportError> {
        self.inner.check(path, control)
    }
}
pub fn run(root: &Path, renderer: &PdfiumRenderer) -> Result<()> {
    let dir = tempfile::tempdir_in(root.join("tests/fixtures"))?;
    let a = dir.path().join("sumber A_日本.pdf");
    let b = dir.path().join("sumber B & test.pdf");
    let bytes_a = super::synthetic::selection_pdf();
    let bytes_b = super::synthetic::selection_pdf_named("SOURCE B");
    std::fs::write(&a, &bytes_a)?;
    std::fs::write(&b, &bytes_b)?;
    let mut w = open_document(renderer, &a, SourceId(7001))?;
    let source = open_document(renderer, &b, SourceId(7002))?
        .source()
        .clone();
    w.execute(Command::Import(source))?;
    let ids = vec![w.pages()[1].id, w.pages()[5].id];
    w.execute(Command::Move {
        ids: ids.clone(),
        target: 0,
    })?;
    w.execute(Command::Rotate { ids, turns: 1 })?;
    let revision = w.revision();
    let full = ExportPlan::capture(&w, 5, ExportScope::All)?;
    let plan = SplitPlan::new(full.clone(), SplitSpec::Every(3))?;
    let exporter = QpdfExporter::new(&root.join("sidecars"));
    let output = dir.path().join("hasil batch_日本");
    let control = ExportControl::default();
    let prepared = prepare_split(
        plan.clone(),
        &output,
        &LocalSplitOutput,
        &exporter,
        &control,
    )?;
    ensure!(!output.exists(), "No folder before commit");
    let mut completed = Vec::new();
    let receipt = finish_split(prepared, false, &exporter, renderer, &control, |n| {
        completed.push(n)
    })
    .context("Initial multi-source split publication")?;
    ensure!(
        receipt.files == 3 && receipt.pages == 8 && completed == [1, 2, 3],
        "Batch counts/progress"
    );
    for (index, part) in plan.parts().iter().enumerate() {
        super::exporting::verify(part, &output.join(SplitPlan::filename(index)), renderer)?;
    }
    ensure!(
        std::fs::read_dir(&output)?.count() == 3,
        "Only PDFs published; no snapshots/work files"
    );
    ensure!(
        w.is_dirty() && w.revision() == revision,
        "Split must not advance saved baseline"
    );
    ensure!(
        matches!(
            prepare_split(
                plan.clone(),
                &output,
                &LocalSplitOutput,
                &exporter,
                &ExportControl::default()
            ),
            Err(SplitError::Exists)
        ),
        "Existing folder refused"
    );

    let manual = SplitPlan::new(full, SplitSpec::Ranges("1-2,4; 6-8"))?;
    let output = dir.path().join("manual");
    let control = ExportControl::default();
    let prepared = prepare_split(
        manual.clone(),
        &output,
        &LocalSplitOutput,
        &exporter,
        &control,
    )?;
    finish_split(prepared, false, &exporter, renderer, &control, |_| {})?;
    for (index, part) in manual.parts().iter().enumerate() {
        super::exporting::verify(part, &output.join(SplitPlan::filename(index)), renderer)?;
    }

    // Mid-batch failures/cancellation remove even successfully staged earlier parts.
    for cancel in [false, true] {
        let output = dir.path().join(if cancel { "cancelled" } else { "failed" });
        let control = ExportControl::default();
        let prepared = prepare_split(
            plan.clone(),
            &output,
            &LocalSplitOutput,
            &exporter,
            &control,
        )?;
        let result = if cancel {
            finish_split(prepared, false, &exporter, renderer, &control, |_| {
                control.cancel()
            })
        } else {
            finish_split(
                prepared,
                false,
                &FailSecond {
                    inner: &exporter,
                    count: AtomicUsize::new(0),
                },
                renderer,
                &control,
                |_| {},
            )
        };
        ensure!(
            result.is_err() && !output.exists(),
            "Failed/cancelled folder must not be published"
        );
    }
    // Race: someone creates an empty destination after staging. It must survive.
    let output = dir.path().join("raced");
    let control = ExportControl::default();
    let prepared = prepare_split(
        plan.clone(),
        &output,
        &LocalSplitOutput,
        &exporter,
        &control,
    )?;
    std::fs::create_dir(&output)?;
    ensure!(
        matches!(
            finish_split(prepared, false, &exporter, renderer, &control, |_| {}),
            Err(SplitError::Exists)
        ),
        "Destination race refused"
    );
    ensure!(
        std::fs::read_dir(&output)?.count() == 0,
        "Existing empty directory not replaced"
    );

    let tagged = dir.path().join("tagged.pdf");
    std::fs::write(&tagged, super::synthetic::tagged_ink_pdf())?;
    let tagged_w = open_document(renderer, &tagged, SourceId(7003))?;
    let tagged_plan = SplitPlan::new(
        ExportPlan::capture(&tagged_w, 5, ExportScope::All)?,
        SplitSpec::Every(1),
    )?;
    let output = dir.path().join("tagged-refused");
    let control = ExportControl::default();
    let prepared = prepare_split(
        tagged_plan.clone(),
        &output,
        &LocalSplitOutput,
        &exporter,
        &control,
    )?;
    ensure!(prepared.report.tags_loss, "Batch-specific tag-loss consent");
    ensure!(
        matches!(
            finish_split(prepared, false, &exporter, renderer, &control, |_| {}),
            Err(SplitError::Export(ExportError::Acknowledgement))
        ),
        "Consent not bypassed"
    );
    ensure!(!output.exists(), "Declined consent publishes nothing");
    let output = dir.path().join("tagged-accepted");
    let control = ExportControl::default();
    let prepared = prepare_split(
        tagged_plan.clone(),
        &output,
        &LocalSplitOutput,
        &exporter,
        &control,
    )?;
    finish_split(prepared, true, &exporter, renderer, &control, |_| {})?;
    for (index, part) in tagged_plan.parts().iter().enumerate() {
        super::exporting::verify(part, &output.join(SplitPlan::filename(index)), renderer)?;
    }

    let copy_source = dir.path().join("empty-form-tagged.pdf");
    let copy_bytes = super::synthetic::export_safety_pdf("empty-form-tagged");
    std::fs::write(&copy_source, &copy_bytes)?;
    let copy_workspace = open_document(renderer, &copy_source, SourceId(7004))?;
    let copy_plan = SplitPlan::new(
        ExportPlan::capture(&copy_workspace, 5, ExportScope::All)?,
        SplitSpec::Every(1),
    )?;
    let output = dir.path().join("whole-copy");
    let control = ExportControl::default();
    let prepared = prepare_split(copy_plan, &output, &LocalSplitOutput, &exporter, &control)?;
    ensure!(
        !prepared.report.requires_acknowledgement(),
        "Whole-source copy needs no loss consent"
    );
    finish_split(prepared, false, &exporter, renderer, &control, |_| {})?;
    ensure!(
        std::fs::read(output.join(SplitPlan::filename(0)))? == copy_bytes,
        "Whole-source bytes preserved"
    );

    let form = dir.path().join("signature.pdf");
    std::fs::write(&form, super::synthetic::export_safety_pdf("signature"))?;
    let mut unsafe_workspace = open_document(renderer, &a, SourceId(7005))?;
    unsafe_workspace.execute(Command::Import(
        open_document(renderer, &form, SourceId(7006))?
            .source()
            .clone(),
    ))?;
    let unsafe_full = ExportPlan::capture(&unsafe_workspace, 5, ExportScope::All)?;
    let unsafe_plan = SplitPlan::new(unsafe_full.clone(), SplitSpec::Every(4))?;
    let output = dir.path().join("unsafe-second-part");
    ensure!(
        prepare_split(
            unsafe_plan,
            &output,
            &LocalSplitOutput,
            &exporter,
            &ExportControl::default()
        )
        .is_err(),
        "Unsafe later part blocks entire batch"
    );
    ensure!(!output.exists(), "Preflight failure publishes nothing");
    // A manual plan referencing only safe pages must not snapshot unused inputs.
    let safe_only = SplitPlan::new(unsafe_full, SplitSpec::Ranges("1-2;3-4"))?;
    ensure!(
        safe_only.full().sources().len() == 1,
        "Only referenced sources captured"
    );
    let output = dir.path().join("safe-only");
    let control = ExportControl::default();
    let prepared = prepare_split(safe_only, &output, &LocalSplitOutput, &exporter, &control)?;
    finish_split(prepared, false, &exporter, renderer, &control, |_| {})?;

    // Original sources are rechecked at publication, even though outputs use snapshots.
    let output = dir.path().join("changed-source");
    let control = ExportControl::default();
    let prepared = prepare_split(plan, &output, &LocalSplitOutput, &exporter, &control)?;
    let mut changed = bytes_a.clone();
    changed.push(b'\n');
    std::fs::write(&a, changed)?;
    ensure!(
        matches!(
            finish_split(prepared, false, &exporter, renderer, &control, |_| {}),
            Err(SplitError::Export(ExportError::SourceChanged))
        ),
        "Changed original refused"
    );
    ensure!(!output.exists(), "Changed original publishes nothing");
    std::fs::write(&a, &bytes_a)?;
    ensure!(
        std::fs::read(&a)? == bytes_a && std::fs::read(&b)? == bytes_b,
        "Source bytes preserved"
    );
    ensure!(
        !std::fs::read_dir(dir.path())?
            .filter_map(Result::ok)
            .any(|e| e
                .file_name()
                .to_string_lossy()
                .starts_with(".pdf-studio-split-")),
        "Batch staging cleaned"
    );
    println!(
        "PASS Split: per-N/manual, multi-source order/rotation/text/raster, progress, consent, no-clobber/race, source checks, mid-batch failure/cancel and cleanup"
    );
    Ok(())
}
