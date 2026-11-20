//! Bounded, page-relative passive annotations. Coordinates precede page rotation.
use serde_json::{Value, json};

pub const MAX_POINTS: usize = 4096;
pub const MAX_PAGE_ANNOTATIONS: usize = 256;
pub const PREFIX: &str = "pdf-studio-annotation-v1:";
#[derive(Clone, Debug, PartialEq)]
pub struct Annotation {
    pub id: u64,
    pub highlight: bool,
    pub color: [u8; 3],
    /// Width in PDF points, independent of zoom.
    pub width: f32,
    /// A stroke, or opposite corners of text rectangles (two points per rectangle).
    pub points: Vec<[f32; 2]>,
    pub rectangles: bool,
    pub signature: Option<std::sync::Arc<SignatureImage>>,
}
impl Annotation {
    pub fn valid(&self) -> bool {
        (self.signature.as_ref().is_none_or(|s| s.valid())
            && (self.signature.is_none()
                || (!self.highlight
                    && !self.rectangles
                    && self.points.len() == 2
                    && self.points[0][0] < self.points[1][0]
                    && self.points[0][1] < self.points[1][1])))
            && self.id > 0
            && self.width.is_finite()
            && (0.5..=40.).contains(&self.width)
            && (2..=MAX_POINTS).contains(&self.points.len())
            && (!self.rectangles || (self.highlight && self.points.len().is_multiple_of(2)))
            && self
                .points
                .iter()
                .flatten()
                .all(|v| v.is_finite() && (0.0..=1.0).contains(v))
    }
    pub fn name(&self) -> String {
        format!(
            "{PREFIX}{}",
            json!([
                self.id,
                self.highlight,
                self.color,
                self.width,
                self.points,
                self.rectangles,
                self.signature
                    .as_ref()
                    .map(|s| json!([s.width, s.height, s.hex()]))
            ])
        )
    }
    pub fn from_name(name: &str) -> Option<Self> {
        if name.len() > 1_100_000 {
            return None;
        }
        let v: Value = serde_json::from_str(name.strip_prefix(PREFIX)?).ok()?;
        let a = Self {
            id: v.get(0)?.as_u64()?,
            highlight: v.get(1)?.as_bool()?,
            color: [
                u8::try_from(v[2][0].as_u64()?).ok()?,
                u8::try_from(v[2][1].as_u64()?).ok()?,
                u8::try_from(v[2][2].as_u64()?).ok()?,
            ],
            width: v.get(3)?.as_f64()? as f32,
            points: v
                .get(4)?
                .as_array()?
                .iter()
                .map(|p| Some([p.get(0)?.as_f64()? as f32, p.get(1)?.as_f64()? as f32]))
                .collect::<Option<Vec<_>>>()?,
            rectangles: v.get(5)?.as_bool()?,
            signature: match v.get(6).filter(|v| !v.is_null()) {
                Some(s) => Some(std::sync::Arc::new(SignatureImage::from_hex(
                    u32::try_from(s[0].as_u64()?).ok()?,
                    u32::try_from(s[1].as_u64()?).ok()?,
                    s[2].as_str()?,
                )?)),
                None => None,
            },
        };
        a.valid().then_some(a)
    }
}
/// Bounded RGBA signature asset; Arc shares it across history snapshots.
#[derive(Clone, Debug, PartialEq)]
pub struct SignatureImage {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}
impl SignatureImage {
    pub fn valid(&self) -> bool {
        self.width > 0
            && self.height > 0
            && self.width <= 512
            && self.height <= 512
            && u64::from(self.width) * u64::from(self.height) <= 512 * 256
            && self.rgba.len() == self.width as usize * self.height as usize * 4
    }
    fn hex(&self) -> String {
        self.rgba.iter().map(|b| format!("{b:02x}")).collect()
    }
    fn from_hex(width: u32, height: u32, text: &str) -> Option<Self> {
        if width == 0
            || height == 0
            || width > 512
            || height > 512
            || u64::from(width) * u64::from(height) > 512 * 256
            || text.len() != width as usize * height as usize * 8
        {
            return None;
        }
        let rgba = text
            .as_bytes()
            .as_chunks::<2>()
            .0
            .iter()
            .map(|p| {
                let digit = |c: u8| match c {
                    b'0'..=b'9' => Some(c - b'0'),
                    b'a'..=b'f' => Some(c - b'a' + 10),
                    _ => None,
                };
                Some(digit(p[0])? * 16 + digit(p[1])?)
            })
            .collect::<Option<Vec<_>>>()?;
        Some(Self {
            width,
            height,
            rgba,
        })
    }
}
pub fn rotate(mut point: [f32; 2], turns: u8) -> [f32; 2] {
    for _ in 0..turns % 4 {
        point = [1. - point[1], point[0]];
    }
    point
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn signature_metadata_roundtrip_rejects_invalid_assets() {
        let mut a = Annotation {
            id: 7,
            highlight: false,
            color: [0, 0, 0],
            width: 1.,
            points: vec![[0.1, 0.2], [0.6, 0.4]],
            rectangles: false,
            signature: Some(std::sync::Arc::new(SignatureImage {
                width: 2,
                height: 1,
                rgba: vec![0, 0, 0, 0, 20, 40, 60, 128],
            })),
        };
        assert_eq!(Annotation::from_name(&a.name()), Some(a.clone()));
        let square = SignatureImage {
            width: 362,
            height: 362,
            rgba: vec![0; 362 * 362 * 4],
        };
        assert!(square.valid());
        a.signature = Some(std::sync::Arc::new(square));
        assert_eq!(Annotation::from_name(&a.name()), Some(a.clone()));
        assert!(
            !SignatureImage {
                width: 512,
                height: 512,
                rgba: vec![0; 512 * 512 * 4]
            }
            .valid()
        );
        a.points.swap(0, 1);
        assert!(!a.valid());
        assert!(Annotation::from_name(&a.name()).is_none());
        assert!(SignatureImage::from_hex(513, 1, "").is_none());
        assert!(SignatureImage::from_hex(1, 1, "zzzzzzzz").is_none());
    }
    #[test]
    fn roundtrip_and_invalid_metadata_are_bounded() {
        let a = Annotation {
            id: 17,
            highlight: true,
            color: [255, 210, 0],
            width: 12.,
            points: vec![[0.1, 0.2], [0.7, 0.8]],
            rectangles: false,
            signature: None,
        };
        assert_eq!(Annotation::from_name(&a.name()), Some(a.clone()));
        let mut bad = a;
        bad.points[0][0] = f32::INFINITY;
        assert!(!bad.valid());
        assert!(Annotation::from_name(&format!("{PREFIX}{}", "x".repeat(1_100_001))).is_none());
        for turn in 0..4 {
            let point = [0.25, 0.75];
            assert_eq!(rotate(rotate(point, turn), (4 - turn) % 4), point);
        }
    }
}
