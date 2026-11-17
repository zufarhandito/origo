use anyhow::{Result, ensure};
use pdf_application::{
    PdfInspector,
    search::{SearchOptions, SearchQuery},
    text::{TextExtractor, TextRequest},
};
use pdf_engine::PdfiumRenderer;
use std::path::Path;

pub fn run(root: &Path, renderer: &PdfiumRenderer) -> Result<()> {
    let temp = tempfile::tempdir_in(root.join("tests/fixtures"))?;
    let path = temp.path().join("search.pdf");
    let bytes = super::synthetic::search_pdf();
    std::fs::write(&path, &bytes)?;
    let info = renderer.inspect(&path)?;
    let query = SearchQuery::new("hasil penelitian", SearchOptions::default())
        .ok_or_else(|| anyhow::anyhow!("Missing query"))?;
    let sensitive = SearchQuery::new(
        "hasil penelitian",
        SearchOptions {
            case_sensitive: true,
            whole_words: false,
        },
    )
    .ok_or_else(|| anyhow::anyhow!("Missing query"))?;
    ensure!(info.pages.len() == 7, "Search fixture page count");
    for page_index in 0..7 {
        let mut page = renderer.extract_text(&TextRequest {
            path: path.clone(),
            page_index,
            fingerprint: info.fingerprint,
        })?;
        let hits = query.find(&page, 100);
        ensure!(
            hits.len() == usize::from(page_index < 5),
            "Native phrase/case/linebreak at page {}",
            page_index + 1
        );
        if page_index == 0 || page_index == 2 {
            ensure!(
                sensitive.find(&page, 100).is_empty(),
                "Native case sensitivity"
            );
        }
        for _ in 0..4 {
            ensure!(
                query.find(&page, 100) == hits,
                "Virtual rotation must not change matching order"
            );
            for hit in &hits {
                let rects = page.highlights(hit.clone());
                ensure!(!rects.is_empty(), "Search result geometry");
                ensure!(
                    rects
                        .iter()
                        .all(|r| r.left >= 0. && r.top >= 0. && r.right <= 1. && r.bottom <= 1.),
                    "Rotated highlight bounds"
                );
            }
            page.rotate(1);
        }
    }
    ensure!(std::fs::read(&path)? == bytes, "Search source unchanged");
    std::fs::write(&path, [&bytes[..], b"\n"].concat())?;
    ensure!(
        matches!(
            renderer.extract_text(&TextRequest {
                path,
                page_index: 0,
                fingerprint: info.fingerprint
            }),
            Err(pdf_application::RenderError::SourceChanged)
        ),
        "Search changed-source rejection"
    );
    println!(
        "PASS search: native phrase/case/linebreak/scattered words, source/virtual rotations, geometry, scan-only and source identity"
    );
    Ok(())
}
