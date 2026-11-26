//! Shapes, Sticky Notes, and Quick Fill & Sign symbol tools for the annotation bar.
use crate::*;
use gpui::assets::IconName;
use gpui::component::{
    ActiveTheme, Disableable, Selectable, Sizable, StyledExt,
    button::{Button, ButtonVariants},
    input::{Input, InputEvent, InputState},
    popover::Popover,
};
use pdf_application::annotation::{rasterize_fill_symbol_preview, validate_note};
use pdf_domain::{
    PageGeometry, PageId,
    annotation::{Annotation, AnnotationExtra, FillSymbol, ShapeKind, SignatureImage, rotate},
};

const TOOL_COLORS: [(&str, [u8; 3]); 5] = [
    ("Kuning", [255, 210, 0]),
    ("Merah", [230, 65, 75]),
    ("Biru", [40, 110, 220]),
    ("Hijau", [30, 170, 95]),
    ("Hitam", [25, 25, 25]),
];

const SHAPE_WIDTHS: [f32; 4] = [1.5, 2.5, 4.0, 6.0];

#[derive(Clone)]
pub(super) struct PendingSymbol {
    pub symbol: FillSymbol,
    pub color: [u8; 3],
}

#[derive(Clone)]
pub(super) struct PendingNote {
    pub content: String,
    pub color: [u8; 3],
}

pub(super) struct ExtraToolsState {
    pub shape_open: bool,
    pub shape_kind: ShapeKind,
    pub shape_color: usize,
    pub shape_width: f32,
    pub symbol_open: bool,
    pub symbol_color: usize,
    pub placing_symbol: Option<PendingSymbol>,
    pub note_open: bool,
    pub note_editing: Option<(PageId, u64)>,
    pub note_input: Option<Entity<InputState>>,
    pub note_color: usize,
    pub note_error: Option<String>,
    pub placing_note: Option<PendingNote>,
}

impl Default for ExtraToolsState {
    fn default() -> Self {
        Self {
            shape_open: false,
            shape_kind: ShapeKind::Rectangle,
            shape_color: 1,
            shape_width: 2.5,
            symbol_open: false,
            symbol_color: 4,
            placing_symbol: None,
            note_open: false,
            note_editing: None,
            note_input: None,
            note_color: 0,
            note_error: None,
            placing_note: None,
        }
    }
}

pub(super) fn tool_color(index: usize) -> [u8; 3] {
    TOOL_COLORS[index.min(TOOL_COLORS.len() - 1)].1
}

fn color_index_for(color: [u8; 3]) -> usize {
    TOOL_COLORS
        .iter()
        .position(|(_, c)| *c == color)
        .unwrap_or(0)
}

pub(super) fn square_placement_rect(
    size_points: f32,
    geometry: PageGeometry,
    point: [f32; 2],
) -> [[f32; 2]; 2] {
    let [page_w, page_h] = crate::signatures::unrotated_page_size(geometry);
    let w = (size_points / page_w.max(1.)).clamp(0.015, 0.25);
    let h = (size_points / page_h.max(1.)).clamp(0.015, 0.25);
    let left = (point[0] - w / 2.).clamp(0., (1. - w).max(0.));
    let top = (point[1] - h / 2.).clamp(0., (1. - h).max(0.));
    [[left, top], [(left + w).min(1.), (top + h).min(1.)]]
}

pub(super) fn symbol_placement_rect(geometry: PageGeometry, point: [f32; 2]) -> [[f32; 2]; 2] {
    square_placement_rect(20., geometry, point)
}

pub(super) fn note_placement_rect(geometry: PageGeometry, point: [f32; 2]) -> [[f32; 2]; 2] {
    square_placement_rect(24., geometry, point)
}

pub(super) fn rasterize_note_preview(color: [u8; 3]) -> SignatureImage {
    let size = 48u32;
    let mut rgba = vec![0u8; (size * size * 4) as usize];
    let border = [
        (u16::from(color[0]) * 3 / 5) as u8,
        (u16::from(color[1]) * 3 / 5) as u8,
        (u16::from(color[2]) * 3 / 5) as u8,
    ];
    for y in 0..size {
        for x in 0..size {
            let idx = ((y * size + x) * 4) as usize;
            let on_border = x < 3 || y < 3 || x + 3 >= size || y + 3 >= size;
            let on_line = (14..=16).contains(&y) && (10..38).contains(&x)
                || (23..=25).contains(&y) && (10..38).contains(&x)
                || (32..=34).contains(&y) && (10..28).contains(&x);
            let c = if on_border || on_line { border } else { color };
            rgba[idx] = c[0];
            rgba[idx + 1] = c[1];
            rgba[idx + 2] = c[2];
            rgba[idx + 3] = 240;
        }
    }
    SignatureImage {
        width: size,
        height: size,
        rgba,
    }
}

impl DocumentSession {
    pub(super) fn shape_tool_button(&self, cx: &mut Context<Self>) -> gpui::AnyElement {
        let owner = cx.entity();
        let active = self.annotation.tools.shape_open
            || matches!(self.annotation.tool, crate::annotations::Tool::Shape(_));
        let c = tool_color(self.annotation.tools.shape_color);
        let tint = gpui::rgb((u32::from(c[0]) << 16) | (u32::from(c[1]) << 8) | u32::from(c[2]));
        let icon = match self.annotation.tools.shape_kind {
            ShapeKind::Rectangle => IconName::Square,
            ShapeKind::Ellipse => IconName::Circle,
            ShapeKind::Line => IconName::Minus,
            ShapeKind::Arrow => IconName::ArrowUpRight,
        };
        Popover::new("shape-tool-popup")
            .trigger(
                Button::new("annotation-shape")
                    .ghost()
                    .icon(gpui::component::Icon::new(icon).text_color(tint))
                    .label("Bentuk")
                    .tooltip("Gambar persegi, elips, garis, atau panah")
                    .selected(active)
                    .disabled(self.edits_locked() || self.opening),
            )
            .open(self.annotation.tools.shape_open)
            .on_open_change(cx.listener(|s, open, _, cx| {
                if *open {
                    s.close_annotation_popups(cx);
                    s.annotation.tools.shape_open = true;
                    s.annotation_popup_changed(true, cx);
                } else {
                    s.annotation.tools.shape_open = false;
                    s.annotation_popup_changed(false, cx);
                }
                cx.notify();
            }))
            .content(move |_, _, cx| owner.update(cx, |s, cx| s.shape_tool_panel(cx)))
            .into_any_element()
    }

    fn shape_tool_panel(&mut self, cx: &mut Context<Self>) -> gpui::AnyElement {
        let selected_kind = self.annotation.tools.shape_kind;
        let selected_color = self.annotation.tools.shape_color;
        let selected_width = self.annotation.tools.shape_width;

        let shapes = gpui::div().flex().flex_wrap().gap_1().children(
            [
                (ShapeKind::Rectangle, "Persegi", IconName::Square),
                (ShapeKind::Ellipse, "Elips", IconName::Circle),
                (ShapeKind::Line, "Garis", IconName::Minus),
                (ShapeKind::Arrow, "Panah", IconName::ArrowUpRight),
            ]
            .into_iter()
            .enumerate()
            .map(|(i, (kind, label, icon))| {
                Button::new(("shape-kind", i))
                    .ghost()
                    .small()
                    .icon(icon)
                    .label(label)
                    .selected(
                        self.annotation.tool == crate::annotations::Tool::Shape(kind)
                            || selected_kind == kind,
                    )
                    .on_click(cx.listener(move |s, _, window, cx| {
                        s.cancel_signature_placement(cx);
                        s.annotation.tools.shape_kind = kind;
                        s.annotation.tools.shape_open = false;
                        s.annotation_popup_changed(false, cx);
                        s.annotation.tool = crate::annotations::Tool::Shape(kind);
                        s.annotation.draft = None;
                        s.annotation.selected = None;
                        s.status = format!("Drag di halaman untuk menggambar {label}.");
                        s.focus.focus(window, cx);
                        cx.notify();
                    }))
            }),
        );

        let colors = gpui::div().flex().gap_2().children(
            TOOL_COLORS.into_iter().enumerate().map(|(i, (label, c))| {
                Button::new(("shape-swatch", i))
                    .ghost()
                    .rounded_full()
                    .accessibility_label(label)
                    .tooltip(label)
                    .selected(selected_color == i)
                    .child(gpui::div().size(px(22.)).rounded_full().bg(gpui::rgb(
                        (u32::from(c[0]) << 16) | (u32::from(c[1]) << 8) | u32::from(c[2]),
                    )))
                    .on_click(cx.listener(move |s, _, _, cx| {
                        s.annotation.tools.shape_color = i;
                        s.annotation.color = i;
                        s.recolor_annotation(cx);
                        cx.notify();
                    }))
            }),
        );

        let widths = gpui::div().flex().gap_1().children(
            SHAPE_WIDTHS.into_iter().enumerate().map(|(i, w)| {
                Button::new(("shape-width", i))
                    .ghost()
                    .small()
                    .label(format!("{w:.1} pt"))
                    .selected((selected_width - w).abs() < 0.1)
                    .on_click(cx.listener(move |s, _, _, cx| {
                        s.annotation.tools.shape_width = w;
                        cx.notify();
                    }))
            }),
        );

        gpui::div()
            .w(px(268.))
            .flex()
            .flex_col()
            .gap_3()
            .p_3()
            .child(gpui::div().text_xs().font_semibold().child("Pilih bentuk"))
            .child(shapes)
            .child(gpui::div().text_xs().font_semibold().child("Warna garis"))
            .child(colors)
            .child(gpui::div().text_xs().font_semibold().child("Ketebalan"))
            .child(widths)
            .into_any_element()
    }

    pub(super) fn symbol_tool_button(&self, cx: &mut Context<Self>) -> gpui::AnyElement {
        let owner = cx.entity();
        let active =
            self.annotation.tools.symbol_open || self.annotation.tools.placing_symbol.is_some();
        Popover::new("symbol-tool-popup")
            .trigger(
                Button::new("annotation-symbol")
                    .ghost()
                    .icon(IconName::Check)
                    .label("Simbol")
                    .tooltip("Fill & Sign cepat: Centang, Silang, Titik")
                    .selected(active)
                    .disabled(self.edits_locked() || self.opening),
            )
            .open(self.annotation.tools.symbol_open)
            .on_open_change(cx.listener(|s, open, _, cx| {
                if *open {
                    s.close_annotation_popups(cx);
                    s.annotation.tools.symbol_open = true;
                    s.annotation_popup_changed(true, cx);
                } else {
                    s.annotation.tools.symbol_open = false;
                    s.annotation_popup_changed(false, cx);
                }
                cx.notify();
            }))
            .content(move |_, _, cx| owner.update(cx, |s, cx| s.symbol_tool_panel(cx)))
            .into_any_element()
    }

    fn symbol_tool_panel(&mut self, cx: &mut Context<Self>) -> gpui::AnyElement {
        let selected_color = self.annotation.tools.symbol_color;
        let symbols = gpui::div().flex().gap_1p5().children(
            [
                (FillSymbol::Check, "Centang ✓", IconName::Check),
                (FillSymbol::Cross, "Silang ✗", IconName::X),
                (FillSymbol::Dot, "Titik ●", IconName::Circle),
            ]
            .into_iter()
            .enumerate()
            .map(|(i, (symbol, label, icon))| {
                Button::new(("fill-symbol", i))
                    .outline()
                    .small()
                    .icon(icon)
                    .label(label)
                    .on_click(cx.listener(move |s, _, window, cx| {
                        s.start_symbol_placement(symbol, window, cx);
                    }))
            }),
        );

        let colors = gpui::div().flex().gap_2().children(
            TOOL_COLORS.into_iter().enumerate().map(|(i, (label, c))| {
                Button::new(("symbol-swatch", i))
                    .ghost()
                    .rounded_full()
                    .accessibility_label(label)
                    .tooltip(label)
                    .selected(selected_color == i)
                    .child(gpui::div().size(px(22.)).rounded_full().bg(gpui::rgb(
                        (u32::from(c[0]) << 16) | (u32::from(c[1]) << 8) | u32::from(c[2]),
                    )))
                    .on_click(cx.listener(move |s, _, _, cx| {
                        s.annotation.tools.symbol_color = i;
                        cx.notify();
                    }))
            }),
        );

        gpui::div()
            .w(px(300.))
            .flex()
            .flex_col()
            .gap_3()
            .p_3()
            .child(
                gpui::div()
                    .text_xs()
                    .font_semibold()
                    .child("Simbol Fill & Sign"),
            )
            .child(symbols)
            .child(gpui::div().text_xs().font_semibold().child("Warna simbol"))
            .child(colors)
            .into_any_element()
    }

    pub(super) fn start_symbol_placement(
        &mut self,
        symbol: FillSymbol,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.edits_locked() || self.opening {
            return;
        }
        let color = tool_color(self.annotation.tools.symbol_color);
        let preview = rasterize_fill_symbol_preview(symbol, color);
        self.close_annotation_popups(cx);
        self.annotation.signature.placement_ghost = Some(crate::signatures::ghost_images(&preview));
        self.annotation.tools.placing_symbol = Some(PendingSymbol { symbol, color });
        self.annotation.tool = crate::annotations::Tool::Select;
        self.status =
            "Klik atau drag di halaman untuk menempatkan simbol. Esc membatalkan.".into();
        self.focus.focus(window, cx);
        cx.notify();
    }

    pub(super) fn note_tool_button(&self, cx: &mut Context<Self>) -> gpui::AnyElement {
        let owner = cx.entity();
        let active =
            self.annotation.tools.note_open || self.annotation.tools.placing_note.is_some();
        Popover::new("note-tool-popup")
            .trigger(
                Button::new("annotation-note")
                    .ghost()
                    .icon(IconName::StickyNote)
                    .label("Catatan")
                    .tooltip("Tambah catatan tempel (Sticky Note)")
                    .selected(active)
                    .disabled(self.edits_locked() || self.opening),
            )
            .open(self.annotation.tools.note_open)
            .on_open_change(cx.listener(|s, open, window, cx| {
                if *open {
                    s.open_note_panel(None, window, cx);
                } else {
                    s.dismiss_note_panel(cx);
                }
            }))
            .content(move |_, window, cx| owner.update(cx, |s, cx| s.note_tool_panel(window, cx)))
            .into_any_element()
    }

    pub(super) fn open_note_panel(
        &mut self,
        editing: Option<(PageId, u64)>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.edits_locked() || self.opening {
            return;
        }
        self.close_annotation_popups(cx);
        if self.annotation.tools.note_input.is_none() {
            let input = cx.new(|cx| {
                InputState::new(window, cx).placeholder("Tulis catatan atau komentar...")
            });
            cx.subscribe_in(&input, window, |s, _, event: &InputEvent, window, cx| {
                match event {
                    InputEvent::Change => {
                        s.annotation.tools.note_error = None;
                        cx.notify();
                    }
                    InputEvent::PressEnter { .. } => s.confirm_note(window, cx),
                    _ => {}
                }
            })
            .detach();
            self.annotation.tools.note_input = Some(input);
        }
        self.annotation.tools.note_open = true;
        self.annotation.tools.note_editing = editing;
        self.annotation.tools.note_error = None;
        self.annotation_popup_changed(true, cx);
        if let Some((page, id)) = editing {
            let existing = self.workspace.as_ref().and_then(|w| {
                w.annotations(page)
                    .iter()
                    .find(|a| a.id == id && matches!(a.extra, Some(AnnotationExtra::Note(_))))
                    .cloned()
            });
            if let Some(a) = existing
                && let Some(AnnotationExtra::Note(content)) = a.extra
            {
                self.annotation.tools.note_color = color_index_for(a.color);
                if let Some(input) = self.annotation.tools.note_input.clone() {
                    input.update(cx, |state, cx| {
                        state.set_value(content, window, cx);
                        state.focus(window, cx);
                    });
                }
            }
        } else if let Some(input) = self.annotation.tools.note_input.clone() {
            input.update(cx, |state, cx| state.focus(window, cx));
        }
        cx.notify();
    }

    pub(super) fn dismiss_note_panel(&mut self, cx: &mut Context<Self>) {
        self.annotation.tools.note_open = false;
        self.annotation.tools.note_editing = None;
        self.annotation.tools.note_error = None;
        self.annotation_popup_changed(false, cx);
        cx.notify();
    }

    fn confirm_note(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.edits_locked() || self.opening {
            return;
        }
        let content = self
            .annotation
            .tools
            .note_input
            .as_ref()
            .map(|input| input.read(cx).value().trim().to_owned())
            .unwrap_or_default();
        if let Err(err) = validate_note(&content) {
            self.annotation.tools.note_error = Some(err);
            cx.notify();
            return;
        }
        let color = tool_color(self.annotation.tools.note_color);
        if let Some((page, id)) = self.annotation.tools.note_editing {
            let mut updated = self
                .workspace
                .as_ref()
                .map(|w| w.annotations(page).to_vec());
            if let Some(all) = updated.as_mut()
                && let Some(existing) = all.iter_mut().find(|a| a.id == id)
            {
                existing.color = color;
                existing.extra = Some(AnnotationExtra::Note(content));
                self.dismiss_note_panel(cx);
                self.set_annotations(page, all.clone(), cx);
                self.annotation.selected = Some((page, id));
                self.focus.focus(window, cx);
                cx.notify();
            }
            return;
        }
        let preview = rasterize_note_preview(color);
        self.dismiss_note_panel(cx);
        self.cancel_signature_placement(cx);
        self.annotation.signature.placement_ghost = Some(crate::signatures::ghost_images(&preview));
        self.annotation.tools.placing_note = Some(PendingNote { content, color });
        self.annotation.tool = crate::annotations::Tool::Select;
        self.status =
            "Klik atau drag di halaman untuk menempelkan catatan. Esc membatalkan.".into();
        self.focus.focus(window, cx);
        cx.notify();
    }

    fn note_tool_panel(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> gpui::AnyElement {
        let editing = self.annotation.tools.note_editing.is_some();
        let selected_color = self.annotation.tools.note_color;
        let error = self.annotation.tools.note_error.clone();

        let colors = gpui::div().flex().gap_2().children(
            TOOL_COLORS.into_iter().enumerate().map(|(i, (label, c))| {
                Button::new(("note-swatch", i))
                    .ghost()
                    .rounded_full()
                    .accessibility_label(label)
                    .tooltip(label)
                    .selected(selected_color == i)
                    .child(gpui::div().size(px(22.)).rounded_full().bg(gpui::rgb(
                        (u32::from(c[0]) << 16) | (u32::from(c[1]) << 8) | u32::from(c[2]),
                    )))
                    .on_click(cx.listener(move |s, _, _, cx| {
                        s.annotation.tools.note_color = i;
                        cx.notify();
                    }))
            }),
        );

        let mut panel = gpui::div()
            .w(px(290.))
            .flex()
            .flex_col()
            .gap_3()
            .p_3()
            .child(gpui::div().text_xs().font_semibold().child(if editing {
                "Edit catatan tempel"
            } else {
                "Catatan tempel (Sticky Note)"
            }));
        if let Some(input) = &self.annotation.tools.note_input {
            panel = panel.child(Input::new(input).aria_label("Isi catatan"));
        }
        panel = panel
            .child(gpui::div().text_xs().font_semibold().child("Warna ikon"))
            .child(colors);
        if let Some(err) = error {
            panel = panel.child(gpui::div().text_xs().text_color(cx.theme().danger).child(err));
        }
        panel
            .child(
                gpui::div()
                    .flex()
                    .justify_end()
                    .gap_2()
                    .child(Button::new("note-cancel").small().label("Batal").on_click(
                        cx.listener(|s, _, _, cx| s.dismiss_note_panel(cx)),
                    ))
                    .child(
                        Button::new("note-confirm")
                            .primary()
                            .small()
                            .label(if editing { "Simpan" } else { "Tempatkan" })
                            .on_click(cx.listener(|s, _, window, cx| s.confirm_note(window, cx))),
                    ),
            )
            .into_any_element()
    }
}

pub(super) fn paint_extra_draft(
    a: &Annotation,
    turns: u8,
    page_width: f32,
    bounds: Bounds<gpui::Pixels>,
    window: &mut Window,
) {
    let Some(AnnotationExtra::Shape(kind)) = &a.extra else {
        return;
    };
    if a.points.len() < 2 {
        return;
    }
    let to_screen = |p: [f32; 2]| {
        let p = rotate(p, turns);
        bounds.origin + gpui::point(bounds.size.width * p[0], bounds.size.height * p[1])
    };
    let p0 = to_screen(a.points[0]);
    let p1 = to_screen(a.points[1]);
    let stroke = (bounds.size.width * (a.width / page_width.max(1.))).max(px(1.5));
    let color = gpui::rgba(
        (u32::from(a.color[0]) << 24)
            | (u32::from(a.color[1]) << 16)
            | (u32::from(a.color[2]) << 8)
            | 235,
    );

    let mut path = gpui::PathBuilder::stroke(stroke);
    match kind {
        ShapeKind::Rectangle => {
            let x0 = p0.x.min(p1.x);
            let y0 = p0.y.min(p1.y);
            let x1 = p0.x.max(p1.x);
            let y1 = p0.y.max(p1.y);
            path.move_to(gpui::point(x0, y0));
            path.line_to(gpui::point(x1, y0));
            path.line_to(gpui::point(x1, y1));
            path.line_to(gpui::point(x0, y1));
            path.line_to(gpui::point(x0, y0));
        }
        ShapeKind::Ellipse => {
            let cx = (p0.x + p1.x) * 0.5;
            let cy = (p0.y + p1.y) * 0.5;
            let rx = (p1.x - p0.x).abs() * 0.5;
            let ry = (p1.y - p0.y).abs() * 0.5;
            const STEPS: usize = 48;
            for i in 0..=STEPS {
                let t = (i as f32 / STEPS as f32) * std::f32::consts::TAU;
                let pt = gpui::point(cx + rx * t.cos(), cy + ry * t.sin());
                if i == 0 {
                    path.move_to(pt);
                } else {
                    path.line_to(pt);
                }
            }
        }
        ShapeKind::Line | ShapeKind::Arrow => {
            path.move_to(p0);
            path.line_to(p1);
            if *kind == ShapeKind::Arrow {
                let dx = f32::from(p1.x - p0.x);
                let dy = f32::from(p1.y - p0.y);
                let len = dx.hypot(dy);
                if len > 2. {
                    let head = (f32::from(stroke) * 4.5).clamp(8., len * 0.35);
                    let ux = dx / len;
                    let uy = dy / len;
                    let px_u = -uy;
                    let py_u = ux;
                    let back = head * 0.82;
                    let spread = head * 0.52;
                    let w1 = gpui::point(
                        p1.x - px(ux * back - px_u * spread),
                        p1.y - px(uy * back - py_u * spread),
                    );
                    let w2 = gpui::point(
                        p1.x - px(ux * back + px_u * spread),
                        p1.y - px(uy * back + py_u * spread),
                    );
                    path.move_to(w1);
                    path.line_to(p1);
                    path.line_to(w2);
                }
            }
        }
    }
    if let Ok(built) = path.build() {
        window.paint_path(built, color);
    }
}
