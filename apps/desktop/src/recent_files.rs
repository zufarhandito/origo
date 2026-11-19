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
                s.refresh_recent_metadata(cx);
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
        self.refresh_recent_metadata(cx);
        cx.notify();
    }
    fn refresh_recent_metadata(&mut self, cx: &mut Context<Self>) {
        self.recent_metadata_revision += 1;
        let revision = self.recent_metadata_revision;
        self.recent_modified.clear();
        let paths = self.recents.iter().take(5).cloned().collect::<Vec<_>>();
        if paths.is_empty() {
            return;
        }
        cx.spawn(async move |this, cx| {
            let entries = cx
                .background_executor()
                .spawn(async move {
                    paths
                        .into_iter()
                        .map(|path| {
                            let modified = std::fs::metadata(&path).and_then(|m| m.modified()).ok();
                            (path_key(&path), modified)
                        })
                        .collect::<HashMap<_, _>>()
                })
                .await;
            let _ = this.update(cx, |s, cx| {
                if s.recent_metadata_revision == revision {
                    s.recent_modified = entries;
                    cx.notify();
                }
            });
        })
        .detach();
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
                                s.refresh_recent_metadata(cx);
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
                        let starred = self.favorites.contains(&path_key(&path));
                        let folder = path
                            .parent()
                            .map(|p| p.to_string_lossy().into_owned())
                            .unwrap_or_default();
                        let modified = match self.recent_modified.get(&path_key(&path)) {
                            Some(Some(time)) => format!(
                                "Diubah {}",
                                chrono::DateTime::<chrono::Local>::from(*time)
                                    .format("%d-%m-%Y %H:%M")
                            ),
                            Some(None) => "Tanggal perubahan tidak tersedia".into(),
                            None => "Memuat tanggal perubahan...".into(),
                        };
                        Button::new(("recent-file", index))
                            .ghost()
                            .h_auto()
                            .min_h(px(64.))
                            .py_2()
                            .child(
                                div()
                                    .flex()
                                    .items_start()
                                    .gap_2()
                                    .w_full()
                                    .min_w_0()
                                    .text_left()
                                    .child(
                                        gpui::component::Icon::new(IconName::File)
                                            .size(px(16.))
                                            .flex_shrink_0(),
                                    )
                                    .child(
                                        div()
                                            .flex_1()
                                            .min_w_0()
                                            .flex()
                                            .flex_col()
                                            .gap_1()
                                            .child(
                                                div()
                                                    .flex()
                                                    .items_center()
                                                    .gap_1()
                                                    .child(
                                                        div()
                                                            .flex_1()
                                                            .min_w_0()
                                                            .truncate()
                                                            .child(name.clone()),
                                                    )
                                                    .when(starred, |row| {
                                                        row.child(
                                                            div()
                                                                .id(("recent-star", index))
                                                                .role(gpui::Role::Group)
                                                                .aria_label(format!(
                                                                    "Favorit: {name}"
                                                                ))
                                                                .child(
                                                                    gpui::component::Icon::new(
                                                                        IconName::Star,
                                                                    )
                                                                    .size(px(14.))
                                                                    .text_color(cx.theme().warning)
                                                                    .flex_shrink_0(),
                                                                ),
                                                        )
                                                    }),
                                            )
                                            .child(
                                                div()
                                                    .w_full()
                                                    .min_w_0()
                                                    .truncate()
                                                    .text_xs()
                                                    .text_color(cx.theme().muted_foreground)
                                                    .id(("recent-location", index))
                                                    .role(gpui::Role::Group)
                                                    .aria_label(format!("Lokasi: {folder}"))
                                                    .child(folder.clone()),
                                            )
                                            .child(
                                                div()
                                                    .text_xs()
                                                    .text_color(cx.theme().muted_foreground)
                                                    .id(("recent-modified", index))
                                                    .role(gpui::Role::Group)
                                                    .aria_label(modified.clone())
                                                    .child(modified.clone()),
                                            ),
                                    ),
                            )
                            .accessibility_label(format!("Buka terakhir: {name}"))
                            .tooltip(format!("{full}\n{modified}"))
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
