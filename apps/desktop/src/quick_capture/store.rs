//! Disk-backed Quick Capture session store and exporters (1 PDF, Folder Gambar, OCR -> TXT).
use super::win_capture::{CaptureRegion, CapturedFrame, HotkeyPreset, fnv1a_rgba_hash};
use image::{ColorType, ImageEncoder, RgbaImage, codecs::jpeg::JpegEncoder, imageops::FilterType};
use pdf_application::{
    export::ExportControl,
    images_to_pdf::{
        ImageMargin, ImageOrientation, ImagePageSize, ImagesToPdfOptions, ImagesToPdfPlan,
        ImagesToPdfReceipt, MAX_IMAGES, run_images_to_pdf,
    },
    split::validate_folder,
};
use pdf_engine::PdfiumRenderer;
use pdf_ocr::{Language, MAX_IMAGE_PIXELS as OCR_MAX_PIXELS, Page as OcrPage, Raster, Recognizer};
use pdf_platform::{
    images_to_pdf::LocalImagePdfWriter, split::publish_directory_no_clobber,
    write_text_file_no_clobber,
};
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

pub const MAX_CAPTURE_ITEMS: usize = MAX_IMAGES;
const THUMB_MAX_WIDTH: u32 = 180;
const THUMB_MAX_HEIGHT: u32 = 135;
const SESSION_VERSION: u64 = 1;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CaptureItemMeta {
    pub id: u64,
    pub width: u32,
    pub height: u32,
    pub hash: u64,
    pub created_ms: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CaptureSessionData {
    pub next_id: u64,
    pub region: Option<CaptureRegion>,
    pub monitor_index: usize,
    pub hotkey: HotkeyPreset,
    pub duplicate_guard: bool,
    pub items: Vec<CaptureItemMeta>,
}

impl Default for CaptureSessionData {
    fn default() -> Self {
        Self {
            next_id: 1,
            region: None,
            monitor_index: 1,
            hotkey: HotkeyPreset::F9,
            duplicate_guard: true,
            items: Vec::new(),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PdfExportPreset {
    #[default]
    AiCompactJpeg85,
    LosslessPng,
}

impl PdfExportPreset {
    pub fn label(self) -> &'static str {
        match self {
            Self::AiCompactJpeg85 => "Hemat AI (JPEG 85%)",
            Self::LosslessPng => "Lossless (PNG)",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ImageFolderFormat {
    #[default]
    Jpeg90,
    Png,
}

impl ImageFolderFormat {
    pub fn label(self) -> &'static str {
        match self {
            Self::Jpeg90 => "JPEG 90%",
            Self::Png => "PNG Lossless",
        }
    }

    pub fn extension(self) -> &'static str {
        match self {
            Self::Jpeg90 => "jpg",
            Self::Png => "png",
        }
    }
}

#[derive(Clone, Debug)]
pub enum AddItemOutcome {
    Added(CaptureItemMeta),
    DuplicateDetected {
        last_index: usize,
    },
    LimitReached,
}

#[derive(Clone, Debug)]
pub struct CaptureSessionStore {
    root: PathBuf,
}

impl CaptureSessionStore {
    pub fn new(state_dir: Option<&Path>) -> Self {
        let base = state_dir
            .map(Path::to_path_buf)
            .unwrap_or_else(|| std::env::temp_dir().join("origo-pdf-studio"));
        Self {
            root: base.join("quick-capture"),
        }
    }

    #[cfg(test)]
    pub fn from_root(root: PathBuf) -> Self {
        Self { root }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    fn items_dir(&self) -> PathBuf {
        self.root.join("items")
    }

    fn thumbs_dir(&self) -> PathBuf {
        self.root.join("thumbs")
    }

    fn meta_path(&self) -> PathBuf {
        self.root.join("session.json")
    }

    pub fn item_path(&self, id: u64) -> PathBuf {
        self.items_dir().join(format!("{id:06}.png"))
    }

    pub fn thumb_path(&self, id: u64) -> PathBuf {
        self.thumbs_dir().join(format!("{id:06}.png"))
    }

    fn ensure_dirs(&self) -> anyhow::Result<()> {
        fs::create_dir_all(self.items_dir())?;
        fs::create_dir_all(self.thumbs_dir())?;
        Ok(())
    }

    pub fn load(&self) -> CaptureSessionData {
        let Ok(bytes) = fs::read(self.meta_path()) else {
            return CaptureSessionData::default();
        };
        let Ok(value) = serde_json::from_slice::<serde_json::Value>(&bytes) else {
            return CaptureSessionData::default();
        };
        if value["version"].as_u64() != Some(SESSION_VERSION) {
            return CaptureSessionData::default();
        }
        let next_id = value["nextId"].as_u64().unwrap_or(1).max(1);
        let monitor_index = value["monitorIndex"].as_u64().unwrap_or(1).max(1) as usize;
        let hotkey = value["hotkey"]
            .as_str()
            .map(HotkeyPreset::from_code)
            .unwrap_or_default();
        let duplicate_guard = value["duplicateGuard"].as_bool().unwrap_or(true);
        let region = value["region"].as_object().and_then(|obj| {
            let monitor_index = obj.get("monitorIndex")?.as_u64()?.max(1) as usize;
            let x = obj.get("x")?.as_u64()? as u32;
            let y = obj.get("y")?.as_u64()? as u32;
            let width = obj.get("width")?.as_u64()? as u32;
            let height = obj.get("height")?.as_u64()? as u32;
            if width >= 4 && height >= 4 {
                Some(CaptureRegion {
                    monitor_index,
                    x,
                    y,
                    width,
                    height,
                })
            } else {
                None
            }
        });
        let mut items = Vec::new();
        if let Some(arr) = value["items"].as_array() {
            for entry in arr.iter().take(MAX_CAPTURE_ITEMS) {
                let Some(id) = entry["id"].as_u64() else {
                    continue;
                };
                let Some(width) = entry["width"].as_u64().map(|v| v as u32) else {
                    continue;
                };
                let Some(height) = entry["height"].as_u64().map(|v| v as u32) else {
                    continue;
                };
                let hash = entry["hash"]
                    .as_str()
                    .and_then(|s| s.parse::<u64>().ok())
                    .unwrap_or(0);
                let created_ms = entry["createdMs"].as_u64().unwrap_or(0);
                if self.item_path(id).is_file() {
                    items.push(CaptureItemMeta {
                        id,
                        width,
                        height,
                        hash,
                        created_ms,
                    });
                }
            }
        }
        let max_existing = items.iter().map(|i| i.id).max().unwrap_or(0);
        CaptureSessionData {
            next_id: next_id.max(max_existing.saturating_add(1)),
            region,
            monitor_index,
            hotkey,
            duplicate_guard,
            items,
        }
    }

    pub fn save(&self, data: &CaptureSessionData) -> anyhow::Result<()> {
        self.ensure_dirs()?;
        let region_json = data.region.map(|r| {
            serde_json::json!({
                "monitorIndex": r.monitor_index,
                "x": r.x,
                "y": r.y,
                "width": r.width,
                "height": r.height,
            })
        });
        let items_json: Vec<_> = data
            .items
            .iter()
            .map(|item| {
                serde_json::json!({
                    "id": item.id,
                    "width": item.width,
                    "height": item.height,
                    "hash": item.hash.to_string(),
                    "createdMs": item.created_ms,
                })
            })
            .collect();
        let payload = serde_json::json!({
            "version": SESSION_VERSION,
            "nextId": data.next_id,
            "monitorIndex": data.monitor_index,
            "hotkey": data.hotkey.code(),
            "duplicateGuard": data.duplicate_guard,
            "region": region_json,
            "items": items_json,
        });
        let bytes = serde_json::to_vec_pretty(&payload)?;
        let temp_path = self.root.join("session.json.tmp");
        {
            let mut file = fs::File::create(&temp_path)?;
            file.write_all(&bytes)?;
            file.sync_all()?;
        }
        let _ = fs::remove_file(self.meta_path());
        fs::rename(&temp_path, self.meta_path())?;
        Ok(())
    }

    pub fn add_capture(
        &self,
        data: &mut CaptureSessionData,
        frame: &CapturedFrame,
        force: bool,
    ) -> anyhow::Result<AddItemOutcome> {
        if data.items.len() >= MAX_CAPTURE_ITEMS {
            return Ok(AddItemOutcome::LimitReached);
        }
        let hash = fnv1a_rgba_hash(frame.width, frame.height, &frame.rgba);
        if !force
            && data.duplicate_guard
            && let Some(last) = data.items.last()
            && last.hash == hash
            && last.width == frame.width
            && last.height == frame.height
        {
            return Ok(AddItemOutcome::DuplicateDetected {
                last_index: data.items.len(),
            });
        }

        let id = data.next_id;
        data.next_id = data.next_id.saturating_add(1);
        self.write_item_and_thumb(id, frame)?;
        let meta = CaptureItemMeta {
            id,
            width: frame.width,
            height: frame.height,
            hash,
            created_ms: now_ms(),
        };
        data.items.push(meta.clone());
        self.save(data)?;
        Ok(AddItemOutcome::Added(meta))
    }

    pub fn replace_capture(
        &self,
        data: &mut CaptureSessionData,
        index: usize,
        frame: &CapturedFrame,
    ) -> anyhow::Result<CaptureItemMeta> {
        anyhow::ensure!(index < data.items.len(), "Nomor tangkapan tidak valid.");
        let id = data.items[index].id;
        let hash = fnv1a_rgba_hash(frame.width, frame.height, &frame.rgba);
        self.write_item_and_thumb(id, frame)?;
        let meta = CaptureItemMeta {
            id,
            width: frame.width,
            height: frame.height,
            hash,
            created_ms: now_ms(),
        };
        data.items[index] = meta.clone();
        self.save(data)?;
        Ok(meta)
    }

    pub fn delete_item(&self, data: &mut CaptureSessionData, index: usize) -> anyhow::Result<()> {
        if index >= data.items.len() {
            return Ok(());
        }
        let removed = data.items.remove(index);
        let _ = fs::remove_file(self.item_path(removed.id));
        let _ = fs::remove_file(self.thumb_path(removed.id));
        self.save(data)?;
        Ok(())
    }

    pub fn undo_last(&self, data: &mut CaptureSessionData) -> anyhow::Result<bool> {
        if data.items.is_empty() {
            return Ok(false);
        }
        let last = data.items.len() - 1;
        self.delete_item(data, last)?;
        Ok(true)
    }

    pub fn move_item(
        &self,
        data: &mut CaptureSessionData,
        from: usize,
        to: usize,
    ) -> anyhow::Result<bool> {
        if from >= data.items.len() || to >= data.items.len() || from == to {
            return Ok(false);
        }
        data.items.swap(from, to);
        self.save(data)?;
        Ok(true)
    }

    pub fn clear_items(&self, data: &mut CaptureSessionData) -> anyhow::Result<()> {
        for item in data.items.drain(..) {
            let _ = fs::remove_file(self.item_path(item.id));
            let _ = fs::remove_file(self.thumb_path(item.id));
        }
        self.save(data)?;
        Ok(())
    }

    pub fn load_thumb_rgba(&self, id: u64) -> Option<CapturedFrame> {
        let path = self.thumb_path(id);
        let img = image::open(&path)
            .or_else(|_| image::open(self.item_path(id)))
            .ok()?
            .into_rgba8();
        let (width, height) = img.dimensions();
        Some(CapturedFrame {
            width,
            height,
            rgba: img.into_raw(),
        })
    }

    pub fn load_item_rgba(&self, id: u64) -> anyhow::Result<CapturedFrame> {
        let img = image::open(self.item_path(id))
            .map_err(|e| anyhow::anyhow!("Gagal membaca gambar tangkapan: {e}"))?
            .into_rgba8();
        let (width, height) = img.dimensions();
        Ok(CapturedFrame {
            width,
            height,
            rgba: img.into_raw(),
        })
    }

    fn write_item_and_thumb(&self, id: u64, frame: &CapturedFrame) -> anyhow::Result<()> {
        self.ensure_dirs()?;
        let img = RgbaImage::from_raw(frame.width, frame.height, frame.rgba.clone())
            .ok_or_else(|| anyhow::anyhow!("Data RGBA tidak sesuai ukuran."))?;
        img.save(self.item_path(id))?;
        let thumb = build_thumbnail(&img);
        thumb.save(self.thumb_path(id))?;
        Ok(())
    }
}

fn build_thumbnail(img: &RgbaImage) -> RgbaImage {
    let (w, h) = img.dimensions();
    if w <= THUMB_MAX_WIDTH && h <= THUMB_MAX_HEIGHT {
        return img.clone();
    }
    let scale = (THUMB_MAX_WIDTH as f32 / w as f32).min(THUMB_MAX_HEIGHT as f32 / h as f32);
    let tw = ((w as f32 * scale).round() as u32).max(1);
    let th = ((h as f32 * scale).round() as u32).max(1);
    image::imageops::resize(img, tw, th, FilterType::Triangle)
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

fn rgba_to_rgb_white_bg(rgba: &RgbaImage) -> Vec<u8> {
    let mut rgb = Vec::with_capacity((rgba.width() as usize) * (rgba.height() as usize) * 3);
    for p in rgba.pixels() {
        let [r, g, b, a] = p.0;
        let alpha = u16::from(a);
        let inv = 255 - alpha;
        rgb.push(((u16::from(r) * alpha + 255 * inv + 127) / 255) as u8);
        rgb.push(((u16::from(g) * alpha + 255 * inv + 127) / 255) as u8);
        rgb.push(((u16::from(b) * alpha + 255 * inv + 127) / 255) as u8);
    }
    rgb
}

fn write_jpeg_file(rgba: &RgbaImage, path: &Path, quality: u8) -> anyhow::Result<()> {
    let rgb = rgba_to_rgb_white_bg(rgba);
    let mut file = fs::File::create(path)?;
    let encoder = JpegEncoder::new_with_quality(&mut file, quality);
    encoder.write_image(&rgb, rgba.width(), rgba.height(), ColorType::Rgb8.into())?;
    file.sync_all()?;
    Ok(())
}

/// Export all captured items into a single PDF file using Origo's validated image-to-PDF writer.
pub fn export_session_to_pdf(
    store: &CaptureSessionStore,
    items: &[CaptureItemMeta],
    preset: PdfExportPreset,
    destination: &Path,
    renderer: &PdfiumRenderer,
    control: &ExportControl,
    mut on_progress: impl FnMut(usize, usize),
) -> anyhow::Result<ImagesToPdfReceipt> {
    anyhow::ensure!(!items.is_empty(), "Belum ada tangkapan soal untuk diekspor.");
    let total = items.len();
    let temp_dir = tempfile::Builder::new()
        .prefix(".origo-capture-pdf-")
        .tempdir()?;

    let mut image_paths = Vec::with_capacity(total);
    match preset {
        PdfExportPreset::LosslessPng => {
            for item in items {
                control.checkpoint()?;
                let path = store.item_path(item.id);
                anyhow::ensure!(
                    path.is_file(),
                    "File tangkapan #{} tidak ditemukan.",
                    item.id
                );
                image_paths.push(path);
            }
        }
        PdfExportPreset::AiCompactJpeg85 => {
            for (idx, item) in items.iter().enumerate() {
                control.checkpoint()?;
                let img = image::open(store.item_path(item.id))?.into_rgba8();
                let jpg_path = temp_dir.path().join(format!("soal-{:04}.jpg", idx + 1));
                write_jpeg_file(&img, &jpg_path, 85)?;
                image_paths.push(jpg_path);
                on_progress(idx + 1, total);
            }
        }
    }

    let options = ImagesToPdfOptions {
        page_size: ImagePageSize::FitImage,
        orientation: ImageOrientation::Auto,
        margin: ImageMargin::None,
    };
    let plan = ImagesToPdfPlan::new(image_paths, options)
        .map_err(|e| anyhow::anyhow!("{e}"))?;
    let receipt = run_images_to_pdf(
        &plan,
        destination,
        &LocalImagePdfWriter,
        renderer,
        control,
        |done| on_progress(done, total),
    )
    .map_err(|e| anyhow::anyhow!("{e}"))?;
    Ok(receipt)
}

/// Export all captured items into a brand-new folder (`soal-0001.jpg` / `.png`) with no-clobber publish.
pub fn export_session_to_images(
    store: &CaptureSessionStore,
    items: &[CaptureItemMeta],
    format: ImageFolderFormat,
    parent_dir: &Path,
    folder_name: &str,
    control: &ExportControl,
    mut on_progress: impl FnMut(usize, usize),
) -> anyhow::Result<PathBuf> {
    anyhow::ensure!(!items.is_empty(), "Belum ada tangkapan soal untuk diekspor.");
    validate_folder(folder_name).map_err(|e| anyhow::anyhow!("{e}"))?;
    let parent = dunce::canonicalize(parent_dir)
        .map_err(|_| anyhow::anyhow!("Folder tujuan tidak ditemukan."))?;
    let destination = pick_non_clobber_dir(&parent, folder_name)?;

    let stage_root = tempfile::Builder::new()
        .prefix(".origo-capture-imgs-")
        .tempdir_in(&parent)?;
    let ready = stage_root.path().join("ready");
    fs::create_dir(&ready)?;

    let total = items.len();
    for (idx, item) in items.iter().enumerate() {
        control.checkpoint()?;
        let out_name = format!("soal-{:04}.{}", idx + 1, format.extension());
        let out_path = ready.join(out_name);
        match format {
            ImageFolderFormat::Png => {
                fs::copy(store.item_path(item.id), &out_path)?;
                let file = fs::OpenOptions::new().read(true).write(true).open(&out_path)?;
                file.sync_all()?;
            }
            ImageFolderFormat::Jpeg90 => {
                let img = image::open(store.item_path(item.id))?.into_rgba8();
                write_jpeg_file(&img, &out_path, 90)?;
            }
        }
        on_progress(idx + 1, total);
    }
    control.checkpoint()?;
    publish_directory_no_clobber(&ready, &destination)
        .map_err(|e| anyhow::anyhow!("{e}"))?;
    Ok(destination)
}

fn pick_non_clobber_dir(parent: &Path, base_name: &str) -> anyhow::Result<PathBuf> {
    let first = parent.join(base_name);
    if fs::symlink_metadata(&first).is_err() {
        return Ok(first);
    }
    for n in 2..=999u32 {
        let candidate = parent.join(format!("{base_name}-{n}"));
        if fs::symlink_metadata(&candidate).is_err() {
            return Ok(candidate);
        }
    }
    anyhow::bail!("Folder tujuan sudah penuh, pilih nama folder lain.")
}

#[derive(Clone, Debug)]
pub struct OcrTxtReport {
    pub destination: PathBuf,
    pub total_items: usize,
    pub non_empty_items: usize,
    pub chars: usize,
}

/// Format an OCR `Page`'s word list back into readable multi-line text.
pub fn format_ocr_page_text(page: &OcrPage) -> String {
    let mut lines: Vec<String> = Vec::new();
    let mut current_line: Option<u32> = None;
    let mut buf = String::new();
    for word in &page.words {
        if current_line != Some(word.line) {
            if !buf.is_empty() {
                lines.push(std::mem::take(&mut buf));
            }
            current_line = Some(word.line);
        } else if !buf.is_empty() {
            buf.push(' ');
        }
        buf.push_str(&word.text);
    }
    if !buf.is_empty() {
        lines.push(buf);
    }
    lines.join("\n")
}

#[derive(Clone, Copy, Debug)]
pub struct OcrTxtOptions {
    pub language: Language,
    pub include_headers: bool,
}

/// Run offline OCR across all captured screenshots and save a clean UTF-8 `.txt` ready for AI study.
pub fn export_session_ocr_txt(
    store: &CaptureSessionStore,
    items: &[CaptureItemMeta],
    recognizer: &impl Recognizer,
    options: OcrTxtOptions,
    destination: &Path,
    control: &ExportControl,
    mut on_progress: impl FnMut(usize, usize),
) -> anyhow::Result<OcrTxtReport> {
    anyhow::ensure!(!items.is_empty(), "Belum ada tangkapan soal untuk di-OCR.");
    let total = items.len();
    let mut sections = Vec::with_capacity(total);
    let mut non_empty_items = 0usize;

    for (idx, item) in items.iter().enumerate() {
        control.checkpoint()?;
        let img = image::open(store.item_path(item.id))?.into_rgba8();
        let raster = prepare_ocr_raster(img);
        let page = recognizer
            .recognize(raster, options.language, control)
            .map_err(|e| anyhow::anyhow!("Soal #{}: {e}", idx + 1))?;
        let text = format_ocr_page_text(&page);
        let trimmed = text.trim();
        if !trimmed.is_empty() {
            non_empty_items += 1;
        }
        if options.include_headers {
            if trimmed.is_empty() {
                sections.push(format!("=== Soal {} ===\n(Tidak ada teks terdeteksi)", idx + 1));
            } else {
                sections.push(format!("=== Soal {} ===\n{trimmed}", idx + 1));
            }
        } else if !trimmed.is_empty() {
            sections.push(trimmed.to_owned());
        }
        on_progress(idx + 1, total);
    }

    control.checkpoint()?;
    let full_text = if sections.is_empty() {
        String::new()
    } else {
        format!("{}\n", sections.join("\n\n"))
    };
    let chars = full_text.chars().count();
    let (saved, _) = write_text_file_no_clobber(destination, &full_text, control)
        .map_err(|e| anyhow::anyhow!("{e}"))?;
    Ok(OcrTxtReport {
        destination: saved,
        total_items: total,
        non_empty_items,
        chars,
    })
}

fn prepare_ocr_raster(img: RgbaImage) -> Raster {
    let (w, h) = img.dimensions();
    let area = (w as usize).saturating_mul(h as usize);
    if area <= OCR_MAX_PIXELS {
        return Raster {
            width: w,
            height: h,
            rgba: img.into_raw(),
        };
    }
    let scale = (OCR_MAX_PIXELS as f64 / area as f64).sqrt();
    let nw = ((w as f64 * scale).floor() as u32).max(1);
    let nh = ((h as f64 * scale).floor() as u32).max(1);
    let resized = image::imageops::resize(&img, nw, nh, FilterType::Triangle);
    Raster {
        width: nw,
        height: nh,
        rgba: resized.into_raw(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pdf_ocr::{OcrError, Word};

    fn solid_frame(w: u32, h: u32, r: u8, g: u8, b: u8) -> CapturedFrame {
        let mut rgba = Vec::with_capacity((w * h * 4) as usize);
        for _ in 0..(w * h) {
            rgba.extend_from_slice(&[r, g, b, 255]);
        }
        CapturedFrame {
            width: w,
            height: h,
            rgba,
        }
    }

    #[test]
    fn session_store_persists_duplicate_guard_reorder_retake_undo() -> anyhow::Result<()> {
        let dir = tempfile::tempdir()?;
        let store = CaptureSessionStore::from_root(dir.path().join("quick-capture"));
        let mut data = store.load();
        assert!(data.items.is_empty());

        let f1 = solid_frame(64, 48, 250, 250, 250);
        let f2 = solid_frame(64, 48, 20, 40, 220);

        let r1 = store.add_capture(&mut data, &f1, false)?;
        assert!(matches!(r1, AddItemOutcome::Added(_)));

        // Identical frame blocked by duplicate guard unless forced
        let dup = store.add_capture(&mut data, &f1, false)?;
        assert!(matches!(dup, AddItemOutcome::DuplicateDetected { last_index: 1 }));

        let forced = store.add_capture(&mut data, &f1, true)?;
        assert!(matches!(forced, AddItemOutcome::Added(_)));
        assert_eq!(data.items.len(), 2);

        // Retake item 2 with different frame
        store.replace_capture(&mut data, 1, &f2)?;
        assert_ne!(data.items[0].hash, data.items[1].hash);

        // Reorder items 0 <-> 1
        let id0 = data.items[0].id;
        let id1 = data.items[1].id;
        assert!(store.move_item(&mut data, 0, 1)?);
        assert_eq!(data.items[0].id, id1);
        assert_eq!(data.items[1].id, id0);

        // Reload from disk verifies full persistence
        let reloaded = store.load();
        assert_eq!(reloaded.items.len(), 2);
        assert_eq!(reloaded.items[0].id, id1);
        assert!(store.item_path(id1).is_file());
        assert!(store.thumb_path(id1).is_file());

        // Undo removes last item & files
        assert!(store.undo_last(&mut data)?);
        assert_eq!(data.items.len(), 1);
        assert!(!store.item_path(id0).exists());
        Ok(())
    }

    struct MockRecognizer;
    impl Recognizer for MockRecognizer {
        fn recognize(
            &self,
            _image: Raster,
            _language: Language,
            _control: &ExportControl,
        ) -> Result<OcrPage, OcrError> {
            Ok(OcrPage {
                words: vec![
                    Word {
                        text: "Pancasila".into(),
                        line: 1,
                        rect: [0.0, 0.0, 0.2, 0.1],
                        confidence: 95.0,
                    },
                    Word {
                        text: "adalah".into(),
                        line: 1,
                        rect: [0.25, 0.0, 0.4, 0.1],
                        confidence: 95.0,
                    },
                    Word {
                        text: "A.".into(),
                        line: 2,
                        rect: [0.0, 0.2, 0.1, 0.3],
                        confidence: 95.0,
                    },
                    Word {
                        text: "Dasar".into(),
                        line: 2,
                        rect: [0.12, 0.2, 0.3, 0.3],
                        confidence: 95.0,
                    },
                ],
            })
        }
    }

    #[test]
    fn ocr_txt_export_formats_lines_and_headers() -> anyhow::Result<()> {
        let dir = tempfile::tempdir()?;
        let store = CaptureSessionStore::from_root(dir.path().join("quick-capture"));
        let mut data = store.load();
        store.add_capture(&mut data, &solid_frame(40, 30, 255, 255, 255), true)?;
        store.add_capture(&mut data, &solid_frame(40, 30, 240, 240, 240), true)?;

        let out_txt = dir.path().join("tryout-cpns.txt");
        let control = ExportControl::with_budget(std::time::Duration::from_secs(30));
        let mut ticks = 0usize;
        let report = export_session_ocr_txt(
            &store,
            &data.items,
            &MockRecognizer,
            OcrTxtOptions {
                language: Language::IndonesianEnglish,
                include_headers: true,
            },
            &out_txt,
            &control,
            |done, total| {
                assert_eq!(total, 2);
                ticks = done;
            },
        )?;
        assert_eq!(ticks, 2);
        assert_eq!(report.total_items, 2);
        assert_eq!(report.non_empty_items, 2);
        let text = fs::read_to_string(&report.destination)?;
        assert!(text.contains("=== Soal 1 ===\nPancasila adalah\nA. Dasar"));
        assert!(text.contains("=== Soal 2 ===\nPancasila adalah\nA. Dasar"));
        Ok(())
    }

    #[test]
    fn export_session_to_images_publishes_numbered_files() -> anyhow::Result<()> {
        let dir = tempfile::tempdir()?;
        let store = CaptureSessionStore::from_root(dir.path().join("quick-capture"));
        let mut data = store.load();
        store.add_capture(&mut data, &solid_frame(32, 32, 200, 100, 50), true)?;
        store.add_capture(&mut data, &solid_frame(32, 32, 50, 100, 200), true)?;

        let control = ExportControl::with_budget(std::time::Duration::from_secs(30));
        let saved = export_session_to_images(
            &store,
            &data.items,
            ImageFolderFormat::Jpeg90,
            dir.path(),
            "tryout-skd",
            &control,
            |_, _| {},
        )?;
        assert!(saved.join("soal-0001.jpg").is_file());
        assert!(saved.join("soal-0002.jpg").is_file());
        Ok(())
    }
}
