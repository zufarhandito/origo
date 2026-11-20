use crate::*;
use gpui::component::{
    ActiveTheme, Disableable, Selectable,
    button::{Button, ButtonVariants},
};
use pdf_domain::{
    Command, PageId,
    annotation::{Annotation, rotate},
};

#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub(super) enum Tool {
    #[default]
    Select,
    Highlight,
    Pen,
    Marker,
    Eraser,
}
#[derive(Default)]
pub(super) struct AnnotationState {
    pub open: bool,
    pub signature: crate::signatures::SignatureState,
    pub tool: Tool,
    pub color: usize,
    pub pen_color: usize,
    pub marker_color: usize,
    pub text_only: bool,
    pub popup_open: bool,
    pub sliders: Option<[gpui::Entity<gpui::component::slider::SliderState>; 2]>,
    pub draft: Option<(PageId, Annotation)>,
    pub selected: Option<(PageId, u64)>,
}
const COLORS: [[u8; 3]; 5] = [
    [255, 210, 0],
    [230, 65, 75],
    [40, 110, 220],
    [30, 170, 95],
    [25, 25, 25],
];
impl DocumentSession {
    pub(super) fn annotation_popup(&self, open: bool) {
        use std::sync::atomic::Ordering;
        if open {
            self.resources
                .annotation_popup_owner
                .store(self.view_id.0, Ordering::SeqCst);
        } else {
            let _ = self.resources.annotation_popup_owner.compare_exchange(
                self.view_id.0,
                0,
                Ordering::SeqCst,
                Ordering::SeqCst,
            );
        }
    }
    pub(super) fn annotation_popup_changed(&self, open: bool, cx: &mut Context<Self>) {
        if open {
            self.annotation_popup(true);
        } else {
            // Keep canvas input blocked for the outside click that dismissed the popup.
            let resources = self.resources.clone();
            let view = self.view_id.0;
            cx.defer(move |_| {
                let _ = resources.annotation_popup_owner.compare_exchange(
                    view,
                    0,
                    std::sync::atomic::Ordering::SeqCst,
                    std::sync::atomic::Ordering::SeqCst,
                );
            });
        }
    }
    pub(super) fn close_annotation_popups(&mut self) {
        self.annotation_popup(false);
        self.annotation.popup_open = false;
        self.annotation.signature.open = false;
        self.annotation.signature.drawing = false;
        self.annotation.signature.gesture = None;
    }

    pub(super) fn toggle_annotations(&mut self, cx: &mut Context<Self>) {
        if self.edits_locked() {
            return;
        }
        self.close_annotation_popups();
        self.annotation.open = !self.annotation.open;
        self.annotation.draft = None;
        self.annotation.selected = None;
        self.annotation.tool = Tool::Select;
        self.annotation.signature.placing = false;
        self.annotation.signature.gesture = None;
        cx.notify();
    }
    pub(super) fn annotation_bar(&mut self, cx: &mut Context<Self>) -> gpui::AnyElement {
        let mut bar = gpui::div()
            .flex()
            .flex_wrap()
            .items_center()
            .gap_1()
            .px_2()
            .py_1()
            .border_b_1()
            .border_color(cx.theme().border);
        if self.annotation.sliders.is_none() {
            use gpui::component::slider::{SliderEvent, SliderState};
            let sliders = [
                cx.new(|_| {
                    SliderState::new()
                        .min(0.5)
                        .max(12.)
                        .step(0.5)
                        .default_value(2.)
                }),
                cx.new(|_| {
                    SliderState::new()
                        .min(4.)
                        .max(40.)
                        .step(1.)
                        .default_value(12.)
                }),
            ];
            for slider in &sliders {
                cx.subscribe(slider, |_, _, event: &SliderEvent, cx| {
                    if matches!(event, SliderEvent::Change(_)) {
                        cx.notify();
                    }
                })
                .detach();
            }
            self.annotation.sliders = Some(sliders);
        }
        if self.annotation.open {
            use gpui::component::popover::Popover;
            use gpui_kit::assets::IconName;
            for (i, (tool, label, icon)) in [
                (Tool::Select, "Pilih", IconName::MousePointer2),
                (Tool::Highlight, "Highlighter", IconName::Highlighter),
                (Tool::Pen, "Pen", IconName::Pencil),
                (Tool::Eraser, "Penghapus", IconName::Eraser),
            ]
            .into_iter()
            .enumerate()
            {
                let selected = self.annotation.tool == tool
                    || (tool == Tool::Highlight && self.annotation.tool == Tool::Marker);
                let color_index = if tool == Tool::Pen {
                    self.annotation.pen_color
                } else {
                    self.annotation.marker_color
                };
                let c = COLORS[color_index];
                let tint = if matches!(tool, Tool::Pen | Tool::Highlight) {
                    gpui::rgb((u32::from(c[0]) << 16) | (u32::from(c[1]) << 8) | u32::from(c[2]))
                        .into()
                } else {
                    cx.theme().foreground
                };
                bar = bar.child(
                    Button::new(("annotation-tool", i))
                        .ghost()
                        .icon(gpui::component::Icon::new(icon).text_color(tint))
                        .tooltip(label)
                        .accessibility_label(label)
                        .selected(selected)
                        .disabled(self.edits_locked())
                        .on_click(cx.listener(move |s, _, window, cx| {
                            s.annotation.signature.placing = false;
                            s.annotation.signature.gesture = None;
                            s.annotation.tool =
                                if tool == Tool::Highlight && !s.annotation.text_only {
                                    Tool::Marker
                                } else {
                                    tool
                                };
                            s.annotation.color = if tool == Tool::Pen {
                                s.annotation.pen_color
                            } else {
                                s.annotation.marker_color
                            };
                            s.annotation.draft = None;
                            s.annotation.selected = None;
                            s.focus.focus(window, cx);
                            cx.notify();
                        })),
                );
                if matches!(tool, Tool::Pen | Tool::Highlight) {
                    let pen = tool == Tool::Pen;
                    let owner = cx.entity();
                    bar = bar.child(
                        Popover::new(("annotation-options", i))
                            .trigger(
                                Button::new(("annotation-dropdown", i))
                                    .ghost()
                                    .icon(IconName::ChevronDown)
                                    .accessibility_label(if pen {
                                        "Pengaturan pen"
                                    } else {
                                        "Pengaturan highlighter"
                                    })
                                    .tooltip(if pen {
                                        "Pengaturan pen"
                                    } else {
                                        "Pengaturan highlighter"
                                    })
                                    .disabled(self.edits_locked()),
                            )
                            .on_open_change(cx.listener(|s, open, _, cx| {
                                s.annotation_popup_changed(*open, cx);
                                s.annotation.popup_open = *open;
                                s.annotation.draft = None;
                                cx.notify();
                            }))
                            .content(move |_, _, cx| {
                                owner.update(cx, |s, cx| s.annotation_options(pen, cx))
                            }),
                    );
                }
            }
            bar = bar.child(
                Button::new("annotation-delete")
                    .ghost()
                    .icon(IconName::Trash)
                    .accessibility_label("Hapus anotasi")
                    .tooltip("Hapus anotasi")
                    .disabled(self.annotation.selected.is_none() || self.edits_locked())
                    .on_click(cx.listener(|s, _, _, cx| s.delete_annotation(cx))),
            );
        }
        if self.annotation.open {
            bar = bar.child(self.signature_button(cx));
            if self.annotation.selected.is_some_and(|(page, id)| {
                self.workspace.as_ref().is_some_and(|w| {
                    w.annotations(page)
                        .iter()
                        .any(|a| a.id == id && a.signature.is_some())
                })
            }) {
                bar = bar.child(
                    Button::new("duplicate-signature")
                        .ghost()
                        .label("Duplikat tanda tangan")
                        .tooltip("Duplikat: Ctrl+D. Geser: Alt+panah.")
                        .on_click(cx.listener(|s, _, window, cx| {
                            s.change_selected_signature(true, [0.02, 0.02], cx);
                            s.focus.focus(window, cx);
                        })),
                )
            }
        }
        bar.into_any_element()
    }
    fn annotation_options(&mut self, pen: bool, cx: &mut Context<Self>) -> gpui::AnyElement {
        use gpui::component::{slider::Slider, switch::Switch};
        let index = usize::from(!pen);
        let color = if pen {
            self.annotation.pen_color
        } else {
            self.annotation.marker_color
        };
        let Some(sliders) = self.annotation.sliders.as_ref() else {
            return gpui::div().into_any_element();
        };
        let slider = sliders[index].clone();
        let width = slider.read(cx).value().start();
        let mut colors = gpui::div().flex().gap_2();
        for (i, label) in ["Kuning", "Merah", "Biru", "Hijau", "Hitam"]
            .into_iter()
            .enumerate()
        {
            let c = COLORS[i];
            colors = colors.child(
                Button::new(("swatch", i))
                    .ghost()
                    .rounded_full()
                    .accessibility_label(label)
                    .tooltip(label)
                    .selected(color == i)
                    .child(gpui::div().size(px(22.)).rounded_full().bg(gpui::rgb(
                        (u32::from(c[0]) << 16) | (u32::from(c[1]) << 8) | u32::from(c[2]),
                    )))
                    .on_click(cx.listener(move |s, _, _, cx| {
                        if pen {
                            s.annotation.pen_color = i;
                        } else {
                            s.annotation.marker_color = i;
                        }
                        s.annotation.color = i;
                        s.recolor_annotation(cx);
                        cx.notify();
                    })),
            );
        }
        let c = COLORS[color];
        let preview = gpui::canvas(
            |_, _, _| {},
            move |bounds, _, window, _| {
                let mut path = gpui::PathBuilder::stroke(px(width));
                for i in 0..40 {
                    let x = i as f32 / 39.;
                    let point = bounds.origin
                        + gpui::point(
                            bounds.size.width * x,
                            bounds.size.height * (0.5 + 0.18 * (x * std::f32::consts::TAU).sin()),
                        );
                    if i == 0 {
                        path.move_to(point);
                    } else {
                        path.line_to(point);
                    }
                }
                if let Ok(path) = path.build() {
                    window.paint_path(
                        path,
                        gpui::rgba(
                            (u32::from(c[0]) << 24)
                                | (u32::from(c[1]) << 16)
                                | (u32::from(c[2]) << 8)
                                | if pen { 255 } else { 128 },
                        ),
                    );
                }
            },
        )
        .w_full()
        .h(px(64.));
        let mut panel = gpui::div()
            .w(px(240.))
            .flex()
            .flex_col()
            .gap_3()
            .p_3()
            .child("Warna")
            .child(colors)
            .child(preview)
            .child(format!("Ketebalan: {width:.1} pt"))
            .child(
                Slider::new(&slider)
                    .accessibility_label("Ketebalan")
                    .disabled(!pen && self.annotation.text_only),
            )
            .child(
                gpui::div()
                    .flex()
                    .justify_between()
                    .child("Tipis")
                    .child("Tebal"),
            );
        if !pen {
            panel = panel.child(
                Switch::new("text-only")
                    .label("Hanya teks")
                    .checked(self.annotation.text_only)
                    .on_change(cx.listener(|s, checked, _, cx| {
                        s.annotation.text_only = *checked;
                        if matches!(s.annotation.tool, Tool::Highlight | Tool::Marker) {
                            s.annotation.tool = if *checked {
                                Tool::Highlight
                            } else {
                                Tool::Marker
                            };
                        }
                        cx.notify();
                    })),
            );
        }
        panel.into_any_element()
    }
    pub(super) fn set_annotations(
        &mut self,
        page: PageId,
        annotations: Vec<Annotation>,
        cx: &mut Context<Self>,
    ) {
        if self.edits_locked() || self.opening {
            return;
        }
        let result = self
            .workspace
            .as_mut()
            .map(|mut w| w.execute(Command::Annotate { page, annotations }));
        match result {
            Some(Ok(true)) => self.edited(cx),
            Some(Err(e)) => {
                self.status = e.to_string();
                cx.notify();
            }
            _ => {}
        }
    }
    pub(super) fn add_annotation(
        &mut self,
        page: PageId,
        mut a: Annotation,
        cx: &mut Context<Self>,
    ) {
        let Some(mut w) = self.workspace.as_mut() else {
            return;
        };
        let mut all = w.annotations(page).to_vec();
        a.id = w.allocate_annotation_id();
        all.push(a);
        drop(w);
        self.set_annotations(page, all, cx);
    }
    pub(super) fn delete_annotation(&mut self, cx: &mut Context<Self>) {
        if let Some((page, id)) = self.annotation.selected.take() {
            let all = self.workspace.as_ref().map(|w| {
                w.annotations(page)
                    .iter()
                    .filter(|a| a.id != id)
                    .cloned()
                    .collect()
            });
            if let Some(all) = all {
                self.set_annotations(page, all, cx);
            }
        }
    }
    fn recolor_annotation(&mut self, cx: &mut Context<Self>) {
        if let Some((page, id)) = self.annotation.selected {
            let all = self.workspace.as_ref().map(|w| {
                w.annotations(page)
                    .iter()
                    .cloned()
                    .map(|mut a| {
                        if a.id == id && a.signature.is_none() {
                            a.color = COLORS[self.annotation.color];
                        }
                        a
                    })
                    .collect()
            });
            if let Some(all) = all {
                self.set_annotations(page, all, cx);
            }
        }
    }
    pub(super) fn annotation_pointer(
        &mut self,
        index: usize,
        bounds: Bounds<gpui::Pixels>,
        position: gpui::Point<gpui::Pixels>,
        start: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        if !self.annotation.open || self.mode != ViewerMode::Read {
            return false;
        }
        if self.edits_locked() || self.opening {
            return true;
        }
        if self.annotation.tool == Tool::Highlight {
            return false;
        }
        let Some(w) = self.workspace.as_ref() else {
            return true;
        };
        let Some(page) = w.pages().get(index) else {
            return true;
        };
        let id = page.id;
        let Ok(g) = w.geometry(index) else {
            return true;
        };
        let point = rotate(
            [
                (f32::from(position.x - bounds.origin.x) / f32::from(bounds.size.width))
                    .clamp(0., 1.),
                (f32::from(position.y - bounds.origin.y) / f32::from(bounds.size.height))
                    .clamp(0., 1.),
            ],
            (4 - g.quarter_turns) % 4,
        );
        drop(w);
        if self.signature_pointer(id, point, start, cx) {
            if start {
                self.focus.focus(window, cx);
                self.annotation.signature.gesture_bounds = Some(bounds);
            }
            return true;
        }
        let Some(w) = self.workspace.as_ref() else {
            return true;
        };
        let tool = self.annotation.tool;
        if matches!(tool, Tool::Select | Tool::Eraser) {
            if !start {
                return true;
            }
            let hit = w
                .annotations(id)
                .iter()
                .rev()
                .find(|a| hit(a, point, 0.012))
                .map(|a| a.id);
            drop(w);
            self.annotation.selected = hit.map(|a| (id, a));
            if tool == Tool::Eraser {
                self.delete_annotation(cx);
            }
            self.focus.focus(window, cx);
            cx.notify();
            return true;
        }
        drop(w);
        if start {
            let highlight = tool == Tool::Marker;
            self.annotation.draft = Some((
                id,
                Annotation {
                    id: 1,
                    highlight,
                    color: COLORS[if highlight {
                        self.annotation.marker_color
                    } else {
                        self.annotation.pen_color
                    }],
                    width: self
                        .annotation
                        .sliders
                        .as_ref()
                        .map_or(if highlight { 12. } else { 2. }, |sliders| {
                            sliders[usize::from(highlight)].read(cx).value().start()
                        }),
                    points: vec![point],
                    rectangles: false,
                    signature: None,
                },
            ));
            self.annotation.selected = None;
            self.focus.focus(window, cx);
        } else if let Some((page, a)) = &mut self.annotation.draft
            && *page == id
            && a.points.len() < pdf_domain::annotation::MAX_POINTS
            && a.points.last() != Some(&point)
        {
            a.points.push(point);
        }
        cx.notify();
        true
    }
    pub(super) fn finish_annotation(&mut self, cx: &mut Context<Self>) {
        self.annotation.signature.drawing = false;
        if let Some((page, a, _, _)) = self.annotation.signature.gesture.take() {
            let all = self.workspace.as_ref().map(|w| {
                let mut all = w.annotations(page).to_vec();
                if let Some(old) = all.iter_mut().find(|old| old.id == a.id) {
                    *old = a.clone();
                } else {
                    all.push(a.clone());
                }
                all
            });
            if let Some(all) = all {
                self.set_annotations(page, all, cx);
            }
            return;
        }
        if let Some((page, mut a)) = self.annotation.draft.take() {
            if a.points.len() == 1 {
                a.points.push(a.points[0]);
            }
            self.add_annotation(page, a, cx);
            return;
        }
        if !self.annotation.open || self.annotation.tool != Tool::Highlight || !self.text.dragging {
            return;
        }
        let Some(selection) = self.text.selection else {
            return;
        };
        let Some(w) = self.workspace.as_ref() else {
            return;
        };
        let mut changes = Vec::new();
        for (index, page) in w.pages().iter().enumerate() {
            let Some(text) = self.text.pages.get(&index) else {
                continue;
            };
            let Ok(g) = w.geometry(index) else {
                continue;
            };
            let rects = text.highlights(selection.range(index, text.chars.len()));
            let points: Vec<_> = rects
                .iter()
                .flat_map(|r| {
                    [
                        rotate([r.left, r.top], (4 - g.quarter_turns) % 4),
                        rotate([r.right, r.bottom], (4 - g.quarter_turns) % 4),
                    ]
                })
                .collect();
            if !points.is_empty() {
                changes.push((
                    page.id,
                    Annotation {
                        id: 1,
                        highlight: true,
                        color: COLORS[self.annotation.marker_color],
                        width: 1.,
                        points,
                        rectangles: true,
                        signature: None,
                    },
                ));
            }
        }
        drop(w);
        let changes = changes
            .into_iter()
            .filter_map(|(page, mut a)| {
                let mut w = self.workspace.as_mut()?;
                let mut all = w.annotations(page).to_vec();
                a.id = w.allocate_annotation_id();
                all.push(a);
                Some((page, all))
            })
            .collect();
        let result = self
            .workspace
            .as_mut()
            .map(|mut w| w.execute(Command::AnnotateMany(changes)));
        match result {
            Some(Ok(true)) => self.edited(cx),
            Some(Err(e)) => {
                self.status = e.to_string();
                cx.notify();
            }
            _ => {}
        }
    }
}
fn hit(a: &Annotation, p: [f32; 2], tolerance: f32) -> bool {
    if a.rectangles || a.signature.is_some() {
        return a.points.as_chunks::<2>().0.iter().any(|r| {
            p[0] >= r[0][0].min(r[1][0])
                && p[0] <= r[0][0].max(r[1][0])
                && p[1] >= r[0][1].min(r[1][1])
                && p[1] <= r[0][1].max(r[1][1])
        });
    }
    a.points.windows(2).any(|r| {
        let dx = r[1][0] - r[0][0];
        let dy = r[1][1] - r[0][1];
        let l = dx * dx + dy * dy;
        let u = if l > 0. {
            ((p[0] - r[0][0]) * dx + (p[1] - r[0][1]) * dy) / l
        } else {
            0.
        }
        .clamp(0., 1.);
        (p[0] - r[0][0] - u * dx).powi(2) + (p[1] - r[0][1] - u * dy).powi(2)
            <= tolerance * tolerance
    })
}
pub(super) fn paint_draft(
    a: &Annotation,
    turns: u8,
    page_width: f32,
    bounds: Bounds<gpui::Pixels>,
    window: &mut Window,
) {
    let mut path = gpui::PathBuilder::stroke(bounds.size.width * (a.width / page_width));
    for (i, p) in a.points.iter().enumerate() {
        let p = rotate(*p, turns);
        let p = bounds.origin + gpui::point(bounds.size.width * p[0], bounds.size.height * p[1]);
        if i == 0 {
            path.move_to(p);
        } else {
            path.line_to(p);
        }
    }
    if let Ok(path) = path.build() {
        window.paint_path(
            path,
            gpui::rgba(
                (u32::from(a.color[0]) << 24)
                    | (u32::from(a.color[1]) << 16)
                    | (u32::from(a.color[2]) << 8)
                    | if a.highlight { 100 } else { 255 },
            ),
        );
    }
}

pub(super) fn paint_selection(
    a: &Annotation,
    turns: u8,
    bounds: Bounds<gpui::Pixels>,
    window: &mut Window,
) {
    let points: Vec<_> = a.points.iter().map(|p| rotate(*p, turns)).collect();
    let left = points.iter().map(|p| p[0]).fold(1., f32::min);
    let top = points.iter().map(|p| p[1]).fold(1., f32::min);
    let right = points.iter().map(|p| p[0]).fold(0., f32::max);
    let bottom = points.iter().map(|p| p[1]).fold(0., f32::max);
    let origin = bounds.origin + gpui::point(bounds.size.width * left, bounds.size.height * top);
    let width = bounds.size.width * (right - left);
    let height = bounds.size.height * (bottom - top);
    if a.signature.is_some() {
        window.paint_quad(gpui::fill(
            Bounds::new(
                origin + gpui::point(width - px(4.), height - px(4.)),
                gpui::size(px(8.), px(8.)),
            ),
            gpui::rgb(0x2563eb),
        ));
    }
    for (offset, size) in [
        (gpui::point(px(0.), px(0.)), gpui::size(width, px(1.))),
        (gpui::point(px(0.), height), gpui::size(width, px(1.))),
        (gpui::point(px(0.), px(0.)), gpui::size(px(1.), height)),
        (gpui::point(width, px(0.)), gpui::size(px(1.), height)),
    ] {
        window.paint_quad(gpui::fill(
            Bounds::new(origin + offset, size),
            gpui::rgb(0x2563eb),
        ));
    }
}
