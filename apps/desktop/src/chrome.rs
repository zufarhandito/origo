use crate::*;
use gpui::assets::IconName;
use gpui::component::{
    ActiveTheme, Disableable, Selectable, Sizable, StyledExt, Theme, ThemeMode,
    button::{Button, ButtonCustomVariant, ButtonVariants},
    input::Input,
    menu::{DropdownMenu, PopupMenuItem},
    tooltip::Tooltip,
};
use gpui::{AnyElement, Focusable, IntoElement, div};

#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub(super) enum ViewerMode {
    #[default]
    Read,
    Organize,
    Convert,
}

fn icon(
    id: impl Into<gpui::ElementId>,
    name: IconName,
    label: &'static str,
    shortcut: &'static str,
) -> Button {
    Button::new(id)
        .ghost()
        .icon(name)
        .accessibility_label(label)
        .tooltip(if shortcut.is_empty() {
            label.to_owned()
        } else {
            format!("{label} - {shortcut}")
        })
        .w(px(32.))
        .h(px(32.))
}
fn divider(cx: &App) -> AnyElement {
    div()
        .w(px(1.))
        .h(px(20.))
        .mx_2()
        .bg(cx.theme().border)
        .flex_shrink_0()
        .into_any_element()
}
impl Viewer {
    pub(super) fn set_mode(
        &mut self,
        mode: ViewerMode,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.opening || self.mode == mode {
            return;
        }
        cx.stop_active_drag(window);
        self.page_dragging = false;
        self.mode = mode;
        if mode == ViewerMode::Organize {
            self.sidebar = true;
            if self.selected_ids().is_empty() {
                self.select_page(usize::from(self.navigation.current), false, false, cx);
            }
            self.page_focus.focus(window, cx);
        } else {
            self.page_anchor = None;
            if let Some(w) = &mut self.workspace {
                w.select([]);
            }
            self.focus.focus(window, cx);
        }
        self.report(cx);
        cx.notify();
    }
    pub(super) fn select_all_pages(&mut self, all: bool, cx: &mut Context<Self>) {
        if self.mode != ViewerMode::Organize || self.opening {
            return;
        }
        if let Some(w) = &mut self.workspace {
            let ids = if all {
                w.pages().iter().map(|p| p.id).collect::<Vec<_>>()
            } else {
                vec![]
            };
            w.select(ids);
        }
        if !all {
            self.page_anchor = None;
        }
        self.report(cx);
        cx.notify();
    }
    pub(super) fn document_header(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let available = self.workspace.is_some();
        let name = self
            .workspace
            .as_ref()
            .and_then(|w| w.source().path.file_name())
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| "PDF Studio".into());
        let detail = self
            .workspace
            .as_ref()
            .and_then(|w| w.geometry(usize::from(self.navigation.current)).ok())
            .map(|g| format!("{name}\n{:.0} x {:.0} pt", g.width_points, g.height_points))
            .unwrap_or_else(|| name.clone());
        let weak = cx.weak_entity();
        let disabled = !available || self.opening;
        let dark = cx.theme().is_dark();
        let more =
            icon("more", IconName::Ellipsis, "Lainnya", "").dropdown_menu(move |menu, _, _| {
                let weak = weak.clone();
                menu.item(PopupMenuItem::new("Impor PDF").disabled(disabled).on_click(
                    move |_, _, cx| {
                        let _ = weak.update(cx, |this, cx| this.import_pdf(cx));
                    },
                ))
                .separator()
                .item(
                    PopupMenuItem::new("Terang")
                        .checked(!dark)
                        .on_click(|_, window, cx| {
                            Theme::change(ThemeMode::Light, Some(window), cx)
                        }),
                )
                .item(
                    PopupMenuItem::new("Gelap")
                        .checked(dark)
                        .on_click(|_, window, cx| Theme::change(ThemeMode::Dark, Some(window), cx)),
                )
            });
        div()
            .id("document-header")
            .h(px(44.))
            .flex_shrink_0()
            .px_4()
            .flex()
            .items_center()
            .gap_3()
            .border_b_1()
            .border_color(cx.theme().border)
            .child(
                div()
                    .id("document-name")
                    .flex_1()
                    .min_w_0()
                    .text_sm()
                    .font_semibold()
                    .text_ellipsis()
                    .tooltip(move |window, cx| Tooltip::new(detail.clone()).build(window, cx))
                    .child(name),
            )
            .child(
                div()
                    .id("viewer-modes")
                    .flex()
                    .gap_1()
                    .child(
                        Button::new("mode-read")
                            .label("Baca")
                            .h(px(32.))
                            .ghost()
                            .selected(self.mode == ViewerMode::Read)
                            .disabled(self.opening)
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.set_mode(ViewerMode::Read, window, cx)
                            })),
                    )
                    .child(
                        Button::new("mode-organize")
                            .label("Atur halaman")
                            .h(px(32.))
                            .ghost()
                            .selected(self.mode == ViewerMode::Organize)
                            .disabled(disabled)
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.set_mode(ViewerMode::Organize, window, cx)
                            })),
                    )
                    .child(
                        Button::new("mode-convert")
                            .label("Konversi")
                            .h(px(32.))
                            .ghost()
                            .selected(self.mode == ViewerMode::Convert)
                            .disabled(self.opening)
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.set_mode(ViewerMode::Convert, window, cx)
                            })),
                    ),
            )
            .child(
                Button::new("open")
                    .primary()
                    .icon(IconName::FolderOpen)
                    .label("Buka PDF")
                    .h(px(32.))
                    .tooltip("Buka PDF - Ctrl+O")
                    .disabled(self.opening)
                    .on_click(cx.listener(|this, _, window, cx| this.open(&Open, window, cx))),
            )
            .child(more)
            .into_any_element()
    }
    pub(super) fn defer_page_input(&self, cx: &mut Context<Self>) {
        let weak = cx.weak_entity();
        let handle = self.window_handle;
        cx.defer(move |cx| {
            let _ = handle.update(cx, |_, window, cx| {
                let _ = weak.update(cx, |this, cx| this.sync_page_input(window, cx));
            });
        });
    }
    fn sync_page_input(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let available = self.navigation.count > 0;
        // Never replace a partially entered number while the field holds focus.
        if !self.jump.read(cx).focus_handle(cx).is_focused(window) {
            let value = if available {
                (self.navigation.current + 1).to_string()
            } else {
                String::new()
            };
            if self.jump.read(cx).value().as_str() != value {
                self.jump
                    .update(cx, |input, cx| input.set_value(value, window, cx));
            }
        }
    }
    pub(super) fn reading_toolbar(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let available = self.navigation.count > 0;
        let compact = window.viewport_size().width < px(1000.);
        let previous = self.navigation.target(Direction::Previous).is_some();
        let next = self.navigation.target(Direction::Next).is_some();
        let weak = cx.weak_entity();
        let zoom = self.zoom;
        let zoom_menu = Button::new("zoom-menu")
            .ghost()
            .label(format!("{}%", self.zoom_percent()))
            .h(px(32.))
            .dropdown_caret(true)
            .accessibility_label("Zoom")
            .disabled(!available)
            .dropdown_menu(move |menu, _, _| {
                let w = weak.clone();
                let w2 = weak.clone();
                let mut menu = menu
                    .item(
                        PopupMenuItem::new("Sesuaikan lebar")
                            .checked(zoom == Zoom::FitWidth)
                            .on_click(move |_, _, cx| {
                                let _ = w.update(cx, |this, cx| this.set_zoom(Zoom::FitWidth, cx));
                            }),
                    )
                    .item(
                        PopupMenuItem::new("Sesuaikan halaman")
                            .checked(zoom == Zoom::FitPage)
                            .on_click(move |_, _, cx| {
                                let _ = w2.update(cx, |this, cx| this.set_zoom(Zoom::FitPage, cx));
                            }),
                    )
                    .separator();
                for percent in [50, 75, 100, 125, 150, 200, 300, 400] {
                    let w = weak.clone();
                    menu = menu.item(
                        PopupMenuItem::new(format!("{percent}%"))
                            .checked(zoom == Zoom::Percent(percent))
                            .on_click(move |_, _, cx| {
                                let _ = w.update(cx, |this, cx| {
                                    this.set_zoom(Zoom::Percent(percent), cx)
                                });
                            }),
                    );
                }
                menu
            });
        let navigation = div()
            .flex()
            .items_center()
            .gap_1()
            .when(!compact, |el| {
                el.child(
                    icon(
                        "first-page",
                        IconName::ChevronsLeft,
                        "Halaman pertama",
                        "Ctrl+Home",
                    )
                    .disabled(!previous)
                    .on_click(cx.listener(|this, _, _, cx| this.navigate(Direction::First, cx))),
                )
            })
            .child(
                icon(
                    "previous-page",
                    IconName::ChevronLeft,
                    "Halaman sebelumnya",
                    "PgUp",
                )
                .disabled(!previous)
                .on_click(cx.listener(|this, _, _, cx| this.navigate(Direction::Previous, cx))),
            )
            .child(
                div()
                    .w(px(56.))
                    .h(px(32.))
                    .flex_shrink_0()
                    .overflow_hidden()
                    .flex()
                    .items_center()
                    .child(
                        Input::new(&self.jump)
                            .small()
                            .w_full()
                            .disabled(!available)
                            .aria_label("Nomor halaman tujuan"),
                    ),
            )
            .child(
                div()
                    .text_sm()
                    .text_color(cx.theme().muted_foreground)
                    .child(format!("/ {}", self.navigation.count)),
            )
            .child(
                icon(
                    "next-page",
                    IconName::ChevronRight,
                    "Halaman berikutnya",
                    "PgDn",
                )
                .disabled(!next)
                .on_click(cx.listener(|this, _, _, cx| this.navigate(Direction::Next, cx))),
            )
            .when(!compact, |el| {
                el.child(
                    icon(
                        "last-page",
                        IconName::ChevronsRight,
                        "Halaman terakhir",
                        "Ctrl+End",
                    )
                    .disabled(!next)
                    .on_click(cx.listener(|this, _, _, cx| this.navigate(Direction::Last, cx))),
                )
            })
            .when(compact, |el| {
                let weak = cx.weak_entity();
                el.child(
                    icon(
                        "navigation-menu",
                        IconName::Ellipsis,
                        "Navigasi halaman",
                        "",
                    )
                    .disabled(!available)
                    .dropdown_menu(move |menu, _, _| {
                        let w = weak.clone();
                        let w2 = weak.clone();
                        menu.item(
                            PopupMenuItem::new("Halaman pertama")
                                .disabled(!previous)
                                .on_click(move |_, _, cx| {
                                    let _ = w
                                        .update(cx, |this, cx| this.navigate(Direction::First, cx));
                                }),
                        )
                        .item(
                            PopupMenuItem::new("Halaman terakhir")
                                .disabled(!next)
                                .on_click(move |_, _, cx| {
                                    let _ = w2
                                        .update(cx, |this, cx| this.navigate(Direction::Last, cx));
                                }),
                        )
                    }),
                )
            });
        div()
            .id("reading-toolbar")
            .h(px(40.))
            .flex_shrink_0()
            .px_3()
            .flex()
            .items_center()
            .justify_between()
            .gap_3()
            .border_b_1()
            .border_color(cx.theme().border)
            .child(
                div()
                    .flex_1()
                    .flex()
                    .items_center()
                    .gap_1()
                    .child(
                        icon("sidebar", IconName::PanelLeft, "Panel thumbnail", "")
                            .selected(self.sidebar)
                            .disabled(self.workspace.is_none())
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.sidebar = !this.sidebar;
                                cx.notify();
                            })),
                    )
                    .child(divider(cx))
                    .child(
                        icon("undo", IconName::Undo2, "Undo", "Ctrl+Z")
                            .disabled(
                                self.opening
                                    || !self.workspace.as_ref().is_some_and(|w| w.can_undo()),
                            )
                            .on_click(cx.listener(|this, _, _, cx| this.history(false, cx))),
                    )
                    .child(
                        icon("redo", IconName::Redo2, "Redo", "Ctrl+Y")
                            .disabled(
                                self.opening
                                    || !self.workspace.as_ref().is_some_and(|w| w.can_redo()),
                            )
                            .on_click(cx.listener(|this, _, _, cx| this.history(true, cx))),
                    ),
            )
            .child(navigation)
            .child(
                div()
                    .flex_1()
                    .flex()
                    .items_center()
                    .justify_end()
                    .gap_1()
                    .child(
                        icon("zoom-out", IconName::Minus, "Perkecil", "Ctrl+-")
                            .disabled(!available || self.zoom_percent() <= 50)
                            .on_click(cx.listener(|this, _, _, cx| this.zoom_step(-25, cx))),
                    )
                    .child(zoom_menu)
                    .child(
                        icon("zoom-in", IconName::Plus, "Perbesar", "Ctrl++")
                            .disabled(!available || self.zoom_percent() >= 400)
                            .on_click(cx.listener(|this, _, _, cx| this.zoom_step(25, cx))),
                    )
                    .when(self.text.selection.is_some(), |el| {
                        el.child(divider(cx)).child(
                            icon("copy-text", IconName::Copy, "Salin teks", "Ctrl+C")
                                .on_click(cx.listener(|this, _, _, cx| this.copy_text(cx))),
                        )
                    }),
            )
            .into_any_element()
    }
    pub(super) fn organizer_toolbar(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let count = self.selected_ids().len();
        let disabled = count == 0 || self.opening;
        let weak = cx.weak_entity();
        let w2 = weak.clone();
        let move_menu = Button::new("move-pages")
            .ghost()
            .label("Pindahkan")
            .h(px(32.))
            .dropdown_caret(true)
            .disabled(disabled)
            .dropdown_menu(move |menu, _, _| {
                let w = weak.clone();
                let w2 = w2.clone();
                menu.item(
                    PopupMenuItem::new("Lebih awal - Alt+Up").on_click(move |_, _, cx| {
                        let _ = w.update(cx, |this, cx| this.move_selection(false, cx));
                    }),
                )
                .item(
                    PopupMenuItem::new("Lebih akhir - Alt+Down").on_click(move |_, _, cx| {
                        let _ = w2.update(cx, |this, cx| this.move_selection(true, cx));
                    }),
                )
            });
        let weak = cx.weak_entity();
        let opening = self.opening;
        let selection_menu = Button::new("select-pages")
            .ghost()
            .label("Seleksi")
            .h(px(32.))
            .dropdown_caret(true)
            .disabled(opening)
            .dropdown_menu(move |menu, _, _| {
                let w = weak.clone();
                let w2 = weak.clone();
                menu.item(PopupMenuItem::new("Pilih semua").on_click(move |_, _, cx| {
                    let _ = w.update(cx, |this, cx| this.select_all_pages(true, cx));
                }))
                .item(
                    PopupMenuItem::new("Bersihkan pilihan").on_click(move |_, _, cx| {
                        let _ = w2.update(cx, |this, cx| this.select_all_pages(false, cx));
                    }),
                )
            });
        div()
            .id("organizer-toolbar")
            .h(px(40.))
            .flex_shrink_0()
            .px_3()
            .flex()
            .items_center()
            .gap_2()
            .border_b_1()
            .border_color(cx.theme().border)
            .child(
                Button::new("import-pdf")
                    .ghost()
                    .icon(IconName::FilePlus)
                    .label("Impor PDF")
                    .h(px(32.))
                    .tooltip("Tambahkan PDF - Ctrl+Shift+O")
                    .disabled(self.opening)
                    .on_click(cx.listener(|this, _, _, cx| this.import_pdf(cx))),
            )
            .child(divider(cx))
            .child(
                div()
                    .text_sm()
                    .text_color(cx.theme().muted_foreground)
                    .child(format!("{count} halaman dipilih")),
            )
            .child(divider(cx))
            .child(
                icon(
                    "rotate-left",
                    IconName::RotateCcw,
                    "Putar kiri",
                    "Ctrl+Shift+R",
                )
                .disabled(disabled)
                .on_click(cx.listener(|this, _, _, cx| {
                    this.edit(
                        pdf_domain::Command::Rotate {
                            ids: this.selected_ids(),
                            turns: 3,
                        },
                        cx,
                    )
                })),
            )
            .child(
                icon("rotate-right", IconName::RotateCw, "Putar kanan", "Ctrl+R")
                    .disabled(disabled)
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.edit(
                            pdf_domain::Command::Rotate {
                                ids: this.selected_ids(),
                                turns: 1,
                            },
                            cx,
                        )
                    })),
            )
            .child(
                icon("delete-pages", IconName::Trash, "Hapus", "Delete")
                    .disabled(disabled)
                    .custom(
                        ButtonCustomVariant::new(cx)
                            .hover(gpui::rgba(0xdc262622).into())
                            .active(gpui::rgba(0xdc262633).into()),
                    )
                    .focus_visible(|style| style.bg(gpui::rgba(0xdc262622)))
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.edit(
                            pdf_domain::Command::Delete {
                                ids: this.selected_ids(),
                            },
                            cx,
                        )
                    })),
            )
            .child(divider(cx))
            .child(move_menu)
            .child(div().flex_1())
            .child(selection_menu)
            .into_any_element()
    }
}
