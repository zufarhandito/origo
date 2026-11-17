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
    pub tool: Tool,
    pub color: usize,
    pub thick: bool,
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
        bar = bar.child(
            Button::new("annotations-toggle")
                .ghost()
                .label("Anotasi")
                .selected(self.annotation.open)
                .disabled(self.edits_locked())
                .on_click(cx.listener(|s, _, _, cx| {
                    s.annotation.open = !s.annotation.open;
                    s.annotation.draft = None;
                    s.annotation.selected = None;
                    s.annotation.tool = Tool::Select;
                    cx.notify();
                })),
        );
        if self.annotation.open {
            for (i, (tool, label)) in [
                (Tool::Select, "Pilih"),
                (Tool::Highlight, "Sorot teks"),
                (Tool::Pen, "Pen"),
                (Tool::Marker, "Stabilo"),
                (Tool::Eraser, "Penghapus"),
            ]
            .into_iter()
            .enumerate()
            {
                bar = bar.child(
                    Button::new(("annotation-tool", i))
                        .ghost()
                        .label(label)
                        .selected(self.annotation.tool == tool)
                        .disabled(self.edits_locked())
                        .on_click(cx.listener(move |s, _, window, cx| {
                            s.annotation.tool = tool;
                            if tool == Tool::Highlight && s.text.selection.is_some() {
                                s.text.dragging = true;
                                s.finish_annotation(cx);
                            }
                            s.annotation.draft = None;
                            s.annotation.selected = None;
                            s.focus.focus(window, cx);
                            cx.notify();
                        })),
                );
            }
            for (i, label) in ["Kuning", "Merah", "Biru", "Hijau", "Hitam"]
                .into_iter()
                .enumerate()
            {
                bar = bar.child(
                    Button::new(("annotation-color", i))
                        .ghost()
                        .label(label)
                        .selected(self.annotation.color == i)
                        .on_click(cx.listener(move |s, _, _, cx| {
                            s.annotation.color = i;
                            s.recolor_annotation(cx);
                            cx.notify();
                        })),
                );
            }
            bar = bar
                .child(
                    Button::new("annotation-width")
                        .ghost()
                        .label(if self.annotation.thick {
                            "Tebal"
                        } else {
                            "Tipis"
                        })
                        .on_click(cx.listener(|s, _, _, cx| {
                            s.annotation.thick = !s.annotation.thick;
                            cx.notify();
                        })),
                )
                .child(
                    Button::new("annotation-delete")
                        .ghost()
                        .label("Hapus anotasi")
                        .disabled(self.annotation.selected.is_none() || self.edits_locked())
                        .on_click(cx.listener(|s, _, _, cx| s.delete_annotation(cx))),
                );
        }
        bar.into_any_element()
    }
    fn set_annotations(
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
    fn add_annotation(&mut self, page: PageId, mut a: Annotation, cx: &mut Context<Self>) {
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
                        if a.id == id {
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
                    color: COLORS[self.annotation.color],
                    width: if highlight {
                        if self.annotation.thick { 24. } else { 12. }
                    } else if self.annotation.thick {
                        4.
                    } else {
                        2.
                    },
                    points: vec![point],
                    rectangles: false,
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
                        color: COLORS[self.annotation.color],
                        width: 1.,
                        points,
                        rectangles: true,
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
    if a.rectangles {
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
