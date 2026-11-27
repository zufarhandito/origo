use pdf_application::text::{TextChar, TextExtractor, TextPage, TextRect, TextRequest};
use pdf_application::{
    ExactPageRenderer, ExactRenderRequest, InspectedDocument, PageGeometry, PageRenderer,
    PdfInspector, RenderError, RenderRequest, RenderedPage, SourceFingerprint,
};
use pdf_domain::annotation::{
    Annotation, AnnotationKind, MAX_PAGE_ANNOTATIONS, MAX_SIGNATURE_PLACEMENTS, MAX_SIGNATURES,
    PREFIX, ParsedName, SignatureImage,
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
    active_documents: AtomicU64,
}
#[derive(Clone, Copy, Debug)]
pub struct SessionStats {
    pub document_opens: u64,
    pub renders: u64,
    pub active_documents: u64,
}
struct SecretString(String);

impl SecretString {
    fn as_str(&self) -> &str {
        &self.0
    }
}

impl Clone for SecretString {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl Drop for SecretString {
    fn drop(&mut self) {
        let mut bytes = std::mem::take(&mut self.0).into_bytes();
        bytes.fill(0);
    }
}

#[derive(Default)]
struct PasswordStore {
    entries: Mutex<std::collections::HashMap<String, (SecretString, u64)>>,
    revision: AtomicU64,
}

impl PasswordStore {
    fn key(path: &Path) -> String {
        let resolved = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
        #[cfg(windows)]
        {
            let text = resolved.to_string_lossy().replace('/', "\\");
            text.strip_prefix("\\\\?\\").unwrap_or(&text).to_lowercase()
        }
        #[cfg(not(windows))]
        {
            resolved.to_string_lossy().into_owned()
        }
    }

    fn set(&self, path: &Path, password: String) {
        let rev = self.revision.fetch_add(1, Ordering::AcqRel) + 1;
        if let Ok(mut entries) = self.entries.lock() {
            entries.insert(Self::key(path), (SecretString(password), rev));
        }
    }

    fn clear(&self, path: &Path) {
        self.revision.fetch_add(1, Ordering::AcqRel);
        if let Ok(mut entries) = self.entries.lock() {
            entries.remove(&Self::key(path));
        }
    }

    fn has(&self, path: &Path) -> bool {
        self.entries
            .lock()
            .ok()
            .is_some_and(|entries| entries.contains_key(&Self::key(path)))
    }

    fn get(&self, path: &Path) -> Option<(SecretString, u64)> {
        self.entries
            .lock()
            .ok()
            .and_then(|entries| entries.get(&Self::key(path)).cloned())
    }
}

#[derive(Clone)]
pub struct PdfiumRenderer {
    library: PathBuf,
    limits: ResourceLimits,
    worker: Arc<OnceLock<Result<mpsc::SyncSender<Job>, String>>>,
    counters: Arc<Counters>,
    passwords: Arc<PasswordStore>,
}
enum Operation {
    Inspect,
    Outline,
    InspectOutput,
    Render(RenderRequest),
    RenderExact(ExactRenderRequest),
    Print(ExactRenderRequest),
    Text(TextRequest),
}
enum Output {
    Inspected(InspectedDocument),
    Rendered(RenderedPage),
    Text(TextPage),
    Outline(pdf_application::outline::Outline),
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
            passwords: Arc::new(PasswordStore::default()),
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
        self.passwords = Arc::new(PasswordStore::default());
        Ok(self)
    }
    /// Store an in-memory, process-local password for opening an encrypted PDF.
    /// Passwords are never persisted to disk and should be cleared on tab close.
    pub fn set_password(&self, path: &Path, password: String) {
        self.passwords.set(path, password);
    }
    /// Remove and wipe any in-memory password stored for `path`.
    pub fn clear_password(&self, path: &Path) {
        self.passwords.clear(path);
    }
    /// Return whether an in-memory password is currently registered for `path`.
    pub fn has_password(&self, path: &Path) -> bool {
        self.passwords.has(path)
    }
    pub fn outline(
        &self,
        path: &Path,
        fingerprint: SourceFingerprint,
    ) -> Result<pdf_application::outline::Outline, RenderError> {
        match self.request(path, Some(fingerprint), Operation::Outline)? {
            Output::Outline(outline) => Ok(outline),
            _ => Err(worker_error()),
        }
    }
    pub fn render_print(&self, request: &ExactRenderRequest) -> Result<RenderedPage, RenderError> {
        match self.request(
            &request.path,
            Some(request.fingerprint),
            Operation::Print(request.clone()),
        )? {
            Output::Rendered(page) => Ok(page),
            _ => Err(worker_error()),
        }
    }
    pub fn session_stats(&self) -> SessionStats {
        SessionStats {
            document_opens: self.counters.opens.load(Ordering::Relaxed),
            renders: self.counters.renders.load(Ordering::Relaxed),
            active_documents: self.counters.active_documents.load(Ordering::Relaxed),
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
                let passwords = self.passwords.clone();
                std::thread::Builder::new()
                    .name("pdfium-document".into())
                    .spawn(move || worker(receiver, library, limits, counters, passwords))
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
    passwords: Arc<PasswordStore>,
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
                    counters.active_documents.store(0, Ordering::Relaxed);
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
            let pw_entry = if matches!(job.operation, Operation::InspectOutput) {
                None
            } else {
                passwords.get(&job.path)
            };
            let pw_rev = pw_entry.as_ref().map_or(0, |(_, rev)| *rev);
            let key = (job.path.clone(), before, pw_rev);
            if session_key.as_ref() != Some(&key) || renders >= SESSION_RENDERS {
                session = None;
                session_key = None;
                renders = 0;
                let document = pdfium
                    .load_pdf_from_file(&job.path, pw_entry.as_ref().map(|(pw, _)| pw.as_str()))
                    .map_err(|err| match err {
                        PdfiumError::PdfiumLibraryInternalError(
                            PdfiumInternalError::PasswordError,
                        ) => RenderError::PasswordRequired,
                        _ => RenderError::InvalidPdf,
                    })?;
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
            drop(pw_entry);
            let document = session.as_ref().ok_or_else(worker_error)?;
            let result = match &job.operation {
                Operation::Outline => Output::Outline(read_outline(document)?),
                Operation::Inspect | Operation::InspectOutput => {
                    Output::Inspected(InspectedDocument {
                        annotations: read_document_annotations(document)?,
                        path: job.path.clone(),
                        fingerprint: before,
                        pages: document
                            .pages()
                            .iter()
                            .map(|page| geometry(&page))
                            .collect::<Result<Vec<_>, _>>()?,
                    })
                }
                Operation::Render(request) => {
                    renders += 1;
                    counters.renders.fetch_add(1, Ordering::Relaxed);
                    Output::Rendered(with_annotations(
                        document,
                        request.page_index,
                        request.annotations.as_deref(),
                        request.rotation_delta,
                        || render(document, request, limits),
                    )?)
                }
                Operation::RenderExact(request) | Operation::Print(request) => {
                    renders += 1;
                    counters.renders.fetch_add(1, Ordering::Relaxed);
                    Output::Rendered(with_annotations(
                        document,
                        request.page_index,
                        request.annotations.as_deref(),
                        request.rotation_delta,
                        || {
                            render_exact(
                                document,
                                request,
                                limits,
                                matches!(job.operation, Operation::Print(_)),
                            )
                        },
                    )?)
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
            if matches!(
                job.operation,
                Operation::InspectOutput | Operation::RenderExact(_) | Operation::Print(_)
            ) {
                session = None;
                session_key = None;
            }
            Ok(result)
        })();
        if result.is_err()
            && let Ok(_guard) = PDFIUM_LIBRARY.lock()
        {
            session = None;
            session_key = None;
        }
        counters
            .active_documents
            .store(u64::from(session.is_some()), Ordering::Relaxed);
        let _ = job.reply.send(result);
    }
    // Serialize native document destruction with all other adapter operations.
    if let Ok(_guard) = PDFIUM_LIBRARY.lock() {
        drop(session.take());
        counters.active_documents.store(0, Ordering::Relaxed);
    }
}
impl PdfInspector for PdfiumRenderer {
    fn inspect(&self, source: &Path) -> Result<InspectedDocument, RenderError> {
        match self.request(source, None, Operation::Inspect)? {
            Output::Inspected(info) => Ok(info),
            _ => Err(worker_error()),
        }
    }
    fn inspect_output(&self, source: &Path) -> Result<InspectedDocument, RenderError> {
        match self.request(source, None, Operation::InspectOutput)? {
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
impl ExactPageRenderer for PdfiumRenderer {
    fn render_exact(&self, request: &ExactRenderRequest) -> Result<RenderedPage, RenderError> {
        match self.request(
            &request.path,
            Some(request.fingerprint),
            Operation::RenderExact(request.clone()),
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

fn render_exact(
    document: &PdfDocument<'_>,
    request: &ExactRenderRequest,
    limits: ResourceLimits,
    printing: bool,
) -> Result<RenderedPage, RenderError> {
    if request.rotation_delta > 3
        || request.width == 0
        || request.height == 0
        || request.width > limits.max_bitmap_dimension
        || request.height > limits.max_bitmap_dimension
        || u64::from(request.width) * u64::from(request.height) > limits.max_bitmap_pixels
    {
        return Err(RenderError::ResourceLimit);
    }
    let page = document
        .pages()
        .get(i32::from(request.page_index))
        .map_err(|_| RenderError::InvalidPage)?;
    let geometry = geometry(&page)?;
    let (width, height) = if request.rotation_delta % 2 == 1 {
        (request.height, request.width)
    } else {
        (request.width, request.height)
    };
    let bitmap = page
        .render_with_config(
            &PdfRenderConfig::new()
                .set_fixed_size(width as i32, height as i32)
                .use_print_quality(printing)
                .set_clear_color(PdfColor::WHITE),
        )
        .map_err(|_| RenderError::InvalidPdf)?;
    if bitmap.width() != width as i32 || bitmap.height() != height as i32 {
        return Err(RenderError::ResourceLimit);
    }
    let mut rgba = bitmap.as_rgba_bytes();
    drop(bitmap);
    if rgba.len() != width as usize * height as usize * 4 {
        return Err(RenderError::InvalidPdf);
    }
    let (mut width, mut height) = (width, height);
    for _ in 0..request.rotation_delta {
        let mut rotated = vec![0; rgba.len()];
        for y in 0..height as usize {
            for x in 0..width as usize {
                let from = (y * width as usize + x) * 4;
                let to = (x * height as usize + height as usize - 1 - y) * 4;
                rotated[to..to + 4].copy_from_slice(&rgba[from..from + 4]);
            }
        }
        rgba = rotated;
        std::mem::swap(&mut width, &mut height);
    }
    Ok(RenderedPage {
        width,
        height,
        rgba,
        page_count: document.pages().len() as u16,
        clockwise_quarter_turns: (geometry.quarter_turns + request.rotation_delta) % 4,
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
    if request.rotation_delta > 3 {
        return Err(RenderError::InvalidPage);
    }
    let requested = f64::from(request.target_width.clamp(32, limits.max_bitmap_dimension))
        / if request.rotation_delta % 2 == 1 {
            aspect
        } else {
            1.
        };
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
    let mut rgba = bitmap.as_rgba_bytes();
    if rgba.len() != width as usize * height as usize * 4 {
        return Err(RenderError::ResourceLimit);
    }
    let (mut width, mut height) = (width, height);
    for _ in 0..request.rotation_delta {
        let mut rotated = vec![0; rgba.len()];
        for y in 0..height as usize {
            for x in 0..width as usize {
                let from = (y * width as usize + x) * 4;
                let to = (x * height as usize + height as usize - 1 - y) * 4;
                rotated[to..to + 4].copy_from_slice(&rgba[from..from + 4]);
            }
        }
        rgba = rotated;
        std::mem::swap(&mut width, &mut height);
    }
    Ok(RenderedPage {
        width,
        height,
        rgba,
        page_count: document.pages().len() as u16,
        clockwise_quarter_turns: (geometry.quarter_turns + request.rotation_delta) % 4,
    })
}

fn annotation_kind(annotation_type: PdfPageAnnotationType) -> Option<AnnotationKind> {
    match annotation_type {
        PdfPageAnnotationType::Ink => Some(AnnotationKind::Ink),
        PdfPageAnnotationType::Highlight => Some(AnnotationKind::Highlight),
        PdfPageAnnotationType::Underline => Some(AnnotationKind::Underline),
        PdfPageAnnotationType::Strikeout => Some(AnnotationKind::StrikeOut),
        PdfPageAnnotationType::Square => Some(AnnotationKind::Square),
        PdfPageAnnotationType::Circle => Some(AnnotationKind::Circle),
        PdfPageAnnotationType::Line => Some(AnnotationKind::Line),
        PdfPageAnnotationType::Text => Some(AnnotationKind::Text),
        PdfPageAnnotationType::Stamp => Some(AnnotationKind::Stamp),
        PdfPageAnnotationType::FreeText => Some(AnnotationKind::FreeText),
        _ => None,
    }
}

fn read_parsed_annotation(
    annotation_type: PdfPageAnnotationType,
    name: Option<String>,
    full_signatures_left: &mut usize,
    total_stamps_left: &mut usize,
) -> Result<Option<ParsedName>, RenderError> {
    let Some(kind) = annotation_kind(annotation_type) else {
        return Ok(None);
    };
    let Some(name) = name.filter(|name| name.starts_with(PREFIX)) else {
        return Ok(None);
    };
    let parsed = Annotation::parse_with_kind(&name, Some(kind));
    if matches!(
        &parsed,
        Some(ParsedName::SignatureRef { .. })
            | Some(ParsedName::Complete(Annotation {
                signature: Some(_),
                ..
            }))
    ) {
        *total_stamps_left = total_stamps_left
            .checked_sub(1)
            .ok_or(RenderError::ResourceLimit)?;
    }
    if matches!(&parsed, Some(ParsedName::Complete(a)) if a.signature.is_some()) {
        *full_signatures_left = full_signatures_left
            .checked_sub(1)
            .ok_or(RenderError::ResourceLimit)?;
    }
    Ok(parsed)
}

fn read_page_parsed(
    page: &PdfPage<'_>,
    full_signatures_left: &mut usize,
    total_stamps_left: &mut usize,
) -> Result<Vec<ParsedName>, RenderError> {
    let mut result = Vec::new();
    for a in page.annotations().iter() {
        if let Some(parsed) = read_parsed_annotation(
            a.annotation_type(),
            a.name(),
            full_signatures_left,
            total_stamps_left,
        )? {
            result.push(parsed);
        }
        if result.len() > MAX_PAGE_ANNOTATIONS {
            return Err(RenderError::ResourceLimit);
        }
    }
    Ok(result)
}

fn with_annotations(
    document: &PdfDocument<'_>,
    index: u16,
    annotations: Option<&[Annotation]>,
    rotation: u8,
    render: impl FnOnce() -> Result<RenderedPage, RenderError>,
) -> Result<RenderedPage, RenderError> {
    let Some(annotations) = annotations else {
        return render();
    };
    let page = document
        .pages()
        .get(i32::from(index))
        .map_err(|_| RenderError::InvalidPage)?;
    let mut hidden = Vec::new();
    let mut hide_error = None;
    let mut full_signatures_left = MAX_SIGNATURES;
    let mut total_stamps_left = MAX_SIGNATURE_PLACEMENTS;
    for mut a in page.annotations().iter() {
        if annotation_kind(a.annotation_type()).is_some() {
            match read_parsed_annotation(
                a.annotation_type(),
                a.name(),
                &mut full_signatures_left,
                &mut total_stamps_left,
            ) {
                Ok(Some(_)) => {
                    let previous = a.is_hidden();
                    let failed = a.set_is_hidden(true).is_err();
                    hidden.push((a, previous));
                    if failed {
                        hide_error = Some(RenderError::InvalidPdf);
                        break;
                    }
                }
                Ok(None) => {}
                Err(error) => {
                    hide_error = Some(error);
                    break;
                }
            }
        }
    }
    let result = if let Some(error) = hide_error {
        Err(error)
    } else {
        render()
    };
    let mut restore_failed = false;
    for (mut a, previous) in hidden {
        restore_failed |= a.set_is_hidden(previous).is_err();
    }
    if restore_failed {
        return Err(RenderError::InvalidPdf);
    }
    let mut result = result?;
    let g = geometry(&page)?;
    let width = if rotation % 2 == 1 {
        g.height_points
    } else {
        g.width_points
    };
    pdf_application::annotation::composite(
        &mut result.rgba,
        result.width,
        result.height,
        width,
        (g.quarter_turns + rotation) % 4,
        annotations,
    );
    Ok(result)
}

fn read_document_annotations(
    document: &PdfDocument<'_>,
) -> Result<Vec<Vec<Annotation>>, RenderError> {
    let mut parsed_pages = Vec::new();
    let mut full_signatures: std::collections::HashMap<u64, Arc<SignatureImage>> =
        std::collections::HashMap::new();
    let mut full_signatures_left = MAX_SIGNATURES;
    let mut total_stamps_left = MAX_SIGNATURE_PLACEMENTS;
    for page in document.pages().iter() {
        let parsed = read_page_parsed(&page, &mut full_signatures_left, &mut total_stamps_left)?;
        for entry in &parsed {
            if let ParsedName::Complete(a) = entry
                && let Some(image) = &a.signature
            {
                full_signatures.insert(a.id, Arc::clone(image));
            }
        }
        parsed_pages.push(parsed);
    }
    let mut result = Vec::with_capacity(parsed_pages.len());
    let mut points = 0usize;
    for page in parsed_pages {
        let mut annotations = Vec::with_capacity(page.len());
        for entry in page {
            let resolved = match entry {
                ParsedName::Complete(a) => Some(a),
                ParsedName::SignatureRef { ref_id, .. } => full_signatures
                    .get(&ref_id)
                    .cloned()
                    .and_then(|image| entry.resolve_signature(image)),
            };
            if let Some(a) = resolved {
                points += a.points.len();
                if points > 65536 {
                    return Err(RenderError::ResourceLimit);
                }
                annotations.push(a);
            }
        }
        result.push(annotations);
    }
    Ok(result)
}

fn read_outline(
    document: &PdfDocument<'_>,
) -> Result<pdf_application::outline::Outline, RenderError> {
    use pdf_application::outline::{Outline, OutlineNode};
    fn destination(
        document: &PdfDocument<'_>,
        dest: &PdfDestination<'_>,
    ) -> Option<(u16, Option<[f32; 2]>)> {
        let index = u16::try_from(dest.page_index().ok()?).ok()?;
        if i32::from(index) >= document.pages().len() {
            return None;
        }
        let page = document.pages().get(i32::from(index)).ok()?;
        let crop = page.boundaries().crop().ok().map(|b| b.bounds);
        let xy = match dest.view_settings().ok()? {
            PdfDestinationViewSettings::SpecificCoordinatesAndZoom(x, y, _) => y.map(|y| {
                (
                    x.or_else(|| crop.map(|c| c.left()))
                        .unwrap_or(PdfPoints::ZERO),
                    y,
                )
            }),
            PdfDestinationViewSettings::FitPageHorizontallyToWindow(y) => {
                y.map(|y| (crop.map(|c| c.left()).unwrap_or(PdfPoints::ZERO), y))
            }
            _ => None,
        };
        let point = xy
            .and_then(|(x, y)| {
                page.points_to_pixels(x, y, &PdfRenderConfig::new().set_fixed_size(16384, 16384))
                    .ok()
            })
            .map(|(x, y)| {
                [
                    (x as f32 / 16384.).clamp(0., 1.),
                    (y as f32 / 16384.).clamp(0., 1.),
                ]
            });
        Some((index, point))
    }
    let mut result = Outline::default();
    let mut stack = Vec::new();
    let mut seen = std::collections::HashSet::new();
    let mut text = 0;
    if let Some(root) = document.bookmarks().root() {
        for node in std::iter::once(root.clone())
            .chain(root.iter_siblings())
            .take(4097)
        {
            stack.push((node, None, 0usize));
        }
        stack.reverse();
    }
    while let Some((bookmark, parent, depth)) = stack.pop() {
        if result.nodes.len() >= 4096 || text >= 256 * 1024 || depth > 64 {
            result.limited = true;
            break;
        }
        if !seen.insert(bookmark.clone()) {
            result.limited = true;
            continue;
        }
        let title = bookmark.title().unwrap_or_default();
        if title.chars().count() > 256 {
            result.limited = true;
        }
        let title = if title.trim().is_empty() {
            "Tanpa judul".into()
        } else {
            title.chars().take(256).collect::<String>()
        };
        text += title.len();
        let action = bookmark.action();
        let blocked = action
            .as_ref()
            .is_some_and(|a| a.action_type() != PdfActionType::GoToDestinationInSameDocument);
        let target = if blocked {
            None
        } else if let Some(dest) = bookmark.destination() {
            destination(document, &dest)
        } else {
            action
                .as_ref()
                .and_then(|a| a.as_local_destination_action())
                .and_then(|a| a.destination().ok().and_then(|d| destination(document, &d)))
        };
        let own = result.nodes.len();
        result.nodes.push(OutlineNode {
            title,
            parent,
            page: target.map(|p| p.0),
            point: target.and_then(|p| p.1),
            blocked,
        });
        let children = bookmark
            .iter_direct_children()
            .take(4097)
            .collect::<Vec<_>>();
        for node in children.into_iter().rev() {
            stack.push((node, Some(own), depth + 1));
        }
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn minimal_pdf() -> Vec<u8> {
        let mut pdf = Vec::from(b"%PDF-1.4\n".as_slice());
        let mut offsets = vec![0usize];
        let objects = [
            "1 0 obj\n<< /Type /Catalog /Pages 2 0 R >>\nendobj\n",
            "2 0 obj\n<< /Type /Pages /Kids [3 0 R] /Count 1 >>\nendobj\n",
            "3 0 obj\n<< /Type /Page /Parent 2 0 R /MediaBox [0 0 200 200] >>\nendobj\n",
        ];
        for obj in objects {
            offsets.push(pdf.len());
            pdf.extend_from_slice(obj.as_bytes());
        }
        let xref = pdf.len();
        pdf.extend_from_slice(b"xref\n0 4\n0000000000 65535 f \n");
        for off in &offsets[1..] {
            pdf.extend_from_slice(format!("{off:010} 00000 n \n").as_bytes());
        }
        pdf.extend_from_slice(
            format!("trailer\n<< /Size 4 /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n").as_bytes(),
        );
        pdf
    }

    #[test]
    fn password_store_normalizes_and_clears() {
        let store = PasswordStore::default();
        let a = Path::new("D:/Tmp/Secret.PDF");
        let b = Path::new("d:\\tmp\\secret.pdf");
        store.set(a, "pw-123".into());
        #[cfg(windows)]
        assert!(store.has(b));
        assert!(store.has(a));
        store.clear(b);
        assert!(!store.has(a));
    }

    #[test]
    fn opens_encrypted_pdf_only_with_correct_password() -> Result<(), Box<dyn std::error::Error>> {
        let sidecars = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../sidecars");
        let qpdf = sidecars.join("qpdf/bin/qpdf.exe");
        if !qpdf.is_file() || !sidecars.join("pdfium/bin/pdfium.dll").is_file() {
            return Ok(());
        }
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let dir = std::env::temp_dir().join(format!("origo-pdfium-pw-{nonce}"));
        std::fs::create_dir_all(&dir)?;
        let plain = dir.join("plain.pdf");
        let locked = dir.join("locked.pdf");
        std::fs::write(&plain, minimal_pdf())?;

        let status = Command::new(&qpdf)
            .args(["--encrypt", "rahasia99", "owner99", "256", "--"])
            .arg(&plain)
            .arg(&locked)
            .status()?;
        assert!(status.success());

        let renderer = PdfiumRenderer::new(&sidecars);
        assert!(matches!(
            renderer.inspect(&locked),
            Err(RenderError::PasswordRequired)
        ));

        renderer.set_password(&locked, "salah".into());
        assert!(matches!(
            renderer.inspect(&locked),
            Err(RenderError::PasswordRequired)
        ));

        renderer.set_password(&locked, "rahasia99".into());
        assert!(renderer.has_password(&locked));
        let inspected = renderer.inspect(&locked)?;
        assert_eq!(inspected.pages.len(), 1);
        let rendered = renderer.render(&locked, 0)?;
        assert!(rendered.width > 0 && rendered.height > 0);
        let text = renderer.extract_text(&TextRequest {
            path: locked.clone(),
            page_index: 0,
            fingerprint: inspected.fingerprint,
        })?;
        assert!(text.chars.is_empty());

        renderer.clear_password(&locked);
        assert!(!renderer.has_password(&locked));
        assert!(matches!(
            renderer.inspect(&locked),
            Err(RenderError::PasswordRequired)
        ));

        let _ = std::fs::remove_dir_all(&dir);
        Ok(())
    }
}
