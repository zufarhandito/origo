use super::*;
impl WindowShell {
    pub(super) fn load_recents(&mut self, cx: &mut Context<Self>) {
        self.pending_preferences += 1;
        let directory = self.state_dir.clone();
        cx.spawn(async move |this,cx| {
            let result=cx.background_executor().spawn(async move {
                crate::local_state::load_recents(&directory.ok_or_else(||anyhow::anyhow!("State tidak tersedia"))?)
            }).await;
            let _=this.update(cx,|s,cx| {
                s.pending_preferences = s.pending_preferences.saturating_sub(1);
                match result {
                    Ok(paths)=>{
                        for path in paths {if !s.recents.iter().any(|p|path_key(p)==path_key(&path)) {s.recents.push(path);}}
                        s.recents.truncate(20);s.recents_ready=true;
                        if !s.recents.is_empty(){s.persist_recents(cx);}
                    }
                    Err(_)=>s.notice=Some("Riwayat terakhir dibuka tidak dapat dibaca. Penyimpanan riwayat dinonaktifkan agar data lama tetap aman.".into()),
                }
                s.advance_close(cx);
                cx.notify();
            });
        }).detach();
    }
    pub(super) fn record_recent(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        self.recents.retain(|p| path_key(p) != path_key(&path));
        self.recents.insert(0, path);
        self.recents.truncate(20);
        self.persist_recents(cx);
        cx.notify();
    }
    fn persist_recents(&mut self, cx: &mut Context<Self>) {
        if !self.recents_ready {
            return;
        }
        self.recents_revision += 1;
        let revision = self.recents_revision;
        let paths = self.recents.clone();
        let directory = self.state_dir.clone();
        let lock = self.recents_writer.clone();
        self.pending_preferences += 1;
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_executor()
                .spawn(async move {
                    let mut written = lock
                        .lock()
                        .map_err(|_| anyhow::anyhow!("Riwayat terkunci"))?;
                    if revision <= *written {
                        return Ok(());
                    }
                    crate::local_state::save_recents(
                        &directory.ok_or_else(|| anyhow::anyhow!("State tidak tersedia"))?,
                        &paths,
                    )?;
                    *written = revision;
                    Ok::<_, anyhow::Error>(())
                })
                .await;
            let _ = this.update(cx, |s, cx| {
                s.pending_preferences = s.pending_preferences.saturating_sub(1);
                if result.is_err() {
                    s.notice =
                        Some("Riwayat berubah untuk sesi ini, tetapi gagal disimpan.".into());
                }
                s.advance_close(cx);
                cx.notify();
            });
        })
        .detach();
    }
    pub(super) fn recent_list(&self, cx: &mut Context<Self>) -> AnyElement {
        if self.recents.is_empty() {
            return div().into_any_element();
        }
        div()
            .w_full()
            .mt_4()
            .flex()
            .flex_col()
            .gap_1()
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(div().text_sm().child("Terakhir dibuka"))
                    .child(
                        Button::new("clear-recents")
                            .ghost()
                            .label("Hapus riwayat")
                            .disabled(!self.recents_ready || self.opening)
                            .on_click(cx.listener(|s, _, _, cx| {
                                s.recents.clear();
                                s.persist_recents(cx);
                                cx.notify();
                            })),
                    ),
            )
            .children(
                self.recents
                    .iter()
                    .take(5)
                    .enumerate()
                    .map(|(index, path)| {
                        let path = path.clone();
                        let full = path.to_string_lossy().into_owned();
                        let name = path
                            .file_name()
                            .unwrap_or_default()
                            .to_string_lossy()
                            .into_owned();
                        Button::new(("recent-file", index))
                            .ghost()
                            .icon(IconName::File)
                            .label(name.clone())
                            .accessibility_label(format!("Buka terakhir: {name}"))
                            .tooltip(full)
                            .w_full()
                            .disabled(self.opening)
                            .on_click(
                                cx.listener(move |s, _, _, cx| {
                                    s.open_paths(vec![path.clone()], cx)
                                }),
                            )
                    }),
            )
            .into_any_element()
    }
}
