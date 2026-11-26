//! Bounded, page-relative passive annotations. Coordinates precede page rotation.
use base64::{Engine, engine::general_purpose::STANDARD};
use serde_json::{Value, json};
use std::io::Write;

pub const MAX_POINTS: usize = 4096;
pub const MAX_PAGE_ANNOTATIONS: usize = 256;
pub const MAX_SIGNATURES: usize = 8;
pub const MAX_SIGNATURE_PLACEMENTS: usize = 1024;
pub const MAX_TEXT_CHARS: usize = 240;
pub const MAX_NOTE_CHARS: usize = 1000;
pub const MIN_TEXT_SIZE: f32 = 8.;
pub const MAX_TEXT_SIZE: f32 = 48.;
pub const PREFIX: &str = "pdf-studio-annotation-v1:";
const MAX_SIGNATURE_BYTES: usize = SignatureImage::MAX_PIXELS as usize * 4;
const MAX_NAME_BYTES: usize = MAX_SIGNATURE_BYTES.div_ceil(3) * 4 + 256;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AnnotationKind {
    Ink,
    Highlight,
    Underline,
    StrikeOut,
    Square,
    Circle,
    Line,
    Text,
    Stamp,
    FreeText,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShapeKind {
    Rectangle,
    Ellipse,
    Line,
    Arrow,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FillSymbol {
    Check,
    Cross,
    Dot,
}

#[derive(Clone, Debug, PartialEq)]
pub enum AnnotationExtra {
    Underline,
    StrikeOut,
    Shape(ShapeKind),
    Note(String),
    Symbol(FillSymbol),
}

impl AnnotationExtra {
    pub fn kind(&self) -> AnnotationKind {
        match self {
            Self::Underline => AnnotationKind::Underline,
            Self::StrikeOut => AnnotationKind::StrikeOut,
            Self::Shape(ShapeKind::Rectangle) => AnnotationKind::Square,
            Self::Shape(ShapeKind::Ellipse) => AnnotationKind::Circle,
            Self::Shape(ShapeKind::Line | ShapeKind::Arrow) => AnnotationKind::Line,
            Self::Note(_) => AnnotationKind::Text,
            Self::Symbol(_) => AnnotationKind::Stamp,
        }
    }

    fn to_json(&self) -> Value {
        match self {
            Self::Underline => json!("underline"),
            Self::StrikeOut => json!("strikeout"),
            Self::Shape(ShapeKind::Rectangle) => json!("rect"),
            Self::Shape(ShapeKind::Ellipse) => json!("ellipse"),
            Self::Shape(ShapeKind::Line) => json!("line"),
            Self::Shape(ShapeKind::Arrow) => json!("arrow"),
            Self::Note(content) => json!(["note", content]),
            Self::Symbol(FillSymbol::Check) => json!("check"),
            Self::Symbol(FillSymbol::Cross) => json!("cross"),
            Self::Symbol(FillSymbol::Dot) => json!("dot"),
        }
    }

    fn from_json(value: &Value) -> Option<Self> {
        if let Some(tag) = value.as_str() {
            return match tag {
                "underline" => Some(Self::Underline),
                "strikeout" => Some(Self::StrikeOut),
                "rect" => Some(Self::Shape(ShapeKind::Rectangle)),
                "ellipse" => Some(Self::Shape(ShapeKind::Ellipse)),
                "line" => Some(Self::Shape(ShapeKind::Line)),
                "arrow" => Some(Self::Shape(ShapeKind::Arrow)),
                "check" => Some(Self::Symbol(FillSymbol::Check)),
                "cross" => Some(Self::Symbol(FillSymbol::Cross)),
                "dot" => Some(Self::Symbol(FillSymbol::Dot)),
                _ => None,
            };
        }
        let arr = value.as_array()?;
        if arr.len() == 2 && arr[0].as_str() == Some("note") {
            return Some(Self::Note(arr[1].as_str()?.to_owned()));
        }
        None
    }
}

/// Passive, single-line text annotation (free text, date, or name — not an interactive form field).
#[derive(Clone, Debug, PartialEq)]
pub struct TextAnnotation {
    pub content: String,
    /// Font size in PDF points, independent of zoom.
    pub font_size: f32,
}

impl TextAnnotation {
    pub fn valid(&self) -> bool {
        !self.content.trim().is_empty()
            && self.content.chars().count() <= MAX_TEXT_CHARS
            && !self.content.chars().any(char::is_control)
            && self.font_size.is_finite()
            && (MIN_TEXT_SIZE..=MAX_TEXT_SIZE).contains(&self.font_size)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Annotation {
    pub id: u64,
    pub highlight: bool,
    pub color: [u8; 3],
    /// Width in PDF points, independent of zoom.
    pub width: f32,
    /// A stroke, or opposite corners of rectangles/boxes (two points per rectangle).
    pub points: Vec<[f32; 2]>,
    pub rectangles: bool,
    pub signature: Option<std::sync::Arc<SignatureImage>>,
    pub text: Option<TextAnnotation>,
    pub extra: Option<AnnotationExtra>,
}

/// Result of parsing an owned `/NM` annotation name, supporting deduplicated signature refs.
#[derive(Clone, Debug, PartialEq)]
pub enum ParsedName {
    Complete(Annotation),
    SignatureRef {
        id: u64,
        color: [u8; 3],
        width: f32,
        points: Vec<[f32; 2]>,
        image_width: u32,
        image_height: u32,
        ref_id: u64,
    },
}

impl ParsedName {
    pub fn kind(&self) -> AnnotationKind {
        match self {
            Self::Complete(a) => a.kind(),
            Self::SignatureRef { .. } => AnnotationKind::Stamp,
        }
    }

    pub fn resolve_signature(self, image: std::sync::Arc<SignatureImage>) -> Option<Annotation> {
        match self {
            Self::Complete(a) => Some(a),
            Self::SignatureRef {
                id,
                color,
                width,
                points,
                image_width,
                image_height,
                ..
            } => {
                if image.width != image_width || image.height != image_height {
                    return None;
                }
                let annotation = Annotation {
                    id,
                    highlight: false,
                    color,
                    width,
                    points,
                    rectangles: false,
                    signature: Some(image),
                    text: None,
                    extra: None,
                };
                annotation.valid().then_some(annotation)
            }
        }
    }
}

impl Annotation {
    pub fn kind(&self) -> AnnotationKind {
        if self.signature.is_some() {
            AnnotationKind::Stamp
        } else if self.text.is_some() {
            AnnotationKind::FreeText
        } else if let Some(extra) = &self.extra {
            extra.kind()
        } else if self.rectangles {
            AnnotationKind::Highlight
        } else {
            AnnotationKind::Ink
        }
    }

    pub fn is_box_object(&self) -> bool {
        self.signature.is_some()
            || self.text.is_some()
            || matches!(
                self.extra,
                Some(
                    AnnotationExtra::Symbol(_)
                        | AnnotationExtra::Note(_)
                        | AnnotationExtra::Shape(ShapeKind::Rectangle | ShapeKind::Ellipse)
                )
            )
    }

    pub fn valid(&self) -> bool {
        let variant_count = usize::from(self.signature.is_some())
            + usize::from(self.text.is_some())
            + usize::from(self.extra.is_some());
        if variant_count > 1 {
            return false;
        }
        let sorted_box = self.points.len() == 2
            && self.points[0][0] < self.points[1][0]
            && self.points[0][1] < self.points[1][1];
        let payload_valid = if let Some(signature) = &self.signature {
            signature.valid() && !self.highlight && !self.rectangles && sorted_box
        } else if let Some(text) = &self.text {
            text.valid() && !self.highlight && !self.rectangles && sorted_box
        } else if let Some(extra) = &self.extra {
            match extra {
                AnnotationExtra::Underline | AnnotationExtra::StrikeOut => {
                    !self.highlight && self.rectangles && self.points.len().is_multiple_of(2)
                }
                AnnotationExtra::Shape(ShapeKind::Rectangle | ShapeKind::Ellipse)
                | AnnotationExtra::Symbol(_) => !self.highlight && !self.rectangles && sorted_box,
                AnnotationExtra::Shape(ShapeKind::Line | ShapeKind::Arrow) => {
                    !self.highlight && !self.rectangles && self.points.len() == 2
                }
                AnnotationExtra::Note(content) => {
                    !self.highlight
                        && !self.rectangles
                        && sorted_box
                        && !content.trim().is_empty()
                        && content.chars().count() <= MAX_NOTE_CHARS
                        && !content.chars().any(|c| c.is_control() && c != '\n')
                }
            }
        } else {
            !self.rectangles || (self.highlight && self.points.len().is_multiple_of(2))
        };
        payload_valid
            && self.id > 0
            && self.width.is_finite()
            && (0.5..=40.).contains(&self.width)
            && (2..=MAX_POINTS).contains(&self.points.len())
            && self
                .points
                .iter()
                .flatten()
                .all(|v| v.is_finite() && (0.0..=1.0).contains(v))
    }

    pub fn name(&self) -> String {
        if let Some(extra) = &self.extra {
            format!(
                "{PREFIX}{}",
                json!([
                    self.id,
                    self.highlight,
                    self.color,
                    self.width,
                    self.points,
                    self.rectangles,
                    Value::Null,
                    Value::Null,
                    extra.to_json()
                ])
            )
        } else if let Some(text) = &self.text {
            format!(
                "{PREFIX}{}",
                json!([
                    self.id,
                    false,
                    self.color,
                    self.width,
                    self.points,
                    false,
                    Value::Null,
                    [text.content, text.font_size]
                ])
            )
        } else {
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
    }

    pub fn name_with_signature_ref(&self, ref_id: u64) -> Option<String> {
        let s = self.signature.as_ref()?;
        if ref_id == 0 || ref_id == self.id {
            return None;
        }
        Some(format!(
            "{PREFIX}{}",
            json!([
                self.id,
                false,
                self.color,
                self.width,
                self.points,
                false,
                [s.width, s.height, format!("ref:{ref_id}")]
            ])
        ))
    }

    pub fn from_name(name: &str) -> Option<Self> {
        match Self::parse_with_kind(name, None)? {
            ParsedName::Complete(a) => Some(a),
            ParsedName::SignatureRef { .. } => None,
        }
    }

    pub fn from_typed_name(name: &str, signature: bool) -> Option<Self> {
        let parsed = Self::parse_with_kind(name, None)?;
        let ParsedName::Complete(a) = parsed else {
            return None;
        };
        (a.signature.is_some() == signature).then_some(a)
    }

    pub fn parse_with_kind(name: &str, expected: Option<AnnotationKind>) -> Option<ParsedName> {
        if name.len() > MAX_NAME_BYTES {
            return None;
        }
        let v: Value = serde_json::from_str(name.strip_prefix(PREFIX)?).ok()?;
        let array = v.as_array()?;
        if !(7..=9).contains(&array.len()) {
            return None;
        }
        let signature_val = v.get(6).filter(|v| !v.is_null());
        let text_val = v.get(7).filter(|v| !v.is_null());
        let extra_val = v.get(8).filter(|v| !v.is_null());
        let variant_count = usize::from(signature_val.is_some())
            + usize::from(text_val.is_some())
            + usize::from(extra_val.is_some());
        if variant_count > 1 {
            return None;
        }
        let extra = match extra_val {
            Some(ev) => Some(AnnotationExtra::from_json(ev)?),
            None => None,
        };
        let id = v.get(0)?.as_u64()?;
        let highlight = v.get(1)?.as_bool()?;
        let color = [
            u8::try_from(v[2][0].as_u64()?).ok()?,
            u8::try_from(v[2][1].as_u64()?).ok()?,
            u8::try_from(v[2][2].as_u64()?).ok()?,
        ];
        let width = v.get(3)?.as_f64()? as f32;
        let points = v
            .get(4)?
            .as_array()?
            .iter()
            .map(|p| Some([p.get(0)?.as_f64()? as f32, p.get(1)?.as_f64()? as f32]))
            .collect::<Option<Vec<_>>>()?;
        let rectangles = v.get(5)?.as_bool()?;
        let kind = if signature_val.is_some() {
            AnnotationKind::Stamp
        } else if text_val.is_some() {
            AnnotationKind::FreeText
        } else if let Some(extra) = &extra {
            extra.kind()
        } else if rectangles {
            AnnotationKind::Highlight
        } else {
            AnnotationKind::Ink
        };
        if expected.is_some_and(|expected| expected != kind) {
            return None;
        }
        if let Some(s) = signature_val {
            let image_width = u32::try_from(s[0].as_u64()?).ok()?;
            let image_height = u32::try_from(s[1].as_u64()?).ok()?;
            let payload = s[2].as_str()?;
            if let Some(ref_text) = payload.strip_prefix("ref:") {
                if !ref_text.bytes().all(|b| b.is_ascii_digit()) {
                    return None;
                }
                let ref_id = ref_text.parse::<u64>().ok()?;
                if id == 0
                    || ref_id == 0
                    || ref_id == id
                    || highlight
                    || rectangles
                    || !width.is_finite()
                    || !(0.5..=40.).contains(&width)
                    || points.len() != 2
                    || points
                        .iter()
                        .flatten()
                        .any(|c| !c.is_finite() || !(0.0..=1.0).contains(c))
                    || points[0][0] >= points[1][0]
                    || points[0][1] >= points[1][1]
                    || SignatureImage::byte_count(image_width, image_height).is_none()
                {
                    return None;
                }
                return Some(ParsedName::SignatureRef {
                    id,
                    color,
                    width,
                    points,
                    image_width,
                    image_height,
                    ref_id,
                });
            }
            let image = SignatureImage::decode(image_width, image_height, payload)?;
            let a = Self {
                id,
                highlight,
                color,
                width,
                points,
                rectangles,
                signature: Some(std::sync::Arc::new(image)),
                text: None,
                extra: None,
            };
            return a.valid().then_some(ParsedName::Complete(a));
        }
        let text = match text_val {
            Some(t) => {
                let arr = t.as_array()?;
                if arr.len() != 2 {
                    return None;
                }
                Some(TextAnnotation {
                    content: arr[0].as_str()?.to_owned(),
                    font_size: arr[1].as_f64()? as f32,
                })
            }
            None => None,
        };
        let a = Self {
            id,
            highlight,
            color,
            width,
            points,
            rectangles,
            signature: None,
            text,
            extra,
        };
        a.valid().then_some(ParsedName::Complete(a))
    }
}

/// Checks that signature/stamp annotations stay within the unique-asset and total-placement budgets.
pub fn within_signature_budget<'a>(annotations: impl Iterator<Item = &'a Annotation>) -> bool {
    let mut unique: Vec<&'a std::sync::Arc<SignatureImage>> = Vec::new();
    let mut placements = 0usize;
    for a in annotations {
        let Some(image) = &a.signature else {
            continue;
        };
        placements += 1;
        if placements > MAX_SIGNATURE_PLACEMENTS {
            return false;
        }
        if !unique.iter().any(|existing| {
            std::sync::Arc::ptr_eq(existing, image) || existing.as_ref() == image.as_ref()
        }) {
            if unique.len() >= MAX_SIGNATURES {
                return false;
            }
            unique.push(image);
        }
    }
    true
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
            text: None,
            extra: None,
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
            text: None,
            extra: None,
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

    #[test]
    fn text_annotation_roundtrip_and_validation() {
        let text = Annotation {
            id: 42,
            highlight: false,
            color: [30, 80, 180],
            width: 1.,
            points: vec![[0.1, 0.2], [0.4, 0.25]],
            rectangles: false,
            signature: None,
            text: Some(TextAnnotation {
                content: "24 November 2026".into(),
                font_size: 12.,
            }),
            extra: None,
        };
        assert!(text.valid());
        assert_eq!(text.kind(), AnnotationKind::FreeText);
        assert_eq!(
            Annotation::parse_with_kind(&text.name(), Some(AnnotationKind::FreeText)),
            Some(ParsedName::Complete(text.clone()))
        );
        assert!(Annotation::parse_with_kind(&text.name(), Some(AnnotationKind::Stamp)).is_none());

        let mut bad = text.clone();
        bad.text = Some(TextAnnotation {
            content: "   ".into(),
            font_size: 12.,
        });
        assert!(!bad.valid());

        bad.text = Some(TextAnnotation {
            content: "line1\nline2".into(),
            font_size: 12.,
        });
        assert!(!bad.valid());

        bad.text = Some(TextAnnotation {
            content: "a".repeat(MAX_TEXT_CHARS + 1),
            font_size: 12.,
        });
        assert!(!bad.valid());

        bad.text = Some(TextAnnotation {
            content: "ok".into(),
            font_size: 7.5,
        });
        assert!(!bad.valid());

        bad.text = Some(TextAnnotation {
            content: "ok".into(),
            font_size: 50.,
        });
        assert!(!bad.valid());

        // Cannot carry both signature and text.
        let mut both = text;
        both.signature = Some(std::sync::Arc::new(SignatureImage {
            width: 1,
            height: 1,
            rgba: vec![0, 0, 0, 255],
        }));
        assert!(!both.valid());
    }

    #[test]
    fn extra_annotations_roundtrip_and_validation() {
        let cases = [
            (
                Annotation {
                    id: 101,
                    highlight: false,
                    color: [40, 110, 220],
                    width: 1.5,
                    points: vec![[0.1, 0.2], [0.5, 0.23]],
                    rectangles: true,
                    signature: None,
                    text: None,
                    extra: Some(AnnotationExtra::Underline),
                },
                AnnotationKind::Underline,
            ),
            (
                Annotation {
                    id: 102,
                    highlight: false,
                    color: [230, 65, 75],
                    width: 1.5,
                    points: vec![[0.1, 0.3], [0.5, 0.33]],
                    rectangles: true,
                    signature: None,
                    text: None,
                    extra: Some(AnnotationExtra::StrikeOut),
                },
                AnnotationKind::StrikeOut,
            ),
            (
                Annotation {
                    id: 103,
                    highlight: false,
                    color: [230, 65, 75],
                    width: 2.,
                    points: vec![[0.1, 0.1], [0.4, 0.3]],
                    rectangles: false,
                    signature: None,
                    text: None,
                    extra: Some(AnnotationExtra::Shape(ShapeKind::Rectangle)),
                },
                AnnotationKind::Square,
            ),
            (
                Annotation {
                    id: 104,
                    highlight: false,
                    color: [30, 170, 95],
                    width: 2.,
                    points: vec![[0.2, 0.2], [0.6, 0.5]],
                    rectangles: false,
                    signature: None,
                    text: None,
                    extra: Some(AnnotationExtra::Shape(ShapeKind::Ellipse)),
                },
                AnnotationKind::Circle,
            ),
            (
                Annotation {
                    id: 105,
                    highlight: false,
                    color: [25, 25, 25],
                    width: 2.,
                    points: vec![[0.7, 0.6], [0.2, 0.2]],
                    rectangles: false,
                    signature: None,
                    text: None,
                    extra: Some(AnnotationExtra::Shape(ShapeKind::Arrow)),
                },
                AnnotationKind::Line,
            ),
            (
                Annotation {
                    id: 106,
                    highlight: false,
                    color: [255, 210, 0],
                    width: 1.5,
                    points: vec![[0.15, 0.15], [0.21, 0.21]],
                    rectangles: false,
                    signature: None,
                    text: None,
                    extra: Some(AnnotationExtra::Note("Periksa pasal 3.\nRevisi nominal.".into())),
                },
                AnnotationKind::Text,
            ),
            (
                Annotation {
                    id: 107,
                    highlight: false,
                    color: [30, 170, 95],
                    width: 2.,
                    points: vec![[0.3, 0.3], [0.35, 0.35]],
                    rectangles: false,
                    signature: None,
                    text: None,
                    extra: Some(AnnotationExtra::Symbol(FillSymbol::Check)),
                },
                AnnotationKind::Stamp,
            ),
        ];

        for (item, expected_kind) in cases {
            assert!(item.valid(), "expected valid for {:?}", item.extra);
            assert_eq!(item.kind(), expected_kind);
            assert_eq!(
                Annotation::parse_with_kind(&item.name(), Some(expected_kind)),
                Some(ParsedName::Complete(item.clone()))
            );
            let wrong_kind = if expected_kind == AnnotationKind::Ink {
                AnnotationKind::Highlight
            } else {
                AnnotationKind::Ink
            };
            assert!(Annotation::parse_with_kind(&item.name(), Some(wrong_kind)).is_none());
        }

        // Empty sticky note or control char (other than \n) must fail validation.
        let bad_note = Annotation {
            id: 108,
            highlight: false,
            color: [255, 210, 0],
            width: 1.5,
            points: vec![[0.1, 0.1], [0.15, 0.15]],
            rectangles: false,
            signature: None,
            text: None,
            extra: Some(AnnotationExtra::Note("   ".into())),
        };
        assert!(!bad_note.valid());
    }

    #[test]
    fn signature_ref_roundtrip_and_budget_limits() {
        let asset = std::sync::Arc::new(SignatureImage {
            width: 2,
            height: 2,
            rgba: [20, 40, 80, 255].repeat(4),
        });
        let second = Annotation {
            id: 9,
            highlight: false,
            color: [0; 3],
            width: 1.,
            points: vec![[0.2, 0.3], [0.5, 0.4]],
            rectangles: false,
            signature: Some(asset.clone()),
            text: None,
            extra: None,
        };
        let Some(encoded) = second.name_with_signature_ref(3) else {
            panic!("signature ref name");
        };
        let Some(parsed) = Annotation::parse_with_kind(&encoded, Some(AnnotationKind::Stamp))
        else {
            panic!("parsed signature ref");
        };
        assert!(matches!(
            parsed,
            ParsedName::SignatureRef {
                id: 9,
                ref_id: 3,
                image_width: 2,
                image_height: 2,
                ..
            }
        ));
        assert_eq!(parsed.resolve_signature(asset.clone()), Some(second));

        // Self-reference is rejected.
        assert!(
            Annotation::parse_with_kind(
                &format!("{PREFIX}[9,false,[0,0,0],1,[[0.2,0.3],[0.5,0.4]],false,[2,2,\"ref:9\"]]"),
                Some(AnnotationKind::Stamp),
            )
            .is_none()
        );

        // One unique signature placed 24 times is within budget.
        let many_same: Vec<_> = (1..=24)
            .map(|id| Annotation {
                id,
                highlight: false,
                color: [0; 3],
                width: 1.,
                points: vec![[0.1, 0.1], [0.2, 0.2]],
                rectangles: false,
                signature: Some(asset.clone()),
                text: None,
                extra: None,
            })
            .collect();
        assert!(within_signature_budget(many_same.iter()));

        // Nine distinct signatures exceed MAX_SIGNATURES.
        let distinct: Vec<_> = (1..=9u8)
            .map(|i| Annotation {
                id: u64::from(i),
                highlight: false,
                color: [0; 3],
                width: 1.,
                points: vec![[0.1, 0.1], [0.2, 0.2]],
                rectangles: false,
                signature: Some(std::sync::Arc::new(SignatureImage {
                    width: 1,
                    height: 1,
                    rgba: vec![i, 0, 0, 255],
                })),
                text: None,
                extra: None,
            })
            .collect();
        assert!(within_signature_budget(distinct[..8].iter()));
        assert!(!within_signature_budget(distinct.iter()));
    }
}
