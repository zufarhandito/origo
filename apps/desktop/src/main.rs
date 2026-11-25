#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]
#![recursion_limit = "256"]

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
mod annotations;
mod chrome;
mod controller;
mod conversion;
mod diagnostics;
mod document_actions;
mod document_model;
mod exporting;
mod feedback;
mod images_to_pdf;
mod ipc;
mod local_state;
mod ocr;
mod outline;
mod pdf_security;
mod pdf_shell_actions;
mod pdf_stamps;
mod png_export;
mod printing;
mod reading_position;
mod search;
mod shared_workspace;
mod shell;
mod signature_library;
mod signature_phone;
mod signature_phone_ui;
mod signature_strokes;
mod signature_templates_ui;
mod signature_text;
mod signatures;
mod splitting;
mod transitions;
use chrome::ViewerMode;
mod navigation;
mod organizer;
mod text_annotations;
mod text_selection;
mod views;
use navigation::{Direction, Navigation};

actions!(
    pdf_studio,
    [
        DeleteAnnotation,
        CancelSignatureRename,
        DuplicateSignature,
        SignatureLeft,
        SignatureRight,
        SignatureUp,
        SignatureDown,
        Open,
        CloseTab,
        NextTab,
        PreviousTab,
        ReopenTab,
        SaveAs,
        PrintPdf,
        ExtractPages,
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
        FindText,
        FindNext,
        FindPrevious,
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
struct DocumentSession {
    annotation: annotations::AnnotationState,
    tab_id: shell::TabId,
    view_id: shell::ViewId,
    document_owner: Option<gpui::WeakEntity<DocumentSession>>,
    observed_model: shared_workspace::ModelSignature,
    current_page_id: Option<pdf_domain::PageId>,
    owner: gpui::WeakEntity<shell::WindowShell>,
    resources: Arc<shell::WindowResources>,
    active: bool,
    visible_for_render: bool,
    split_presentation: bool,
    text_budget: usize,
    close_approved: bool,
    split_dialog: Option<splitting::SplitDialog>,
    png_dialog: Option<png_export::PngDialog>,
    print_dialog: Option<printing::PrintDialog>,
    page_numbers_dialog: Option<pdf_stamps::PageNumbersDialog>,
    watermark_dialog: Option<pdf_stamps::WatermarkDialog>,
    transition: transitions::Transition,
    error_popup: bool,
    export: exporting::ExportState,
    exporter: qpdf_adapter::QpdfExporter,
    workspace_instance: u64,
    mode: ViewerMode,
    page_dragging: bool,
    text: text_selection::TextState,
    ocr: Entity<ocr::OcrJobs>,
    ocr_revision: u64,
    ocr_resume: bool,
    ocr_indices: (u64, HashMap<pdf_ocr::PageKey, Vec<usize>>),
    search: search::SearchState,
    search_input: Entity<InputState>,
    page_anchor: Option<pdf_domain::PageId>,
    page_focus: FocusHandle,
    focus: FocusHandle,
    renderer: PdfiumRenderer,
    workspace: shared_workspace::SharedWorkspace,
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
    pending_reading: Option<reading_position::ReadingPosition>,
    dpr: f32,
    sidebar: bool,
    outline: outline::OutlineState,
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
#[cfg(windows)]
mod updates;

struct CliOptions {
    initial: ipc::LaunchIntent,
    metrics: Option<PathBuf>,
    state_dir: Option<PathBuf>,
}

fn parse_cli_args(
    args: impl IntoIterator<Item = std::ffi::OsString>,
) -> anyhow::Result<CliOptions> {
    enum ManifestKind {
        ImagesDialog,
        ImagesQuick,
        PdfOpen,
        PdfCompress,
        PdfSplit,
        PdfPng,
        PdfOcr,
        PdfCombineOrganize,
        PdfCombineQuick,
    }

    let mut initial_pdf: Option<PathBuf> = None;
    let mut images_dialog: Vec<PathBuf> = Vec::new();
    let mut images_quick: Vec<PathBuf> = Vec::new();
    let mut manifest: Option<(ManifestKind, PathBuf)> = None;
    let mut delete_list = false;
    let mut metrics = None;
    let mut state_dir = None;
    let mut collect_mode: Option<bool> = None; // Some(false) = dialog, Some(true) = quick
    let mut iter = args.into_iter();
    while let Some(arg) = iter.next() {
        if arg == "--metrics" {
            collect_mode = None;
            metrics =
                Some(PathBuf::from(iter.next().ok_or_else(|| {
                    anyhow::anyhow!("--metrics requires a path")
                })?));
        } else if arg == "--state-dir" {
            collect_mode = None;
            let path = PathBuf::from(
                iter.next()
                    .ok_or_else(|| anyhow::anyhow!("--state-dir requires a path"))?,
            );
            anyhow::ensure!(path.is_absolute(), "--state-dir requires an absolute path");
            state_dir = Some(path);
        } else if arg == "--images-to-pdf" {
            collect_mode = Some(false);
        } else if arg == "--images-to-pdf-quick" {
            collect_mode = Some(true);
        } else if let Some((kind, flag_name)) = match arg.to_str() {
            Some("--images-to-pdf-list") => {
                Some((ManifestKind::ImagesDialog, "--images-to-pdf-list"))
            }
            Some("--images-to-pdf-quick-list") => {
                Some((ManifestKind::ImagesQuick, "--images-to-pdf-quick-list"))
            }
            Some("--open-pdf-list") => Some((ManifestKind::PdfOpen, "--open-pdf-list")),
            Some("--pdf-compress-list") => Some((ManifestKind::PdfCompress, "--pdf-compress-list")),
            Some("--pdf-split-list") => Some((ManifestKind::PdfSplit, "--pdf-split-list")),
            Some("--pdf-png-list") => Some((ManifestKind::PdfPng, "--pdf-png-list")),
            Some("--pdf-ocr-list") => Some((ManifestKind::PdfOcr, "--pdf-ocr-list")),
            Some("--merge-pdf-list") => Some((ManifestKind::PdfCombineOrganize, "--merge-pdf-list")),
            Some("--merge-pdf-quick-list") => {
                Some((ManifestKind::PdfCombineQuick, "--merge-pdf-quick-list"))
            }
            _ => None,
        } {
            collect_mode = None;
            let list_path = PathBuf::from(
                iter.next()
                    .ok_or_else(|| anyhow::anyhow!("{flag_name} requires a path"))?,
            );
            manifest = Some((kind, list_path));
        } else if arg == "--delete-list" {
            collect_mode = None;
            delete_list = true;
        } else if let Some(quick) = collect_mode {
            if quick {
                images_quick.push(PathBuf::from(arg));
            } else {
                images_dialog.push(PathBuf::from(arg));
            }
        } else if initial_pdf.is_none() {
            initial_pdf = Some(PathBuf::from(arg));
        } else {
            anyhow::bail!(
                "Usage: pdf-studio [file.pdf] [--images-to-pdf <img...>] [--images-to-pdf-quick <img...>] [--images-to-pdf-list <file>] [--images-to-pdf-quick-list <file>] [--open-pdf-list <file>] [--pdf-compress-list <file>] [--pdf-split-list <file>] [--pdf-png-list <file>] [--pdf-ocr-list <file>] [--merge-pdf-list <file>] [--merge-pdf-quick-list <file>] [--delete-list] [--metrics diagnostics.json] [--state-dir directory]"
            );
        }
    }
    let mut initial = ipc::LaunchIntent::None;
    if let Some((kind, list_path)) = manifest {
        let paths = ipc::parse_manifest_file(&list_path, delete_list)?;
        match kind {
            ManifestKind::ImagesDialog => images_dialog.extend(paths),
            ManifestKind::ImagesQuick => images_quick.extend(paths),
            ManifestKind::PdfOpen => {
                if let Some(path) = paths.into_iter().next() {
                    initial = ipc::LaunchIntent::OpenPdf(path);
                }
            }
            ManifestKind::PdfCompress => {
                if let Some(path) = paths.into_iter().next() {
                    initial = ipc::LaunchIntent::PdfAction {
                        path,
                        action: ipc::PdfShellAction::Compress,
                    };
                }
            }
            ManifestKind::PdfSplit => {
                if let Some(path) = paths.into_iter().next() {
                    initial = ipc::LaunchIntent::PdfAction {
                        path,
                        action: ipc::PdfShellAction::Split,
                    };
                }
            }
            ManifestKind::PdfPng => {
                if let Some(path) = paths.into_iter().next() {
                    initial = ipc::LaunchIntent::PdfAction {
                        path,
                        action: ipc::PdfShellAction::Png,
                    };
                }
            }
            ManifestKind::PdfOcr => {
                if let Some(path) = paths.into_iter().next() {
                    initial = ipc::LaunchIntent::PdfAction {
                        path,
                        action: ipc::PdfShellAction::Ocr,
                    };
                }
            }
            ManifestKind::PdfCombineOrganize if !paths.is_empty() => {
                initial = ipc::LaunchIntent::PdfCombineOrganize(paths);
            }
            ManifestKind::PdfCombineQuick if !paths.is_empty() => {
                initial = ipc::LaunchIntent::PdfCombineQuick(paths);
            }
            _ => {}
        }
    }
    if !images_quick.is_empty() {
        initial = ipc::LaunchIntent::ImagesToPdfQuick(images_quick);
    } else if !images_dialog.is_empty() {
        initial = ipc::LaunchIntent::ImagesToPdfDialog(images_dialog);
    } else if initial.is_none()
        && let Some(path) = initial_pdf
    {
        initial = ipc::LaunchIntent::OpenPdf(path);
    }
    Ok(CliOptions {
        initial,
        metrics,
        state_dir,
    })
}

fn main() -> anyhow::Result<()> {
    let executable = std::env::current_exe()?;
    let cli = parse_cli_args(std::env::args_os().skip(1))?;
    let initial = cli.initial;
    let metrics = cli.metrics;
    let mut state_dir = cli.state_dir;
    #[cfg(windows)]
    let installed = match updates::start(&executable)? {
        updates::Startup::Unmanaged => None,
        updates::Startup::Installed(app) => Some(app),
        updates::Startup::AlreadyRunning {
            state_dir: installed_state,
        } => {
            if let Some(dir) = state_dir
                .or(installed_state)
                .or_else(local_state::directory)
                && (metrics.is_none() || !initial.has_document_open())
            {
                let _ = ipc::try_forward_to_running(&dir, &initial);
            }
            return Ok(());
        }
        updates::Startup::Exit => return Ok(()),
    };
    let sidecars = executable
        .parent()
        .ok_or_else(|| anyhow::anyhow!("Executable directory missing"))?
        .join("sidecars");
    let version = env!("CARGO_PKG_VERSION").to_owned();
    #[cfg(windows)]
    let version = if let Some(installed) = &installed {
        if state_dir.is_none() {
            state_dir = installed.state_dir.clone();
        }
        installed.version()
    } else {
        version
    };
    let effective_state_dir = state_dir.or_else(local_state::directory);
    if !initial.is_none()
        && let Some(dir) = effective_state_dir.as_deref()
        && (metrics.is_none() || !initial.has_document_open())
        && ipc::try_forward_to_running(dir, &initial)
    {
        return Ok(());
    }
    let (ipc_guard, ipc_rx) = match effective_state_dir.as_deref().and_then(ipc::start_listener) {
        Some((guard, rx)) => (Some(guard), Some(rx)),
        None => (None, None),
    };
    #[cfg(windows)]
    let updater = installed.as_ref().map(updates::Installed::updater);
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
                KeyBinding::new("ctrl-p", PrintPdf, Some("PdfViewer")),
                KeyBinding::new("ctrl-f", FindText, Some("PdfViewer")),
                KeyBinding::new("f3", FindNext, Some("PdfViewer")),
                KeyBinding::new("shift-f3", FindPrevious, Some("PdfViewer")),
                KeyBinding::new("ctrl-w", CloseTab, None),
                KeyBinding::new("ctrl-tab", NextTab, None),
                KeyBinding::new("ctrl-shift-tab", PreviousTab, None),
                KeyBinding::new("ctrl-shift-t", ReopenTab, None),
                KeyBinding::new("ctrl-shift-s", SaveAs, Some("PdfViewer")),
                KeyBinding::new("ctrl-shift-o", ImportPdf, Some("PdfViewer")),
                KeyBinding::new("ctrl-z", UndoEdit, Some("PdfViewer")),
                KeyBinding::new("ctrl-y", RedoEdit, Some("PdfViewer")),
                KeyBinding::new("delete", DeleteAnnotation, Some("PdfViewer")),
                KeyBinding::new(
                    "escape",
                    CancelSignatureRename,
                    Some("SignatureTemplateRename"),
                ),
                KeyBinding::new("ctrl-d", DuplicateSignature, Some("PdfViewer")),
                KeyBinding::new("alt-left", SignatureLeft, Some("PdfViewer")),
                KeyBinding::new("alt-right", SignatureRight, Some("PdfViewer")),
                KeyBinding::new("alt-up", SignatureUp, Some("PdfViewer")),
                KeyBinding::new("alt-down", SignatureDown, Some("PdfViewer")),
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
                        title: Some("Origo".into()),
                        ..gpui_kit::component::TitleBar::title_bar_options()
                    }),
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    window_min_size: Some(size(px(860.), px(500.))),
                    ..gpui_kit::component::TitleBar::window_options()
                },
                cx,
                |window, cx| {
                    shell::WindowShell::new(
                        window,
                        cx,
                        shell::ShellOptions {
                            sidecars: sidecars.clone(),
                            page_budget,
                            thumbnail_budget,
                            version: version.clone(),
                            metrics: metrics.clone(),
                            state_dir: effective_state_dir.clone(),
                            initial: initial.clone(),
                            ipc_guard,
                            ipc_rx,
                        },
                    )
                },
            );
            #[cfg(windows)]
            if let (Ok((window, shell)), Some(updater)) = (&result, updater) {
                let updater = updater(*window, cx);
                shell.update(cx, |shell, cx| shell.attach_updater(updater, cx));
            }
            if let Err(error) = result {
                eprintln!("Jendela tidak dapat dibuka: {error}");
                cx.quit();
            }
            cx.activate(true);
        });
    Ok(())
}

impl DocumentSession {
    #[allow(clippy::too_many_arguments)]
    fn new(
        window: &mut Window,
        cx: &mut App,
        sidecars: &std::path::Path,
        renderer: PdfiumRenderer,
        page_budget: usize,
        thumbnail_budget: usize,
        metrics: Option<PathBuf>,
        owner: gpui::WeakEntity<shell::WindowShell>,
        resources: Arc<shell::WindowResources>,
        ocr: Entity<ocr::OcrJobs>,
        tab_id: shell::TabId,
        view_id: shell::ViewId,
    ) -> Entity<Self> {
        cx.new(|cx| {
            let focus = cx.focus_handle();
            cx.observe(&ocr, |this: &mut DocumentSession, _, cx| {
                if this.visible_for_render {
                    this.report(cx);
                    cx.notify();
                }
            })
            .detach();
            cx.subscribe(&ocr, |this: &mut DocumentSession, _, change, cx| {
                this.ocr_changed(change, cx);
            })
            .detach();
            let focus_owner = owner.clone();
            cx.on_focus_in(
                &focus,
                window,
                move |this: &mut DocumentSession, window, cx| {
                    let owner = focus_owner.clone();
                    let id = this.view_id;
                    let view = cx.weak_entity();
                    let handle = window.window_handle();
                    cx.defer(move |cx| {
                        let _ = handle.update(cx, |_, window, cx| {
                            if view
                                .upgrade()
                                .is_some_and(|v| v.read(cx).focus.contains_focused(window, cx))
                            {
                                let _ = owner.update(cx, |shell, cx| {
                                    shell.focus_visible_view(id, cx);
                                });
                            }
                        });
                    });
                },
            )
            .detach();
            cx.observe_window_activation(window, |this: &mut DocumentSession, window, _| {
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
            let search_input =
                cx.new(|cx| InputState::new(window, cx).placeholder("Cari dalam PDF…"));
            cx.subscribe_in(
                &search_input,
                window,
                |this: &mut DocumentSession, _, event: &InputEvent, _, cx| match event {
                    InputEvent::Change => this.restart_search(cx),
                    InputEvent::PressEnter { shift, .. } => this.move_search(*shift, cx),
                    _ => {}
                },
            )
            .detach();
            cx.subscribe_in(
                &jump,
                window,
                |this: &mut DocumentSession, _, event: &InputEvent, window, cx| {
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
                    let _ = entity.update(cx, |this: &mut DocumentSession, cx| {
                        this.scrolled(range, cx)
                    });
                });
            });
            let thumbnail_list = ListState::new(0, ListAlignment::Top, px(50.));
            let entity = cx.weak_entity();
            thumbnail_list.set_scroll_handler(move |event, _, cx| {
                let range = event.visible_range.clone();
                let entity = entity.clone();
                cx.defer(move |cx| {
                    let _ = entity.update(cx, |this: &mut DocumentSession, cx| {
                        this.thumbnail_visible = range;
                        this.trim_gpu(cx);
                    });
                });
            });
            DocumentSession {
                tab_id,
                view_id,
                document_owner: None,
                observed_model: Default::default(),
                current_page_id: None,
                owner,
                resources: resources.clone(),
                active: false,
                visible_for_render: false,
                split_presentation: false,
                text_budget: 16 * 1024 * 1024,
                close_approved: false,
                split_dialog: None,
                png_dialog: None,
                print_dialog: None,
                page_numbers_dialog: None,
                watermark_dialog: None,
                transition: transitions::Transition::default(),
                error_popup: false,
                export: exporting::ExportState::default(),
                exporter: qpdf_adapter::QpdfExporter::new(sidecars),
                workspace_instance: 0,
                mode: ViewerMode::Read,
                page_dragging: false,
                annotation: annotations::AnnotationState::default(),
                text: text_selection::TextState::default(),
                ocr,
                ocr_revision: 0,
                ocr_resume: false,
                ocr_indices: Default::default(),
                search: search::SearchState::default(),
                search_input,
                page_anchor: None,
                page_focus: cx.focus_handle(),
                focus,
                renderer,
                workspace: Default::default(),
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
                pending_reading: None,
                dpr: window.scale_factor(),
                sidebar: true,
                outline: outline::OutlineState::default(),
                generation: 0,
                source_sequence: tab_id.0 << 32,
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
                gpu_releases: resources.gpu_releases.clone(),
                gpu_release_errors: resources.gpu_release_errors.clone(),
            }
        })
    }
}
