//! Extract original embedded images (native JPEG & lossless Flate/raw → PNG) from a PDF via QPDF JSON v2.
use flate2::read::ZlibDecoder;
use image::ImageEncoder;
use pdf_application::{
    export::{ExportControl, ExportError},
    split::{SplitError, validate_folder},
};
use pdf_platform::{run, split::publish_directory_no_clobber};
use serde_json::Value;
use std::{
    collections::HashSet,
    ffi::OsString,
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
    process::Command,
};

const MAX_IMAGE_DIM: u32 = 16_384;
const MAX_IMAGE_PIXELS: u64 = 64 * 1024 * 1024;
const MAX_DECOMPRESSED_BYTES: u64 = 128 * 1024 * 1024;
const MAX_TOTAL_OUTPUT_BYTES: u64 = 1024 * 1024 * 1024;
const MAX_JSON_BYTES: usize = 64 * 1024 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ExtractImagesOptions {
    /// Minimum width and height in pixels to extract (skips tiny icon/mask artifacts).
    pub min_dimension: u32,
    /// Skip image objects referenced exclusively as `/SMask` or `/Mask`.
    pub skip_soft_masks: bool,
}

impl Default for ExtractImagesOptions {
    fn default() -> Self {
        Self {
            min_dimension: 32,
            skip_soft_masks: true,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExtractImagesReport {
    pub destination: PathBuf,
    pub extracted: usize,
    pub jpeg_count: usize,
    pub png_count: usize,
    pub skipped: usize,
    pub total_bytes: u64,
}

#[derive(Debug, thiserror::Error)]
pub enum ExtractImagesError {
    #[error("Tidak ditemukan gambar asli yang dapat diekstrak pada PDF ini.")]
    NoImagesFound,
    #[error(
        "Total ukuran gambar hasil ekstraksi melebihi batas 1 GiB. Folder hasil tidak diterbitkan."
    )]
    OutputLimit,
    #[error("Gagal mengekstrak gambar dari PDF: {0}")]
    Failed(String),
    #[error(transparent)]
    Batch(#[from] SplitError),
    #[error(transparent)]
    Export(#[from] ExportError),
}

#[derive(Clone)]
pub struct QpdfImageExtractor {
    executable: PathBuf,
}

impl QpdfImageExtractor {
    pub fn new(sidecars: &Path) -> Self {
        Self {
            executable: sidecars.join("qpdf/bin/qpdf.exe"),
        }
    }

    pub fn extract_images(
        &self,
        source: &Path,
        destination: &Path,
        options: ExtractImagesOptions,
        control: &ExportControl,
        mut progress: impl FnMut(usize, usize),
    ) -> Result<ExtractImagesReport, ExtractImagesError> {
        control.checkpoint()?;
        let folder_name = destination
            .file_name()
            .and_then(|s| s.to_str())
            .ok_or(SplitError::Folder)?;
        validate_folder(folder_name)?;
        let parent = dunce::canonicalize(destination.parent().ok_or(SplitError::Output)?)
            .map_err(|_| SplitError::Output)?;
        let canonical_dest = parent.join(folder_name);
        match fs::symlink_metadata(&canonical_dest) {
            Ok(_) => return Err(SplitError::Exists.into()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(_) => return Err(SplitError::Output.into()),
        }

        let canonical_source = dunce::canonicalize(source).map_err(|_| ExportError::Io)?;
        let stage_dir = tempfile::Builder::new()
            .prefix(".pdf-studio-extract-img-")
            .tempdir_in(&parent)
            .map_err(|_| SplitError::Output)?;
        let work_dir = stage_dir.path().join("work");
        let results_dir = stage_dir.path().join("results");
        fs::create_dir(&work_dir).map_err(|_| SplitError::Output)?;
        fs::create_dir(&results_dir).map_err(|_| SplitError::Output)?;

        let stream_prefix = work_dir.join("stream");
        let json_doc =
            self.dump_pdf_json_with_streams(&canonical_source, &stream_prefix, control)?;
        if json_doc
            .pointer("/qpdf/0/jsonversion")
            .and_then(Value::as_u64)
            != Some(2)
        {
            return Err(ExtractImagesError::Failed(
                "Format JSON QPDF tidak didukung.".into(),
            ));
        }
        let Some(objects) = json_doc.pointer("/qpdf/1").and_then(Value::as_object) else {
            return Err(ExtractImagesError::NoImagesFound);
        };

        let mask_refs = collect_mask_references(objects);
        let mut image_keys: Vec<(u64, &str)> = objects
            .iter()
            .filter_map(|(key, val)| {
                let id = parse_obj_key_id(key)?;
                let dict = val.pointer("/stream/dict")?;
                (dict.get("/Subtype").and_then(Value::as_str) == Some("/Image"))
                    .then_some((id, key.as_str()))
            })
            .collect();
        image_keys.sort_by_key(|(id, _)| *id);

        let total_candidates = image_keys.len();
        let mut extracted = 0usize;
        let mut jpeg_count = 0usize;
        let mut png_count = 0usize;
        let mut skipped = 0usize;
        let mut total_bytes = 0u64;

        for (idx, (obj_id, key)) in image_keys.iter().enumerate() {
            control.checkpoint()?;
            progress(idx + 1, total_candidates.max(1));

            let Some(obj) = objects.get(*key) else {
                continue;
            };
            let Some(stream) = obj.get("stream").and_then(Value::as_object) else {
                skipped += 1;
                continue;
            };
            let Some(dict) = stream.get("dict").and_then(Value::as_object) else {
                skipped += 1;
                continue;
            };

            if dict.get("/ImageMask").and_then(Value::as_bool) == Some(true) {
                skipped += 1;
                continue;
            }
            let obj_ref = format!("{obj_id} 0 R");
            if options.skip_soft_masks && mask_refs.contains(&obj_ref) {
                skipped += 1;
                continue;
            }

            let Some(width) = dict
                .get("/Width")
                .and_then(Value::as_u64)
                .and_then(|v| u32::try_from(v).ok())
            else {
                skipped += 1;
                continue;
            };
            let Some(height) = dict
                .get("/Height")
                .and_then(Value::as_u64)
                .and_then(|v| u32::try_from(v).ok())
            else {
                skipped += 1;
                continue;
            };
            if width < options.min_dimension
                || height < options.min_dimension
                || width > MAX_IMAGE_DIM
                || height > MAX_IMAGE_DIM
                || u64::from(width) * u64::from(height) > MAX_IMAGE_PIXELS
            {
                skipped += 1;
                continue;
            }

            let Some(datafile_raw) = stream.get("datafile").and_then(Value::as_str) else {
                skipped += 1;
                continue;
            };
            let datafile = resolve_datafile_path(datafile_raw, &work_dir);
            if !datafile.is_file() {
                skipped += 1;
                continue;
            }

            let filters = extract_filter_chain(dict.get("/Filter"));
            match classify_filters(&filters) {
                FilterKind::DirectJpeg => {
                    let bytes = fs::read(&datafile).map_err(|_| ExportError::Io)?;
                    if !is_valid_jpeg(&bytes) {
                        skipped += 1;
                        continue;
                    }
                    let out_path = results_dir.join(format!("image-{:04}.jpg", extracted + 1));
                    let written = write_new_file(&out_path, &bytes)?;
                    total_bytes = add_output_bytes(total_bytes, written)?;
                    extracted += 1;
                    jpeg_count += 1;
                }
                FilterKind::FlateJpeg => {
                    let Ok(bytes) = decompress_zlib_file(&datafile, MAX_DECOMPRESSED_BYTES) else {
                        skipped += 1;
                        continue;
                    };
                    if !is_valid_jpeg(&bytes) {
                        skipped += 1;
                        continue;
                    }
                    let out_path = results_dir.join(format!("image-{:04}.jpg", extracted + 1));
                    let written = write_new_file(&out_path, &bytes)?;
                    total_bytes = add_output_bytes(total_bytes, written)?;
                    extracted += 1;
                    jpeg_count += 1;
                }
                FilterKind::RawPixels { flate } => {
                    let bpc = dict
                        .get("/BitsPerComponent")
                        .and_then(Value::as_u64)
                        .unwrap_or(8);
                    if bpc != 8 {
                        skipped += 1;
                        continue;
                    }
                    let Some(color) = resolve_color_space(dict.get("/ColorSpace"), objects) else {
                        skipped += 1;
                        continue;
                    };
                    let predictor = parse_decode_parms_predictor(dict.get("/DecodeParms"));
                    if !matches!(predictor, Predictor::None | Predictor::Png) {
                        skipped += 1;
                        continue;
                    }
                    let raw_pixels = if flate {
                        let Ok(decoded) = decompress_zlib_file(&datafile, MAX_DECOMPRESSED_BYTES)
                        else {
                            skipped += 1;
                            continue;
                        };
                        decoded
                    } else {
                        let Ok(meta) = fs::metadata(&datafile) else {
                            skipped += 1;
                            continue;
                        };
                        if meta.len() > MAX_DECOMPRESSED_BYTES {
                            skipped += 1;
                            continue;
                        }
                        fs::read(&datafile).map_err(|_| ExportError::Io)?
                    };

                    let channels = color.channels();
                    let pixels = match predictor {
                        Predictor::None => raw_pixels,
                        Predictor::Png => {
                            let Some(unfiltered) = undo_png_predictor(
                                &raw_pixels,
                                width as usize,
                                height as usize,
                                channels,
                            ) else {
                                skipped += 1;
                                continue;
                            };
                            unfiltered
                        }
                        Predictor::Unsupported => {
                            skipped += 1;
                            continue;
                        }
                    };
                    let expected_len = width as usize * height as usize * channels;
                    if pixels.len() < expected_len {
                        skipped += 1;
                        continue;
                    }
                    let pixels = &pixels[..expected_len];

                    let smask_alpha = dict
                        .get("/SMask")
                        .and_then(Value::as_str)
                        .and_then(|r| load_smask_alpha(r, width, height, objects, &work_dir));

                    let out_path = results_dir.join(format!("image-{:04}.png", extracted + 1));
                    let Ok(written) = write_pixels_as_png(
                        &out_path,
                        width,
                        height,
                        color,
                        pixels,
                        smask_alpha.as_deref(),
                        dict.get("/Decode"),
                    ) else {
                        skipped += 1;
                        continue;
                    };
                    total_bytes = add_output_bytes(total_bytes, written)?;
                    extracted += 1;
                    png_count += 1;
                }
                FilterKind::Unsupported => {
                    skipped += 1;
                }
            }
        }

        if extracted == 0 {
            return Err(ExtractImagesError::NoImagesFound);
        }

        control.checkpoint()?;
        publish_directory_no_clobber(&results_dir, &canonical_dest)?;
        Ok(ExtractImagesReport {
            destination: canonical_dest,
            extracted,
            jpeg_count,
            png_count,
            skipped,
            total_bytes,
        })
    }

    fn dump_pdf_json_with_streams(
        &self,
        source: &Path,
        stream_prefix: &Path,
        control: &ExportControl,
    ) -> Result<Value, ExtractImagesError> {
        let mut prefix_arg = OsString::from("--json-stream-prefix=");
        prefix_arg.push(stream_prefix.as_os_str());
        let mut command = Command::new(&self.executable);
        command.args([
            OsString::from("--json=2"),
            OsString::from("--json-stream-data=file"),
            prefix_arg,
            source.as_os_str().to_owned(),
        ]);
        let output = run(command, MAX_JSON_BYTES, control)?;
        // QPDF exit code 0 = ok, 3 = ok with warnings.
        if !matches!(output.code, Some(0 | 3)) {
            return Err(ExtractImagesError::Failed(
                "QPDF gagal membaca struktur gambar PDF.".into(),
            ));
        }
        serde_json::from_slice(&output.stdout).map_err(|_| {
            ExtractImagesError::Failed("Output JSON QPDF tidak valid atau terpotong.".into())
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum FilterKind {
    DirectJpeg,
    FlateJpeg,
    RawPixels { flate: bool },
    Unsupported,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ColorKind {
    Gray,
    Rgb,
    Cmyk,
}

impl ColorKind {
    fn channels(self) -> usize {
        match self {
            Self::Gray => 1,
            Self::Rgb => 3,
            Self::Cmyk => 4,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Predictor {
    None,
    Png,
    Unsupported,
}

fn parse_obj_key_id(key: &str) -> Option<u64> {
    let rest = key.strip_prefix("obj:")?;
    let id_part = rest.split_whitespace().next()?;
    id_part.parse::<u64>().ok()
}

fn collect_mask_references(objects: &serde_json::Map<String, Value>) -> HashSet<String> {
    let mut masks = HashSet::new();
    for val in objects.values() {
        let Some(dict) = val.pointer("/stream/dict").and_then(Value::as_object) else {
            continue;
        };
        for key in ["/SMask", "/Mask"] {
            if let Some(r) = dict.get(key).and_then(Value::as_str)
                && r.ends_with(" R")
            {
                masks.insert(r.to_owned());
            }
        }
    }
    masks
}

fn extract_filter_chain(val: Option<&Value>) -> Vec<&str> {
    match val {
        None | Some(Value::Null) => Vec::new(),
        Some(Value::String(s)) => vec![s.as_str()],
        Some(Value::Array(arr)) => arr.iter().filter_map(Value::as_str).collect(),
        _ => vec!["/Unsupported"],
    }
}

fn classify_filters(filters: &[&str]) -> FilterKind {
    match filters {
        [] => FilterKind::RawPixels { flate: false },
        ["/DCTDecode"] => FilterKind::DirectJpeg,
        ["/FlateDecode", "/DCTDecode"] => FilterKind::FlateJpeg,
        ["/FlateDecode"] => FilterKind::RawPixels { flate: true },
        _ => FilterKind::Unsupported,
    }
}

fn resolve_color_space(
    val: Option<&Value>,
    objects: &serde_json::Map<String, Value>,
) -> Option<ColorKind> {
    match val {
        Some(Value::String(s)) => match s.as_str() {
            "/DeviceGray" | "/G" => Some(ColorKind::Gray),
            "/DeviceRGB" | "/RGB" => Some(ColorKind::Rgb),
            "/DeviceCMYK" | "/CMYK" => Some(ColorKind::Cmyk),
            r if r.ends_with(" R") => {
                let target = objects.get(&format!("obj:{r}"))?.get("value")?;
                resolve_color_space(Some(target), objects)
            }
            _ => None,
        },
        Some(Value::Array(arr)) => {
            let head = arr.first().and_then(Value::as_str)?;
            match head {
                "/CalGray" => Some(ColorKind::Gray),
                "/CalRGB" => Some(ColorKind::Rgb),
                "/ICCBased" => {
                    let ref_str = arr.get(1).and_then(Value::as_str)?;
                    let n = objects
                        .get(&format!("obj:{ref_str}"))?
                        .pointer("/stream/dict//N")
                        .or_else(|| {
                            objects
                                .get(&format!("obj:{ref_str}"))
                                .and_then(|o| o.pointer("/stream/dict"))
                                .and_then(|d| d.get("/N"))
                        })
                        .and_then(Value::as_u64)?;
                    match n {
                        1 => Some(ColorKind::Gray),
                        3 => Some(ColorKind::Rgb),
                        4 => Some(ColorKind::Cmyk),
                        _ => None,
                    }
                }
                _ => None,
            }
        }
        _ => None,
    }
}

fn parse_decode_parms_predictor(val: Option<&Value>) -> Predictor {
    let pred = match val {
        None | Some(Value::Null) => return Predictor::None,
        Some(Value::Object(map)) => map.get("/Predictor").and_then(Value::as_u64).unwrap_or(1),
        Some(Value::Array(arr)) => arr
            .first()
            .and_then(Value::as_object)
            .and_then(|m| m.get("/Predictor"))
            .and_then(Value::as_u64)
            .unwrap_or(1),
        _ => return Predictor::Unsupported,
    };
    match pred {
        0 | 1 => Predictor::None,
        10..=15 => Predictor::Png,
        _ => Predictor::Unsupported,
    }
}

fn resolve_datafile_path(raw: &str, work_dir: &Path) -> PathBuf {
    let p = PathBuf::from(raw);
    if p.is_absolute() { p } else { work_dir.join(p) }
}

fn is_valid_jpeg(bytes: &[u8]) -> bool {
    bytes.len() > 16 && bytes[0] == 0xFF && bytes[1] == 0xD8
}

fn decompress_zlib_file(path: &Path, max_bytes: u64) -> Result<Vec<u8>, std::io::Error> {
    let file = File::open(path)?;
    let mut decoder = ZlibDecoder::new(file).take(max_bytes.saturating_add(1));
    let mut out = Vec::new();
    decoder.read_to_end(&mut out)?;
    if out.len() as u64 > max_bytes {
        return Err(std::io::Error::other("decompressed stream exceeds limit"));
    }
    Ok(out)
}

/// Undo PNG row predictors (PDF `/Predictor` 10..=15) for 8-bit samples.
pub fn undo_png_predictor(
    data: &[u8],
    width: usize,
    height: usize,
    channels: usize,
) -> Option<Vec<u8>> {
    let row_bytes = width.checked_mul(channels)?;
    let stride = row_bytes.checked_add(1)?;
    let required = stride.checked_mul(height)?;
    if data.len() < required {
        return None;
    }
    let mut out = vec![0u8; row_bytes.checked_mul(height)?];
    for row in 0..height {
        let filter = data[row * stride];
        let src = &data[row * stride + 1..(row + 1) * stride];
        let dst_start = row * row_bytes;
        for col in 0..row_bytes {
            let x = src[col];
            let left = if col >= channels {
                out[dst_start + col - channels]
            } else {
                0
            };
            let up = if row > 0 {
                out[dst_start - row_bytes + col]
            } else {
                0
            };
            let up_left = if row > 0 && col >= channels {
                out[dst_start - row_bytes + col - channels]
            } else {
                0
            };
            let val = match filter {
                0 => x,
                1 => x.wrapping_add(left),
                2 => x.wrapping_add(up),
                3 => x.wrapping_add(((u16::from(left) + u16::from(up)) / 2) as u8),
                4 => x.wrapping_add(paeth_predictor(left, up, up_left)),
                _ => return None,
            };
            out[dst_start + col] = val;
        }
    }
    Some(out)
}

fn paeth_predictor(a: u8, b: u8, c: u8) -> u8 {
    let ia = i32::from(a);
    let ib = i32::from(b);
    let ic = i32::from(c);
    let p = ia + ib - ic;
    let pa = (p - ia).abs();
    let pb = (p - ib).abs();
    let pc = (p - ic).abs();
    if pa <= pb && pa <= pc {
        a
    } else if pb <= pc {
        b
    } else {
        c
    }
}

fn load_smask_alpha(
    ref_str: &str,
    expected_w: u32,
    expected_h: u32,
    objects: &serde_json::Map<String, Value>,
    work_dir: &Path,
) -> Option<Vec<u8>> {
    let obj = objects.get(&format!("obj:{ref_str}"))?;
    let stream = obj.get("stream")?.as_object()?;
    let dict = stream.get("dict")?.as_object()?;
    let w = dict.get("/Width").and_then(Value::as_u64)? as u32;
    let h = dict.get("/Height").and_then(Value::as_u64)? as u32;
    if w != expected_w || h != expected_h {
        return None;
    }
    if dict
        .get("/BitsPerComponent")
        .and_then(Value::as_u64)
        .unwrap_or(8)
        != 8
    {
        return None;
    }
    let filters = extract_filter_chain(dict.get("/Filter"));
    let FilterKind::RawPixels { flate } = classify_filters(&filters) else {
        return None;
    };
    let predictor = parse_decode_parms_predictor(dict.get("/DecodeParms"));
    let datafile = resolve_datafile_path(stream.get("datafile")?.as_str()?, work_dir);
    let raw = if flate {
        decompress_zlib_file(&datafile, MAX_DECOMPRESSED_BYTES).ok()?
    } else {
        fs::read(&datafile).ok()?
    };
    let pixels = match predictor {
        Predictor::None => raw,
        Predictor::Png => undo_png_predictor(&raw, w as usize, h as usize, 1)?,
        Predictor::Unsupported => return None,
    };
    let expected = w as usize * h as usize;
    (pixels.len() >= expected).then(|| pixels[..expected].to_vec())
}

fn is_inverted_cmyk_decode(decode: Option<&Value>) -> bool {
    let Some(arr) = decode.and_then(Value::as_array) else {
        return false;
    };
    if arr.len() != 8 {
        return false;
    }
    arr.first().and_then(Value::as_f64) == Some(1.0)
        && arr.get(1).and_then(Value::as_f64) == Some(0.0)
}

fn write_pixels_as_png(
    out_path: &Path,
    width: u32,
    height: u32,
    color: ColorKind,
    pixels: &[u8],
    alpha: Option<&[u8]>,
    decode: Option<&Value>,
) -> Result<u64, ExtractImagesError> {
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(out_path)
        .map_err(|_| ExportError::Io)?;
    let encoder = image::codecs::png::PngEncoder::new(&mut file);
    match (color, alpha) {
        (ColorKind::Rgb, Some(a)) => {
            let mut rgba = Vec::with_capacity(width as usize * height as usize * 4);
            for (rgb, &alpha_byte) in pixels.as_chunks::<3>().0.iter().zip(a.iter()) {
                rgba.push(rgb[0]);
                rgba.push(rgb[1]);
                rgba.push(rgb[2]);
                rgba.push(alpha_byte);
            }
            encoder
                .write_image(&rgba, width, height, image::ExtendedColorType::Rgba8)
                .map_err(|_| ExtractImagesError::Failed("Gagal menulis PNG RGBA.".into()))?;
        }
        (ColorKind::Rgb, None) => {
            encoder
                .write_image(pixels, width, height, image::ExtendedColorType::Rgb8)
                .map_err(|_| ExtractImagesError::Failed("Gagal menulis PNG RGB.".into()))?;
        }
        (ColorKind::Gray, Some(a)) => {
            let mut la = Vec::with_capacity(width as usize * height as usize * 2);
            for (&g, &alpha_byte) in pixels.iter().zip(a.iter()) {
                la.push(g);
                la.push(alpha_byte);
            }
            encoder
                .write_image(&la, width, height, image::ExtendedColorType::La8)
                .map_err(|_| ExtractImagesError::Failed("Gagal menulis PNG Gray+Alpha.".into()))?;
        }
        (ColorKind::Gray, None) => {
            encoder
                .write_image(pixels, width, height, image::ExtendedColorType::L8)
                .map_err(|_| ExtractImagesError::Failed("Gagal menulis PNG Grayscale.".into()))?;
        }
        (ColorKind::Cmyk, _) => {
            let inverted = is_inverted_cmyk_decode(decode);
            let mut rgb = Vec::with_capacity(width as usize * height as usize * 3);
            for cmyk in pixels.as_chunks::<4>().0 {
                let (c, m, y, k) = if inverted {
                    (255 - cmyk[0], 255 - cmyk[1], 255 - cmyk[2], 255 - cmyk[3])
                } else {
                    (cmyk[0], cmyk[1], cmyk[2], cmyk[3])
                };
                let inv_k = u16::from(255 - k);
                rgb.push(((u16::from(255 - c) * inv_k) / 255) as u8);
                rgb.push(((u16::from(255 - m) * inv_k) / 255) as u8);
                rgb.push(((u16::from(255 - y) * inv_k) / 255) as u8);
            }
            encoder
                .write_image(&rgb, width, height, image::ExtendedColorType::Rgb8)
                .map_err(|_| ExtractImagesError::Failed("Gagal menulis PNG dari CMYK.".into()))?;
        }
    }
    file.sync_all().map_err(|_| ExportError::Io)?;
    let len = file.metadata().map_err(|_| ExportError::Io)?.len();
    Ok(len)
}

fn write_new_file(path: &Path, bytes: &[u8]) -> Result<u64, ExtractImagesError> {
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|_| ExportError::Io)?;
    file.write_all(bytes).map_err(|_| ExportError::Io)?;
    file.sync_all().map_err(|_| ExportError::Io)?;
    Ok(bytes.len() as u64)
}

fn add_output_bytes(current: u64, added: u64) -> Result<u64, ExtractImagesError> {
    current
        .checked_add(added)
        .filter(|b| *b <= MAX_TOTAL_OUTPUT_BYTES)
        .ok_or(ExtractImagesError::OutputLimit)
}

#[cfg(test)]
mod tests {
    use super::*;
    use flate2::{Compression, write::ZlibEncoder};

    #[test]
    fn undo_png_predictor_restores_sub_and_up_rows() {
        // 2x2 RGB image:
        // Row 0: [10, 20, 30, 15, 25, 35] -> Sub filter (1): [10, 20, 30, 5, 5, 5]
        // Row 1: [12, 22, 32, 20, 30, 40] -> Up filter (2):  [2, 2, 2, 5, 5, 5]
        let encoded = vec![
            1, 10, 20, 30, 5, 5, 5, // row 0 (Sub)
            2, 2, 2, 2, 5, 5, 5, // row 1 (Up)
        ];
        let decoded = undo_png_predictor(&encoded, 2, 2, 3);
        assert_eq!(
            decoded,
            Some(vec![10, 20, 30, 15, 25, 35, 12, 22, 32, 20, 30, 40])
        );
    }

    #[test]
    fn extract_embedded_jpeg_and_flate_png_with_real_qpdf() -> Result<(), Box<dyn std::error::Error>>
    {
        let sidecars =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../vendor/sidecars/windows-x64");
        if !sidecars.join("qpdf/bin/qpdf.exe").is_file() {
            return Ok(());
        }
        let dir = tempfile::tempdir()?;

        // Build a synthetic PDF containing:
        // - Image 1: 48x36 native JPEG (/DCTDecode)
        // - Image 2: 40x40 Flate RGB (/FlateDecode /DeviceRGB)
        let mut jpeg_bytes = Vec::new();
        let rgb_jpg = vec![180u8; 48 * 36 * 3];
        image::codecs::jpeg::JpegEncoder::new_with_quality(&mut jpeg_bytes, 90).write_image(
            &rgb_jpg,
            48,
            36,
            image::ExtendedColorType::Rgb8,
        )?;

        let rgb_flate = vec![64u8; 40 * 40 * 3];
        let mut z = ZlibEncoder::new(Vec::new(), Compression::default());
        z.write_all(&rgb_flate)?;
        let flate_bytes = z.finish()?;

        let pdf_path = dir.path().join("embedded.pdf");
        write_two_image_pdf(&pdf_path, &jpeg_bytes, (48, 36), &flate_bytes, (40, 40))?;

        let out_folder = dir.path().join("extracted-images");
        let extractor = QpdfImageExtractor::new(&sidecars);
        let control = ExportControl::with_budget(std::time::Duration::from_secs(30));
        let report = extractor.extract_images(
            &pdf_path,
            &out_folder,
            ExtractImagesOptions::default(),
            &control,
            |_, _| {},
        )?;

        assert_eq!(report.extracted, 2);
        assert_eq!(report.jpeg_count, 1);
        assert_eq!(report.png_count, 1);
        assert_eq!(report.skipped, 0);
        assert!(out_folder.join("image-0001.jpg").is_file());
        assert!(out_folder.join("image-0002.png").is_file());

        let img1 = image::open(out_folder.join("image-0001.jpg"))?;
        assert_eq!((img1.width(), img1.height()), (48, 36));
        let img2 = image::open(out_folder.join("image-0002.png"))?;
        assert_eq!((img2.width(), img2.height()), (40, 40));

        // Second run to the same destination must fail with Exists (no-clobber).
        let err = extractor.extract_images(
            &pdf_path,
            &out_folder,
            ExtractImagesOptions::default(),
            &control,
            |_, _| {},
        );
        assert!(matches!(
            err,
            Err(ExtractImagesError::Batch(SplitError::Exists))
        ));
        Ok(())
    }

    fn write_two_image_pdf(
        path: &Path,
        jpeg: &[u8],
        jpeg_dims: (u32, u32),
        flate: &[u8],
        flate_dims: (u32, u32),
    ) -> std::io::Result<()> {
        let mut out = Vec::new();
        let mut offsets = vec![0usize];
        out.extend_from_slice(b"%PDF-1.4\n%\xE2\xE3\xCF\xD3\n");
        let mut push_obj = |out: &mut Vec<u8>, id: usize, body: &[u8]| {
            offsets.push(out.len());
            out.extend_from_slice(format!("{id} 0 obj\n").as_bytes());
            out.extend_from_slice(body);
            out.extend_from_slice(b"\nendobj\n");
        };
        push_obj(&mut out, 1, b"<< /Type /Catalog /Pages 2 0 R >>");
        push_obj(&mut out, 2, b"<< /Type /Pages /Kids [3 0 R] /Count 1 >>");
        push_obj(
            &mut out,
            3,
            b"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 200 200] /Resources << /XObject << /Im1 4 0 R /Im2 5 0 R >> >> /Contents 6 0 R >>",
        );

        let mut im1 = format!(
            "<< /Type /XObject /Subtype /Image /Width {} /Height {} /ColorSpace /DeviceRGB /BitsPerComponent 8 /Filter /DCTDecode /Length {} >>\nstream\n",
            jpeg_dims.0,
            jpeg_dims.1,
            jpeg.len()
        )
        .into_bytes();
        im1.extend_from_slice(jpeg);
        im1.extend_from_slice(b"\nendstream");
        push_obj(&mut out, 4, &im1);

        let mut im2 = format!(
            "<< /Type /XObject /Subtype /Image /Width {} /Height {} /ColorSpace /DeviceRGB /BitsPerComponent 8 /Filter /FlateDecode /Length {} >>\nstream\n",
            flate_dims.0,
            flate_dims.1,
            flate.len()
        )
        .into_bytes();
        im2.extend_from_slice(flate);
        im2.extend_from_slice(b"\nendstream");
        push_obj(&mut out, 5, &im2);

        let content = b"q 50 0 0 50 10 10 cm /Im1 Do Q q 50 0 0 50 80 10 cm /Im2 Do Q";
        let mut c_obj = format!("<< /Length {} >>\nstream\n", content.len()).into_bytes();
        c_obj.extend_from_slice(content);
        c_obj.extend_from_slice(b"\nendstream");
        push_obj(&mut out, 6, &c_obj);

        let xref = out.len();
        out.extend_from_slice(
            format!("xref\n0 {}\n0000000000 65535 f \n", offsets.len()).as_bytes(),
        );
        for off in &offsets[1..] {
            out.extend_from_slice(format!("{off:010} 00000 n \n").as_bytes());
        }
        out.extend_from_slice(
            format!(
                "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n",
                offsets.len()
            )
            .as_bytes(),
        );
        fs::write(path, out)
    }
}
