//! Raster projection shared by the viewer and PNG export.
use pdf_domain::annotation::{Annotation, SignatureImage, rotate};

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
        if let Some(image) = &a.signature {
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
}
