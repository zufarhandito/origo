use crate::*;
use gpui_kit::assets::IconName;
use gpui_kit::component::{
    ActiveTheme, Icon, Sizable,
    button::{Button, ButtonVariants},
    empty::{Empty, EmptyDescription, EmptyHeader, EmptyMedia, EmptyTitle},
    scroll::Scrollbar,
    spinner::Spinner,
    status_bar::StatusBar,
};
use gpui_kit::{IntoElement, MouseButton, Render, canvas, div, img, list};
use pdf_application::viewer::RenderKind;
use std::{cell::Cell, rc::Rc};

type PaintedSignatureGhost = Rc<Cell<Option<[[f32; 2]; 2]>>>;

impl DocumentSession {
    // Canvas listeners are window-global: clipping alone does not exclude the
    // tool drawer painted over the page, or a modal owned by another document.
    fn document_input_allowed(&self, position: gpui::Point<gpui::Pixels>, window: &Window) -> bool {
        self.visible_for_render
            && self
                .resources
                .annotation_popup_owner
                .load(std::sync::atomic::Ordering::SeqCst)
                == 0
            && !self.annotation.signature.open
            && !self.annotation.text.open
            && !self.annotation.apply_pages.open
            && !self.annotation.popup_open
            && !self
                .resources
                .tab_picker_open
                .load(std::sync::atomic::Ordering::SeqCst)
            && !self.error_popup
            && !(self
                .resources
                .organizer_drawer
                .load(std::sync::atomic::Ordering::SeqCst)
                && position.x >= window.viewport_size().width - px(280.))
            && self
                .resources
                .modal_owner
                .load(std::sync::atomic::Ordering::SeqCst)
                == 0
            && !(self.mode == ViewerMode::Organize
                && window.viewport_size().width < px(1100.)
                && position.x >= window.viewport_size().width - px(280.))
    }
    fn thumbnail_sidebar(
        &mut self,
        dragging_pages: bool,
        cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
        let c = cx.theme().colors;
        let organizing = self.mode == ViewerMode::Organize;
        if self.sidebar {
            if self.outline.open && !organizing {
                return self.outline_sidebar(cx);
            }
            let weak = cx.weak_entity();
            div()
                .id("page-organizer")
                .when(organizing, |el| el.key_context("PageOrganizer"))
                .on_action(cx.listener(|this, _: &ClearPages, window, cx| {
                    if this.edits_locked() {
                        return;
                    }
                    cx.stop_active_drag(window);
                    if window.viewport_size().width < px(1100.) {
                        this.set_mode(ViewerMode::Read, window, cx);
                        return;
                    }
                    this.page_dragging = false;
                    if let Some(mut w) = this.workspace.as_mut() {
                        w.select([]);
                    }
                    this.page_anchor = None;
                    cx.notify();
                }))
                .on_action(cx.listener(|this, _: &PageFocusPrevious, _, cx| {
                    this.navigate(Direction::Previous, cx)
                }))
                .on_action(
                    cx.listener(|this, _: &PageFocusNext, _, cx| {
                        this.navigate(Direction::Next, cx)
                    }),
                )
                .on_action(cx.listener(|this, _: &RotateRight, _, cx| {
                    this.edit(
                        pdf_domain::Command::Rotate {
                            ids: this.selected_ids(),
                            turns: 1,
                        },
                        cx,
                    )
                }))
                .on_action(cx.listener(|this, _: &RotateLeft, _, cx| {
                    this.edit(
                        pdf_domain::Command::Rotate {
                            ids: this.selected_ids(),
                            turns: 3,
                        },
                        cx,
                    )
                }))
                .track_focus(&self.page_focus)
                .on_action(cx.listener(|this, _: &DeletePages, _, cx| {
                    this.edit(
                        pdf_domain::Command::Delete {
                            ids: this.selected_ids(),
                        },
                        cx,
                    )
                }))
                .on_action(cx.listener(|this, _: &SelectPages, _, cx| {
                    if let Some(mut w) = this.workspace.as_mut() {
                        let ids = w.pages().iter().map(|p| p.id).collect::<Vec<_>>();
                        w.select(ids);
                    }
                    cx.notify();
                }))
                .on_action(
                    cx.listener(|this, _: &MoveEarlier, _, cx| this.move_selection(false, cx)),
                )
                .on_action(cx.listener(|this, _: &MoveLater, _, cx| this.move_selection(true, cx)))
                .on_action(
                    cx.listener(|this, _: &ExtendPrevious, _, cx| this.page_step(false, true, cx)),
                )
                .on_action(
                    cx.listener(|this, _: &ExtendNext, _, cx| this.page_step(true, true, cx)),
                )
                .on_action(
                    cx.listener(|this, _: &PagePrevious, _, cx| this.page_step(false, false, cx)),
                )
                .on_action(cx.listener(|this, _: &PageNext, _, cx| this.page_step(true, false, cx)))
                .on_action(cx.listener(|this, _: &TogglePage, _, cx| {
                    this.select_page(usize::from(this.navigation.current), true, false, cx)
                }))
                .w(px(196.))
                .flex_shrink_0()
                .min_h_0()
                .flex()
                .flex_col()
                .bg(c.background)
                .border_r_1()
                .border_color(c.border)
                .child(self.outline_header(cx))
                .child(
                    div()
                        .flex_1()
                        .min_h_0()
                        .relative()
                        .child(
                            list(self.thumbnail_list.clone(), move |index, _, cx| {
                                weak.update(cx, |this, cx| {
                                    this.item(index, RenderKind::Thumbnail, None, cx)
                                })
                                .unwrap_or_else(|_| div().into_any_element())
                            })
                            .size_full(),
                        )
                        .child(Scrollbar::vertical(&self.thumbnail_list)),
                )
                .when(dragging_pages, |el| {
                    el.child(
                        div()
                            .id("drop-at-end")
                            .h_10()
                            .flex()
                            .items_center()
                            .justify_center()
                            .text_xs()
                            .border_t_2()
                            .border_color(c.primary)
                            .child("Letakkan di akhir")
                            .on_drop(cx.listener(|this, drag: &organizer::PageDrag, _, cx| {
                                if drag.0 != this.tab_id {
                                    return;
                                }
                                let target = this.workspace.as_ref().map(|w| w.pages().len());
                                if let Some(target) = target {
                                    this.edit(
                                        pdf_domain::Command::Move {
                                            ids: drag.1.clone(),
                                            target,
                                        },
                                        cx,
                                    );
                                }
                            })),
                    )
                })
                .into_any_element()
        } else {
            div().into_any_element()
        }
    }
    fn item(
        &mut self,
        index: usize,
        kind: RenderKind,
        painted_ghost: Option<PaintedSignatureGhost>,
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
        let organizing = self.mode == ViewerMode::Organize;
        let reading = usize::from(self.navigation.current) == index;
        let active = if thumbnail {
            organizing && workspace.selected().contains(&page_id)
        } else {
            usize::from(self.navigation.current) == index
        };
        let drag_ids = if workspace.selected().contains(&page_id) {
            self.selected_ids()
        } else {
            vec![page_id]
        };
        drop(workspace);
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
        let draft = self
            .annotation
            .draft
            .clone()
            .filter(|(id, _)| *id == page_id);
        let selected_annotation = self
            .annotation
            .selected
            .filter(|(id, _)| *id == page_id)
            .and_then(|(_, id)| {
                self.workspace
                    .as_ref()
                    .and_then(|w| w.annotations(page_id).iter().find(|a| a.id == id).cloned())
            });
        let selected_annotation = self
            .annotation
            .signature
            .gesture
            .as_ref()
            .filter(|_| self.annotation.signature.placement_ghost.is_none())
            .filter(|(id, _, _, _)| *id == page_id)
            .map(|(_, a, _, _)| a.clone())
            .or(selected_annotation);
        let overlay = if !thumbnail {
            let text = self.text.pages.get(&index);
            if text.is_none() && !self.text.errors.contains_key(&index) {
                let weak = cx.weak_entity();
                cx.defer(move |cx| {
                    let _ = weak.update(cx, |this, cx| this.request_text(index, cx));
                });
            }
            let selection = self.text.selection;
            let current_search_match = self.search.current;
            let search_highlights: Vec<_> = if self.search.open {
                self.search
                    .matches
                    .iter()
                    .enumerate()
                    .filter(|(_, hit)| hit.page == index)
                    .flat_map(|(i, hit)| {
                        text.as_ref()
                            .map(|text| text.highlights(hit.range.clone()))
                            .unwrap_or_default()
                            .into_iter()
                            .map(move |rect| (rect, current_search_match == Some(i)))
                    })
                    .collect()
            } else {
                Vec::new()
            };
            let weak = cx.weak_entity();
            let track_ghost = self.active && self.annotation.signature.placement_ghost.is_some();
            let resize_cursor = if geometry.quarter_turns.is_multiple_of(2) {
                gpui::CursorStyle::ResizeUpLeftDownRight
            } else {
                gpui::CursorStyle::ResizeUpRightDownLeft
            };
            let move_cursor = gpui::CursorStyle::PointingHand;
            let edit_signatures = !track_ghost
                && self.annotation.open
                && self.annotation.tool == annotations::Tool::Select
                && self.mode == ViewerMode::Read
                && !self.opening
                && !self.edits_locked();
            let mut cursor_regions = Vec::new();
            let mut gesture_cursor = None;
            if edit_signatures && let Some(workspace) = self.workspace.as_ref() {
                cursor_regions.extend(
                    workspace
                        .annotations(page_id)
                        .iter()
                        .filter(|a| a.is_box_object())
                        .flat_map(|a| {
                            [(false, move_cursor), (true, resize_cursor)].map(|(resize, cursor)| {
                                let [[left, right], [top, bottom]] =
                                    signatures::signature_hit_bounds(a, resize);
                                ([[left, top], [right, bottom]], cursor)
                            })
                        }),
                );
                if self.active
                    && let Some((page, _, _, resize)) = &self.annotation.signature.gesture
                    && *page == page_id
                {
                    gesture_cursor = Some(if *resize { resize_cursor } else { move_cursor });
                }
            }
            canvas(
                move |bounds, window, _| {
                    cursor_regions
                        .into_iter()
                        .map(|(corners, cursor)| {
                            let [a, b] = corners
                                .map(|p| pdf_domain::annotation::rotate(p, geometry.quarter_turns));
                            let to_screen = |x: f32, y: f32| {
                                bounds.origin
                                    + gpui::point(bounds.size.width * x, bounds.size.height * y)
                            };
                            let region = Bounds::from_corners(
                                to_screen(a[0].min(b[0]), a[1].min(b[1])),
                                to_screen(a[0].max(b[0]), a[1].max(b[1])),
                            )
                            .intersect(&bounds);
                            (
                                window.insert_hitbox(region, gpui::HitboxBehavior::Normal),
                                cursor,
                            )
                        })
                        .collect::<Vec<_>>()
                },
                move |bounds, cursor_hitboxes, window, app| {
                    for (hitbox, cursor) in &cursor_hitboxes {
                        window.set_cursor_style(*cursor, hitbox);
                    }
                    if let Some(cursor) = gesture_cursor {
                        window.set_window_cursor_style(cursor);
                    }
                    if let Some((_, a)) = &draft {
                        annotations::paint_draft(
                            a,
                            geometry.quarter_turns,
                            geometry.width_points,
                            bounds,
                            window,
                        );
                    }
                    if let Some(a) = &selected_annotation {
                        annotations::paint_selection(a, geometry.quarter_turns, bounds, window);
                    }
                    for (rect, active) in &search_highlights {
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
                        window.paint_quad(gpui::fill(
                            highlight,
                            gpui::rgba(if *active { 0xff8c0090 } else { 0xffdf0060 }),
                        ));
                    }
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
                    // Global window listeners must honor the page's visible clip. A
                    // scrolled-off page can otherwise intercept toolbar/sidebar clicks.
                    let input_clip = window.content_mask().bounds;
                    let pointer = window.mouse_position();
                    if track_ghost
                        && bounds.contains(&pointer)
                        && input_clip.contains(&pointer)
                        && let Ok(Some((image, rect))) = weak.read_with(app, |view, _| {
                            if !view.document_input_allowed(pointer, window) {
                                return None;
                            }
                            view.signature_ghost_at(page_id, geometry, bounds, pointer)
                        })
                    {
                        let [a, b] = rect.map(|point| {
                            pdf_domain::annotation::rotate(point, geometry.quarter_turns)
                        });
                        let image_bounds = Bounds::new(
                            bounds.origin
                                + gpui::point(
                                    bounds.size.width * a[0].min(b[0]),
                                    bounds.size.height * a[1].min(b[1]),
                                ),
                            gpui::size(
                                bounds.size.width * (a[0] - b[0]).abs(),
                                bounds.size.height * (a[1] - b[1]).abs(),
                            ),
                        );
                        let _ = window.paint_image(
                            image_bounds,
                            image_bounds,
                            gpui::Corners::default(),
                            image,
                            0,
                            false,
                        );
                        if let Some(ghost) = &painted_ghost {
                            ghost.set(Some(rect));
                        }
                    }

                    let down = weak.clone();
                    window.on_mouse_event(
                        move |event: &gpui::MouseDownEvent, phase, window, cx| {
                            if phase == gpui::DispatchPhase::Bubble
                                && event.button == MouseButton::Left
                                && bounds.contains(&event.position)
                                && input_clip.contains(&event.position)
                            {
                                let handled = down.update(cx, |this, cx| {
                                    if !this.document_input_allowed(event.position, window) {
                                        return false;
                                    }
                                    if this.annotation_pointer(
                                        index,
                                        bounds,
                                        event.position,
                                        true,
                                        window,
                                        cx,
                                    ) {
                                        return true;
                                    }
                                    this.select_text(
                                        index,
                                        bounds,
                                        event.position,
                                        event.click_count,
                                        window,
                                        cx,
                                    );
                                    true
                                });
                                if handled.unwrap_or(false) {
                                    cx.stop_propagation();
                                }
                            }
                        },
                    );
                    window.on_mouse_event(
                        move |event: &gpui::MouseMoveEvent, phase, window, cx| {
                            if phase == gpui::DispatchPhase::Bubble
                                && bounds.contains(&event.position)
                                && input_clip.contains(&event.position)
                            {
                                let _ = weak.update(cx, |this, cx| {
                                    if event.pressed_button == Some(MouseButton::Left)
                                        && this.document_input_allowed(event.position, window)
                                        && this.annotation_pointer(
                                            index,
                                            bounds,
                                            event.position,
                                            false,
                                            window,
                                            cx,
                                        )
                                    {
                                        return;
                                    }
                                    // Hover/repaint events can carry no button. End the
                                    // press on mouse-up, not on synthetic hover updates.
                                    if event.pressed_button == Some(MouseButton::Left)
                                        && this.text.dragging
                                        && this.document_input_allowed(event.position, window)
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
        let placing_signature =
            !thumbnail && self.active && self.annotation.signature.placement_ghost.is_some();
        let selectable = !thumbnail
            && !placing_signature
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
            .when(placing_signature, |this| this.cursor_crosshair())
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
                        .border_color(if active || reading {
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
                .when(organizing && !self.opening && !self.edits_locked(), |el| {
                    let weak = cx.weak_entity();
                    let document = self.tab_id;
                    el.on_drag(
                        organizer::PageDrag(document, drag_ids),
                        move |_, _, _, cx| {
                            let _ = weak.update(cx, |this, cx| {
                                this.page_dragging = true;
                                cx.notify();
                            });
                            cx.new(|_| organizer::DragLabel)
                        },
                    )
                    .drag_over::<organizer::PageDrag>(
                        move |style, drag, _, _| {
                            if drag.0 == document {
                                style.border_t_2().border_color(colors.primary)
                            } else {
                                style
                            }
                        },
                    )
                })
                .on_drop(cx.listener(move |this, drag: &organizer::PageDrag, _, cx| {
                    if drag.0 != this.tab_id {
                        return;
                    }
                    this.edit(
                        pdf_domain::Command::Move {
                            ids: drag.1.clone(),
                            target: index,
                        },
                        cx,
                    );
                }))
                .on_click(
                    cx.listener(move |this, event: &gpui::ClickEvent, window, cx| {
                        let m = event.modifiers();
                        if this.mode == ViewerMode::Organize {
                            this.select_page(index, m.control, m.shift, cx);
                        } else {
                            this.go_to(index as u16, cx);
                        }
                        this.page_focus.focus(window, cx);
                    }),
                )
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
                .py_6()
                .child(page)
                .into_any_element()
        }
    }
}

impl Render for DocumentSession {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let c = cx.theme().colors;
        let has_document = self.workspace.is_some();
        let organizing = self.mode == ViewerMode::Organize;
        let dragging_pages = organizing && self.page_dragging && cx.has_active_drag();

        let reading_toolbar = (self.mode != ViewerMode::Convert && !self.split_presentation)
            .then(|| self.reading_toolbar(window, cx));
        let annotation_bar =
            (has_document && self.mode == ViewerMode::Read && self.annotation.open)
                .then(|| self.annotation_bar(cx));
        let search_bar = (has_document && self.search.open && self.mode != ViewerMode::Convert)
            .then(|| self.search_bar(cx));
        let ocr_chip = self.ocr_chip(cx);
        let ocr_batch = self.ocr_batch_controls(cx);

        let body = if self.mode == ViewerMode::Convert {
            self.conversion_hub(window, cx)
        } else if has_document {
            let weak = cx.weak_entity();
            // Use the picture actually painted in this frame for optional UI diagnostics.
            let track_ghost = self.active && self.annotation.signature.placement_ghost.is_some();
            let painted_ghost =
                (track_ghost && self.metrics.is_some()).then(|| Rc::new(Cell::new(None)));
            let list_ghost = painted_ghost.clone();
            let main_list = list(self.page_list.clone(), move |index, _, cx| {
                weak.update(cx, |this, cx| {
                    this.item(index, RenderKind::Page, list_ghost.clone(), cx)
                })
                .unwrap_or_else(|_| div().into_any_element())
            });
            let content_width = self.content_width();
            let weak = cx.weak_entity();
            let scroll_weak = weak.clone();
            let previous = self.viewport;
            let previous_dpr = self.dpr;
            let restore_reading = self.pending_reading.is_some();
            let probe = canvas(
                move |bounds, window, cx| {
                    let viewport = (f32::from(bounds.size.width), f32::from(bounds.size.height));
                    let dpr = window.scale_factor();
                    if restore_reading
                        || (viewport.0 - previous.0).abs() >= 2.
                        || (viewport.1 - previous.1).abs() >= 2.
                        || (dpr - previous_dpr).abs() >= 0.01
                    {
                        cx.defer(move |cx| {
                            let _ = weak.update(cx, |this, cx| {
                                this.viewport_changed(viewport, dpr, cx);
                                this.apply_pending_reading(cx);
                            });
                        });
                    }
                },
                move |bounds, _, window, app| {
                    if let Some(ghost) = &painted_ghost {
                        let rect = ghost.take();
                        let weak = scroll_weak.clone();
                        if weak
                            .read_with(app, |view, _| {
                                view.annotation.signature.signature_ghost_rect != rect
                            })
                            .unwrap_or(false)
                        {
                            app.defer(move |cx| {
                                let _ = weak.update(cx, |view, cx| {
                                    if view.annotation.signature.placement_ghost.is_some() {
                                        view.annotation.signature.signature_ghost_rect = rect;
                                        view.report(cx);
                                    }
                                });
                            });
                        }
                    }
                    if track_ghost {
                        let move_view = scroll_weak.clone();
                        window.on_mouse_event(
                            move |_: &gpui::MouseMoveEvent, phase, window, cx| {
                                if phase == gpui::DispatchPhase::Capture {
                                    let _ = move_view.update(cx, |s, _| {
                                        s.annotation.signature.ghost_pointer_exited = false
                                    });
                                    window.refresh();
                                }
                            },
                        );
                        let exit_view = scroll_weak.clone();
                        window.on_mouse_event(
                            move |_: &gpui::MouseExitEvent, phase, window, cx| {
                                if phase == gpui::DispatchPhase::Capture {
                                    let _ = exit_view.update(cx, |s, _| {
                                        s.annotation.signature.ghost_pointer_exited = true
                                    });
                                    window.refresh();
                                }
                            },
                        );
                    }
                    let release_weak = scroll_weak.clone();
                    window.on_mouse_event(move |event: &gpui::MouseUpEvent, phase, window, cx| {
                        if phase == gpui::DispatchPhase::Capture
                            && event.button == MouseButton::Left
                        {
                            let _ = release_weak.update(cx, |this, cx| {
                                if let Some(page_bounds) = this.annotation.signature.gesture_bounds
                                {
                                    if !page_bounds.contains(&event.position)
                                        || (this.annotation.signature.placement_ghost.is_some()
                                            && (!bounds.contains(&event.position)
                                                || !this.document_input_allowed(
                                                    event.position,
                                                    window,
                                                )))
                                    {
                                        this.annotation.signature.gesture = None;
                                    } else if this.annotation.signature.placement_ghost.is_some()
                                        && let Some((page, _, _, _)) =
                                            &this.annotation.signature.gesture
                                    {
                                        let page = *page;
                                        let index = this.workspace.as_ref().and_then(|w| {
                                            w.pages().iter().position(|p| p.id == page)
                                        });
                                        if let Some(index) = index {
                                            this.annotation_pointer(
                                                index,
                                                page_bounds,
                                                event.position,
                                                false,
                                                window,
                                                cx,
                                            );
                                        }
                                    }
                                }
                                if this.annotation.signature.gesture_bounds.take().is_some() {
                                    cx.notify();
                                }
                                this.finish_annotation(cx);
                                this.text.dragging = false;
                                this.text.pending = None;
                                this.text.sentence_anchor = None;
                                if this.page_dragging {
                                    this.page_dragging = false;
                                    cx.notify();
                                }
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
                            let handled = scroll_weak.update(cx, |this, cx| {
                                if !this.document_input_allowed(event.position, window) {
                                    return false;
                                }
                                this.gesture_zoom(delta, event.position, event.touch_phase, cx);
                                true
                            });
                            if handled.unwrap_or(false) {
                                window.prevent_default();
                                cx.stop_propagation();
                            }
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
                    if !this.document_input_allowed(event.position, window) {
                        return;
                    }
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
                .when(self.navigation.count == 0, |el| {
                    el.child(
                        div()
                            .absolute()
                            .top_4()
                            .left_4()
                            .child("Tidak ada halaman. Impor PDF atau gunakan Undo."),
                    )
                })
                .child(Scrollbar::vertical(&self.page_list))
                .child(Scrollbar::horizontal(&self.horizontal));
            let sidebar = self.thumbnail_sidebar(dragging_pages, cx);
            let panel =
                (organizing && !self.split_presentation).then(|| self.organizer_toolbar(cx));
            let narrow = window.viewport_size().width < px(1100.);
            div()
                .flex_1()
                .min_h_0()
                .flex()
                .relative()
                .child(sidebar)
                .child(main)
                .when_some(panel, |el, panel| {
                    el.child(
                        div()
                            .id("tool-panel")
                            .w(px(280.))
                            .h_full()
                            .flex_shrink_0()
                            .bg(c.background)
                            .border_l_1()
                            .border_color(c.border)
                            .when(narrow, |el| el.absolute().right_0().top_0().shadow_lg())
                            .child(panel),
                    )
                })
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
        Self::document_actions(
        div()
            .size_full()
            .flex()
            .flex_col()
            .bg(c.background)
            .text_color(c.foreground)
            .font_family("Segoe UI")
            .track_focus(&self.focus)
        , cx)
            .when(self.export.running && self.export.optimizing, |el| {
                el.child(div().px_4().py_2().text_sm().child("Optimasi lossless tanpa penurunan kualitas gambar. Ukuran hasil dapat tetap sama atau lebih besar; sumber tetap utuh."))
            })
            .when(self.export.running && self.export.split_total > 0, |el| {
                el.child(div().id("split-progress").px_4().py_2().text_sm().child(format!(
                    "Split PDF: {} dari {} file selesai divalidasi. Folder diterbitkan setelah seluruh batch berhasil.",
                    self.export.split_done.load(std::sync::atomic::Ordering::SeqCst), self.export.split_total)))
            })
            .when(self.export.running && self.export.png, |el| {
                el.child(div().id("png-progress").px_4().py_2().text_sm().child(format!(
                    "PNG: {} dari {} halaman selesai divalidasi. Folder diterbitkan setelah seluruh batch berhasil.",
                    self.export.png_done.load(std::sync::atomic::Ordering::SeqCst), self.export.png_total)))
            })
            .when(
                self.transition.phase == transitions::Phase::WaitingExport,
                |el| {
                    el.child(
                        div()
                            .id("waiting-to-leave")
                            .px_4()
                            .py_2()
                            .flex()
                            .items_center()
                            .gap_3()
                            .child(
                                div()
                                    .flex_1()
                                    .text_sm()
                                    .child("Menunggu ekspor selesai sebelum melanjutkan…"),
                            )
                            .child(
                                Button::new("cancel-leaving")
                                    .ghost()
                                    .label("Tetap di aplikasi")
                                    .on_click(
                                        cx.listener(|this, _, _, cx| this.cancel_transition(cx)),
                                    ),
                            ),
                    )
                },
            )
            .when_some(self.export.notice.clone(), |el, notice| {
                el.child(
                    div()
                        .id("export-notice")
                        .role(gpui::Role::Group)
                        .aria_label(notice.clone())
                        .px_4()
                        .py_2()
                        .flex()
                        .items_center()
                        .gap_3()
                        .border_b_1()
                        .border_color(c.border)
                        .child(div().flex_1().text_sm().child(notice))
                        .child(
                            Button::new("dismiss-export")
                                .ghost()
                                .icon(IconName::X)
                                .accessibility_label("Tutup pemberitahuan ekspor")
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.dismiss_export_notice(cx);
                                })),
                        ),
                )
            })
            .children(reading_toolbar)
            .children(search_bar)
            .children(ocr_batch)
            .children(annotation_bar)

            .child(body)
            .child(
                StatusBar::new()
                    .px_4()
                    .h(px(24.))
                    .py_0()
                    .left(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .when(
                                self.opening || self.inflight.is_some() || self.export.running,
                                |this| this.child(Spinner::new().xsmall()),
                            )
                            .child(if self.export.running {
                                self.export.phase_label().to_owned()
                            } else if self.mode == ViewerMode::Convert {
                                "Konversi - fitur akan datang".to_owned()
                            } else {
                                self.status.clone()
                            })
                            .children(ocr_chip)
                            .when(self.workspace.is_some(), |el| {
                                el.child(format!(
                                    " - {} sumber",
                                    self.workspace.as_ref().map_or(0, |w| w.sources().len())
                                ))
                            }),
                    )
                    .right(if self.workspace.as_ref().is_some_and(|w| w.is_dirty()) {
                        "Perubahan sesi belum diekspor"
                    } else {
                        "File asli tetap utuh"
                    }),
            )
    }
}
