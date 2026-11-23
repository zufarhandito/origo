//! Two visible documents on a locked GPUI Kit Dock layout. Closing belongs to the app.
use super::*;
use gpui::base::ElementExt;
use gpui::component::dock::{
    BasePanel, DockArea, DockEvent, DockLayout, DockPlacement, PaneRef, Panel, PanelControl,
    PanelEvent, panel_handle,
};
use gpui::{EventEmitter, Focusable};

#[derive(Clone)]
pub(super) struct SplitView {
    pub ids: [TabId; 2],
    pub views: [ViewId; 2],
    area: Entity<DockArea>,
    pub(super) width: f32,
    organizer_open: bool,
    pub(super) origin_x: f32,
    pub(super) divider_x: Option<f32>,
    pub(super) dragging: bool,
}

struct PdfPane {
    id: ViewId,
    name: String,
    session: Entity<DocumentSession>,
    owner: gpui::WeakEntity<WindowShell>,
    focus: FocusHandle,
    is_left: bool,
}
impl BasePanel for PdfPane {
    fn panel_name(&self) -> &'static str {
        "pdf-document-pane"
    }
    fn closable(&self, _: &App) -> bool {
        false
    }
    fn zoomable(&self, _: &App) -> bool {
        false
    }
}
impl Panel for PdfPane {
    fn title(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        self.name.clone()
    }
    fn title_bar(&self, _: &App) -> bool {
        false
    }
    fn inner_padding(&self, _: &App) -> bool {
        false
    }
    fn zoom_control(&self, _: &App) -> Option<PanelControl> {
        None
    }
}
impl EventEmitter<PanelEvent> for PdfPane {}
impl Focusable for PdfPane {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}
impl Render for PdfPane {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let colors = cx.theme().colors;
        let active = self.session.read(cx).active;
        let geometry_owner = self.owner.clone();
        let is_left = self.is_left;
        div()
            .id(("pdf-pane", self.id.0))
            .role(gpui::Role::Group)
            .aria_label(format!("Panel PDF: {}", self.name))
            .size_full()
            .min_w_0()
            .min_h_0()
            .relative()
            .flex()
            .flex_col()
            .overflow_hidden()
            .border_2()
            .border_color(if active {
                colors.primary
            } else {
                colors.border
            })
            .on_prepaint(move |bounds, _, cx| {
                if is_left {
                    let _ = geometry_owner.update(cx, |shell, _| {
                        if let Some(split) = &mut shell.split_view {
                            split.origin_x = f32::from(bounds.left());
                            split.divider_x = Some(f32::from(bounds.right()));
                        }
                    });
                }
            })
            .capture_any_mouse_down(cx.listener(|this, _, window, cx| {
                let focused = this
                    .owner
                    .update(cx, |shell, cx| shell.focus_visible_view(this.id, cx))
                    .unwrap_or(false);
                if focused {
                    this.session.read(cx).focus.clone().focus(window, cx);
                }
            }))
            .child(
                div()
                    .flex()
                    .items_center()
                    .flex_shrink_0()
                    .gap_1()
                    .px_2()
                    .child(div().flex_1().min_w_0().truncate().child(format!(
                        "{}{}",
                        self.name,
                        if active { " — Aktif" } else { "" }
                    )))
                    .child(
                        Button::new(("single-pane", self.id.0))
                            .ghost()
                            .label("Tampilan tunggal")
                            .accessibility_label(format!("Tampilan tunggal: {}", self.name))
                            .on_click(cx.listener(|this, _, _, cx| {
                                let _ = this.owner.update(cx, |shell, cx| {
                                    if shell.focus_visible_view(this.id, cx) {
                                        shell.exit_comparison(cx);
                                    }
                                });
                            })),
                    )
                    .child(
                        Button::new(("close-reading-view", self.id.0))
                            .ghost()
                            .icon(IconName::X)
                            .accessibility_label(format!("Tutup tampilan: {}", self.name))
                            .on_click(cx.listener(|this, _, _, cx| {
                                let _ = this
                                    .owner
                                    .update(cx, |shell, cx| shell.close_reading_view(this.id, cx));
                            })),
                    ),
            )
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .min_w_0()
                    .child(self.session.clone()),
            )
    }
}

fn clamp_left(width: f32, left: f32) -> Option<f32> {
    (width.is_finite() && left.is_finite() && width >= 640.).then(|| left.clamp(320., width - 320.))
}

impl WindowShell {
    pub(super) fn choose_comparison(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.enabled(MenuAction::Compare, cx) {
            return;
        }
        self.tab_picker_open = false;
        self.toggle_tab_picker(window, cx);
        self.tab_picker_pairing = true;
        cx.notify();
    }
    pub(super) fn select_picker_tab(
        &mut self,
        id: TabId,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.tab_picker_pairing {
            if let Some(left) = self.active {
                self.enter_comparison(left, id, window, cx);
            }
        } else {
            self.tab_picker_open = false;
            self.switch(id, cx);
        }
    }
    pub(super) fn enter_comparison(
        &mut self,
        left: TabId,
        right: TabId,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.close.is_some() || self.modal(cx) || self.opening {
            return;
        }
        let mut panes = Vec::new();
        let mut views = Vec::new();
        for (pane_index, id) in [left, right].into_iter().enumerate() {
            let Some(tab) = self.tabs.iter().find(|t| t.id == id) else {
                return;
            };
            let Some(session) = (if left == right {
                if pane_index == 0 {
                    tab.session.clone()
                } else {
                    tab.secondary.clone()
                }
            } else {
                tab.preferred_session()
            })
            .filter(|s| s.read(cx).workspace.is_some()) else {
                return;
            };
            let mut name = tab
                .path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned();
            if left == right {
                name.push_str(if pane_index == 0 {
                    " (kiri)"
                } else {
                    " (kanan)"
                });
            }
            let id = session.read(cx).view_id;
            views.push(id);
            let owner = cx.weak_entity();
            panes.push(cx.new(|cx| {
                let focus = cx.focus_handle();
                cx.on_focus(&focus, window, |pane: &mut PdfPane, window, cx| {
                    if pane.focus.is_focused(window) {
                        pane.session.read(cx).focus.clone().focus(window, cx);
                    }
                })
                .detach();
                PdfPane {
                    id,
                    name,
                    session,
                    owner,
                    focus,
                    is_left: pane_index == 0,
                }
            }));
        }
        let organizer_open = self
            .tabs
            .iter()
            .find(|t| t.id == left)
            .and_then(Tab::preferred_session)
            .is_some_and(|s| s.read(cx).mode == ViewerMode::Organize);
        let width = f32::from(window.viewport_size().width)
            - if organizer_open && window.viewport_size().width >= px(1100.) {
                280.
            } else {
                0.
            };
        // Uncached panels keep native accessibility nodes available after modal
        // and focus changes in this integration. Dock still owns layout/resize;
        // PDF panes supply their own themed chrome.
        let area = cx.new(|cx| DockArea::new("pdf-comparison", None, window, cx));
        let layout = DockLayout::h_split()
            .child(
                DockLayout::tabs().panel_view(panel_handle(panes[0].clone()), cx),
                Some(px(width / 2.)),
            )
            .child(
                DockLayout::tabs().panel_view(panel_handle(panes[1].clone()), cx),
                None,
            );
        area.update(cx, |area, cx| {
            area.set_center(layout, window, cx);
            area.set_locked(true, window, cx);
        });
        self.split_view = Some(SplitView {
            ids: [left, right],
            views: [views[0], views[1]],
            area: area.clone(),
            width,
            organizer_open,
            origin_x: 0.,
            divider_x: None,
            dragging: false,
        });
        cx.subscribe_in(&area, window, |this, _, event: &DockEvent, window, cx| {
            if matches!(event, DockEvent::LayoutChanged) {
                this.clamp_comparison(window, cx);
            }
        })
        .detach();
        self.set_comparison_organizer(organizer_open, cx);
        if left == right
            && let Some(tab) = self.tabs.iter_mut().find(|t| t.id == left)
        {
            tab.preferred_secondary = false;
        }
        self.activate(left, cx);
    }
    pub(super) fn exit_comparison(&mut self, cx: &mut Context<Self>) {
        if self.close.is_some() || self.modal(cx) {
            return;
        }
        self.split_view = None;
        self.resources
            .organizer_drawer
            .store(false, Ordering::SeqCst);
        self.sync_visible_sessions(cx);
        if let Some(id) = self.active {
            self.activate(id, cx);
        }
        cx.notify();
    }
    pub(crate) fn focus_visible_view(&mut self, id: ViewId, cx: &mut Context<Self>) -> bool {
        if self.close.is_some()
            || self.modal(cx)
            || !self
                .split_view
                .as_ref()
                .is_some_and(|s| s.views.contains(&id))
        {
            return false;
        }
        let Some(tab) = self
            .tabs
            .iter_mut()
            .find(|t| t.views().any(|s| s.read(cx).view_id == id))
        else {
            return false;
        };
        let doc = tab.id;
        let secondary = id.0 % 2 == 1;
        if self.active != Some(doc) || tab.preferred_secondary != secondary {
            tab.preferred_secondary = secondary;
            self.active = Some(doc);
            self.sync_visible_sessions(cx);
            cx.notify();
        }
        true
    }
    fn close_reading_view(&mut self, id: ViewId, cx: &mut Context<Self>) {
        let other = self
            .split_view
            .as_ref()
            .and_then(|s| s.views.iter().copied().find(|view| *view != id));
        if let Some(other) = other
            && self.focus_visible_view(other, cx)
        {
            self.exit_comparison(cx);
        }
    }
    pub(crate) fn set_comparison_organizer(&mut self, open: bool, cx: &mut Context<Self>) {
        let Some(split) = &mut self.split_view else {
            return;
        };
        split.organizer_open = open;
        let ids = split.ids;
        for session in self
            .tabs
            .iter()
            .filter(|t| ids.contains(&t.id))
            .flat_map(Tab::views)
        {
            session.update(cx, |s, cx| {
                s.mode = if open {
                    ViewerMode::Organize
                } else {
                    ViewerMode::Read
                };
                if open {
                    s.sidebar = true;
                }
                s.release_hidden_thumbnails(cx);
                cx.notify();
            });
        }
        cx.notify();
    }
    pub(super) fn sync_visible_sessions(&mut self, cx: &mut Context<Self>) {
        let ids = self.split_view.as_ref().map(|s| s.views);
        let focused_entity = self.session();
        for tab in &self.tabs {
            for session in tab.views() {
                let view = session.read(cx).view_id;
                let focused = focused_entity.as_ref() == Some(session);
                let visible = ids.map_or(focused, |ids| ids.contains(&view));
                let split = ids.is_some_and(|ids| ids.contains(&view));
                let divided = if split { 2 } else { 1 };
                session.update(cx, |s, cx| {
                    let was_visible = s.visible_for_render;
                    let changed = s.active != focused
                        || was_visible != visible
                        || s.split_presentation != split;
                    if !changed {
                        return;
                    }
                    if !focused && s.active {
                        s.text.cancel_copy();
                        s.text.dragging = false;
                        s.text.pending = None;
                        s.page_dragging = false;
                    }
                    if s.split_presentation != split {
                        s.invalidate_work();
                        s.clear_bitmaps(cx);
                        s.pages = ByteCache::new(self.page_budget / divided);
                        s.thumbnails = ByteCache::new(self.thumbnail_budget / divided);
                        s.text_budget = 16 * 1024 * 1024 / divided;
                        s.text.pages = ByteCache::new(s.text_budget);
                        s.queue = RenderQueue::new(32 / divided);
                        s.queue.reset(s.generation);
                    }
                    if s.active && !focused {
                        s.cancel_signature_placement(cx);
                    }
                    s.active = focused;
                    s.visible_for_render = visible;
                    if !visible {
                        s.annotation.draft = None;
                        s.close_annotation_popups(cx);
                    }
                    s.split_presentation = split;
                    if !visible {
                        s.search.pause();
                        s.invalidate_work();
                        s.clear_bitmaps(cx);
                        s.text.pages.clear();
                        s.text.pending = None;
                        s.text.cancel_copy();
                        s.page_dragging = false;
                    } else if !was_visible {
                        if s.search.running {
                            s.restart_search(cx);
                        }
                        s.schedule_current(cx);
                    } else {
                        s.schedule_current(cx);
                    }
                    s.report(cx);
                    cx.notify();
                });
            }
        }
    }
    fn clamp_comparison(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(split) = &self.split_view else {
            return;
        };
        let area = split.area.clone();
        let width = split.width;
        let change = area
            .read(cx)
            .layout(DockPlacement::Center)
            .and_then(|tree| {
                let root = tree.root();
                let PaneRef::Split { sizes, .. } = root.kind() else {
                    return None;
                };
                let left = sizes
                    .first()
                    .copied()
                    .flatten()
                    .map(f32::from)
                    .unwrap_or(width / 2.);
                let clamped = clamp_left(width, left)?;
                ((left - clamped).abs() > 0.5).then_some((root.id(), clamped))
            });
        if let Some((node, left)) = change {
            area.update(cx, |area, cx| {
                area.set_split_sizes(node, vec![px(left), px(width - left)], window, cx)
            });
        }
    }
    pub(super) fn render_comparison(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let Some(split) = &self.split_view else {
            return div().into_any_element();
        };
        let area = split.area.clone();
        let previous_width = split.width;
        let organizer_open = split.organizer_open;
        let narrow = window.viewport_size().width < px(1100.);
        self.resources
            .organizer_drawer
            .store(organizer_open && narrow, Ordering::SeqCst);
        let Some(session) = self.session() else {
            return div().into_any_element();
        };
        let toolbar = session.update(cx, |s, cx| {
            let toolbar = s.reading_toolbar(window, cx);
            DocumentSession::document_actions(div(), cx)
                .child(toolbar)
                .into_any_element()
        });
        let tools = organizer_open.then(|| {
            session.update(cx, |s, cx| {
                let name = s
                    .workspace
                    .as_ref()
                    .map(|w| {
                        w.source()
                            .path
                            .file_name()
                            .unwrap_or_default()
                            .to_string_lossy()
                            .into_owned()
                    })
                    .unwrap_or_default();
                let tools = s.organizer_toolbar(cx);
                DocumentSession::document_actions(div(), cx)
                    .child(
                        div()
                            .id("comparison-tool-document")
                            .role(gpui::Role::Group)
                            .aria_label(format!("Dokumen: {name}"))
                            .p_2()
                            .child(format!("Dokumen: {name}")),
                    )
                    .child(tools)
                    .into_any_element()
            })
        });
        let colors = cx.theme().colors;
        let weak = cx.weak_entity();
        let resize_owner = weak.clone();
        div()
            .size_full()
            .flex()
            .flex_col()
            .min_h_0()
            .child(toolbar)
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .flex()
                    .relative()
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .min_h_0()
                            .relative()
                            .child(area)
                            .child(
                                gpui::canvas(
                                    move |bounds, _, cx| {
                                        let width = f32::from(bounds.size.width);
                                        if (width - previous_width).abs() >= 1. {
                                            let weak = weak.clone();
                                            cx.defer(move |cx| {
                                                let _ = weak.update(cx, |this, cx| {
                                                    if let Some(split) = &mut this.split_view {
                                                        split.width = width;
                                                    }
                                                    let handle = this.handle;
                                                    let owner = cx.weak_entity();
                                                    cx.defer(move |cx| {
                                                        let _ =
                                                            handle.update(cx, |_, window, cx| {
                                                                let _ = owner.update(
                                                                    cx,
                                                                    |shell, cx| {
                                                                        shell.clamp_comparison(
                                                                            window, cx,
                                                                        )
                                                                    },
                                                                );
                                                            });
                                                    });
                                                });
                                            });
                                        }
                                    },
                                    move |bounds, _, window, _| {
                                        let down = resize_owner.clone();
                                        window.on_mouse_event(
                                            move |event: &gpui::MouseDownEvent, phase, _, cx| {
                                                if phase == gpui::DispatchPhase::Capture
                                                    && event.button == gpui::MouseButton::Left
                                                {
                                                    let _ = down.update(cx, |shell, _| {
                                                        if let Some(split) = &mut shell.split_view {
                                                            split.dragging = bounds
                                                                .contains(&event.position)
                                                                && split.divider_x.is_some_and(
                                                                    |x| {
                                                                        (f32::from(
                                                                            event.position.x,
                                                                        ) - x)
                                                                            .abs()
                                                                            <= 6.
                                                                    },
                                                                );
                                                        }
                                                    });
                                                }
                                            },
                                        );
                                        let moving = resize_owner.clone();
                                        window.on_mouse_event(
                                            move |event: &gpui::MouseMoveEvent, phase, _, cx| {
                                                if phase != gpui::DispatchPhase::Capture {
                                                    return;
                                                }
                                                let block = moving
                                                    .update(cx, |shell, _| {
                                                        let Some(split) = &mut shell.split_view
                                                        else {
                                                            return false;
                                                        };
                                                        if !split.dragging {
                                                            return false;
                                                        }
                                                        // Dock continues resizing until MouseUp,
                                                        // including hover/repaint move events.
                                                        let left = f32::from(event.position.x)
                                                            - split.origin_x;
                                                        left < 320. || left > split.width - 320.
                                                    })
                                                    .unwrap_or(false);
                                                if block {
                                                    cx.stop_propagation();
                                                }
                                            },
                                        );
                                        window.on_mouse_event(
                                            move |_: &gpui::MouseUpEvent, phase, _, cx| {
                                                if phase == gpui::DispatchPhase::Capture {
                                                    let _ = resize_owner.update(cx, |shell, _| {
                                                        if let Some(split) = &mut shell.split_view {
                                                            split.dragging = false;
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
                                .size_full(),
                            ),
                    )
                    .when_some(tools, |el, tools| {
                        el.child(
                            div()
                                .id("comparison-tools")
                                .w(px(280.))
                                .h_full()
                                .flex_shrink_0()
                                .bg(colors.background)
                                .border_l_1()
                                .border_color(colors.border)
                                .occlude()
                                .when(narrow, |el| el.absolute().right_0().top_0().shadow_lg())
                                .child(tools),
                        )
                    }),
            )
            .into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn pane_sizes_keep_two_readable_panels() {
        assert_eq!(clamp_left(860., 100.), Some(320.));
        assert_eq!(clamp_left(860., 800.), Some(540.));
        assert_eq!(clamp_left(1200., 600.), Some(600.));
        assert_eq!(clamp_left(640., 500.), Some(320.));
        assert_eq!(clamp_left(0., 0.), None);
        assert_eq!(clamp_left(860., f32::NAN), None);
    }
}
