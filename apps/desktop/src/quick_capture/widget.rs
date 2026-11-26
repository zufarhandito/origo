//! Always-on-top floating Quick Capture pill & expandable drawer window.
use super::{
    overlay::RegionPickerOverlay,
    store::{
        AddItemOutcome, CaptureSessionData, CaptureSessionStore, ImageFolderFormat,
        MAX_CAPTURE_ITEMS, OcrTxtOptions, PdfExportPreset, export_session_ocr_txt,
        export_session_to_images, export_session_to_pdf,
    },
    win_capture::{
        CaptureRegion, CapturedFrame, GlobalHotkeyListener, HotkeyAction, capture_monitor_rgba,
        configure_floating_hwnd, crop_rgba, monitor_count, restore_window_from_minimized,
        rgba_to_render_image,
    },
};
use crate::shell::WindowShell;
use gpui::{
    AnyWindowHandle, App, Bounds, Context, FocusHandle, MouseButton, PathPromptOptions, Render,
    RenderImage, WeakEntity, Window, WindowBounds, WindowDecorations, WindowKind, WindowOptions,
    component::{
        ActiveTheme, Disableable, Selectable, Sizable,
        button::{Button, ButtonVariants},
    },
    div, prelude::*, px, size,
};
use pdf_application::export::ExportControl;
use pdf_engine::PdfiumRenderer;
use pdf_ocr::{Language, Tesseract};
use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};

const PILL_WIDTH: f32 = 388.0;
const PILL_HEIGHT_COMPACT: f32 = 52.0;
const PILL_HEIGHT_BANNER: f32 = 90.0;
const DRAWER_WIDTH: f32 = 412.0;
const DRAWER_HEIGHT: f32 = 584.0;

#[derive(Clone, Copy, PartialEq, Eq)]
enum DrawerTab {
    List,
    Export,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum BannerTone {
    Info,
    Success,
    Warning,
    Error,
}

#[derive(Clone)]
struct StatusBanner {
    tone: BannerTone,
    text: String,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ExportKind {
    Pdf,
    Images,
    OcrTxt,
}

struct ActiveExport {
    kind: ExportKind,
    total: usize,
    done: Arc<AtomicUsize>,
    control: ExportControl,
}

impl ActiveExport {
    fn label(&self) -> String {
        let done = self.done.load(Ordering::SeqCst);
        let total = self.total;
        match self.kind {
            ExportKind::Pdf => format!("Menyusun PDF ({done}/{total})…"),
            ExportKind::Images => format!("Menyimpan gambar ({done}/{total})…"),
            ExportKind::OcrTxt => format!("OCR soal ke TXT ({done}/{total})…"),
        }
    }
}

pub struct QuickCaptureContext {
    pub main_shell: WeakEntity<WindowShell>,
    pub main_window: AnyWindowHandle,
    pub state_dir: Option<PathBuf>,
    pub sidecars: PathBuf,
    pub renderer: PdfiumRenderer,
}

pub struct QuickCaptureWidget {
    main_shell: WeakEntity<WindowShell>,
    main_window: AnyWindowHandle,
    sidecars: PathBuf,
    renderer: PdfiumRenderer,
    store: CaptureSessionStore,
    data: CaptureSessionData,
    focus: FocusHandle,
    expanded: bool,
    tab: DrawerTab,
    capturing: bool,
    picking_region: bool,
    retake_target: Option<usize>,
    pending_duplicate: Option<CapturedFrame>,
    confirm_clear: bool,
    banner: Option<StatusBanner>,
    hotkeys: GlobalHotkeyListener,
    thumbs: HashMap<u64, Arc<RenderImage>>,
    loading_thumbs: HashSet<u64>,
    preview: Option<(u64, Arc<RenderImage>)>,
    pdf_preset: PdfExportPreset,
    open_pdf_after: bool,
    image_format: ImageFolderFormat,
    ocr_language: Language,
    ocr_headers: bool,
    active_export: Option<ActiveExport>,
    last_window_height: f32,
}

impl QuickCaptureWidget {
    pub fn open(
        ctx: QuickCaptureContext,
        cx: &mut App,
    ) -> anyhow::Result<(AnyWindowHandle, gpui::Entity<Self>)> {
        let primary = cx.primary_display();
        let origin = if let Some(display) = primary {
            let b = display.bounds();
            gpui::point(
                b.origin.x + b.size.width - px(PILL_WIDTH + 24.0),
                b.origin.y + px(24.0),
            )
        } else {
            gpui::point(px(64.0), px(24.0))
        };
        let bounds = Bounds::new(origin, size(px(PILL_WIDTH), px(PILL_HEIGHT_COMPACT)));

        let (handle, view) = gpui_kit::open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                titlebar: None,
                focus: true,
                show: true,
                kind: WindowKind::PopUp,
                is_movable: true,
                is_resizable: false,
                is_minimizable: false,
                window_decorations: Some(WindowDecorations::Client),
                ..Default::default()
            },
            cx,
            |window, cx| {
                configure_floating_hwnd(window);
                window.activate_window();
                let widget_window = window.window_handle();
                cx.new(|cx| {
                    let store = CaptureSessionStore::new(ctx.state_dir.as_deref());
                    let data = store.load();
                    let hotkeys = GlobalHotkeyListener::start(data.hotkey);
                    let status = hotkeys.status();
                    let banner = if !status.capture_ok {
                        Some(StatusBanner {
                            tone: BannerTone::Warning,
                            text: format!(
                                "Hotkey {} dipakai aplikasi lain — ubah di drawer.",
                                data.hotkey.label()
                            ),
                        })
                    } else if data.region.is_none() {
                        Some(StatusBanner {
                            tone: BannerTone::Info,
                            text: format!(
                                "Pilih Area sekali, lalu tekan {} untuk jepret cepat.",
                                data.hotkey.label()
                            ),
                        })
                    } else {
                        None
                    };
                    let focus = cx.focus_handle();
                    let main_shell_close = ctx.main_shell.clone();
                    let main_window_close = ctx.main_window;
                    window.on_window_should_close(cx, move |_, cx| {
                        let _ = main_shell_close.update(cx, |shell, cx| {
                            shell.clear_quick_capture_window(cx);
                        });
                        let _ = main_window_close.update(cx, |_, main_win, _| {
                            restore_window_from_minimized(main_win);
                            main_win.activate_window();
                        });
                        true
                    });

                    let mut widget = Self {
                        main_shell: ctx.main_shell,
                        main_window: ctx.main_window,
                        sidecars: ctx.sidecars,
                        renderer: ctx.renderer,
                        store,
                        data,
                        focus,
                        expanded: false,
                        tab: DrawerTab::List,
                        capturing: false,
                        picking_region: false,
                        retake_target: None,
                        pending_duplicate: None,
                        confirm_clear: false,
                        banner,
                        hotkeys,
                        thumbs: HashMap::new(),
                        loading_thumbs: HashSet::new(),
                        preview: None,
                        pdf_preset: PdfExportPreset::AiCompactJpeg85,
                        open_pdf_after: true,
                        image_format: ImageFolderFormat::Jpeg90,
                        ocr_language: Language::IndonesianEnglish,
                        ocr_headers: true,
                        active_export: None,
                        last_window_height: PILL_HEIGHT_COMPACT,
                    };
                    widget.sync_window_size(window);
                    widget.start_hotkey_poller(widget_window, cx);
                    widget
                })
            },
        )?;
        Ok((handle, view))
    }

    fn start_hotkey_poller(&self, widget_window: AnyWindowHandle, cx: &mut Context<Self>) {
        cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(45))
                    .await;
                let keep_going = widget_window.update(cx, |_, window, cx| {
                    this.update(cx, |this, cx| {
                        this.poll_hotkeys(window, cx);
                        if this.active_export.is_some() {
                            cx.notify();
                        }
                        true
                    })
                });
                if !matches!(keep_going, Ok(Ok(true))) {
                    break;
                }
            }
        })
        .detach();
    }

    fn poll_hotkeys(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        while let Some(action) = self.hotkeys.try_recv() {
            match action {
                HotkeyAction::Capture => {
                    self.trigger_quick_capture(window, cx);
                }
                HotkeyAction::PickRegion => {
                    self.start_region_picker(None, window, cx);
                }
            }
        }
    }

    fn desired_window_size(&self) -> (f32, f32) {
        if self.expanded {
            (DRAWER_WIDTH, DRAWER_HEIGHT)
        } else if self.banner.is_some()
            || self.pending_duplicate.is_some()
            || self.active_export.is_some()
        {
            (PILL_WIDTH, PILL_HEIGHT_BANNER)
        } else {
            (PILL_WIDTH, PILL_HEIGHT_COMPACT)
        }
    }

    fn sync_window_size(&mut self, window: &mut Window) {
        let (w, h) = self.desired_window_size();
        if (self.last_window_height - h).abs() > 0.5
            || (f32::from(window.viewport_size().width) - w).abs() > 2.0
        {
            self.last_window_height = h;
            window.resize(size(px(w), px(h)));
            configure_floating_hwnd(window);
        }
    }

    fn set_banner(&mut self, tone: BannerTone, text: impl Into<String>, window: &mut Window) {
        self.banner = Some(StatusBanner {
            tone,
            text: text.into(),
        });
        self.sync_window_size(window);
    }

    fn clear_banner(&mut self, window: &mut Window) {
        self.banner = None;
        self.pending_duplicate = None;
        self.sync_window_size(window);
    }

    fn toggle_drawer(&mut self, tab: Option<DrawerTab>, window: &mut Window, cx: &mut Context<Self>) {
        match tab {
            Some(target) => {
                if self.expanded && self.tab == target {
                    self.expanded = false;
                } else {
                    self.expanded = true;
                    self.tab = target;
                }
            }
            None => {
                self.expanded = !self.expanded;
            }
        }
        self.confirm_clear = false;
        self.sync_window_size(window);
        if self.expanded && self.tab == DrawerTab::List {
            self.ensure_thumbs_loaded(cx);
        }
        cx.notify();
    }

    fn restore_origo(&mut self, cx: &mut Context<Self>) {
        let main_window = self.main_window;
        cx.defer(move |cx| {
            let _ = main_window.update(cx, |_, window, _| {
                restore_window_from_minimized(window);
                window.activate_window();
            });
        });
    }

    fn close_widget(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let main_shell = self.main_shell.clone();
        let main_window = self.main_window;
        cx.defer(move |cx| {
            let _ = main_shell.update(cx, |shell, cx| {
                shell.clear_quick_capture_window(cx);
            });
            let _ = main_window.update(cx, |_, main_win, _| {
                restore_window_from_minimized(main_win);
                main_win.activate_window();
            });
        });
        window.remove_window();
    }

    fn trigger_quick_capture(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.capturing || self.picking_region || self.active_export.is_some() {
            return;
        }
        let Some(region) = self.data.region else {
            self.start_region_picker(None, window, cx);
            return;
        };
        self.capture_region_now(region, None, window, cx);
    }

    fn retake_item(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        if self.capturing || self.picking_region || self.active_export.is_some() {
            return;
        }
        if let Some(region) = self.data.region {
            self.capture_region_now(region, Some(index), window, cx);
        } else {
            self.start_region_picker(Some(index), window, cx);
        }
    }

    fn capture_region_now(
        &mut self,
        region: CaptureRegion,
        retake_index: Option<usize>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.capturing = true;
        self.pending_duplicate = None;
        self.confirm_clear = false;
        let widget_window = window.window_handle();
        cx.notify();

        let task = cx.background_executor().spawn(async move {
            let full = capture_monitor_rgba(region.monitor_index)?;
            crop_rgba(&full, region)
        });

        cx.spawn(async move |this, cx| {
            let result = task.await;
            let _ = widget_window.update(cx, |_, window, cx| {
                let _ = this.update(cx, |this, cx| {
                    this.capturing = false;
                    match result {
                        Ok(cropped) => {
                            this.commit_captured_frame(cropped, retake_index, false, window, cx);
                        }
                        Err(err) => {
                            this.set_banner(BannerTone::Error, format!("{err}"), window);
                        }
                    }
                    cx.notify();
                });
            });
        })
        .detach();
    }

    fn start_region_picker(
        &mut self,
        retake_index: Option<usize>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.capturing || self.picking_region || self.active_export.is_some() {
            return;
        }
        self.picking_region = true;
        self.retake_target = retake_index;
        self.pending_duplicate = None;
        self.confirm_clear = false;
        let monitor_index = self.data.monitor_index;
        let widget_window = window.window_handle();
        self.set_banner(BannerTone::Info, "Membekukan layar untuk pilih area…", window);
        cx.notify();

        let task = cx
            .background_executor()
            .spawn(async move { capture_monitor_rgba(monitor_index) });

        cx.spawn(async move |this, cx| {
            let result = task.await;
            let _ = widget_window.update(cx, |_, window, cx| {
                let _ = this.update(cx, |this, cx| {
                    match result {
                        Ok(frame) => {
                            let weak = cx.weak_entity();
                            let win_for_cb = widget_window;
                            let opened = RegionPickerOverlay::open(
                                monitor_index,
                                frame,
                                Box::new(move |picked, cx| {
                                    let _ = win_for_cb.update(cx, |_, window, cx| {
                                        let _ = weak.update(cx, |this, cx| {
                                            this.picking_region = false;
                                            let retake = this.retake_target.take();
                                            match picked {
                                                Some((region, cropped)) => {
                                                    this.data.region = Some(region);
                                                    this.data.monitor_index = region.monitor_index;
                                                    let _ = this.store.save(&this.data);
                                                    this.commit_captured_frame(
                                                        cropped, retake, false, window, cx,
                                                    );
                                                }
                                                None => {
                                                    this.set_banner(
                                                        BannerTone::Info,
                                                        "Pemilihan area dibatalkan.",
                                                        window,
                                                    );
                                                }
                                            }
                                            cx.notify();
                                        });
                                    });
                                }),
                                cx,
                            );
                            if let Err(err) = opened {
                                this.picking_region = false;
                                this.retake_target = None;
                                this.set_banner(
                                    BannerTone::Error,
                                    format!("Overlay gagal dibuka: {err}"),
                                    window,
                                );
                            }
                        }
                        Err(err) => {
                            this.picking_region = false;
                            this.retake_target = None;
                            this.set_banner(BannerTone::Error, format!("{err}"), window);
                        }
                    }
                    cx.notify();
                });
            });
        })
        .detach();
    }

    fn commit_captured_frame(
        &mut self,
        frame: CapturedFrame,
        retake_index: Option<usize>,
        force: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(index) = retake_index {
            match self.store.replace_capture(&mut self.data, index, &frame) {
                Ok(meta) => {
                    self.thumbs.remove(&meta.id);
                    if self.preview.as_ref().is_some_and(|(id, _)| *id == meta.id) {
                        self.preview = rgba_to_render_image(frame.width, frame.height, &frame.rgba)
                            .map(|img| (meta.id, img));
                    }
                    self.ensure_thumb_loaded(meta.id, cx);
                    self.set_banner(
                        BannerTone::Success,
                        format!(
                            "Soal #{} di-retake ({}×{} px).",
                            index + 1,
                            meta.width,
                            meta.height
                        ),
                        window,
                    );
                }
                Err(err) => {
                    self.set_banner(BannerTone::Error, format!("Gagal retake: {err}"), window);
                }
            }
            return;
        }

        match self.store.add_capture(&mut self.data, &frame, force) {
            Ok(AddItemOutcome::Added(meta)) => {
                self.pending_duplicate = None;
                self.ensure_thumb_loaded(meta.id, cx);
                let count = self.data.items.len();
                self.set_banner(
                    BannerTone::Success,
                    format!(
                        "Tersimpan Soal #{count} ({}×{} px) · {} untuk lanjut",
                        meta.width,
                        meta.height,
                        self.data.hotkey.label()
                    ),
                    window,
                );
            }
            Ok(AddItemOutcome::DuplicateDetected { last_index }) => {
                self.pending_duplicate = Some(frame);
                self.set_banner(
                    BannerTone::Warning,
                    format!("Identik dengan Soal #{last_index} (lupa klik Next?)"),
                    window,
                );
            }
            Ok(AddItemOutcome::LimitReached) => {
                self.set_banner(
                    BannerTone::Warning,
                    format!("Maksimal {MAX_CAPTURE_ITEMS} tangkapan per sesi. Ekspor dulu ya."),
                    window,
                );
            }
            Err(err) => {
                self.set_banner(BannerTone::Error, format!("Gagal menyimpan: {err}"), window);
            }
        }
    }

    fn force_save_duplicate(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(frame) = self.pending_duplicate.take() {
            self.commit_captured_frame(frame, None, true, window, cx);
            cx.notify();
        }
    }

    fn undo_last(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.active_export.is_some() {
            return;
        }
        let removed_id = self.data.items.last().map(|i| i.id);
        match self.store.undo_last(&mut self.data) {
            Ok(true) => {
                if let Some(id) = removed_id {
                    self.thumbs.remove(&id);
                    if self.preview.as_ref().is_some_and(|(pid, _)| *pid == id) {
                        self.preview = None;
                    }
                }
                self.set_banner(
                    BannerTone::Info,
                    format!("Tangkapan terakhir dihapus (sisa {}).", self.data.items.len()),
                    window,
                );
            }
            Ok(false) => {}
            Err(err) => self.set_banner(BannerTone::Error, format!("{err}"), window),
        }
        cx.notify();
    }

    fn delete_item(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        if self.active_export.is_some() {
            return;
        }
        let removed_id = self.data.items.get(index).map(|i| i.id);
        if let Err(err) = self.store.delete_item(&mut self.data, index) {
            self.set_banner(BannerTone::Error, format!("{err}"), window);
        } else {
            if let Some(id) = removed_id {
                self.thumbs.remove(&id);
                if self.preview.as_ref().is_some_and(|(pid, _)| *pid == id) {
                    self.preview = None;
                }
            }
            self.set_banner(
                BannerTone::Info,
                format!("Soal #{} dihapus (sisa {}).", index + 1, self.data.items.len()),
                window,
            );
        }
        cx.notify();
    }

    fn move_item(&mut self, from: usize, to: usize, window: &mut Window, cx: &mut Context<Self>) {
        if self.active_export.is_some() {
            return;
        }
        if let Err(err) = self.store.move_item(&mut self.data, from, to) {
            self.set_banner(BannerTone::Error, format!("{err}"), window);
        }
        cx.notify();
    }

    fn clear_all_items(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.active_export.is_some() {
            return;
        }
        self.confirm_clear = false;
        self.preview = None;
        self.thumbs.clear();
        self.loading_thumbs.clear();
        if let Err(err) = self.store.clear_items(&mut self.data) {
            self.set_banner(BannerTone::Error, format!("{err}"), window);
        } else {
            self.set_banner(BannerTone::Info, "Semua tangkapan sesi dibersihkan.", window);
        }
        cx.notify();
    }

    fn toggle_hotkey_preset(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.data.hotkey = self.data.hotkey.toggle();
        let _ = self.store.save(&self.data);
        self.hotkeys = GlobalHotkeyListener::start(self.data.hotkey);
        let status = self.hotkeys.status();
        if status.capture_ok {
            self.set_banner(
                BannerTone::Success,
                format!("Hotkey aktif: {} (Area: Alt+Shift+R)", self.data.hotkey.label()),
                window,
            );
        } else {
            self.set_banner(
                BannerTone::Warning,
                format!(
                    "Hotkey {} gagal didaftarkan (bentrok dengan aplikasi lain).",
                    self.data.hotkey.label()
                ),
                window,
            );
        }
        cx.notify();
    }

    fn toggle_duplicate_guard(&mut self, cx: &mut Context<Self>) {
        self.data.duplicate_guard = !self.data.duplicate_guard;
        let _ = self.store.save(&self.data);
        cx.notify();
    }

    fn cycle_monitor(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let total = monitor_count();
        if total <= 1 {
            return;
        }
        self.data.monitor_index = (self.data.monitor_index % total) + 1;
        self.data.region = None;
        let _ = self.store.save(&self.data);
        self.set_banner(
            BannerTone::Info,
            format!(
                "Target berpindah ke Layar {}. Klik Area untuk mengunci area soal.",
                self.data.monitor_index
            ),
            window,
        );
        cx.notify();
    }

    fn open_preview(&mut self, id: u64, cx: &mut Context<Self>) {
        if self.preview.as_ref().is_some_and(|(pid, _)| *pid == id) {
            self.preview = None;
            cx.notify();
            return;
        }
        let store = self.store.clone();
        let task = cx.background_executor().spawn(async move {
            let frame = store.load_item_rgba(id).ok()?;
            rgba_to_render_image(frame.width, frame.height, &frame.rgba)
        });
        cx.spawn(async move |this, cx| {
            if let Some(image) = task.await {
                let _ = this.update(cx, |this, cx| {
                    this.preview = Some((id, image));
                    cx.notify();
                });
            }
        })
        .detach();
    }

    fn ensure_thumbs_loaded(&mut self, cx: &mut Context<Self>) {
        let ids: Vec<u64> = self
            .data
            .items
            .iter()
            .map(|i| i.id)
            .filter(|id| !self.thumbs.contains_key(id) && !self.loading_thumbs.contains(id))
            .collect();
        for id in ids {
            self.ensure_thumb_loaded(id, cx);
        }
    }

    fn ensure_thumb_loaded(&mut self, id: u64, cx: &mut Context<Self>) {
        if self.thumbs.contains_key(&id) || !self.loading_thumbs.insert(id) {
            return;
        }
        let store = self.store.clone();
        let task = cx.background_executor().spawn(async move {
            let frame = store.load_thumb_rgba(id)?;
            rgba_to_render_image(frame.width, frame.height, &frame.rgba)
        });
        cx.spawn(async move |this, cx| {
            let result = task.await;
            let _ = this.update(cx, |this, cx| {
                this.loading_thumbs.remove(&id);
                if let Some(image) = result
                    && this.data.items.iter().any(|item| item.id == id)
                {
                    this.thumbs.insert(id, image);
                    cx.notify();
                }
            });
        })
        .detach();
    }

    fn default_export_dir(&self) -> PathBuf {
        std::env::var_os("USERPROFILE")
            .map(|home| PathBuf::from(home).join("Documents"))
            .filter(|p| p.is_dir())
            .unwrap_or_else(|| self.store.root().to_path_buf())
    }

    fn start_export_pdf(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.active_export.is_some() || self.data.items.is_empty() {
            return;
        }
        let stamp = chrono::Local::now().format("%Y%m%d-%H%M").to_string();
        let suggested = format!("soal-capture-{stamp}.pdf");
        let dir = self.default_export_dir();
        let picker = cx.prompt_for_new_path(&dir, Some(&suggested));
        let widget_window = window.window_handle();

        cx.spawn(async move |this, cx| {
            let picked = picker.await;
            let destination = match picked {
                Ok(Ok(Some(mut path))) => {
                    if !path
                        .extension()
                        .is_some_and(|e| e.eq_ignore_ascii_case("pdf"))
                    {
                        path.set_extension("pdf");
                    }
                    path
                }
                _ => return,
            };

            let started = widget_window.update(cx, |_, window, cx| {
                this.update(cx, |this, cx| {
                    if this.active_export.is_some() || this.data.items.is_empty() {
                        return None;
                    }
                    let items = this.data.items.clone();
                    let total = items.len();
                    let done = Arc::new(AtomicUsize::new(0));
                    let control = ExportControl::with_budget(Duration::from_secs(600));
                    this.active_export = Some(ActiveExport {
                        kind: ExportKind::Pdf,
                        total,
                        done: done.clone(),
                        control: control.clone(),
                    });
                    this.sync_window_size(window);
                    cx.notify();
                    Some((
                        this.store.clone(),
                        items,
                        this.pdf_preset,
                        this.open_pdf_after,
                        this.renderer.clone(),
                        done,
                        control,
                    ))
                })
            });

            let Ok(Ok(Some((store, items, preset, open_after, renderer, done, control)))) = started
            else {
                return;
            };

            let task = cx.background_executor().spawn(async move {
                export_session_to_pdf(
                    &store,
                    &items,
                    preset,
                    &destination,
                    &renderer,
                    &control,
                    |n, _| {
                        done.store(n, Ordering::SeqCst);
                    },
                )
            });

            let result = task.await;
            let _ = widget_window.update(cx, |_, window, cx| {
                let _ = this.update(cx, |this, cx| {
                    this.active_export = None;
                    match result {
                        Ok(receipt) => {
                            let mb = receipt.bytes as f64 / (1024.0 * 1024.0);
                            let name = receipt
                                .destination
                                .file_name()
                                .unwrap_or_default()
                                .to_string_lossy()
                                .to_string();
                            this.set_banner(
                                BannerTone::Success,
                                format!("PDF tersimpan: {name} ({} hal · {mb:.1} MB)", receipt.pages),
                                window,
                            );
                            if open_after {
                                let main_shell = this.main_shell.clone();
                                let main_window = this.main_window;
                                let path = receipt.destination;
                                cx.defer(move |cx| {
                                    let _ = main_window.update(cx, |_, win, _| {
                                        restore_window_from_minimized(win);
                                        win.activate_window();
                                    });
                                    let _ = main_shell.update(cx, |shell, cx| {
                                        shell.open_paths(vec![path], cx);
                                    });
                                });
                            }
                        }
                        Err(err) => {
                            this.set_banner(BannerTone::Error, format!("{err}"), window);
                        }
                    }
                    cx.notify();
                });
            });
        })
        .detach();
    }

    fn start_export_images(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.active_export.is_some() || self.data.items.is_empty() {
            return;
        }
        let picker = cx.prompt_for_paths(PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: Some("Pilih folder tujuan untuk menyimpan gambar soal".into()),
        });
        let widget_window = window.window_handle();

        cx.spawn(async move |this, cx| {
            let picked = picker.await;
            let parent_dir = match picked {
                Ok(Ok(Some(paths))) => match paths.into_iter().next() {
                    Some(dir) => dir,
                    None => return,
                },
                _ => return,
            };

            let started = widget_window.update(cx, |_, window, cx| {
                this.update(cx, |this, cx| {
                    if this.active_export.is_some() || this.data.items.is_empty() {
                        return None;
                    }
                    let items = this.data.items.clone();
                    let total = items.len();
                    let done = Arc::new(AtomicUsize::new(0));
                    let control = ExportControl::with_budget(Duration::from_secs(300));
                    this.active_export = Some(ActiveExport {
                        kind: ExportKind::Images,
                        total,
                        done: done.clone(),
                        control: control.clone(),
                    });
                    this.sync_window_size(window);
                    cx.notify();
                    Some((this.store.clone(), items, this.image_format, done, control))
                })
            });

            let Ok(Ok(Some((store, items, format, done, control)))) = started else {
                return;
            };
            let stamp = chrono::Local::now().format("%Y%m%d-%H%M").to_string();
            let folder_name = format!("capture-soal-{stamp}");

            let task = cx.background_executor().spawn(async move {
                export_session_to_images(
                    &store,
                    &items,
                    format,
                    &parent_dir,
                    &folder_name,
                    &control,
                    |n, _| {
                        done.store(n, Ordering::SeqCst);
                    },
                )
            });

            let result = task.await;
            let _ = widget_window.update(cx, |_, window, cx| {
                let _ = this.update(cx, |this, cx| {
                    this.active_export = None;
                    match result {
                        Ok(dest) => {
                            let name = dest
                                .file_name()
                                .unwrap_or_default()
                                .to_string_lossy()
                                .to_string();
                            this.set_banner(
                                BannerTone::Success,
                                format!("Folder gambar tersimpan: {name} ({} file)", this.data.items.len()),
                                window,
                            );
                        }
                        Err(err) => {
                            this.set_banner(BannerTone::Error, format!("{err}"), window);
                        }
                    }
                    cx.notify();
                });
            });
        })
        .detach();
    }

    fn start_export_ocr_txt(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.active_export.is_some() || self.data.items.is_empty() {
            return;
        }
        let stamp = chrono::Local::now().format("%Y%m%d-%H%M").to_string();
        let suggested = format!("soal-ocr-{stamp}.txt");
        let dir = self.default_export_dir();
        let picker = cx.prompt_for_new_path(&dir, Some(&suggested));
        let widget_window = window.window_handle();

        cx.spawn(async move |this, cx| {
            let picked = picker.await;
            let destination = match picked {
                Ok(Ok(Some(mut path))) => {
                    if !path
                        .extension()
                        .is_some_and(|e| e.eq_ignore_ascii_case("txt"))
                    {
                        path.set_extension("txt");
                    }
                    path
                }
                _ => return,
            };

            let started = widget_window.update(cx, |_, window, cx| {
                this.update(cx, |this, cx| {
                    if this.active_export.is_some() || this.data.items.is_empty() {
                        return None;
                    }
                    let items = this.data.items.clone();
                    let total = items.len();
                    let done = Arc::new(AtomicUsize::new(0));
                    let control = ExportControl::with_budget(Duration::from_secs(1800));
                    this.active_export = Some(ActiveExport {
                        kind: ExportKind::OcrTxt,
                        total,
                        done: done.clone(),
                        control: control.clone(),
                    });
                    this.sync_window_size(window);
                    cx.notify();
                    Some((
                        this.store.clone(),
                        items,
                        this.sidecars.clone(),
                        this.ocr_language,
                        this.ocr_headers,
                        done,
                        control,
                    ))
                })
            });

            let Ok(Ok(Some((store, items, sidecars, language, headers, done, control)))) = started
            else {
                return;
            };

            let task = cx.background_executor().spawn(async move {
                let tesseract = Tesseract::new(&sidecars);
                export_session_ocr_txt(
                    &store,
                    &items,
                    &tesseract,
                    OcrTxtOptions {
                        language,
                        include_headers: headers,
                    },
                    &destination,
                    &control,
                    |n, _| {
                        done.store(n, Ordering::SeqCst);
                    },
                )
            });

            let result = task.await;
            let _ = widget_window.update(cx, |_, window, cx| {
                let _ = this.update(cx, |this, cx| {
                    this.active_export = None;
                    match result {
                        Ok(report) => {
                            let name = report
                                .destination
                                .file_name()
                                .unwrap_or_default()
                                .to_string_lossy()
                                .to_string();
                            this.set_banner(
                                BannerTone::Success,
                                format!(
                                    "TXT OCR tersimpan: {name} ({}/{} soal · {} kar)",
                                    report.non_empty_items, report.total_items, report.chars
                                ),
                                window,
                            );
                        }
                        Err(err) => {
                            this.set_banner(BannerTone::Error, format!("{err}"), window);
                        }
                    }
                    cx.notify();
                });
            });
        })
        .detach();
    }

    fn cancel_export(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(active) = &self.active_export {
            active.control.cancel();
            self.set_banner(BannerTone::Info, "Membatalkan ekspor…", window);
            cx.notify();
        }
    }

    fn render_list_tab(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let colors = cx.theme().colors;
        let busy = self.capturing || self.picking_region || self.active_export.is_some();
        let count = self.data.items.len();

        let preview_meta = self.preview.as_ref().and_then(|(pid, img)| {
            self.data
                .items
                .iter()
                .enumerate()
                .find(|(_, item)| item.id == *pid)
                .map(|(idx, item)| (idx, item.width, item.height, img.clone()))
        });

        let mut list = div()
            .id("qc-items-scroll")
            .flex_1()
            .min_h_0()
            .overflow_y_scroll()
            .flex()
            .flex_col()
            .gap_1p5()
            .pr_1();

        if count == 0 {
            list = list.child(
                div()
                    .p_4()
                    .rounded_md()
                    .border_1()
                    .border_color(colors.border)
                    .bg(colors.secondary)
                    .flex()
                    .flex_col()
                    .gap_1()
                    .child(
                        div()
                            .text_sm()
                            .text_color(colors.foreground)
                            .child("Belum ada tangkapan soal."),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(colors.muted_foreground)
                            .child(format!(
                                "1) Klik 'Area' untuk drag & kunci kotak soal di browser.\n2) Tekan {} (atau klik Jepret) untuk setiap soal berikutnya.\n3) Buka tab Ekspor untuk simpan ke 1 PDF, Gambar, atau OCR TXT.",
                                self.data.hotkey.label()
                            )),
                    ),
            );
        } else {
            for (index, item) in self.data.items.iter().enumerate() {
                let id = item.id;
                let thumb = self.thumbs.get(&id).cloned();
                let is_previewed = self.preview.as_ref().is_some_and(|(pid, _)| *pid == id);
                let is_first = index == 0;
                let is_last = index + 1 == count;
                let dims = format!("{} × {} px", item.width, item.height);

                list = list.child(
                    div()
                        .id(("qc-item-row", id))
                        .flex()
                        .items_center()
                        .gap_2()
                        .p_1p5()
                        .rounded_md()
                        .border_1()
                        .border_color(if is_previewed {
                            colors.primary
                        } else {
                            colors.border
                        })
                        .bg(if is_previewed {
                            colors.accent
                        } else {
                            colors.background
                        })
                        .child(
                            div()
                                .id(("qc-thumb", id))
                                .w(px(68.))
                                .h(px(44.))
                                .flex_shrink_0()
                                .rounded_sm()
                                .overflow_hidden()
                                .border_1()
                                .border_color(colors.border)
                                .bg(colors.secondary)
                                .flex()
                                .items_center()
                                .justify_center()
                                .cursor_pointer()
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.open_preview(id, cx);
                                }))
                                .when_some(thumb, |el, image| {
                                    el.child(
                                        gpui::img(image)
                                            .size_full()
                                            .object_fit(gpui::ObjectFit::Contain),
                                    )
                                }),
                        )
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .flex()
                                .flex_col()
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(colors.foreground)
                                        .child(format!("Soal #{:03}", index + 1)),
                                )
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(colors.muted_foreground)
                                        .child(dims),
                                ),
                        )
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap_1()
                                .child(
                                    Button::new(("qc-retake", id))
                                        .ghost()
                                        .xsmall()
                                        .label("Ulang")
                                        .tooltip("Jepret ulang soal ini dari area terkunci")
                                        .disabled(busy)
                                        .on_click(cx.listener(move |this, _, window, cx| {
                                            this.retake_item(index, window, cx);
                                        })),
                                )
                                .child(
                                    Button::new(("qc-up", id))
                                        .ghost()
                                        .xsmall()
                                        .label("↑")
                                        .tooltip("Naikkan urutan")
                                        .disabled(busy || is_first)
                                        .on_click(cx.listener(move |this, _, window, cx| {
                                            this.move_item(index, index.saturating_sub(1), window, cx);
                                        })),
                                )
                                .child(
                                    Button::new(("qc-down", id))
                                        .ghost()
                                        .xsmall()
                                        .label("↓")
                                        .tooltip("Turunkan urutan")
                                        .disabled(busy || is_last)
                                        .on_click(cx.listener(move |this, _, window, cx| {
                                            this.move_item(index, index + 1, window, cx);
                                        })),
                                )
                                .child(
                                    Button::new(("qc-del", id))
                                        .ghost()
                                        .xsmall()
                                        .label("×")
                                        .tooltip("Hapus soal ini")
                                        .disabled(busy)
                                        .on_click(cx.listener(move |this, _, window, cx| {
                                            this.delete_item(index, window, cx);
                                        })),
                                ),
                        ),
                );
            }
        }

        div()
            .flex_1()
            .min_h_0()
            .flex()
            .flex_col()
            .gap_2()
            .when_some(preview_meta, |el, (idx, w, h, img)| {
                el.child(
                    div()
                        .p_2()
                        .rounded_md()
                        .border_1()
                        .border_color(colors.primary)
                        .bg(colors.secondary)
                        .flex()
                        .flex_col()
                        .gap_1p5()
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .justify_between()
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(colors.foreground)
                                        .child(format!("Pratinjau Soal #{:03} ({w}×{h} px)", idx + 1)),
                                )
                                .child(
                                    Button::new("qc-close-preview")
                                        .ghost()
                                        .xsmall()
                                        .label("Tutup")
                                        .on_click(cx.listener(|this, _, _, cx| {
                                            this.preview = None;
                                            cx.notify();
                                        })),
                                ),
                        )
                        .child(
                            div()
                                .w_full()
                                .h(px(156.))
                                .rounded_sm()
                                .overflow_hidden()
                                .bg(colors.background)
                                .flex()
                                .items_center()
                                .justify_center()
                                .child(gpui::img(img).size_full().object_fit(gpui::ObjectFit::Contain)),
                        ),
                )
            })
            .child(list)
    }

    fn render_export_tab(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let colors = cx.theme().colors;
        let count = self.data.items.len();
        let disabled = count == 0 || self.active_export.is_some() || self.capturing;

        div()
            .id("qc-export-scroll")
            .flex_1()
            .min_h_0()
            .overflow_y_scroll()
            .flex()
            .flex_col()
            .gap_2p5()
            .pr_1()
            // Card 1: 1 PDF (Siap Upload AI)
            .child(
                div()
                    .p_2p5()
                    .rounded_md()
                    .border_1()
                    .border_color(colors.border)
                    .bg(colors.secondary)
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .child(
                                div()
                                    .text_sm()
                                    .text_color(colors.foreground)
                                    .child("1. Gabung jadi 1 File PDF"),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(colors.muted_foreground)
                                    .child(format!("{count} halaman")),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .gap_1p5()
                            .child(
                                Button::new("qc-pdf-jpeg")
                                    .xsmall()
                                    .outline()
                                    .selected(self.pdf_preset == PdfExportPreset::AiCompactJpeg85)
                                    .label(PdfExportPreset::AiCompactJpeg85.label())
                                    .disabled(self.active_export.is_some())
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.pdf_preset = PdfExportPreset::AiCompactJpeg85;
                                        cx.notify();
                                    })),
                            )
                            .child(
                                Button::new("qc-pdf-png")
                                    .xsmall()
                                    .outline()
                                    .selected(self.pdf_preset == PdfExportPreset::LosslessPng)
                                    .label(PdfExportPreset::LosslessPng.label())
                                    .disabled(self.active_export.is_some())
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.pdf_preset = PdfExportPreset::LosslessPng;
                                        cx.notify();
                                    })),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .gap_2()
                            .child(
                                Button::new("qc-open-after")
                                    .ghost()
                                    .xsmall()
                                    .label(if self.open_pdf_after {
                                        "[✓] Buka di Origo"
                                    } else {
                                        "[ ] Buka di Origo"
                                    })
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.open_pdf_after = !this.open_pdf_after;
                                        cx.notify();
                                    })),
                            )
                            .child(
                                Button::new("qc-export-pdf")
                                    .primary()
                                    .small()
                                    .label("Simpan 1 PDF…")
                                    .disabled(disabled)
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.start_export_pdf(window, cx);
                                    })),
                            ),
                    ),
            )
            // Card 2: Folder Gambar
            .child(
                div()
                    .p_2p5()
                    .rounded_md()
                    .border_1()
                    .border_color(colors.border)
                    .bg(colors.secondary)
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(
                        div()
                            .text_sm()
                            .text_color(colors.foreground)
                            .child("2. Simpan sebagai Folder Gambar"),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .gap_2()
                            .child(
                                div()
                                    .flex()
                                    .gap_1p5()
                                    .child(
                                        Button::new("qc-img-jpg")
                                            .xsmall()
                                            .outline()
                                            .selected(self.image_format == ImageFolderFormat::Jpeg90)
                                            .label(ImageFolderFormat::Jpeg90.label())
                                            .disabled(self.active_export.is_some())
                                            .on_click(cx.listener(|this, _, _, cx| {
                                                this.image_format = ImageFolderFormat::Jpeg90;
                                                cx.notify();
                                            })),
                                    )
                                    .child(
                                        Button::new("qc-img-png")
                                            .xsmall()
                                            .outline()
                                            .selected(self.image_format == ImageFolderFormat::Png)
                                            .label(ImageFolderFormat::Png.label())
                                            .disabled(self.active_export.is_some())
                                            .on_click(cx.listener(|this, _, _, cx| {
                                                this.image_format = ImageFolderFormat::Png;
                                                cx.notify();
                                            })),
                                    ),
                            )
                            .child(
                                Button::new("qc-export-imgs")
                                    .small()
                                    .label("Simpan Folder…")
                                    .disabled(disabled)
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.start_export_images(window, cx);
                                    })),
                            ),
                    ),
            )
            // Card 3: OCR -> TXT (Bahan AI)
            .child(
                div()
                    .p_2p5()
                    .rounded_md()
                    .border_1()
                    .border_color(colors.border)
                    .bg(colors.secondary)
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .child(
                                div()
                                    .text_sm()
                                    .text_color(colors.foreground)
                                    .child("3. OCR → TXT (Langsung Bahan AI)"),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(colors.muted_foreground)
                                    .child(
                                        "Ekstrak teks semua screenshot secara offline (Tesseract) siap tempel ke ChatGPT / NotebookLM.",
                                    ),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .gap_1()
                            .child(
                                Button::new("qc-ocr-iden")
                                    .xsmall()
                                    .outline()
                                    .selected(self.ocr_language == Language::IndonesianEnglish)
                                    .label("ID + EN")
                                    .disabled(self.active_export.is_some())
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.ocr_language = Language::IndonesianEnglish;
                                        cx.notify();
                                    })),
                            )
                            .child(
                                Button::new("qc-ocr-id")
                                    .xsmall()
                                    .outline()
                                    .selected(self.ocr_language == Language::Indonesian)
                                    .label("ID")
                                    .disabled(self.active_export.is_some())
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.ocr_language = Language::Indonesian;
                                        cx.notify();
                                    })),
                            )
                            .child(
                                Button::new("qc-ocr-en")
                                    .xsmall()
                                    .outline()
                                    .selected(self.ocr_language == Language::English)
                                    .label("EN")
                                    .disabled(self.active_export.is_some())
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.ocr_language = Language::English;
                                        cx.notify();
                                    })),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .gap_2()
                            .child(
                                Button::new("qc-ocr-headers")
                                    .ghost()
                                    .xsmall()
                                    .label(if self.ocr_headers {
                                        "[✓] === Soal N ==="
                                    } else {
                                        "[ ] === Soal N ==="
                                    })
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.ocr_headers = !this.ocr_headers;
                                        cx.notify();
                                    })),
                            )
                            .child(
                                Button::new("qc-export-ocr")
                                    .primary()
                                    .small()
                                    .label("OCR → TXT…")
                                    .disabled(disabled)
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.start_export_ocr_txt(window, cx);
                                    })),
                            ),
                    ),
            )
    }
}

impl Render for QuickCaptureWidget {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let colors = cx.theme().colors;
        let count = self.data.items.len();
        let region_locked = self.data.region.is_some();
        let busy = self.capturing || self.picking_region || self.active_export.is_some();
        let monitors = monitor_count();
        let hotkey_label = self.data.hotkey.label();

        div()
            .id("quick-capture-floating")
            .track_focus(&self.focus)
            .size_full()
            .flex()
            .flex_col()
            .rounded_lg()
            .border_1()
            .border_color(colors.border)
            .bg(colors.background)
            .overflow_hidden()
            // Top Pill Bar (always visible)
            .child(
                div()
                    .h(px(PILL_HEIGHT_COMPACT))
                    .px_2()
                    .flex()
                    .items_center()
                    .gap_1()
                    .border_b_1()
                    .border_color(if self.expanded
                        || self.banner.is_some()
                        || self.pending_duplicate.is_some()
                        || self.active_export.is_some()
                    {
                        colors.border
                    } else {
                        gpui::transparent_black()
                    })
                    // Drag handle
                    .child(
                        div()
                            .id("qc-drag-handle")
                            .px_1p5()
                            .py_1()
                            .rounded_sm()
                            .cursor_grab()
                            .text_xs()
                            .text_color(colors.muted_foreground)
                            .child("⋮⋮")
                            .on_mouse_down(MouseButton::Left, |_, window, _| {
                                window.start_window_move();
                            }),
                    )
                    // Primary Capture Button
                    .child(
                        Button::new("qc-capture-btn")
                            .primary()
                            .small()
                            .label(if self.capturing {
                                "…"
                            } else if region_locked {
                                "Jepret"
                            } else {
                                "Pilih & Jepret"
                            })
                            .tooltip(if region_locked {
                                format!("Jepret area terkunci ({hotkey_label})")
                            } else {
                                "Drag area soal pertama kali".into()
                            })
                            .disabled(busy)
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.trigger_quick_capture(window, cx);
                            })),
                    )
                    // Area Lock / Re-pick Button
                    .child(
                        Button::new("qc-area-btn")
                            .small()
                            .outline()
                            .label(if region_locked { "● Area" } else { "Area" })
                            .tooltip("Pilih ulang & kunci area soal (Alt+Shift+R)")
                            .disabled(busy)
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.start_region_picker(None, window, cx);
                            })),
                    )
                    // Counter Pill -> toggle List drawer
                    .child(
                        Button::new("qc-count-btn")
                            .small()
                            .outline()
                            .selected(self.expanded && self.tab == DrawerTab::List)
                            .label(format!("{count}"))
                            .tooltip("Buka daftar & pratinjau tangkapan")
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.toggle_drawer(Some(DrawerTab::List), window, cx);
                            })),
                    )
                    // Export shortcut button -> toggle Export drawer
                    .child(
                        Button::new("qc-export-tab-btn")
                            .small()
                            .ghost()
                            .selected(self.expanded && self.tab == DrawerTab::Export)
                            .label("Ekspor")
                            .tooltip("Ekspor ke 1 PDF, Folder Gambar, atau OCR TXT")
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.toggle_drawer(Some(DrawerTab::Export), window, cx);
                            })),
                    )
                    .child(div().flex_1())
                    // Restore Origo button
                    .child(
                        Button::new("qc-restore-origo")
                            .ghost()
                            .xsmall()
                            .label("↗")
                            .tooltip("Tampilkan kembali jendela utama Origo")
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.restore_origo(cx);
                            })),
                    )
                    // Close widget button
                    .child(
                        Button::new("qc-close-btn")
                            .ghost()
                            .xsmall()
                            .label("×")
                            .tooltip("Tutup widget Capture & kembali ke Origo")
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.close_widget(window, cx);
                            })),
                    ),
            )
            // Active Export Progress Bar
            .when_some(self.active_export.as_ref().map(ActiveExport::label), |el, label| {
                el.child(
                    div()
                        .px_2p5()
                        .py_1p5()
                        .bg(colors.accent)
                        .border_b_1()
                        .border_color(colors.border)
                        .flex()
                        .items_center()
                        .gap_2()
                        .child(div().flex_1().text_xs().truncate().child(label))
                        .child(
                            Button::new("qc-cancel-export")
                                .ghost()
                                .xsmall()
                                .label("Batal")
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.cancel_export(window, cx);
                                })),
                        ),
                )
            })
            // Status / Duplicate Banner
            .when(self.active_export.is_none(), |el| {
                el.when_some(self.banner.clone(), |el, banner| {
                    let has_dup = self.pending_duplicate.is_some();
                    let text_color = match banner.tone {
                        BannerTone::Error => colors.danger,
                        BannerTone::Warning => gpui::rgb(0xb45309).into(),
                        BannerTone::Success => gpui::rgb(0x15803d).into(),
                        BannerTone::Info => colors.muted_foreground,
                    };
                    el.child(
                        div()
                            .px_2p5()
                            .py_1p5()
                            .bg(colors.secondary)
                            .border_b_1()
                            .border_color(colors.border)
                            .flex()
                            .items_center()
                            .gap_1p5()
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .text_xs()
                                    .text_color(text_color)
                                    .truncate()
                                    .child(banner.text),
                            )
                            .when(has_dup, |row| {
                                row.child(
                                    Button::new("qc-force-dup")
                                        .xsmall()
                                        .outline()
                                        .label("Tetap Simpan")
                                        .on_click(cx.listener(|this, _, window, cx| {
                                            this.force_save_duplicate(window, cx);
                                        })),
                                )
                            })
                            .child(
                                Button::new("qc-dismiss-banner")
                                    .ghost()
                                    .xsmall()
                                    .label("×")
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.clear_banner(window);
                                        cx.notify();
                                    })),
                            ),
                    )
                })
            })
            // Expandable Drawer Body
            .when(self.expanded, |el| {
                el.child(
                    div()
                        .flex_1()
                        .min_h_0()
                        .p_2p5()
                        .flex()
                        .flex_col()
                        .gap_2()
                        // Quick Settings & Utility Strip
                        .child(
                            div()
                                .flex()
                                .flex_wrap()
                                .items_center()
                                .gap_1()
                                .child(
                                    Button::new("qc-hotkey-toggle")
                                        .xsmall()
                                        .outline()
                                        .label(format!("Hotkey: {hotkey_label}"))
                                        .tooltip("Klik untuk ganti hotkey global (F9 ↔ Alt+Shift+C)")
                                        .on_click(cx.listener(|this, _, window, cx| {
                                            this.toggle_hotkey_preset(window, cx);
                                        })),
                                )
                                .when(monitors > 1, |row| {
                                    row.child(
                                        Button::new("qc-monitor-cycle")
                                            .xsmall()
                                            .outline()
                                            .label(format!("Layar {}", self.data.monitor_index))
                                            .tooltip("Ganti monitor target tangkapan")
                                            .on_click(cx.listener(|this, _, window, cx| {
                                                this.cycle_monitor(window, cx);
                                            })),
                                    )
                                })
                                .child(
                                    Button::new("qc-dup-guard")
                                        .xsmall()
                                        .outline()
                                        .selected(self.data.duplicate_guard)
                                        .label("Anti-duplikat")
                                        .tooltip("Cegah jepret ganda bila soal belum berpindah")
                                        .on_click(cx.listener(|this, _, _, cx| {
                                            this.toggle_duplicate_guard(cx);
                                        })),
                                )
                                .child(div().flex_1())
                                .child(
                                    Button::new("qc-undo-btn")
                                        .xsmall()
                                        .ghost()
                                        .label("Undo")
                                        .disabled(count == 0 || busy)
                                        .on_click(cx.listener(|this, _, window, cx| {
                                            this.undo_last(window, cx);
                                        })),
                                )
                                .when(!self.confirm_clear, |row| {
                                    row.child(
                                        Button::new("qc-clear-ask")
                                            .xsmall()
                                            .ghost()
                                            .label("Kosongkan")
                                            .disabled(count == 0 || busy)
                                            .on_click(cx.listener(|this, _, _, cx| {
                                                this.confirm_clear = true;
                                                cx.notify();
                                            })),
                                    )
                                })
                                .when(self.confirm_clear, |row| {
                                    row.child(
                                        Button::new("qc-clear-yes")
                                            .xsmall()
                                            .danger()
                                            .label("Yakin Hapus?")
                                            .on_click(cx.listener(|this, _, window, cx| {
                                                this.clear_all_items(window, cx);
                                            })),
                                    )
                                }),
                        )
                        // Drawer Tabs
                        .child(
                            div()
                                .flex()
                                .gap_1p5()
                                .child(
                                    Button::new("qc-tab-list")
                                        .small()
                                        .outline()
                                        .selected(self.tab == DrawerTab::List)
                                        .label(format!("Daftar ({count})"))
                                        .on_click(cx.listener(|this, _, _, cx| {
                                            this.tab = DrawerTab::List;
                                            this.ensure_thumbs_loaded(cx);
                                            cx.notify();
                                        })),
                                )
                                .child(
                                    Button::new("qc-tab-export")
                                        .small()
                                        .outline()
                                        .selected(self.tab == DrawerTab::Export)
                                        .label("Ekspor (PDF / Gambar / OCR TXT)")
                                        .on_click(cx.listener(|this, _, _, cx| {
                                            this.tab = DrawerTab::Export;
                                            cx.notify();
                                        })),
                                ),
                        )
                        // Locked region summary pill
                        .child(
                            div()
                                .text_xs()
                                .text_color(colors.muted_foreground)
                                .child(match self.data.region {
                                    Some(r) => format!(
                                        "Area terkunci: {}×{} px di ({}, {}) · Layar {} · Area ulang: Alt+Shift+R",
                                        r.width, r.height, r.x, r.y, r.monitor_index
                                    ),
                                    None => {
                                        "Area belum dikunci — klik 'Area' atau tekan Alt+Shift+R untuk drag kotak soal."
                                            .into()
                                    }
                                }),
                        )
                        // Tab Content
                        .when(self.tab == DrawerTab::List, |body| {
                            body.child(self.render_list_tab(cx))
                        })
                        .when(self.tab == DrawerTab::Export, |body| {
                            body.child(self.render_export_tab(cx))
                        }),
                )
            })
    }
}
