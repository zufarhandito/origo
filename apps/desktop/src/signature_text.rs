//! Offline handwriting assets from bundled OFL fonts.
use fontdue::{
    Font, FontSettings,
    layout::{CoordinateSystem, Layout, LayoutSettings, TextStyle},
};
use pdf_domain::annotation::SignatureImage;
use std::sync::OnceLock;

static FONTS: [OnceLock<Result<Font, String>>; 2] = [OnceLock::new(), OnceLock::new()];
const BYTES: [&[u8]; 2] = [
    include_bytes!("../assets/signature-fonts/caveat-Caveat.ttf"),
    include_bytes!("../assets/signature-fonts/dancingscript-DancingScript.ttf"),
];
pub(super) fn render(text: &str, style: usize, color: [u8; 3]) -> Result<SignatureImage, String> {
    let text = text.trim();
    if text.is_empty() || text.chars().count() > 80 || text.chars().any(char::is_control) {
        return Err("Isi nama/inisial 1-80 karakter dalam satu baris.".into());
    }
    let style = style.min(1);
    let font = FONTS[style]
        .get_or_init(|| {
            Font::from_bytes(BYTES[style], FontSettings::default()).map_err(str::to_owned)
        })
        .as_ref()
        .map_err(Clone::clone)?;
    if text
        .chars()
        .any(|c| !c.is_whitespace() && font.lookup_glyph_index(c) == 0)
    {
        return Err(
            "Font ini belum mendukung sebagian karakter. Coba font lain atau gambar/impor.".into(),
        );
    }
    let mut layout = Layout::new(CoordinateSystem::PositiveYDown);
    layout.reset(&LayoutSettings::default());
    layout.append(&[font], &TextStyle::new(text, 96., 0));
    let glyphs = layout.glyphs();
    let left = glyphs
        .iter()
        .map(|g| g.x.floor() as i32)
        .min()
        .unwrap_or(0)
        .min(0);
    let top = glyphs
        .iter()
        .map(|g| g.y.floor() as i32)
        .min()
        .unwrap_or(0)
        .min(0);
    let right = glyphs
        .iter()
        .map(|g| g.x.ceil() as i32 + g.width as i32)
        .max()
        .unwrap_or(0);
    let bottom = glyphs
        .iter()
        .map(|g| g.y.ceil() as i32 + g.height as i32)
        .max()
        .unwrap_or(0);
    let (width, height) = ((right - left + 4) as u32, (bottom - top + 4) as u32);
    if width > 16384 || height > 512 {
        return Err("Nama terlalu panjang untuk dirender.".into());
    }
    let mut image = image::RgbaImage::new(width, height);
    for g in glyphs {
        let (_, mask) = font.rasterize_config(g.key);
        for y in 0..g.height {
            for x in 0..g.width {
                let alpha = mask[y * g.width + x];
                let (px, py) = (
                    (g.x as i32 - left + 2) as u32 + x as u32,
                    (g.y as i32 - top + 2) as u32 + y as u32,
                );
                if px < width && py < height && alpha > 0 {
                    let prev = image.get_pixel(px, py)[3] as u16;
                    let a = alpha as u16 + prev * (255 - alpha as u16) / 255;
                    image.put_pixel(px, py, image::Rgba([color[0], color[1], color[2], a as u8]));
                }
            }
        }
    }
    let mut image = image::DynamicImage::ImageRgba8(image)
        .thumbnail(512, 256)
        .to_rgba8();
    for p in image.pixels_mut() {
        if p[3] > 0 {
            p[0] = color[0];
            p[1] = color[1];
            p[2] = color[2];
        }
    }
    super::signatures::crop_asset(image).ok_or_else(|| "Nama tidak menghasilkan gambar.".into())
}
#[cfg(test)]
mod tests {
    #[test]
    fn fonts_are_offline_bounded_and_validate_glyphs() -> anyhow::Result<()> {
        for style in 0..2 {
            let a =
                super::render("Zufar Handito", style, [20, 60, 150]).map_err(anyhow::Error::msg)?;
            assert!(a.valid());
            assert!(
                a.rgba
                    .as_chunks::<4>()
                    .0
                    .iter()
                    .any(|p| p[3] > 0 && p[3] < 255)
            );
            assert!(
                a.rgba
                    .as_chunks::<4>()
                    .0
                    .iter()
                    .filter(|p| p[3] > 0)
                    .all(|p| p[..3] == [20, 60, 150])
            );
        }
        assert!(super::render("", 0, [0; 3]).is_err());
        assert!(super::render("hello\nworld", 0, [0; 3]).is_err());
        assert!(super::render(&"a".repeat(81), 0, [0; 3]).is_err());
        assert!(super::render("\u{7b7e}\u{540d}", 0, [0; 3]).is_err());
        Ok(())
    }
}
