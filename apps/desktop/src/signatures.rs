//! Local visual signatures with opt-in templates; no certificate fields.
use crate::signature_templates_ui::GalleryView;
use crate::*;
use gpui::StyledImage;
use gpui::assets::IconName;
use gpui::component::Icon;
use gpui::component::{
    ActiveTheme, Disableable, Selectable, Sizable, StyledExt,
    button::{Button, ButtonVariants},
    checkbox::Checkbox,
    input::{Input, InputEvent, InputState},
    menu::{DropdownMenu, PopupMenuItem},
    popover::Popover,
};
use pdf_domain::{
    PageGeometry, PageId,
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
    pub phone: Option<crate::signature_phone::PhoneSession>,
    pub phone_qr: Option<Arc<RenderImage>>,
    pub phone_revision: u64,
    pub phone_status: Option<String>,
    pub method: SignatureMethod,
    pub kind: SignatureKind,
    pub gallery: crate::signature_templates_ui::GalleryState,
    pub editor_revision: u64,
    save_template: bool,
    inactive_drafts: [SignatureCategoryDraft; 3],
    stamp_return_method: SignatureMethod,
    font_samples: [Option<Arc<RenderImage>>; 2],
    pub library_label: Option<Entity<InputState>>,
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
    pub placement_ghost: Option<[Option<Arc<RenderImage>>; 4]>,
    pub ghost_pointer_exited: bool,
    pub signature_ghost_rect: Option<[[f32; 2]; 2]>,

    pub error: Option<String>,
    pub gesture_bounds: Option<Bounds<gpui::Pixels>>,
    pub gesture: Option<(PageId, Annotation, [f32; 2], bool)>,
}
#[derive(Default)]
struct SignatureCategoryDraft {
    strokes: Vec<Vec<[f32; 2]>>,
    imported: Option<Arc<SignatureImage>>,
    remove_white: bool,
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
                    .disabled(self.edits_locked() || self.opening),
            )
            .open(self.annotation.signature.open)
            .on_open_change(cx.listener(|s, open, window, cx| {
                s.annotation_popup_changed(*open, cx);
                s.annotation.signature.open = *open;
                s.annotation.signature.editor_revision += 1;
                s.annotation.signature.drawing = false;
                s.annotation.draft = None;
                if *open {
                    s.annotation.text.open = false;
                    s.annotation.text.editing = None;
                    s.annotation.apply_pages.open = false;
                    s.cancel_signature_placement(cx);
                    s.annotation.signature.save_template = false;
                    s.init_signature_text(window, cx);
                    s.load_signature_library(cx);
                    s.refresh_signature_asset(cx);
                } else {
                    s.cancel_signature_phone();
                    s.annotation.signature.gallery.reset_selection();
                }
                cx.notify();
            }))
            .content(move |_, window, cx| owner.update(cx, |s, cx| s.signature_panel(window, cx)))
            .into_any_element()
    }
    fn init_signature_text(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.annotation.signature.text.is_some() {
            return;
        }
        self.annotation.signature.library_label =
            Some(cx.new(|cx| InputState::new(window, cx).placeholder("Nama template (opsional)")));
        let input = cx.new(|cx| InputState::new(window, cx).placeholder("Ketik nama lengkap"));
        cx.subscribe_in(&input, window, |s, _, event: &InputEvent, _, cx| {
            if matches!(event, InputEvent::Change)
                && s.annotation.signature.method == SignatureMethod::Text
            {
                s.refresh_signature_text(cx);
            }
        })
        .detach();
        self.annotation.signature.text = Some(input);
        let initials = cx.new(|cx| InputState::new(window, cx).placeholder("Ketik inisial"));
        cx.subscribe_in(&initials, window, |s, _, event: &InputEvent, _, cx| {
            if matches!(event, InputEvent::Change)
                && s.annotation.signature.method == SignatureMethod::Text
            {
                s.refresh_signature_text(cx);
            }
        })
        .detach();
        self.annotation.signature.initials = Some(initials);
        self.annotation.signature.font_samples = std::array::from_fn(|style| {
            crate::signature_text::render(["Caveat", "Dancing Script"][style], style, [25; 3])
                .ok()
                .as_ref()
                .and_then(asset_preview)
        });
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
    fn signature_method(&mut self, method: SignatureMethod, cx: &mut Context<Self>) {
        if self.opening {
            return;
        }
        self.cancel_signature_phone();
        let state = &mut self.annotation.signature;
        state.drawing = false;
        state.phone_status = None;
        state.method = method;
        self.refresh_signature_asset(cx);
    }

    fn refresh_signature_asset(&mut self, cx: &mut Context<Self>) {
        let state = &mut self.annotation.signature;
        state.error = None;
        state.asset = match state.method {
            SignatureMethod::Text => {
                self.refresh_signature_text(cx);
                return;
            }
            SignatureMethod::Draw => draw_asset_with(
                &state.strokes,
                signature_color(state.color),
                state.thickness,
            )
            .map(Arc::new),
            SignatureMethod::Image => state.imported.as_ref().and_then(|original| {
                if state.remove_white {
                    image_variant(original, true).map(Arc::new)
                } else {
                    Some(original.clone())
                }
            }),
        };
        if state.method == SignatureMethod::Image && state.remove_white && state.asset.is_none() {
            state.error =
                Some("Tidak ada isi tersisa. Klik lagi untuk mengembalikan latar asli.".into());
        }
        state.preview = state.asset.as_deref().and_then(asset_preview);
        cx.notify();
    }

    fn signature_panel(&mut self, window: &mut Window, cx: &mut Context<Self>) -> gpui::AnyElement {
        let state = &self.annotation.signature;
        let theme = cx.theme().colors;
        let view = state.gallery.view;
        let count = state.gallery.entries.len();
        let usable = !self.opening
            && !self.edits_locked()
            && match view {
                GalleryView::Gallery => self.selected_signature_template().is_some(),
                GalleryView::Create => state.phone.is_none() && state.asset.is_some(),
                GalleryView::Loading => false,
            };
        let mut navigation = gpui::div().flex().items_center().justify_between().gap_2();
        if view == GalleryView::Gallery {
            navigation = navigation
                .child(
                    Button::new("signature-saved-tab")
                        .outline()
                        .label(format!("Tersimpan ({count})"))
                        .selected(true)
                        .on_click(cx.listener(|s, _, _, cx| s.show_signature_gallery(cx))),
                )
                .child(
                    Button::new("signature-new")
                        .outline()
                        .icon(IconName::Plus)
                        .label("Buat Baru")
                        .disabled(self.opening)
                        .on_click(cx.listener(|s, _, _, cx| {
                            s.annotation.signature.editor_revision += 1;
                            s.show_signature_creation(cx);
                        })),
                );
        } else if view == GalleryView::Create && count > 0 {
            navigation = navigation.child(
                Button::new("signature-back")
                    .outline()
                    .icon(IconName::ArrowLeft)
                    .label("Kembali ke Tersimpan")
                    .disabled(self.opening)
                    .on_click(cx.listener(|s, _, _, cx| {
                        s.cancel_signature_phone();
                        s.annotation.signature.editor_revision += 1;
                        s.show_signature_gallery(cx);
                    })),
            );
        }
        let header = gpui::div()
            .flex()
            .flex_col()
            .gap_2()
            .px_3()
            .py_2()
            .border_b_1()
            .border_color(theme.border)
            .child(
                gpui::div()
                    .text_base()
                    .font_semibold()
                    .child("Tanda Tangan & Stempel"),
            )
            .when(
                view == GalleryView::Gallery || (view == GalleryView::Create && count > 0),
                |el| el.child(navigation),
            );

        let body_height = (window.viewport_size().height - px(305.)).max(px(165.));
        let panel = match view {
            GalleryView::Loading => gpui::div()
                .p_4()
                .child("Memuat template...")
                .into_any_element(),
            GalleryView::Gallery => self.signature_gallery_panel(cx),
            GalleryView::Create => self.signature_creation_panel(body_height, cx),
        };
        let mut body = gpui::div()
            .id(("signature-editor-scroll", view as usize))
            .max_h(body_height)
            .overflow_y_scroll()
            .w_full()
            .p_3()
            .child(panel);
        if let Some(error) = &self.annotation.signature.gallery.error {
            body = body.child(
                gpui::div()
                    .mt_2()
                    .text_sm()
                    .text_color(cx.theme().danger)
                    .child(error.clone()),
            );
        }
        gpui::div()
            .w(px(530.))
            .max_w_full()
            .flex()
            .flex_col()
            .child(header)
            .child(body)
            .child(
                gpui::div()
                    .w_full()
                    .flex()
                    .justify_between()
                    .items_center()
                    .px_3()
                    .py_2()
                    .border_t_1()
                    .border_color(cx.theme().border)
                    .child(
                        Button::new("signature-cancel")
                            .outline()
                            .icon(IconName::X)
                            .label("Batal")
                            .accessibility_label("Batal tanda tangan")
                            .disabled(self.opening)
                            .on_click(cx.listener(|s, _, window, cx| {
                                s.dismiss_signature_editor(cx);
                                s.focus.focus(window, cx);
                                cx.notify();
                            })),
                    )
                    .child(
                        Button::new("signature-use")
                            .primary()
                            .icon(IconName::Check)
                            .label("Gunakan Tanda Tangan")
                            .disabled(!usable)
                            .on_click(cx.listener(|s, _, window, cx| s.use_signature(window, cx))),
                    ),
            )
            .into_any_element()
    }

    fn dismiss_signature_editor(&mut self, cx: &mut Context<Self>) {
        self.cancel_signature_phone();
        let state = &mut self.annotation.signature;
        state.open = false;
        state.drawing = false;
        state.editor_revision += 1;
        state.gallery.reset_selection();
        self.annotation_popup_changed(false, cx);
    }

    fn use_signature(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.opening || self.edits_locked() {
            return;
        }
        let state = &self.annotation.signature;
        match state.gallery.view {
            GalleryView::Gallery => {
                if let Some(entry) = self.selected_signature_template() {
                    self.begin_signature_placement(entry.asset, window, cx);
                }
            }
            GalleryView::Create if state.phone.is_none() => {
                let Some(asset) = state.asset.clone() else {
                    return;
                };
                if state.save_template {
                    let label = state
                        .library_label
                        .as_ref()
                        .map(|input| input.read(cx).value().trim().to_owned())
                        .unwrap_or_default();
                    self.save_signature_template(asset, state.kind, label, window, cx);
                } else {
                    self.begin_signature_placement(asset, window, cx);
                }
            }
            _ => {}
        }
    }

    pub(super) fn begin_signature_placement(
        &mut self,
        asset: Arc<SignatureImage>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.annotation.signature.open || self.edits_locked() || self.opening {
            return;
        }
        self.dismiss_signature_editor(cx);
        self.annotation_popup(false);
        self.cancel_signature_placement(cx);
        self.annotation.signature.placement_ghost = Some(ghost_images(&asset));
        self.annotation.signature.asset = Some(asset);
        self.annotation.signature.placing = true;
        self.annotation.signature.save_template = false;
        if let Some(input) = self.annotation.signature.library_label.clone() {
            input.update(cx, |state, cx| state.set_value("", window, cx));
        }
        self.annotation.tool = crate::annotations::Tool::Select;
        self.status =
            "Klik atau drag di halaman untuk menempatkan tanda tangan. Esc membatalkan.".into();
        self.focus.focus(window, cx);
        cx.notify();
    }

    fn signature_creation_panel(
        &mut self,
        body_height: gpui::Pixels,
        cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
        let kind = self.annotation.signature.kind;
        let method = self.annotation.signature.method;
        let qr_active = self.annotation.signature.phone.is_some();
        let mut panel = gpui::div()
            .flex()
            .flex_col()
            .gap_3()
            .child(self.signature_categories(cx));
        if kind != SignatureKind::Stamp {
            panel = panel.child(self.signature_methods(cx));
        }
        panel = panel.child(match method {
            SignatureMethod::Draw if qr_active => self.signature_phone_panel(cx),
            SignatureMethod::Draw => self.signature_draw_panel((body_height - px(30.)) * 2., cx),
            SignatureMethod::Text => self.signature_text_panel(cx),
            SignatureMethod::Image => self.signature_upload_panel(cx),
        });
        if let Some(error) = &self.annotation.signature.error {
            panel = panel.child(
                gpui::div()
                    .text_sm()
                    .text_color(cx.theme().danger)
                    .child(error.clone()),
            );
        }
        if !qr_active {
            let state = &self.annotation.signature;
            let can_save = state.gallery.ready && state.gallery.entries.len() < 8;
            panel = panel.child(
                Checkbox::new("signature-save-template")
                    .label("Simpan ke template lokal")
                    .checked(state.save_template)
                    .disabled(!can_save || self.opening)
                    .on_change(cx.listener(|s, checked, _, cx| {
                        s.annotation.signature.save_template = *checked;
                        cx.notify();
                    })),
            );
            if state.save_template
                && let Some(input) = &state.library_label
            {
                panel = panel.child(Input::new(input).aria_label("Nama template tanda tangan"));
            } else if state.gallery.entries.len() >= 8 {
                panel = panel.child(
                    gpui::div()
                        .text_xs()
                        .text_color(cx.theme().muted_foreground)
                        .child("Pustaka penuh. Hapus template untuk menyimpan yang baru."),
                );
            }
        }
        panel.into_any_element()
    }

    fn signature_categories(&self, cx: &mut Context<Self>) -> gpui::Div {
        gpui::div()
            .flex()
            .w_full()
            .gap_1()
            .p_1()
            .rounded_lg()
            .bg(cx.theme().muted)
            .children(
                [
                    (
                        SignatureKind::Signature,
                        "Tanda Tangan",
                        "Tanda tangan penuh",
                    ),
                    (SignatureKind::Initials, "Inisial", "Inisial"),
                    (SignatureKind::Stamp, "Stempel", "Stempel"),
                ]
                .into_iter()
                .map(|(kind, label, accessibility)| {
                    Button::new(("signature-kind", kind as usize))
                        .flex_1()
                        .min_w_0()
                        .label(label)
                        .accessibility_label(accessibility)
                        .selected(self.annotation.signature.kind == kind)
                        .disabled(self.opening)
                        .on_click(cx.listener(move |s, _, window, cx| {
                            s.change_signature_kind(kind, window, cx)
                        }))
                }),
            )
    }

    fn change_signature_kind(
        &mut self,
        kind: SignatureKind,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let state = &mut self.annotation.signature;
        if state.kind == kind {
            return;
        }
        let method = match (state.kind, kind) {
            (_, SignatureKind::Stamp) => {
                state.stamp_return_method = state.method;
                SignatureMethod::Image
            }
            (SignatureKind::Stamp, _) => state.stamp_return_method,
            _ => state.method,
        };
        let previous = SignatureCategoryDraft {
            strokes: std::mem::take(&mut state.strokes),
            imported: state.imported.take(),
            remove_white: state.remove_white,
        };
        state.inactive_drafts[state.kind as usize] = previous;
        let next = std::mem::take(&mut state.inactive_drafts[kind as usize]);
        state.kind = kind;
        state.strokes = next.strokes;
        state.imported = next.imported;
        state.remove_white = next.remove_white;
        if kind == SignatureKind::Initials
            && let Some(input) = state.initials.clone()
            && input.read(cx).value().is_empty()
            && let Some(name) = &state.text
        {
            let suggestion = name
                .read(cx)
                .value()
                .split_whitespace()
                .filter_map(|word| word.chars().next())
                .take(8)
                .collect::<String>()
                .to_uppercase();
            input.update(cx, |input, cx| input.set_value(suggestion, window, cx));
        }
        self.signature_method(method, cx);
    }

    fn signature_methods(&self, cx: &mut Context<Self>) -> gpui::Div {
        let theme = cx.theme().colors;
        let active = self.annotation.signature.method;
        gpui::div()
            .flex()
            .gap_2()
            .border_b_1()
            .border_color(theme.border)
            .children(
                [
                    (SignatureMethod::Draw, "Gambar", IconName::Pencil),
                    (SignatureMethod::Text, "Ketik", IconName::Type),
                    (SignatureMethod::Image, "Unggah", IconName::Image),
                ]
                .into_iter()
                .map(|(method, label, icon)| {
                    gpui::div()
                        .pb_1()
                        .border_b_2()
                        .border_color(if method == active {
                            theme.primary
                        } else {
                            gpui::transparent_black()
                        })
                        .child(
                            Button::new(("signature-method", method as usize))
                                .icon(icon)
                                .label(label)
                                .selected(method == active)
                                .disabled(self.opening)
                                .on_click(
                                    cx.listener(move |s, _, _, cx| s.signature_method(method, cx)),
                                ),
                        )
                }),
            )
    }

    fn signature_draw_panel(
        &self,
        max_pad_width: gpui::Pixels,
        cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
        let weak = cx.weak_entity();
        let strokes = self.annotation.signature.strokes.clone();
        let color = signature_color(self.annotation.signature.color);
        let thickness = self.annotation.signature.thickness;
        let pad = gpui::canvas(
            |bounds, window, _| window.insert_hitbox(bounds, gpui::HitboxBehavior::Normal),
            move |bounds, hitbox, window, _| {
                paint_signature_ink(window, bounds, &strokes, color, thickness);
                let down = weak.clone();
                window.on_mouse_event(move |e: &gpui::MouseDownEvent, phase, window, cx| {
                    if phase == gpui::DispatchPhase::Bubble
                        && e.button == gpui::MouseButton::Left
                        && hitbox.is_hovered_at(e.position, window)
                    {
                        let _ = down.update(cx, |s, cx| {
                            if !s.annotation.signature.open {
                                return;
                            }
                            if s.annotation.signature.strokes.len() >= 128 {
                                return;
                            }
                            s.annotation.signature.phone_status = None;
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
                            if s.annotation.signature.drawing {
                                s.annotation.signature.drawing = false;
                                s.refresh_signature_asset(cx);
                            }
                        });
                    }
                });
            },
        )
        .w_full()
        .aspect_ratio(2.0);
        let state = &self.annotation.signature;
        let empty = state.strokes.is_empty();
        let button_bar = gpui::div()
            .absolute()
            .top_2()
            .right_2()
            .flex()
            .gap_1()
            .block_mouse_except_scroll()
            .child(
                Button::new("signature-undo")
                    .outline()
                    .small()
                    .icon(IconName::Undo2)
                    .accessibility_label("Undo goresan")
                    .tooltip("Undo goresan")
                    .disabled(empty)
                    .on_click(cx.listener(|s, _, _, cx| {
                        s.annotation.signature.strokes.pop();
                        s.refresh_signature_asset(cx);
                    })),
            )
            .child(
                Button::new("signature-clear")
                    .outline()
                    .small()
                    .icon(IconName::Trash)
                    .accessibility_label("Hapus semua")
                    .tooltip("Hapus semua goresan")
                    .disabled(empty)
                    .on_click(cx.listener(|s, _, _, cx| {
                        s.annotation.signature.strokes.clear();
                        s.refresh_signature_asset(cx);
                    })),
            );
        let thickness = gpui::div()
            .flex()
            .items_center()
            .gap_1()
            .child(gpui::div().mr_1().text_sm().child("Ketebalan"))
            .children(["Tipis", "Sedang", "Tebal"].into_iter().enumerate().map(
                |(index, label)| {
                    Button::new(("signature-thickness", index))
                        .small()
                        .w(px(36.))
                        .accessibility_label(label)
                        .tooltip(label)
                        .selected(state.thickness == index)
                        .child(
                            gpui::div()
                                .w(px(16.))
                                .h(px((index + 1) as f32))
                                .rounded_full()
                                .bg(cx.theme().foreground),
                        )
                        .on_click(cx.listener(move |s, _, _, cx| {
                            s.annotation.signature.thickness = index;
                            s.refresh_signature_asset(cx);
                        }))
                },
            ));
        gpui::div()
            .flex()
            .flex_col()
            .gap_2()
            .child(
                gpui::div()
                    .flex()
                    .justify_between()
                    .items_center()
                    .gap_2()
                    .child(gpui::div().text_sm().child("Goreskan di area berikut"))
                    .child(self.signature_phone_button(cx)),
            )
            .child(
                gpui::div()
                    .id("signature-pad")
                    .role(gpui::Role::Group)
                    .aria_label("Area gambar tanda tangan")
                    .relative()
                    .w_full()
                    .max_w(max_pad_width)
                    .mx_auto()
                    .rounded_lg()
                    .overflow_hidden()
                    .bg(gpui::white())
                    .border_1()
                    .border_color(cx.theme().border)
                    .child(pad)
                    .child(button_bar),
            )
            .child(
                gpui::div()
                    .flex()
                    .justify_between()
                    .items_center()
                    .gap_2()
                    .child(self.signature_swatches(cx))
                    .child(thickness),
            )
            .when_some(state.phone_status.clone(), |el, status| {
                el.child(gpui::div().text_sm().child(status))
            })
            .into_any_element()
    }

    fn signature_swatches(&self, cx: &mut Context<Self>) -> gpui::Div {
        let theme = cx.theme().colors;
        gpui::div()
            .flex()
            .items_center()
            .gap_1()
            .child(gpui::div().mr_1().text_sm().child("Warna"))
            .children(
                ["Hitam", "Biru", "Merah"]
                    .into_iter()
                    .enumerate()
                    .map(|(color, label)| {
                        let [r, g, b] = signature_color(color);
                        Button::new(("signature-color", color))
                            .small()
                            .size(px(30.))
                            .p_0()
                            .rounded_full()
                            .accessibility_label(label)
                            .tooltip(label)
                            .selected(color == self.annotation.signature.color)
                            .border_2()
                            .border_color(if color == self.annotation.signature.color {
                                theme.primary
                            } else {
                                theme.border
                            })
                            .child(
                                gpui::div()
                                    .size(px(16.))
                                    .flex_shrink_0()
                                    .rounded_full()
                                    .bg(gpui::rgb(u32::from_be_bytes([0, r, g, b]))),
                            )
                            .on_click(cx.listener(move |s, _, _, cx| {
                                s.annotation.signature.color = color;
                                s.refresh_signature_asset(cx);
                            }))
                    }),
            )
    }

    fn signature_text_panel(&self, cx: &mut Context<Self>) -> gpui::AnyElement {
        let state = &self.annotation.signature;
        let input = if state.kind == SignatureKind::Initials {
            &state.initials
        } else {
            &state.text
        };
        let fonts = gpui::div()
            .flex()
            .items_center()
            .gap_1()
            .child(gpui::div().mr_1().text_sm().child("Gaya Tulisan"))
            .children(["Caveat", "Dancing Script"].into_iter().enumerate().map(
                |(style, label)| {
                    let mut button = Button::new(("signature-font", style))
                        .small()
                        .accessibility_label(label)
                        .tooltip(label)
                        .selected(state.style == style)
                        .on_click(cx.listener(move |s, _, _, cx| {
                            s.annotation.signature.style = style;
                            s.refresh_signature_text(cx);
                        }));
                    if let Some(sample) = &state.font_samples[style] {
                        let width = if style == 0 { px(48.) } else { px(84.) };
                        button = button.child(
                            gpui::div().rounded_sm().bg(gpui::white()).child(
                                gpui::img(sample.clone())
                                    .w(width)
                                    .h(px(19.))
                                    .object_fit(gpui::ObjectFit::Contain),
                            ),
                        );
                    } else {
                        button = button.label(label);
                    }
                    button
                },
            ));
        gpui::div()
            .flex()
            .flex_col()
            .gap_2()
            .when_some(input.as_ref(), |el, input| {
                el.child(Input::new(input).aria_label("Nama atau inisial tanda tangan"))
            })
            .child(self.signature_preview_box("Pratinjau tanda tangan", cx))
            .child(
                gpui::div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap_2()
                    .child(fonts)
                    .child(self.signature_swatches(cx)),
            )
            .into_any_element()
    }

    fn signature_preview_box(&self, label: &'static str, cx: &App) -> gpui::Stateful<gpui::Div> {
        gpui::div()
            .id("signature-preview")
            .role(gpui::Role::Group)
            .aria_label(label)
            .w_full()
            .h(px(180.))
            .p_4()
            .flex()
            .items_center()
            .justify_center()
            .relative()
            .rounded_lg()
            .overflow_hidden()
            .border_1()
            .border_color(cx.theme().border)
            .bg(gpui::rgb(0xf8fafc))
            .when(
                self.annotation.signature.method == SignatureMethod::Image,
                |el| el.child(transparency_grid()),
            )
            .when_some(self.annotation.signature.preview.clone(), |el, preview| {
                el.child(
                    gpui::img(preview)
                        .size_full()
                        .object_fit(gpui::ObjectFit::Contain),
                )
            })
    }

    fn signature_upload_panel(&self, cx: &mut Context<Self>) -> gpui::AnyElement {
        let theme = cx.theme().colors;
        let empty = self.annotation.signature.imported.is_none();
        gpui::div()
            .id("signature-upload")
            .w_full()
            .flex()
            .flex_col()
            .gap_2()
            .drag_over::<gpui::ExternalPaths>(|style, _, _, cx| style.bg(cx.theme().muted))
            .on_drop(cx.listener(|s, paths: &gpui::ExternalPaths, _, cx| {
                if s.opening || s.edits_locked() {
                    return;
                }
                if let [path] = paths.0.as_slice() {
                    s.decode_signature_image(path.clone(), cx);
                } else {
                    s.annotation.signature.error = Some("Seret satu gambar PNG atau JPEG.".into());
                    cx.notify();
                }
            }))
            .when(empty, |el| {
                el.child(
                    Button::new("signature-import")
                        .w_full()
                        .h(px(180.))
                        .border_2()
                        .border_dashed()
                        .border_color(theme.border)
                        .rounded_lg()
                        .accessibility_label("Pilih gambar tanda tangan")
                        .disabled(self.opening)
                        .child(
                            gpui::div()
                                .flex()
                                .flex_col()
                                .items_center()
                                .gap_2()
                                .child(Icon::new(IconName::Upload).size(px(24.)))
                                .child(
                                    gpui::div()
                                        .font_semibold()
                                        .child("Klik untuk pilih gambar atau seret ke sini"),
                                )
                                .child(
                                    gpui::div()
                                        .text_xs()
                                        .text_color(theme.muted_foreground)
                                        .child("PNG atau JPEG. Transparansi PNG dipertahankan."),
                                ),
                        )
                        .on_click(cx.listener(|s, _, _, cx| s.import_signature(cx))),
                )
            })
            .when(!empty, |el| {
                el.child(self.signature_preview_box("Pratinjau gambar tanda tangan", cx))
                    .child(
                        gpui::div()
                            .flex()
                            .justify_between()
                            .gap_2()
                            .child(
                                Button::new("signature-remove-white")
                                    .outline()
                                    .icon(IconName::WandSparkles)
                                    .label("Hapus latar putih")
                                    .selected(self.annotation.signature.remove_white)
                                    .disabled(self.opening)
                                    .on_click(cx.listener(|s, _, _, cx| {
                                        s.annotation.signature.remove_white =
                                            !s.annotation.signature.remove_white;
                                        s.refresh_signature_asset(cx);
                                    })),
                            )
                            .child(
                                Button::new("signature-replace-image")
                                    .outline()
                                    .icon(IconName::Upload)
                                    .label("Ganti gambar")
                                    .disabled(self.opening)
                                    .on_click(cx.listener(|s, _, _, cx| s.import_signature(cx))),
                            ),
                    )
            })
            .into_any_element()
    }

    fn import_signature(&mut self, cx: &mut Context<Self>) {
        if self.edits_locked() || self.opening {
            return;
        }
        let revision = self.annotation.signature.editor_revision;
        let picker = cx.prompt_for_paths(gpui::PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some("Unggah gambar PNG atau JPEG".into()),
        });
        self.opening = true;
        cx.notify();
        cx.spawn(async move |this, cx| {
            let path = match picker.await {
                Ok(Ok(Some(paths))) => paths.into_iter().next(),
                _ => None,
            };
            let _ = this.update(cx, |s, cx| {
                if let Some(path) = path
                    && s.annotation.signature.open
                    && s.annotation.signature.editor_revision == revision
                {
                    s.decode_signature_image(path, cx);
                } else {
                    s.opening = false;
                    cx.notify();
                }
            });
        })
        .detach();
    }

    fn decode_signature_image(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        let revision = self.annotation.signature.editor_revision;
        self.opening = true;
        cx.notify();
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_executor()
                .spawn(async move { load_asset(&path) })
                .await;
            let _ = this.update(cx, |s, cx| {
                s.opening = false;
                let state = &mut s.annotation.signature;
                if state.open && state.editor_revision == revision {
                    match result {
                        Ok(asset) => {
                            state.imported = Some(Arc::new(asset));
                            state.remove_white = false;
                            s.refresh_signature_asset(cx);
                        }
                        Err(error) => state.error = Some(error),
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    pub(super) fn signature_transform_menu(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let owner = cx.weak_entity();
        Button::new("signature-transform")
            .ghost()
            .icon(IconName::RotateCw)
            .label("Transformasi")
            .dropdown_caret(true)
            .accessibility_label("Transformasi tanda tangan")
            .disabled(self.edits_locked() || self.opening || self.annotation.signature.open)
            .dropdown_menu(move |mut menu, _, _| {
                use SignatureTransform::*;
                for (label, operation) in [
                    ("Putar kiri 90°", RotateLeft),
                    ("Putar kanan 90°", RotateRight),
                    ("Balik horizontal", FlipHorizontal),
                    ("Balik vertikal", FlipVertical),
                ] {
                    let owner = owner.clone();
                    menu = menu.item(PopupMenuItem::new(label).on_click(move |_, window, cx| {
                        let _ = owner.update(cx, |view, cx| {
                            view.transform_selected_signature(operation, cx);
                            view.focus.focus(window, cx);
                        });
                    }));
                }
                menu
            })
    }

    fn transform_selected_signature(
        &mut self,
        operation: SignatureTransform,
        cx: &mut Context<Self>,
    ) {
        if self.edits_locked()
            || self.opening
            || !self.annotation.open
            || self.annotation.signature.open
            || self.annotation.signature.placing
            || self.annotation.signature.gesture.is_some()
        {
            return;
        }
        let Some((page, id)) = self.annotation.selected else {
            return;
        };
        let Some(workspace) = self.workspace.as_ref() else {
            return;
        };
        let Some(geometry) = workspace
            .pages()
            .iter()
            .position(|p| p.id == page)
            .and_then(|index| workspace.geometry(index).ok())
        else {
            return;
        };
        let mut annotations = workspace.annotations(page).to_vec();
        drop(workspace);
        if let Some(annotation) = annotations.iter_mut().find(|a| a.id == id)
            && operation.apply(annotation, geometry)
        {
            self.set_annotations(page, annotations, cx);
        }
    }

    pub(super) fn change_selected_signature(
        &mut self,
        duplicate: bool,
        delta: [f32; 2],
        cx: &mut Context<Self>,
    ) {
        if self.edits_locked()
            || self.opening
            || self.annotation.signature.open
            || self.annotation.text.open
            || self.annotation.apply_pages.open
            || !self.annotation.open
        {
            return;
        }
        let Some((page, id)) = self.annotation.selected else {
            return;
        };
        let Some(mut w) = self.workspace.as_mut() else {
            return;
        };
        let turns = w
            .pages()
            .iter()
            .position(|p| p.id == page)
            .and_then(|i| w.geometry(i).ok())
            .map(|g| g.quarter_turns)
            .unwrap_or(0);
        let delta = signature_delta(delta, turns);
        let mut all = w.annotations(page).to_vec();
        let Some(index) = all.iter().position(|a| a.id == id && a.is_box_object()) else {
            return;
        };
        let mut asset = all[index].clone();
        if duplicate && asset.signature.is_some() {
            let simulated = w
                .pages()
                .iter()
                .flat_map(|p| w.annotations(p.id).iter())
                .chain(std::iter::once(&asset));
            if !pdf_domain::annotation::within_signature_budget(simulated) {
                drop(w);
                self.status = "Batas penempatan tanda tangan/stempel pada dokumen tercapai.".into();
                cx.notify();
                return;
            }
        }
        let dx = delta[0].clamp(-asset.points[0][0], 1. - asset.points[1][0]);
        let dy = delta[1].clamp(-asset.points[0][1], 1. - asset.points[1][1]);
        for p in &mut asset.points {
            p[0] += dx;
            p[1] += dy;
        }
        if duplicate {
            asset.id = w.allocate_annotation_id();
            all.push(asset.clone());
        } else {
            all[index] = asset.clone();
        }
        drop(w);
        self.set_annotations(page, all, cx);
        self.annotation.selected = Some((page, asset.id));
        cx.notify();
    }
    pub(super) fn cancel_signature_placement(&mut self, cx: &mut Context<Self>) {
        self.annotation.text.placing = None;
        self.annotation.tools.placing_symbol = None;
        self.annotation.tools.placing_note = None;
        let state = &mut self.annotation.signature;
        state.placing = false;
        state.gesture = None;
        state.gesture_bounds = None;
        state.ghost_pointer_exited = false;
        state.signature_ghost_rect = None;
        if let Some(images) = state.placement_ghost.take() {
            self.release_images(images.into_iter().flatten().collect(), cx);
        }
    }

    pub(super) fn signature_ghost_at(
        &self,
        page: PageId,
        geometry: PageGeometry,
        bounds: Bounds<gpui::Pixels>,
        cursor: gpui::Point<gpui::Pixels>,
    ) -> Option<(Arc<RenderImage>, [[f32; 2]; 2])> {
        let state = &self.annotation.signature;
        let images = state.placement_ghost.as_ref()?;
        let placing = state.placing
            || self.annotation.text.placing.is_some()
            || self.annotation.tools.placing_symbol.is_some()
            || self.annotation.tools.placing_note.is_some();
        if !self.active
            || !self.annotation.open
            || self.mode != ViewerMode::Read
            || state.ghost_pointer_exited
            || self.edits_locked()
            || (!placing
                && !state
                    .gesture
                    .as_ref()
                    .is_some_and(|(id, _, _, _)| *id == page))
        {
            return None;
        }
        let point = signature_page_point(bounds, cursor, geometry.quarter_turns);
        let rect = if let Some(pending) = &self.annotation.text.placing {
            crate::text_annotations::text_placement_rect(
                &pending.content,
                pending.font_size,
                geometry,
                point,
            )?
        } else if self.annotation.tools.placing_symbol.is_some() {
            crate::annotation_tools::symbol_placement_rect(geometry, point)
        } else if self.annotation.tools.placing_note.is_some() {
            crate::annotation_tools::note_placement_rect(geometry, point)
        } else if let Some((gesture_page, annotation, _, _)) = &state.gesture
            && *gesture_page == page
            && let Some(text) = &annotation.text
        {
            crate::text_annotations::text_placement_rect(
                &text.content,
                text.font_size,
                geometry,
                point,
            )?
        } else if let Some((gesture_page, annotation, _, _)) = &state.gesture
            && *gesture_page == page
            && matches!(
                annotation.extra,
                Some(pdf_domain::annotation::AnnotationExtra::Symbol(_))
            )
        {
            crate::annotation_tools::symbol_placement_rect(geometry, point)
        } else if let Some((gesture_page, annotation, _, _)) = &state.gesture
            && *gesture_page == page
            && matches!(
                annotation.extra,
                Some(pdf_domain::annotation::AnnotationExtra::Note(_))
            )
        {
            crate::annotation_tools::note_placement_rect(geometry, point)
        } else {
            signature_placement_rect(state.asset.as_deref()?, geometry, point)
        };
        Some((
            images[usize::from(geometry.quarter_turns % 4)].clone()?,
            rect,
        ))
    }

    pub(super) fn signature_pointer(
        &mut self,
        page: PageId,
        p: [f32; 2],
        start: bool,
        cx: &mut Context<Self>,
    ) -> bool {
        let placing_symbol = self.annotation.tools.placing_symbol.is_some()
            || (self.annotation.signature.placement_ghost.is_some()
                && self
                    .annotation
                    .signature
                    .gesture
                    .as_ref()
                    .is_some_and(|(_, a, _, _)| {
                        matches!(
                            a.extra,
                            Some(pdf_domain::annotation::AnnotationExtra::Symbol(_))
                        )
                    }));
        if placing_symbol {
            if !start && self.annotation.tools.placing_symbol.is_some() {
                return true;
            }
            let Some(geometry) = self.workspace.as_ref().and_then(|w| {
                w.pages()
                    .iter()
                    .position(|a| a.id == page)
                    .and_then(|index| w.geometry(index).ok())
            }) else {
                return false;
            };
            let corners = crate::annotation_tools::symbol_placement_rect(geometry, p);
            if start && let Some(pending) = self.annotation.tools.placing_symbol.take() {
                let id = self
                    .workspace
                    .as_mut()
                    .map(|mut w| w.allocate_annotation_id())
                    .unwrap_or(1);
                let annotation = Annotation {
                    id,
                    highlight: false,
                    color: pending.color,
                    width: 2.,
                    points: corners.to_vec(),
                    rectangles: false,
                    signature: None,
                    text: None,
                    extra: Some(pdf_domain::annotation::AnnotationExtra::Symbol(
                        pending.symbol,
                    )),
                };
                self.annotation.selected = Some((page, id));
                self.annotation.signature.gesture = Some((page, annotation, p, false));
                cx.notify();
            } else if let Some((gesture_page, annotation, _, _)) =
                &mut self.annotation.signature.gesture
                && *gesture_page == page
                && annotation.points.as_slice() != corners.as_slice()
            {
                annotation.points.copy_from_slice(&corners);
                cx.notify();
            }
            return true;
        }
        let placing_note = self.annotation.tools.placing_note.is_some()
            || (self.annotation.signature.placement_ghost.is_some()
                && self
                    .annotation
                    .signature
                    .gesture
                    .as_ref()
                    .is_some_and(|(_, a, _, _)| {
                        matches!(
                            a.extra,
                            Some(pdf_domain::annotation::AnnotationExtra::Note(_))
                        )
                    }));
        if placing_note {
            if !start && self.annotation.tools.placing_note.is_some() {
                return true;
            }
            let Some(geometry) = self.workspace.as_ref().and_then(|w| {
                w.pages()
                    .iter()
                    .position(|a| a.id == page)
                    .and_then(|index| w.geometry(index).ok())
            }) else {
                return false;
            };
            let corners = crate::annotation_tools::note_placement_rect(geometry, p);
            if start && let Some(pending) = self.annotation.tools.placing_note.take() {
                let id = self
                    .workspace
                    .as_mut()
                    .map(|mut w| w.allocate_annotation_id())
                    .unwrap_or(1);
                let annotation = Annotation {
                    id,
                    highlight: false,
                    color: pending.color,
                    width: 1.,
                    points: corners.to_vec(),
                    rectangles: false,
                    signature: None,
                    text: None,
                    extra: Some(pdf_domain::annotation::AnnotationExtra::Note(
                        pending.content,
                    )),
                };
                self.annotation.selected = Some((page, id));
                self.annotation.signature.gesture = Some((page, annotation, p, false));
                cx.notify();
            } else if let Some((gesture_page, annotation, _, _)) =
                &mut self.annotation.signature.gesture
                && *gesture_page == page
                && annotation.points.as_slice() != corners.as_slice()
            {
                annotation.points.copy_from_slice(&corners);
                cx.notify();
            }
            return true;
        }
        let placing_text = self.annotation.text.placing.is_some()
            || (self.annotation.signature.placement_ghost.is_some()
                && self
                    .annotation
                    .signature
                    .gesture
                    .as_ref()
                    .is_some_and(|(_, a, _, _)| a.text.is_some()));
        if placing_text {
            if !start && self.annotation.text.placing.is_some() {
                return true;
            }
            let Some(geometry) = self.workspace.as_ref().and_then(|w| {
                w.pages()
                    .iter()
                    .position(|a| a.id == page)
                    .and_then(|index| w.geometry(index).ok())
            }) else {
                return false;
            };
            if start && let Some(pending) = self.annotation.text.placing.take() {
                let Some(corners) = crate::text_annotations::text_placement_rect(
                    &pending.content,
                    pending.font_size,
                    geometry,
                    p,
                ) else {
                    self.cancel_signature_placement(cx);
                    return false;
                };
                let id = self
                    .workspace
                    .as_mut()
                    .map(|mut w| w.allocate_annotation_id())
                    .unwrap_or(1);
                let annotation = Annotation {
                    id,
                    highlight: false,
                    color: pending.color,
                    width: 1.,
                    points: corners.to_vec(),
                    rectangles: false,
                    signature: None,
                    text: Some(pdf_domain::annotation::TextAnnotation {
                        content: pending.content,
                        font_size: pending.font_size,
                    }),
                    extra: None,
                };
                self.annotation.selected = Some((page, id));
                self.annotation.signature.gesture = Some((page, annotation, p, false));
                cx.notify();
            } else if let Some((gesture_page, annotation, _, _)) =
                &mut self.annotation.signature.gesture
                && *gesture_page == page
                && let Some(text) = &annotation.text
                && let Some(corners) = crate::text_annotations::text_placement_rect(
                    &text.content,
                    text.font_size,
                    geometry,
                    p,
                )
                && annotation.points.as_slice() != corners.as_slice()
            {
                annotation.points.copy_from_slice(&corners);
                cx.notify();
            }
            return true;
        }
        if self.annotation.signature.placing || self.annotation.signature.placement_ghost.is_some()
        {
            if !start && self.annotation.signature.placing {
                return true;
            }
            let Some(asset) = self.annotation.signature.asset.clone() else {
                return false;
            };
            let Some(w) = self.workspace.as_ref() else {
                return false;
            };
            let Some(index) = w.pages().iter().position(|a| a.id == page) else {
                return false;
            };
            let Ok(geometry) = w.geometry(index) else {
                return false;
            };
            drop(w);
            let corners = signature_placement_rect(&asset, geometry, p);
            if start && self.annotation.signature.placing {
                let id = self
                    .workspace
                    .as_mut()
                    .map(|mut w| w.allocate_annotation_id())
                    .unwrap_or(1);
                let annotation = Annotation {
                    id,
                    highlight: false,
                    color: [0, 0, 0],
                    width: 1.,
                    points: corners.to_vec(),
                    rectangles: false,
                    signature: Some(asset),
                    text: None,
                    extra: None,
                };
                self.annotation.selected = Some((page, id));
                self.annotation.signature.gesture = Some((page, annotation, p, false));
                self.annotation.signature.placing = false;
                cx.notify();
            } else if let Some((gesture_page, annotation, _, _)) =
                &mut self.annotation.signature.gesture
                && *gesture_page == page
                && annotation.points.as_slice() != corners.as_slice()
            {
                annotation.points.copy_from_slice(&corners);
                cx.notify();
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
                        a.is_box_object()
                            && signature_hit_bounds(a, false)
                                .iter()
                                .zip(p)
                                .all(|([min, max], value)| (*min..=*max).contains(&value))
                    })
                    .cloned()
            });
            if let Some(a) = hit {
                let resize = signature_hit_bounds(&a, true)
                    .iter()
                    .zip(p)
                    .all(|([min, max], value)| (*min..=*max).contains(&value));
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
                if matches!(
                    a.extra,
                    Some(pdf_domain::annotation::AnnotationExtra::Shape(
                        pdf_domain::annotation::ShapeKind::Rectangle
                            | pdf_domain::annotation::ShapeKind::Ellipse
                    ))
                ) {
                    let max_w = (1. - a.points[0][0]).max(0.01);
                    let max_h = (1. - a.points[0][1]).max(0.01);
                    let new_w = (p[0] - a.points[0][0]).clamp(0.015_f32.min(max_w), max_w);
                    let new_h = (p[1] - a.points[0][1]).clamp(0.015_f32.min(max_h), max_h);
                    a.points[1] = [a.points[0][0] + new_w, a.points[0][1] + new_h];
                } else {
                    let width = a.points[1][0] - a.points[0][0];
                    let height = a.points[1][1] - a.points[0][1];
                    let ratio = if width > 0. { height / width } else { 1. };
                    let max = (1. - a.points[0][0]).min((1. - a.points[0][1]) / ratio);
                    let new_width = (p[0] - a.points[0][0]).clamp(0.02_f32.min(max), max);
                    if let Some(text) = &mut a.text {
                        if width > 0. {
                            let scaled = (text.font_size * (new_width / width)).clamp(
                                pdf_domain::annotation::MIN_TEXT_SIZE,
                                pdf_domain::annotation::MAX_TEXT_SIZE,
                            );
                            let clamped_width = (width * (scaled / text.font_size)).min(max);
                            text.font_size = (text.font_size * (clamped_width / width)).clamp(
                                pdf_domain::annotation::MIN_TEXT_SIZE,
                                pdf_domain::annotation::MAX_TEXT_SIZE,
                            );
                            a.points[1] = [
                                a.points[0][0] + clamped_width,
                                a.points[0][1] + clamped_width * ratio,
                            ];
                        }
                    } else {
                        a.points[1] = [
                            a.points[0][0] + new_width,
                            a.points[0][1] + new_width * ratio,
                        ];
                    }
                }
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
#[derive(Clone, Copy)]
enum SignatureTransform {
    RotateLeft,
    RotateRight,
    FlipHorizontal,
    FlipVertical,
}

impl SignatureTransform {
    fn apply(self, annotation: &mut Annotation, geometry: PageGeometry) -> bool {
        use SignatureTransform::*;
        let Some(asset) = &annotation.signature else {
            return false;
        };
        let Some(image) = image::RgbaImage::from_raw(asset.width, asset.height, asset.rgba.clone())
        else {
            return false;
        };
        let odd_turns = geometry.quarter_turns % 2 == 1;
        // Flips follow the displayed axes, which are exchanged on sideways pages.
        let image = match (self, odd_turns) {
            (RotateLeft, _) => image::imageops::rotate270(&image),
            (RotateRight, _) => image::imageops::rotate90(&image),
            (FlipHorizontal, false) | (FlipVertical, true) => {
                image::imageops::flip_horizontal(&image)
            }
            (FlipVertical, false) | (FlipHorizontal, true) => {
                image::imageops::flip_vertical(&image)
            }
        };
        if matches!(self, RotateLeft | RotateRight) {
            let [page_width, page_height] = unrotated_page_size(geometry);
            let [min, max] = [annotation.points[0], annotation.points[1]];
            // Swap physical dimensions while keeping the center where the page allows it.
            let width = (max[1] - min[1]) * page_height / page_width;
            let height = (max[0] - min[0]) * page_width / page_height;
            let scale = 1. / width.max(height).max(1.);
            let size = [width * scale, height * scale];
            for (axis, length) in size.into_iter().enumerate() {
                let start = ((min[axis] + max[axis] - length) / 2.).clamp(0., 1. - length);
                annotation.points[0][axis] = start;
                annotation.points[1][axis] = (start + length).min(1.);
            }
        }
        annotation.signature = Some(Arc::new(SignatureImage {
            width: image.width(),
            height: image.height(),
            rgba: image.into_raw(),
        }));
        true
    }
}

/// Hit ranges on the two unrotated page axes, shared by input and cursors.
pub(super) fn signature_hit_bounds(a: &Annotation, resize: bool) -> [[f32; 2]; 2] {
    [0, 1].map(|axis| {
        let end = a.points[1][axis];
        let start = if resize {
            (end - 0.02).max(a.points[0][axis] - 0.015)
        } else {
            a.points[0][axis] - 0.015
        };
        [start, end + 0.015]
    })
}

pub(super) fn signature_page_point(
    bounds: Bounds<gpui::Pixels>,
    cursor: gpui::Point<gpui::Pixels>,
    turns: u8,
) -> [f32; 2] {
    pdf_domain::annotation::rotate(pad_point(bounds, cursor), (4 - turns % 4) % 4)
}

fn signature_placement_rect(
    asset: &SignatureImage,
    geometry: PageGeometry,
    point: [f32; 2],
) -> [[f32; 2]; 2] {
    let [width, height] = unrotated_page_size(geometry);
    let ratio = asset.height as f32 / asset.width as f32 * width / height;
    let width = 0.3_f32.min(0.25 / ratio);
    let height = width * ratio;
    let left = (point[0] - width / 2.).clamp(0., 1. - width);
    let top = (point[1] - height / 2.).clamp(0., 1. - height);
    [
        [left, top],
        [(left + width).min(1.), (top + height).min(1.)],
    ]
}

pub(super) fn unrotated_page_size(geometry: PageGeometry) -> [f32; 2] {
    // Annotation coordinates precede page rotation. Workspace sizes are displayed sizes.
    if geometry.quarter_turns % 2 == 1 {
        [geometry.height_points, geometry.width_points]
    } else {
        [geometry.width_points, geometry.height_points]
    }
}

pub(super) fn ghost_images(asset: &SignatureImage) -> [Option<Arc<RenderImage>>; 4] {
    let Some(mut image) = image::RgbaImage::from_raw(asset.width, asset.height, asset.rgba.clone())
    else {
        return std::array::from_fn(|_| None);
    };
    for pixel in image.pixels_mut() {
        pixel[3] = (u16::from(pixel[3]) * 2 / 3) as u8;
        pixel.0.swap(0, 2);
    }
    std::array::from_fn(|turns| {
        let rotated = match turns {
            1 => image::imageops::rotate90(&image),
            2 => image::imageops::rotate180(&image),
            3 => image::imageops::rotate270(&image),
            _ => image.clone(),
        };
        Some(Arc::new(RenderImage::new(smallvec::smallvec![
            image::Frame::new(rotated)
        ])))
    })
}

fn transparency_grid() -> impl gpui::IntoElement {
    gpui::canvas(
        |_, _, _| (),
        |bounds, (), window, _| {
            let tile = px(12.);
            let cols = (f32::from(bounds.size.width) / f32::from(tile)).ceil() as usize;
            let rows = (f32::from(bounds.size.height) / f32::from(tile)).ceil() as usize;
            for y in 0..rows {
                for x in (y % 2..cols).step_by(2) {
                    let origin = bounds.origin + gpui::point(tile * x as f32, tile * y as f32);
                    window.paint_quad(gpui::fill(
                        Bounds::new(origin, size(tile, tile)),
                        gpui::rgb(0xeff2f6),
                    ));
                }
            }
        },
    )
    .absolute()
    .inset_0()
}

fn signature_delta(delta: [f32; 2], turns: u8) -> [f32; 2] {
    let point =
        pdf_domain::annotation::rotate([0.5 + delta[0], 0.5 + delta[1]], (4 - turns % 4) % 4);
    [point[0] - 0.5, point[1] - 0.5]
}
pub(super) fn signature_color(index: usize) -> [u8; 3] {
    [[25, 25, 25], [30, 80, 180], [180, 35, 45]][index.min(2)]
}
fn paint_signature_ink(
    window: &mut Window,
    bounds: Bounds<gpui::Pixels>,
    strokes: &[Vec<[f32; 2]>],
    [r, g, b]: [u8; 3],
    thickness: usize,
) {
    use crate::signature_strokes::{self, HEIGHT, Segment, WIDTH};
    use lyon_tessellation::{LineCap, LineJoin};

    let scale_x = f32::from(bounds.size.width) / WIDTH as f32;
    let scale_y = f32::from(bounds.size.height) / HEIGHT as f32;
    let project = |[x, y]: [f32; 2]| bounds.origin + gpui::point(px(x * scale_x), px(y * scale_y));
    let color = gpui::rgb(u32::from_be_bytes([0, r, g, b]));
    let stroke_width = signature_strokes::stroke_width(thickness) * scale_x.min(scale_y);
    if stroke_width <= 0. {
        return;
    }
    let options = gpui::StrokeOptions::default()
        .with_line_width(stroke_width)
        .with_line_cap(LineCap::Round)
        .with_line_join(LineJoin::Round);

    for stroke in strokes
        .iter()
        .filter_map(|stroke| signature_strokes::smooth(stroke))
    {
        let first = project(stroke.start);
        if stroke.segments.is_empty() {
            let radius = px(stroke_width / 2.);
            window.paint_quad(gpui::quad(
                Bounds::new(
                    first - gpui::point(radius, radius),
                    size(radius * 2., radius * 2.),
                ),
                radius,
                color,
                px(0.),
                gpui::transparent_black(),
                gpui::BorderStyle::default(),
            ));
            continue;
        }
        let mut path = gpui::PathBuilder::fill().with_style(gpui::PathStyle::Stroke(options));
        path.move_to(first);
        for segment in stroke.segments {
            match segment {
                Segment::LineTo(end) => path.line_to(project(end)),
                Segment::CurveTo { control, end } => path.curve_to(project(end), project(control)),
            }
        }
        if let Ok(path) = path.build() {
            window.paint_path(path, color);
        }
    }
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
pub(super) fn draw_asset_with(
    strokes: &[Vec<[f32; 2]>],
    color: [u8; 3],
    thickness: usize,
) -> Option<SignatureImage> {
    crop_asset(crate::signature_strokes::rasterize(
        strokes, color, thickness,
    )?)
}
pub(super) fn crop_asset(image: image::RgbaImage) -> Option<SignatureImage> {
    let (x, y, width, height) = visible_bounds(&image)?;
    let image = if (width, height) == image.dimensions() {
        image
    } else {
        image::imageops::crop_imm(&image, x, y, width, height).to_image()
    };
    Some(SignatureImage {
        width,
        height,
        rgba: image.into_raw(),
    })
}

fn visible_bounds(
    image: &impl image::GenericImageView<Pixel = image::Rgba<u8>>,
) -> Option<(u32, u32, u32, u32)> {
    let mut bounds = [image.width(), image.height(), 0, 0];
    for (x, y, p) in image.pixels() {
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
    Some((
        bounds[0],
        bounds[1],
        bounds[2] - bounds[0],
        bounds[3] - bounds[1],
    ))
}

pub(super) fn downsize_asset(image: image::DynamicImage) -> image::RgbaImage {
    let (w, h) = (f64::from(image.width()), f64::from(image.height()));
    let max_side = f64::from(SignatureImage::MAX_SIDE);
    let scale = (f64::from(SignatureImage::MAX_PIXELS) / (w * h))
        .sqrt()
        .min(max_side / w)
        .min(max_side / h);
    if scale < 1. {
        image
            .resize_exact(
                (w * scale).floor().max(1.) as u32,
                (h * scale).floor().max(1.) as u32,
                image::imageops::FilterType::Lanczos3,
            )
            .into_rgba8()
    } else {
        image.into_rgba8()
    }
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
    if image.color().has_alpha() {
        let (x, y, width, height) =
            visible_bounds(&image).ok_or("Gambar sepenuhnya transparan.")?;
        if (width, height) != (image.width(), image.height()) {
            image = image.crop_imm(x, y, width, height);
        }
    }
    crop_asset(downsize_asset(image)).ok_or_else(|| "Gambar sepenuhnya transparan.".into())
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
pub(super) fn asset_preview(asset: &SignatureImage) -> Option<Arc<RenderImage>> {
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
    fn placement_is_bounded_and_preserves_aspect_at_all_page_rotations() {
        let asset = SignatureImage {
            width: 4,
            height: 3,
            rgba: vec![255; 48],
        };
        for turns in 0..4 {
            let (width, height) = if turns % 2 == 0 {
                (600., 800.)
            } else {
                (800., 600.)
            };
            let geometry = PageGeometry {
                width_points: width,
                height_points: height,
                quarter_turns: turns,
            };
            for cursor in [[0., 0.], [0.5, 0.5], [1., 1.], [1., 0.]] {
                let point = pdf_domain::annotation::rotate(cursor, (4 - turns) % 4);
                let [a, b] = signature_placement_rect(&asset, geometry, point);
                assert!(a[0] >= 0. && a[1] >= 0. && b[0] <= 1. && b[1] <= 1.);
                let [a, b] = [a, b].map(|p| pdf_domain::annotation::rotate(p, turns));
                let aspect = (b[0] - a[0]).abs() * width / ((b[1] - a[1]).abs() * height);
                let expected = if turns % 2 == 0 { 4. / 3. } else { 3. / 4. };
                assert!((aspect - expected).abs() < 0.001);
            }
        }
    }

    #[test]
    fn keyboard_delta_follows_display_rotation() {
        for turns in 0..4 {
            let d = signature_delta([0.01, 0.], turns);
            let p = pdf_domain::annotation::rotate([0.5 + d[0], 0.5 + d[1]], turns);
            assert!((p[0] - 0.51).abs() < 0.0001 && (p[1] - 0.5).abs() < 0.0001);
        }
    }

    #[test]
    fn signature_transform_preserves_pixels_bounds_and_saved_metadata() {
        let asset = Arc::new(SignatureImage {
            width: 3,
            height: 2,
            rgba: (1..=6).flat_map(|value| [value, 0, 0, 255]).collect(),
        });
        let original = Annotation {
            id: 1,
            highlight: false,
            color: [0; 3],
            width: 1.,
            points: vec![[0.4, 0.3], [0.7, 0.5]],
            rectangles: false,
            signature: Some(asset.clone()),
            text: None,
            extra: None,
        };
        let page = PageGeometry {
            width_points: 600.,
            height_points: 800.,
            quarter_turns: 0,
        };
        let red = |a: &Annotation| {
            a.signature
                .iter()
                .flat_map(|image| image.rgba.iter().step_by(4).copied())
                .collect::<Vec<_>>()
        };
        let mut rotated = original.clone();
        assert!(SignatureTransform::RotateRight.apply(&mut rotated, page));
        assert_eq!(red(&rotated), [4, 1, 5, 2, 6, 3]);
        assert!(Arc::ptr_eq(
            original.signature.as_ref().unwrap_or(&asset),
            &asset
        ));
        let [min, max] = [rotated.points[0], rotated.points[1]];
        assert!(((max[0] - min[0]) * 600. - 160.).abs() < 0.001);
        assert!(((max[1] - min[1]) * 800. - 180.).abs() < 0.001);
        assert_eq!(
            Annotation::from_typed_name(&rotated.name(), true),
            Some(rotated.clone())
        );
        assert!(SignatureTransform::RotateLeft.apply(&mut rotated, page));
        assert_eq!(red(&rotated), red(&original));

        let sideways = PageGeometry {
            width_points: 800.,
            height_points: 600.,
            quarter_turns: 1,
        };
        let mut flipped = original.clone();
        assert!(SignatureTransform::FlipHorizontal.apply(&mut flipped, sideways));
        assert_eq!(red(&flipped), [4, 5, 6, 1, 2, 3]);
        assert!(SignatureTransform::FlipVertical.apply(&mut flipped, sideways));
        assert_eq!(red(&flipped), [6, 5, 4, 3, 2, 1]);
        assert_eq!(flipped.points, original.points);

        let mut wide = original;
        wide.points = vec![[0., 0.7], [1., 1.]];
        let short_page = PageGeometry {
            width_points: 800.,
            height_points: 200.,
            quarter_turns: 0,
        };
        assert!(SignatureTransform::RotateLeft.apply(&mut wide, short_page));
        assert!(wide.valid());
        assert_eq!([wide.points[0][1], wide.points[1][1]], [0., 1.]);
        assert!((wide.points[1][0] - wide.points[0][0] - 0.01875).abs() < 0.001);
    }
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
        assert_eq!(asset.height, 600);
        assert!(asset.width * asset.height <= SignatureImage::MAX_PIXELS);
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
        assert!((513..530).contains(&asset.width));
        assert!((151..170).contains(&asset.height));
        let dir = tempfile::tempdir()?;
        let path = dir.path().join("alpha.png");
        let mut image = image::RgbaImage::new(1200, 800);
        image.put_pixel(1100, 700, image::Rgba([0, 40, 120, 128]));
        image.save(&path)?;
        let imported = load_asset(&path).map_err(anyhow::Error::msg)?;
        assert_eq!((imported.width, imported.height), (1, 1));
        assert_eq!(imported.rgba, [0, 40, 120, 128]);
        let jpeg = dir.path().join("image.jpg");
        image::RgbImage::from_pixel(1500, 600, image::Rgb([255, 255, 255])).save(&jpeg)?;
        let imported = load_asset(&jpeg).map_err(anyhow::Error::msg)?;
        assert!(imported.valid());
        assert_eq!(imported.width, 1024);
        std::fs::write(&path, b"invalid image")?;
        assert!(load_asset(&path).is_err());
        image::RgbaImage::new(10, 10).save(&path)?;
        assert!(load_asset(&path).is_err());
        Ok(())
    }
}
