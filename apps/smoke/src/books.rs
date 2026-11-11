use anyhow::{Result, ensure};
use pdf_application::{PageRenderer, PdfInspector, RenderError, RenderRequest};
use pdf_engine::PdfiumRenderer;
use std::{path::Path, time::Instant};

pub fn run(root: &Path) -> Result<()> {
    let directory = root.join(".research/book-benchmark");
    std::fs::create_dir_all(&directory)?;
    let mut csv = String::from(
        "kind,pages,file_bytes,inspect_ms,first_ms,p50_ms,p95_ms,renders,document_opens\n",
    );
    for scan in [false, true] {
        for count in [100, 300, 500, 1000] {
            let kind = if scan { "simulated-scan" } else { "vector" };
            let bytes = if scan {
                super::synthetic::scanned_book(count)
            } else {
                super::synthetic::pdf(count, true)
            };
            let path = directory.join(format!("{kind}-{count}.pdf"));
            std::fs::write(&path, &bytes)?;
            let renderer = PdfiumRenderer::new(&root.join("sidecars"));
            let start = Instant::now();
            let info = renderer.inspect(&path)?;
            let inspect = start.elapsed().as_millis();
            ensure!(info.pages.len() == count, "Book metadata count");
            let mut timings = Vec::new();
            // Cross the 64-render session recycle boundary and include distant jumps.
            for iteration in 0..72 {
                let index = if iteration < 36 {
                    iteration
                } else {
                    count - 1 - (iteration - 36)
                };
                let start = Instant::now();
                let page = renderer.render_page(&RenderRequest {
                    path: path.clone(),
                    fingerprint: Some(info.fingerprint),
                    page_index: index as u16,
                    target_width: 1200,
                    rotation_delta: 0,
                })?;
                timings.push(start.elapsed().as_micros() as f64 / 1000.);
                ensure!(
                    page.page_count as usize == count && page.rgba.len() <= 64 * 1024 * 1024,
                    "Book raster bounds"
                );
                if scan {
                    ensure!(
                        page.rgba[..3].iter().all(|value| *value > 230),
                        "Scan background decoded"
                    );
                }
            }
            let first = timings[0];
            timings.sort_by(f64::total_cmp);
            let stats = renderer.session_stats();
            ensure!(
                stats.renders == 72 && stats.document_opens == 2,
                "Persistent session reused and recycled"
            );
            // Failed/stale requests must not serve the already-open document.
            let mut wrong = info.fingerprint;
            wrong.bytes += 1;
            ensure!(
                matches!(
                    renderer.render_page(&RenderRequest {
                        path: path.clone(),
                        fingerprint: Some(wrong),
                        page_index: 0,
                        target_width: 256,
                        rotation_delta: 0,
                    }),
                    Err(RenderError::SourceChanged)
                ),
                "Cached session rejects stale fingerprint"
            );
            renderer.inspect(&path)?;
            ensure!(
                renderer.session_stats().document_opens == 3,
                "Changed session was released"
            );
            if scan && count == 1000 {
                println!("Checking idle-session release (16 seconds)...");
                std::thread::sleep(std::time::Duration::from_secs(16));
                renderer.inspect(&path)?;
                ensure!(
                    renderer.session_stats().document_opens == 4,
                    "Idle session released and reopened"
                );
            }
            ensure!(std::fs::read(&path)? == bytes, "Book source unchanged");
            let row = format!(
                "{kind},{count},{},{inspect},{first:.2},{:.2},{:.2},{},{}\n",
                bytes.len(),
                timings[36],
                timings[68],
                stats.renders,
                stats.document_opens
            );
            print!("{row}");
            csv.push_str(&row);
        }
    }
    std::fs::write(directory.join("results.csv"), csv)?;
    let oversized = directory.join("over-page-limit.pdf");
    std::fs::write(&oversized, super::synthetic::pdf(1025, false))?;
    let renderer = PdfiumRenderer::new(&root.join("sidecars"));
    ensure!(
        matches!(
            renderer.inspect(&oversized),
            Err(RenderError::ResourceLimit)
        ),
        "Default page limit enforced"
    );
    let oversized_file = directory.join("over-file-limit.pdf");
    let file = std::fs::File::create(&oversized_file)?;
    file.set_len(128 * 1024 * 1024 + 1)?;
    drop(file);
    ensure!(
        matches!(
            renderer.inspect(&oversized_file),
            Err(RenderError::ResourceLimit)
        ),
        "Default byte limit enforced before parse"
    );
    std::fs::remove_file(oversized_file)?;
    println!(
        "PASS book benchmark: 8 cases, 576 renders, reuse/recycle/source protection; synthetic fixtures only"
    );
    Ok(())
}
