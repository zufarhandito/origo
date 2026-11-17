use anyhow::{Result, ensure};
use pdf_application::{
    ExactPageRenderer, ExactRenderRequest, SourceId,
    export::{ExportControl, ExportError, ExportPlan, ExportScope},
    open_document,
    png::{PngDpi, PngError, PngPlan, PngWriter, dimensions, finish_png, prepare_png},
    split::SplitError,
};
use pdf_domain::Command;
use pdf_engine::PdfiumRenderer;
use pdf_platform::png::{LocalPngOutput, LocalPngWriter};
use qpdf_adapter::QpdfExporter;
use std::{
    path::Path,
    sync::atomic::{AtomicUsize, Ordering},
};

struct FailSecond {
    count: AtomicUsize,
}
impl PngWriter for FailSecond {
    fn write_png(
        &self,
        raster: &pdf_application::RenderedPage,
        path: &Path,
        remaining: u64,
        control: &ExportControl,
    ) -> Result<u64, PngError> {
        if self.count.fetch_add(1, Ordering::SeqCst) == 1 {
            return Err(PngError::Encoding);
        }
        LocalPngWriter.write_png(raster, path, remaining, control)
    }
}
pub fn run(root: &Path, renderer: &PdfiumRenderer) -> Result<()> {
    let dir = tempfile::tempdir_in(root.join("tests/fixtures"))?;
    let a = dir.path().join("sumber A_日本.pdf");
    let b = dir.path().join("sumber B & test.pdf");
    let bytes_a = super::synthetic::selection_pdf();
    let bytes_b = super::synthetic::mixed_selection_pdf_named("SOURCE B");
    std::fs::write(&a, &bytes_a)?;
    std::fs::write(&b, &bytes_b)?;
    let exporter = QpdfExporter::new(&root.join("sidecars"));
    let mut w = open_document(renderer, &a, SourceId(8101))?;
    w.execute(Command::Import(
        open_document(renderer, &b, SourceId(8102))?
            .source()
            .clone(),
    ))?;
    let moved = w.pages()[5].id;
    w.execute(Command::Move {
        ids: vec![moved],
        target: 0,
    })?;
    let all: Vec<_> = w.pages().iter().map(|p| p.id).collect();
    let sources = w.sources().to_vec();
    for dpi in [PngDpi::Dpi150, PngDpi::Dpi300] {
        for turns in 0..4 {
            let snapshot = ExportPlan::capture(&w, 17, ExportScope::All)?;
            let revision = w.revision();
            let plan = PngPlan::new(snapshot.clone(), dpi)?;
            let output = dir.path().join(format!("all-{}-{turns}", dpi.value()));
            let control = ExportControl::default();
            let prepared = prepare_png(plan, &output, &LocalPngOutput, &exporter, &control)?;
            ensure!(!output.exists(), "PNG folder not published before commit");
            let mut progress = Vec::new();
            let receipt = finish_png(prepared, renderer, &LocalPngWriter, &control, |n| {
                progress.push(n)
            })?;
            ensure!(
                receipt.pages == 8
                    && receipt.instance == 17
                    && receipt.revision == revision
                    && receipt.bytes > 0
                    && receipt.dpi == dpi,
                "PNG receipt origin/counts"
            );
            ensure!(
                progress == (1..=8).collect::<Vec<_>>() && std::fs::read_dir(&output)?.count() == 8,
                "PNG progress and exact files"
            );
            for (index, page) in snapshot.pages().iter().enumerate() {
                let source = sources
                    .iter()
                    .find(|s| s.id == page.entry.source_id)
                    .ok_or_else(|| anyhow::anyhow!("source"))?;
                let geometry = source.geometries[usize::from(page.entry.source_page_index)];
                let (width, height) = dimensions(geometry, dpi)?;
                let base = renderer.render_exact(&ExactRenderRequest {
                    path: source.path.clone(),
                    page_index: page.entry.source_page_index,
                    rotation_delta: 0,
                    fingerprint: source.fingerprint,
                    width,
                    height,
                })?;
                let base = image::RgbaImage::from_raw(width, height, base.rgba)
                    .ok_or_else(|| anyhow::anyhow!("base raster"))?;
                let expected = match page.entry.rotation_delta {
                    0 => base,
                    1 => image::imageops::rotate90(&base),
                    2 => image::imageops::rotate180(&base),
                    _ => image::imageops::rotate270(&base),
                };
                let actual = image::open(output.join(PngPlan::filename(index)))?.into_rgba8();
                ensure!(
                    actual == expected,
                    "Full decoded PNG order/channel/rotation pixels: {index} {turns}"
                );
                ensure!(
                    actual.pixels().all(|p| p[3] == 255),
                    "Opaque white rendering contract"
                );
            }
            ensure!(
                w.is_dirty() && w.revision() == revision,
                "PNG does not save edits"
            );
            w.execute(Command::Rotate {
                ids: all.clone(),
                turns: 1,
            })?;
        }
    }
    w.select([all[6], all[0]]);
    let snapshot = ExportPlan::capture(&w, 18, ExportScope::Selected)?;
    let plan = PngPlan::new(snapshot.clone(), PngDpi::Dpi150)?;
    let output = dir.path().join("selected_日本 & images");
    let control = ExportControl::default();
    finish_png(
        prepare_png(plan.clone(), &output, &LocalPngOutput, &exporter, &control)?,
        renderer,
        &LocalPngWriter,
        &control,
        |_| {},
    )?;
    ensure!(
        std::fs::read_dir(&output)?.count() == 2,
        "Only selected PNGs published"
    );
    ensure!(
        matches!(
            prepare_png(plan.clone(), &output, &LocalPngOutput, &exporter, &control),
            Err(PngError::Batch(SplitError::Exists))
        ),
        "Existing directory rejected"
    );
    let existing = dir.path().join("existing-file");
    std::fs::write(&existing, b"KEEP")?;
    ensure!(
        prepare_png(
            plan.clone(),
            &existing,
            &LocalPngOutput,
            &exporter,
            &control
        )
        .is_err()
            && std::fs::read(&existing)? == b"KEEP",
        "Existing file protected"
    );
    let changed_source = snapshot.sources()[0].path.clone();
    let changed_bytes = std::fs::read(&changed_source)?;
    for mode in ["cancel", "failure", "limit", "race", "source-change"] {
        let output = dir.path().join(mode);
        let control = ExportControl::default();
        let plan = if mode == "limit" {
            plan.clone().with_output_limit(32)?
        } else {
            plan.clone()
        };
        let prepared = prepare_png(plan, &output, &LocalPngOutput, &exporter, &control)?;
        let result = if mode == "failure" {
            finish_png(
                prepared,
                renderer,
                &FailSecond {
                    count: AtomicUsize::new(0),
                },
                &control,
                |_| {},
            )
        } else {
            finish_png(prepared, renderer, &LocalPngWriter, &control, |n| {
                if n == 1 {
                    if mode == "cancel" {
                        control.cancel();
                    }
                    if mode == "race" {
                        let _ = std::fs::create_dir(&output);
                        let _ = std::fs::write(output.join("keep.txt"), b"KEEP");
                    }
                    if mode == "source-change" {
                        let _ = std::fs::write(&changed_source, b"SOURCE CHANGED");
                    }
                }
            })
        };
        ensure!(result.is_err(), "Expected PNG failure: {mode}");
        if mode == "cancel" {
            ensure!(
                matches!(result, Err(PngError::Export(ExportError::Cancelled))),
                "PNG cancellation type"
            );
        }
        if mode == "limit" {
            ensure!(
                matches!(result, Err(PngError::OutputLimit)),
                "PNG aggregate byte limit"
            );
        }
        if mode == "race" {
            ensure!(
                std::fs::read(output.join("keep.txt"))? == b"KEEP"
                    && std::fs::read_dir(&output)?.count() == 1,
                "Raced directory untouched"
            );
        } else {
            ensure!(!output.exists(), "Failed PNG folder absent: {mode}");
        }
        if mode == "source-change" {
            std::fs::write(&changed_source, &changed_bytes)?;
        }
        ensure!(
            !std::fs::read_dir(dir.path())?
                .filter_map(Result::ok)
                .any(|e| e.file_name().to_string_lossy().starts_with(".pdf-studio-")),
            "No staged PNG/source files left: {mode}"
        );
    }
    for feature in [
        "info",
        "bookmark",
        "form",
        "signature",
        "tagged",
        "annotation",
        "javascript",
        "labels",
        "unknown",
    ] {
        let source = dir.path().join(format!("{feature}.pdf"));
        std::fs::write(&source, super::synthetic::export_safety_pdf(feature))?;
        let workspace = open_document(renderer, &source, SourceId(8199))?;
        let plan = PngPlan::new(
            ExportPlan::capture(&workspace, 20, ExportScope::All)?,
            PngDpi::Dpi150,
        )?;
        let output = dir.path().join(format!("feature-{feature}"));
        let control = ExportControl::default();
        let result = prepare_png(plan, &output, &LocalPngOutput, &exporter, &control);
        if feature == "info" || feature == "bookmark" {
            finish_png(result?, renderer, &LocalPngWriter, &control, |_| {})?;
        } else {
            ensure!(
                result.is_err() && !output.exists(),
                "Complex PNG source blocked: {feature}"
            );
        }
    }
    ensure!(
        std::fs::read(&a)? == bytes_a && std::fs::read(&b)? == bytes_b,
        "PNG sources byte-identical"
    );
    // Fail early on invalid exact sizes; the viewer's downscaling contract is separate.
    ensure!(
        renderer
            .render_exact(&ExactRenderRequest {
                path: a,
                page_index: 0,
                rotation_delta: 0,
                fingerprint: sources[0].fingerprint,
                width: 8193,
                height: 1
            })
            .is_err(),
        "Exact raster never clamps"
    );
    println!(
        "PASS PNG: 150/300 DPI, 16 intrinsic/virtual rotations, multi-source order, full decoded pixels, selection, metadata/bookmarks, blockers, dirty state, byte cap, failure/cancel/race/source change and staging cleanup"
    );
    Ok(())
}
