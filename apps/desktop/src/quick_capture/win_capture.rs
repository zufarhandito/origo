//! Safe Windows screen capture (`windows-capture`), Win32 floating-window flags
//! (`winsafe`), global hotkeys, and pure-Rust RGBA crop/hash helpers.
use gpui::{RenderImage, Window};
use std::sync::Arc;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CapturedFrame {
    pub width: u32,
    pub height: u32,
    /// Tightly packed RGBA8 (4 bytes per pixel).
    pub rgba: Vec<u8>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CaptureRegion {
    /// 1-based monitor index (1 = primary / first enumerated display).
    pub monitor_index: usize,
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum HotkeyPreset {
    #[default]
    F9,
    AltShiftC,
}

impl HotkeyPreset {
    pub fn label(self) -> &'static str {
        match self {
            Self::F9 => "F9",
            Self::AltShiftC => "Alt+Shift+C",
        }
    }

    pub fn code(self) -> &'static str {
        match self {
            Self::F9 => "f9",
            Self::AltShiftC => "alt_shift_c",
        }
    }

    pub fn from_code(code: &str) -> Self {
        match code {
            "alt_shift_c" => Self::AltShiftC,
            _ => Self::F9,
        }
    }

    pub fn toggle(self) -> Self {
        match self {
            Self::F9 => Self::AltShiftC,
            Self::AltShiftC => Self::F9,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HotkeyAction {
    Capture,
    PickRegion,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct GlobalHotkeyStatus {
    pub capture_ok: bool,
    pub region_ok: bool,
}

/// Crop an RGBA frame cleanly, clamping coordinates to frame boundaries.
pub fn crop_rgba(frame: &CapturedFrame, region: CaptureRegion) -> anyhow::Result<CapturedFrame> {
    let expected = (frame.width as usize)
        .checked_mul(frame.height as usize)
        .and_then(|px| px.checked_mul(4))
        .ok_or_else(|| anyhow::anyhow!("Ukuran frame tidak valid."))?;
    anyhow::ensure!(
        frame.width > 0 && frame.height > 0 && frame.rgba.len() == expected,
        "Buffer gambar tidak sesuai dimensi."
    );
    let x = region.x.min(frame.width.saturating_sub(1));
    let y = region.y.min(frame.height.saturating_sub(1));
    let max_w = frame.width.saturating_sub(x);
    let max_h = frame.height.saturating_sub(y);
    let w = region.width.min(max_w);
    let h = region.height.min(max_h);
    anyhow::ensure!(w >= 4 && h >= 4, "Area tangkapan terlalu kecil.");

    let src_stride = (frame.width as usize) * 4;
    let dst_stride = (w as usize) * 4;
    let mut out = Vec::with_capacity(dst_stride * (h as usize));
    for row in 0..(h as usize) {
        let src_offset = ((y as usize) + row) * src_stride + (x as usize) * 4;
        let end = src_offset + dst_stride;
        out.extend_from_slice(&frame.rgba[src_offset..end]);
    }
    Ok(CapturedFrame {
        width: w,
        height: h,
        rgba: out,
    })
}

/// Fast 64-bit FNV-1a content fingerprint used for anti-duplicate capture detection.
pub fn fnv1a_rgba_hash(width: u32, height: u32, rgba: &[u8]) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in width.to_le_bytes().into_iter().chain(height.to_le_bytes()) {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x0100_0000_01b3);
    }
    let step = if rgba.len() > 262_144 { 8 } else { 1 };
    let mut i = 0;
    while i < rgba.len() {
        hash ^= u64::from(rgba[i]);
        hash = hash.wrapping_mul(0x0100_0000_01b3);
        i += step;
    }
    hash
}

/// Convert RGBA8 pixel data into a GPUI `RenderImage` (swapping R and B channels to BGRA).
pub fn rgba_to_render_image(width: u32, height: u32, rgba: &[u8]) -> Option<Arc<RenderImage>> {
    if width == 0 || height == 0 || rgba.len() != (width as usize) * (height as usize) * 4 {
        return None;
    }
    let mut bgra = rgba.to_vec();
    for pixel in bgra.as_chunks_mut::<4>().0 {
        pixel.swap(0, 2);
    }
    let img = image::RgbaImage::from_raw(width, height, bgra)?;
    Some(Arc::new(RenderImage::new(smallvec::smallvec![
        image::Frame::new(img)
    ])))
}

#[cfg(windows)]
mod platform {
    use super::*;
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    use std::{
        sync::{
            Mutex,
            atomic::{AtomicBool, Ordering},
            mpsc,
        },
        thread::{self, JoinHandle},
        time::Duration,
    };
    use windows_capture::{
        capture::{Context as CaptureContext, GraphicsCaptureApiHandler},
        frame::Frame,
        graphics_capture_api::InternalCaptureControl,
        monitor::Monitor,
        settings::{
            ColorFormat, CursorCaptureSettings, DirtyRegionSettings, DrawBorderSettings,
            MinimumUpdateIntervalSettings, SecondaryWindowSettings, Settings,
        },
    };
    use winsafe::{HWND, HwndPlace, MSG, POINT, SIZE, co, prelude::Handle};

    const HOTKEY_ID_CAPTURE: i32 = 1;
    const HOTKEY_ID_REGION: i32 = 2;

    struct SingleFrameFlags {
        tx: Mutex<Option<mpsc::Sender<anyhow::Result<CapturedFrame>>>>,
    }

    struct SingleFrameCapture {
        tx: Mutex<Option<mpsc::Sender<anyhow::Result<CapturedFrame>>>>,
        captured: bool,
    }

    impl GraphicsCaptureApiHandler for SingleFrameCapture {
        type Flags = SingleFrameFlags;
        type Error = anyhow::Error;

        fn new(ctx: CaptureContext<Self::Flags>) -> Result<Self, Self::Error> {
            Ok(Self {
                tx: ctx.flags.tx,
                captured: false,
            })
        }

        fn on_frame_arrived(
            &mut self,
            frame: &mut Frame,
            capture_control: InternalCaptureControl,
        ) -> Result<(), Self::Error> {
            if self.captured {
                capture_control.stop();
                return Ok(());
            }
            self.captured = true;
            let width = frame.width();
            let height = frame.height();
            let result = frame
                .buffer()
                .and_then(|mut buf| buf.as_nopadding_buffer().map(|slice| slice.to_vec()))
                .map(|rgba| CapturedFrame {
                    width,
                    height,
                    rgba,
                })
                .map_err(|e| anyhow::anyhow!("Gagal membaca frame layar: {e}"));
            if let Ok(mut guard) = self.tx.lock()
                && let Some(tx) = guard.take()
            {
                let _ = tx.send(result);
            }
            capture_control.stop();
            Ok(())
        }

        fn on_closed(&mut self) -> Result<(), Self::Error> {
            if let Ok(mut guard) = self.tx.lock()
                && let Some(tx) = guard.take()
            {
                let _ = tx.send(Err(anyhow::anyhow!("Sesi tangkap layar ditutup.")));
            }
            Ok(())
        }
    }

    pub fn monitor_count() -> usize {
        Monitor::enumerate()
            .map(|list| list.len().max(1))
            .unwrap_or(1)
    }

    pub fn capture_monitor_rgba(monitor_index: usize) -> anyhow::Result<CapturedFrame> {
        let monitor = Monitor::from_index(monitor_index.max(1))
            .or_else(|_| Monitor::primary())
            .map_err(|e| anyhow::anyhow!("Monitor tidak ditemukan: {e}"))?;
        let (tx, rx) = mpsc::channel();
        let settings = Settings::new(
            monitor,
            CursorCaptureSettings::WithoutCursor,
            DrawBorderSettings::WithoutBorder,
            SecondaryWindowSettings::Default,
            MinimumUpdateIntervalSettings::Default,
            DirtyRegionSettings::Default,
            ColorFormat::Rgba8,
            SingleFrameFlags {
                tx: Mutex::new(Some(tx)),
            },
        );
        let control = SingleFrameCapture::start_free_threaded(settings)
            .map_err(|e| anyhow::anyhow!("Gagal memulai tangkap layar Windows: {e}"))?;
        let frame = rx
            .recv_timeout(Duration::from_secs(5))
            .map_err(|_| anyhow::anyhow!("Waktu tunggu tangkap layar habis."))?;
        let _ = control.stop();
        frame
    }

    fn window_hwnd(window: &Window) -> Option<HWND> {
        let raw = HasWindowHandle::window_handle(window).ok()?.as_raw();
        let RawWindowHandle::Win32(raw) = raw else {
            return None;
        };
        let mut found = None;
        let _ = winsafe::EnumWindows(|hwnd| {
            if hwnd.ptr() as isize == raw.hwnd.get() {
                found = Some(hwnd);
                return false;
            }
            true
        });
        found
    }

    /// Pin the floating widget always-on-top and exclude it from Windows Graphics Capture
    /// so it never shows up in screenshots and never flickers.
    pub fn configure_floating_hwnd(window: &Window) {
        if let Some(hwnd) = window_hwnd(window) {
            let _ = hwnd.SetWindowDisplayAffinity(co::WDA::EXCLUDEFROMCAPTURE);
            let _ = hwnd.SetWindowPos(
                HwndPlace::Place(co::HWND_PLACE::TOPMOST),
                POINT::default(),
                SIZE::default(),
                co::SWP::NOMOVE | co::SWP::NOSIZE | co::SWP::NOACTIVATE,
            );
        }
    }

    /// Pin the fullscreen region-picker overlay on top of browser windows.
    pub fn configure_overlay_hwnd(window: &Window) {
        if let Some(hwnd) = window_hwnd(window) {
            let _ = hwnd.SetWindowPos(
                HwndPlace::Place(co::HWND_PLACE::TOPMOST),
                POINT::default(),
                SIZE::default(),
                co::SWP::NOMOVE | co::SWP::NOSIZE,
            );
        }
    }

    /// Restore a minimized Origo window and bring it to the foreground.
    pub fn restore_window_from_minimized(window: &Window) {
        if let Some(hwnd) = window_hwnd(window) {
            hwnd.ShowWindow(co::SW::RESTORE);
            let _ = hwnd.SetForegroundWindow();
        }
    }

    pub struct GlobalHotkeyListener {
        stop: Arc<AtomicBool>,
        rx: mpsc::Receiver<HotkeyAction>,
        status: GlobalHotkeyStatus,
        thread: Option<JoinHandle<()>>,
    }

    impl GlobalHotkeyListener {
        pub fn start(preset: HotkeyPreset) -> Self {
            let stop = Arc::new(AtomicBool::new(false));
            let stop_flag = stop.clone();
            let (tx, rx) = mpsc::channel();
            let (status_tx, status_rx) = mpsc::channel();

            let handle = thread::spawn(move || {
                let (cap_mod, cap_vk) = match preset {
                    HotkeyPreset::F9 => (co::MOD::NOREPEAT, co::VK::F9),
                    HotkeyPreset::AltShiftC => (
                        co::MOD::ALT | co::MOD::SHIFT | co::MOD::NOREPEAT,
                        co::VK::CHAR_C,
                    ),
                };
                let capture_ok = HWND::NULL
                    .RegisterHotKey(HOTKEY_ID_CAPTURE, cap_mod, cap_vk)
                    .is_ok();
                let region_ok = HWND::NULL
                    .RegisterHotKey(
                        HOTKEY_ID_REGION,
                        co::MOD::ALT | co::MOD::SHIFT | co::MOD::NOREPEAT,
                        co::VK::CHAR_R,
                    )
                    .is_ok();
                let _ = status_tx.send(GlobalHotkeyStatus {
                    capture_ok,
                    region_ok,
                });

                let mut msg = MSG::default();
                while !stop_flag.load(Ordering::Relaxed) {
                    while winsafe::PeekMessage(&mut msg, None, 0, 0, co::PM::REMOVE) {
                        if msg.message == co::WM::HOTKEY {
                            let id = msg.wParam as i32;
                            if id == HOTKEY_ID_CAPTURE {
                                let _ = tx.send(HotkeyAction::Capture);
                            } else if id == HOTKEY_ID_REGION {
                                let _ = tx.send(HotkeyAction::PickRegion);
                            }
                        }
                    }
                    thread::sleep(Duration::from_millis(25));
                }

                if capture_ok {
                    let _ = HWND::NULL.UnregisterHotKey(HOTKEY_ID_CAPTURE);
                }
                if region_ok {
                    let _ = HWND::NULL.UnregisterHotKey(HOTKEY_ID_REGION);
                }
            });

            let status = status_rx
                .recv_timeout(Duration::from_secs(2))
                .unwrap_or_default();

            Self {
                stop,
                rx,
                status,
                thread: Some(handle),
            }
        }

        pub fn status(&self) -> GlobalHotkeyStatus {
            self.status
        }

        pub fn try_recv(&self) -> Option<HotkeyAction> {
            self.rx.try_recv().ok()
        }
    }

    impl Drop for GlobalHotkeyListener {
        fn drop(&mut self) {
            self.stop.store(true, Ordering::Relaxed);
            if let Some(handle) = self.thread.take() {
                let _ = handle.join();
            }
        }
    }
}

#[cfg(not(windows))]
mod platform {
    use super::*;

    pub fn monitor_count() -> usize {
        1
    }

    pub fn capture_monitor_rgba(_monitor_index: usize) -> anyhow::Result<CapturedFrame> {
        anyhow::bail!("Tangkap layar cepat hanya didukung di Windows.")
    }

    pub fn configure_floating_hwnd(_window: &Window) {}
    pub fn configure_overlay_hwnd(_window: &Window) {}
    pub fn restore_window_from_minimized(_window: &Window) {}

    pub struct GlobalHotkeyListener;

    impl GlobalHotkeyListener {
        pub fn start(_preset: HotkeyPreset) -> Self {
            Self
        }

        pub fn status(&self) -> GlobalHotkeyStatus {
            GlobalHotkeyStatus::default()
        }

        pub fn try_recv(&self) -> Option<HotkeyAction> {
            None
        }
    }
}

pub use platform::{
    GlobalHotkeyListener, capture_monitor_rgba, configure_floating_hwnd, configure_overlay_hwnd,
    monitor_count, restore_window_from_minimized,
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crop_rgba_extracts_exact_subregion_and_clamps_bounds() -> anyhow::Result<()> {
        let mut rgba = Vec::with_capacity(4 * 4 * 4);
        for y in 0..4u8 {
            for x in 0..4u8 {
                rgba.extend_from_slice(&[x, y, 100, 255]);
            }
        }
        let frame = CapturedFrame {
            width: 4,
            height: 4,
            rgba,
        };
        let full = crop_rgba(
            &frame,
            CaptureRegion {
                monitor_index: 1,
                x: 0,
                y: 0,
                width: 100,
                height: 100,
            },
        )?;
        assert_eq!(full.width, 4);
        assert_eq!(full.height, 4);
        assert_eq!(full.rgba.len(), 64);

        let mut big_rgba = Vec::with_capacity(10 * 10 * 4);
        for y in 0..10u8 {
            for x in 0..10u8 {
                big_rgba.extend_from_slice(&[x, y, 50, 255]);
            }
        }
        let big = CapturedFrame {
            width: 10,
            height: 10,
            rgba: big_rgba,
        };
        let cropped = crop_rgba(
            &big,
            CaptureRegion {
                monitor_index: 1,
                x: 3,
                y: 2,
                width: 4,
                height: 5,
            },
        )?;
        assert_eq!(cropped.width, 4);
        assert_eq!(cropped.height, 5);
        assert_eq!(&cropped.rgba[0..4], &[3, 2, 50, 255]);
        let last = cropped.rgba.len() - 4;
        assert_eq!(&cropped.rgba[last..], &[6, 6, 50, 255]);
        Ok(())
    }

    #[test]
    fn fnv1a_hash_detects_content_changes() {
        let a = vec![255u8; 16 * 16 * 4];
        let mut b = a.clone();
        b[40] = 0;
        assert_eq!(fnv1a_rgba_hash(16, 16, &a), fnv1a_rgba_hash(16, 16, &a));
        assert_ne!(fnv1a_rgba_hash(16, 16, &a), fnv1a_rgba_hash(16, 16, &b));
    }
}
