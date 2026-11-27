//! Fullscreen freeze-frame region selector overlay for Quick Capture.
use super::win_capture::{
    CaptureRegion, CapturedFrame, configure_overlay_hwnd, crop_rgba, rgba_to_render_image,
};
use gpui::{
    AnyWindowHandle, App, Bounds, Context, FocusHandle, KeyDownEvent, MouseButton, MouseDownEvent,
    MouseMoveEvent, MouseUpEvent, Pixels, Point, Render, RenderImage, Window, WindowBounds,
    WindowDecorations, WindowKind, WindowOptions, div, prelude::*, px, rgba,
};
use std::sync::Arc;

pub type RegionCallback = Box<dyn FnOnce(Option<(CaptureRegion, CapturedFrame)>, &mut App)>;

pub struct RegionPickerOverlay {
    monitor_index: usize,
    frame: CapturedFrame,
    image: Option<Arc<RenderImage>>,
    focus: FocusHandle,
    drag_start: Option<Point<Pixels>>,
    drag_current: Option<Point<Pixels>>,
    on_complete: Option<RegionCallback>,
    completed: bool,
}

impl RegionPickerOverlay {
    pub fn open(
        monitor_index: usize,
        frame: CapturedFrame,
        on_complete: RegionCallback,
        cx: &mut App,
    ) -> anyhow::Result<AnyWindowHandle> {
        let displays = cx.displays();
        let display = displays
            .get(monitor_index.saturating_sub(1))
            .cloned()
            .or_else(|| cx.primary_display());
        let (bounds, display_id) = match display {
            Some(d) => (d.bounds(), Some(d.id())),
            None => (
                Bounds::centered(None, gpui::size(px(1280.), px(720.)), cx),
                None,
            ),
        };
        let image = rgba_to_render_image(frame.width, frame.height, &frame.rgba);
        let (handle, _view) = gpui_kit::open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                titlebar: None,
                focus: true,
                show: true,
                kind: WindowKind::PopUp,
                is_movable: false,
                is_resizable: false,
                is_minimizable: false,
                display_id,
                window_decorations: Some(WindowDecorations::Client),
                ..Default::default()
            },
            cx,
            |window, cx| {
                configure_overlay_hwnd(window);
                window.activate_window();
                cx.new(|cx| {
                    let focus = cx.focus_handle();
                    window.focus(&focus, cx);
                    let weak = cx.weak_entity();
                    window.on_window_should_close(cx, move |_, cx| {
                        let _ = weak.update(cx, |this: &mut Self, cx| {
                            this.finish(None, cx);
                        });
                        true
                    });
                    Self {
                        monitor_index: monitor_index.max(1),
                        frame,
                        image,
                        focus,
                        drag_start: None,
                        drag_current: None,
                        on_complete: Some(on_complete),
                        completed: false,
                    }
                })
            },
        )?;
        Ok(handle)
    }

    fn finish(&mut self, result: Option<(CaptureRegion, CapturedFrame)>, cx: &mut Context<Self>) {
        if self.completed {
            return;
        }
        self.completed = true;
        if let Some(callback) = self.on_complete.take() {
            cx.defer(move |cx| callback(result, cx));
        }
    }

    fn selection_rect(&self) -> Option<(f32, f32, f32, f32)> {
        let start = self.drag_start?;
        let current = self.drag_current?;
        let x0 = f32::from(start.x).min(f32::from(current.x));
        let y0 = f32::from(start.y).min(f32::from(current.y));
        let x1 = f32::from(start.x).max(f32::from(current.x));
        let y1 = f32::from(start.y).max(f32::from(current.y));
        Some((x0, y0, (x1 - x0).max(0.0), (y1 - y0).max(0.0)))
    }

    fn map_to_region(&self, rect: (f32, f32, f32, f32), window: &Window) -> Option<CaptureRegion> {
        let vp = window.viewport_size();
        let vp_w = f32::from(vp.width).max(1.0);
        let vp_h = f32::from(vp.height).max(1.0);
        let (x, y, w, h) = rect;
        if w < 12.0 || h < 12.0 {
            return None;
        }
        let sx = self.frame.width as f32 / vp_w;
        let sy = self.frame.height as f32 / vp_h;
        let px_x = ((x * sx).round() as i64).clamp(0, i64::from(self.frame.width.saturating_sub(1)))
            as u32;
        let px_y = ((y * sy).round() as i64)
            .clamp(0, i64::from(self.frame.height.saturating_sub(1))) as u32;
        let px_w = ((w * sx).round() as u32)
            .max(16)
            .min(self.frame.width.saturating_sub(px_x));
        let px_h = ((h * sy).round() as u32)
            .max(16)
            .min(self.frame.height.saturating_sub(px_y));
        if px_w < 8 || px_h < 8 {
            return None;
        }
        Some(CaptureRegion {
            monitor_index: self.monitor_index,
            x: px_x,
            y: px_y,
            width: px_w,
            height: px_h,
        })
    }

    fn accept_full_screen(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let region = CaptureRegion {
            monitor_index: self.monitor_index,
            x: 0,
            y: 0,
            width: self.frame.width,
            height: self.frame.height,
        };
        let frame = self.frame.clone();
        self.finish(Some((region, frame)), cx);
        window.remove_window();
    }

    fn cancel(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.finish(None, cx);
        window.remove_window();
    }
}

impl Render for RegionPickerOverlay {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let vp = window.viewport_size();
        let vp_w = f32::from(vp.width).max(1.0);
        let vp_h = f32::from(vp.height).max(1.0);
        let selection = self.selection_rect();
        let pixel_badge = selection
            .and_then(|rect| self.map_to_region(rect, window))
            .map(|r| format!("{} × {} px", r.width, r.height));

        let scrim = rgba(0x0000_008c);

        div()
            .id("quick-capture-overlay")
            .track_focus(&self.focus)
            .relative()
            .size_full()
            .cursor_crosshair()
            .overflow_hidden()
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                let key = event.keystroke.key.as_str();
                if key.eq_ignore_ascii_case("escape") {
                    this.cancel(window, cx);
                } else if key.eq_ignore_ascii_case("enter") {
                    this.accept_full_screen(window, cx);
                }
            }))
            .on_mouse_down(
                MouseButton::Right,
                cx.listener(|this, _, window, cx| this.cancel(window, cx)),
            )
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, event: &MouseDownEvent, _, cx| {
                    this.drag_start = Some(event.position);
                    this.drag_current = Some(event.position);
                    cx.notify();
                }),
            )
            .on_mouse_move(cx.listener(|this, event: &MouseMoveEvent, _, cx| {
                if this.drag_start.is_some() {
                    this.drag_current = Some(event.position);
                    cx.notify();
                }
            }))
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|this, event: &MouseUpEvent, window, cx| {
                    if this.drag_start.is_some() {
                        this.drag_current = Some(event.position);
                        if let Some(rect) = this.selection_rect()
                            && let Some(region) = this.map_to_region(rect, window)
                            && let Ok(cropped) = crop_rgba(&this.frame, region)
                        {
                            this.finish(Some((region, cropped)), cx);
                            window.remove_window();
                            return;
                        }
                        this.drag_start = None;
                        this.drag_current = None;
                        cx.notify();
                    }
                }),
            )
            .when_some(self.image.clone(), |el, image| {
                el.child(
                    gpui::img(image)
                        .absolute()
                        .inset_0()
                        .size_full()
                        .object_fit(gpui::ObjectFit::Fill),
                )
            })
            .when(selection.is_none(), |el| {
                el.child(div().absolute().inset_0().bg(scrim))
            })
            .when_some(selection, |el, (x, y, w, h)| {
                let right_x = (x + w).min(vp_w);
                let bottom_y = (y + h).min(vp_h);
                el
                    // Top scrim
                    .child(
                        div()
                            .absolute()
                            .left(px(0.))
                            .top(px(0.))
                            .w(px(vp_w))
                            .h(px(y.max(0.)))
                            .bg(scrim),
                    )
                    // Bottom scrim
                    .child(
                        div()
                            .absolute()
                            .left(px(0.))
                            .top(px(bottom_y))
                            .w(px(vp_w))
                            .h(px((vp_h - bottom_y).max(0.)))
                            .bg(scrim),
                    )
                    // Left scrim
                    .child(
                        div()
                            .absolute()
                            .left(px(0.))
                            .top(px(y.max(0.)))
                            .w(px(x.max(0.)))
                            .h(px(h.max(0.)))
                            .bg(scrim),
                    )
                    // Right scrim
                    .child(
                        div()
                            .absolute()
                            .left(px(right_x))
                            .top(px(y.max(0.)))
                            .w(px((vp_w - right_x).max(0.)))
                            .h(px(h.max(0.)))
                            .bg(scrim),
                    )
                    // Crisp selection border
                    .child(
                        div()
                            .absolute()
                            .left(px(x))
                            .top(px(y))
                            .w(px(w.max(2.)))
                            .h(px(h.max(2.)))
                            .border_2()
                            .border_color(gpui::rgb(0x38bdf8)),
                    )
                    // Pixel dimensions badge
                    .when_some(pixel_badge, |el, badge| {
                        let badge_y = if y > 34.0 { y - 30.0 } else { y + 8.0 };
                        el.child(
                            div()
                                .absolute()
                                .left(px(x.clamp(8.0, (vp_w - 130.0).max(8.0))))
                                .top(px(badge_y))
                                .px_2()
                                .py_1()
                                .rounded_md()
                                .bg(rgba(0x0f17_2ae6))
                                .text_xs()
                                .text_color(gpui::rgb(0xf8fafc))
                                .child(badge),
                        )
                    })
            })
            // Floating instruction bar at top center
            .child(
                div()
                    .absolute()
                    .top(px(18.))
                    .left_0()
                    .right_0()
                    .flex()
                    .justify_center()
                    .child(
                        div()
                            .px_4()
                            .py_2()
                            .rounded_lg()
                            .bg(rgba(0x0f17_2ae6))
                            .border_1()
                            .border_color(rgba(0x38bd_f888))
                            .text_sm()
                            .text_color(gpui::rgb(0xf8fafc))
                            .child(
                                "Drag untuk kunci & jepret area soal · Enter = layar penuh · Esc / Klik kanan = batal",
                            ),
                    ),
            )
    }
}
