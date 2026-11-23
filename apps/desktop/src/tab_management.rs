//! Tab actions share the shell's stable identity and close guard.
use super::*;
#[derive(Clone)]
pub(super) struct TabDrag(pub(super) TabId);
pub(super) struct TabDragLabel;
impl Render for TabDragLabel {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().p_2().child("Pindahkan tab")
    }
}

impl WindowShell {
    pub(super) fn load_favorites(&mut self, cx: &mut Context<Self>) {
        let directory = self.state_dir.clone();
        cx.spawn(async move |this, cx| {
            let result = cx.background_executor().spawn(async move {
                directory
                    .ok_or_else(|| anyhow::anyhow!("Direktori preferensi tidak tersedia"))
                    .and_then(|dir| crate::local_state::load_favorites(&dir))
            }).await;
            let _ = this.update(cx, |this, cx| {
                match result {
                    Ok(stars) => { this.favorites = stars; this.favorites_ready = true; }
                    Err(_) => this.notice = Some("Preferensi favorit tidak dapat dibaca. Bintang dinonaktifkan agar data lama tetap aman.".into()),
                }
                cx.notify();
            });
        }).detach();
    }
    pub(super) fn toggle_favorite(&mut self, id: TabId, cx: &mut Context<Self>) {
        if self.active != Some(id)
            || !self.favorites_ready
            || self.close.is_some()
            || self.modal(cx)
        {
            return;
        }
        let Some(tab) = self.tabs.iter().find(|t| t.id == id) else {
            return;
        };
        let key = path_key(&tab.path);
        if !self.favorites.remove(&key) {
            self.favorites.insert(key);
        }
        let favorites = self.favorites.clone();
        self.favorites_revision += 1;
        let revision = self.favorites_revision;
        let lock = self.favorites_writer.clone();
        let directory = self.state_dir.clone();
        self.pending_preferences += 1;
        cx.spawn(async move |this, cx| {
            let result = cx.background_executor().spawn(async move {
                let mut written = lock.lock().map_err(|_| anyhow::anyhow!("Preferensi terkunci"))?;
                if revision <= *written { return Ok(()); }
                let dir = directory.ok_or_else(|| anyhow::anyhow!("Direktori preferensi tidak tersedia"))?;
                crate::local_state::save_favorites(&dir, &favorites)?;
                *written = revision;
                Ok::<_, anyhow::Error>(())
            }).await;
            let _ = this.update(cx, |this, cx| {
                this.pending_preferences = this.pending_preferences.saturating_sub(1);
                    if result.is_err() && this.favorites_revision == revision {
                        this.notice = Some("Bintang berubah untuk sesi ini, tetapi gagal disimpan. Coba ubah bintang lagi untuk menyimpan.".into());
                        cx.notify();
                    }
                this.advance_close(cx);
                cx.notify();
            });
        }).detach();
        cx.notify();
    }
    // Insert before a target identity; None means the end. Never recreate a session.
    pub(super) fn move_tab(&mut self, id: TabId, before: Option<TabId>, cx: &mut Context<Self>) {
        if self.close.is_some() || self.modal(cx) || before == Some(id) {
            return;
        }
        if before.is_some_and(|target| !self.tabs.iter().any(|t| t.id == target)) {
            return;
        }
        let Some(index) = self.tabs.iter().position(|t| t.id == id) else {
            return;
        };
        let tab = self.tabs.remove(index);
        let target = before
            .and_then(|id| self.tabs.iter().position(|t| t.id == id))
            .unwrap_or(self.tabs.len());
        self.tabs.insert(target, tab);
        cx.notify();
    }
}

pub(super) fn tab_context_menu(
    mut menu: gpui::component::menu::PopupMenu,
    weak: gpui::WeakEntity<WindowShell>,
    id: TabId,
    cx: &mut Context<gpui::component::menu::PopupMenu>,
) -> gpui::component::menu::PopupMenu {
    let Some(shell) = weak.upgrade() else {
        return menu;
    };
    let shell = shell.read(cx);
    let Some(index) = shell.tabs.iter().position(|t| t.id == id) else {
        return menu;
    };
    let blocked = shell.close.is_some() || shell.modal(cx);
    let starred = shell.favorites.contains(&path_key(&shell.tabs[index].path));
    let actions = [
        (
            if starred { "Hapus bintang" } else { "Bintangi" },
            0,
            blocked || !shell.favorites_ready,
        ),
        ("Pindahkan ke awal", 1, blocked || index == 0),
        (
            "Pindahkan ke akhir",
            2,
            blocked || index + 1 == shell.tabs.len(),
        ),
        ("Tutup tab", 3, blocked || shell.opening),
        (
            "Tutup tab lainnya",
            4,
            blocked || shell.opening || shell.tabs.len() == 1,
        ),
        (
            "Tutup tab di kanan",
            5,
            blocked || shell.opening || index + 1 == shell.tabs.len(),
        ),
    ];
    for (label, action, disabled) in actions {
        if action == 0 && shell.active != Some(id) {
            continue;
        }
        let weak = weak.clone();
        menu = menu.item(
            PopupMenuItem::new(label)
                .disabled(disabled)
                .on_click(move |_, _, cx| {
                    let _ = weak.update(cx, |this, cx| {
                        let Some(index) = this.tabs.iter().position(|t| t.id == id) else {
                            return;
                        };
                        match action {
                            0 => this.toggle_favorite(id, cx),
                            1 => this.move_tab(id, this.tabs.first().map(|t| t.id), cx),
                            2 => this.move_tab(id, None, cx),
                            _ => {
                                let ids = this
                                    .tabs
                                    .iter()
                                    .enumerate()
                                    .filter(|(i, t)| match action {
                                        3 => t.id == id,
                                        4 => t.id != id,
                                        _ => *i > index,
                                    })
                                    .map(|(_, t)| t.id)
                                    .collect();
                                this.begin_close_ids(false, ids, cx);
                            }
                        }
                    });
                }),
        );
    }
    let disabled = !shell.enabled(MenuAction::Reopen, cx);
    menu = menu.item(
        PopupMenuItem::new("Buka ulang tab tertutup")
            .disabled(disabled)
            .on_click(move |_, _, cx| {
                let _ = weak.update(cx, |this, cx| this.reopen_tab(cx));
            }),
    );
    menu
}
