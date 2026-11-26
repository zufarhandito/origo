//! Bounded PNG encoding/decode validation and shared safe folder publication.
use super::*;
use image::{ImageDecoder, ImageEncoder};
use pdf_application::{
    RenderedPage,
    png::{ImageExportFormat, PngError, PngWriter},
    split::{SplitError, SplitOutput},
};
pub struct LocalPngOutput;
impl SplitOutput for LocalPngOutput {
    type Transaction = super::split::SafeSplit;
    fn stage(
        &self,
        plan: &ExportPlan,
        destination: &Path,
        control: &ExportControl,
    ) -> Result<Self::Transaction, SplitError> {
        super::split::stage_batch(plan, destination, control, ".pdf-studio-png-")
    }
}
pub struct LocalPngWriter;
struct LimitedWriter<'a> {
    file: &'a mut File,
    control: &'a ExportControl,
    remaining: u64,
    written: u64,
    exceeded: bool,
}
impl Write for LimitedWriter<'_> {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.control.checkpoint().map_err(std::io::Error::other)?;
        if bytes.len() as u64 > self.remaining {
            self.exceeded = true;
            return Err(std::io::Error::other("image output limit"));
        }
        let count = self.file.write(bytes)?;
        self.remaining -= count as u64;
        self.written += count as u64;
        Ok(count)
    }
    fn flush(&mut self) -> std::io::Result<()> {
        self.file.flush()
    }
}
fn validate_raster(raster: &RenderedPage) -> Result<u64, PngError> {
    if raster.width == 0
        || raster.height == 0
        || raster.width > pdf_application::png::MAX_RASTER_SIDE
        || raster.height > pdf_application::png::MAX_RASTER_SIDE
        || u64::from(raster.width) * u64::from(raster.height)
            > pdf_application::png::MAX_RASTER_PIXELS
    {
        return Err(PngError::Dimensions);
    }
    let expected_bytes = u64::from(raster.width) * u64::from(raster.height) * 4;
    if raster.rgba.len() as u64 != expected_bytes {
        return Err(PngError::Encoding);
    }
    Ok(expected_bytes)
}
impl PngWriter for LocalPngWriter {
    fn write_png(
        &self,
        raster: &RenderedPage,
        path: &Path,
        remaining: u64,
        control: &ExportControl,
    ) -> Result<u64, PngError> {
        control.checkpoint()?;
        let expected_bytes = validate_raster(raster)?;
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)
            .map_err(|_| PngError::Encoding)?;
        let mut limited = LimitedWriter {
            file: &mut file,
            control,
            remaining,
            written: 0,
            exceeded: false,
        };
        let encoded = image::codecs::png::PngEncoder::new(&mut limited).write_image(
            &raster.rgba,
            raster.width,
            raster.height,
            image::ExtendedColorType::Rgba8,
        );
        control.checkpoint()?;
        if limited.exceeded {
            return Err(PngError::OutputLimit);
        }
        encoded.map_err(|_| PngError::Encoding)?;
        let bytes = limited.written;
        file.sync_all().map_err(|_| PngError::Encoding)?;
        drop(file);
        let file = File::open(path).map_err(|_| PngError::Encoding)?;
        let mut decoder = image::codecs::png::PngDecoder::new(std::io::BufReader::new(file))
            .map_err(|_| PngError::Encoding)?;
        if decoder.dimensions() != (raster.width, raster.height)
            || decoder.total_bytes() != expected_bytes
        {
            return Err(PngError::Encoding);
        }
        let mut limits = image::Limits::default();
        limits.max_image_width = Some(8192);
        limits.max_image_height = Some(8192);
        limits.max_alloc = Some(128 * 1024 * 1024);
        decoder.set_limits(limits).map_err(|_| PngError::Encoding)?;
        let mut decoded = vec![0; raster.rgba.len()];
        decoder
            .read_image(&mut decoded)
            .map_err(|_| PngError::Encoding)?;
        if decoded != raster.rgba {
            return Err(PngError::Encoding);
        }
        control.checkpoint()?;
        Ok(bytes)
    }

    fn write_image(
        &self,
        raster: &RenderedPage,
        path: &Path,
        format: ImageExportFormat,
        remaining: u64,
        control: &ExportControl,
    ) -> Result<u64, PngError> {
        match format {
            ImageExportFormat::Png => self.write_png(raster, path, remaining, control),
            ImageExportFormat::Jpeg { quality } => {
                control.checkpoint()?;
                validate_raster(raster)?;
                let mut rgb = Vec::with_capacity(raster.width as usize * raster.height as usize * 3);
                for px in raster.rgba.as_chunks::<4>().0 {
                    rgb.push(px[0]);
                    rgb.push(px[1]);
                    rgb.push(px[2]);
                }
                let mut file = OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .open(path)
                    .map_err(|_| PngError::Encoding)?;
                let mut limited = LimitedWriter {
                    file: &mut file,
                    control,
                    remaining,
                    written: 0,
                    exceeded: false,
                };
                let encoded = image::codecs::jpeg::JpegEncoder::new_with_quality(
                    &mut limited,
                    quality.clamp(10, 100),
                )
                .write_image(
                    &rgb,
                    raster.width,
                    raster.height,
                    image::ExtendedColorType::Rgb8,
                );
                control.checkpoint()?;
                if limited.exceeded {
                    return Err(PngError::OutputLimit);
                }
                encoded.map_err(|_| PngError::Encoding)?;
                let bytes = limited.written;
                file.sync_all().map_err(|_| PngError::Encoding)?;
                drop(file);
                let file = File::open(path).map_err(|_| PngError::Encoding)?;
                let mut decoder =
                    image::codecs::jpeg::JpegDecoder::new(std::io::BufReader::new(file))
                        .map_err(|_| PngError::Encoding)?;
                if decoder.dimensions() != (raster.width, raster.height) {
                    return Err(PngError::Encoding);
                }
                let mut limits = image::Limits::default();
                limits.max_image_width = Some(8192);
                limits.max_image_height = Some(8192);
                limits.max_alloc = Some(128 * 1024 * 1024);
                decoder.set_limits(limits).map_err(|_| PngError::Encoding)?;
                let mut decoded = vec![0; decoder.total_bytes() as usize];
                decoder
                    .read_image(&mut decoded)
                    .map_err(|_| PngError::Encoding)?;
                control.checkpoint()?;
                Ok(bytes)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn jpeg_writer_encodes_validates_and_enforces_limit() -> Result<(), Box<dyn std::error::Error>> {
        let dir = tempfile::tempdir()?;
        let out = dir.path().join("page-0001.jpg");
        let raster = RenderedPage {
            width: 32,
            height: 24,
            rgba: vec![200u8; 32 * 24 * 4],
            page_count: 1,
            clockwise_quarter_turns: 0,
        };
        let control = ExportControl::with_budget(Duration::from_secs(30));
        let written = LocalPngWriter.write_image(
            &raster,
            &out,
            ImageExportFormat::Jpeg { quality: 90 },
            1024 * 1024,
            &control,
        )?;
        assert!(written > 100);
        let raw = fs::read(&out)?;
        assert!(raw.starts_with(&[0xFF, 0xD8]));
        let too_small = dir.path().join("page-0002.jpg");
        let err = LocalPngWriter.write_image(
            &raster,
            &too_small,
            ImageExportFormat::Jpeg { quality: 90 },
            32,
            &control,
        );
        assert!(matches!(err, Err(PngError::OutputLimit)));
        Ok(())
    }
}
