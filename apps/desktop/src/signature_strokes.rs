//! Shared ink geometry for the editor pad and the embedded signature image.
use image::{Rgba, RgbaImage};
use tiny_skia::{FillRule, LineCap, LineJoin, Mask, PathBuilder, Stroke, Transform};

pub(super) const WIDTH: u32 = 512;
pub(super) const HEIGHT: u32 = 256;
const OVERSAMPLE: u32 = 4;
const MIN_SAMPLE_GAP: f32 = 0.8;
const MAX_CORNER_TRIM: f32 = 3.0;

pub(super) type InkPoint = [f32; 2];

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum Segment {
    LineTo(InkPoint),
    CurveTo { control: InkPoint, end: InkPoint },
}

#[derive(Debug, PartialEq)]
pub(super) struct InkPath {
    pub start: InkPoint,
    pub segments: Vec<Segment>,
}

pub(super) fn stroke_width(thickness: usize) -> f32 {
    [3., 5., 7.][thickness.min(2)]
}

/// Coalesce subpixel samples and soften shallow turns without rounding cusps.
/// Geometry is in output pixels so the distance threshold is isotropic.
pub(super) fn smooth(stroke: &[InkPoint]) -> Option<InkPath> {
    let mut samples = stroke
        .iter()
        .filter(|[x, y]| x.is_finite() && y.is_finite())
        .map(|&[x, y]| {
            [
                x.clamp(0., 1.) * WIDTH as f32,
                y.clamp(0., 1.) * HEIGHT as f32,
            ]
        });
    let start = samples.next()?;
    let mut samples = samples.peekable();
    let mut points = Vec::with_capacity(stroke.len());
    points.push(start);
    let mut previous = start;
    while let Some(point) = samples.next() {
        let distance = (point[0] - previous[0]).hypot(point[1] - previous[1]);
        if distance >= MIN_SAMPLE_GAP || (distance > 0. && samples.peek().is_none()) {
            previous = point;
            points.push(point);
        }
    }

    let mut segments = Vec::with_capacity(points.len());
    for triple in points.windows(3) {
        let [a, b, c] = [triple[0], triple[1], triple[2]];
        let incoming = [b[0] - a[0], b[1] - a[1]];
        let outgoing = [c[0] - b[0], c[1] - b[1]];
        let incoming_len = incoming[0].hypot(incoming[1]);
        let outgoing_len = outgoing[0].hypot(outgoing[1]);
        let forwardness = incoming[0] * outgoing[0] + incoming[1] * outgoing[1];
        let scale = incoming_len * outgoing_len;
        if forwardness <= 0.5 * scale || forwardness >= 0.999 * scale {
            segments.push(Segment::LineTo(b));
            continue;
        }

        let before = (0.25 * incoming_len).min(MAX_CORNER_TRIM) / incoming_len;
        let after = (0.25 * outgoing_len).min(MAX_CORNER_TRIM) / outgoing_len;
        let entry = [b[0] - incoming[0] * before, b[1] - incoming[1] * before];
        let exit = [b[0] + outgoing[0] * after, b[1] + outgoing[1] * after];
        segments.push(Segment::LineTo(entry));
        segments.push(Segment::CurveTo {
            control: b,
            end: exit,
        });
    }
    if points.len() > 1 {
        segments.push(Segment::LineTo(previous));
    }
    Some(InkPath { start, segments })
}

pub(super) fn rasterize(
    strokes: &[Vec<InkPoint>],
    color: [u8; 3],
    thickness: usize,
) -> Option<RgbaImage> {
    let width = stroke_width(thickness);
    let mut paths = strokes
        .iter()
        .filter_map(|stroke| smooth(stroke))
        .peekable();
    paths.peek()?;
    let mut mask = Mask::new(WIDTH * OVERSAMPLE, HEIGHT * OVERSAMPLE)?;
    let transform = Transform::from_scale(OVERSAMPLE as f32, OVERSAMPLE as f32);
    let stroke = Stroke {
        width,
        line_cap: LineCap::Round,
        line_join: LineJoin::Round,
        ..Stroke::default()
    };
    // Keep strokes separate to bound active edges. Rasterize a one-byte mask
    // per sample and derive antialiasing from area coverage when downsampling.
    for path in paths {
        let [x, y] = path.start;
        let mut builder = PathBuilder::new();
        if path.segments.is_empty() {
            builder.push_circle(x, y, width / 2.);
            if let Some(dot) = builder.finish() {
                mask.fill_path(&dot, FillRule::Winding, false, transform);
            }
            continue;
        }

        builder.move_to(x, y);
        for segment in path.segments {
            match segment {
                Segment::LineTo([x, y]) => builder.line_to(x, y),
                Segment::CurveTo {
                    control: [cx, cy],
                    end: [x, y],
                } => {
                    builder.quad_to(cx, cy, x, y);
                }
            }
        }
        if let Some(outline) = builder
            .finish()
            .and_then(|line| line.stroke(&stroke, OVERSAMPLE as f32))
        {
            mask.fill_path(&outline, FillRule::Winding, false, transform);
        }
    }

    let pixels = mask.data();
    let stride = mask.width() as usize;
    let [r, g, b] = color;
    Some(RgbaImage::from_fn(WIDTH, HEIGHT, |x, y| {
        let mut coverage = 0u32;
        for dy in 0..OVERSAMPLE {
            let row = (y * OVERSAMPLE + dy) as usize * stride;
            for dx in 0..OVERSAMPLE {
                coverage += u32::from(pixels[row + (x * OVERSAMPLE + dx) as usize]);
            }
        }
        let samples = OVERSAMPLE * OVERSAMPLE;
        let alpha = ((coverage + samples / 2) / samples) as u8;
        if alpha == 0 {
            Rgba([0; 4])
        } else {
            Rgba([r, g, b, alpha])
        }
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn smooth_keeps_endpoints_and_sharp_turns() -> anyhow::Result<()> {
        let stroke = [[0., 0.], [0.25, 0.], [0.25, 0.5], [0.5, 0.875]];
        let path = smooth(&stroke).ok_or_else(|| anyhow::anyhow!("missing path"))?;
        assert_eq!(path.start, [0., 0.]);
        assert_eq!(path.segments.first(), Some(&Segment::LineTo([128., 0.])));
        assert_eq!(path.segments.last(), Some(&Segment::LineTo([256., 224.])));
        assert!(
            path.segments
                .iter()
                .any(|segment| matches!(segment, Segment::CurveTo { .. }))
        );
        Ok(())
    }

    #[test]
    fn anti_aliases_diagonals_and_preserves_separate_dots() -> anyhow::Result<()> {
        assert!(rasterize(&[], [0; 3], 0).is_none());
        let image = rasterize(
            &[vec![[0.2, 0.2], [0.8, 0.8]], vec![[0.8, 0.2]]],
            [30, 80, 180],
            0,
        )
        .ok_or_else(|| anyhow::anyhow!("missing raster"))?;
        assert_eq!(image.dimensions(), (512, 256));
        assert!(image.pixels().any(|pixel| (1..255).contains(&pixel[3])));
        assert!(image.get_pixel(409, 51)[3] > 0);
        assert_eq!(image.get_pixel(256, 51)[3], 0);
        assert!(
            image
                .pixels()
                .filter(|p| p[3] > 0)
                .all(|p| p.0[..3] == [30, 80, 180])
        );
        Ok(())
    }
}
