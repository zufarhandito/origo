use anyhow::{Context, Result, ensure};
use pdf_application::export::ExportControl;
use pdf_application::{PdfInspector, SourceId, Workspace, open_document, text::TextRequest};
use pdf_engine::PdfiumRenderer;
use pdf_ocr::{Cached, DocumentPage, Language, OcrError, Raster, Recognizer, Store, Tesseract};
use std::{
    path::Path,
    time::{Duration, Instant},
};

pub fn make_book(fixtures: &Path, pages: usize, mixed: bool) -> Result<Vec<u8>> {
    let source = image::open(fixtures.join("indonesia.png"))?;
    let width = source.width();
    let height = source.height();
    let mut jpeg = Vec::new();
    image::codecs::jpeg::JpegEncoder::new_with_quality(&mut jpeg, 90).encode_image(&source)?;
    Ok(crate::synthetic::ocr_scan_pdf(
        &(jpeg, width, height),
        pages,
        mixed,
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
