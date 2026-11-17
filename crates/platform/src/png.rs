//! Bounded PNG encoding/decode validation and shared safe folder publication.
use super::*;
use image::{ImageDecoder, ImageEncoder};
use pdf_application::{
    RenderedPage,
    png::{PngError, PngWriter},
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
            return Err(std::io::Error::other("PNG output limit"));
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
impl PngWriter for LocalPngWriter {
    fn write_png(
        &self,
        raster: &RenderedPage,
        path: &Path,
        remaining: u64,
        control: &ExportControl,
    ) -> Result<u64, PngError> {
        control.checkpoint()?;
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
}
