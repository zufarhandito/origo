use crate::*;
use gpui::component::{Disableable, Sizable, button::Button};
use pdf_application::export::ExportControl;
use pdf_application::text::TextRequest;
use pdf_ocr::{Cached, DocumentPage, Language, PageKey, Recognition, Store, Tesseract};
use std::{collections::VecDeque, path::Path, time::Duration};

const MANUAL_QUEUE_LIMIT: usize = 16;
const OCR_TIMEOUT: Duration = Duration::from_secs(60);

struct Job {
    tab: shell::TabId,
    key: PageKey,
    page: DocumentPage,
}

struct Running {
    job: Job,
    control: ExportControl,
}

pub(super) enum Change {
    Page(PageKey, Recognition),
    Cleared,
    Notice(String),
}

pub(super) struct OcrJobs {
    renderer: PdfiumRenderer,
    engine: Tesseract,
    pub store: Store,
    queue: VecDeque<Job>,
    running: Option<Running>,
    failures: VecDeque<(PageKey, String)>,
    clearing: bool,
}

impl gpui::EventEmitter<Change> for OcrJobs {}

impl OcrJobs {
    pub fn new(sidecars: &Path, state: Option<&Path>, renderer: PdfiumRenderer) -> Self {
        Self {
            renderer,
            engine: Tesseract::new(sidecars),
            store: Store::new(state),
            queue: VecDeque::new(),
            running: None,
            failures: VecDeque::new(),
            clearing: false,
        }
    }

    pub fn can_clear(&self) -> bool {
        self.running.is_none() && self.queue.is_empty() && !self.clearing
    }

    pub fn clear(&mut self, cx: &mut Context<Self>) {
        if !self.can_clear() {
            return;
        }
        self.clearing = true;
        let store = self.store.clone();
        let work = cx.background_executor().spawn(async move { store.clear() });
        cx.spawn(async move |this, cx| {
            let result = work.await;
            let _ = this.update(cx, |this, cx| {
                this.clearing = false;
                match result {
                    Ok(()) => {
                        this.failures.clear();
                        cx.emit(Change::Cleared);
                        cx.emit(Change::Notice("Hasil OCR lokal dihapus.".into()));
                    }
                    Err(error) => cx.emit(Change::Notice(error.to_string())),
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn queued(&self, key: &PageKey) -> bool {
        self.queue.iter().any(|job| job.key == *key)
    }

    fn active(&self, key: &PageKey) -> Option<&Running> {
        self.running.as_ref().filter(|work| work.job.key == *key)
    }

    fn request(&mut self, tab: shell::TabId, page: DocumentPage, cx: &mut Context<Self>) {
        let key = page.key();
        if self.clearing || self.active(&key).is_some() || self.queued(&key) {
            return;
        }
        self.failures.retain(|(failed, _)| *failed != key);
        if self.queue.len() >= MANUAL_QUEUE_LIMIT {
            self.remember_failure(
                key,
                "Antrean OCR penuh. Coba lagi setelah halaman lain selesai.".into(),
            );
            cx.notify();
            return;
        }
        self.queue.push_back(Job { tab, key, page });
        self.pump(cx);
        cx.notify();
    }

    fn cancel_page(&mut self, key: &PageKey, cx: &mut Context<Self>) {
        self.queue.retain(|job| job.key != *key);
        if let Some(work) = self.active(key) {
            work.control.cancel();
        }
        cx.notify();
    }

    pub fn stop_tabs(&mut self, tabs: &[shell::TabId]) -> bool {
        self.queue.retain(|job| !tabs.contains(&job.tab));
        if let Some(work) = &self.running
            && tabs.contains(&work.job.tab)
        {
            work.control.cancel();
            return true;
        }
        self.clearing
    }

    fn remember_failure(&mut self, key: PageKey, message: String) {
        if self.failures.len() >= 32 {
            self.failures.pop_front();
        }
        self.failures.push_back((key, message));
    }

    fn pump(&mut self, cx: &mut Context<Self>) {
        if self.running.is_some() || self.clearing {
            return;
        }
        let Some(job) = self.queue.pop_front() else {
            return;
        };
        let control = ExportControl::with_budget(OCR_TIMEOUT);
        let page = job.page.clone();
        let worker_control = control.clone();
        let renderer = self.renderer.clone();
        let engine = self.engine.clone();
        let store = self.store.clone();
        let task = cx.background_executor().spawn(async move {
            page.recognize(&renderer, &engine, &store, &worker_control, true)
        });
        self.running = Some(Running { job, control });
        cx.spawn(async move |this, cx| {
            let result = task.await;
            let _ = this.update(cx, |this, cx| {
                let Some(work) = this.running.take() else {
                    return;
                };
                match result {
                    Ok(Cached::Text(text)) => cx.emit(Change::Page(work.job.key, text)),
                    Ok(_) => {}
                    Err(error) if work.control.checkpoint().is_ok() => {
                        let message = error.to_string();
                        this.remember_failure(work.job.key, message.clone());
                        cx.emit(Change::Notice(message));
                    }
                    Err(_) => {}
                }
                this.pump(cx);
                cx.notify();
            });
        })
        .detach();
    }
}

impl DocumentSession {
    pub(super) fn ocr_page(&self, index: usize) -> Option<DocumentPage> {
        let workspace = self.workspace.as_ref()?;
        let page = workspace.pages().get(index)?;
        let source = workspace.resolve(page.source_id)?;
        Some(DocumentPage {
            request: TextRequest {
                path: source.path.clone(),
                page_index: page.source_page_index,
                fingerprint: source.fingerprint,
            },
            rotation: page.rotation_delta,
            geometry: workspace.geometry(index).ok()?,
            language: Language::default(),
        })
    }

    pub(super) fn ocr_changed(&mut self, change: &Change, cx: &mut Context<Self>) {
        match change {
            Change::Page(key, recognition) => {
                let matching: Vec<_> = (0..usize::from(self.navigation.count))
                    .filter(|&index| self.ocr_page(index).is_some_and(|page| page.key() == *key))
                    .collect();
                if matching.is_empty() {
                    return;
                }
                self.ocr_revision = self.ocr_revision.wrapping_add(1);
                for index in matching {
                    self.text.pages.remove(&index);
                    self.text.errors.remove(&index);
                    let source_text = recognition.text_page();
                    self.update_ocr_matches(index, &source_text, cx);
                }
            }
            Change::Cleared => {
                self.ocr_revision = self.ocr_revision.wrapping_add(1);
                self.text.pages.clear();
                self.text.ocr_pages.clear();
                self.text.errors.clear();
                self.clear_selection(cx);
                self.restart_search(cx);
            }
            Change::Notice(_) => return,
        }
        if self.visible_for_render {
            self.request_text(usize::from(self.navigation.current), cx);
        }
        self.report(cx);
        cx.notify();
    }

    pub(super) fn ocr_chip(&mut self, cx: &mut Context<Self>) -> Option<gpui::AnyElement> {
        if self.mode != ViewerMode::Read {
            return None;
        }
        let index = usize::from(self.navigation.current);
        let page = self.ocr_page(index)?;
        let text = self.text.pages.get(&index)?;
        let is_ocr = self.text.ocr_pages.contains(&index);
        let key = page.key();
        let jobs = self.ocr.read(cx);
        let locked = self.edits_locked() || jobs.clearing;
        let chip = Button::new("ocr-page").outline().xsmall().rounded_full();
        let button = if let Some(work) = jobs.active(&key) {
            chip.label("OCR berjalan · Batal")
                .disabled(locked || work.control.checkpoint().is_err())
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.ocr.update(cx, |jobs, cx| jobs.cancel_page(&key, cx));
                }))
        } else if jobs.queued(&key) {
            chip.label("OCR mengantre · Batal")
                .disabled(locked)
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.ocr.update(cx, |jobs, cx| jobs.cancel_page(&key, cx));
                }))
        } else if pdf_ocr::has_text(&text) {
            return is_ocr.then(|| gpui::div().child("Teks OCR").into_any_element());
        } else {
            let (label, detail) =
                if let Some((_, error)) = jobs.failures.iter().find(|(failed, _)| *failed == key) {
                    ("Coba OCR lagi", error.clone())
                } else if is_ocr {
                    ("OCR tidak menemukan teks", "Klik untuk mencoba lagi".into())
                } else {
                    ("Butuh OCR", "Kenali teks halaman ini secara lokal".into())
                };
            chip.label(label)
                .tooltip(detail)
                .disabled(locked)
                .on_click(cx.listener(move |this, _, _, cx| {
                    let page = page.clone();
                    let tab = this.tab_id;
                    this.ocr.update(cx, |jobs, cx| jobs.request(tab, page, cx));
                }))
        };
        Some(button.into_any_element())
    }
}
