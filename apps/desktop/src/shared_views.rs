//! Reading views belong to one tab and retain a single canonical job/close actor.
use super::*;

impl Tab {
    pub(super) fn preferred_session(&self) -> Option<Entity<DocumentSession>> {
        if self.preferred_secondary {
            self.secondary.clone().or_else(|| self.session.clone())
        } else {
            self.session.clone()
        }
    }
    pub(super) fn views(&self) -> impl Iterator<Item = &Entity<DocumentSession>> {
        self.session.iter().chain(self.secondary.iter())
    }
}
impl WindowShell {
    pub(super) fn observe_document_view(
        &mut self,
        session: &Entity<DocumentSession>,
        id: TabId,
        actor: bool,
        cx: &mut Context<Self>,
    ) {
        cx.observe(session, move |_, _, cx| {
            let weak = cx.weak_entity();
            cx.defer(move |cx| {
                let _ = weak.update(cx, |this, cx| {
                    this.sync_document_views(id, actor, cx);
                    this.advance_close(cx);
                    cx.notify();
                });
            });
        })
        .detach();
    }
    fn sync_document_views(&mut self, id: TabId, actor_changed: bool, cx: &mut Context<Self>) {
        let Some(tab) = self.tabs.iter().find(|t| t.id == id) else {
            return;
        };
        let actor = tab.session.clone();
        let views: Vec<_> = tab.views().cloned().collect();
        let export = actor_changed
            .then(|| actor.as_ref().map(|s| s.read(cx).export.clone()))
            .flatten();
        for view in views {
            let mirror = export.clone().filter(|_| actor.as_ref() != Some(&view));
            view.update(cx, |s, cx| {
                s.sync_shared_model(cx);
                if let Some(export) = mirror {
                    s.export = export;
                    cx.notify();
                }
            });
        }
    }
    pub(crate) fn restore_document_focus(&self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(view) = self.session() {
            view.read(cx).focus.clone().focus(window, cx);
        }
    }
    fn ensure_secondary(
        &mut self,
        id: TabId,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<Entity<DocumentSession>> {
        let tab = self.tabs.iter().find(|t| t.id == id)?;
        if let Some(view) = &tab.secondary {
            return Some(view.clone());
        }
        let actor = tab.session.clone()?;
        if actor.read(cx).workspace.is_none() {
            return None;
        }
        let owner = cx.weak_entity();
        let secondary = DocumentSession::new(
            window,
            cx,
            &self.sidecars,
            self.renderer.clone(),
            self.page_budget,
            self.thumbnail_budget,
            self.metrics.clone(),
            owner,
            self.resources.clone(),
            id,
            ViewId(id.0 * 2 + 1),
        );
        let model = actor.read(cx).workspace.clone();
        let instance = actor.read(cx).workspace_instance;
        let index = actor.read(cx).navigation.current;
        let zoom = actor.read(cx).zoom;
        let sidebar = actor.read(cx).sidebar;
        let mode = actor.read(cx).mode;
        let export = actor.read(cx).export.clone();
        secondary.update(cx, |s, cx| {
            s.document_owner = Some(actor.downgrade());
            s.workspace = model;
            s.workspace_instance = instance;
            s.navigation.count = s.workspace.as_ref().map_or(0, |w| w.pages().len() as u16);
            s.navigation.current = index;
            s.zoom = zoom;
            s.sidebar = sidebar;
            s.mode = mode;
            s.export = export;
            s.observed_model = s.workspace.signature();
            s.remember_view_page();
            s.reset_layout();
            s.update_status();
            cx.notify();
        });
        self.observe_document_view(&secondary, id, false, cx);
        self.tabs.iter_mut().find(|t| t.id == id)?.secondary = Some(secondary.clone());
        Some(secondary)
    }
    pub(super) fn duplicate_view(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.enabled(MenuAction::DuplicateView, cx) {
            return;
        }
        let Some(id) = self.active else {
            return;
        };
        if self.ensure_secondary(id, window, cx).is_some() {
            self.enter_comparison(id, id, window, cx);
        }
    }
}
