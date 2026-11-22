//! Card gallery and asynchronous persistence for local signature templates.
use crate::signature_library::{self, Entry};
use crate::signatures::{SignatureKind, asset_preview};
use crate::*;
use gpui::assets::IconName;
use gpui::component::{
    ActiveTheme, Disableable, Sizable,
    button::{Button, ButtonVariants},
    input::Input,
};
use gpui::{ObjectFit, StyledImage, div};

#[derive(Default, Clone, Copy, PartialEq, Eq)]
pub(super) enum GalleryView {
    Loading,
    #[default]
    Create,
    Gallery,
}

#[derive(Default)]
pub(super) struct GalleryState {
    pub entries: Vec<Entry>,
    pub selected: Option<String>,
    pub view: GalleryView,
    pub ready: bool,
    pub error: Option<String>,
    loading: bool,
    previews: Vec<Option<Arc<RenderImage>>>,
    menu: Option<String>,
    renaming: Option<InlineRename>,
    deleting: Option<String>,
}

struct InlineRename {
    original: String,
    input: Entity<InputState>,
}

impl GalleryState {
    pub(super) fn reset_selection(&mut self) {
        self.selected = None;
        self.menu = None;
        self.renaming = None;
        self.deleting = None;
    }

    fn replace_entries(&mut self, entries: Vec<Entry>) {
        self.previews = entries
            .iter()
            .map(|entry| asset_preview(&entry.asset))
            .collect();
        self.entries = entries;
        if self
            .selected
            .as_ref()
            .is_some_and(|name| !self.entries.iter().any(|entry| &entry.label == name))
        {
            self.selected = None;
        }
    }
}

enum LibraryEdit {
    Add(Entry),
    Rename(String, String),
    Delete(String),
}

fn category_badge(kind: usize) -> &'static str {
    match kind {
        1 => "Inisial",
        2 => "Stempel",
        _ => "TTD",
    }
}

fn category_name(kind: usize) -> &'static str {
    match kind {
        1 => "Inisial",
        2 => "Stempel",
        _ => "Tanda Tangan",
    }
}

impl DocumentSession {
    pub(super) fn show_signature_gallery(&mut self, cx: &mut Context<Self>) {
        let gallery = &mut self.annotation.signature.gallery;
        gallery.reset_selection();
        gallery.view = GalleryView::Gallery;
        cx.notify();
    }

    pub(super) fn show_signature_creation(&mut self, cx: &mut Context<Self>) {
        let gallery = &mut self.annotation.signature.gallery;
        gallery.reset_selection();
        gallery.view = GalleryView::Create;
        cx.notify();
    }

    pub(super) fn selected_signature_template(&self) -> Option<Entry> {
        let gallery = &self.annotation.signature.gallery;
        let name = gallery.selected.as_ref()?;
        gallery
            .entries
            .iter()
            .find(|entry| &entry.label == name)
            .cloned()
    }

    pub(super) fn signature_gallery_panel(&self, cx: &mut Context<Self>) -> gpui::AnyElement {
        let gallery = &self.annotation.signature.gallery;
        div()
            .grid()
            .grid_cols(2)
            .gap_3()
            .children(
                gallery
                    .entries
                    .iter()
                    .enumerate()
                    .map(|(index, entry)| self.signature_template_card(index, entry, cx)),
            )
            .into_any_element()
    }

    fn signature_template_card(
        &self,
        index: usize,
        entry: &Entry,
        cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
        let gallery = &self.annotation.signature.gallery;
        let theme = cx.theme().colors;
        let accent = gpui::Hsla::from(gpui::rgb(0x3b82f6));
        let selected = gallery.selected.as_ref() == Some(&entry.label);
        let menu_open = gallery.menu.as_ref() == Some(&entry.label);
        let deleting = gallery.deleting.as_ref() == Some(&entry.label);
        let renaming = gallery
            .renaming
            .as_ref()
            .filter(|rename| rename.original == entry.label);
        let card_label = entry.label.clone();
        let surface_label = entry.label.clone();
        let menu_label = entry.label.clone();
        let preview = gallery.previews.get(index).and_then(Clone::clone);
        let card = div()
            .id(("signature-template", index))
            .relative()
            .flex()
            .flex_col()
            .min_w_0()
            .gap_2()
            .rounded_lg()
            .border_1()
            .border_color(if selected { accent } else { theme.border })
            .bg(if selected {
                accent.opacity(0.12)
            } else {
                theme.background
            })
            .hover(|style| style.border_color(accent.opacity(0.7)))
            .when(!self.opening && !deleting && renaming.is_none(), |card| {
                card.cursor_pointer().on_click(cx.listener(
                    move |s, event: &gpui::ClickEvent, window, cx| {
                        s.select_signature_template(
                            &surface_label,
                            event.click_count(),
                            window,
                            cx,
                        );
                    },
                ))
            })
            .p_2()
            .child(
                div()
                    .flex()
                    .justify_between()
                    .items_center()
                    .child(
                        div()
                            .rounded_md()
                            .bg(theme.muted)
                            .text_color(theme.foreground)
                            .text_xs()
                            .px_2()
                            .py_1()
                            .child(category_badge(entry.kind)),
                    )
                    .child(
                        Button::new(("signature-template-options", index))
                            .outline()
                            .small()
                            .icon(IconName::EllipsisVertical)
                            .tooltip("Opsi template")
                            .accessibility_label(format!("Opsi template: {}", entry.label))
                            .disabled(self.opening || deleting)
                            .on_click(cx.listener(move |s, _, _, cx| {
                                cx.stop_propagation();
                                let gallery = &mut s.annotation.signature.gallery;
                                gallery.deleting = None;
                                gallery.menu = if gallery.menu.as_ref() == Some(&menu_label) {
                                    None
                                } else {
                                    Some(menu_label.clone())
                                };
                                cx.notify();
                            })),
                    ),
            )
            .child({
                let mut button = Button::new(("signature-template-select", index))
                    .ghost()
                    .w_full()
                    .h_auto()
                    .p_0()
                    .cursor_pointer()
                    .accessibility_label(format!("Pakai template: {}", entry.label))
                    .tooltip(entry.label.clone())
                    .disabled(self.opening || deleting)
                    .on_click(cx.listener(move |s, event: &gpui::ClickEvent, window, cx| {
                        cx.stop_propagation();
                        s.select_signature_template(&card_label, event.click_count(), window, cx);
                    }));
                let preview_box = div()
                    .w_full()
                    .aspect_ratio(2.)
                    .flex()
                    .items_center()
                    .justify_center()
                    .overflow_hidden()
                    .rounded_md()
                    .bg(gpui::rgb(0xf8fafc))
                    .p_2()
                    .when_some(preview, |thumbnail, image| {
                        thumbnail.child(gpui::img(image).size_full().object_fit(ObjectFit::Contain))
                    });
                button = button.child(
                    div()
                        .flex()
                        .flex_col()
                        .w_full()
                        .min_w_0()
                        .gap_2()
                        .child(preview_box)
                        .when(renaming.is_none(), |preview_and_name| {
                            preview_and_name
                                .child(div().text_left().truncate().child(entry.label.clone()))
                        }),
                );
                button
            })
            .when_some(renaming, |card, rename| {
                card.child(
                    div()
                        .id(("signature-template-rename-field", index))
                        .key_context("SignatureTemplateRename")
                        .on_action(cx.listener(|s, _: &CancelSignatureRename, _, cx| {
                            cx.stop_propagation();
                            let gallery = &mut s.annotation.signature.gallery;
                            gallery.renaming = None;
                            gallery.error = None;
                            cx.notify();
                        }))
                        .child(
                            Input::new(&rename.input)
                                .aria_label(format!("Nama template: {}", entry.label)),
                        ),
                )
            });
        card.when(menu_open, |card| {
            card.child(self.signature_template_menu(index, entry, cx))
        })
        .when(deleting, |card| {
            card.child(self.signature_template_delete_overlay(index, entry, cx))
        })
        .into_any_element()
    }

    fn select_signature_template(
        &mut self,
        label: &str,
        click_count: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.finish_template_rename(cx, window);
        let gallery = &mut self.annotation.signature.gallery;
        gallery.selected = Some(label.to_owned());
        gallery.menu = None;
        gallery.deleting = None;
        if click_count == 2
            && let Some(template) = self.selected_signature_template()
        {
            self.begin_signature_placement(template.asset, window, cx);
        }
        cx.notify();
    }

    fn signature_template_menu(
        &self,
        index: usize,
        entry: &Entry,
        cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
        let theme = cx.theme().colors;
        let rename = entry.label.clone();
        let delete = entry.label.clone();
        div()
            .id(("signature-template-menu", index))
            .on_click(|_, _, cx| cx.stop_propagation())
            .absolute()
            .top(px(40.))
            .right_2()
            .flex()
            .flex_col()
            .gap_1()
            .p_1()
            .bg(theme.background)
            .border_1()
            .border_color(theme.border)
            .rounded_md()
            .shadow_md()
            .child(
                Button::new(("signature-template-rename", index))
                    .outline()
                    .small()
                    .icon(IconName::Pencil)
                    .label("Ubah nama")
                    .disabled(self.opening)
                    .on_click(cx.listener(move |s, _, window, cx| {
                        s.begin_template_rename(rename.clone(), window, cx);
                    })),
            )
            .child(
                Button::new(("signature-template-delete", index))
                    .outline()
                    .small()
                    .icon(IconName::Trash)
                    .label("Hapus")
                    .disabled(self.opening)
                    .on_click(cx.listener(move |s, _, _, cx| {
                        let gallery = &mut s.annotation.signature.gallery;
                        gallery.menu = None;
                        gallery.renaming = None;
                        gallery.deleting = Some(delete.clone());
                        cx.notify();
                    })),
            )
            .into_any_element()
    }

    fn signature_template_delete_overlay(
        &self,
        index: usize,
        entry: &Entry,
        cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
        let theme = cx.theme().colors;
        let delete = entry.label.clone();
        div()
            .absolute()
            .top_0()
            .left_0()
            .size_full()
            .rounded_lg()
            .border_1()
            .border_color(theme.danger.opacity(0.6))
            .bg(theme.background)
            .p_2()
            .flex()
            .flex_col()
            .justify_center()
            .items_center()
            .gap_2()
            .child(div().text_sm().child("Hapus template?"))
            .child(
                div()
                    .flex()
                    .justify_center()
                    .flex_wrap()
                    .gap_2()
                    .child(
                        Button::new(("signature-cancel-delete", index))
                            .outline()
                            .small()
                            .label("Batal")
                            .accessibility_label("Batal hapus template")
                            .disabled(self.opening)
                            .on_click(cx.listener(|s, _, _, cx| {
                                s.annotation.signature.gallery.deleting = None;
                                cx.notify();
                            })),
                    )
                    .child(
                        Button::new(("signature-confirm-delete", index))
                            .danger()
                            .small()
                            .icon(IconName::Trash)
                            .label("Ya, Hapus")
                            .disabled(self.opening)
                            .on_click(cx.listener(move |s, _, window, cx| {
                                s.edit_signature_library(
                                    LibraryEdit::Delete(delete.clone()),
                                    None,
                                    window,
                                    cx,
                                );
                            })),
                    ),
            )
            .into_any_element()
    }

    fn begin_template_rename(
        &mut self,
        original: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.finish_template_rename(cx, window);
        if self.opening {
            return;
        }
        let input = cx.new(|cx| InputState::new(window, cx));
        input.update(cx, |input, cx| {
            input.set_value(original.clone(), window, cx)
        });
        cx.subscribe_in(
            &input,
            window,
            |s, emitter, event: &InputEvent, window, cx| {
                if matches!(event, InputEvent::Blur | InputEvent::PressEnter { .. })
                    && s.annotation
                        .signature
                        .gallery
                        .renaming
                        .as_ref()
                        .is_some_and(|rename| rename.input == *emitter)
                {
                    s.finish_template_rename(cx, window);
                }
            },
        )
        .detach();

        let gallery = &mut self.annotation.signature.gallery;
        gallery.menu = None;
        gallery.error = None;
        gallery.renaming = Some(InlineRename {
            original,
            input: input.clone(),
        });
        let view = cx.weak_entity();
        window.on_next_frame(move |window, cx| {
            if view
                .read_with(cx, |view, _| {
                    view.annotation.signature.open
                        && view
                            .annotation
                            .signature
                            .gallery
                            .renaming
                            .as_ref()
                            .is_some_and(|rename| rename.input == input)
                })
                .unwrap_or(false)
            {
                input.update(cx, |input, cx| {
                    input.focus(window, cx);
                    input.select_all(window, cx);
                });
            }
        });
        cx.notify();
    }

    fn finish_template_rename(&mut self, cx: &mut Context<Self>, window: &mut Window) {
        let gallery = &mut self.annotation.signature.gallery;
        let Some(rename) = gallery.renaming.as_ref() else {
            return;
        };
        let name = rename.input.read(cx).value().trim().to_owned();
        if name == rename.original {
            gallery.renaming = None;
            gallery.error = None;
            cx.notify();
            return;
        }
        if let Err(error) = signature_library::valid_label(&name) {
            gallery.error = Some(error.to_string());
            cx.notify();
            return;
        }
        if gallery.entries.iter().any(|entry| entry.label == name) {
            gallery.error = Some("Nama template sudah digunakan.".into());
            cx.notify();
            return;
        }

        let old = rename.original.clone();
        gallery.renaming = None;
        self.edit_signature_library(LibraryEdit::Rename(old, name), None, window, cx);
    }

    pub(super) fn load_signature_library(&mut self, cx: &mut Context<Self>) {
        let gallery = &mut self.annotation.signature.gallery;
        gallery.reset_selection();
        if gallery.loading {
            return;
        }
        gallery.error = None;
        let Some(dir) = self.resources.templates_dir.clone() else {
            gallery.view = GalleryView::Create;
            return;
        };
        gallery.view = GalleryView::Loading;
        gallery.loading = true;
        let lock = self.resources.templates_lock.clone();
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_executor()
                .spawn(async move {
                    let _guard = lock
                        .lock()
                        .map_err(|_| anyhow::anyhow!("Pustaka terkunci"))?;
                    signature_library::load(&dir)
                })
                .await;
            let _ = this.update(cx, |s, cx| {
                let gallery = &mut s.annotation.signature.gallery;
                gallery.loading = false;
                match result {
                    Ok(entries) => {
                        gallery.replace_entries(entries);
                        gallery.ready = true;
                    }
                    Err(error) => {
                        gallery.replace_entries(Vec::new());
                        gallery.ready = false;
                        gallery.error = Some(format!("Pustaka tidak dapat dibaca: {error}"));
                    }
                }
                gallery.view = if gallery.entries.is_empty() {
                    GalleryView::Create
                } else {
                    GalleryView::Gallery
                };
                cx.notify();
            });
        })
        .detach();
    }

    pub(super) fn save_signature_template(
        &mut self,
        asset: Arc<pdf_domain::annotation::SignatureImage>,
        kind: SignatureKind,
        label: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let entry = Entry {
            asset: asset.clone(),
            kind: kind as usize,
            label,
        };
        self.edit_signature_library(LibraryEdit::Add(entry), Some(asset), window, cx);
    }

    fn edit_signature_library(
        &mut self,
        edit: LibraryEdit,
        place_after_save: Option<Arc<pdf_domain::annotation::SignatureImage>>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.opening || !self.annotation.signature.gallery.ready {
            return;
        }
        let Some(dir) = self.resources.templates_dir.clone() else {
            return;
        };
        let lock = self.resources.templates_lock.clone();
        let editor_revision = self.annotation.signature.editor_revision;
        let workspace_instance = self.workspace_instance;
        self.opening = true;
        self.annotation.signature.gallery.error = None;
        cx.notify();
        cx.spawn_in(window, async move |this, cx| {
            let result = cx
                .background_executor()
                .spawn(async move {
                    let _guard = lock
                        .lock()
                        .map_err(|_| anyhow::anyhow!("Pustaka terkunci"))?;
                    let mut entries = signature_library::load(&dir)?;
                    let renamed = match edit {
                        LibraryEdit::Add(mut entry) => {
                            if entry.label.trim().is_empty() {
                                let name = category_name(entry.kind);
                                entry.label = (1..=entries.len() + 1)
                                    .map(|index| format!("{name} {index}"))
                                    .find(|name| entries.iter().all(|other| &other.label != name))
                                    .ok_or_else(|| {
                                        anyhow::anyhow!("Nama template otomatis tidak tersedia")
                                    })?;
                            }
                            entries.push(entry);
                            None
                        }
                        LibraryEdit::Rename(old, new) => {
                            let entry = entries
                                .iter_mut()
                                .find(|entry| entry.label == old)
                                .ok_or_else(|| {
                                    anyhow::anyhow!("Template sudah berubah. Buka kembali editor.")
                                })?;
                            entry.label = new.clone();
                            Some((old, new))
                        }
                        LibraryEdit::Delete(label) => {
                            entries.retain(|entry| entry.label != label);
                            None
                        }
                    };
                    signature_library::save(&dir, &entries)?;
                    Ok::<_, anyhow::Error>((entries, renamed))
                })
                .await;
            let _ = cx.update(|window, cx| {
                let _ = this.update(cx, |s, cx| {
                    s.opening = false;
                    let state = &mut s.annotation.signature;
                    let gallery = &mut state.gallery;
                    match result {
                        Ok((entries, renamed)) => {
                            if let Some((old, new)) = renamed
                                && gallery.selected.as_ref() == Some(&old)
                            {
                                gallery.selected = Some(new);
                            }
                            gallery.replace_entries(entries);
                            gallery.deleting = None;
                            gallery.error = None;
                            if gallery.entries.is_empty() && gallery.view == GalleryView::Gallery {
                                gallery.view = GalleryView::Create;
                            }
                            if let Some(asset) = place_after_save
                                && state.open
                                && state.editor_revision == editor_revision
                                && s.workspace_instance == workspace_instance
                                && gallery.view == GalleryView::Create
                                && s.active
                            {
                                s.begin_signature_placement(asset, window, cx);
                            }
                        }
                        Err(error) => {
                            gallery.error = Some(format!("Pustaka belum disimpan: {error}"));
                        }
                    }
                    cx.notify();
                });
            });
        })
        .detach();
    }
}
