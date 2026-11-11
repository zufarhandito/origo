#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

extern crate gpui_kit as gpui;
use gpui_kit::component::input::{InputEvent, InputState};
use gpui_kit::{
    App, Bounds, Context, Entity, FocusHandle, KeyBinding, ListAlignment, ListState, RenderImage,
    ScrollHandle, Window, WindowBounds, WindowOptions, actions, prelude::*, px, size,
};
use pdf_application::{
    Workspace,
    viewer::{ByteCache, GestureZoom, RenderKey, RenderQueue, RenderTicket, Zoom},
};
use pdf_engine::PdfiumRenderer;
use std::{collections::HashMap, ops::Range, path::PathBuf, sync::Arc};
mod chrome;
mod controller;
use chrome::ViewerMode;
mod navigation;
mod organizer;
mod text_selection;
mod views;
use navigation::{Direction, Navigation};

actions!(
    pdf_studio,
    [
        Open,
        ImportPdf,
        UndoEdit,
        RedoEdit,
        DeletePages,
        RotateRight,
        RotateLeft,
        SelectPages,
        MoveEarlier,
        MoveLater,
        ExtendPrevious,
        ExtendNext,
        PagePrevious,
        PageNext,
        TogglePage,
        ClearPages,
        PageFocusPrevious,
        PageFocusNext,
        FirstPage,
        PreviousPage,
        NextPage,
        LastPage,
        ZoomIn,
        ZoomOut,
        FitWidth,
        FitPage,
        ToggleSidebar,
        FocusPage,
        CopyText,
        ClearTextSelection
    ]
);

#[derive(Default)]
struct RenderStats {
    full: u64,
    navigation_commands: u64,
    thumbnails: u64,
    stale: u64,
    peak_cache_bytes: usize,
    max_bitmap_bytes: usize,
}
struct Viewer {
    mode: ViewerMode,
    page_dragging: bool,
    text: text_selection::TextState,
    page_anchor: Option<pdf_domain::PageId>,
    page_focus: FocusHandle,
    focus: FocusHandle,
    renderer: PdfiumRenderer,
    workspace: Option<Workspace>,
    navigation: Navigation,
    opening: bool,
    error: Option<String>,
    status: String,
    page_list: ListState,
    thumbnail_list: ListState,
    horizontal: ScrollHandle,
    zoom: Zoom,
    gesture_zoom: GestureZoom,
    viewport: (f32, f32),
    dpr: f32,
    sidebar: bool,
    generation: u64,
    source_sequence: u64,
    queue: RenderQueue,
    inflight: Option<RenderTicket>,
    pages: ByteCache<RenderKey, Arc<RenderImage>>,
    thumbnails: ByteCache<RenderKey, Arc<RenderImage>>,
    failures: HashMap<RenderKey, String>,
    gpu_images: HashMap<RenderKey, (Arc<RenderImage>, usize)>,
    thumbnail_visible: Range<usize>,
    window_handle: gpui_kit::AnyWindowHandle,
    jump: Entity<InputState>,
    visible: Range<usize>,
    stats: RenderStats,
    metrics: Option<PathBuf>,
    metrics_sequence: Arc<std::sync::atomic::AtomicU64>,
    metrics_lock: Arc<std::sync::Mutex<()>>,
    gpu_releases: Arc<std::sync::atomic::AtomicU64>,
    gpu_release_errors: Arc<std::sync::atomic::AtomicU64>,
}

fn cache_budget(name: &str, default: usize, min: usize, max: usize) -> anyhow::Result<usize> {
    let value = match std::env::var(name) {
        Ok(value) => value.parse::<usize>()?,
        Err(_) => default,
    };
    anyhow::ensure!((min..=max).contains(&value), "Invalid cache size: {name}");
    Ok(value * 1024 * 1024)
}
fn main() -> anyhow::Result<()> {
    let executable = std::env::current_exe()?;
    let sidecars = executable
        .parent()
        .ok_or_else(|| anyhow::anyhow!("Executable directory missing"))?
        .join("sidecars");
    let mut initial = None;
    let mut metrics = None;
    let mut args = std::env::args_os().skip(1);
    while let Some(arg) = args.next() {
        if arg == "--metrics" {
            metrics =
                Some(PathBuf::from(args.next().ok_or_else(|| {
                    anyhow::anyhow!("--metrics requires a path")
                })?));
        } else if initial.is_none() {
            initial = Some(PathBuf::from(arg));
        } else {
            anyhow::bail!("Usage: pdf-studio [file.pdf] [--metrics diagnostics.json]");
        }
    }
    let page_budget = cache_budget("PDF_STUDIO_PAGE_CACHE_MIB", 128, 128, 512)?;
    let thumbnail_budget = cache_budget("PDF_STUDIO_THUMBNAIL_CACHE_MIB", 16, 16, 64)?;
    gpui_kit::application()
        .with_assets(gpui_kit::assets::AllAssets)
        .run(move |cx: &mut App| {
            gpui_kit::init(cx);
            gpui_kit::component::Theme::change(gpui_kit::component::ThemeMode::Light, None, cx);
            gpui_kit::component::Theme::update(cx, |theme| theme.font_family = "Segoe UI".into());
            cx.bind_keys([
                KeyBinding::new("ctrl-o", Open, None),
                KeyBinding::new("ctrl-shift-o", ImportPdf, Some("PdfViewer")),
                KeyBinding::new("ctrl-z", UndoEdit, Some("PdfViewer")),
                KeyBinding::new("ctrl-y", RedoEdit, Some("PdfViewer")),
                KeyBinding::new("delete", DeletePages, Some("PageOrganizer")),
                KeyBinding::new("ctrl-a", SelectPages, Some("PageOrganizer")),
                KeyBinding::new("alt-up", MoveEarlier, Some("PageOrganizer")),
                KeyBinding::new("alt-down", MoveLater, Some("PageOrganizer")),
                KeyBinding::new("shift-up", ExtendPrevious, Some("PageOrganizer")),
                KeyBinding::new("shift-down", ExtendNext, Some("PageOrganizer")),
                KeyBinding::new("up", PagePrevious, Some("PageOrganizer")),
                KeyBinding::new("down", PageNext, Some("PageOrganizer")),
                KeyBinding::new("space", TogglePage, Some("PageOrganizer")),
                KeyBinding::new("escape", ClearPages, Some("PageOrganizer")),
                KeyBinding::new("ctrl-up", PageFocusPrevious, Some("PageOrganizer")),
                KeyBinding::new("ctrl-down", PageFocusNext, Some("PageOrganizer")),
                KeyBinding::new("ctrl-r", RotateRight, Some("PageOrganizer")),
                KeyBinding::new("ctrl-shift-r", RotateLeft, Some("PageOrganizer")),
                KeyBinding::new("pageup", PreviousPage, None),
                KeyBinding::new("pagedown", NextPage, None),
                KeyBinding::new("left", PreviousPage, None),
                KeyBinding::new("right", NextPage, None),
                KeyBinding::new("ctrl-home", FirstPage, None),
                KeyBinding::new("ctrl-end", LastPage, None),
                KeyBinding::new("ctrl-=", ZoomIn, None),
                KeyBinding::new("ctrl-+", ZoomIn, None),
                KeyBinding::new("ctrl--", ZoomOut, None),
                KeyBinding::new("ctrl-0", FitPage, None),
                KeyBinding::new("ctrl-1", FitWidth, None),
                KeyBinding::new("ctrl-g", FocusPage, None),
                KeyBinding::new("ctrl-c", CopyText, Some("PdfViewer")),
                KeyBinding::new("escape", ClearTextSelection, Some("PdfViewer")),
            ]);
            cx.on_window_closed(|cx, _| {
                if cx.windows().is_empty() {
                    cx.quit();
                }
            })
            .detach();
            let bounds = Bounds::centered(None, size(px(1200.), px(900.)), cx);
            let result = gpui_kit::open_window(
                WindowOptions {
                    titlebar: Some(gpui_kit::TitlebarOptions {
                        title: Some("PDF Studio".into()),
                        ..gpui_kit::component::TitleBar::title_bar_options()
                    }),
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    window_min_size: Some(size(px(860.), px(500.))),
                    ..gpui_kit::component::TitleBar::window_options()
                },
                cx,
                |window, cx| {
                    cx.new(|cx| {
                        let focus = cx.focus_handle();
                        cx.observe_window_activation(window, |this: &mut Viewer, window, _| {
                            if !window.is_window_active() {
                                this.page_dragging = false;
                                this.text.dragging = false;
                                this.text.pending = None;
                                this.text.sentence_anchor = None;
                            }
                        })
                        .detach();
                        focus.focus(window, cx);
                        let jump = cx.new(|cx| InputState::new(window, cx).placeholder("No."));
                        cx.subscribe_in(
                            &jump,
                            window,
                            |this: &mut Viewer, _, event: &InputEvent, window, cx| {
                                if matches!(event, InputEvent::PressEnter { .. }) {
                                    this.jump_to(window, cx);
                                } else if matches!(event, InputEvent::Blur) {
                                    this.defer_page_input(cx);
                                }
                            },
                        )
                        .detach();
                        let page_list = ListState::new(0, ListAlignment::Top, px(100.));
                        let entity = cx.weak_entity();
                        page_list.set_scroll_handler(move |event, _, cx| {
                            let range = event.visible_range.clone();
                            let entity = entity.clone();
                            cx.defer(move |cx| {
                                let _ = entity
                                    .update(cx, |this: &mut Viewer, cx| this.scrolled(range, cx));
                            });
                        });
                        let thumbnail_list = ListState::new(0, ListAlignment::Top, px(50.));
                        let entity = cx.weak_entity();
                        thumbnail_list.set_scroll_handler(move |event, _, cx| {
                            let range = event.visible_range.clone();
                            let entity = entity.clone();
                            cx.defer(move |cx| {
                                let _ = entity.update(cx, |this: &mut Viewer, cx| {
                                    this.thumbnail_visible = range;
                                    this.trim_gpu(cx);
                                });
                            });
                        });
                        let mut viewer = Viewer {
                            mode: ViewerMode::Read,
                            page_dragging: false,
                            text: text_selection::TextState::default(),
                            page_anchor: None,
                            page_focus: cx.focus_handle(),
                            focus,
                            renderer: PdfiumRenderer::new(&sidecars),
                            workspace: None,
                            navigation: Navigation::default(),
                            opening: false,
                            error: None,
                            status: "Siap · Ctrl+O untuk membuka PDF".into(),
                            page_list,
                            thumbnail_list,
                            horizontal: ScrollHandle::new(),
                            zoom: Zoom::default(),
                            gesture_zoom: GestureZoom::default(),
                            viewport: (900., 650.),
                            dpr: window.scale_factor(),
                            sidebar: true,
                            generation: 0,
                            source_sequence: 0,
                            queue: RenderQueue::new(32),
                            inflight: None,
                            pages: ByteCache::new(page_budget),
                            thumbnails: ByteCache::new(thumbnail_budget),
                            failures: HashMap::new(),
                            gpu_images: HashMap::new(),
                            thumbnail_visible: 0..0,
                            window_handle: window.window_handle(),
                            jump,
                            visible: 0..0,
                            stats: RenderStats::default(),
                            metrics,
                            metrics_sequence: Arc::new(std::sync::atomic::AtomicU64::new(0)),
                            metrics_lock: Arc::new(std::sync::Mutex::new(())),
                            gpu_releases: Arc::new(std::sync::atomic::AtomicU64::new(0)),
                            gpu_release_errors: Arc::new(std::sync::atomic::AtomicU64::new(0)),
                        };
                        if let Some(path) = initial {
                            viewer.load(path, cx);
                        }
                        viewer
                    })
                },
            );
            if let Err(error) = result {
                eprintln!("Jendela tidak dapat dibuka: {error}");
                cx.quit();
            }
            cx.activate(true);
        });
    Ok(())
}
