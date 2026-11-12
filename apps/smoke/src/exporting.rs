use anyhow::{Result, ensure};
use pdf_application::{
    ExportSupport, PageRenderer, PdfInspector, RenderRequest, SourceDocument, SourceId,
    export::{
        ExportControl, ExportError, ExportPlan, ExportPreflight, ExportScope, finish_export,
        prepare_export,
    },
    open_document,
    text::{TextExtractor, TextRequest},
};
use pdf_domain::Command;
use pdf_engine::PdfiumRenderer;
use pdf_platform::{LocalOutput, fingerprint};
use qpdf_adapter::QpdfExporter;
use std::{path::Path, process::Command as ProcessCommand};

fn verify(plan: &ExportPlan, path: &Path, renderer: &PdfiumRenderer) -> Result<()> {
    let output = renderer.inspect(path)?;
    ensure!(output.pages.len() == plan.pages().len(), "M3 output count");
    for (index, page) in plan.pages().iter().enumerate() {
        let source = plan
            .sources()
            .iter()
            .find(|s| s.id == page.entry.source_id)
            .ok_or_else(|| anyhow::anyhow!("Missing source"))?;
        let original = renderer.extract_text(&TextRequest {
            path: source.path.clone(),
            page_index: page.entry.source_page_index,
            fingerprint: source.fingerprint,
        })?;
        let actual = renderer.extract_text(&TextRequest {
            path: path.into(),
            page_index: index as u16,
            fingerprint: output.fingerprint,
        })?;
        let mut expected_lines = original
            .copy_range(0..original.chars.len())
            .lines()
            .map(str::to_owned)
            .collect::<Vec<_>>();
        expected_lines.sort();
        let mut actual_lines = actual
            .copy_range(0..actual.chars.len())
            .lines()
            .map(str::to_owned)
            .collect::<Vec<_>>();
        actual_lines.sort();
        ensure!(
            expected_lines == actual_lines,
            "M3 source/page text identity at {index}"
        );
        let expected = renderer.render_page(&RenderRequest {
            path: source.path.clone(),
            page_index: page.entry.source_page_index,
            target_width: 256,
            rotation_delta: page.entry.rotation_delta,
            fingerprint: Some(source.fingerprint),
        })?;
        let actual = renderer.render_page(&RenderRequest {
            path: path.into(),
            page_index: index as u16,
            target_width: 256,
            rotation_delta: 0,
            fingerprint: Some(output.fingerprint),
        })?;
        let mut mismatches = 0;
        for y in 0..8 {
            for x in 0..8 {
                let a = (((expected.height as usize * (y * 2 + 1) / 16) * expected.width as usize
                    + expected.width as usize * (x * 2 + 1) / 16)
                    * 4)
                .min(expected.rgba.len() - 4);
                let b = (((actual.height as usize * (y * 2 + 1) / 16) * actual.width as usize
                    + actual.width as usize * (x * 2 + 1) / 16)
                    * 4)
                .min(actual.rgba.len() - 4);
                if (0..3).any(|c| expected.rgba[a + c].abs_diff(actual.rgba[b + c]) > 30) {
                    mismatches += 1;
                }
            }
        }
        ensure!(
            mismatches <= 8,
            "M3 representative raster identity at {index}"
        );
    }
    Ok(())
}
pub fn run(root: &Path, renderer: &PdfiumRenderer) -> Result<()> {
    let dir = tempfile::tempdir_in(root.join("tests/fixtures"))?;
    let a = dir.path().join("-source A & test.pdf");
    let b = dir.path().join("Sumber B \u{65e5}\u{672c}.pdf");
    let bytes_a = super::synthetic::selection_pdf();
    let bytes_b = super::synthetic::selection_pdf_named("SOURCE B");
    std::fs::write(&a, &bytes_a)?;
    std::fs::write(&b, &bytes_b)?;
    let exporter = QpdfExporter::new(&root.join("sidecars"));
    let mut workspace = open_document(renderer, &a, SourceId(501))?;
    let source_b = open_document(renderer, &b, SourceId(502))?.source().clone();
    workspace.execute(Command::Import(source_b))?;
    let ids = workspace.pages().iter().map(|p| p.id).collect::<Vec<_>>();
    workspace.execute(Command::Move {
        ids: vec![ids[0], ids[5]],
        target: 7,
    })?;
    workspace.execute(Command::Rotate {
        ids: vec![ids[0], ids[2], ids[5]],
        turns: 1,
    })?;
    workspace.execute(Command::Delete { ids: vec![ids[6]] })?;
    let plan = ExportPlan::capture(&workspace, 55, ExportScope::All)?;
    let dest = dir.path().join("edited \u{65e5}\u{672c} & output.pdf");
    let c = ExportControl::default();
    let prepared = prepare_export(plan.clone(), &dest, &LocalOutput, &exporter, &c)?;
    ensure!(!prepared.report.metadata_loss, "Plain report");
    let receipt = finish_export(prepared, false, &exporter, renderer, &c)?;
    verify(&plan, &dest, renderer)?;
    ensure!(!receipt.apply(56, &mut workspace), "Instance isolation");
    ensure!(receipt.apply(55, &mut workspace), "Full Save As baseline");
    ensure!(!workspace.is_dirty(), "Clean after export");
    workspace.select([ids[5], ids[2]]);
    let subset = ExportPlan::capture(&workspace, 55, ExportScope::Selected)?;
    let dest = dir.path().join("extract.pdf");
    let c = ExportControl::default();
    let prepared = prepare_export(subset.clone(), &dest, &LocalOutput, &exporter, &c)?;
    let extracted = finish_export(prepared, false, &exporter, renderer, &c)?;
    verify(&subset, &dest, renderer)?;
    ensure!(
        !extracted.apply(55, &mut workspace),
        "Extract is not Save As"
    );
    for turns in 0..4 {
        let mut w = open_document(renderer, &a, SourceId(600 + u64::from(turns)))?;
        let all = w.pages().iter().map(|p| p.id).collect();
        w.execute(Command::Rotate { ids: all, turns })?;
        let plan = ExportPlan::capture(&w, 1, ExportScope::All)?;
        let c = ExportControl::default();
        let dest = dir.path().join(format!("rotation-{turns}.pdf"));
        let prepared = prepare_export(plan.clone(), &dest, &LocalOutput, &exporter, &c)?;
        finish_export(prepared, false, &exporter, renderer, &c)?;
        verify(&plan, &dest, renderer)?;
    }
    for feature in [
        "plain",
        "info",
        "metadata",
        "form",
        "signature",
        "tagged",
        "bookmark",
        "bookmark-action",
        "bookmark-named",
        "bookmark-legacy",
        "bookmark-nested",
        "bookmark-info",
        "bookmark-javascript",
        "bookmark-remote",
        "bookmark-chained",
        "annotation",
        "javascript",
        "attachment",
        "labels",
        "unknown",
    ] {
        let path = dir.path().join(format!("{feature}.pdf"));
        std::fs::write(&path, super::synthetic::export_safety_pdf(feature))?;
        let source = open_document(renderer, &path, SourceId(800))?
            .source()
            .clone();
        let report = exporter.preflight(&[source], &ExportControl::default());
        let bookmarks = matches!(
            feature,
            "bookmark"
                | "bookmark-action"
                | "bookmark-named"
                | "bookmark-legacy"
                | "bookmark-nested"
                | "bookmark-info"
        );
        let metadata = matches!(feature, "info" | "metadata" | "bookmark-info");
        if matches!(feature, "plain" | "info" | "metadata") || bookmarks {
            let report = report?;
            ensure!(
                report.metadata_loss == metadata && report.bookmarks_loss == bookmarks,
                "Feature detection: {feature}"
            );
        } else {
            ensure!(report.is_err(), "Must reject {feature}");
        }
        if metadata || bookmarks {
            let w = open_document(renderer, &path, SourceId(801))?;
            let plan = ExportPlan::capture(&w, 1, ExportScope::All)?;
            let dest = dir.path().join(format!("{feature}-approved.pdf"));
            let c = ExportControl::default();
            let prepared = prepare_export(plan.clone(), &dest, &LocalOutput, &exporter, &c)?;
            ensure!(
                matches!(
                    finish_export(prepared, false, &exporter, renderer, &c),
                    Err(ExportError::Acknowledgement)
                ),
                "Explicit feature-loss consent: {feature}"
            );
            ensure!(!dest.exists(), "No unapproved file");
            let prepared = prepare_export(plan.clone(), &dest, &LocalOutput, &exporter, &c)?;
            finish_export(prepared, true, &exporter, renderer, &c)?;
            verify(&plan, &dest, renderer)?;
            let output_source = open_document(renderer, &dest, SourceId(802))?
                .source()
                .clone();
            let output_report = exporter.preflight(&[output_source], &ExportControl::default())?;
            ensure!(
                !output_report.requires_acknowledgement(),
                "Output must actually omit bookmark/metadata structures: {feature}"
            );
            // Selected extraction has its own consent, including after rotation.
            if feature == "bookmark-nested" {
                let mut subset = w;
                let ids = subset.pages().iter().map(|p| p.id).collect::<Vec<_>>();
                subset.execute(Command::Rotate {
                    ids: ids.clone(),
                    turns: 1,
                })?;
                subset.select(ids);
                let selected = ExportPlan::capture(&subset, 1, ExportScope::Selected)?;
                let c = ExportControl::default();
                let prepared = prepare_export(
                    selected.clone(),
                    &dir.path().join("bookmark-extract.pdf"),
                    &LocalOutput,
                    &exporter,
                    &c,
                )?;
                ensure!(
                    prepared.report.bookmarks_loss,
                    "Extract still requires bookmark consent"
                );
                let receipt = finish_export(prepared, true, &exporter, renderer, &c)?;
                ensure!(
                    !receipt.apply(1, &mut subset) && subset.is_dirty(),
                    "Extract baseline"
                );
                verify(&selected, &receipt.destination, renderer)?;
            }
        }
    }
    let plain = dir.path().join("plain.pdf");
    // A registry entry retained for Undo must not participate after all its pages
    // have been removed from the captured plan.
    let mut remaining = open_document(renderer, &plain, SourceId(850))?;
    let form_source = open_document(renderer, &dir.path().join("form.pdf"), SourceId(851))?
        .source()
        .clone();
    remaining.execute(Command::Import(form_source))?;
    let form_pages = remaining
        .pages()
        .iter()
        .filter(|p| p.source_id == SourceId(851))
        .map(|p| p.id)
        .collect();
    remaining.execute(Command::Delete { ids: form_pages })?;
    let remaining_plan = ExportPlan::capture(&remaining, 1, ExportScope::All)?;
    ensure!(
        remaining.sources().len() == 2 && remaining_plan.sources().len() == 1,
        "Only referenced sources enter export"
    );
    let c = ExportControl::default();
    let prepared = prepare_export(
        remaining_plan,
        &dir.path().join("remaining.pdf"),
        &LocalOutput,
        &exporter,
        &c,
    )?;
    finish_export(prepared, false, &exporter, renderer, &c)?;

    let recovered = dir.path().join("recovered.pdf");
    let mut recovered_bytes = super::synthetic::export_safety_pdf("plain");
    let marker = b"startxref\n";
    let offset = recovered_bytes
        .windows(marker.len())
        .position(|s| s == marker)
        .ok_or_else(|| anyhow::anyhow!("Missing synthetic xref marker"))?
        + marker.len();
    for digit in recovered_bytes[offset..]
        .iter_mut()
        .take_while(|b| b.is_ascii_digit())
    {
        *digit = b'0';
    }
    std::fs::write(&recovered, recovered_bytes)?;
    let mut check = ProcessCommand::new(root.join("sidecars/qpdf/bin/qpdf.exe"));
    check.arg("--check").arg(&recovered);
    ensure!(
        pdf_platform::run(check, 65536, &ExportControl::default())?.code == Some(3),
        "Fixture really requires QPDF warning/recovery"
    );
    let recovered_source = open_document(renderer, &recovered, SourceId(852))?
        .source()
        .clone();
    ensure!(
        matches!(
            exporter.preflight(&[recovered_source], &ExportControl::default()),
            Err(ExportError::Structure)
        ),
        "Recovered warning must block export"
    );

    let encrypted = dir.path().join("encrypted.pdf");
    let mut cmd = ProcessCommand::new(root.join("sidecars/qpdf/bin/qpdf.exe"));
    cmd.args(["--encrypt", "user", "owner", "256", "--"])
        .arg(&plain)
        .arg(&encrypted);
    ensure!(
        pdf_platform::run(cmd, 65536, &ExportControl::default())?.code == Some(0),
        "Encryption fixture"
    );
    let info = renderer.inspect(&plain)?;
    let source = SourceDocument {
        id: SourceId(900),
        path: encrypted.clone(),
        fingerprint: fingerprint(&encrypted)?,
        geometries: info.pages.clone(),
        export_support: ExportSupport::NotPreflighted,
    };
    ensure!(
        matches!(
            exporter.preflight(&[source], &ExportControl::default()),
            Err(ExportError::Unsupported(_))
        ),
        "Encrypted rejection"
    );
    let corrupt = dir.path().join("malformed.pdf");
    std::fs::write(&corrupt, b"%PDF-1.4\nbroken")?;
    let source = SourceDocument {
        id: SourceId(901),
        path: corrupt.clone(),
        fingerprint: fingerprint(&corrupt)?,
        geometries: info.pages,
        export_support: ExportSupport::NotPreflighted,
    };
    ensure!(
        exporter
            .preflight(&[source], &ExportControl::default())
            .is_err(),
        "Malformed rejection"
    );
    let c = ExportControl::default();
    let dest = dir.path().join("changed.pdf");
    let prepared = prepare_export(plan, &dest, &LocalOutput, &exporter, &c)?;
    let mut modified = bytes_a.clone();
    modified.push(b'\n');
    std::fs::write(&a, &modified)?;
    ensure!(
        matches!(
            finish_export(prepared, false, &exporter, renderer, &c),
            Err(ExportError::SourceChanged)
        ),
        "Live source change before commit"
    );
    ensure!(!dest.exists(), "Changed source not published");
    std::fs::write(&a, &bytes_a)?;
    ensure!(
        std::fs::read(&a)? == bytes_a && std::fs::read(&b)? == bytes_b,
        "M3 originals unchanged"
    );
    ensure!(
        !std::fs::read_dir(dir.path())?
            .filter_map(Result::ok)
            .any(|e| e
                .file_name()
                .to_string_lossy()
                .starts_with(".pdf-studio-export-")),
        "M3 staging cleanup"
    );
    println!(
        "PASS M3: multi-source Save As/Extract, source identity/raster, 16 rotations, metadata/bookmark consent, local/named/nested bookmark removal, unsafe/encrypted/malformed/recovered rejection, unused-source filtering, stale source and staging cleanup"
    );
    Ok(())
}
