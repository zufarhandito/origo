//! Local, bounded recognition. GPUI and document state live outside this crate.

mod batch;
mod document;
mod store;

pub use batch::{BatchPlan, MAX_BATCH_PAGES, PageStatus, Progress};
pub use document::{DocumentError, DocumentPage, has_text, read_text};
pub use store::{CacheError, Cached, PageKey, Recognition, Store};

use pdf_application::export::{ExportControl, ExportError};
use serde::{Deserialize, Serialize};
use std::{
    io::Write,
    path::{Path, PathBuf},
    process::Command,
    sync::{Arc, OnceLock},
};

pub const MAX_IMAGE_PIXELS: usize = 12_000_000;
pub const MAX_WORDS: usize = 4096;
const MAX_TEXT_BYTES: usize = 64 * 1024;
const MAX_OUTPUT_BYTES: usize = 2 * 1024 * 1024;
pub const ENGINE_ID: &str = "tesseract-5.5.4-fast-4.1.0-v1";
const VERSION: &str = "tesseract 5.5.4";

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub enum Language {
    Indonesian,
    English,
    #[default]
    IndonesianEnglish,
}

impl Language {
    pub fn code(self) -> &'static str {
        match self {
            Self::Indonesian => "ind",
            Self::English => "eng",
            Self::IndonesianEnglish => "ind+eng",
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum OcrError {
    #[error("Mesin OCR lokal tidak tersedia atau versinya berbeda.")]
    EngineUnavailable,
    #[error("Model bahasa OCR tidak tersedia: {0}.")]
    ModelMissing(String),
    #[error("Gambar atau hasil OCR melampaui batas sumber daya.")]
    Limit,
    #[error("OCR gagal membaca gambar halaman.")]
    Recognition,
    #[error("Format hasil mesin OCR tidak dikenali.")]
    InvalidOutput,
    #[error("OCR melewati batas waktu.")]
    Timeout,
    #[error("OCR dibatalkan.")]
    Cancelled,
}

impl From<ExportError> for OcrError {
    fn from(error: ExportError) -> Self {
        match error {
            ExportError::ResourceLimit => Self::Limit,
            ExportError::Timeout => Self::Timeout,
            ExportError::Cancelled => Self::Cancelled,
            ExportError::Dependency => Self::EngineUnavailable,
            _ => Self::Recognition,
        }
    }
}

pub struct Raster {
    pub width: u32,
    pub height: u32,
    /// Tightly packed RGBA8.
    pub rgba: Vec<u8>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Word {
    pub text: String,
    /// Monotonically increasing line number in the recognizer's reading order.
    pub line: u32,
    /// Left, top, right and bottom, normalized to the supplied raster.
    pub rect: [f32; 4],
    pub confidence: f32,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Page {
    pub words: Vec<Word>,
}

impl Page {
    pub fn valid(&self) -> bool {
        self.words.len() <= MAX_WORDS
            && self.words.iter().map(|w| w.text.len()).sum::<usize>() <= MAX_TEXT_BYTES
            && self.words.windows(2).all(|w| w[0].line <= w[1].line)
            && self.words.iter().all(|w| {
                !w.text.is_empty()
                    && !w.text.chars().any(|c| c.is_control() || c.is_whitespace())
                    && w.rect.iter().all(|v| (0. ..=1.).contains(v))
                    && w.rect[0] < w.rect[2]
                    && w.rect[1] < w.rect[3]
                    && (0. ..=100.).contains(&w.confidence)
            })
    }
}

pub trait Recognizer: Send + Sync {
    fn recognize(
        &self,
        image: Raster,
        language: Language,
        control: &ExportControl,
    ) -> Result<Page, OcrError>;
}

#[derive(Clone)]
pub struct Tesseract {
    directory: PathBuf,
    verified: Arc<OnceLock<()>>,
}

impl Tesseract {
    pub fn new(sidecars: &Path) -> Self {
        Self {
            directory: sidecars.join("ocr"),
            verified: Arc::default(),
        }
    }

    fn command(&self) -> Result<Command, OcrError> {
        let path = self.directory.join("tesseract.exe");
        if !path.is_absolute() || !path.is_file() {
            return Err(OcrError::EngineUnavailable);
        }
        let mut command = Command::new(path);
        command
            .current_dir(&self.directory)
            .env_remove("TESSDATA_PREFIX")
            .env("OMP_THREAD_LIMIT", "1");
        Ok(command)
    }

    pub fn check(&self, language: Language, control: &ExportControl) -> Result<(), OcrError> {
        if self.verified.get().is_none() {
            let mut command = self.command()?;
            command.arg("--version");
            let report = pdf_platform::run(command, 4096, control)?;
            if report.code != Some(0)
                || String::from_utf8_lossy(&report.stdout).lines().next() != Some(VERSION)
            {
                return Err(OcrError::EngineUnavailable);
            }
            let _ = self.verified.set(());
        }
        for code in language.code().split('+') {
            if !self
                .directory
                .join("tessdata")
                .join(format!("{code}.traineddata"))
                .is_file()
            {
                return Err(OcrError::ModelMissing(code.into()));
            }
        }
        Ok(())
    }
}

impl Recognizer for Tesseract {
    fn recognize(
        &self,
        image: Raster,
        language: Language,
        control: &ExportControl,
    ) -> Result<Page, OcrError> {
        self.check(language, control)?;
        let (width, height) = (image.width, image.height);
        let pgm = grayscale_pgm(image)?;
        let mut command = self.command()?;
        command.args([
            "stdin",
            "stdout",
            "--tessdata-dir",
            "tessdata",
            "-l",
            language.code(),
            "--oem",
            "1",
            "--psm",
            "3",
            "--dpi",
            "300",
            "tsv",
        ]);
        let result = pdf_platform::run_with_input(command, pgm, MAX_OUTPUT_BYTES, control)?;
        if result.code != Some(0) {
            return Err(OcrError::Recognition);
        }
        parse_tsv(&result.stdout, width, height)
    }
}

fn grayscale_pgm(image: Raster) -> Result<Vec<u8>, OcrError> {
    let Raster {
        width,
        height,
        rgba,
    } = image;
    let area = u64::from(width) * u64::from(height);
    if width == 0 || height == 0 || area > MAX_IMAGE_PIXELS as u64 || rgba.len() as u64 != area * 4
    {
        return Err(OcrError::Limit);
    }
    let mut output = Vec::with_capacity(area as usize + 40);
    write!(output, "P5\n{width} {height}\n255\n").map_err(|_| OcrError::Recognition)?;
    output.extend(rgba.as_chunks::<4>().0.iter().map(|[r, g, b, a]| {
        let gray = (77 * u32::from(*r) + 150 * u32::from(*g) + 29 * u32::from(*b)) >> 8;
        let alpha = u32::from(*a);
        ((gray * alpha + 255 * (255 - alpha) + 127) / 255) as u8
    }));
    Ok(output)
}

fn parse_tsv(output: &[u8], width: u32, height: u32) -> Result<Page, OcrError> {
    let mut lines = std::str::from_utf8(output)
        .map_err(|_| OcrError::InvalidOutput)?
        .lines();
    if lines.next()
        != Some(
            "level\tpage_num\tblock_num\tpar_num\tline_num\tword_num\tleft\ttop\twidth\theight\tconf\ttext",
        )
    {
        return Err(OcrError::InvalidOutput);
    }
    let mut page = Page::default();
    let mut prior_line = None;
    let mut line = 0;
    let mut text_len = 0;
    for row in lines.filter(|s| s.starts_with("5\t")) {
        let mut fields = row.splitn(12, '\t');
        let columns: [Option<&str>; 12] = std::array::from_fn(|_| fields.next());
        let [
            Some("5"),
            Some("1"),
            Some(block),
            Some(paragraph),
            Some(row_line),
            Some(_),
            Some(x),
            Some(y),
            Some(w),
            Some(h),
            Some(conf),
            Some(text),
        ] = columns
        else {
            return Err(OcrError::InvalidOutput);
        };
        let text = text.trim();
        if text.is_empty() {
            continue;
        }
        let [x, y, w, h] = [x, y, w, h].map(|v| v.parse::<u32>().ok());
        let [Some(x), Some(y), Some(w), Some(h)] = [x, y, w, h] else {
            return Err(OcrError::InvalidOutput);
        };
        let right = x.saturating_add(w);
        let bottom = y.saturating_add(h);
        if right > width || bottom > height || w == 0 || h == 0 {
            return Err(OcrError::InvalidOutput);
        }
        let key = Some((block, paragraph, row_line));
        if key != prior_line {
            line += 1;
            prior_line = key;
        }
        text_len += text.len();
        if page.words.len() >= MAX_WORDS || text_len > MAX_TEXT_BYTES {
            return Err(OcrError::Limit);
        }
        let confidence = conf.parse().map_err(|_| OcrError::InvalidOutput)?;
        let (width, height) = (width as f32, height as f32);
        // Some model tokens include internal spaces. Keep one box per token
        // part, distributing its width so Find and copying share the same text.
        let mut offset = 0;
        let char_count = text.chars().count() as f32;
        for part in text.split_whitespace() {
            let tail = &text[offset..];
            let start = tail.find(part).ok_or(OcrError::InvalidOutput)? + offset;
            offset = start + part.len();
            let left = text[..start].chars().count() as f32 / char_count;
            let right = left + part.chars().count() as f32 / char_count;
            if page.words.len() >= MAX_WORDS {
                return Err(OcrError::Limit);
            }
            page.words.push(Word {
                text: part.into(),
                line,
                rect: [
                    (x as f32 + w as f32 * left) / width,
                    y as f32 / height,
                    ((x as f32 + w as f32 * right) / width).min(1.),
                    bottom as f32 / height,
                ],
                confidence,
            });
        }
    }
    if !page.valid() {
        return Err(OcrError::InvalidOutput);
    }
    Ok(page)
}

#[cfg(test)]
mod tests {
    use super::*;
    const HEADER: &str = "level\tpage_num\tblock_num\tpar_num\tline_num\tword_num\tleft\ttop\twidth\theight\tconf\ttext\n";

    #[test]
    fn tsv_has_order_normalized_boxes_and_bounded_results() -> Result<(), OcrError> {
        let row = "5\t1\t1\t1\t1\t1\t10\t20\t30\t10\t90.25\tbuku";
        let tsv = format!("{HEADER}{row}\n{}", row.replace("buku", "ringan"));
        let result = parse_tsv(tsv.as_bytes(), 100, 100)?;
        assert_eq!(result.words[0].rect, [0.1, 0.2, 0.4, 0.3]);
        assert_eq!(result.words[1].text, "ringan");
        assert_eq!(result.words[1].line, result.words[0].line);
        assert_eq!(result.words[0].confidence, 90.25);
        assert!(parse_tsv(HEADER.as_bytes(), 20, 20)?.words.is_empty());
        assert!(matches!(
            parse_tsv(tsv.as_bytes(), 20, 100),
            Err(OcrError::InvalidOutput)
        ));
        assert!(matches!(
            parse_tsv(b"wrong", 100, 100),
            Err(OcrError::InvalidOutput)
        ));
        Ok(())
    }

    #[test]
    fn grayscale_pgm_flattens_transparency_and_checks_size() -> Result<(), OcrError> {
        assert_eq!(
            grayscale_pgm(Raster {
                width: 2,
                height: 1,
                rgba: vec![0, 0, 0, 0, 0, 0, 0, 255]
            })?,
            b"P5\n2 1\n255\n\xff\x00"
        );
        assert!(matches!(
            grayscale_pgm(Raster {
                width: u32::MAX,
                height: u32::MAX,
                rgba: vec![]
            }),
            Err(OcrError::Limit)
        ));
        Ok(())
    }
}
