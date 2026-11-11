use crate::*;
use gpui_kit::assets::IconName;
use gpui_kit::component::{
    ActiveTheme, Disableable, Icon, Sizable, StyledExt, Theme, ThemeMode, TitleBar,
    button::{Button, ButtonVariants},
    empty::{Empty, EmptyDescription, EmptyHeader, EmptyMedia, EmptyTitle},
    input::Input,
    scroll::Scrollbar,
    spinner::Spinner,
    status_bar::StatusBar,
    toolbar::Toolbar,
};
use gpui_kit::{IntoElement, MouseButton, Render, canvas, div, img, list};
use pdf_application::viewer::RenderKind;

impl Viewer {
    fn item(
        &mut self,
        index: usize,
        kind: RenderKind,
        cx: &mut Context<Self>,
    ) -> gpui_kit::AnyElement {
        let Some(workspace) = self.workspace.as_ref() else {
            return div().into_any_element();
        };
        let Ok(geometry) = workspace.geometry(index) else {
            return div().into_any_element();
        };
        let page_id = workspace.pages()[index].id;
        let Some(key) = self.key(index, kind) else {
            return div().into_any_element();
        };
        let thumbnail = kind == RenderKind::Thumbnail;
        let element_id: gpui_kit::SharedString = format!(
            "{}-{}-{}",
            if thumbnail { "thumbnail" } else { "page" },
            page_id.source.0,
            page_id.serial
        )
        .into();
        let (width, height) = if thumbnail {
            (128., 128. * geometry.height_points / geometry.width_points)
        } else {
            self.zoom.size(geometry, self.viewport)
        };
        let colors = cx.theme().colors;
        let active = usize::from(self.navigation.current) == index;
        let failed = self.failures.get(&key).cloned();
        let mut bitmap_key = key.clone();
        let mut bitmap = self.bitmap(&key);
        let full_missing = bitmap.is_none();
        if full_missing && !thumbnail {
            bitmap_key = key.preview();
            bitmap = self.bitmap(&bitmap_key);
        }
        if full_missing && failed.is_none() {
            let generation = self.generation;
            let weak = cx.weak_entity();
            cx.defer(move |cx| {
                let _ = weak.update(cx, |this, cx| {
                    if this.generation == generation {
                        this.ensure_render(index, kind, if thumbnail { 3 } else { 0 }, cx);
                    }
                });
            });
        }
        let content = match bitmap {
            Some(bitmap) => {
                self.gpu_images.insert(bitmap_key, (bitmap.clone(), index));
                img(bitmap).w(px(width)).h(px(height)).into_any_element()
            }
            None => div()
                .size_full()
                .flex()
                .items_center()
                .justify_center()
                .text_color(colors.muted_foreground)
                .text_xs()
                .when(!thumbnail, |this| {
                    this.child(failed.unwrap_or_else(|| "Menyiapkan halaman…".into()))
                })
                .when(thumbnail, |this| this.child(format!("{}", index + 1)))
                .into_any_element(),
        };
        let overlay = if !thumbnail {
            let text = self.text.pages.get(&index);
            if text.is_none() && !self.text.errors.contains_key(&index) {
                let weak = cx.weak_entity();
                cx.defer(move |cx| {
                    let _ = weak.update(cx, |this, cx| this.request_text(index, cx));
                });
            }
            let selection = self.text.selection;
            let weak = cx.weak_entity();
            canvas(
                |_, _, _| {},
                move |bounds, _, window, _| {
                    if let (Some(text), Some(selection)) = (&text, selection) {
                        for rect in text.highlights(selection.range(index, text.chars.len())) {
                            let highlight = Bounds::new(
                                bounds.origin
                                    + gpui::point(
                                        bounds.size.width * rect.left,
                                        bounds.size.height * rect.top,
                                    ),
                                gpui::size(
                                    bounds.size.width * (rect.right - rect.left),
                                    bounds.size.height * (rect.bottom - rect.top),
                                ),
                            );
                            window.paint_quad(gpui::fill(highlight, gpui::rgba(0x3b82f650)));
                        }
                    }
                    let down = weak.clone();
                    window.on_mouse_event(
                        move |event: &gpui::MouseDownEvent, phase, window, cx| {
                            if phase == gpui::DispatchPhase::Bubble
                                && event.button == MouseButton::Left
                                && bounds.contains(&event.position)
                            {
                                let _ = down.update(cx, |this, cx| {
                                    this.select_text(
                                        index,
                                        bounds,
                                        event.position,
                                        event.click_count,
                                        window,
                                        cx,
                                    )
                                });
                                cx.stop_propagation();
                            }
                        },
                    );
                    window.on_mouse_event(
                        move |event: &gpui::MouseMoveEvent, phase, window, cx| {
                            if phase == gpui::DispatchPhase::Bubble
                                && bounds.contains(&event.position)
                            {
                                let _ = weak.update(cx, |this, cx| {
                                    // Hover/repaint events can carry no button. End the
                                    // press on mouse-up, not on synthetic hover updates.
                                    if event.pressed_button == Some(MouseButton::Left)
                                        && this.text.dragging
                                    {
                                        this.select_text(
                                            index,
                                            bounds,
                                            event.position,
                                            0,
                                            window,
                                            cx,
                                        );
                                    }
                                });
                            }
                        },
                    );
                },
            )
            .absolute()
            .top_0()
            .left_0()
            .size_full()
            .into_any_element()
        } else {
            div().into_any_element()
        };
        let selectable = !thumbnail
            && self
                .text
                .pages
                .get(&index)
                .is_some_and(|page| page.chars.iter().any(|ch| ch.bounds.is_some()));
        let page = div()
            .id(("page-text", index))
            .when(!thumbnail, |this| {
                this.role(gpui::Role::Document)
                    .aria_label(format!("Isi halaman {}", index + 1))
            })
            .when(selectable, |this| this.cursor_text())
            .w(px(width))
            .h(px(height))
            .flex_shrink_0()
            .bg(gpui_kit::white())
            .shadow_md()
            .overflow_hidden()
            .relative()
            .child(content)
            .child(overlay);
        if thumbnail {
            div()
                .id(element_id)
                .role(gpui_kit::Role::Button)
                .aria_label(format!("Halaman {}", index + 1))
                .focusable()
                .w_full()
                .h(px(height + 48.))
                .flex()
                .flex_col()
                .items_center()
                .justify_center()
                .gap_2()
                .cursor_pointer()
                .bg(if active {
                    colors.accent
                } else {
                    colors.background
                })
                .child(
                    div()
                        .border_2()
                        .border_color(if active {
                            colors.primary
                        } else {
                            colors.border
                        })
                        .child(page),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(colors.muted_foreground)
                        .child(format!("{}", index + 1)),
                )
                .on_click(cx.listener(move |this, _, window, cx| {
                    this.go_to(index as u16, cx);
                    this.focus.focus(window, cx);
                }))
                .into_any_element()
        } else {
            div()
                .id(element_id)
                .w_full()
                .h(px(height + 48.))
                .flex()
                .flex_col()
                .items_center()
                .gap_2()
                .child(
                    div()
                        .h_6()
                        .text_xs()
                        .text_color(colors.muted_foreground)
                        .child(format!("Halaman {}", index + 1)),
                )
                .child(page)
                .into_any_element()
        }
    }
}

impl Render for Viewer {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let c = cx.theme().colors;
        let dark = cx.theme().is_dark();
        let has_document = self.workspace.is_some();
        let can_previous = self.navigation.target(Direction::Previous).is_some();
        let can_next = self.navigation.target(Direction::Next).is_some();
        let name = self
            .workspace
            .as_ref()
            .and_then(|w| w.source().path.file_name())
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_else(|| "Ruang baca PDF".into());
        let subtitle = self
            .workspace
            .as_ref()
            .and_then(|w| w.geometry(usize::from(self.navigation.current)).ok())
            .map(|g| {
                format!(
                    "{} halaman · {:.0} × {:.0} pt · Dokumen lokal",
                    self.navigation.count, g.width_points, g.height_points
                )
            })
            .unwrap_or_else(|| "Buka dokumen untuk mulai membaca".into());
        let open_button = Button::new("open")
            .primary()
            .icon(IconName::FolderOpen)
            .label("Buka PDF")
            .disabled(self.opening)
            .tooltip("Buka dokumen · Ctrl+O")
            .on_click(cx.listener(|this, _, window, cx| this.open(&Open, window, cx)));
        let theme_button = Button::new("theme")
            .accessibility_label("Ganti tema")
            .ghost()
            .icon(if dark { IconName::Sun } else { IconName::Moon })
            .tooltip("Ganti tema terang/gelap")
            .on_click(|_, window, cx| {
                let mode = if cx.theme().is_dark() {
                    ThemeMode::Light
                } else {
                    ThemeMode::Dark
                };
                Theme::change(mode, Some(window), cx);
            });
        let header = Toolbar::new("document-toolbar")
            .px_5()
            .py_3()
            .border_b_1()
            .border_color(c.border)
            .content(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .child(div().text_sm().font_semibold().text_ellipsis().child(name))
                    .child(
                        div()
                            .text_xs()
                            .text_color(c.muted_foreground)
                            .child(subtitle),
                    ),
            )
            .child(
                Button::new("copy-text")
                    .accessibility_label("Salin teks")
                    .ghost()
                    .icon(IconName::Copy)
                    .label("Salin")
                    .tooltip("Salin teks terpilih · Ctrl+C")
                    .disabled(self.text.selection.is_none())
                    .on_click(cx.listener(|this, _, _, cx| this.copy_text(cx))),
            )
            .child(theme_button)
            .child(open_button);
        let page_controls = Toolbar::new("page-toolbar")
            .child(
                Button::new("sidebar")
                    .accessibility_label("Panel thumbnail")
                    .ghost()
                    .icon(IconName::PanelLeft)
                    .tooltip("Tampilkan/sembunyikan thumbnail")
                    .disabled(!has_document)
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.sidebar = !this.sidebar;
                        cx.notify();
                    })),
            )
            .child(
                Button::new("first-page")
                    .accessibility_label("Halaman pertama")
                    .ghost()
                    .icon(IconName::ChevronsLeft)
                    .tooltip("Halaman pertama · Ctrl+Home")
                    .disabled(!can_previous)
                    .on_click(cx.listener(|this, _, _, cx| this.navigate(Direction::First, cx))),
            )
            .child(
                Button::new("previous-page")
                    .accessibility_label("Halaman sebelumnya")
                    .ghost()
                    .icon(IconName::ChevronLeft)
                    .tooltip("Sebelumnya · PgUp")
                    .disabled(!can_previous)
                    .on_click(cx.listener(|this, _, _, cx| this.navigate(Direction::Previous, cx))),
            )
            .content(
                Input::new(&self.jump)
                    .small()
                    .w(px(60.))
                    .disabled(!has_document)
                    .aria_label("Nomor halaman tujuan"),
            )
            .child(
                Button::new("jump-page")
                    .ghost()
                    .label("Ke")
                    .tooltip("Lompat ke halaman · Ctrl+G")
                    .disabled(!has_document)
                    .on_click(cx.listener(|this, _, window, cx| this.jump_to(window, cx))),
            )
            .content(
                div()
                    .text_sm()
                    .text_color(c.muted_foreground)
                    .child(format!("/ {}", self.navigation.count)),
            )
            .child(
                Button::new("next-page")
                    .accessibility_label("Halaman berikutnya")
                    .ghost()
                    .icon(IconName::ChevronRight)
                    .tooltip("Berikutnya · PgDn")
                    .disabled(!can_next)
                    .on_click(cx.listener(|this, _, _, cx| this.navigate(Direction::Next, cx))),
            )
            .child(
                Button::new("last-page")
                    .accessibility_label("Halaman terakhir")
                    .ghost()
                    .icon(IconName::ChevronsRight)
                    .tooltip("Halaman terakhir · Ctrl+End")
                    .disabled(!can_next)
                    .on_click(cx.listener(|this, _, _, cx| this.navigate(Direction::Last, cx))),
            );
        let zoom_controls = Toolbar::new("zoom-toolbar")
            .child(
                Button::new("zoom-out")
                    .accessibility_label("Perkecil")
                    .ghost()
                    .icon(IconName::Minus)
                    .tooltip("Perkecil · Ctrl+-")
                    .disabled(!has_document || self.zoom_percent() <= 50)
                    .on_click(cx.listener(|this, _, _, cx| this.zoom_step(-25, cx))),
            )
            .child(
                Button::new("zoom-reset")
                    .ghost()
                    .label(format!("{}%", self.zoom_percent()))
                    .tooltip("Ukuran 100%")
                    .disabled(!has_document)
                    .on_click(cx.listener(|this, _, _, cx| this.set_zoom(Zoom::Percent(100), cx))),
            )
            .child(
                Button::new("zoom-in")
                    .accessibility_label("Perbesar")
                    .ghost()
                    .icon(IconName::Plus)
                    .tooltip("Perbesar · Ctrl++")
                    .disabled(!has_document || self.zoom_percent() >= 400)
                    .on_click(cx.listener(|this, _, _, cx| this.zoom_step(25, cx))),
            )
            .child(
                Button::new("fit-width")
                    .ghost()
                    .icon(IconName::MoveHorizontal)
                    .label("Lebar")
                    .tooltip("Fit width · Ctrl+1")
                    .disabled(!has_document)
                    .on_click(cx.listener(|this, _, _, cx| this.set_zoom(Zoom::FitWidth, cx))),
            )
            .child(
                Button::new("fit-page")
                    .ghost()
                    .icon(IconName::Scan)
                    .label("Halaman")
                    .tooltip("Fit page · Ctrl+0")
                    .disabled(!has_document)
                    .on_click(cx.listener(|this, _, _, cx| this.set_zoom(Zoom::FitPage, cx))),
            );

        let body = if has_document {
            let weak = cx.weak_entity();
            let main_list = list(self.page_list.clone(), move |index, _, cx| {
                weak.update(cx, |this, cx| this.item(index, RenderKind::Page, cx))
                    .unwrap_or_else(|_| div().into_any_element())
            });
            let content_width = self.content_width();
            let weak = cx.weak_entity();
            let scroll_weak = weak.clone();
            let previous = self.viewport;
            let previous_dpr = self.dpr;
            let probe = canvas(
                move |bounds, window, cx| {
                    let viewport = (f32::from(bounds.size.width), f32::from(bounds.size.height));
                    let dpr = window.scale_factor();
                    if (viewport.0 - previous.0).abs() >= 2.
                        || (viewport.1 - previous.1).abs() >= 2.
                        || (dpr - previous_dpr).abs() >= 0.01
                    {
                        cx.defer(move |cx| {
                            let _ = weak
                                .update(cx, |this, cx| this.viewport_changed(viewport, dpr, cx));
                        });
                    }
                },
                move |bounds, _, window, _| {
                    let release_weak = scroll_weak.clone();
                    window.on_mouse_event(move |event: &gpui::MouseUpEvent, phase, _, cx| {
                        if phase == gpui::DispatchPhase::Capture
                            && event.button == MouseButton::Left
                        {
                            let _ = release_weak.update(cx, |this, _| {
                                this.text.dragging = false;
                                this.text.pending = None;
                                this.text.sentence_anchor = None;
                            });
                        }
                    });
                    // Capture Ctrl+scroll before the list handles its bubble phase.
                    // Plain two-finger scrolling continues through unchanged.
                    window.on_mouse_event(move |event: &gpui::ScrollEvent, phase, window, cx| {
                        if phase == gpui::DispatchPhase::Capture
                            && event.modifiers.control
                            && bounds.contains(&event.position)
                        {
                            let dy = f32::from(event.delta.pixel_delta(px(20.)).y);
                            let delta = (dy.clamp(-240., 240.) * 0.002).exp() - 1.;
                            let _ = scroll_weak.update(cx, |this, cx| {
                                this.gesture_zoom(delta, event.position, event.touch_phase, cx);
                            });
                            window.prevent_default();
                            cx.stop_propagation();
                        }
                    });
                },
            )
            .absolute()
            .top_0()
            .left_0()
            .size_full();
            let main = div()
                .id("document-viewport")
                .flex_1()
                .min_w_0()
                .min_h_0()
                .relative()
                .bg(c.muted)
                .overflow_hidden()
                .on_pinch(cx.listener(|this, event: &gpui::PinchEvent, window, cx| {
                    this.gesture_zoom(event.delta, event.position, event.phase, cx);
                    window.prevent_default();
                    cx.stop_propagation();
                }))
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(|this, _, window, cx| this.focus.focus(window, cx)),
                )
                .child(
                    div()
                        .id("horizontal-pages")
                        .size_full()
                        .overflow_x_scroll()
                        .track_scroll(&self.horizontal)
                        .child(main_list.w(px(content_width)).h_full()),
                )
                .child(probe)
                .child(Scrollbar::vertical(&self.page_list))
                .child(Scrollbar::horizontal(&self.horizontal));
            let sidebar = if self.sidebar {
                let weak = cx.weak_entity();
                div()
                    .w(px(196.))
                    .flex_shrink_0()
                    .min_h_0()
                    .flex()
                    .flex_col()
                    .bg(c.background)
                    .border_r_1()
                    .border_color(c.border)
                    .child(
                        div()
                            .h_10()
                            .px_4()
                            .py_2()
                            .text_xs()
                            .text_color(c.muted_foreground)
                            .child("HALAMAN"),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_h_0()
                            .relative()
                            .child(
                                list(self.thumbnail_list.clone(), move |index, _, cx| {
                                    weak.update(cx, |this, cx| {
                                        this.item(index, RenderKind::Thumbnail, cx)
                                    })
                                    .unwrap_or_else(|_| div().into_any_element())
                                })
                                .size_full(),
                            )
                            .child(Scrollbar::vertical(&self.thumbnail_list)),
                    )
                    .into_any_element()
            } else {
                div().into_any_element()
            };
            div()
                .flex_1()
                .min_h_0()
                .flex()
                .child(sidebar)
                .child(main)
                .into_any_element()
        } else {
            let title = if self.opening {
                "Memeriksa dokumen…"
            } else if self.error.is_some() {
                "PDF belum dapat dibuka"
            } else {
                "Dokumenmu, ruang bacamu."
            };
            let description = self.error.clone().unwrap_or_else(|| if self.opening {
                "Membaca ukuran dan orientasi halaman. Bitmap akan dimuat saat diperlukan.".into()
            } else { "Buka PDF dari perangkatmu. Baca setiap halaman dengan nyaman, tanpa mengubah file aslinya.".into() });
            div()
                .flex_1()
                .min_h_0()
                .flex()
                .justify_center()
                .bg(c.muted)
                .p_8()
                .child(
                    Empty::new()
                        .max_w(px(540.))
                        .header(
                            EmptyHeader::new()
                                .media(
                                    EmptyMedia::new().child(
                                        Icon::new(IconName::FileText)
                                            .size_12()
                                            .text_color(c.primary),
                                    ),
                                )
                                .title(EmptyTitle::new().text_2xl().child(title))
                                .description(EmptyDescription::new().child(description)),
                        )
                        .when(self.opening, |this| this.child(Spinner::new()))
                        .when(!self.opening, |this| {
                            this.child(
                                Button::new("open-empty")
                                    .primary()
                                    .icon(IconName::FolderOpen)
                                    .label("Pilih dokumen")
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.open(&Open, window, cx)
                                    })),
                            )
                        }),
                )
                .into_any_element()
        };
        div()
            .size_full()
            .flex()
            .flex_col()
            .bg(c.background)
            .text_color(c.foreground)
            .font_family("Segoe UI")
            .track_focus(&self.focus)
            .key_context("PdfViewer")
            .on_action(cx.listener(|this, _: &CopyText, _, cx| this.copy_text(cx)))
            .on_action(cx.listener(|this, _: &ClearTextSelection, _, cx| this.clear_selection(cx)))
            .on_action(cx.listener(Self::open))
            .on_action(
                cx.listener(|this, _: &FirstPage, _, cx| this.navigate(Direction::First, cx)),
            )
            .on_action(
                cx.listener(|this, _: &PreviousPage, _, cx| this.navigate(Direction::Previous, cx)),
            )
            .on_action(cx.listener(|this, _: &NextPage, _, cx| this.navigate(Direction::Next, cx)))
            .on_action(cx.listener(|this, _: &LastPage, _, cx| this.navigate(Direction::Last, cx)))
            .on_action(cx.listener(|this, _: &ZoomIn, _, cx| this.zoom_step(25, cx)))
            .on_action(cx.listener(|this, _: &ZoomOut, _, cx| this.zoom_step(-25, cx)))
            .on_action(cx.listener(|this, _: &FitWidth, _, cx| this.set_zoom(Zoom::FitWidth, cx)))
            .on_action(cx.listener(|this, _: &FitPage, _, cx| this.set_zoom(Zoom::FitPage, cx)))
            .on_action(cx.listener(|this, _: &ToggleSidebar, _, cx| {
                this.sidebar = !this.sidebar;
                cx.notify();
            }))
            .on_action(cx.listener(|this, _: &FocusPage, window, cx| {
                this.jump.update(cx, |state, cx| {
                    state.set_value("", window, cx);
                    state.focus(window, cx);
                });
            }))
            .child(
                TitleBar::new().child(
                    div()
                        .flex()
                        .items_center()
                        .gap_2()
                        .text_sm()
                        .child(Icon::new(IconName::FileText).small().text_color(c.primary))
                        .child("PDF Studio"),
                ),
            )
            .child(header)
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .justify_between()
                    .px_3()
                    .py_1()
                    .border_b_1()
                    .border_color(c.border)
                    .child(page_controls)
                    .child(zoom_controls),
            )
            .child(body)
            .child(
                StatusBar::new()
                    .px_4()
                    .py_2()
                    .left(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .when(self.opening || self.inflight.is_some(), |this| {
                                this.child(Spinner::new().xsmall())
                            })
                            .child(self.status.clone()),
                    )
                    .right(format!("{}% · Ctrl+G: ke halaman", self.zoom_percent())),
            )
    }
}
