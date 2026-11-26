//! Quick Capture Floating Widget ("Capture Soal Cepat") integration with `WindowShell`.
mod overlay;
mod store;
mod widget;
mod win_capture;

use crate::shell::WindowShell;
use gpui::{Context, Window};
use widget::{QuickCaptureContext, QuickCaptureWidget};

impl WindowShell {
    pub(super) fn open_quick_capture(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(existing) = self.quick_capture_window {
            let activated = existing.update(cx, |_, widget_win, _| {
                widget_win.activate_window();
            });
            if activated.is_ok() {
                window.minimize_window();
                return;
            }
            self.quick_capture_window = None;
        }

        let ctx = QuickCaptureContext {
            main_shell: cx.weak_entity(),
            main_window: self.handle,
            state_dir: self.state_dir.clone(),
            sidecars: self.sidecars.clone(),
            renderer: self.renderer.clone(),
        };

        match QuickCaptureWidget::open(ctx, cx) {
            Ok((handle, _entity)) => {
                self.quick_capture_window = Some(handle);
                // Minimize main Origo window so browser / tryout screen is unobstructed.
                window.minimize_window();
                cx.notify();
            }
            Err(err) => {
                self.notice = Some(format!("Widget Capture Soal Cepat gagal dibuka: {err}"));
                cx.notify();
            }
        }
    }

    pub(super) fn clear_quick_capture_window(&mut self, cx: &mut Context<Self>) {
        self.quick_capture_window = None;
        cx.notify();
    }

    pub(super) fn close_quick_capture_window(&mut self, cx: &mut Context<Self>) {
        if let Some(handle) = self.quick_capture_window.take() {
            cx.defer(move |cx| {
                let _ = handle.update(cx, |_, widget_win, _| {
                    widget_win.remove_window();
                });
            });
        }
    }
}
