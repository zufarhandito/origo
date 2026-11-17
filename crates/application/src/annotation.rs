//! Raster projection shared by the viewer and PNG export. PDF output stays vector.
use pdf_domain::annotation::{Annotation, rotate};

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

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn highlighter_preserves_black_text_and_does_not_stack_its_own_crossing() {
        let a = Annotation {
            id: 1,
            highlight: true,
            color: [255, 210, 0],
            width: 4.,
            points: vec![[0.1, 0.5], [0.9, 0.5], [0.1, 0.5]],
            rectangles: false,
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
