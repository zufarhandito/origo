//! Raster projection shared by the viewer, PNG export, and PDF appearance streams.
use fontdue::{
    Font, FontSettings,
    layout::{CoordinateSystem, Layout, LayoutSettings, TextStyle},
};
use pdf_domain::annotation::{
    Annotation, MAX_TEXT_CHARS, MAX_TEXT_SIZE, MIN_TEXT_SIZE, SignatureImage, rotate,
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
                let [p, q] = [pair[0], pair[1]];
                let dx = q[0] - p[0];
                let dy = q[1] - p[1];
                let length = dx * dx + dy * dy;
                let x0 = (p[0].min(q[0]) - radius).max(0.) as usize;
                let x1 = ((p[0].max(q[0]) + radius).ceil() as usize).min(w);
                let y0 = (p[1].min(q[1]) - radius).max(0.) as usize;
                let y1 = ((p[1].max(q[1]) + radius).ceil() as usize).min(h);
                for y in y0..y1 {
                    for x in x0..x1 {
                        let u = if length > 0. {
                            (((x as f32 + 0.5 - p[0]) * dx + (y as f32 + 0.5 - p[1]) * dy) / length)
                                .clamp(0., 1.)
                        } else {
                            0.
                        };
                        if (x as f32 + 0.5 - p[0] - u * dx).powi(2)
                            + (y as f32 + 0.5 - p[1] - u * dy).powi(2)
                            <= radius * radius
                        {
                            mask[y * w + x] = true;
                        }
                    }
                }
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
        for y in min_y..max_y {
            for x in min_x..max_x {
                let offset = y * w + x;
                if mask[offset] {
                    mask[offset] = false;
                    let pixel = &mut rgba[offset * 4..offset * 4 + 4];
                    for (i, channel) in pixel.iter_mut().enumerate().take(3) {
                        *channel = if a.highlight {
                            ((u32::from(*channel) * (255 + u32::from(a.color[i]))) / 510) as u8
                        } else {
                            a.color[i]
                        };
                    }
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
}
