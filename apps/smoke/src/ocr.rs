use anyhow::{Context, Result, ensure};
use pdf_application::export::ExportControl;
use pdf_ocr::{Language, OcrError, Raster, Recognizer, Tesseract};
use std::{
    path::Path,
    time::{Duration, Instant},
};

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
