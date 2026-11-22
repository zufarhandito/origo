//! Bounded, page-relative passive annotations. Coordinates precede page rotation.
use base64::{Engine, engine::general_purpose::STANDARD};
use serde_json::{Value, json};
use std::io::Write;

pub const MAX_POINTS: usize = 4096;
pub const MAX_PAGE_ANNOTATIONS: usize = 256;
pub const MAX_SIGNATURES: usize = 8;
pub const PREFIX: &str = "pdf-studio-annotation-v1:";
const MAX_SIGNATURE_BYTES: usize = SignatureImage::MAX_PIXELS as usize * 4;
const MAX_NAME_BYTES: usize = MAX_SIGNATURE_BYTES.div_ceil(3) * 4 + 256;
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
                    .map(|s| json!([s.width, s.height, s.encode()]))
            ])
        )
    }
    pub fn from_name(name: &str) -> Option<Self> {
        Self::parse_name(name, None)
    }
    pub fn from_typed_name(name: &str, signature: bool) -> Option<Self> {
        Self::parse_name(name, Some(signature))
    }
    fn parse_name(name: &str, expected_signature: Option<bool>) -> Option<Self> {
        if name.len() > MAX_NAME_BYTES {
            return None;
        }
        let v: Value = serde_json::from_str(name.strip_prefix(PREFIX)?).ok()?;
        let signature = v.get(6).filter(|v| !v.is_null());
        if expected_signature.is_some_and(|expected| expected != signature.is_some()) {
            return None;
        }
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
            signature: match signature {
                Some(s) => Some(std::sync::Arc::new(SignatureImage::decode(
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
    pub const MAX_SIDE: u32 = 1024;
    pub const MAX_PIXELS: u32 = 1024 * 512;
    pub const LEGACY_MAX_PIXELS: u32 = 512 * 256;

    pub fn valid(&self) -> bool {
        Self::byte_count(self.width, self.height) == Some(self.rgba.len())
    }

    fn byte_count(width: u32, height: u32) -> Option<usize> {
        if !(1..=Self::MAX_SIDE).contains(&width) || !(1..=Self::MAX_SIDE).contains(&height) {
            return None;
        }
        let pixels = width * height;
        (pixels <= Self::MAX_PIXELS).then_some(pixels as usize * 4)
    }

    fn encode(&self) -> String {
        let mut compressed =
            flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::fast());
        if compressed.write_all(&self.rgba).is_ok()
            && let Ok(compressed) = compressed.finish()
            && compressed.len() < self.rgba.len()
        {
            return format!("z:{}", STANDARD.encode(compressed));
        }
        format!("b:{}", STANDARD.encode(&self.rgba))
    }

    fn decode(width: u32, height: u32, text: &str) -> Option<Self> {
        let expected = Self::byte_count(width, height)?;
        let encoded_size = expected.div_ceil(3) * 4;
        let rgba = if let Some(encoded) = text.strip_prefix("z:") {
            if encoded.len() > encoded_size {
                return None;
            }
            let bytes = STANDARD.decode(encoded).ok()?;
            // Leave space for one excess byte and reject truncated or trailing data.
            let mut rgba = vec![0; expected + 1];
            let mut decoder = flate2::Decompress::new(true);
            let status = decoder
                .decompress(&bytes, &mut rgba, flate2::FlushDecompress::Finish)
                .ok()?;
            if status != flate2::Status::StreamEnd
                || decoder.total_in() != bytes.len() as u64
                || decoder.total_out() != expected as u64
            {
                return None;
            }
            rgba.truncate(expected);
            rgba
        } else if let Some(encoded) = text.strip_prefix("b:") {
            if encoded.len() != encoded_size {
                return None;
            }
            STANDARD.decode(encoded).ok()?
        } else {
            Self::decode_legacy(width, height, expected, text)?
        };
        (rgba.len() == expected).then_some(Self {
            width,
            height,
            rgba,
        })
    }

    fn decode_legacy(width: u32, height: u32, expected: usize, text: &str) -> Option<Vec<u8>> {
        if width > 512
            || height > 512
            || expected > Self::LEGACY_MAX_PIXELS as usize * 4
            || text.len() != expected * 2
        {
            return None;
        }
        text.as_bytes()
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
            .collect()
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
        let legacy = format!(
            "{PREFIX}[7,false,[0,0,0],1,[[0.1,0.2],[0.6,0.4]],false,[2,1,\"0000000014283c80\"]]"
        );
        assert_eq!(Annotation::from_name(&legacy), Some(a.clone()));
        let square = SignatureImage {
            width: 724,
            height: 724,
            rgba: vec![0; 724 * 724 * 4],
        };
        assert!(square.valid());
        assert!(square.encode().starts_with("z:"));
        a.signature = Some(std::sync::Arc::new(square));
        assert_eq!(Annotation::from_name(&a.name()), Some(a.clone()));
        assert!(
            !SignatureImage {
                width: 1024,
                height: 513,
                rgba: vec![0; 1024 * 513 * 4]
            }
            .valid()
        );
        a.points.swap(0, 1);
        assert!(!a.valid());
        assert!(Annotation::from_name(&a.name()).is_none());
        assert!(SignatureImage::decode(1025, 1, "").is_none());
        assert!(SignatureImage::decode(1, 1, "zzzzzzzz").is_none());
        assert!(SignatureImage::decode(1, 1, "b:AAAA").is_none());
        let compressed = SignatureImage {
            width: 100,
            height: 1,
            rgba: vec![0; 400],
        }
        .encode();
        assert!(SignatureImage::decode(99, 1, &compressed).is_none());
        assert!(SignatureImage::decode(101, 1, &compressed).is_none());
        assert!(SignatureImage::decode(100, 1, &compressed[..compressed.len() - 4]).is_none());
    }

    #[test]
    fn maximum_signature_falls_back_to_bounded_base64() {
        let mut state = 1u32;
        let image = SignatureImage {
            width: SignatureImage::MAX_SIDE,
            height: SignatureImage::MAX_PIXELS / SignatureImage::MAX_SIDE,
            rgba: (0..MAX_SIGNATURE_BYTES)
                .map(|_| {
                    state ^= state << 13;
                    state ^= state >> 17;
                    state ^= state << 5;
                    state as u8
                })
                .collect(),
        };
        let encoded = image.encode();
        assert!(encoded.starts_with("b:"));
        assert_eq!(encoded.len(), 2 + MAX_SIGNATURE_BYTES.div_ceil(3) * 4);
        assert_eq!(
            SignatureImage::decode(image.width, image.height, &encoded),
            Some(image)
        );
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
        assert!(
            Annotation::from_name(&format!("{PREFIX}{}", "x".repeat(MAX_NAME_BYTES))).is_none()
        );
        for turn in 0..4 {
            let point = [0.25, 0.75];
            assert_eq!(rotate(rotate(point, turn), (4 - turn) % 4), point);
        }
    }
}
