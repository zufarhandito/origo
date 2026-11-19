use anyhow::{Result, ensure};
use pdf_application::{
    ExactPageRenderer, ExactRenderRequest, PdfInspector, SourceId,
    export::{ExportPlan, ExportScope},
    printing::PrintPlan,
};
use pdf_engine::PdfiumRenderer;
use std::path::Path;
pub fn verify_printed(path: &Path, renderer: &PdfiumRenderer) -> Result<()> {
    let info = renderer.inspect(path)?;
    ensure!(info.pages.len() == 1, "printed current page count");
    let raster = renderer.render_exact(&ExactRenderRequest {
        path: path.into(),
        page_index: 0,
        width: 600,
        height: 840,
        rotation_delta: 0,
        fingerprint: info.fingerprint,
        annotations: Some(vec![]),
    })?;
    let blue = raster
        .rgba
        .as_chunks::<4>()
        .0
        .iter()
        .filter(|p| {
            p[2] > 120
                && u16::from(p[2]) > u16::from(p[0]) + 40
                && u16::from(p[2]) > u16::from(p[1]) + 20
        })
        .count();
    ensure!(
        blue > 20,
        "unsaved blue annotation appears in printed output: {blue}"
    );
    println!("PASS printed PDF: one page with unsaved blue annotation, {blue} blue pixels");
    Ok(())
}
pub fn write(root: &Path) -> Result<()> {
    let dir = root.join(".research/outline-print-fixtures");
    std::fs::create_dir_all(&dir)?;
    std::fs::write(dir.join("outline.pdf"), crate::synthetic::outline_pdf())?;
    std::fs::write(
        dir.join("long-outline.pdf"),
        crate::synthetic::long_outline_pdf(),
    )?;
    std::fs::write(
        dir.join("no-outline.pdf"),
        crate::synthetic::export_safety_pdf("plain"),
    )?;
    Ok(())
}
pub fn run(root: &Path, renderer: &PdfiumRenderer) -> Result<()> {
    let dir = tempfile::tempdir_in(root.join(".research"))?;
    let source = dir.path().join("outline.pdf");
    let bytes = crate::synthetic::outline_pdf();
    std::fs::write(&source, &bytes)?;
    let info = renderer.inspect(&source)?;
    let outline = renderer.outline(&source, info.fingerprint)?;
    ensure!(
        outline.nodes.len() == 6,
        "outline length {}",
        outline.nodes.len()
    );
    ensure!(outline.nodes[0].title == "Bab 1" && outline.nodes[0].page == Some(0));
    ensure!(
        outline.nodes[1].parent == Some(0)
            && outline.nodes[1].page == Some(1)
            && outline.nodes[1].point.is_some(),
        "named destination"
    );
    ensure!(
        outline.nodes[2].blocked && outline.nodes[2].page.is_none(),
        "external action never navigates"
    );
    ensure!(
        outline.nodes[3].page == Some(2) && outline.nodes[3].point.is_some(),
        "rotated crop coordinates"
    );
    ensure!(
        outline.nodes[4].page.is_none() && outline.nodes[5].parent == Some(4),
        "parent without destination"
    );
    let large = dir.path().join("large.pdf");
    std::fs::write(&large, crate::synthetic::large_outline_pdf(4200))?;
    let large_info = renderer.inspect(&large)?;
    let limited = renderer.outline(&large, large_info.fingerprint)?;
    ensure!(
        limited.nodes.len() == 4096 && limited.limited,
        "bounded outline reports truncation"
    );
    let w = pdf_application::open_document(renderer, &source, SourceId(1))?;
    let plan = PrintPlan::new(ExportPlan::capture(&w, 1, ExportScope::All)?, "3,1")?;
    ensure!(plan.indices == [0, 2] && !w.is_dirty());
    std::fs::write(&source, [bytes.clone(), b"\n".to_vec()].concat())?;
    ensure!(
        renderer.outline(&source, info.fingerprint).is_err(),
        "stale outline rejected"
    );
    let source = dir.path().join("print.pdf");
    let bytes = crate::synthetic::print_flags_pdf();
    std::fs::write(&source, &bytes)?;
    let info = renderer.inspect(&source)?;
    let request = ExactRenderRequest {
        path: source.clone(),
        page_index: 0,
        width: 600,
        height: 840,
        rotation_delta: 0,
        fingerprint: info.fingerprint,
        annotations: Some(vec![]),
    };
    let viewer = renderer.render_exact(&request)?;
    let print = renderer.render_print(&request)?;
    let at = |x: usize, y: usize| (y * 600 + x) * 4;
    ensure!(
        viewer.rgba[at(100, 760)..at(100, 760) + 3] == [255, 0, 0],
        "non-printing mark visible in viewer"
    );
    ensure!(
        print.rgba[at(100, 760)..at(100, 760) + 3] == [255, 255, 255],
        "non-printable annotation omitted"
    );
    ensure!(
        print.rgba[at(360, 760)..at(360, 760) + 3] == [0, 0, 255],
        "printable annotation retained"
    );
    ensure!(std::fs::read(&source)? == bytes);
    println!(
        "PASS outline/print: nested/named/cropped destinations, action exclusion, parent target, stale source, print intent flags, snapshot ranges"
    );
    Ok(())
}
