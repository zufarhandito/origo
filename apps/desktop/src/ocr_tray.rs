use super::*;
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use std::time::Duration;
use tray_icon::{
    Icon, TrayIcon, TrayIconBuilder,
    menu::{Menu, MenuEvent, MenuItem},
};
use winsafe::{HWND, co};

pub(super) struct OcrTray {
    icon: TrayIcon,
    show: MenuItem,
    pause: MenuItem,
    exit: MenuItem,
    native: HWND,
    maximized: bool,
}

#[derive(Clone, Copy)]
enum TrayAction {
    Show,
    Pause,
    Exit,
}

impl OcrTray {
    fn new(window: &Window) -> anyhow::Result<Self> {
        let raw = HasWindowHandle::window_handle(window)
            .map_err(|_| anyhow::anyhow!("Jendela Windows tidak ditemukan."))?
            .as_raw();
        let RawWindowHandle::Win32(raw) = raw else {
            anyhow::bail!("Jendela Windows tidak ditemukan.");
        };
        let mut native = None;
        winsafe::EnumWindows(|hwnd| {
            if hwnd.ptr() as isize == raw.hwnd.get() {
                native = Some(hwnd);
            }
            true
        })
        .map_err(|_| anyhow::anyhow!("Jendela Windows tidak dapat ditemukan."))?;
        let native = native.ok_or_else(|| anyhow::anyhow!("Jendela Windows tidak ditemukan."))?;
        let maximized = window.is_maximized();

        let menu = Menu::new();
        let show = MenuItem::new("Buka Origo", true, None);
        let pause = MenuItem::new("Jeda OCR", true, None);
        let exit = MenuItem::new("Keluar", true, None);
        menu.append_items(&[&show, &pause, &exit])?;
        let rgba = image::load_from_memory(include_bytes!("../resources/branding/origo-32.png"))?
            .into_rgba8();
        let (width, height) = rgba.dimensions();
        let icon = TrayIconBuilder::new()
            .with_icon(Icon::from_rgba(rgba.into_raw(), width, height)?)
            .with_menu(Box::new(menu))
            .build()?;
        // The builder can return before Explorer registers an icon. A failed
        // update leaves the main window available instead of hiding it.
        icon.set_tooltip(Some("Origo OCR"))?;
        Ok(Self {
            icon,
            show,
            pause,
            exit,
            native,
            maximized,
        })
    }

    fn hide_window(&self) -> bool {
        self.native.ShowWindow(co::SW::HIDE);
        !self.native.IsWindowVisible()
    }

    fn show_window(&self) {
        self.native.ShowWindow(if self.maximized {
            co::SW::SHOWMAXIMIZED
        } else {
            co::SW::SHOW
        });
    }

    fn action(&self) -> Option<TrayAction> {
        MenuEvent::receiver().try_iter().find_map(|event| {
            if event.id() == self.show.id() {
                Some(TrayAction::Show)
            } else if event.id() == self.pause.id() {
                Some(TrayAction::Pause)
            } else if event.id() == self.exit.id() {
                Some(TrayAction::Exit)
            } else {
                None
            }
        })
    }

    fn update(&self, ocr: &ocr::OcrJobs) {
        let progress = ocr.batch_label().unwrap_or_else(|| "Siap".into());
        let tooltip = format!("Origo OCR · {progress}")
            .chars()
            .take(120)
            .collect::<String>();
        let _ = self.icon.set_tooltip(Some(tooltip));
        self.pause.set_text(if ocr.batch_paused() {
            "Lanjutkan OCR"
        } else {
            "Jeda OCR"
        });
        self.pause.set_enabled(ocr.batch_can_toggle());
    }
}

impl WindowShell {
    pub(super) fn sync_ocr_tray(&self, cx: &App) {
        if let Some(tray) = &self.ocr_tray {
            tray.update(self.ocr.read(cx));
        }
    }

    pub(super) fn hide_to_ocr_tray(&mut self, window: &Window, cx: &mut Context<Self>) {
        if self.ocr_tray.is_some() || !self.ocr.read(cx).has_batch() {
            return;
        }
        let tray = match OcrTray::new(window) {
            Ok(tray) => tray,
            Err(_) => {
                self.notice =
                    Some("Ikon tray tidak dapat dibuat. OCR tetap berjalan di jendela ini.".into());
                cx.notify();
                return;
            }
        };
        if !tray.hide_window() {
            self.notice = Some("Origo tidak dapat disembunyikan. OCR tetap berjalan.".into());
            cx.notify();
            return;
        }
        self.ocr_tray = Some(tray);
        self.sync_ocr_tray(cx);
        let handle = self.handle;
        cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(175))
                    .await;
                let keep_polling = handle.update(cx, |_, window, cx| {
                    this.update(cx, |this, cx| {
                        let Some(tray) = &this.ocr_tray else {
                            return false;
                        };
                        match tray.action() {
                            Some(TrayAction::Pause) => {
                                this.ocr.update(cx, |ocr, cx| ocr.toggle_batch(cx));
                                true
                            }
                            Some(action @ (TrayAction::Exit | TrayAction::Show)) => {
                                if let Some(tray) = this.ocr_tray.take() {
                                    tray.show_window();
                                }
                                window.activate_window();
                                this.report(cx);
                                if matches!(action, TrayAction::Exit) {
                                    this.begin_close(true, cx);
                                }
                                cx.notify();
                                false
                            }
                            None => true,
                        }
                    })
                });
                if !matches!(keep_polling, Ok(Ok(true))) {
                    break;
                }
            }
        })
        .detach();
        self.report(cx);
        cx.notify();
    }
}
