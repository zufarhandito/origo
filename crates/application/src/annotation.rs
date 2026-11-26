//! Raster projection shared by the viewer, PNG export, and PDF appearance streams.
use fontdue::{
    Font, FontSettings,
    layout::{CoordinateSystem, Layout, LayoutSettings, TextStyle},
};
use pdf_domain::annotation::{
    Annotation, AnnotationExtra, FillSymbol, MAX_NOTE_CHARS, MAX_TEXT_CHARS, MAX_TEXT_SIZE,
    MIN_TEXT_SIZE, ShapeKind, SignatureImage, rotate,
};
use std::sync::OnceLock;

static TEXT_FONT: OnceLock<Result<Font, String>> = OnceLock::new();
const TEXT_FONT_BYTES: &[u8] = include_bytes!("../assets/fonts/IBMPlexSans-Regular.ttf");

fn text_font() -> Result<&'static Font, String> {
    TEXT_FONT
        .get_or_init(|| {
            Font::from_bytes(TEXT_FONT_BYTES, FontSettings::default()).map_err(str::to_owned)
        })
        .as_ref()
        .map_err(Clone::clone)
}

/// Validates multi-line sticky note text content.
pub fn validate_note(content: &str) -> Result<(), String> {
    let trimmed = content.trim();
    if trimmed.is_empty()
        || content.chars().count() > MAX_NOTE_CHARS
        || content.chars().any(|c| c.is_control() && c != '\n')
    {
        return Err(format!(
            "Isi catatan 1-{MAX_NOTE_CHARS} karakter (boleh beberapa baris)."
        ));
    }
    Ok(())
}

/// Rasterizes a crisp RGBA preview image for a Fill & Sign vector symbol (`✓`, `✗`, `●`).
pub fn rasterize_fill_symbol_preview(symbol: FillSymbol, color: [u8; 3]) -> SignatureImage {
    let size = 96u32;
    let mut rgba = vec![0u8; size as usize * size as usize * 4];
    let mut mask = vec![false; size as usize * size as usize];
    let w = size as usize;
    let h = size as usize;
    let s = size as f32;
    let radius = (s * 0.085).max(2.0);
    match symbol {
        FillSymbol::Check => {
            let pts = [
                [0.16 * s, 0.54 * s],
                [0.40 * s, 0.80 * s],
                [0.85 * s, 0.22 * s],
            ];
            for pair in pts.windows(2) {
                stamp_segment(&mut mask, w, h, pair[0], pair[1], radius);
            }
        }
        FillSymbol::Cross => {
            stamp_segment(&mut mask, w, h, [0.20 * s, 0.20 * s], [0.80 * s, 0.80 * s], radius);
            stamp_segment(&mut mask, w, h, [0.80 * s, 0.20 * s], [0.20 * s, 0.80 * s], radius);
        }
        FillSymbol::Dot => {
            stamp_disc(&mut mask, w, h, [0.5 * s, 0.5 * s], 0.34 * s);
        }
    }
    for (idx, on) in mask.into_iter().enumerate() {
        if on {
            let offset = idx * 4;
            rgba[offset] = color[0];
            rgba[offset + 1] = color[1];
            rgba[offset + 2] = color[2];
            rgba[offset + 3] = 255;
        }
    }
    SignatureImage {
        width: size,
        height: size,
        rgba,
    }
}

/// Validates single-line passive text content and verifies bundled font glyph coverage.
pub fn validate_text(content: &str) -> Result<(), String> {
    let trimmed = content.trim();
    if trimmed.is_empty()
        || content.chars().count() > MAX_TEXT_CHARS
        || content.chars().any(char::is_control)
    {
        return Err(format!(
            "Isi teks 1-{MAX_TEXT_CHARS} karakter dalam satu baris."
        ));
    }
    let font = text_font()?;
    if content
        .chars()
        .any(|c| !c.is_whitespace() && font.lookup_glyph_index(c) == 0)
    {
        return Err(
            "Sebagian karakter belum didukung font teks lokal. Gunakan huruf Latin, angka, atau tanda baca umum.".into(),
        );
    }
    Ok(())
}

fn layout_text(font: &Font, content: &str, px_size: f32) -> (Layout, f32, f32, f32, f32) {
    let mut layout = Layout::new(CoordinateSystem::PositiveYDown);
    layout.reset(&LayoutSettings::default());
    layout.append(&[font], &TextStyle::new(content, px_size, 0));
    let line_height = font
        .horizontal_line_metrics(px_size)
        .map(|m| m.new_line_size)
        .unwrap_or(px_size * 1.25)
        .max(px_size);
    let glyphs = layout.glyphs();
    let left = glyphs.iter().map(|g| g.x).fold(0.0_f32, f32::min);
    let top = glyphs.iter().map(|g| g.y).fold(0.0_f32, f32::min);
    let mut right = glyphs
        .iter()
        .map(|g| g.x + g.width as f32)
        .fold(0.0_f32, f32::max);
    if let Some(last) = glyphs.last() {
        let advance = font
            .metrics_indexed(last.key.glyph_index, px_size)
            .advance_width;
        right = right.max(last.x + advance);
    }
    let bottom = glyphs
        .iter()
        .map(|g| g.y + g.height as f32)
        .fold(line_height, f32::max);
    (layout, left, top, right, bottom)
}

/// Measures single-line text bounds in PDF points `[width, height]`.
pub fn measure_text_points(content: &str, font_size: f32) -> Option<[f32; 2]> {
    if content.trim().is_empty()
        || content.chars().count() > MAX_TEXT_CHARS
        || content.chars().any(char::is_control)
        || !font_size.is_finite()
        || !(MIN_TEXT_SIZE..=MAX_TEXT_SIZE).contains(&font_size)
    {
        return None;
    }
    let font = text_font().ok()?;
    let (_, left, top, right, bottom) = layout_text(font, content, font_size);
    let pad = (font_size * 0.15).max(1.5);
    let width = (right - left + pad * 2.).max(font_size * 0.6);
    let height = (bottom - top + pad * 2.).max(font_size * 1.2);
    Some([width, height])
}

/// Deterministically rasterizes a passive text annotation to a bounded RGBA image.
pub fn rasterize_text(content: &str, font_size: f32, color: [u8; 3]) -> Option<SignatureImage> {
    let [pts_w, pts_h] = measure_text_points(content, font_size)?;
    let font = text_font().ok()?;
    let max_side = SignatureImage::MAX_SIDE as f32;
    let max_pixels = SignatureImage::MAX_PIXELS as f32;
    let scale = 3.0_f32
        .min((max_side - 2.) / pts_w)
        .min((max_side - 2.) / pts_h)
        .min(((max_pixels * 0.95) / (pts_w * pts_h)).sqrt())
        .max(0.5);
    let px_size = font_size * scale;
    let pad = (font_size * 0.15).max(1.5) * scale;
    let (layout, left, top, right, bottom) = layout_text(font, content, px_size);
    let width = ((right - left + pad * 2.).ceil() as u32).clamp(1, SignatureImage::MAX_SIDE);
    let height = ((bottom - top + pad * 2.).ceil() as u32).clamp(1, SignatureImage::MAX_SIDE);
    if u64::from(width) * u64::from(height) > u64::from(SignatureImage::MAX_PIXELS) {
        return None;
    }
    let [r, g, b] = color;
    let mut rgba = vec![0u8; width as usize * height as usize * 4];
    for pixel in rgba.as_chunks_mut::<4>().0.iter_mut() {
        pixel[0] = r;
        pixel[1] = g;
        pixel[2] = b;
    }
    let offset_x = pad - left;
    let offset_y = pad - top;
    for glyph in layout.glyphs() {
        if glyph.width == 0 || glyph.height == 0 {
            continue;
        }
        let (_, mask) = font.rasterize_config(glyph.key);
        let gx = (glyph.x + offset_x).round() as i32;
        let gy = (glyph.y + offset_y).round() as i32;
        for y in 0..glyph.height {
            let py = gy + y as i32;
            if py < 0 || py >= height as i32 {
                continue;
            }
            for x in 0..glyph.width {
                let px = gx + x as i32;
                if px < 0 || px >= width as i32 {
                    continue;
                }
                let alpha = mask[y * glyph.width + x];
                if alpha == 0 {
                    continue;
                }
                let index = (py as usize * width as usize + px as usize) * 4 + 3;
                let prev = u16::from(rgba[index]);
                let combined = u16::from(alpha) + prev * (255 - u16::from(alpha)) / 255;
                rgba[index] = combined.min(255) as u8;
            }
        }
    }
    let image = SignatureImage {
        width,
        height,
        rgba,
    };
    image.valid().then_some(image)
}

/// Parses 1-based page target expressions like `"1-3, 5, 8-10"` into sorted unique 0-based indices.
pub fn parse_page_targets(
    spec: &str,
    page_count: usize,
    exclude_index: Option<usize>,
) -> Result<Vec<usize>, String> {
    if page_count == 0 {
        return Err("Dokumen tidak memiliki halaman.".into());
    }
    let trimmed = spec.trim();
    if trimmed.is_empty() || trimmed.len() > 2048 {
        return Err(format!(
            "Masukkan rentang halaman antara 1 dan {page_count}, misalnya 1-3, 5."
        ));
    }
    let mut selected = vec![false; page_count];
    for part in trimmed.split(',') {
        let token = part.trim();
        if token.is_empty() {
            return Err("Format rentang tidak valid. Contoh: 1-3, 5, 8.".into());
        }
        let mut bounds = token.split('-');
        let parse_page = |s: &str| -> Result<usize, String> {
            let s = s.trim();
            if s.is_empty() || !s.bytes().all(|b| b.is_ascii_digit()) {
                return Err("Gunakan angka dan tanda hubung saja, misalnya 2-5, 8.".into());
            }
            let number = s
                .parse::<usize>()
                .map_err(|_| "Nomor halaman di luar batas dokumen.".to_owned())?;
            if number == 0 || number > page_count {
                return Err(format!("Nomor halaman harus antara 1 dan {page_count}."));
            }
            Ok(number)
        };
        let start = parse_page(bounds.next().unwrap_or_default())?;
        let end = match bounds.next() {
            Some(value) => parse_page(value)?,
            None => start,
        };
        if bounds.next().is_some() || end < start {
            return Err(format!(
                "Rentang '{token}' tidak valid. Gunakan urutan naik seperti 2-5."
            ));
        }
        for page in start..=end {
            selected[page - 1] = true;
        }
    }
    if let Some(exclude) = exclude_index
        && exclude < page_count
    {
        selected[exclude] = false;
    }
    Ok(selected
        .into_iter()
        .enumerate()
        .filter_map(|(index, on)| on.then_some(index))
        .collect())
}

pub const MAX_WATERMARK_CHARS: usize = 64;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PageNumberFormat {
    #[default]
    NumberOnly,
    SlashTotal,
    IndonesianFull,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PageNumberPosition {
    TopLeft,
    TopCenter,
    TopRight,
    BottomLeft,
    #[default]
    BottomCenter,
    BottomRight,
}

/// Formats a 1-based display number and total count for passive page numbering.
pub fn format_page_number(format: PageNumberFormat, number: usize, total: usize) -> String {
    match format {
        PageNumberFormat::NumberOnly => number.to_string(),
        PageNumberFormat::SlashTotal => format!("{number} / {total}"),
        PageNumberFormat::IndonesianFull => format!("Halaman {number} dari {total}"),
    }
}

/// Computes normalized unrotated bounding corners `[[x0, y0], [x1, y1]]` for a page number label.
pub fn page_number_rect(
    content: &str,
    font_size: f32,
    page_w: f32,
    page_h: f32,
    position: PageNumberPosition,
) -> Option<[[f32; 2]; 2]> {
    if !page_w.is_finite() || !page_h.is_finite() || page_w <= 0. || page_h <= 0. {
        return None;
    }
    let [pts_w, pts_h] = measure_text_points(content, font_size)?;
    let mut width = (pts_w / page_w).clamp(0.01, 0.92);
    let mut height = (pts_h / page_h).clamp(0.01, 0.40);
    let fit = (0.92 / width).min(0.40 / height).min(1.0);
    width *= fit;
    height *= fit;
    let margin_x = (36.0 / page_w).clamp(0.02, 0.12);
    let margin_y = (28.0 / page_h).clamp(0.02, 0.12);
    let left = match position {
        PageNumberPosition::TopLeft | PageNumberPosition::BottomLeft => margin_x,
        PageNumberPosition::TopCenter | PageNumberPosition::BottomCenter => (1.0 - width) * 0.5,
        PageNumberPosition::TopRight | PageNumberPosition::BottomRight => 1.0 - margin_x - width,
    }
    .clamp(0.01, (0.99 - width).max(0.01));
    let top = match position {
        PageNumberPosition::TopLeft
        | PageNumberPosition::TopCenter
        | PageNumberPosition::TopRight => margin_y,
        PageNumberPosition::BottomLeft
        | PageNumberPosition::BottomCenter
        | PageNumberPosition::BottomRight => 1.0 - margin_y - height,
    }
    .clamp(0.01, (0.99 - height).max(0.01));
    Some([
        [left, top],
        [(left + width).min(0.999), (top + height).min(0.999)],
    ])
}

/// Rasterizes a semi-transparent rotated single-line text watermark into a deduplicatable RGBA stamp.
pub fn rasterize_watermark(
    content: &str,
    color: [u8; 3],
    opacity_percent: u8,
    angle_deg: i16,
) -> Option<SignatureImage> {
    let trimmed = content.trim();
    if trimmed.is_empty()
        || trimmed.chars().count() > MAX_WATERMARK_CHARS
        || validate_text(trimmed).is_err()
    {
        return None;
    }
    let font = text_font().ok()?;
    let base_pt = 48.0_f32;
    let [pts_w, pts_h] = measure_text_points(trimmed, base_pt)?;
    let clamped_angle = f32::from(angle_deg.clamp(-90, 90)).to_radians();
    let cos = clamped_angle.cos();
    let sin = clamped_angle.sin();
    let rot_w_unit = pts_w * cos.abs() + pts_h * sin.abs();
    let rot_h_unit = pts_w * sin.abs() + pts_h * cos.abs();
    if rot_w_unit <= 0. || rot_h_unit <= 0. {
        return None;
    }
    let max_side = (SignatureImage::MAX_SIDE - 8) as f32;
    let max_pixels = (SignatureImage::MAX_PIXELS as f32) * 0.85;
    let scale = 2.0_f32
        .min(max_side / rot_w_unit)
        .min(max_side / rot_h_unit)
        .min((max_pixels / (rot_w_unit * rot_h_unit)).sqrt())
        .max(0.25);

    let px_size = base_pt * scale;
    let pad = (base_pt * 0.15).max(2.0) * scale;
    let (layout, left, top, right, bottom) = layout_text(font, trimmed, px_size);
    let src_w = ((right - left + pad * 2.0).ceil() as usize).clamp(1, 2048);
    let src_h = ((bottom - top + pad * 2.0).ceil() as usize).clamp(1, 2048);
    let mut src_alpha = vec![0u8; src_w * src_h];
    let offset_x = pad - left;
    let offset_y = pad - top;
    for glyph in layout.glyphs() {
        if glyph.width == 0 || glyph.height == 0 {
            continue;
        }
        let (_, mask) = font.rasterize_config(glyph.key);
        let gx = (glyph.x + offset_x).round() as i32;
        let gy = (glyph.y + offset_y).round() as i32;
        for y in 0..glyph.height {
            let py = gy + y as i32;
            if py < 0 || py >= src_h as i32 {
                continue;
            }
            for x in 0..glyph.width {
                let px = gx + x as i32;
                if px < 0 || px >= src_w as i32 {
                    continue;
                }
                let alpha = mask[y * glyph.width + x];
                if alpha == 0 {
                    continue;
                }
                let idx = py as usize * src_w + px as usize;
                let prev = u16::from(src_alpha[idx]);
                let combined = u16::from(alpha) + prev * (255 - u16::from(alpha)) / 255;
                src_alpha[idx] = combined.min(255) as u8;
            }
        }
    }

    let raw_dst_w = (src_w as f32 * cos.abs() + src_h as f32 * sin.abs()).ceil() + 4.0;
    let raw_dst_h = (src_w as f32 * sin.abs() + src_h as f32 * cos.abs()).ceil() + 4.0;
    let dst_w = (raw_dst_w as u32).clamp(1, SignatureImage::MAX_SIDE);
    let dst_h = (raw_dst_h as u32).clamp(1, SignatureImage::MAX_SIDE);
    if u64::from(dst_w) * u64::from(dst_h) > u64::from(SignatureImage::MAX_PIXELS) {
        return None;
    }

    let sample_alpha = |fx: f32, fy: f32| -> f32 {
        let x0 = fx.floor() as i32;
        let y0 = fy.floor() as i32;
        let tx = fx - x0 as f32;
        let ty = fy - y0 as f32;
        let at = |ix: i32, iy: i32| -> f32 {
            if ix < 0 || iy < 0 || ix >= src_w as i32 || iy >= src_h as i32 {
                0.0
            } else {
                f32::from(src_alpha[iy as usize * src_w + ix as usize])
            }
        };
        let a00 = at(x0, y0);
        let a10 = at(x0 + 1, y0);
        let a01 = at(x0, y0 + 1);
        let a11 = at(x0 + 1, y0 + 1);
        let top_mix = a00 * (1.0 - tx) + a10 * tx;
        let bot_mix = a01 * (1.0 - tx) + a11 * tx;
        top_mix * (1.0 - ty) + bot_mix * ty
    };

    let opacity = f32::from(opacity_percent.clamp(5, 100)) / 100.0;
    let [r, g, b] = color;
    let mut rgba = vec![0u8; dst_w as usize * dst_h as usize * 4];
    let dst_cx = dst_w as f32 * 0.5;
    let dst_cy = dst_h as f32 * 0.5;
    let src_cx = src_w as f32 * 0.5;
    let src_cy = src_h as f32 * 0.5;
    let mut any_visible = false;
    for y in 0..dst_h as usize {
        let dy = (y as f32 + 0.5) - dst_cy;
        for x in 0..dst_w as usize {
            let dx = (x as f32 + 0.5) - dst_cx;
            let sx = dx * cos + dy * sin + src_cx - 0.5;
            let sy = -dx * sin + dy * cos + src_cy - 0.5;
            let alpha = (sample_alpha(sx, sy) * opacity).round().clamp(0.0, 255.0) as u8;
            let offset = (y * dst_w as usize + x) * 4;
            rgba[offset] = r;
            rgba[offset + 1] = g;
            rgba[offset + 2] = b;
            rgba[offset + 3] = alpha;
            if alpha > 0 {
                any_visible = true;
            }
        }
    }
    if !any_visible {
        return None;
    }
    let image = SignatureImage {
        width: dst_w,
        height: dst_h,
        rgba,
    };
    image.valid().then_some(image)
}

/// Computes centered normalized bounding corners `[[x0, y0], [x1, y1]]` for a watermark stamp.
pub fn watermark_rect(
    image_width: u32,
    image_height: u32,
    page_w: f32,
    page_h: f32,
    coverage: f32,
) -> Option<[[f32; 2]; 2]> {
    if image_width == 0
        || image_height == 0
        || !page_w.is_finite()
        || !page_h.is_finite()
        || page_w <= 0.
        || page_h <= 0.
    {
        return None;
    }
    let cov = coverage.clamp(0.20, 0.90);
    let max_w = page_w * cov;
    let max_h = page_h * cov;
    let scale = (max_w / image_width as f32).min(max_h / image_height as f32);
    let norm_w = ((image_width as f32 * scale) / page_w).clamp(0.05, 0.94);
    let norm_h = ((image_height as f32 * scale) / page_h).clamp(0.05, 0.94);
    let left = (0.5 - norm_w * 0.5).clamp(0.02, 0.98 - norm_w);
    let top = (0.5 - norm_h * 0.5).clamp(0.02, 0.98 - norm_h);
    Some([
        [left, top],
        [(left + norm_w).min(0.98), (top + norm_h).min(0.98)],
    ])
}

pub fn composite(
    rgba: &mut [u8],
    width: u32,
    height: u32,
    page_width: f32,
    turns: u8,
    annotations: &[Annotation],
) {
    if annotations.is_empty() {
        return;
    }
    let (w, h) = (width as usize, height as usize);
    if rgba.len() != w * h * 4 {
        return;
    }
    let mut mask = vec![false; w * h];
    for a in annotations.iter().filter(|a| a.valid()) {
        let text_bitmap = a
            .text
            .as_ref()
            .and_then(|t| rasterize_text(&t.content, t.font_size, a.color));
        if let Some(image) = a.signature.as_deref().or(text_bitmap.as_ref()) {
            let corners = [rotate(a.points[0], turns), rotate(a.points[1], turns)];
            let x0 = (corners[0][0].min(corners[1][0]) * width as f32).floor() as usize;
            let x1 = (corners[0][0].max(corners[1][0]) * width as f32).ceil() as usize;
            let y0 = (corners[0][1].min(corners[1][1]) * height as f32).floor() as usize;
            let y1 = (corners[0][1].max(corners[1][1]) * height as f32).ceil() as usize;
            let (page_x, page_y) = if turns.is_multiple_of(2) {
                (width, height)
            } else {
                (height, width)
            };
            let signature_width = (a.points[1][0] - a.points[0][0]) * page_x as f32;
            let signature_height = (a.points[1][1] - a.points[0][1]) * page_y as f32;
            let footprint = [
                (image.width as f32 / signature_width).max(1.),
                (image.height as f32 / signature_height).max(1.),
            ];
            for y in y0.min(h)..y1.min(h) {
                for x in x0.min(w)..x1.min(w) {
                    let p = rotate(
                        [
                            (x as f32 + 0.5) / width as f32,
                            (y as f32 + 0.5) / height as f32,
                        ],
                        (4 - turns) % 4,
                    );
                    let u = (p[0] - a.points[0][0]) / (a.points[1][0] - a.points[0][0]);
                    let v = (p[1] - a.points[0][1]) / (a.points[1][1] - a.points[0][1]);
                    if !(0. ..1.).contains(&u) || !(0. ..1.).contains(&v) {
                        continue;
                    }
                    let offset = (y * w + x) * 4;
                    blend_signature(image, [u, v], footprint, &mut rgba[offset..offset + 4]);
                }
            }
            continue;
        }
        if let Some(extra) = &a.extra {
            composite_extra(
                rgba,
                &mut mask,
                width,
                height,
                page_width,
                turns,
                a,
                extra,
            );
            continue;
        }
        let points: Vec<_> = a
            .points
            .iter()
            .map(|p| {
                let p = rotate(*p, turns);
                [p[0] * width as f32, p[1] * height as f32]
            })
            .collect();
        if a.rectangles {
            for pair in points.as_chunks::<2>().0.iter() {
                let (x0, x1) = (
                    pair[0][0].min(pair[1][0]) as usize,
                    pair[0][0].max(pair[1][0]).ceil() as usize,
                );
                let (y0, y1) = (
                    pair[0][1].min(pair[1][1]) as usize,
                    pair[0][1].max(pair[1][1]).ceil() as usize,
                );
                for y in y0.min(h)..y1.min(h) {
                    for x in x0.min(w)..x1.min(w) {
                        mask[y * w + x] = true;
                    }
                }
            }
        } else {
            let radius = (a.width * width as f32 / page_width / 2.).max(0.5);
            for pair in points.windows(2) {
                stamp_segment(&mut mask, w, h, pair[0], pair[1], radius);
            }
        }
        let radius = (a.width * width as f32 / page_width / 2.).max(1.);
        let min_x =
            (points.iter().map(|p| p[0]).fold(f32::INFINITY, f32::min) - radius).max(0.) as usize;
        let max_x = ((points
            .iter()
            .map(|p| p[0])
            .fold(f32::NEG_INFINITY, f32::max)
            + radius)
            .ceil() as usize)
            .min(w);
        let min_y =
            (points.iter().map(|p| p[1]).fold(f32::INFINITY, f32::min) - radius).max(0.) as usize;
        let max_y = ((points
            .iter()
            .map(|p| p[1])
            .fold(f32::NEG_INFINITY, f32::max)
            + radius)
            .ceil() as usize)
            .min(h);
        flush_mask(rgba, &mut mask, w, min_x, max_x, min_y, max_y, a.color, a.highlight);
    }
}

fn stamp_segment(mask: &mut [bool], w: usize, h: usize, p: [f32; 2], q: [f32; 2], radius: f32) {
    let dx = q[0] - p[0];
    let dy = q[1] - p[1];
    let length = dx * dx + dy * dy;
    let x0 = (p[0].min(q[0]) - radius).max(0.) as usize;
    let x1 = ((p[0].max(q[0]) + radius).ceil() as usize).min(w);
    let y0 = (p[1].min(q[1]) - radius).max(0.) as usize;
    let y1 = ((p[1].max(q[1]) + radius).ceil() as usize).min(h);
    let r2 = radius * radius;
    for y in y0..y1 {
        let py = y as f32 + 0.5;
        for x in x0..x1 {
            let px = x as f32 + 0.5;
            let u = if length > 0. {
                (((px - p[0]) * dx + (py - p[1]) * dy) / length).clamp(0., 1.)
            } else {
                0.
            };
            if (px - p[0] - u * dx).powi(2) + (py - p[1] - u * dy).powi(2) <= r2 {
                mask[y * w + x] = true;
            }
        }
    }
}

fn stamp_disc(mask: &mut [bool], w: usize, h: usize, center: [f32; 2], radius: f32) {
    let x0 = (center[0] - radius).max(0.) as usize;
    let x1 = ((center[0] + radius).ceil() as usize).min(w);
    let y0 = (center[1] - radius).max(0.) as usize;
    let y1 = ((center[1] + radius).ceil() as usize).min(h);
    let r2 = radius * radius;
    for y in y0..y1 {
        let dy = y as f32 + 0.5 - center[1];
        for x in x0..x1 {
            let dx = x as f32 + 0.5 - center[0];
            if dx * dx + dy * dy <= r2 {
                mask[y * w + x] = true;
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn flush_mask(
    rgba: &mut [u8],
    mask: &mut [bool],
    w: usize,
    min_x: usize,
    max_x: usize,
    min_y: usize,
    max_y: usize,
    color: [u8; 3],
    highlight: bool,
) {
    for y in min_y..max_y {
        for x in min_x..max_x {
            let offset = y * w + x;
            if mask[offset] {
                mask[offset] = false;
                let pixel = &mut rgba[offset * 4..offset * 4 + 4];
                for (i, channel) in pixel.iter_mut().enumerate().take(3) {
                    *channel = if highlight {
                        ((u32::from(*channel) * (255 + u32::from(color[i]))) / 510) as u8
                    } else {
                        color[i]
                    };
                }
            }
        }
    }
}

/// Computes the two arrowhead wing endpoints in the same coordinate space as `start -> end`.
pub fn arrow_head_wings(start: [f32; 2], end: [f32; 2], head_len: f32) -> Option<[[f32; 2]; 2]> {
    let dx = end[0] - start[0];
    let dy = end[1] - start[1];
    let dist = (dx * dx + dy * dy).sqrt();
    if dist < 1e-3 {
        return None;
    }
    let len = head_len.min(dist * 0.45);
    let angle = dy.atan2(dx);
    let spread = std::f32::consts::FRAC_PI_6; // 30 degrees
    let left = [
        end[0] - len * (angle - spread).cos(),
        end[1] - len * (angle - spread).sin(),
    ];
    let right = [
        end[0] - len * (angle + spread).cos(),
        end[1] - len * (angle + spread).sin(),
    ];
    Some([left, right])
}

#[allow(clippy::too_many_arguments)]
fn composite_extra(
    rgba: &mut [u8],
    mask: &mut [bool],
    width: u32,
    height: u32,
    page_width: f32,
    turns: u8,
    a: &Annotation,
    extra: &AnnotationExtra,
) {
    let (w, h) = (width as usize, height as usize);
    let to_px = |p: [f32; 2]| -> [f32; 2] {
        let r = rotate(p, turns);
        [r[0] * width as f32, r[1] * height as f32]
    };
    let pt_scale = (width as f32 / page_width.max(1.0)).max(0.1);
    let stroke_radius = (a.width * pt_scale / 2.0).max(0.75);

    let mut min_x = w;
    let mut max_x = 0usize;
    let mut min_y = h;
    let mut max_y = 0usize;
    let mut touch = |p: [f32; 2], rad: f32| {
        min_x = min_x.min((p[0] - rad).max(0.0) as usize);
        max_x = max_x.max(((p[0] + rad).ceil() as usize).min(w));
        min_y = min_y.min((p[1] - rad).max(0.0) as usize);
        max_y = max_y.max(((p[1] + rad).ceil() as usize).min(h));
    };

    match extra {
        AnnotationExtra::Underline | AnnotationExtra::StrikeOut => {
            let strike = matches!(extra, AnnotationExtra::StrikeOut);
            for pair in a.points.as_chunks::<2>().0.iter() {
                let left = pair[0][0].min(pair[1][0]);
                let right = pair[0][0].max(pair[1][0]);
                let top = pair[0][1].min(pair[1][1]);
                let bottom = pair[0][1].max(pair[1][1]);
                let y = if strike {
                    (top + bottom) * 0.5
                } else {
                    bottom - (bottom - top) * 0.08
                };
                let p0 = to_px([left, y]);
                let p1 = to_px([right, y]);
                let box_h_px = ((bottom - top) * if turns.is_multiple_of(2) { height } else { width } as f32).abs();
                let rad = (box_h_px * 0.055).max(stroke_radius).max(0.8);
                stamp_segment(mask, w, h, p0, p1, rad);
                touch(p0, rad + 1.0);
                touch(p1, rad + 1.0);
            }
            if min_x < max_x && min_y < max_y {
                flush_mask(rgba, mask, w, min_x, max_x, min_y, max_y, a.color, false);
            }
        }
        AnnotationExtra::Shape(ShapeKind::Rectangle) => {
            let [p0, p1] = [a.points[0], a.points[1]];
            let corners = [
                to_px([p0[0], p0[1]]),
                to_px([p1[0], p0[1]]),
                to_px([p1[0], p1[1]]),
                to_px([p0[0], p1[1]]),
            ];
            for i in 0..4 {
                let start = corners[i];
                let end = corners[(i + 1) % 4];
                stamp_segment(mask, w, h, start, end, stroke_radius);
                touch(start, stroke_radius + 1.0);
            }
            if min_x < max_x && min_y < max_y {
                flush_mask(rgba, mask, w, min_x, max_x, min_y, max_y, a.color, false);
            }
        }
        AnnotationExtra::Shape(ShapeKind::Ellipse) => {
            let [p0, p1] = [a.points[0], a.points[1]];
            let cx = (p0[0] + p1[0]) * 0.5;
            let cy = (p0[1] + p1[1]) * 0.5;
            let rx = (p1[0] - p0[0]) * 0.5;
            let ry = (p1[1] - p0[1]) * 0.5;
            const SEGMENTS: usize = 64;
            let mut prev = to_px([cx + rx, cy]);
            touch(prev, stroke_radius + 1.0);
            for i in 1..=SEGMENTS {
                let theta = std::f32::consts::TAU * (i as f32 / SEGMENTS as f32);
                let curr = to_px([cx + rx * theta.cos(), cy + ry * theta.sin()]);
                stamp_segment(mask, w, h, prev, curr, stroke_radius);
                touch(curr, stroke_radius + 1.0);
                prev = curr;
            }
            if min_x < max_x && min_y < max_y {
                flush_mask(rgba, mask, w, min_x, max_x, min_y, max_y, a.color, false);
            }
        }
        AnnotationExtra::Shape(ShapeKind::Line | ShapeKind::Arrow) => {
            let start = to_px(a.points[0]);
            let end = to_px(a.points[1]);
            stamp_segment(mask, w, h, start, end, stroke_radius);
            touch(start, stroke_radius + 1.0);
            touch(end, stroke_radius + 1.0);
            if matches!(extra, AnnotationExtra::Shape(ShapeKind::Arrow)) {
                let head_len = (a.width * pt_scale * 4.5).max(10.0 * pt_scale.max(0.5));
                if let Some([left, right]) = arrow_head_wings(start, end, head_len) {
                    stamp_segment(mask, w, h, end, left, stroke_radius);
                    stamp_segment(mask, w, h, end, right, stroke_radius);
                    touch(left, stroke_radius + 1.0);
                    touch(right, stroke_radius + 1.0);
                }
            }
            if min_x < max_x && min_y < max_y {
                flush_mask(rgba, mask, w, min_x, max_x, min_y, max_y, a.color, false);
            }
        }
        AnnotationExtra::Symbol(symbol) => {
            let [p0, p1] = [a.points[0], a.points[1]];
            let map = |u: f32, v: f32| -> [f32; 2] {
                to_px([p0[0] + (p1[0] - p0[0]) * u, p0[1] + (p1[1] - p0[1]) * v])
            };
            let top_left = map(0.0, 0.0);
            let top_right = map(1.0, 0.0);
            let bot_left = map(0.0, 1.0);
            let side_w = ((top_right[0] - top_left[0]).powi(2) + (top_right[1] - top_left[1]).powi(2)).sqrt();
            let side_h = ((bot_left[0] - top_left[0]).powi(2) + (bot_left[1] - top_left[1]).powi(2)).sqrt();
            let min_side = side_w.min(side_h);
            let rad = (min_side * 0.085).max(1.2);
            match symbol {
                FillSymbol::Check => {
                    let pts = [map(0.16, 0.54), map(0.40, 0.80), map(0.85, 0.22)];
                    for pair in pts.windows(2) {
                        stamp_segment(mask, w, h, pair[0], pair[1], rad);
                        touch(pair[0], rad + 1.0);
                        touch(pair[1], rad + 1.0);
                    }
                }
                FillSymbol::Cross => {
                    let s0 = map(0.20, 0.20);
                    let e0 = map(0.80, 0.80);
                    let s1 = map(0.80, 0.20);
                    let e1 = map(0.20, 0.80);
                    stamp_segment(mask, w, h, s0, e0, rad);
                    stamp_segment(mask, w, h, s1, e1, rad);
                    for p in [s0, e0, s1, e1] {
                        touch(p, rad + 1.0);
                    }
                }
                FillSymbol::Dot => {
                    let center = map(0.50, 0.50);
                    let disc_r = (min_side * 0.34).max(1.5);
                    stamp_disc(mask, w, h, center, disc_r);
                    touch(center, disc_r + 1.0);
                }
            }
            if min_x < max_x && min_y < max_y {
                flush_mask(rgba, mask, w, min_x, max_x, min_y, max_y, a.color, false);
            }
        }
        AnnotationExtra::Note(_) => {
            let [p0, p1] = [a.points[0], a.points[1]];
            let corners = [rotate(p0, turns), rotate(p1, turns)];
            let bx0 = (corners[0][0].min(corners[1][0]) * width as f32).floor().max(0.0) as usize;
            let bx1 = ((corners[0][0].max(corners[1][0]) * width as f32).ceil() as usize).min(w);
            let by0 = (corners[0][1].min(corners[1][1]) * height as f32).floor().max(0.0) as usize;
            let by1 = ((corners[0][1].max(corners[1][1]) * height as f32).ceil() as usize).min(h);
            let fold_color = [
                (u16::from(a.color[0]) * 70 / 100) as u8,
                (u16::from(a.color[1]) * 70 / 100) as u8,
                (u16::from(a.color[2]) * 70 / 100) as u8,
            ];
            let border_color = [
                (u16::from(a.color[0]) * 38 / 100) as u8,
                (u16::from(a.color[1]) * 38 / 100) as u8,
                (u16::from(a.color[2]) * 38 / 100) as u8,
            ];
            for y in by0..by1 {
                for x in bx0..bx1 {
                    let p = rotate(
                        [
                            (x as f32 + 0.5) / width as f32,
                            (y as f32 + 0.5) / height as f32,
                        ],
                        (4 - turns) % 4,
                    );
                    let u = (p[0] - p0[0]) / (p1[0] - p0[0]);
                    let v = (p[1] - p0[1]) / (p1[1] - p0[1]);
                    if !(0.0..=1.0).contains(&u) || !(0.0..=1.0).contains(&v) {
                        continue;
                    }
                    // Cut bottom-right corner diagonal (u + v > 1.74)
                    if u + v > 1.74 {
                        continue;
                    }
                    let on_border = u < 0.08
                        || v < 0.08
                        || (u > 0.92 && v <= 0.74)
                        || (v > 0.92 && u <= 0.74)
                        || (u + v > 1.66);
                    let in_fold = u >= 0.74 && v >= 0.74;
                    let on_line = (0.20..=0.78).contains(&u)
                        && ((0.27..=0.34).contains(&v)
                            || (0.46..=0.53).contains(&v)
                            || ((0.20..=0.60).contains(&u) && (0.65..=0.72).contains(&v)));
                    let color = if on_border || on_line {
                        border_color
                    } else if in_fold {
                        fold_color
                    } else {
                        a.color
                    };
                    let offset = (y * w + x) * 4;
                    rgba[offset] = color[0];
                    rgba[offset + 1] = color[1];
                    rgba[offset + 2] = color[2];
                    rgba[offset + 3] = 255;
                }
            }
        }
    }
}

fn blend_signature(image: &SignatureImage, [u, v]: [f32; 2], [dx, dy]: [f32; 2], dst: &mut [u8]) {
    let width = image.width as f32;
    let height = image.height as f32;
    let (x, y) = (u * width, v * height);
    let (left, right) = ((x - dx / 2.).max(0.), (x + dx / 2.).min(width));
    let (top, bottom) = ((y - dy / 2.).max(0.), (y + dy / 2.).min(height));
    let area = (right - left) * (bottom - top);
    let mut alpha = 0.;
    let mut ink = [0.; 3];
    // Average the source area for small previews. A one-texel footprint gives
    // bilinear interpolation when enlarged. Mix premultiplied colors in both cases.
    for row in top as usize..bottom.ceil() as usize {
        let overlap_y = bottom.min(row as f32 + 1.) - top.max(row as f32);
        for col in left as usize..right.ceil() as usize {
            let offset = (row * image.width as usize + col) * 4;
            let source = &image.rgba[offset..offset + 4];
            let overlap_x = right.min(col as f32 + 1.) - left.max(col as f32);
            let weight = overlap_x * overlap_y / area * (f32::from(source[3]) / 255.);
            alpha += weight;
            for (ink, channel) in ink.iter_mut().zip(source) {
                *ink += f32::from(*channel) * weight;
            }
        }
    }
    for (channel, ink) in dst.iter_mut().zip(ink) {
        *channel = (ink + f32::from(*channel) * (1. - alpha)).round() as u8;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn signature_resampling_preserves_thin_ink_and_ignores_invisible_rgb() {
        let signature = Annotation {
            id: 1,
            highlight: false,
            color: [255, 0, 0],
            width: 1.,
            points: vec![[0., 0.], [1., 1.]],
            rectangles: false,
            signature: Some(std::sync::Arc::new(SignatureImage {
                width: 4,
                height: 1,
                rgba: [0, 0, 255, 0, 255, 0, 0, 255, 0, 0, 255, 0, 0, 0, 255, 0].to_vec(),
            })),
            text: None,
            extra: None,
        };
        let mut narrow = [255; 8];
        composite(&mut narrow, 1, 2, 1., 1, std::slice::from_ref(&signature));
        assert_eq!(narrow, [255, 128, 128, 255, 255, 255, 255, 255]);

        let mut wide = [255; 32];
        composite(&mut wide, 8, 1, 8., 0, &[signature]);
        assert_eq!(&wide[8..12], &[255, 64, 64, 255]);
    }

    #[test]
    fn highlighter_preserves_black_text_and_does_not_stack_its_own_crossing() {
        let a = Annotation {
            id: 1,
            highlight: true,
            color: [255, 210, 0],
            width: 4.,
            points: vec![[0.1, 0.5], [0.9, 0.5], [0.1, 0.5]],
            rectangles: false,
            signature: None,
            text: None,
            extra: None,
        };
        let mut raster = vec![255; 40 * 40 * 4];
        let offset = (20 * 40 + 20) * 4;
        raster[offset..offset + 3].fill(0);
        composite(&mut raster, 40, 40, 40., 0, &[a]);
        assert_eq!(&raster[offset..offset + 4], &[0, 0, 0, 255]);
        assert_eq!(
            &raster[(20 * 40 + 10) * 4..(20 * 40 + 10) * 4 + 4],
            &[255, 232, 127, 255]
        );
    }

    #[test]
    fn text_rasterization_and_composite_are_deterministic_and_bounded() {
        assert!(validate_text("24 November 2026").is_ok());
        assert!(validate_text("   ").is_err());
        assert!(validate_text("baris 1\nbaris 2").is_err());
        assert!(validate_text("\u{7b7e}\u{540d}").is_err());

        let Some([w, h]) = measure_text_points("Zufar Handito", 14.) else {
            panic!("text should measure");
        };
        assert!(w > 40. && h >= 14.);

        let Some(image) = rasterize_text("Zufar Handito", 14., [30, 80, 180]) else {
            panic!("text should rasterize");
        };
        assert!(image.valid());
        assert!(image.rgba.as_chunks::<4>().0.iter().any(|p| p[3] > 200));
        assert!(
            image
                .rgba
                .as_chunks::<4>()
                .0
                .iter()
                .all(|p| p[..3] == [30, 80, 180])
        );

        let long_text = "A".repeat(MAX_TEXT_CHARS);
        let Some(long_image) = rasterize_text(&long_text, MAX_TEXT_SIZE, [25, 25, 25]) else {
            panic!("max length text must clamp within bounds");
        };
        assert!(long_image.valid());

        let text_annotation = Annotation {
            id: 5,
            highlight: false,
            color: [30, 80, 180],
            width: 1.,
            points: vec![[0.1, 0.2], [0.7, 0.4]],
            rectangles: false,
            signature: None,
            text: Some(pdf_domain::annotation::TextAnnotation {
                content: "24 November 2026".into(),
                font_size: 14.,
            }),
            extra: None,
        };
        for turns in 0..4 {
            let mut canvas = vec![255; 120 * 120 * 4];
            composite(
                &mut canvas,
                120,
                120,
                600.,
                turns,
                std::slice::from_ref(&text_annotation),
            );
            assert!(
                canvas.as_chunks::<4>().0.iter().any(|p| p[0] < 240),
                "text must paint visible pixels at rotation {turns}"
            );
        }
    }

    #[test]
    fn extra_annotations_composite_cleanly_at_all_rotations() {
        assert!(validate_note("Catatan revisi\nBaris kedua").is_ok());
        assert!(validate_note("  ").is_err());

        for sym in [FillSymbol::Check, FillSymbol::Cross, FillSymbol::Dot] {
            let preview = rasterize_fill_symbol_preview(sym, [30, 170, 95]);
            assert!(preview.valid());
            assert!(preview.rgba.as_chunks::<4>().0.iter().any(|p| p[3] == 255));
        }

        let items = [
            Annotation {
                id: 11,
                highlight: false,
                color: [40, 110, 220],
                width: 2.,
                points: vec![[0.1, 0.2], [0.8, 0.28]],
                rectangles: true,
                signature: None,
                text: None,
                extra: Some(AnnotationExtra::Underline),
            },
            Annotation {
                id: 12,
                highlight: false,
                color: [230, 65, 75],
                width: 2.,
                points: vec![[0.1, 0.4], [0.8, 0.48]],
                rectangles: true,
                signature: None,
                text: None,
                extra: Some(AnnotationExtra::StrikeOut),
            },
            Annotation {
                id: 13,
                highlight: false,
                color: [230, 65, 75],
                width: 2.,
                points: vec![[0.2, 0.2], [0.7, 0.6]],
                rectangles: false,
                signature: None,
                text: None,
                extra: Some(AnnotationExtra::Shape(ShapeKind::Rectangle)),
            },
            Annotation {
                id: 14,
                highlight: false,
                color: [30, 170, 95],
                width: 2.,
                points: vec![[0.2, 0.2], [0.7, 0.6]],
                rectangles: false,
                signature: None,
                text: None,
                extra: Some(AnnotationExtra::Shape(ShapeKind::Ellipse)),
            },
            Annotation {
                id: 15,
                highlight: false,
                color: [25, 25, 25],
                width: 2.,
                points: vec![[0.15, 0.2], [0.85, 0.8]],
                rectangles: false,
                signature: None,
                text: None,
                extra: Some(AnnotationExtra::Shape(ShapeKind::Arrow)),
            },
            Annotation {
                id: 16,
                highlight: false,
                color: [255, 210, 0],
                width: 2.,
                points: vec![[0.2, 0.2], [0.35, 0.35]],
                rectangles: false,
                signature: None,
                text: None,
                extra: Some(AnnotationExtra::Note("Periksa tanda tangan".into())),
            },
            Annotation {
                id: 17,
                highlight: false,
                color: [30, 170, 95],
                width: 2.,
                points: vec![[0.3, 0.3], [0.5, 0.5]],
                rectangles: false,
                signature: None,
                text: None,
                extra: Some(AnnotationExtra::Symbol(FillSymbol::Check)),
            },
        ];

        for item in &items {
            for turns in 0..4 {
                let mut canvas = vec![255; 100 * 100 * 4];
                composite(&mut canvas, 100, 100, 500., turns, std::slice::from_ref(item));
                assert!(
                    canvas.as_chunks::<4>().0.iter().any(|p| p[..3] != [255, 255, 255]),
                    "{:?} must paint visible pixels at rotation {turns}",
                    item.extra
                );
            }
        }
    }

    #[test]
    fn page_target_parser_handles_ranges_dedup_exclusion_and_errors() {
        assert_eq!(
            parse_page_targets("1-3, 5, 2-4", 6, Some(0)),
            Ok(vec![1, 2, 3, 4])
        );
        assert_eq!(parse_page_targets("4, 2", 5, None), Ok(vec![1, 3]));
        assert!(parse_page_targets("", 5, None).is_err());
        assert!(parse_page_targets("0-2", 5, None).is_err());
        assert!(parse_page_targets("1-6", 5, None).is_err());
        assert!(parse_page_targets("4-2", 5, None).is_err());
        assert!(parse_page_targets("1, abc", 5, None).is_err());
        assert!(parse_page_targets("1,,2", 5, None).is_err());
    }

    #[test]
    fn page_numbers_format_and_place_cleanly_in_all_six_positions() {
        assert_eq!(format_page_number(PageNumberFormat::NumberOnly, 3, 12), "3");
        assert_eq!(
            format_page_number(PageNumberFormat::SlashTotal, 3, 12),
            "3 / 12"
        );
        assert_eq!(
            format_page_number(PageNumberFormat::IndonesianFull, 3, 12),
            "Halaman 3 dari 12"
        );

        let positions = [
            PageNumberPosition::TopLeft,
            PageNumberPosition::TopCenter,
            PageNumberPosition::TopRight,
            PageNumberPosition::BottomLeft,
            PageNumberPosition::BottomCenter,
            PageNumberPosition::BottomRight,
        ];
        for pos in positions {
            let Some([a, b]) = page_number_rect("Halaman 3 dari 12", 12., 595., 842., pos) else {
                panic!("rect should be computed");
            };
            assert!(a[0] >= 0.01 && a[1] >= 0.01 && b[0] <= 0.999 && b[1] <= 0.999);
            assert!(a[0] < b[0] && a[1] < b[1]);
        }
        let Some([tl_a, _]) =
            page_number_rect("1 / 5", 12., 595., 842., PageNumberPosition::TopLeft)
        else {
            panic!("top-left");
        };
        let Some([br_a, _]) =
            page_number_rect("1 / 5", 12., 595., 842., PageNumberPosition::BottomRight)
        else {
            panic!("bottom-right");
        };
        assert!(tl_a[0] < br_a[0] && tl_a[1] < br_a[1]);
    }

    #[test]
    fn diagonal_watermark_rasterizes_bounded_transparent_stamp() {
        for angle in [-45, 0, 45] {
            let Some(wm) = rasterize_watermark("RAHASIA", [185, 28, 28], 25, angle) else {
                panic!("watermark should rasterize at angle {angle}");
            };
            assert!(wm.valid());
            assert!(wm.width <= SignatureImage::MAX_SIDE);
            assert!(wm.height <= SignatureImage::MAX_SIDE);
            let max_alpha = wm
                .rgba
                .as_chunks::<4>()
                .0
                .iter()
                .map(|p| p[3])
                .max()
                .unwrap_or(0);
            assert!(
                (40..=75).contains(&max_alpha),
                "25% opacity should cap alpha near 64, got {max_alpha}"
            );
            assert!(
                wm.rgba
                    .as_chunks::<4>()
                    .0
                    .iter()
                    .all(|p| p[..3] == [185, 28, 28])
            );
            let Some([a, b]) = watermark_rect(wm.width, wm.height, 595., 842., 0.62) else {
                panic!("watermark rect");
            };
            assert!(a[0] < b[0] && a[1] < b[1]);
            assert!(((a[0] + b[0]) * 0.5 - 0.5).abs() < 0.01);
            assert!(((a[1] + b[1]) * 0.5 - 0.5).abs() < 0.01);
        }
        assert!(rasterize_watermark("   ", [0, 0, 0], 25, -45).is_none());
        assert!(rasterize_watermark(&"X".repeat(MAX_WATERMARK_CHARS + 1), [0, 0, 0], 25, -45).is_none());
    }
}

