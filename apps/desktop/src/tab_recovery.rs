use super::*;
use crate::reading_position::ReadingPosition;
use gpui::component::{Sizable, input::Input};

#[derive(Clone)]
pub(super) struct ClosedTab {
    path: PathBuf,
    fingerprint: pdf_domain::SourceFingerprint,
    position: ReadingPosition,
    index: usize,
}

impl WindowShell {
    pub(super) fn remember_closed(&mut self, ids: &[TabId], cx: &App) {
        for (index, tab) in self
            .tabs
            .iter()
            .enumerate()
            .filter(|(_, t)| ids.contains(&t.id))
        {
            let Some(session) = tab.preferred_session() else {
                continue;
            };
            let session = session.read(cx);
            let Some(workspace) = session.workspace.as_ref() else {
                continue;
            };
            self.closed_tabs.push_back(ClosedTab {
                path: tab.path.clone(),
                fingerprint: workspace.source().fingerprint,
                position: session.reading_position(),
                index,
            });
            while self.closed_tabs.len() > 20 {
                self.closed_tabs.pop_front();
            }
        }
    }
    pub(super) fn reopen_tab(&mut self, cx: &mut Context<Self>) {
        if !self.enabled(MenuAction::Reopen, cx) {
            return;
        }
        let Some(tab) = self.closed_tabs.back().cloned() else {
            return;
        };
        self.tab_picker_open = false;
        self.reopening_tab = Some(tab.clone());
        self.open_paths(vec![tab.path], cx);
    }
    pub(super) fn finish_reopen(
        &mut self,
        id: TabId,
        session: &Entity<DocumentSession>,
        cx: &mut Context<Self>,
    ) {
        let Some(saved) = &self.reopening_tab else {
            return;
        };
        let Some(index) = self
            .tabs
            .iter()
            .position(|t| t.id == id && path_key(&t.path) == path_key(&saved.path))
        else {
            return;
        };
        let saved = saved.clone();
        let matches = session
            .read(cx)
            .workspace
            .as_ref()
            .is_some_and(|w| w.source().fingerprint == saved.fingerprint);
        if matches {
            session.update(cx, |s, cx| s.queue_reading_position(saved.position, cx));
        }
        let tab = self.tabs.remove(index);
        self.tabs.insert(saved.index.min(self.tabs.len()), tab);
        self.closed_tabs.pop_back();
        self.reopening_tab = None;
    }
    fn filtered_tabs(&self, cx: &App) -> Vec<TabId> {
        let query = self.tab_picker_input.read(cx).value().to_lowercase();
        self.tabs
            .iter()
            .filter(|t| {
                !self.tab_picker_pairing
                    || (Some(t.id) != self.active
                        && t.session
                            .as_ref()
                            .is_some_and(|s| s.read(cx).workspace.is_some()))
            })
            .filter(|t| t.path.to_string_lossy().to_lowercase().contains(&query))
            .map(|t| t.id)
            .collect()
    }
    pub(super) fn toggle_tab_picker(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.close.is_some() || self.modal(cx) {
            return;
        }
        self.tab_picker_open = !self.tab_picker_open;
        self.tab_picker_pairing = false;
        self.tab_picker_selected = 0;
        if self.tab_picker_open {
            self.tab_picker_input.update(cx, |input, cx| {
                input.set_value("", window, cx);
                input.focus(window, cx);
            });
        } else if let Some(id) = self.active {
            self.activate(id, cx);
        }
        cx.notify();
    }
    pub(super) fn tab_picker_key(
        &mut self,
        event: &gpui::KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.tab_picker_open {
            return;
        }
        let ids = self.filtered_tabs(cx);
        self.tab_picker_selected = self.tab_picker_selected.min(ids.len().saturating_sub(1));
        match event.keystroke.key.as_str() {
            "up" => self.tab_picker_selected = self.tab_picker_selected.saturating_sub(1),
            "down" => {
                self.tab_picker_selected =
                    (self.tab_picker_selected + 1).min(ids.len().saturating_sub(1))
            }
            "enter" => {
                if let Some(id) = ids.get(self.tab_picker_selected).copied()
                    && self.tabs.iter().any(|t| t.id == id && t.session.is_some())
                {
                    self.select_picker_tab(id, window, cx);
                }
            }
            "escape" => self.toggle_tab_picker(window, cx),
            _ => return,
        }
        window.prevent_default();
        self.tab_picker_scroll
            .scroll_to_item(self.tab_picker_selected);
        cx.stop_propagation();
        cx.notify();
    }
    pub(super) fn render_tab_picker(&self, cx: &mut Context<Self>) -> AnyElement {
        let ids = self.filtered_tabs(cx);
        let colors = cx.theme().colors;
        div()
            .id("tab-picker")
            .absolute()
            .top(px(68.))
            .right(px(8.))
            .w(px(600.))
            .max_w_full()
            .bg(colors.background)
            .border_1()
            .rounded_md()
            .shadow_lg()
            .occlude()
            .on_mouse_down_out(
                cx.listener(|this, event: &gpui::MouseDownEvent, window, cx| {
                    if this.tab_picker_open && event.position.y > px(68.) {
                        this.toggle_tab_picker(window, cx);
                    }
                }),
            )
            .flex()
            .flex_col()
            .flex_shrink_0()
            .p_2()
            .gap_1()
            .border_b_1()
            .border_color(colors.border)
            .child(
                Input::new(&self.tab_picker_input)
                    .small()
                    .aria_label("Cari tab"),
            )
            .child(
                div()
                    .id("tab-picker-results")
                    .track_scroll(&self.tab_picker_scroll)
                    .max_h(px(240.))
                    .overflow_y_scroll()
                    .when(ids.is_empty(), |el| {
                        el.child(
                            div()
                                .id("empty-tab-search")
                                .role(gpui::Role::Group)
                                .aria_label("Tidak ada tab yang cocok.")
                                .child("Tidak ada tab yang cocok."),
                        )
                    })
                    .children(ids.iter().enumerate().filter_map(|(index, id)| {
                        let tab = self.tabs.iter().find(|t| t.id == *id)?;
                        let id = *id;
                        let dirty = tab.session.as_ref().is_some_and(|s| {
                            s.read(cx).workspace.as_ref().is_some_and(|w| w.is_dirty())
                        });
                        let status = format!(
                            "{}{}{}{}",
                            if Some(id) == self.active {
                                "Aktif "
                            } else {
                                ""
                            },
                            if self.favorites.contains(&path_key(&tab.path)) {
                                "★ "
                            } else {
                                ""
                            },
                            if dirty { "• " } else { "" },
                            if tab.session.is_none() { "Memuat " } else { "" }
                        );
                        Some(
                            div()
                                .flex()
                                .items_center()
                                .gap_2()
                                .when(index == self.tab_picker_selected, |el| {
                                    el.bg(colors.secondary)
                                })
                                .child(
                                    Button::new(("tab-picker-item", id.0))
                                        .ghost()
                                        .min_w_0()
                                        .flex_1()
                                        .label(tab.path.display().to_string())
                                        .disabled(tab.session.is_none())
                                        .on_click(cx.listener(move |this, _, window, cx| {
                                            this.select_picker_tab(id, window, cx);
                                        })),
                                )
                                .child(div().text_xs().child(status)),
                        )
                    })),
            )
            .into_any_element()
    }
}
