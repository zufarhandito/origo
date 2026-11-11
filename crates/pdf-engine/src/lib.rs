use pdf_application::text::{TextChar, TextExtractor, TextPage, TextRect, TextRequest};
use pdf_application::{
    InspectedDocument, PageGeometry, PageRenderer, PdfInspector, RenderError, RenderRequest,
    RenderedPage, SourceFingerprint,
};
use pdfium_render::prelude::*;
use std::{
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    sync::{Arc, Mutex, OnceLock, mpsc},
    time::{Duration, UNIX_EPOCH},
};

static PDFIUM_LIBRARY: Mutex<Option<PathBuf>> = Mutex::new(None);
const SESSION_RENDERS: u64 = 64;
const IDLE_TIMEOUT: Duration = Duration::from_secs(15);

#[derive(Clone, Copy)]
pub struct ResourceLimits {
    pub max_file_bytes: u64,
    pub max_pages: i32,
    pub max_bitmap_dimension: u32,
    pub max_bitmap_pixels: u64,
}
impl Default for ResourceLimits {
    fn default() -> Self {
        Self {
            max_file_bytes: 128 * 1024 * 1024,
            max_pages: 1024,
            max_bitmap_dimension: 8192,
            max_bitmap_pixels: 16 * 1024 * 1024,
        }
    }
}

#[derive(Default)]
struct Counters {
    opens: AtomicU64,
    renders: AtomicU64,
}
#[derive(Clone, Copy, Debug)]
pub struct SessionStats {
    pub document_opens: u64,
    pub renders: u64,
}
#[derive(Clone)]
pub struct PdfiumRenderer {
    library: PathBuf,
    limits: ResourceLimits,
    worker: Arc<OnceLock<Result<mpsc::SyncSender<Job>, String>>>,
    counters: Arc<Counters>,
}
enum Operation {
    Inspect,
    Render(RenderRequest),
    Text(TextRequest),
}
enum Output {
    Inspected(InspectedDocument),
    Rendered(RenderedPage),
    Text(TextPage),
}
struct Job {
    path: PathBuf,
    expected: Option<SourceFingerprint>,
    operation: Operation,
    reply: mpsc::SyncSender<Result<Output, RenderError>>,
}
impl PdfiumRenderer {
    pub fn new(sidecars: &Path) -> Self {
        Self {
            library: sidecars.join("pdfium/bin/pdfium.dll"),
            limits: ResourceLimits::default(),
            worker: Arc::new(OnceLock::new()),
            counters: Arc::new(Counters::default()),
        }
    }
    pub fn with_limits(mut self, limits: ResourceLimits) -> Result<Self, RenderError> {
        if limits.max_pages < 1
            || limits.max_pages > i32::from(u16::MAX)
            || limits.max_file_bytes == 0
            || !(32..=8192).contains(&limits.max_bitmap_dimension)
            || limits.max_bitmap_pixels < 1024
            || limits.max_bitmap_pixels > 16 * 1024 * 1024
        {
            return Err(RenderError::ResourceLimit);
        }
        self.limits = limits;
        self.worker = Arc::new(OnceLock::new());
        self.counters = Arc::new(Counters::default());
        Ok(self)
    }
    pub fn session_stats(&self) -> SessionStats {
        SessionStats {
            document_opens: self.counters.opens.load(Ordering::Relaxed),
            renders: self.counters.renders.load(Ordering::Relaxed),
        }
    }
    fn request(
        &self,
        path: &Path,
        expected: Option<SourceFingerprint>,
        operation: Operation,
    ) -> Result<Output, RenderError> {
        let sender = self
            .worker
            .get_or_init(|| {
                let (sender, receiver) = mpsc::sync_channel(1);
                let library = self.library.clone();
                let limits = self.limits;
                let counters = self.counters.clone();
                std::thread::Builder::new()
                    .name("pdfium-document".into())
                    .spawn(move || worker(receiver, library, limits, counters))
                    .map_err(|error| error.to_string())?;
                Ok(sender)
            })
            .as_ref()
            .map_err(|error| RenderError::DependencyUnavailable(error.clone()))?;
        let (reply, receiver) = mpsc::sync_channel(1);
        sender
            .send(Job {
                path: path.into(),
                expected,
                operation,
                reply,
            })
            .map_err(|_| worker_error())?;
        receiver.recv().map_err(|_| worker_error())?
    }
}
fn worker_error() -> RenderError {
    RenderError::DependencyUnavailable("PDFium worker berhenti; buka ulang aplikasi".into())
}
fn initialize(library: &Path) -> Result<Pdfium, RenderError> {
    let mut initialized = PDFIUM_LIBRARY.lock().map_err(|_| worker_error())?;
    match initialized.as_ref() {
        None => {
            let bindings = Pdfium::bind_to_library(library)
                .map_err(|e| RenderError::DependencyUnavailable(e.to_string()))?;
            let pdfium = Pdfium::new(bindings);
            *initialized = Some(library.into());
            Ok(pdfium)
        }
        Some(path) if path == library => Ok(Pdfium::default()),
        Some(_) => Err(RenderError::DependencyUnavailable(
            "A different PDFium library is already initialized".into(),
        )),
    }
}
fn worker(
    receiver: mpsc::Receiver<Job>,
    library: PathBuf,
    limits: ResourceLimits,
    counters: Arc<Counters>,
) {
    // Construct Pdfium and its borrowed document on this thread. No self-reference,
    // native handle, or borrowed object crosses the channel.
    let pdfium = initialize(&library);
    let mut session = None;
    let mut session_key = None;
    let mut renders = 0;
    loop {
        let job = match receiver.recv_timeout(IDLE_TIMEOUT) {
            Ok(job) => job,
            Err(mpsc::RecvTimeoutError::Timeout) => {
                if let Ok(_guard) = PDFIUM_LIBRARY.lock() {
                    session = None;
                    session_key = None;
                }
                continue;
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        };
        let result = (|| {
            let pdfium = pdfium
                .as_ref()
                .map_err(|e| RenderError::DependencyUnavailable(e.to_string()))?;
            let _guard = PDFIUM_LIBRARY.lock().map_err(|_| worker_error())?;
            let before = fingerprint(&job.path)?;
            if before.bytes > limits.max_file_bytes {
                return Err(RenderError::ResourceLimit);
            }
            if job.expected.is_some_and(|expected| expected != before) {
                session = None;
                session_key = None;
                return Err(RenderError::SourceChanged);
            }
            let key = (job.path.clone(), before);
            if session_key.as_ref() != Some(&key) || renders >= SESSION_RENDERS {
                session = None;
                session_key = None;
                renders = 0;
                let document = pdfium
                    .load_pdf_from_file(&job.path, None)
                    .map_err(|_| RenderError::InvalidPdf)?;
                counters.opens.fetch_add(1, Ordering::Relaxed);
                if document.pages().is_empty() {
                    return Err(RenderError::InvalidPdf);
                }
                if document.pages().len() > limits.max_pages {
                    return Err(RenderError::ResourceLimit);
                }
                session = Some(document);
                session_key = Some(key);
            }
            let document = session.as_ref().ok_or_else(worker_error)?;
            let result = match &job.operation {
                Operation::Inspect => Output::Inspected(InspectedDocument {
                    path: job.path.clone(),
                    fingerprint: before,
                    pages: document
                        .pages()
                        .iter()
                        .map(|page| geometry(&page))
                        .collect::<Result<Vec<_>, _>>()?,
                }),
                Operation::Render(request) => {
                    renders += 1;
                    counters.renders.fetch_add(1, Ordering::Relaxed);
                    Output::Rendered(render(document, request, limits)?)
                }
                Operation::Text(request) => {
                    renders += 1;
                    Output::Text(extract(document, request.page_index)?)
                }
            };
            if fingerprint(&job.path)? != before {
                session = None;
                session_key = None;
                return Err(RenderError::SourceChanged);
            }
            Ok(result)
        })();
        if result.is_err()
            && let Ok(_guard) = PDFIUM_LIBRARY.lock()
        {
            session = None;
            session_key = None;
        }
        let _ = job.reply.send(result);
    }
    // Serialize native document destruction with all other adapter operations.
    if let Ok(_guard) = PDFIUM_LIBRARY.lock() {
        drop(session.take());
    }
}
impl PdfInspector for PdfiumRenderer {
    fn inspect(&self, source: &Path) -> Result<InspectedDocument, RenderError> {
        match self.request(source, None, Operation::Inspect)? {
            Output::Inspected(info) => Ok(info),
            _ => Err(worker_error()),
        }
    }
}
impl PageRenderer for PdfiumRenderer {
    fn render_page(&self, request: &RenderRequest) -> Result<RenderedPage, RenderError> {
        match self.request(
            &request.path,
            request.fingerprint,
            Operation::Render(request.clone()),
        )? {
            Output::Rendered(page) => Ok(page),
            _ => Err(worker_error()),
        }
    }
}
impl TextExtractor for PdfiumRenderer {
    fn extract_text(&self, request: &TextRequest) -> Result<TextPage, RenderError> {
        match self.request(
            &request.path,
            Some(request.fingerprint),
            Operation::Text(request.clone()),
        )? {
            Output::Text(page) => Ok(page),
            _ => Err(worker_error()),
        }
    }
}
fn extract(document: &PdfDocument<'_>, index: u16) -> Result<TextPage, RenderError> {
    let page = document
        .pages()
        .get(i32::from(index))
        .map_err(|_| RenderError::InvalidPage)?;
    let text = page.text().map_err(|_| RenderError::InvalidPdf)?;
    if text.len() < 0 {
        return Err(RenderError::InvalidPdf);
    }
    if text.len() > 200_000 {
        return Err(RenderError::ResourceLimit);
    }
    let chars = text.chars();
    // This config is for coordinate conversion only; no bitmap is allocated.
    let config = PdfRenderConfig::new().set_fixed_size(16384, 16384);
    let mut result = Vec::with_capacity(chars.len());
    for ch in chars.iter() {
        let unicode = ch
            .unicode_char()
            .filter(|c| *c != '\0')
            .unwrap_or('\u{fffd}');
        let bounds = if unicode.is_control() {
            None
        } else {
            ch.loose_bounds()
                .or_else(|_| ch.tight_bounds())
                .ok()
                .and_then(|rect| {
                    let a = page
                        .points_to_pixels(rect.left(), rect.top(), &config)
                        .ok()?;
                    let b = page
                        .points_to_pixels(rect.right(), rect.bottom(), &config)
                        .ok()?;
                    let bounds = TextRect {
                        left: a.0.min(b.0) as f32 / 16384.,
                        top: a.1.min(b.1) as f32 / 16384.,
                        right: a.0.max(b.0) as f32 / 16384.,
                        bottom: a.1.max(b.1) as f32 / 16384.,
                    };
                    (bounds.right > bounds.left && bounds.bottom > bounds.top).then_some(bounds)
                })
        };
        result.push(TextChar { unicode, bounds });
    }
    Ok(TextPage { chars: result })
}
fn fingerprint(path: &Path) -> Result<SourceFingerprint, RenderError> {
    let metadata = std::fs::metadata(path).map_err(|_| RenderError::InvalidPdf)?;
    if !metadata.is_file() {
        return Err(RenderError::InvalidPdf);
    }
    Ok(SourceFingerprint {
        bytes: metadata.len(),
        modified_ns: metadata
            .modified()
            .map_err(|_| RenderError::InvalidPdf)?
            .duration_since(UNIX_EPOCH)
            .map_err(|_| RenderError::InvalidPdf)?
            .as_nanos(),
    })
}

fn geometry(page: &PdfPage<'_>) -> Result<PageGeometry, RenderError> {
    let quarter_turns = match page.rotation().map_err(|_| RenderError::InvalidPdf)? {
        PdfPageRenderRotation::None => 0,
        PdfPageRenderRotation::Degrees90 => 1,
        PdfPageRenderRotation::Degrees180 => 2,
        PdfPageRenderRotation::Degrees270 => 3,
    };
    let (width, height) = (page.width().value, page.height().value);
    if !width.is_finite()
        || !height.is_finite()
        || width <= 0.
        || height <= 0.
        || width > 100000.
        || height > 100000.
    {
        return Err(RenderError::ResourceLimit);
    }
    Ok(PageGeometry {
        width_points: width,
        height_points: height,
        quarter_turns,
    })
}

fn render(
    document: &PdfDocument<'_>,
    request: &RenderRequest,
    limits: ResourceLimits,
) -> Result<RenderedPage, RenderError> {
    let page = document
        .pages()
        .get(i32::from(request.page_index))
        .map_err(|_| RenderError::InvalidPage)?;
    let geometry = geometry(&page)?;
    let aspect = f64::from(geometry.height_points / geometry.width_points);
    let requested = f64::from(request.target_width.clamp(32, limits.max_bitmap_dimension));
    let factor = (f64::from(limits.max_bitmap_dimension) / (requested * aspect))
        .min((limits.max_bitmap_pixels as f64 / (requested * requested * aspect)).sqrt())
        .min(1.);
    let width = (requested * factor * 0.999).floor().max(1.) as i32;
    let bitmap = page
        .render_with_config(
            &PdfRenderConfig::new()
                .set_target_width(width)
                .set_maximum_width(limits.max_bitmap_dimension as i32)
                .set_maximum_height(limits.max_bitmap_dimension as i32),
        )
        .map_err(|_| RenderError::InvalidPdf)?;
    let width = u32::try_from(bitmap.width()).map_err(|_| RenderError::ResourceLimit)?;
    let height = u32::try_from(bitmap.height()).map_err(|_| RenderError::ResourceLimit)?;
    if width == 0 || height == 0 || u64::from(width) * u64::from(height) > limits.max_bitmap_pixels
    {
        return Err(RenderError::ResourceLimit);
    }
    let rgba = bitmap.as_rgba_bytes();
    if rgba.len() != width as usize * height as usize * 4 {
        return Err(RenderError::ResourceLimit);
    }
    Ok(RenderedPage {
        width,
        height,
        rgba,
        page_count: document.pages().len() as u16,
        clockwise_quarter_turns: geometry.quarter_turns,
    })
}
