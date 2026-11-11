use anyhow::{Result, ensure};
use pdf_application::{
    PageRenderer, PdfInspector, RenderError,
    text::{TextExtractor, TextRequest},
};
use pdf_engine::PdfiumRenderer;
use std::path::Path;

pub fn run(root: &Path, renderer: &PdfiumRenderer) -> Result<()> {
    let temp = tempfile::tempdir_in(root.join("tests/fixtures"))?;
    let path = temp.path().join("text.pdf");
    let bytes = super::synthetic::selection_pdf();
    std::fs::write(&path, &bytes)?;
    let info = renderer.inspect(&path)?;
    let mut probes = Vec::new();
    for page_index in 0..4 {
        let text = renderer.extract_text(&TextRequest {
            path: path.clone(),
            page_index,
            fingerprint: info.fingerprint,
        })?;
        let copied = text.copy_range(0..text.chars.len());
        let label = format!("PAGE {}", page_index + 1);
        let mut lines = copied.split("\r\n").collect::<Vec<_>>();
        lines.sort();
        ensure!(
            lines == vec![label.as_str(), "Second line"],
            "Native text content/newline: {copied:?}"
        );
        let raster = renderer.render(&path, page_index)?;
        for ch in text.chars.iter().filter(|ch| !ch.unicode.is_whitespace()) {
            let rect = ch
                .bounds
                .ok_or_else(|| anyhow::anyhow!("Visible glyph has no bounds"))?;
            ensure!(
                rect.left >= 0. && rect.top >= 0. && rect.right <= 1. && rect.bottom <= 1.,
                "Normalized rotated bounds"
            );
            let mut white = 0;
            for y in (rect.top * raster.height as f32) as u32
                ..((rect.bottom * raster.height as f32).ceil() as u32).min(raster.height)
            {
                for x in (rect.left * raster.width as f32) as u32
                    ..((rect.right * raster.width as f32).ceil() as u32).min(raster.width)
                {
                    let offset = ((y * raster.width + x) * 4) as usize;
                    if raster.rgba[offset..offset + 3]
                        .iter()
                        .all(|value| *value > 220)
                    {
                        white += 1;
                    }
                }
            }
            ensure!(
                white > 0,
                "Text box aligns with raster ink at rotation {}",
                page_index * 90
            );
        }
        let start = copied
            .find(&label)
            .ok_or_else(|| anyhow::anyhow!("Page label"))?;
        let first = text
            .chars
            .get(start)
            .and_then(|ch| ch.bounds)
            .ok_or_else(|| anyhow::anyhow!("First glyph"))?;
        let last = text
            .chars
            .get(start + 5)
            .and_then(|ch| ch.bounds)
            .ok_or_else(|| anyhow::anyhow!("Last glyph"))?;
        probes.push(format!(
            "{{\"page\":{},\"firstX\":{},\"firstY\":{},\"lastX\":{},\"lastY\":{}}}",
            page_index + 1,
            (first.left + first.right) / 2.,
            (first.top + first.bottom) / 2.,
            (last.left + last.right) / 2.,
            (last.top + last.bottom) / 2.
        ));
    }
    let scan = temp.path().join("scan.pdf");
    std::fs::write(&scan, super::synthetic::scanned_book(1))?;
    let scanned = renderer.inspect(&scan)?;
    ensure!(
        renderer
            .extract_text(&TextRequest {
                path: scan,
                page_index: 0,
                fingerprint: scanned.fingerprint
            })?
            .chars
            .is_empty(),
        "Image-only scan has no invented text"
    );
    let mut changed = bytes.clone();
    changed.push(b'\n');
    std::fs::write(&path, changed)?;
    ensure!(
        matches!(
            renderer.extract_text(&TextRequest {
                path,
                page_index: 0,
                fingerprint: info.fingerprint
            }),
            Err(RenderError::SourceChanged)
        ),
        "Changed source rejects text"
    );
    let directory = root.join(".research/text-selection");
    std::fs::create_dir_all(&directory)?;
    std::fs::write(
        directory.join("probe.json"),
        format!("[{}]", probes.join(",")),
    )?;
    println!(
        "PASS text: exact content/newlines, all four source rotations and raster alignment, empty scan, changed-source rejection"
    );
    Ok(())
}
