//! Local visual signatures. No certificate field or template persistence.
use crate::*;
use gpui::component::{
    ActiveTheme, Disableable, Selectable,
    button::{Button, ButtonVariants},
    input::{Input, InputEvent, InputState},
    popover::Popover,
};
use pdf_domain::{
    PageId,
    annotation::{Annotation, SignatureImage},
};

#[derive(Default, Clone, Copy, PartialEq, Eq)]
pub(super) enum SignatureMethod {
    #[default]
    Draw,
    Image,
    Text,
}
#[derive(Default, Clone, Copy, PartialEq, Eq)]
pub(super) enum SignatureKind {
    #[default]
    Signature,
    Initials,
    Stamp,
}
#[derive(Default)]
pub(super) struct SignatureState {
    pub open: bool,
    pub method: SignatureMethod,
    pub kind: SignatureKind,
    pub text: Option<Entity<InputState>>,
    pub initials: Option<Entity<InputState>>,
    pub style: usize,
    pub color: usize,
    pub remove_white: bool,
    pub thickness: usize,
    pub imported: Option<Arc<SignatureImage>>,
    pub drawing: bool,
    pub strokes: Vec<Vec<[f32; 2]>>,
    pub asset: Option<Arc<SignatureImage>>,
    pub preview: Option<Arc<RenderImage>>,
    pub placing: bool,
    pub error: Option<String>,
    pub gesture_bounds: Option<Bounds<gpui::Pixels>>,
    pub gesture: Option<(PageId, Annotation, [f32; 2], bool)>,
}
impl DocumentSession {
    pub(super) fn signature_button(&self, cx: &mut Context<Self>) -> gpui::AnyElement {
        let owner = cx.entity();
        Popover::new("signature-popup")
            .trigger(
                Button::new("signature")
                    .ghost()
                    .icon(gpui::assets::IconName::Signature)
                    .label("Tanda tangan")
                    .disabled(self.edits_locked()),
            )
            .open(self.annotation.signature.open)
            .on_open_change(cx.listener(|s, open, window, cx| {
                s.init_signature_text(window, cx);
                s.annotation_popup_changed(*open, cx);
                s.annotation.signature.open = *open;
                s.annotation.signature.drawing = false;
                s.annotation.draft = None;
                cx.notify();
            }))
            .content(move |_, window, cx| owner.update(cx, |s, cx| s.signature_panel(window, cx)))
            .into_any_element()
    }
    fn init_signature_text(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.annotation.signature.text.is_some() {
            return;
        }
        let input = cx.new(|cx| InputState::new(window, cx).placeholder("Nama atau inisial"));
        cx.subscribe_in(&input, window, |s, _, event: &InputEvent, _, cx| {
            if matches!(event, InputEvent::Change)
                && s.annotation.signature.method == SignatureMethod::Text
            {
                s.refresh_signature_text(cx);
            }
        })
        .detach();
        self.annotation.signature.text = Some(input);
        let initials = cx.new(|cx| InputState::new(window, cx).placeholder("Inisial"));
        cx.subscribe_in(&initials, window, |s, _, event: &InputEvent, _, cx| {
            if matches!(event, InputEvent::Change)
                && s.annotation.signature.method == SignatureMethod::Text
            {
                s.refresh_signature_text(cx);
            }
        })
        .detach();
        self.annotation.signature.initials = Some(initials);
    }
    fn refresh_signature_text(&mut self, cx: &mut Context<Self>) {
        let state = &mut self.annotation.signature;
        state.asset = None;
        state.preview = None;
        state.error = None;
        let input = if state.kind == SignatureKind::Initials {
            &state.initials
        } else {
            &state.text
        };
        let text = input
            .as_ref()
            .map(|i| i.read(cx).value().to_string())
            .unwrap_or_default();
        if !text.trim().is_empty() {
            match crate::signature_text::render(&text, state.style, signature_color(state.color)) {
                Ok(asset) => {
                    state.preview = asset_preview(&asset);
                    state.asset = Some(Arc::new(asset));
                }
                Err(error) => state.error = Some(error),
            }
        }
        cx.notify();
    }
    fn signature_method(&mut self, mut method: SignatureMethod, cx: &mut Context<Self>) {
        if self.annotation.signature.kind == SignatureKind::Stamp {
            method = SignatureMethod::Image;
        }
        self.annotation.signature.method = method;
        self.annotation.signature.error = None;
        if method == SignatureMethod::Text {
            self.refresh_signature_text(cx);
            return;
        }
        self.annotation.signature.asset = if method == SignatureMethod::Draw {
            draw_asset_with(
                &self.annotation.signature.strokes,
                signature_color(self.annotation.signature.color),
                self.annotation.signature.thickness,
            )
            .map(Arc::new)
        } else {
            self.annotation
                .signature
                .imported
                .as_deref()
                .and_then(|a| image_variant(a, self.annotation.signature.remove_white))
                .map(Arc::new)
        };
        if method == SignatureMethod::Image
            && self.annotation.signature.remove_white
            && self.annotation.signature.asset.is_none()
        {
            self.annotation.signature.error = Some(
                "Tidak ada isi tersisa setelah latar putih dihapus. Kembalikan gambar asli.".into(),
            );
        }
        self.annotation.signature.preview = self
            .annotation
            .signature
            .asset
            .as_deref()
            .and_then(asset_preview);
        cx.notify();
    }
    fn signature_panel(&mut self, window: &mut Window, cx: &mut Context<Self>) -> gpui::AnyElement {
        let weak = cx.weak_entity();
        let strokes = self.annotation.signature.strokes.clone();
        let color = signature_color(self.annotation.signature.color);
        let thickness = [2., 4., 6.][self.annotation.signature.thickness.min(2)];
        let pad = gpui::canvas(
            |_, _, _| {},
            move |bounds, _, window, _| {
                for stroke in &strokes {
                    let mut path = gpui::PathBuilder::stroke(px(thickness));
                    for (i, p) in stroke.iter().enumerate() {
                        let p = bounds.origin
                            + gpui::point(bounds.size.width * p[0], bounds.size.height * p[1]);
                        if i == 0 {
                            path.move_to(p);
                        } else {
                            path.line_to(p);
                        }
                    }
                    if let Ok(path) = path.build() {
                        window.paint_path(
                            path,
                            gpui::rgb(
                                (u32::from(color[0]) << 16)
                                    | (u32::from(color[1]) << 8)
                                    | u32::from(color[2]),
                            ),
                        );
                    }
                }
                let down = weak.clone();
                window.on_mouse_event(move |e: &gpui::MouseDownEvent, phase, _, cx| {
                    if phase == gpui::DispatchPhase::Bubble
                        && e.button == gpui::MouseButton::Left
                        && bounds.contains(&e.position)
                    {
                        let _ = down.update(cx, |s, cx| {
                            if !s.annotation.signature.open {
                                return;
                            }
                            if s.annotation.signature.strokes.len() >= 128 {
                                return;
                            }
                            s.annotation.signature.drawing = true;
                            s.annotation.signature.asset = None;
                            s.annotation.signature.preview = None;
                            s.annotation.signature.error = None;
                            s.annotation
                                .signature
                                .strokes
                                .push(vec![pad_point(bounds, e.position)]);
                            cx.notify();
                        });
                    }
                });
                let movement = weak.clone();
                window.on_mouse_event(move |e: &gpui::MouseMoveEvent, phase, _, cx| {
                    if phase == gpui::DispatchPhase::Bubble
                        && e.pressed_button == Some(gpui::MouseButton::Left)
                    {
                        let _ = movement.update(cx, |s, cx| {
                            if !s.annotation.signature.open || !s.annotation.signature.drawing {
                                return;
                            }
                            if let Some(stroke) = s.annotation.signature.strokes.last_mut()
                                && stroke.len() < 1024
                            {
                                stroke.push(pad_point(bounds, e.position));
                                cx.notify();
                            }
                        });
                    }
                });
                window.on_mouse_event(move |e: &gpui::MouseUpEvent, phase, _, cx| {
                    if phase == gpui::DispatchPhase::Bubble && e.button == gpui::MouseButton::Left {
                        let _ = weak.update(cx, |s, cx| {
                            s.annotation.signature.drawing = false;
                            if let Some(asset) = draw_asset_with(
                                &s.annotation.signature.strokes,
                                signature_color(s.annotation.signature.color),
                                s.annotation.signature.thickness,
                            ) {
                                s.annotation.signature.preview = asset_preview(&asset);
                                s.annotation.signature.asset = Some(Arc::new(asset));
                            }
                            cx.notify();
                        });
                    }
                });
            },
        )
        .w_full()
        .h(px(180.));
        let mut panel = gpui::div()
            .id("signature-editor-scroll")
            .max_h((window.viewport_size().height - px(180.)).max(px(240.)))
            .overflow_y_scroll()
            .w(px(500.))
            .max_w_full()
            .flex()
            .flex_col()
            .gap_3()
            .p_2()
            .child("Buat tanda tangan")
            .child(
                gpui::div().flex().gap_2().children(
                    [
                        (SignatureKind::Signature, "Tanda tangan penuh"),
                        (SignatureKind::Initials, "Inisial"),
                        (SignatureKind::Stamp, "Stempel"),
                    ]
                    .into_iter()
                    .map(|(kind, label)| {
                        Button::new(("signature-kind", kind as usize))
                            .ghost()
                            .label(label)
                            .selected(self.annotation.signature.kind == kind)
                            .on_click(cx.listener(move |s, _, window, cx| {
                                s.annotation.signature.kind = kind;
                                if kind == SignatureKind::Initials {
                                    let name = s
                                        .annotation
                                        .signature
                                        .text
                                        .as_ref()
                                        .map(|i| i.read(cx).value().to_string())
                                        .unwrap_or_default();
                                    if let Some(input) = s.annotation.signature.initials.clone()
                                        && input.read(cx).value().is_empty()
                                    {
                                        let suggestion = name
                                            .split_whitespace()
                                            .filter_map(|w| w.chars().next())
                                            .take(8)
                                            .collect::<String>()
                                            .to_uppercase();
                                        input.update(cx, |i, cx| {
                                            i.set_value(suggestion, window, cx)
                                        });
                                    }
                                }
                                s.signature_method(
                                    if kind == SignatureKind::Stamp {
                                        SignatureMethod::Image
                                    } else {
                                        s.annotation.signature.method
                                    },
                                    cx,
                                );
                            }))
                    }),
                ),
            )
            .child(
                gpui::div()
                    .flex()
                    .gap_2()
                    .child(
                        Button::new("signature-method-text")
                            .ghost()
                            .label("Ketik")
                            .disabled(self.annotation.signature.kind == SignatureKind::Stamp)
                            .selected(self.annotation.signature.method == SignatureMethod::Text)
                            .on_click(cx.listener(|s, _, _, cx| {
                                s.signature_method(SignatureMethod::Text, cx)
                            })),
                    )
                    .child(
                        Button::new("signature-method-draw")
                            .ghost()
                            .label("Gambar")
                            .disabled(self.annotation.signature.kind == SignatureKind::Stamp)
                            .selected(self.annotation.signature.method == SignatureMethod::Draw)
                            .on_click(cx.listener(|s, _, _, cx| {
                                s.signature_method(SignatureMethod::Draw, cx);
                            })),
                    )
                    .child(
                        Button::new("signature-method-image")
                            .ghost()
                            .label("Dari gambar")
                            .selected(self.annotation.signature.method == SignatureMethod::Image)
                            .on_click(cx.listener(|s, _, _, cx| {
                                s.signature_method(SignatureMethod::Image, cx);
                            })),
                    ),
            )
            .child(
                if self.annotation.signature.method == SignatureMethod::Draw {
                    "Gambar tanda tangan dengan mouse di area berikut."
                } else if self.annotation.signature.method == SignatureMethod::Text {
                    "Ketik nama atau inisial dan pilih gaya tulisan."
                } else {
                    "Impor PNG/JPEG. Transparansi PNG dipertahankan."
                },
            )
            .when(
                self.annotation.signature.method == SignatureMethod::Draw,
                |panel| {
                    panel.child(
                        gpui::div()
                            .id("signature-pad")
                            .role(gpui::Role::Group)
                            .aria_label("Area gambar tanda tangan")
                            .w_full()
                            .bg(gpui::rgb(0xffffff))
                            .border_1()
                            .border_color(cx.theme().border)
                            .child(pad),
                    )
                },
            );
        if self.annotation.signature.method == SignatureMethod::Text {
            let input = if self.annotation.signature.kind == SignatureKind::Initials {
                &self.annotation.signature.initials
            } else {
                &self.annotation.signature.text
            };
            if let Some(input) = input {
                panel = panel.child(Input::new(input).aria_label("Nama atau inisial tanda tangan"));
            }
            panel = panel.child(
                gpui::div().flex().gap_2().children(
                    ["Caveat", "Dancing Script"]
                        .into_iter()
                        .enumerate()
                        .map(|(style, name)| {
                            Button::new(("signature-font", style))
                                .ghost()
                                .label(name)
                                .selected(self.annotation.signature.style == style)
                                .on_click(cx.listener(move |s, _, _, cx| {
                                    s.annotation.signature.style = style;
                                    s.refresh_signature_text(cx);
                                }))
                        }),
                ),
            );
            panel = panel.child(
                gpui::div().flex().gap_2().children(
                    ["Hitam", "Biru", "Merah"]
                        .into_iter()
                        .enumerate()
                        .map(|(color, name)| {
                            Button::new(("signature-text-color", color))
                                .ghost()
                                .label(name)
                                .selected(self.annotation.signature.color == color)
                                .on_click(cx.listener(move |s, _, _, cx| {
                                    s.annotation.signature.color = color;
                                    s.refresh_signature_text(cx);
                                }))
                        }),
                ),
            );
        }
        if self.annotation.signature.method == SignatureMethod::Draw {
            panel = panel.child(
                gpui::div().flex().gap_2().children(
                    ["Hitam", "Biru", "Merah"]
                        .into_iter()
                        .enumerate()
                        .map(|(color, label)| {
                            Button::new(("signature-draw-color", color))
                                .ghost()
                                .label(label)
                                .selected(self.annotation.signature.color == color)
                                .on_click(cx.listener(move |s, _, _, cx| {
                                    s.annotation.signature.color = color;
                                    s.signature_method(SignatureMethod::Draw, cx);
                                }))
                        }),
                ),
            );
            panel =
                panel.child(gpui::div().flex().gap_2().children(
                    ["Tipis", "Sedang", "Tebal"].into_iter().enumerate().map(
                        |(thickness, label)| {
                            Button::new(("signature-thickness", thickness))
                                .ghost()
                                .label(label)
                                .selected(self.annotation.signature.thickness == thickness)
                                .on_click(cx.listener(move |s, _, _, cx| {
                                    s.annotation.signature.thickness = thickness;
                                    s.signature_method(SignatureMethod::Draw, cx);
                                }))
                        },
                    ),
                ));
        }
        if self.annotation.signature.method == SignatureMethod::Image {
            panel = panel
                .child(
                    Button::new("signature-remove-white")
                        .ghost()
                        .label("Hapus latar putih")
                        .selected(self.annotation.signature.remove_white)
                        .disabled(self.annotation.signature.imported.is_none())
                        .on_click(cx.listener(|s, _, _, cx| {
                            s.annotation.signature.remove_white =
                                !s.annotation.signature.remove_white;
                            s.signature_method(SignatureMethod::Image, cx);
                        })),
                )
                .child("Penghapusan latar opsional. Klik lagi untuk mengembalikan gambar asli.");
        }
        if let Some(preview) = &self.annotation.signature.preview {
            panel = panel.child(
                gpui::div()
                    .bg(gpui::rgb(0xffffff))
                    .child(gpui::img(preview.clone()).h(px(90.)).max_w_full()),
            );
        }
        if let Some(error) = &self.annotation.signature.error {
            panel = panel.child(error.clone());
        }
        panel.child(gpui::div().flex().gap_2()
            .child(Button::new("signature-undo").ghost().label("Undo goresan").disabled(self.annotation.signature.strokes.is_empty())
                .on_click(cx.listener(|s,_,_,cx|{s.annotation.signature.strokes.pop();s.annotation.signature.asset=None;s.annotation.signature.preview=None;cx.notify();})))
            .child(Button::new("signature-clear").ghost().label("Hapus semua")
                .on_click(cx.listener(|s,_,_,cx|{s.annotation.signature.strokes.clear();s.annotation.signature.asset=None;s.annotation.signature.imported=None;s.annotation.signature.preview=None;s.annotation.signature.error=None;cx.notify();})))
            .child(Button::new("signature-import").ghost().label("Impor gambar")
                .on_click(cx.listener(|s,_,_,cx|s.import_signature(cx)))))
            .child(Button::new("signature-cancel").ghost().label("Batal tanda tangan")
                .on_click(cx.listener(|s,_,_,cx|{s.annotation.signature.open=false;s.annotation.signature.drawing=false;s.annotation_popup_changed(false,cx);cx.notify();})))
            .child(Button::new("signature-use").label("Gunakan tanda tangan")
                .disabled(self.annotation.signature.asset.is_none() && (self.annotation.signature.method!=SignatureMethod::Draw || self.annotation.signature.strokes.iter().all(|s|s.len()<2)))
                .on_click(cx.listener(|s,_,window,cx|{
                    if s.annotation.signature.asset.is_none() && s.annotation.signature.method==SignatureMethod::Draw {s.annotation.signature.asset=draw_asset_with(&s.annotation.signature.strokes,signature_color(s.annotation.signature.color),s.annotation.signature.thickness).map(Arc::new);}
                    if s.annotation.signature.asset.is_some(){
                        s.annotation_popup(false);s.annotation.signature.placing=true;s.annotation.signature.open=false;
                        s.annotation.tool=crate::annotations::Tool::Select;
                        s.status="Klik atau drag di halaman untuk menempatkan tanda tangan. Esc membatalkan.".into();
                        s.focus.focus(window,cx);
                    }
                    cx.notify();
                }))).into_any_element()
    }
    fn import_signature(&mut self, cx: &mut Context<Self>) {
        if self.edits_locked() || self.opening {
            return;
        }
        let picker = cx.prompt_for_paths(gpui::PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some("Impor tanda tangan PNG atau JPEG".into()),
        });
        self.opening = true;
        cx.notify();
        cx.spawn(async move |this, cx| {
            let result = picker.await;
            let path = match result {
                Ok(Ok(Some(paths))) => paths.into_iter().next(),
                _ => None,
            };
            if let Some(path) = path {
                let task = cx
                    .background_executor()
                    .spawn(async move { load_asset(&path) });
                let result = task.await;
                let _ = this.update(cx, |s, cx| {
                    s.opening = false;
                    match result {
                        Ok(asset) => {
                            s.annotation.signature.method = SignatureMethod::Image;
                            s.annotation.signature.preview = asset_preview(&asset);
                            s.annotation.signature.asset = Some(Arc::new(asset));
                            s.annotation.signature.imported = s.annotation.signature.asset.clone();
                            s.annotation.signature.remove_white = false;
                            s.annotation.signature.strokes.clear();
                            s.annotation.signature.error = None;
                        }
                        Err(e) => s.annotation.signature.error = Some(e),
                    }
                    cx.notify();
                });
            } else {
                let _ = this.update(cx, |s, cx| {
                    s.opening = false;
                    cx.notify();
                });
            }
        })
        .detach();
    }
    pub(super) fn signature_pointer(
        &mut self,
        page: PageId,
        p: [f32; 2],
        start: bool,
        cx: &mut Context<Self>,
    ) -> bool {
        if self.annotation.signature.placing {
            let Some(asset) = self.annotation.signature.asset.clone() else {
                return false;
            };
            let Some(w) = self.workspace.as_ref() else {
                return false;
            };
            let Some(index) = w.pages().iter().position(|a| a.id == page) else {
                return false;
            };
            let Ok(g) = w.geometry(index) else {
                return false;
            };
            let (page_width, page_height) = if g.quarter_turns % 2 == 1 {
                (g.height_points, g.width_points)
            } else {
                (g.width_points, g.height_points)
            };
            let ratio = asset.height as f32 / asset.width as f32 * page_width / page_height;
            let width = 0.3_f32.min(0.25 / ratio);
            let height = width * ratio;
            let left = (p[0] - width / 2.).clamp(0., 1. - width);
            let top = (p[1] - height / 2.).clamp(0., 1. - height);
            drop(w);
            if start {
                let id = self
                    .workspace
                    .as_mut()
                    .map(|mut w| w.allocate_annotation_id())
                    .unwrap_or(1);
                let a = Annotation {
                    id,
                    highlight: false,
                    color: [0, 0, 0],
                    width: 1.,
                    points: vec![[left, top], [left + width, top + height]],
                    rectangles: false,
                    signature: Some(asset),
                };
                self.annotation.selected = Some((page, id));
                self.annotation.signature.gesture = Some((page, a, p, false));
                cx.notify();
                self.annotation.signature.placing = false;
            }
            return true;
        }
        if self.annotation.tool != crate::annotations::Tool::Select {
            return false;
        }
        if start {
            let hit = self.workspace.as_ref().and_then(|w| {
                w.annotations(page)
                    .iter()
                    .rev()
                    .find(|a| {
                        a.signature.is_some()
                            && p[0] >= a.points[0][0] - 0.015
                            && p[0] <= a.points[1][0] + 0.015
                            && p[1] >= a.points[0][1] - 0.015
                            && p[1] <= a.points[1][1] + 0.015
                    })
                    .cloned()
            });
            if let Some(a) = hit {
                let resize =
                    (p[0] - a.points[1][0]).abs() < 0.02 && (p[1] - a.points[1][1]).abs() < 0.02;
                self.annotation.selected = Some((page, a.id));
                self.annotation.signature.gesture = Some((page, a, p, resize));
                cx.notify();
                return true;
            }
        } else if let Some((gesture_page, a, previous, resize)) =
            &mut self.annotation.signature.gesture
        {
            if *gesture_page != page {
                return true;
            }
            if *resize {
                let width = a.points[1][0] - a.points[0][0];
                let height = a.points[1][1] - a.points[0][1];
                let ratio = height / width;
                let max = (1. - a.points[0][0]).min((1. - a.points[0][1]) / ratio);
                let width = (p[0] - a.points[0][0]).clamp(0.02_f32.min(max), max);
                a.points[1] = [a.points[0][0] + width, a.points[0][1] + width * ratio];
            } else {
                let dx = (p[0] - previous[0]).clamp(-a.points[0][0], 1. - a.points[1][0]);
                let dy = (p[1] - previous[1]).clamp(-a.points[0][1], 1. - a.points[1][1]);
                for point in &mut a.points {
                    point[0] += dx;
                    point[1] += dy;
                }
            }
            *previous = p;
            cx.notify();
            return true;
        }
        false
    }
}
fn signature_color(index: usize) -> [u8; 3] {
    [[25, 25, 25], [30, 80, 180], [180, 35, 45]][index.min(2)]
}
fn pad_point(bounds: Bounds<gpui::Pixels>, p: gpui::Point<gpui::Pixels>) -> [f32; 2] {
    [
        (f32::from(p.x - bounds.origin.x) / f32::from(bounds.size.width)).clamp(0., 1.),
        (f32::from(p.y - bounds.origin.y) / f32::from(bounds.size.height)).clamp(0., 1.),
    ]
}
#[cfg(test)]
fn draw_asset(strokes: &[Vec<[f32; 2]>]) -> Option<SignatureImage> {
    draw_asset_with(strokes, [25, 25, 25], 0)
}
fn draw_asset_with(
    strokes: &[Vec<[f32; 2]>],
    color: [u8; 3],
    thickness: usize,
) -> Option<SignatureImage> {
    let mut rgba = vec![0; 512 * 256 * 4];
    for stroke in strokes {
        for pair in stroke.windows(2) {
            let (p, q) = (pair[0], pair[1]);
            let dx = (q[0] - p[0]) * 512.;
            let dy = (q[1] - p[1]) * 256.;
            let steps = dx.abs().max(dy.abs()).ceil() as usize;
            for i in 0..=steps {
                let t = i as f32 / steps.max(1) as f32;
                let x = (p[0] * 511. + dx * t) as i32;
                let y = (p[1] * 255. + dy * t) as i32;
                let radius = (thickness.min(2) + 1) as i32;
                for oy in -radius..=radius {
                    for ox in -radius..=radius {
                        let (x, y) = (x + ox, y + oy);
                        if (0..512).contains(&x) && (0..256).contains(&y) {
                            let at = (y as usize * 512 + x as usize) * 4;
                            rgba[at..at + 4].copy_from_slice(&[color[0], color[1], color[2], 255]);
                        }
                    }
                }
            }
        }
    }
    crop_asset(image::RgbaImage::from_raw(512, 256, rgba)?)
}
pub(super) fn crop_asset(image: image::RgbaImage) -> Option<SignatureImage> {
    let mut bounds = [image.width(), image.height(), 0, 0];
    for (x, y, p) in image.enumerate_pixels() {
        if p[3] > 0 {
            bounds[0] = bounds[0].min(x);
            bounds[1] = bounds[1].min(y);
            bounds[2] = bounds[2].max(x + 1);
            bounds[3] = bounds[3].max(y + 1);
        }
    }
    if bounds[0] >= bounds[2] || bounds[1] >= bounds[3] {
        return None;
    }
    let cropped = image::imageops::crop_imm(
        &image,
        bounds[0],
        bounds[1],
        bounds[2] - bounds[0],
        bounds[3] - bounds[1],
    )
    .to_image();
    Some(SignatureImage {
        width: cropped.width(),
        height: cropped.height(),
        rgba: cropped.into_raw(),
    })
}
fn load_asset(path: &std::path::Path) -> Result<SignatureImage, String> {
    use image::ImageDecoder;
    if std::fs::metadata(path)
        .map_err(|_| "Gambar tidak dapat dibaca.")?
        .len()
        > 8 * 1024 * 1024
    {
        return Err("Gambar maksimal 8 MiB.".into());
    }
    let mut reader = image::ImageReader::open(path)
        .map_err(|_| "Gambar tidak dapat dibaca.")?
        .with_guessed_format()
        .map_err(|_| "Format gambar tidak dikenali.")?;
    if !matches!(
        reader.format(),
        Some(image::ImageFormat::Png | image::ImageFormat::Jpeg)
    ) {
        return Err("Gunakan PNG atau JPEG.".into());
    }
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(8192);
    limits.max_image_height = Some(8192);
    limits.max_alloc = Some(64 * 1024 * 1024);
    reader.limits(limits);
    let mut decoder = reader
        .into_decoder()
        .map_err(|_| "Gambar rusak atau terlalu besar.")?;
    if decoder.total_bytes() > 64 * 1024 * 1024 {
        return Err("Gambar memerlukan memori decode lebih dari 64 MiB.".into());
    }
    let orientation = decoder
        .orientation()
        .map_err(|_| "Orientasi gambar tidak dapat dibaca.")?;
    let mut image = image::DynamicImage::from_decoder(decoder)
        .map_err(|_| "Gambar rusak atau terlalu besar.")?;
    image.apply_orientation(orientation);
    let (w, h) = (image.width() as f64, image.height() as f64);
    let scale = (512. * 256. / (w * h))
        .sqrt()
        .min(512. / w)
        .min(512. / h)
        .min(1.);
    let image = if scale < 1. {
        image
            .resize_exact(
                (w * scale).floor().max(1.) as u32,
                (h * scale).floor().max(1.) as u32,
                image::imageops::FilterType::Lanczos3,
            )
            .to_rgba8()
    } else {
        image.to_rgba8()
    };
    crop_asset(image).ok_or_else(|| "Gambar sepenuhnya transparan.".into())
}
fn image_variant(asset: &SignatureImage, remove_white: bool) -> Option<SignatureImage> {
    let mut image = image::RgbaImage::from_raw(asset.width, asset.height, asset.rgba.clone())?;
    if remove_white {
        for p in image.pixels_mut() {
            let low = p[0].min(p[1]).min(p[2]);
            let high = p[0].max(p[1]).max(p[2]);
            if low >= 220 && high - low <= 20 {
                let opacity = 255 - u16::from(low);
                for i in 0..3 {
                    p[i] = ((u16::from(p[i]) - u16::from(low)) * 255)
                        .checked_div(opacity)
                        .unwrap_or(0) as u8;
                }
                p[3] = (u16::from(p[3]) * opacity / 255) as u8;
            }
        }
    }
    crop_asset(image)
}
fn asset_preview(asset: &SignatureImage) -> Option<Arc<RenderImage>> {
    let mut rgba = asset.rgba.clone();
    for p in rgba.as_chunks_mut::<4>().0.iter_mut() {
        p.swap(0, 2);
    }
    let image = image::RgbaImage::from_raw(asset.width, asset.height, rgba)?;
    Some(Arc::new(RenderImage::new(smallvec::smallvec![
        image::Frame::new(image)
    ])))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn stamp_aspect_budget_and_optional_white_removal() -> anyhow::Result<()> {
        let dir = tempfile::tempdir()?;
        let path = dir.path().join("stamp.png");
        let mut image = image::RgbaImage::from_pixel(600, 600, image::Rgba([255; 4]));
        for y in 100..500 {
            for x in 100..500 {
                image.put_pixel(x, y, image::Rgba([20, 60, 160, 255]));
            }
        }
        image.save(&path)?;
        let asset = load_asset(&path).map_err(anyhow::Error::msg)?;
        assert!(asset.valid());
        assert_eq!(asset.width, asset.height);
        assert!(asset.height > 256);
        assert!(asset.width * asset.height <= 512 * 256);
        let original = image_variant(&asset, false).ok_or_else(|| anyhow::anyhow!("original"))?;
        assert_eq!(original, asset);
        let removed = image_variant(&asset, true).ok_or_else(|| anyhow::anyhow!("removed"))?;
        assert!(removed.width < asset.width);
        assert!(removed.valid());
        let white = SignatureImage {
            width: 1,
            height: 1,
            rgba: vec![255; 4],
        };
        assert!(image_variant(&white, true).is_none());
        Ok(())
    }
    #[test]
    fn draw_crop_and_import_are_bounded_and_preserve_alpha() -> anyhow::Result<()> {
        assert!(draw_asset(&[]).is_none());
        let asset = draw_asset(&[vec![[0.2, 0.3], [0.7, 0.6]]])
            .ok_or_else(|| anyhow::anyhow!("empty draw"))?;
        assert!(asset.valid());
        assert!(asset.width < 512 && asset.height < 256);
        let dir = tempfile::tempdir()?;
        let path = dir.path().join("alpha.png");
        let mut image = image::RgbaImage::new(40, 30);
        image.put_pixel(15, 10, image::Rgba([0, 40, 120, 128]));
        image.save(&path)?;
        let imported = load_asset(&path).map_err(anyhow::Error::msg)?;
        assert_eq!((imported.width, imported.height), (1, 1));
        assert_eq!(imported.rgba, [0, 40, 120, 128]);
        let jpeg = dir.path().join("image.jpg");
        image::RgbImage::from_pixel(1000, 400, image::Rgb([255, 255, 255])).save(&jpeg)?;
        let imported = load_asset(&jpeg).map_err(anyhow::Error::msg)?;
        assert!(imported.valid());
        assert_eq!(imported.width, 512);
        std::fs::write(&path, b"invalid image")?;
        assert!(load_asset(&path).is_err());
        image::RgbaImage::new(10, 10).save(&path)?;
        assert!(load_asset(&path).is_err());
        Ok(())
    }
}
