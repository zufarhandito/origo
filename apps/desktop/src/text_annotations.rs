//! Passive text/date/name annotations and multi-page apply workflow.
use crate::*;
use gpui::StyledImage;
use gpui::assets::IconName;
use gpui::component::{
    ActiveTheme, Disableable, Selectable, Sizable, StyledExt,
    button::{Button, ButtonVariants},
    input::{Input, InputEvent, InputState},
    popover::Popover,
};
use pdf_application::annotation::{
    measure_text_points, parse_page_targets, rasterize_text, validate_text,
};
use pdf_domain::{
    Command, PageGeometry, PageId,
    annotation::{Annotation, MAX_PAGE_ANNOTATIONS, TextAnnotation, within_signature_budget},
};

const TEXT_SIZES: [f32; 5] = [10., 12., 14., 18., 24.];

#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub(super) enum TextPreset {
    #[default]
    Free,
    Date,
    Name,
}

#[derive(Clone)]
pub(super) struct PendingText {
    pub content: String,
    pub font_size: f32,
    pub color: [u8; 3],
}

pub(super) struct TextAnnotationState {
    pub open: bool,
    pub editing: Option<(PageId, u64)>,
    pub input: Option<Entity<InputState>>,
    pub preset: TextPreset,
    pub font_size: f32,
    pub color: usize,
    pub last_name: String,
    pub preview: Option<Arc<RenderImage>>,
    pub error: Option<String>,
    pub placing: Option<PendingText>,
}

impl Default for TextAnnotationState {
    fn default() -> Self {
        Self {
            open: false,
            editing: None,
            input: None,
            preset: TextPreset::Free,
            font_size: 12.,
            color: 0,
            last_name: String::new(),
            preview: None,
            error: None,
            placing: None,
        }
    }
}

#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub(super) enum ApplyPagesMode {
    #[default]
    All,
    Selected,
    Range,
}

#[derive(Default)]
pub(super) struct ApplyPagesState {
    pub open: bool,
    pub mode: ApplyPagesMode,
    pub range_input: Option<Entity<InputState>>,
}

pub(super) fn today_indonesian() -> String {
    use chrono::{Datelike, Local};
    const MONTHS: [&str; 12] = [
        "Januari",
        "Februari",
        "Maret",
        "April",
        "Mei",
        "Juni",
        "Juli",
        "Agustus",
        "September",
        "Oktober",
        "November",
        "Desember",
    ];
    let today = Local::now().date_naive();
    let month = MONTHS[usize::try_from(today.month0()).unwrap_or(0).min(11)];
    format!("{} {} {}", today.day(), month, today.year())
}

pub(super) fn text_box_size(
    content: &str,
    font_size: f32,
    geometry: PageGeometry,
) -> Option<[f32; 2]> {
    let [pts_w, pts_h] = measure_text_points(content, font_size)?;
    let [page_w, page_h] = crate::signatures::unrotated_page_size(geometry);
    if page_w <= 0. || page_h <= 0. {
        return None;
    }
    let mut width = pts_w / page_w;
    let mut height = pts_h / page_h;
    let scale = (0.96 / width).min(0.5 / height).min(1.);
    width *= scale;
    height *= scale;
    Some([width.max(0.01), height.max(0.01)])
}

pub(super) fn text_placement_rect(
    content: &str,
    font_size: f32,
    geometry: PageGeometry,
    point: [f32; 2],
) -> Option<[[f32; 2]; 2]> {
    let [width, height] = text_box_size(content, font_size, geometry)?;
    let left = (point[0] - width / 2.).clamp(0., (1. - width).max(0.));
    let top = (point[1] - height / 2.).clamp(0., (1. - height).max(0.));
    Some([
        [left, top],
        [(left + width).min(1.), (top + height).min(1.)],
    ])
}

fn text_edit_rect(
    origin: [f32; 2],
    content: &str,
    font_size: f32,
    geometry: PageGeometry,
) -> Option<[[f32; 2]; 2]> {
    let [width, height] = text_box_size(content, font_size, geometry)?;
    let left = origin[0].clamp(0., (1. - width).max(0.));
    let top = origin[1].clamp(0., (1. - height).max(0.));
    Some([
        [left, top],
        [(left + width).min(1.), (top + height).min(1.)],
    ])
}

fn color_index_for(color: [u8; 3]) -> usize {
    (0..3)
        .find(|&index| crate::signatures::signature_color(index) == color)
        .unwrap_or(0)
}

impl DocumentSession {
    pub(super) fn text_annotation_button(&self, cx: &mut Context<Self>) -> gpui::AnyElement {
        let owner = cx.entity();
        let active = self.annotation.text.open || self.annotation.text.placing.is_some();
        Popover::new("text-annotation-popup")
            .trigger(
                Button::new("text-annotation")
                    .ghost()
                    .icon(IconName::Type)
                    .label("Teks")
                    .tooltip("Tambah teks, tanggal, atau nama (pasif)")
                    .selected(active)
                    .disabled(self.edits_locked() || self.opening),
            )
            .open(self.annotation.text.open)
            .on_open_change(cx.listener(|s, open, window, cx| {
                if *open {
                    s.open_text_annotation_panel(None, window, cx);
                } else {
                    s.dismiss_text_annotation_panel(cx);
                }
            }))
            .content(move |_, window, cx| {
                owner.update(cx, |s, cx| s.text_annotation_panel(window, cx))
            })
            .into_any_element()
    }

    pub(super) fn open_text_annotation_panel(
        &mut self,
        editing: Option<(PageId, u64)>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.edits_locked() || self.opening {
            return;
        }
        self.close_annotation_popups(cx);
        self.init_text_annotation_input(window, cx);
        self.annotation.text.open = true;
        self.annotation.text.editing = editing;
        self.annotation.draft = None;
        self.annotation_popup_changed(true, cx);
        if let Some((page, id)) = editing {
            let existing = self.workspace.as_ref().and_then(|w| {
                w.annotations(page)
                    .iter()
                    .find(|a| a.id == id && a.text.is_some())
                    .cloned()
            });
            if let Some(a) = existing
                && let Some(text) = a.text
            {
                self.annotation.text.preset = TextPreset::Free;
                self.annotation.text.font_size = text.font_size;
                self.annotation.text.color = color_index_for(a.color);
                if let Some(input) = self.annotation.text.input.clone() {
                    input.update(cx, |state, cx| {
                        state.set_value(text.content, window, cx);
                        state.focus(window, cx);
                    });
                }
            }
        } else if let Some(input) = self.annotation.text.input.clone() {
            input.update(cx, |state, cx| state.focus(window, cx));
        }
        self.refresh_text_annotation_preview(cx);
    }

    pub(super) fn dismiss_text_annotation_panel(&mut self, cx: &mut Context<Self>) {
        self.annotation.text.open = false;
        self.annotation.text.editing = None;
        self.annotation.text.error = None;
        self.annotation_popup_changed(false, cx);
        cx.notify();
    }

    fn init_text_annotation_input(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.annotation.text.input.is_some() {
            return;
        }
        let input =
            cx.new(|cx| InputState::new(window, cx).placeholder("Ketik teks, tanggal, atau nama"));
        cx.subscribe_in(
            &input,
            window,
            |s, _, event: &InputEvent, window, cx| match event {
                InputEvent::Change => s.refresh_text_annotation_preview(cx),
                InputEvent::PressEnter { .. } => s.confirm_text_annotation(window, cx),
                _ => {}
            },
        )
        .detach();
        self.annotation.text.input = Some(input);
    }

    fn refresh_text_annotation_preview(&mut self, cx: &mut Context<Self>) {
        let value = self
            .annotation
            .text
            .input
            .as_ref()
            .map(|input| input.read(cx).value().to_string())
            .unwrap_or_default();
        let font_size = self.annotation.text.font_size;
        let color = crate::signatures::signature_color(self.annotation.text.color);
        if let Some(old) = self.annotation.text.preview.take() {
            self.release_images(vec![old], cx);
        }
        self.annotation.text.error = None;
        if !value.trim().is_empty() {
            match validate_text(&value) {
                Ok(()) => {
                    self.annotation.text.preview = rasterize_text(&value, font_size, color)
                        .as_ref()
                        .and_then(crate::signatures::asset_preview);
                }
                Err(error) => {
                    self.annotation.text.error = Some(error);
                }
            }
        }
        cx.notify();
    }

    fn apply_text_preset(
        &mut self,
        preset: TextPreset,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.annotation.text.preset = preset;
        let next_value = match preset {
            TextPreset::Free => None,
            TextPreset::Date => Some(today_indonesian()),
            TextPreset::Name => {
                if !self.annotation.text.last_name.trim().is_empty() {
                    Some(self.annotation.text.last_name.clone())
                } else {
                    self.annotation
                        .signature
                        .text
                        .as_ref()
                        .map(|input| input.read(cx).value().trim().to_owned())
                        .filter(|value| !value.is_empty())
                }
            }
        };
        if let Some(input) = self.annotation.text.input.clone() {
            input.update(cx, |state, cx| {
                if let Some(value) = next_value {
                    state.set_value(value, window, cx);
                }
                state.focus(window, cx);
            });
        }
        self.refresh_text_annotation_preview(cx);
    }

    fn confirm_text_annotation(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.edits_locked() || self.opening {
            return;
        }
        let content = self
            .annotation
            .text
            .input
            .as_ref()
            .map(|input| input.read(cx).value().trim().to_owned())
            .unwrap_or_default();
        if let Err(error) = validate_text(&content) {
            self.annotation.text.error = Some(error);
            cx.notify();
            return;
        }
        let font_size = self.annotation.text.font_size;
        let color = crate::signatures::signature_color(self.annotation.text.color);
        if self.annotation.text.preset == TextPreset::Name {
            self.annotation.text.last_name = content.clone();
        }
        if let Some((page, id)) = self.annotation.text.editing {
            let geometry = self.workspace.as_ref().and_then(|w| {
                w.pages()
                    .iter()
                    .position(|p| p.id == page)
                    .and_then(|index| w.geometry(index).ok())
            });
            let mut updated = self
                .workspace
                .as_ref()
                .map(|w| w.annotations(page).to_vec());
            if let (Some(geometry), Some(all)) = (geometry, updated.as_mut())
                && let Some(existing) = all.iter_mut().find(|a| a.id == id && a.text.is_some())
                && let Some(rect) =
                    text_edit_rect(existing.points[0], &content, font_size, geometry)
            {
                existing.color = color;
                existing.points = rect.to_vec();
                existing.text = Some(TextAnnotation { content, font_size });
                self.dismiss_text_annotation_panel(cx);
                self.set_annotations(page, all.clone(), cx);
                self.annotation.selected = Some((page, id));
                self.focus.focus(window, cx);
                cx.notify();
            }
            return;
        }
        let Some(bitmap) = rasterize_text(&content, font_size, color) else {
            self.annotation.text.error = Some("Teks tidak dapat dirender.".into());
            cx.notify();
            return;
        };
        self.dismiss_text_annotation_panel(cx);
        self.annotation_popup(false);
        self.cancel_signature_placement(cx);
        self.annotation.signature.placement_ghost = Some(crate::signatures::ghost_images(&bitmap));
        self.annotation.text.placing = Some(PendingText {
            content,
            font_size,
            color,
        });
        self.annotation.tool = crate::annotations::Tool::Select;
        self.status = "Klik atau drag di halaman untuk menempatkan teks. Esc membatalkan.".into();
        self.focus.focus(window, cx);
        cx.notify();
    }

    fn text_annotation_panel(
        &mut self,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
        let state = &self.annotation.text;
        let editing = state.editing.is_some();
        let theme = cx.theme().colors;
        let usable = !self.opening
            && !self.edits_locked()
            && state.preview.is_some()
            && state.error.is_none();

        let presets = gpui::div()
            .flex()
            .w_full()
            .gap_1()
            .p_1()
            .rounded_lg()
            .bg(theme.muted)
            .children(
                [
                    (TextPreset::Free, "Teks"),
                    (TextPreset::Date, "Tanggal"),
                    (TextPreset::Name, "Nama"),
                ]
                .into_iter()
                .map(|(preset, label)| {
                    Button::new(("text-preset", preset as usize))
                        .flex_1()
                        .small()
                        .label(label)
                        .selected(state.preset == preset)
                        .disabled(self.opening)
                        .on_click(cx.listener(move |s, _, window, cx| {
                            s.apply_text_preset(preset, window, cx);
                        }))
                }),
            );

        let sizes = gpui::div()
            .flex()
            .items_center()
            .gap_1()
            .child(gpui::div().mr_1().text_sm().child("Ukuran"))
            .children(TEXT_SIZES.into_iter().enumerate().map(|(i, size)| {
                let selected = (state.font_size - size).abs() < 0.25;
                Button::new(("text-size", i))
                    .small()
                    .label(format!("{}", size as u32))
                    .accessibility_label(format!("{} pt", size as u32))
                    .selected(selected)
                    .on_click(cx.listener(move |s, _, _, cx| {
                        s.annotation.text.font_size = size;
                        s.refresh_text_annotation_preview(cx);
                    }))
            }));

        let swatches = gpui::div()
            .flex()
            .items_center()
            .gap_1()
            .child(gpui::div().mr_1().text_sm().child("Warna"))
            .children(
                ["Hitam", "Biru", "Merah"]
                    .into_iter()
                    .enumerate()
                    .map(|(i, label)| {
                        let [r, g, b] = crate::signatures::signature_color(i);
                        Button::new(("text-color", i))
                            .ghost()
                            .rounded_full()
                            .accessibility_label(format!("Warna teks {label}"))
                            .tooltip(label)
                            .selected(state.color == i)
                            .child(
                                gpui::div()
                                    .size(px(18.))
                                    .rounded_full()
                                    .bg(gpui::rgb(u32::from_be_bytes([0, r, g, b]))),
                            )
                            .on_click(cx.listener(move |s, _, _, cx| {
                                s.annotation.text.color = i;
                                s.refresh_text_annotation_preview(cx);
                            }))
                    }),
            );

        let preview = gpui::div()
            .id("text-annotation-preview")
            .w_full()
            .h(px(64.))
            .px_3()
            .py_2()
            .flex()
            .items_center()
            .justify_center()
            .rounded_lg()
            .border_1()
            .border_color(theme.border)
            .bg(gpui::rgb(0xf8fafc))
            .when_some(state.preview.clone(), |el, image| {
                el.child(
                    gpui::img(image)
                        .max_w_full()
                        .max_h_full()
                        .object_fit(gpui::ObjectFit::Contain),
                )
            })
            .when(state.preview.is_none(), |el| {
                el.child(
                    gpui::div()
                        .text_xs()
                        .text_color(theme.muted_foreground)
                        .child("Pratinjau teks"),
                )
            });

        let mut body = gpui::div()
            .flex()
            .flex_col()
            .gap_3()
            .p_3()
            .child(presets)
            .when_some(state.input.as_ref(), |el, input| {
                el.child(Input::new(input).aria_label("Isi anotasi teks"))
            })
            .child(preview)
            .child(
                gpui::div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap_2()
                    .child(sizes),
            )
            .child(swatches);
        if let Some(error) = &state.error {
            body = body.child(
                gpui::div()
                    .text_xs()
                    .text_color(cx.theme().danger)
                    .child(error.clone()),
            );
        }

        gpui::div()
            .w(px(360.))
            .max_w_full()
            .flex()
            .flex_col()
            .child(
                gpui::div()
                    .px_3()
                    .py_2()
                    .border_b_1()
                    .border_color(theme.border)
                    .flex()
                    .flex_col()
                    .gap_0p5()
                    .child(gpui::div().text_base().font_semibold().child(if editing {
                        "Edit Anotasi Teks"
                    } else {
                        "Anotasi Teks"
                    }))
                    .child(
                        gpui::div()
                            .text_xs()
                            .text_color(theme.muted_foreground)
                            .child("Teks pasif (bukan field form interaktif)"),
                    ),
            )
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
                    .border_color(theme.border)
                    .child(
                        Button::new("text-annotation-cancel")
                            .outline()
                            .icon(IconName::X)
                            .label("Batal")
                            .accessibility_label("Batal anotasi teks")
                            .on_click(cx.listener(|s, _, window, cx| {
                                s.dismiss_text_annotation_panel(cx);
                                s.focus.focus(window, cx);
                            })),
                    )
                    .child(
                        Button::new("text-annotation-confirm")
                            .primary()
                            .icon(IconName::Check)
                            .label(if editing {
                                "Simpan Perubahan"
                            } else {
                                "Tempatkan Teks"
                            })
                            .disabled(!usable)
                            .on_click(cx.listener(|s, _, window, cx| {
                                s.confirm_text_annotation(window, cx);
                            })),
                    ),
            )
            .into_any_element()
    }

    pub(super) fn apply_pages_button(&self, cx: &mut Context<Self>) -> gpui::AnyElement {
        let owner = cx.entity();
        let page_count = self.workspace.as_ref().map_or(0, |w| w.pages().len());
        Popover::new("apply-pages-popup")
            .trigger(
                Button::new("apply-pages")
                    .ghost()
                    .icon(IconName::Copy)
                    .label("Terapkan ke halaman")
                    .tooltip("Salin anotasi ini ke banyak halaman (1 langkah Undo)")
                    .disabled(self.edits_locked() || self.opening || page_count <= 1),
            )
            .open(self.annotation.apply_pages.open)
            .on_open_change(cx.listener(|s, open, window, cx| {
                if *open {
                    s.open_apply_pages_panel(window, cx);
                } else {
                    s.dismiss_apply_pages_panel(cx);
                }
            }))
            .content(move |_, window, cx| owner.update(cx, |s, cx| s.apply_pages_panel(window, cx)))
            .into_any_element()
    }

    fn open_apply_pages_panel(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.edits_locked() || self.opening {
            return;
        }
        let selected = self.annotation.selected;
        self.close_annotation_popups(cx);
        self.annotation.selected = selected;
        self.init_apply_pages_input(window, cx);
        self.annotation.apply_pages.open = true;
        self.annotation_popup_changed(true, cx);
        cx.notify();
    }

    pub(super) fn dismiss_apply_pages_panel(&mut self, cx: &mut Context<Self>) {
        self.annotation.apply_pages.open = false;
        self.annotation_popup_changed(false, cx);
        cx.notify();
    }

    fn init_apply_pages_input(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.annotation.apply_pages.range_input.is_some() {
            return;
        }
        let input = cx.new(|cx| InputState::new(window, cx).placeholder("Contoh: 1-3, 5, 8"));
        cx.subscribe_in(
            &input,
            window,
            |s, _, event: &InputEvent, window, cx| match event {
                InputEvent::Change => cx.notify(),
                InputEvent::PressEnter { .. } => s.execute_apply_pages(window, cx),
                _ => {}
            },
        )
        .detach();
        self.annotation.apply_pages.range_input = Some(input);
    }

    fn resolve_apply_pages_targets(
        &self,
        cx: &App,
    ) -> Result<(PageId, Annotation, Vec<PageId>), String> {
        let Some((source_page, id)) = self.annotation.selected else {
            return Err("Pilih tanda tangan, stempel, atau teks terlebih dahulu.".into());
        };
        let Some(w) = self.workspace.as_ref() else {
            return Err("Tidak ada dokumen terbuka.".into());
        };
        let Some(source_index) = w.pages().iter().position(|p| p.id == source_page) else {
            return Err("Halaman asal tidak ditemukan.".into());
        };
        let Some(source_annotation) = w
            .annotations(source_page)
            .iter()
            .find(|a| a.id == id && a.is_box_object())
            .cloned()
        else {
            return Err("Pilih tanda tangan, stempel, atau teks terlebih dahulu.".into());
        };
        let page_count = w.pages().len();
        let target_indices = match self.annotation.apply_pages.mode {
            ApplyPagesMode::All => (0..page_count)
                .filter(|&index| index != source_index)
                .collect::<Vec<_>>(),
            ApplyPagesMode::Selected => {
                let selected = w.selected();
                let indices: Vec<_> = w
                    .pages()
                    .iter()
                    .enumerate()
                    .filter(|&(index, page)| index != source_index && selected.contains(&page.id))
                    .map(|(index, _)| index)
                    .collect();
                if indices.is_empty() {
                    return Err(
                        "Belum ada halaman lain yang dipilih di sidebar atau mode Atur Halaman."
                            .into(),
                    );
                }
                indices
            }
            ApplyPagesMode::Range => {
                let spec = self
                    .annotation
                    .apply_pages
                    .range_input
                    .as_ref()
                    .map(|input| input.read(cx).value().to_string())
                    .unwrap_or_default();
                parse_page_targets(&spec, page_count, Some(source_index))?
            }
        };
        if target_indices.is_empty() {
            return Err("Tidak ada halaman tujuan selain halaman asal.".into());
        }
        let target_ids: Vec<_> = target_indices
            .iter()
            .map(|&index| w.pages()[index].id)
            .collect();
        for &target_id in &target_ids {
            if w.annotations(target_id).len() >= MAX_PAGE_ANNOTATIONS {
                return Err(format!(
                    "Salah satu halaman tujuan sudah mencapai batas {MAX_PAGE_ANNOTATIONS} anotasi."
                ));
            }
        }
        if source_annotation.signature.is_some() {
            let simulated = w
                .pages()
                .iter()
                .flat_map(|p| w.annotations(p.id).iter())
                .chain(std::iter::repeat_n(&source_annotation, target_ids.len()));
            if !within_signature_budget(simulated) {
                return Err(
                    "Melebihi batas penempatan tanda tangan/stempel pada dokumen ini.".into(),
                );
            }
        }
        Ok((source_page, source_annotation, target_ids))
    }

    fn execute_apply_pages(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.edits_locked() || self.opening {
            return;
        }
        let Ok((_, source_annotation, target_ids)) = self.resolve_apply_pages_targets(cx) else {
            return;
        };
        let count = target_ids.len();
        let Some(mut w) = self.workspace.as_mut() else {
            return;
        };
        let mut changes = Vec::with_capacity(count);
        for target_id in target_ids {
            let mut all = w.annotations(target_id).to_vec();
            let mut copy = source_annotation.clone();
            copy.id = w.allocate_annotation_id();
            all.push(copy);
            changes.push((target_id, all));
        }
        let result = w.execute(Command::AnnotateMany(changes));
        drop(w);
        self.dismiss_apply_pages_panel(cx);
        match result {
            Ok(true) => {
                self.status = format!("Diterapkan ke {count} halaman. Ctrl+Z untuk membatalkan.");
                self.edited(cx);
            }
            Err(error) => {
                self.status = error.to_string();
                cx.notify();
            }
            Ok(false) => {}
        }
        self.focus.focus(window, cx);
    }

    fn apply_pages_panel(
        &mut self,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
        let theme = cx.theme().colors;
        let state = &self.annotation.apply_pages;
        let source_page_number = self
            .annotation
            .selected
            .and_then(|(page, _)| {
                self.workspace
                    .as_ref()
                    .and_then(|w| w.pages().iter().position(|p| p.id == page))
            })
            .map(|index| index + 1);
        let resolution = self.resolve_apply_pages_targets(cx);
        let usable = !self.opening && !self.edits_locked() && resolution.is_ok();

        let modes = gpui::div()
            .flex()
            .w_full()
            .gap_1()
            .p_1()
            .rounded_lg()
            .bg(theme.muted)
            .children(
                [
                    (ApplyPagesMode::All, "Semua"),
                    (ApplyPagesMode::Selected, "Terpilih"),
                    (ApplyPagesMode::Range, "Rentang"),
                ]
                .into_iter()
                .map(|(mode, label)| {
                    Button::new(("apply-pages-mode", mode as usize))
                        .flex_1()
                        .small()
                        .label(label)
                        .selected(state.mode == mode)
                        .on_click(cx.listener(move |s, _, window, cx| {
                            s.annotation.apply_pages.mode = mode;
                            if mode == ApplyPagesMode::Range
                                && let Some(input) = s.annotation.apply_pages.range_input.clone()
                            {
                                input.update(cx, |state, cx| state.focus(window, cx));
                            }
                            cx.notify();
                        }))
                }),
            );

        let mut body = gpui::div().flex().flex_col().gap_3().p_3().child(modes);

        if state.mode == ApplyPagesMode::Range
            && let Some(input) = &state.range_input
        {
            body = body.child(Input::new(input).aria_label("Rentang halaman tujuan"));
        }

        body = match &resolution {
            Ok((_, _, targets)) => body.child(
                gpui::div()
                    .text_xs()
                    .text_color(theme.muted_foreground)
                    .child(format!(
                        "Akan ditambahkan ke {} halaman di posisi yang sama. 1 langkah Undo.",
                        targets.len()
                    )),
            ),
            Err(message) => body.child(
                gpui::div()
                    .text_xs()
                    .text_color(cx.theme().danger)
                    .child(message.clone()),
            ),
        };

        gpui::div()
            .w(px(340.))
            .max_w_full()
            .flex()
            .flex_col()
            .child(
                gpui::div()
                    .px_3()
                    .py_2()
                    .border_b_1()
                    .border_color(theme.border)
                    .flex()
                    .flex_col()
                    .gap_0p5()
                    .child(
                        gpui::div()
                            .text_base()
                            .font_semibold()
                            .child("Terapkan ke Banyak Halaman"),
                    )
                    .when_some(source_page_number, |el, page| {
                        el.child(
                            gpui::div()
                                .text_xs()
                                .text_color(theme.muted_foreground)
                                .child(format!(
                                    "Menyalin dari halaman {page} tanpa menimpa halaman asal"
                                )),
                        )
                    }),
            )
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
                    .border_color(theme.border)
                    .child(
                        Button::new("apply-pages-cancel")
                            .outline()
                            .icon(IconName::X)
                            .label("Batal")
                            .accessibility_label("Batal terapkan halaman")
                            .on_click(cx.listener(|s, _, window, cx| {
                                s.dismiss_apply_pages_panel(cx);
                                s.focus.focus(window, cx);
                            })),
                    )
                    .child(
                        Button::new("apply-pages-confirm")
                            .primary()
                            .icon(IconName::Check)
                            .label("Terapkan")
                            .disabled(!usable)
                            .on_click(cx.listener(|s, _, window, cx| {
                                s.execute_apply_pages(window, cx);
                            })),
                    ),
            )
            .into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn indonesian_date_is_valid_passive_text() {
        let date = today_indonesian();
        assert!(validate_text(&date).is_ok());
        assert_eq!(date.split_whitespace().count(), 3);
    }

    #[test]
    fn text_placement_and_edit_rects_stay_within_page_bounds_for_all_rotations() {
        for turns in 0..4 {
            let (w, h) = if turns % 2 == 0 {
                (600., 800.)
            } else {
                (800., 600.)
            };
            let geometry = PageGeometry {
                width_points: w,
                height_points: h,
                quarter_turns: turns,
            };
            for cursor in [[0., 0.], [0.5, 0.5], [1., 1.], [0.99, 0.01]] {
                let Some([a, b]) = text_placement_rect("24 November 2026", 14., geometry, cursor)
                else {
                    panic!("text placement rect");
                };
                assert!(a[0] >= 0. && a[1] >= 0. && b[0] <= 1. && b[1] <= 1.);
                assert!(b[0] > a[0] && b[1] > a[1]);
            }
            let Some([a, b]) = text_edit_rect([0.95, 0.95], "Teks panjang sekali", 24., geometry)
            else {
                panic!("text edit rect");
            };
            assert!(a[0] >= 0. && a[1] >= 0. && b[0] <= 1. && b[1] <= 1.);
            assert!(b[0] > a[0] && b[1] > a[1]);
        }
    }
}
