//! Window ownership, stable document identities and transactional close sequencing.
use crate::*;
use gpui::assets::IconName;
use gpui::component::{
    ActiveTheme, Disableable, Selectable, Theme, ThemeMode, TitleBar, WindowExt,
    button::{Button, ButtonCustomVariant, ButtonVariants},
    menu::{ContextMenuExt, DropdownMenu, PopupMenuItem},
    tab::Tab as DocumentTab,
};
use gpui::{AnyElement, Render, div};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

const MAX_TABS: usize = 8;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct TabId(pub u64);
#[derive(Default)]
pub(super) struct WindowResources {
    pub export_owner: AtomicU64,
    pub modal_owner: AtomicU64,
    pub closing: AtomicBool,
    pub tab_count: std::sync::atomic::AtomicUsize,
    pub metrics_sequence: Arc<AtomicU64>,
    pub metrics_lock: Arc<std::sync::Mutex<()>>,
    pub gpu_releases: Arc<AtomicU64>,
    pub gpu_release_errors: Arc<AtomicU64>,
}
struct Tab {
    id: TabId,
    path: PathBuf,
    session: Option<Entity<DocumentSession>>,
}
struct CloseBatch {
    ids: Vec<TabId>,
    index: usize,
    waiting: bool,
    window: bool,
    previous: Option<TabId>,
    external_export: Option<(TabId, u64)>,
}
pub(super) struct WindowShell {
    tabs: Vec<Tab>,
    active: Option<TabId>,
    next_id: u64,
    sidecars: PathBuf,
    renderer: PdfiumRenderer,
    resources: Arc<WindowResources>,
    page_budget: usize,
    thumbnail_budget: usize,
    metrics: Option<PathBuf>,
    focus: FocusHandle,
    tab_scroll: ScrollHandle,
    handle: gpui::AnyWindowHandle,
    opening: bool,
    close: Option<CloseBatch>,
    notice: Option<String>,
    favorites: std::collections::BTreeSet<String>,
    favorites_ready: bool,
    favorites_revision: u64,
    favorites_writer: Arc<std::sync::Mutex<u64>>,
    pending_preferences: usize,
    state_dir: Option<PathBuf>,
}

fn path_key(path: &std::path::Path) -> String {
    path.to_string_lossy().replace('/', "\\").to_lowercase()
}
fn capacity(count: usize, added: usize) -> bool {
    added <= MAX_TABS.saturating_sub(count)
}
impl WindowShell {
    pub fn new(window: &mut Window, cx: &mut App, options: ShellOptions) -> Entity<Self> {
        let ShellOptions {
            sidecars,
            page_budget,
            thumbnail_budget,
            metrics,
            state_dir,
            initial,
        } = options;
        cx.new(|cx| {
            let weak = cx.weak_entity();
            window.on_window_should_close(cx, move |_, cx| {
                weak.update(cx, |this: &mut Self, cx| this.begin_close(true, cx))
                    .unwrap_or(true)
            });
            let mut shell = Self {
                tabs: vec![],
                active: None,
                next_id: 1,
                renderer: PdfiumRenderer::new(&sidecars),
                sidecars,
                resources: Arc::default(),
                page_budget,
                thumbnail_budget,
                metrics,
                focus: cx.focus_handle(),
                tab_scroll: ScrollHandle::new(),
                handle: window.window_handle(),
                opening: false,
                close: None,
                notice: None,
                favorites: Default::default(),
                favorites_ready: false,
                favorites_revision: 0,
                favorites_writer: Arc::default(),
                pending_preferences: 0,
                state_dir,
            };
            shell.load_favorites(cx);
            shell.focus.focus(window, cx);
            if shell.metrics.is_some() {
                cx.spawn(async move |this, cx| {
                    loop {
                        cx.background_executor()
                            .timer(std::time::Duration::from_millis(500))
                            .await;
                        if this.update(cx, |this, cx| this.report(cx)).is_err() {
                            break;
                        }
                    }
                })
                .detach();
            }
            if let Some(path) = initial {
                shell.open_paths(vec![path], cx);
            }
            shell
        })
    }
    fn session(&self) -> Option<Entity<DocumentSession>> {
        self.tabs
            .iter()
            .find(|t| Some(t.id) == self.active)
            .and_then(|t| t.session.clone())
    }
    pub(super) fn report(&self, cx: &mut Context<Self>) {
        let Some(path) = self.metrics.clone() else {
            return;
        };
        let sessions: Vec<_> = self
            .tabs
            .iter()
            .filter_map(|t| t.session.as_ref())
            .map(|s| s.read(cx).diagnostics())
            .collect();
        let mut json = self
            .session()
            .map(|s| s.read(cx).diagnostics())
            .unwrap_or_else(|| {
                serde_json::json!({
                    "pageCount": 0, "activePage": 0, "tabId": 0, "queue": 0, "rendering": false,
                    "opening": false, "textLoading": false, "exporting": false, "dirty": false,
                    "pageCacheBytes": 0, "thumbnailCacheBytes": 0, "textCacheBytes": 0,
                    "selectedChars": 0, "copyingText": false, "transition": "Idle",
                })
            });
        let sum = |key: &str| sessions.iter().filter_map(|s| s[key].as_u64()).sum::<u64>();
        json["tabCount"] = serde_json::json!(self.tabs.len());
        json["tabOrder"] = serde_json::json!(self.tabs.iter().map(|t| t.id.0).collect::<Vec<_>>());
        json["starredTabIds"] = serde_json::json!(
            self.tabs
                .iter()
                .filter(|t| self.favorites.contains(&path_key(&t.path)))
                .map(|t| t.id.0)
                .collect::<Vec<_>>()
        );
        json["favoritesReady"] = serde_json::json!(self.favorites_ready);
        json["pendingPreferences"] = serde_json::json!(self.pending_preferences);
        json["tabsOpening"] = serde_json::json!(self.opening);
        json["closingTabs"] = serde_json::json!(self.close.is_some());
        json["windowExportOwner"] =
            serde_json::json!(self.resources.export_owner.load(Ordering::SeqCst));
        json["totalPageCacheBytes"] = serde_json::json!(sum("pageCacheBytes"));
        json["totalThumbnailCacheBytes"] = serde_json::json!(sum("thumbnailCacheBytes"));
        json["totalTextCacheBytes"] = serde_json::json!(sum("textCacheBytes"));
        json["totalCopyBufferBytes"] = serde_json::json!(sum("copyBufferBytes"));
        json["totalGpuImages"] = serde_json::json!(sum("gpuImages"));
        json["windowGpuReleases"] =
            serde_json::json!(self.resources.gpu_releases.load(Ordering::SeqCst));
        json["windowGpuReleaseErrors"] =
            serde_json::json!(self.resources.gpu_release_errors.load(Ordering::SeqCst));
        json["inactiveCacheBytes"] = serde_json::json!(
            sessions
                .iter()
                .filter(|s| s["active"] == false)
                .map(|s| {
                    [
                        "pageCacheBytes",
                        "thumbnailCacheBytes",
                        "textCacheBytes",
                        "copyBufferBytes",
                    ]
                    .iter()
                    .filter_map(|key| s[*key].as_u64())
                    .sum::<u64>()
                })
                .sum::<u64>()
        );
        json["inactiveGpuImages"] = serde_json::json!(
            sessions
                .iter()
                .filter(|s| s["active"] == false)
                .filter_map(|s| s["gpuImages"].as_u64())
                .sum::<u64>()
        );
        json["nativeActiveDocuments"] =
            serde_json::json!(self.renderer.session_stats().active_documents);
        json["sessions"] = serde_json::json!(sessions);
        let sequence = self.resources.metrics_sequence.clone();
        let number = sequence.fetch_add(1, Ordering::SeqCst) + 1;
        let lock = self.resources.metrics_lock.clone();
        cx.background_executor()
            .spawn(async move {
                if let Ok(_guard) = lock.lock()
                    && sequence.load(Ordering::SeqCst) == number
                {
                    let _ = std::fs::write(path, json.to_string());
                }
            })
            .detach();
    }
    fn modal(&self, cx: &App) -> bool {
        self.tabs
            .iter()
            .filter_map(|t| t.session.as_ref())
            .any(|s| {
                let s = s.read(cx);
                s.error_popup || s.split_dialog.is_some() || s.transition.active()
            })
    }
    pub(super) fn choose_open(&mut self, cx: &mut Context<Self>) {
        if self.opening || self.close.is_some() || self.modal(cx) {
            return;
        }
        self.opening = true;
        let picker = cx.prompt_for_paths(gpui::PathPromptOptions {
            files: true,
            directories: false,
            multiple: true,
            prompt: Some("Pilih PDF (maksimal 8 tab)".into()),
        });
        cx.notify();
        cx.spawn(async move |this, cx| {
            let result = picker.await;
            let _ = this.update(cx, |this, cx| {
                this.opening = false;
                match result {
                    Ok(Ok(Some(paths))) => this.open_paths(paths, cx),
                    Ok(Ok(None)) => cx.notify(),
                    _ => {
                        this.notice = Some("Dialog file gagal dibuka.".into());
                        cx.notify();
                    }
                }
            });
        })
        .detach();
    }
    fn open_paths(&mut self, paths: Vec<PathBuf>, cx: &mut Context<Self>) {
        if self.opening || self.close.is_some() {
            return;
        }
        self.opening = true;
        let task = cx.background_executor().spawn(async move {
            paths
                .into_iter()
                .map(|p| dunce::canonicalize(&p).map_err(|e| format!("{}: {e}", p.display())))
                .collect::<Vec<_>>()
        });
        cx.spawn(async move |this, cx| {
            let paths = task.await;
            let _ = this.update(cx, |this, cx| this.reserve_paths(paths, cx));
        })
        .detach();
        cx.notify();
    }
    fn reserve_paths(&mut self, results: Vec<Result<PathBuf, String>>, cx: &mut Context<Self>) {
        // Source-change invalidation requires reopening, rather than focusing an
        // unusable old session forever. Only discard an already-invalidated tab.
        let reopening: Vec<_> = results
            .iter()
            .filter_map(|r| r.as_ref().ok())
            .map(|p| path_key(p))
            .collect();
        self.tabs.retain(|tab| {
            !(reopening.contains(&path_key(&tab.path))
                && tab.session.as_ref().is_some_and(|s| {
                    let s = s.read(cx);
                    s.workspace.is_none() && !s.export.running && !s.opening
                }))
        });
        if !self.tabs.iter().any(|t| Some(t.id) == self.active) {
            self.active = None;
        }
        let mut failures = vec![];
        let mut paths = vec![];
        let mut first_existing = None;
        for result in results {
            match result {
                Err(e) => failures.push(e),
                Ok(path) => {
                    if let Some(tab) = self
                        .tabs
                        .iter()
                        .find(|t| path_key(&t.path) == path_key(&path))
                    {
                        first_existing.get_or_insert(tab.id);
                    } else if !paths
                        .iter()
                        .any(|p: &PathBuf| path_key(p) == path_key(&path))
                    {
                        paths.push(path);
                    }
                }
            }
        }
        if !capacity(self.tabs.len(), paths.len()) {
            self.opening = false;
            self.notice = Some(format!(
                "Maksimal 8 tab. Tersisa {} slot; tutup tab atau pilih lebih sedikit PDF.",
                MAX_TABS - self.tabs.len()
            ));
            cx.notify();
            return;
        }
        if paths.is_empty() {
            self.opening = false;
            if let Some(id) = first_existing {
                self.activate(id, cx);
            }
            self.notice = (!failures.is_empty()).then(|| failures.join("\n"));
            cx.notify();
            return;
        }
        let mut jobs = vec![];
        for path in paths {
            let id = TabId(self.next_id);
            self.next_id += 1;
            self.tabs.push(Tab {
                id,
                path: path.clone(),
                session: None,
            });
            jobs.push((id, path));
        }
        let renderer = self.renderer.clone();
        self.resources
            .tab_count
            .store(self.tabs.len(), Ordering::SeqCst);
        let task = cx.background_executor().spawn(async move {
            jobs.into_iter()
                .map(|(id, path)| {
                    let result = pdf_application::open_document(
                        &renderer,
                        &path,
                        pdf_domain::SourceId(id.0 << 32),
                    )
                    .and_then(|w| {
                        let actual = pdf_platform::fingerprint(&path)
                            .map_err(|_| pdf_application::RenderError::SourceChanged)?;
                        if actual != w.source().fingerprint {
                            return Err(pdf_application::RenderError::SourceChanged);
                        }
                        Ok(w)
                    })
                    .map_err(|e| format!("{}: {e}", path.display()));
                    (id, result)
                })
                .collect::<Vec<_>>()
        });
        let handle = self.handle;
        cx.spawn(async move |this, cx| {
            let results = task.await;
            let _ = handle.update(cx, |_, window, cx| {
                let _ = this.update(cx, |this, cx| {
                    let mut first = None;
                    for (id, result) in results {
                        match result {
                            Ok(workspace) => {
                                let owner = cx.weak_entity();
                                let session = DocumentSession::new(
                                    window,
                                    cx,
                                    &this.sidecars,
                                    this.renderer.clone(),
                                    this.page_budget,
                                    this.thumbnail_budget,
                                    this.metrics.clone(),
                                    owner,
                                    this.resources.clone(),
                                    id,
                                );
                                session.update(cx, |s, cx| s.install_workspace(workspace, cx));
                                cx.observe(&session, |_, _, cx| {
                                    let weak = cx.weak_entity();
                                    cx.defer(move |cx| {
                                        let _ = weak.update(cx, |this, cx| {
                                            this.advance_close(cx);
                                            cx.notify();
                                        });
                                    });
                                })
                                .detach();
                                if let Some(tab) = this.tabs.iter_mut().find(|t| t.id == id) {
                                    tab.session = Some(session);
                                }
                                first.get_or_insert(id);
                            }
                            Err(e) => {
                                failures.push(e);
                                this.tabs.retain(|t| t.id != id);
                            }
                        }
                    }
                    this.opening = false;
                    this.resources
                        .tab_count
                        .store(this.tabs.len(), Ordering::SeqCst);
                    this.notice = (!failures.is_empty())
                        .then(|| format!("PDF yang gagal dibuka:\n{}", failures.join("\n")));
                    if let Some(id) = first.or(first_existing) {
                        this.activate(id, cx);
                    }
                    cx.notify();
                });
            });
        })
        .detach();
        cx.notify();
    }
    fn activate(&mut self, id: TabId, cx: &mut Context<Self>) {
        if !self.tabs.iter().any(|t| t.id == id && t.session.is_some()) {
            return;
        }
        if self.active != Some(id) {
            if let Some(session) = self.session() {
                session.update(cx, |s, cx| {
                    s.active = false;
                    s.search.pause();
                    s.invalidate_work();
                    s.clear_bitmaps(cx);
                    s.text.pages.clear();
                    s.text.dragging = false;
                    s.text.pending = None;
                    s.text.cancel_copy();
                    s.page_dragging = false;
                });
            }
            self.active = Some(id);
            if let Some(session) = self.session() {
                session.update(cx, |s, cx| {
                    s.active = true;
                    if s.search.running {
                        s.restart_search(cx);
                    }
                    s.schedule_current(cx);
                    s.report(cx);
                    cx.notify();
                });
            }
        }
        if let Some(session) = self.session() {
            let focus = session.read(cx).focus.clone();
            let handle = self.handle;
            cx.defer(move |cx| {
                let _ = handle.update(cx, |_, window, cx| focus.focus(window, cx));
            });
        }
        if let Some(index) = self.tabs.iter().position(|t| t.id == id) {
            self.tab_scroll.scroll_to_item(index);
        }
        cx.notify();
    }
    fn switch(&mut self, id: TabId, cx: &mut Context<Self>) {
        if self.close.is_none() && !self.modal(cx) {
            self.activate(id, cx);
        }
    }
    fn cycle(&mut self, backwards: bool, cx: &mut Context<Self>) {
        let ids: Vec<_> = self
            .tabs
            .iter()
            .filter(|t| t.session.is_some())
            .map(|t| t.id)
            .collect();
        if ids.is_empty() {
            return;
        }
        let index = ids
            .iter()
            .position(|id| Some(*id) == self.active)
            .unwrap_or(0);
        let next = if backwards {
            (index + ids.len() - 1) % ids.len()
        } else {
            (index + 1) % ids.len()
        };
        self.switch(ids[next], cx);
    }
    fn begin_close(&mut self, window: bool, cx: &mut Context<Self>) -> bool {
        self.begin_close_target(window, None, cx)
    }
    fn begin_close_target(
        &mut self,
        window: bool,
        target: Option<TabId>,
        cx: &mut Context<Self>,
    ) -> bool {
        let ids = if window {
            self.tabs.iter().map(|t| t.id).collect()
        } else {
            target.or(self.active).into_iter().collect()
        };
        self.begin_close_ids(window, ids, cx)
    }
    fn begin_close_ids(&mut self, window: bool, ids: Vec<TabId>, cx: &mut Context<Self>) -> bool {
        if self.close.is_some()
            || self.opening
            || self.modal(cx)
            || self
                .tabs
                .iter()
                .filter_map(|t| t.session.as_ref())
                .any(|s| s.read(cx).opening)
        {
            return false;
        }
        if self.tabs.is_empty() {
            return window;
        }
        let ids: Vec<_> = ids
            .into_iter()
            .filter(|id| self.tabs.iter().any(|t| t.id == *id))
            .collect();
        if ids.is_empty() {
            return false;
        }
        self.resources.closing.store(true, Ordering::SeqCst);
        self.close = Some(CloseBatch {
            ids,
            index: 0,
            waiting: false,
            window,
            previous: self.active,
            external_export: None,
        });
        self.advance_close(cx);
        false
    }
    fn cancel_close(&mut self, cx: &mut Context<Self>) {
        let previous = self.close.take().and_then(|b| b.previous);
        self.resources.closing.store(false, Ordering::SeqCst);
        let mut prompt = false;
        for tab in &self.tabs {
            if let Some(s) = &tab.session {
                s.update(cx, |s, cx| {
                    if s.transition.active() {
                        if s.transition.phase == transitions::Phase::Prompt {
                            prompt = true;
                            s.clear_modal();
                        }
                        s.cancel_transition(cx);
                    }
                    s.close_approved = false;
                    cx.notify();
                });
            }
        }
        if prompt {
            let handle = self.handle;
            cx.defer(move |cx| {
                let _ = handle.update(cx, |_, w, cx| w.close_dialog(cx));
            });
        }
        if let Some(id) = previous {
            self.activate(id, cx);
        }
        cx.notify();
    }
    fn advance_close(&mut self, cx: &mut Context<Self>) {
        loop {
            let Some(batch) = &self.close else {
                return;
            };
            if batch.index == batch.ids.len() {
                if self.pending_preferences != 0 {
                    return;
                }
                let Some(batch) = self.close.take() else {
                    return;
                };
                for tab in &self.tabs {
                    if batch.ids.contains(&tab.id)
                        && let Some(s) = &tab.session
                    {
                        s.update(cx, |s, cx| {
                            s.active = false;
                            s.invalidate_work();
                            s.clear_bitmaps(cx);
                        });
                    }
                }
                self.tabs.retain(|t| !batch.ids.contains(&t.id));
                self.resources
                    .tab_count
                    .store(self.tabs.len(), Ordering::SeqCst);
                self.active = self
                    .active
                    .filter(|id| self.tabs.iter().any(|t| t.id == *id));
                self.resources.closing.store(false, Ordering::SeqCst);
                if batch.window {
                    let handle = self.handle;
                    cx.defer(move |cx| {
                        let _ = handle.update(cx, |_, w, _| w.remove_window());
                    });
                } else if let Some(id) = self
                    .active
                    .or(batch
                        .previous
                        .filter(|id| self.tabs.iter().any(|t| t.id == *id)))
                    .or_else(|| self.tabs.first().map(|t| t.id))
                {
                    self.activate(id, cx);
                }
                cx.notify();
                return;
            }
            let id = batch.ids[batch.index];
            let waiting = batch.waiting;
            if let Some((owner, completed)) = batch.external_export {
                if let Some(s) = self
                    .tabs
                    .iter()
                    .find(|t| t.id == owner)
                    .and_then(|t| t.session.as_ref())
                {
                    let s = s.read(cx);
                    if s.export.running {
                        return;
                    }
                    if s.export.completed <= completed || !s.export.success {
                        self.cancel_close(cx);
                        return;
                    }
                }
                if let Some(b) = &mut self.close {
                    b.external_export = None;
                }
            }
            let Some(session) = self
                .tabs
                .iter()
                .find(|t| t.id == id)
                .and_then(|t| t.session.clone())
            else {
                self.cancel_close(cx);
                return;
            };
            if waiting {
                let s = session.read(cx);
                if s.close_approved {
                    if let Some(b) = &mut self.close {
                        b.index += 1;
                        b.waiting = false;
                    }
                    continue;
                }
                if !s.transition.active() {
                    self.cancel_close(cx);
                }
                return;
            }
            let s = session.read(cx);
            let owner = self.resources.export_owner.load(Ordering::SeqCst);
            if owner != 0 && owner != id.0 && s.workspace.as_ref().is_some_and(Workspace::is_dirty)
            {
                let completed = self
                    .tabs
                    .iter()
                    .find(|t| t.id.0 == owner)
                    .and_then(|t| t.session.as_ref())
                    .map(|s| s.read(cx).export.completed)
                    .unwrap_or(0);
                if let Some(b) = &mut self.close {
                    b.external_export = Some((TabId(owner), completed));
                }
                self.notice = Some("Menunggu slot ekspor sebelum menyimpan tab. Gunakan Tetap di aplikasi untuk membatalkan penutupan.".into());
                cx.notify();
                return;
            }
            if self.resources.modal_owner.load(Ordering::SeqCst) != 0 {
                return;
            }
            if s.export.running || s.workspace.as_ref().is_some_and(Workspace::is_dirty) {
                self.activate(id, cx);
            }
            let ready = session.update(cx, |s, cx| {
                s.close_approved = false;
                s.request_close(cx)
            });
            if let Some(b) = &mut self.close {
                if ready {
                    b.index += 1;
                } else {
                    b.waiting = true;
                }
            }
        }
    }
}

pub(super) struct ShellOptions {
    pub sidecars: PathBuf,
    pub page_budget: usize,
    pub thumbnail_budget: usize,
    pub metrics: Option<PathBuf>,
    pub state_dir: Option<PathBuf>,
    pub initial: Option<PathBuf>,
}

#[derive(Clone, Copy)]
enum MenuAction {
    Open,
    Save,
    Close,
    Quit,
    Undo,
    Redo,
    Copy,
    Find,
    Select,
    Clear,
    Sidebar,
    Panel,
    First,
    Previous,
    Next,
    Last,
    FitPage,
    FitWidth,
    Light,
    Dark,
    Organize,
    Import,
    Extract,
    Split,
    Png,
    Optimize,
    Help,
    About,
}
impl WindowShell {
    fn action(&mut self, action: MenuAction, window: &mut Window, cx: &mut Context<Self>) {
        if !self.enabled(action, cx) {
            return;
        }
        use MenuAction::*;
        match action {
            Open => self.choose_open(cx),
            Close => {
                self.begin_close(false, cx);
            }
            Quit => {
                if self.begin_close(true, cx) {
                    window.remove_window();
                }
            }
            Light | Dark => Theme::change(
                if matches!(action, Light) {
                    ThemeMode::Light
                } else {
                    ThemeMode::Dark
                },
                Some(window),
                cx,
            ),
            Help | About => {
                let message = if matches!(action, Help) {
                    "Ctrl+O Buka PDF · Ctrl+W Tutup tab · Ctrl+Tab / Ctrl+Shift+Tab Pindah tab\nCtrl+Shift+S Simpan sebagai · Ctrl+Shift+O Impor PDF\nCtrl+Z / Ctrl+Y Undo / Redo · Ctrl+C Salin\nCtrl+F Cari teks · F3 / Shift+F3 Hasil berikutnya / sebelumnya\nCtrl+G Halaman · Ctrl+0 Fit halaman · Ctrl+1 Fit lebar"
                } else {
                    "PDF Studio 0.1.0\nPengatur PDF lokal untuk Windows. Sumber tetap utuh; output disimpan ke file baru."
                };
                window.open_alert_dialog(cx, move |d, _, _| {
                    d.title(if matches!(action, Help) {
                        "Shortcut"
                    } else {
                        "Tentang PDF Studio"
                    })
                    .description(message)
                    .centered()
                });
            }
            _ => {
                if let Some(s) = self.session() {
                    s.update(cx, |s, cx| match action {
                        Save => s.start_export(pdf_application::export::ExportScope::All, cx),
                        Undo | Redo => s.history(matches!(action, Redo), cx),
                        Copy => s.copy_text(cx),
                        Find => s.show_search(window, cx),
                        Select | Clear => {
                            if s.mode == ViewerMode::Organize {
                                s.select_all_pages(matches!(action, Select), cx);
                            } else if matches!(action, Clear) {
                                s.clear_selection(cx);
                            }
                        }
                        Sidebar => {
                            s.sidebar = !s.sidebar;
                            cx.notify();
                        }
                        Panel => s.set_mode(
                            if s.mode == ViewerMode::Organize {
                                ViewerMode::Read
                            } else {
                                ViewerMode::Organize
                            },
                            window,
                            cx,
                        ),
                        Organize => s.set_mode(ViewerMode::Organize, window, cx),
                        Import => s.import_pdf(cx),
                        Extract => {
                            s.start_export(pdf_application::export::ExportScope::Selected, cx)
                        }
                        Split => s.show_split(window, cx),
                        Png => s.show_png(window, cx),
                        Optimize => {
                            s.start_export(pdf_application::export::ExportScope::Optimized, cx)
                        }
                        FitPage | FitWidth => s.set_zoom(
                            if matches!(action, FitPage) {
                                Zoom::FitPage
                            } else {
                                Zoom::FitWidth
                            },
                            cx,
                        ),
                        First => s.navigate(Direction::First, cx),
                        Previous => s.navigate(Direction::Previous, cx),
                        Next => s.navigate(Direction::Next, cx),
                        Last => s.navigate(Direction::Last, cx),
                        _ => {}
                    });
                }
            }
        }
    }
    fn enabled(&self, action: MenuAction, cx: &App) -> bool {
        use MenuAction::*;
        if self.close.is_some() || self.modal(cx) {
            return false;
        }
        if matches!(action, Open) {
            return !self.opening;
        }
        if matches!(action, Quit | Light | Dark | Help | About) {
            return true;
        }
        let Some(s) = self.session() else {
            return false;
        };
        let s = s.read(cx);
        let content = s.workspace.is_some() && !s.opening;
        match action {
            Close => !self.opening,
            Undo => content && s.workspace.as_ref().is_some_and(|w| w.can_undo()),
            Redo => content && s.workspace.as_ref().is_some_and(|w| w.can_redo()),
            Copy => s.text.selection.is_some(),
            Select | Clear => content && s.mode == ViewerMode::Organize,
            Save | Optimize | Split | Png => {
                content
                    && s.navigation.count > 0
                    && self.resources.export_owner.load(Ordering::SeqCst) == 0
            }
            Extract => {
                content
                    && !s.selected_ids().is_empty()
                    && self.resources.export_owner.load(Ordering::SeqCst) == 0
            }
            _ => content,
        }
    }
    fn menu(
        &self,
        label: &'static str,
        entries: &[(&'static str, MenuAction)],
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let weak = cx.weak_entity();
        let entries: Vec<_> = entries
            .iter()
            .map(|(label, action)| (*label, *action, self.enabled(*action, cx)))
            .collect();
        Button::new(label)
            .ghost()
            .label(label)
            .dropdown_menu(move |mut menu, _, _| {
                for (label, action, enabled) in &entries {
                    let weak = weak.clone();
                    let action = *action;
                    menu = menu.item(PopupMenuItem::new(*label).disabled(!enabled).on_click(
                        move |_, window, cx| {
                            let _ = weak.update(cx, |this, cx| this.action(action, window, cx));
                        },
                    ));
                }
                menu
            })
            .into_any_element()
    }
}
#[path = "tab_management.rs"]
mod tab_management;
use tab_management::{TabDrag, TabDragLabel, tab_context_menu};

impl Render for WindowShell {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        use MenuAction::*;
        let colors = cx.theme().colors;
        let mut strip = div()
            .id("document-tabs")
            .role(gpui::Role::TabList)
            .flex()
            .items_center()
            .overflow_x_scroll()
            .track_scroll(&self.tab_scroll)
            .min_w_0()
            .flex_shrink(1.)
            .on_drag_move(
                cx.listener(|this, event: &gpui::DragMoveEvent<TabDrag>, _, cx| {
                    if this.close.is_some() || this.modal(cx) {
                        return;
                    }
                    let x = event.event.position.x;
                    let delta = if x < event.bounds.left() + px(32.) {
                        24.
                    } else if x > event.bounds.right() - px(32.) {
                        -24.
                    } else {
                        0.
                    };
                    if delta != 0. {
                        let mut offset = this.tab_scroll.offset();
                        offset.x += px(delta);
                        this.tab_scroll.set_offset(offset);
                        cx.notify();
                    }
                }),
            );
        for tab in &self.tabs {
            let id = tab.id;
            let name = tab.path.file_name().unwrap_or_default().to_string_lossy();
            let duplicate = self
                .tabs
                .iter()
                .filter(|t| t.path.file_name() == tab.path.file_name())
                .count()
                > 1;
            let dirty = tab.session.as_ref().is_some_and(|s| {
                s.read(cx)
                    .workspace
                    .as_ref()
                    .is_some_and(Workspace::is_dirty)
            });
            let label = format!(
                "{}{}{}",
                name,
                if duplicate {
                    format!(
                        " ({})",
                        tab.path
                            .parent()
                            .unwrap_or(std::path::Path::new(""))
                            .display()
                    )
                } else {
                    String::new()
                },
                if dirty {
                    " •"
                } else if tab.session.is_none() {
                    " …"
                } else {
                    ""
                }
            );
            let weak = cx.weak_entity();
            let starred = self.favorites.contains(&path_key(&tab.path));
            let blocked = self.close.is_some() || self.modal(cx);
            strip = strip.child(
                div()
                    .id(("document-tab", id.0))
                    .flex_shrink_0()
                    .child(
                        DocumentTab::new()
                            .folder()
                            .selected(self.active == Some(id))
                            .max_w(px(260.))
                            .flex_shrink_0()
                            .aria_label(label.clone())
                            .child(div().max_w(px(160.)).truncate().child(label))
                            .prefix(
                                Button::new(("star-tab", id.0))
                                    .ghost()
                                    .icon(if starred {
                                        IconName::StarFill
                                    } else {
                                        IconName::Star
                                    })
                                    .when(starred, |button| {
                                        button.custom(
                                            ButtonCustomVariant::new(cx)
                                                .foreground(gpui::rgb(0xeab308).into()),
                                        )
                                    })
                                    .w(px(24.))
                                    .h(px(24.))
                                    .accessibility_label(format!(
                                        "{}: {name}",
                                        if starred { "Hapus bintang" } else { "Bintangi" }
                                    ))
                                    .disabled(blocked || !self.favorites_ready)
                                    .on_mouse_down(gpui::MouseButton::Left, |_, _, cx| {
                                        cx.stop_propagation()
                                    })
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        cx.stop_propagation();
                                        this.toggle_favorite(id, cx);
                                    })),
                            )
                            .when(!blocked, |tab| {
                                tab.on_drag(TabDrag(id), move |_, _, _, cx| {
                                    cx.new(|_| TabDragLabel)
                                })
                                .drag_over::<TabDrag>(
                                    move |style, _, _, _| {
                                        style.border_l_2().border_color(colors.primary)
                                    },
                                )
                            })
                            .on_drop(cx.listener(move |this, drag: &TabDrag, _, cx| {
                                this.move_tab(drag.0, Some(id), cx)
                            }))
                            .on_click(cx.listener(move |this, _, _, cx| this.switch(id, cx)))
                            .suffix(
                                Button::new(("close-tab", id.0))
                                    .ghost()
                                    .icon(IconName::X)
                                    .w(px(24.))
                                    .h(px(24.))
                                    .accessibility_label(format!("Tutup tab: {name}"))
                                    .tooltip(format!("Tutup {}", tab.path.display()))
                                    .disabled(
                                        self.opening || self.close.is_some() || self.modal(cx),
                                    )
                                    .on_mouse_down(gpui::MouseButton::Left, |_, _, cx| {
                                        cx.stop_propagation()
                                    })
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        cx.stop_propagation();
                                        this.begin_close_target(false, Some(id), cx);
                                    })),
                            ),
                    )
                    .context_menu(move |menu, _, cx| tab_context_menu(menu, weak.clone(), id, cx)),
            );
        }
        strip = strip.child(
            div()
                .id("tab-drop-end")
                .w(px(12.))
                .h(px(32.))
                .flex_shrink_0()
                .drag_over::<TabDrag>(move |style, _, _, _| {
                    style.border_l_2().border_color(colors.primary)
                })
                .on_drop(
                    cx.listener(|this, drag: &TabDrag, _, cx| this.move_tab(drag.0, None, cx)),
                ),
        );
        let weak = cx.weak_entity();
        let entries: Vec<_> = self
            .tabs
            .iter()
            .map(|t| {
                (
                    t.id,
                    format!(
                        "{}{}",
                        if self.favorites.contains(&path_key(&t.path)) {
                            "★ "
                        } else {
                            ""
                        },
                        t.path.display()
                    ),
                )
            })
            .collect();
        let tab_list = Button::new("tab-list")
            .outline()
            .dropdown_caret(true)
            .label("Daftar tab")
            .dropdown_menu(move |mut menu, _, _| {
                for (id, name) in &entries {
                    let weak = weak.clone();
                    let id = *id;
                    menu = menu.item(PopupMenuItem::new(name.clone()).on_click(move |_, _, cx| {
                        let _ = weak.update(cx, |this, cx| this.switch(id, cx));
                    }));
                }
                menu
            });
        let menus = div()
            .flex()
            .gap_1()
            .child(self.menu(
                "Berkas",
                &[
                    ("Buka PDF…  Ctrl+O", Open),
                    ("Simpan sebagai…  Ctrl+Shift+S", Save),
                    ("Tutup tab  Ctrl+W", Close),
                    ("Keluar", Quit),
                ],
                cx,
            ))
            .child(self.menu(
                "Edit",
                &[
                    ("Undo  Ctrl+Z", Undo),
                    ("Redo  Ctrl+Y", Redo),
                    ("Salin  Ctrl+C", Copy),
                    ("Cari teks…  Ctrl+F", Find),
                    ("Pilih semua halaman", Select),
                    ("Bersihkan pilihan", Clear),
                ],
                cx,
            ))
            .child(self.menu(
                "Tampilan",
                &[
                    ("Thumbnail", Sidebar),
                    ("Panel alat", Panel),
                    ("Halaman pertama", First),
                    ("Halaman sebelumnya", Previous),
                    ("Halaman berikutnya", Next),
                    ("Halaman terakhir", Last),
                    ("Sesuaikan halaman", FitPage),
                    ("Sesuaikan lebar", FitWidth),
                    ("Terang", Light),
                    ("Gelap", Dark),
                ],
                cx,
            ))
            .child(self.menu(
                "Alat",
                &[
                    ("Atur halaman", Organize),
                    ("Impor PDF…", Import),
                    ("Ekstrak…", Extract),
                    ("Split PDF…", Split),
                    ("Ekspor PNG…", Png),
                    ("Optimize PDF (lossless)…", Optimize),
                ],
                cx,
            ))
            .child(self.menu(
                "Bantuan",
                &[("Daftar shortcut", Help), ("Tentang PDF Studio", About)],
                cx,
            ));
        let export_owner = self.resources.export_owner.load(Ordering::SeqCst);
        let job = self
            .tabs
            .iter()
            .find(|t| t.id.0 == export_owner)
            .and_then(|t| t.session.clone());
        let session = self.session();
        div()
            .size_full()
            .flex()
            .flex_col()
            .bg(colors.background)
            .text_color(colors.foreground)
            .track_focus(&self.focus)
            .on_action(cx.listener(|this, _: &crate::Open, _, cx| this.choose_open(cx)))
            .on_action(cx.listener(|this, _: &CloseTab, _, cx| {
                this.begin_close(false, cx);
            }))
            .on_action(cx.listener(|this, _: &NextTab, _, cx| this.cycle(false, cx)))
            .on_action(cx.listener(|this, _: &PreviousTab, _, cx| this.cycle(true, cx)))
            .child(TitleBar::new().child("PDF Studio"))
            .child(menus)
            .child(
                div()
                    .flex()
                    .items_center()
                    .border_b_1()
                    .border_color(colors.border)
                    .child(strip)
                    .child(
                        Button::new("new-tab")
                            .ghost()
                            .icon(IconName::Plus)
                            .accessibility_label("Buka PDF di tab baru")
                            .disabled(!self.enabled(Open, cx) || self.tabs.len() >= MAX_TABS)
                            .on_click(cx.listener(|this, _, _, cx| this.choose_open(cx))),
                    )
                    .child(div().flex_1())
                    .child(tab_list),
            )
            .when_some(self.notice.clone(), |el, notice| {
                el.child(
                    div()
                        .id("shell-notice")
                        .flex()
                        .items_center()
                        .px_3()
                        .py_2()
                        .child(
                            div()
                                .id("shell-notice-message")
                                .flex_1()
                                .text_sm()
                                .role(gpui::Role::Group)
                                .aria_label(notice.clone())
                                .child(notice),
                        )
                        .child(
                            Button::new("dismiss-shell-notice")
                                .ghost()
                                .label("Tutup")
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.notice = None;
                                    cx.notify();
                                })),
                        ),
                )
            })
            .when(self.close.is_some(), |el| {
                el.child(
                    Button::new("cancel-close-batch")
                        .ghost()
                        .label("Tetap di aplikasi")
                        .on_click(cx.listener(|this, _, _, cx| this.cancel_close(cx))),
                )
            })
            .when_some(job, |el, job| {
                let name = self
                    .tabs
                    .iter()
                    .find(|t| t.id.0 == export_owner)
                    .map(|t| {
                        t.path
                            .file_name()
                            .unwrap_or_default()
                            .to_string_lossy()
                            .into_owned()
                    })
                    .unwrap_or_default();
                let s = job.read(cx);
                let phase = s.export.phase_label();
                let text = format!("{name} — {phase}");
                el.child(
                    div()
                        .flex()
                        .px_3()
                        .items_center()
                        .child(div().flex_1().text_sm().child(text))
                        .child(
                            Button::new("cancel-window-export")
                                .ghost()
                                .label("Batal ekspor")
                                .on_click(move |_, _, cx| {
                                    job.update(cx, |s, cx| s.cancel_export(cx))
                                }),
                        ),
                )
            })
            .when_some(session.clone(), |el, s| {
                el.child(div().flex_1().min_h_0().child(s))
            })
            .when(session.is_none(), |el| {
                el.child(
                    div()
                        .flex_1()
                        .flex()
                        .flex_col()
                        .items_center()
                        .justify_center()
                        .gap_3()
                        .child(if self.opening {
                            "Memeriksa dokumen…"
                        } else {
                            "Buka beberapa PDF, satu dokumen per tab."
                        })
                        .child(
                            Button::new("open-empty")
                                .primary()
                                .label("Buka PDF")
                                .disabled(self.opening)
                                .on_click(cx.listener(|this, _, _, cx| this.choose_open(cx))),
                        ),
                )
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn loading_slots_count_and_duplicate_windows_paths_match() {
        assert!(capacity(6, 2));
        assert!(!capacity(6, 3));
        assert!(capacity(8, 0));
        assert_eq!(
            path_key(std::path::Path::new("D:/Books/A.pdf")),
            path_key(std::path::Path::new("d:\\books\\a.PDF"))
        );
    }
}
