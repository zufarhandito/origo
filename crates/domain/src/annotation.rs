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
}
impl Annotation {
    pub fn valid(&self) -> bool {
        self.id > 0
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
                self.rectangles
            ])
        )
    }
    pub fn from_name(name: &str) -> Option<Self> {
        if name.len() > 160_000 {
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
        };
        a.valid().then_some(a)
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
    fn roundtrip_and_invalid_metadata_are_bounded() {
        let a = Annotation {
            id: 17,
            highlight: true,
            color: [255, 210, 0],
            width: 12.,
            points: vec![[0.1, 0.2], [0.7, 0.8]],
            rectangles: false,
        };
        assert_eq!(Annotation::from_name(&a.name()), Some(a.clone()));
        let mut bad = a;
        bad.points[0][0] = f32::INFINITY;
        assert!(!bad.valid());
        assert!(Annotation::from_name(&format!("{PREFIX}{}", "x".repeat(160_001))).is_none());
        for turn in 0..4 {
            let point = [0.25, 0.75];
            assert_eq!(rotate(rotate(point, turn), (4 - turn) % 4), point);
        }
    }
}
