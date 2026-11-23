use anyhow::{Context, Result, ensure};
use pdf_application::export::ExportControl;
use pdf_application::{PdfInspector, SourceId, Workspace, open_document, text::TextRequest};
use pdf_engine::PdfiumRenderer;
use pdf_ocr::{
    BatchPlan, Cached, DocumentPage, Language, MAX_BATCH_PAGES, OcrError, PageStatus, Raster,
    Recognizer, Store, Tesseract,
};
use std::{
    path::Path,
    time::{Duration, Instant},
};

pub fn make_book(fixtures: &Path, pages: usize, mixed: bool) -> Result<Vec<u8>> {
    let mut source = image::open(fixtures.join("indonesia.png"))?;
    let size = if pages >= 100 {
        source = source.thumbnail(1240, 1754);
        [297.6, 420.96]
    } else {
        [595.2, 841.92]
    };
    let width = source.width();
    let height = source.height();
    let mut jpeg = Vec::new();
    image::codecs::jpeg::JpegEncoder::new_with_quality(&mut jpeg, 90).encode_image(&source)?;
    Ok(crate::synthetic::ocr_scan_pdf(
        &(jpeg, width, height),
        pages,
        mixed,
        size,
    ))
}

pub fn request_for(workspace: &Workspace, index: usize) -> Result<DocumentPage> {
    let entry = workspace.pages().get(index).context("missing test page")?;
    let source = workspace
        .resolve(entry.source_id)
        .context("missing test source")?;
    Ok(DocumentPage {
        request: TextRequest {
            path: source.path.clone(),
            page_index: entry.source_page_index,
            fingerprint: source.fingerprint,
        },
        geometry: workspace.geometry(index)?,
        rotation: entry.rotation_delta,
        language: Language::default(),
    })
}

pub fn viewer_smoke(root: &Path, fixtures: &Path) -> Result<()> {
    let sidecars = root.join("sidecars");
    let renderer = PdfiumRenderer::new(&sidecars);
    let engine = Tesseract::new(&sidecars);
    let source = fixtures.join("viewer.pdf");
    let pdf_bytes = make_book(fixtures, 3, true)?;
    std::fs::write(&source, &pdf_bytes)?;
    let dir = tempfile::Builder::new()
        .prefix("uji-bukü-")
        .tempdir_in(fixtures)?;
    let unicode = dir.path().join("Pindai 日本.pdf");
    std::fs::write(&unicode, &pdf_bytes)?;
    let workspace = open_document(&renderer, &unicode, SourceId(1))?;
    let scan = request_for(&workspace, 0)?;
    let native = request_for(&workspace, 2)?;
    let store = Store::new(Some(dir.path()));
    let (before, from_ocr) = pdf_ocr::read_text(&renderer, &store, &scan.request, scan.language)?;
    ensure!(!pdf_ocr::has_text(&before) && !from_ocr);
    let control = ExportControl::default();
    let start = Instant::now();
    let entry = scan.recognize(&renderer, &engine, &store, &control, false)?;
    let Cached::Text(recognized) = entry else {
        anyhow::bail!("expected OCR result");
    };
    ensure!(
        recognized
            .page
            .words
            .first()
            .is_some_and(|w| w.text == "Buku")
    );
    drop(store);
    let store = Store::new(Some(dir.path()));
    ensure!(matches!(store.get(&scan.key())?, Some(Cached::Text(_))));
    ensure!(matches!(
        native.recognize(&renderer, &engine, &store, &control, false)?,
        Cached::Native
    ));
    let (mut text, from_ocr) = pdf_ocr::read_text(&renderer, &store, &scan.request, scan.language)?;
    ensure!(from_ocr);
    ensure!(text.copy_range(0..text.chars.len()) == "Buku ini memuat 123 halaman.");
    let (native, from_ocr) =
        pdf_ocr::read_text(&renderer, &store, &native.request, native.language)?;
    ensure!(
        !from_ocr
            && native
                .copy_range(0..native.chars.len())
                .contains("Native text")
    );
    let search = pdf_application::search::SearchQuery::new("memuat 123", Default::default())
        .context("query")?;
    let hits = search.find(&text, 16);
    ensure!(hits.len() == 1);
    for _ in 0..4 {
        let word = text.chars[0].bounds.context("missing OCR geometry")?;
        ensure!(
            text.hit(
                (word.left + word.right) / 2.,
                (word.top + word.bottom) / 2.,
                true
            ) == Some(0)
        );
        text.rotate(1);
    }
    ensure!(std::fs::read(&unicode)? == pdf_bytes && std::fs::read(&source)? == pdf_bytes);
    ensure!(renderer.inspect(&source)?.pages.len() == 3);
    println!(
        "PASS OCR PDF: search, selection, copy, four rotations, native text, persistent cache, source bytes ({} ms)",
        start.elapsed().as_millis()
    );
    Ok(())
}

pub fn batch_smoke(root: &Path, fixtures: &Path, count: usize) -> Result<()> {
    ensure!(matches!(count, 1 | 100 | 1000 | 1024 | 1025));
    let sidecars = root.join("sidecars");
    let renderer = PdfiumRenderer::new(&sidecars);
    let tesseract = Tesseract::new(&sidecars);
    let dir = tempfile::Builder::new()
        .prefix("ocr-batch-")
        .tempdir_in(fixtures)?;
    let path = dir.path().join("book.pdf");
    let fixture = make_book(fixtures, count, false)?;
    std::fs::write(&path, &fixture)?;
    if count > 1024 {
        ensure!(matches!(
            renderer.inspect(&path),
            Err(pdf_application::RenderError::ResourceLimit)
        ));
        println!("PASS OCR book limit: {count} pages rejected by native inspection");
        return Ok(());
    }
    let workspace = open_document(&renderer, &path, SourceId(1))?;
    let requests = (0..count)
        .map(|index| request_for(&workspace, index))
        .collect::<Result<Vec<_>>>()?;
    let marker = requests[0].key();
    let mut store = Store::new(Some(dir.path()));
    let mut batch = BatchPlan::load(requests.clone(), &store)?;
    store.set_checkpoint(&marker, true)?;
    let start = Instant::now();
    let mut done = 0;
    let mut total_raster_time = Duration::ZERO;
    while let Some((mut id, mut page)) = batch.claim_next() {
        if done == 37 {
            // The request was claimed with no result committed. Restart the
            // store and prove the same page is pending and earlier ones are ready.
            let interrupted = page.request.page_index;
            drop(batch);
            drop(store);
            store = Store::new(Some(dir.path()));
            ensure!(store.has_checkpoint(&marker)?);
            batch = BatchPlan::load(requests.clone(), &store)?;
            ensure!(batch.progress().ready == done);
            (id, page) = batch.claim_next().context("interrupted page missing")?;
            ensure!(page.request.page_index == interrupted);
        }
        let item_time = Instant::now();
        let entry = page.recognize(
            &renderer,
            &tesseract,
            &store,
            &ExportControl::default(),
            false,
        )?;
        total_raster_time += item_time.elapsed();
        batch.finish(id, PageStatus::from(&entry));
        done += 1;
        if done.is_multiple_of(100) {
            println!(
                "OCR batch {done}/{count}, elapsed {:.1} s",
                start.elapsed().as_secs_f64()
            );
        }
    }
    ensure!(batch.progress().ready == count && batch.progress().remaining() == 0);
    store.set_checkpoint(&marker, false)?;
    ensure!(!store.has_checkpoint(&marker)?);
    ensure!(std::fs::read(&path)? == fixture);
    println!(
        "PASS OCR batch {count}: {:.2} s, {:.1} ms/page, {} bytes SQLite; source identical",
        start.elapsed().as_secs_f64(),
        total_raster_time.as_secs_f64() * 1000. / count as f64,
        std::fs::metadata(dir.path().join("ocr-v1.sqlite3"))?.len(),
    );
    Ok(())
}

pub fn write_batch_fixture(fixtures: &Path, scan_pages: Option<usize>) -> Result<()> {
    let count = scan_pages.unwrap_or(300);
    ensure!((1..=MAX_BATCH_PAGES + 1).contains(&count));
    let name = scan_pages.map_or_else(
        || "batch.pdf".to_owned(),
        |count| format!("batch-{count}.pdf"),
    );
    std::fs::write(
        fixtures.join(name),
        make_book(fixtures, count, scan_pages.is_none())?,
    )?;
    Ok(())
}

pub fn engine_smoke(root: &Path, fixtures: &Path) -> Result<()> {
    let engine = Tesseract::new(&root.join("sidecars"));
    let raster = |name: &str| -> Result<Raster> {
        let image = image::open(fixtures.join(format!("{name}.png")))
            .with_context(|| format!("Cannot load synthetic fixture {name}"))?
            .into_rgba8();
        Ok(Raster {
            width: image.width(),
            height: image.height(),
            rgba: image.into_raw(),
        })
    };
    for (name, locale, expected) in [
        (
            "indonesia",
            Language::Indonesian,
            "Buku ini memuat 123 halaman.",
        ),
        (
            "english",
            Language::English,
            "A small library has 456 books.",
        ),
        (
            "mixed",
            Language::IndonesianEnglish,
            "Perpustakaan kota The library is open today.",
        ),
        ("blank", Language::IndonesianEnglish, ""),
        (
            "dua-kolom",
            Language::IndonesianEnglish,
            "Kolom kiri memuat catatan Kolom kanan berisi daftar",
        ),
        (
            "buram",
            Language::Indonesian,
            "Buku ini memuat 123 halaman.",
        ),
        (
            "miring",
            Language::Indonesian,
            "Buku ini memuat 123 halaman.",
        ),
    ] {
        let image = raster(name)?;
        let start = Instant::now();
        let result = engine.recognize(
            image,
            locale,
            &ExportControl::with_budget(Duration::from_secs(30)),
        )?;
        ensure!(result.valid());
        let text = result
            .words
            .iter()
            .map(|w| w.text.as_str())
            .collect::<Vec<_>>()
            .join(" ");
        println!(
            "OCR {name}: {} ms, {} words, text={text:?}",
            start.elapsed().as_millis(),
            result.words.len()
        );
        if matches!(name, "indonesia" | "english" | "mixed" | "blank") {
            ensure!(text == expected, "OCR reference mismatch on {name}");
        }
        if name == "dua-kolom" {
            ensure!(
                text.starts_with("Kolom kiri") && text.contains("Kolom kanan"),
                "unexpected column reading order"
            );
        }
    }
    let cancelled = ExportControl::default();
    cancelled.cancel();
    ensure!(matches!(
        engine.recognize(raster("english")?, Language::English, &cancelled),
        Err(OcrError::Cancelled)
    ));
    let timed = ExportControl::with_budget(Duration::from_nanos(1));
    ensure!(matches!(
        engine.recognize(raster("english")?, Language::English, &timed),
        Err(OcrError::Timeout)
    ));
    let missing = Tesseract::new(fixtures);
    ensure!(matches!(
        missing.check(Language::English, &ExportControl::default()),
        Err(OcrError::EngineUnavailable)
    ));

    let unicode = tempfile::Builder::new()
        .prefix("uji-認識-")
        .tempdir_in(fixtures)?;
    let sidecars = unicode.path();
    let ocr = sidecars.join("ocr");
    std::fs::create_dir_all(ocr.join("tessdata/configs"))?;
    for file in [
        "tesseract.exe",
        "tessdata/eng.traineddata",
        "tessdata/configs/tsv",
    ] {
        std::fs::copy(root.join("sidecars/ocr").join(file), ocr.join(file))?;
    }
    let partial = Tesseract::new(sidecars);
    ensure!(matches!(
        partial.check(Language::Indonesian, &ExportControl::default()),
        Err(OcrError::ModelMissing(_))
    ));
    ensure!(
        !partial
            .recognize(
                raster("english")?,
                Language::English,
                &ExportControl::default()
            )?
            .words
            .is_empty()
    );
    println!("PASS OCR executable, missing model, Unicode runtime path, cancel, timeout");
    Ok(())
}
