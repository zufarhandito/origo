//! View-local outline navigation; source data is read-only and never executes actions.
use crate::*;
use gpui::component::{
    ActiveTheme, Disableable, Selectable,
    button::{Button, ButtonVariants},
    scroll::Scrollbar,
};
use pdf_application::outline::Outline;
use pdf_domain::{PageId, SourceFingerprint, SourceId};
use std::collections::HashSet;
#[derive(Default)]
pub(super) struct OutlineState {
    pub open: bool,
    cache: HashMap<SourceId, (SourceFingerprint, Result<Arc<Outline>, String>)>,
    loading: HashSet<SourceId>,
    collapsed: HashSet<(SourceId, usize)>,
    selected: Option<(SourceId, usize)>,
    list: Option<ListState>,
    count: usize,
}
#[derive(Clone)]
enum Row {
    Text(String),
    Node(
        SourceId,
        usize,
        pdf_application::outline::OutlineNode,
        usize,
        bool,
    ),
}
impl DocumentSession {
    pub(super) fn outline_header(&mut self, cx: &mut Context<Self>) -> gpui::AnyElement {
        let outline_open = self.outline.open && self.mode != ViewerMode::Organize;
        gpui::div()
            .flex()
            .items_center()
            .gap_1()
            .p_1()
            .child(
                Button::new("nav-thumbnails")
                    .ghost()
                    .icon(gpui::assets::IconName::LayoutGrid)
                    .accessibility_label("Thumbnail")
                    .tooltip("Thumbnail")
                    .w(px(32.))
                    .h(px(32.))
                    .selected(!outline_open)
                    .on_click(cx.listener(|s, _, _, cx| {
                        s.outline.open = false;
                        cx.notify();
                    })),
            )
            .child(
                Button::new("nav-outline")
                    .ghost()
                    .icon(gpui::assets::IconName::ListTree)
                    .accessibility_label("Daftar isi")
                    .tooltip("Daftar isi")
                    .w(px(32.))
                    .h(px(32.))
                    .selected(outline_open)
                    .on_click(cx.listener(|s, _, window, cx| {
                        if s.mode == ViewerMode::Organize {
                            s.set_mode(ViewerMode::Read, window, cx);
                        }
                        s.outline.open = true;
                        s.load_outlines(cx);
                        cx.notify();
                    })),
            )
            .into_any_element()
    }
    pub(super) fn load_outlines(&mut self, cx: &mut Context<Self>) {
        let Some(w) = self.workspace.as_ref() else {
            return;
        };
        let sources = w.sources().to_vec();
        drop(w);
        let ids = sources.iter().map(|s| s.id).collect::<HashSet<_>>();
        self.outline.cache.retain(|id, _| ids.contains(id));
        for source in sources {
            if self
                .outline
                .cache
                .get(&source.id)
                .is_some_and(|(f, _)| *f == source.fingerprint)
                || self.outline.loading.contains(&source.id)
            {
                continue;
            }
            // Bound accumulated metadata across imported sources, not just each native read.
            let total = self
                .outline
                .cache
                .values()
                .filter_map(|(_, data)| data.as_ref().ok())
                .flat_map(|o| &o.nodes)
                .map(|n| n.title.len())
                .sum::<usize>();
            if total >= 2 * 1024 * 1024 {
                self.outline.cache.insert(
                    source.id,
                    (
                        source.fingerprint,
                        Err("Batas data daftar isi tercapai.".into()),
                    ),
                );
                continue;
            }
            self.outline.loading.insert(source.id);
            let renderer = self.renderer.clone();
            let id = source.id;
            let fingerprint = source.fingerprint;
            let instance = self.workspace_instance;
            let task = cx
                .background_executor()
                .spawn(async move { renderer.outline(&source.path, fingerprint) });
            cx.spawn(async move |this, cx| {
                let result = task.await.map(Arc::new).map_err(|e| e.to_string());
                let _ = this.update(cx, |s, cx| {
                    if s.workspace_instance != instance {
                        return;
                    }
                    s.outline.loading.remove(&id);
                    if s.workspace.as_ref().is_some_and(|w| {
                        w.resolve(id)
                            .is_some_and(|source| source.fingerprint == fingerprint)
                    }) {
                        let old = s
                            .outline
                            .cache
                            .iter()
                            .filter(|(source, _)| **source != id)
                            .filter_map(|(_, (_, data))| data.as_ref().ok())
                            .collect::<Vec<_>>();
                        let nodes = old.iter().map(|o| o.nodes.len()).sum::<usize>();
                        let text = old
                            .iter()
                            .flat_map(|o| &o.nodes)
                            .map(|n| n.title.len())
                            .sum::<usize>();
                        let result = if result.as_ref().is_ok_and(|o| {
                            nodes + o.nodes.len() > 16384
                                || text + o.nodes.iter().map(|n| n.title.len()).sum::<usize>()
                                    > 2 * 1024 * 1024
                        }) {
                            Err("Batas data daftar isi tercapai.".into())
                        } else {
                            result
                        };
                        s.outline.collapsed.retain(|(source, _)| *source != id);
                        if let Ok(outline) = &result {
                            s.outline.collapsed.extend(
                                outline
                                    .nodes
                                    .iter()
                                    .filter_map(|n| n.parent)
                                    .map(|parent| (id, parent)),
                            );
                        }
                        s.outline.cache.insert(id, (fingerprint, result));
                        s.report(cx);
                        cx.notify();
                    }
                });
            })
            .detach();
        }
    }
    pub(super) fn outline_sidebar(&mut self, cx: &mut Context<Self>) -> gpui::AnyElement {
        self.load_outlines(cx);
        let mut rows = Vec::new();
        if let Some(w) = self.workspace.as_ref() {
            let multiple = w.sources().len() > 1;
            for source in w.sources() {
                if multiple {
                    rows.push(Row::Text(
                        source
                            .path
                            .file_name()
                            .unwrap_or_default()
                            .to_string_lossy()
                            .chars()
                            .take(128)
                            .collect(),
                    ));
                }
                match self.outline.cache.get(&source.id).map(|(_, data)| data) {
                    None => rows.push(Row::Text("Membaca daftar isi...".into())),
                    Some(Err(error)) => {
                        rows.push(Row::Text(format!("Daftar isi tidak dapat dibaca: {error}")))
                    }
                    Some(Ok(outline)) => {
                        if outline.nodes.is_empty() {
                            rows.push(Row::Text(
                                "PDF ini tidak memiliki daftar isi navigasi.".into(),
                            ));
                        }
                        let mut hidden = vec![false; outline.nodes.len()];
                        let mut depths = vec![0usize; outline.nodes.len()];
                        let parents = outline
                            .nodes
                            .iter()
                            .filter_map(|n| n.parent)
                            .collect::<HashSet<_>>();
                        for (index, node) in outline.nodes.iter().enumerate() {
                            if let Some(parent) = node.parent.filter(|p| *p < index) {
                                hidden[index] = hidden[parent]
                                    || self.outline.collapsed.contains(&(source.id, parent));
                                depths[index] = depths[parent] + 1;
                            }
                            if !hidden[index] {
                                rows.push(Row::Node(
                                    source.id,
                                    index,
                                    node.clone(),
                                    depths[index],
                                    parents.contains(&index),
                                ));
                            }
                            if rows.len() >= 16384 {
                                rows.push(Row::Text("Tampilan daftar isi dibatasi.".into()));
                                break;
                            }
                        }
                        if outline.limited {
                            rows.push(Row::Text("Sebagian daftar isi dibatasi atau rusak.".into()));
                        }
                    }
                }
                if rows.len() >= 16384 {
                    break;
                }
            }
        }
        let count = rows.len();
        let list = self
            .outline
            .list
            .get_or_insert_with(|| ListState::new(count, ListAlignment::Top, px(40.)))
            .clone();
        if count != self.outline.count {
            list.reset(count);
            self.outline.count = count;
        }
        let rows = Arc::new(rows);
        let weak = cx.weak_entity();
        gpui::div()
            .id("document-outline")
            .role(gpui::Role::Group)
            .aria_label("Daftar isi PDF")
            .w(px(256.))
            .flex_shrink_0()
            .min_h_0()
            .flex()
            .flex_col()
            .border_r_1()
            .border_color(cx.theme().border)
            .child(self.outline_header(cx))
            .child(
                gpui::div()
                    .flex_1()
                    .min_h_0()
                    .relative()
                    .child(
                        gpui::list(list.clone(), move |index, _, cx| {
                            let row = rows.get(index).cloned();
                            weak.update(cx, |s, cx| s.outline_row(row, cx))
                                .unwrap_or_else(|_| gpui::div().into_any_element())
                        })
                        .size_full(),
                    )
                    .child(Scrollbar::vertical(&list)),
            )
            .into_any_element()
    }
    fn outline_row(&mut self, row: Option<Row>, cx: &mut Context<Self>) -> gpui::AnyElement {
        match row {
            Some(Row::Text(text)) => gpui::div().p_2().text_sm().child(text).into_any_element(),
            Some(Row::Node(source, index, node, depth, children)) => {
                let collapsed = self.outline.collapsed.contains(&(source, index));
                let available = node.page.is_some_and(|serial| {
                    self.workspace.as_ref().is_some_and(|w| {
                        w.pages().iter().any(|p| {
                            p.id == PageId {
                                source,
                                serial: u32::from(serial),
                            }
                        })
                    })
                });
                let reason = if node.blocked {
                    "Aksi eksternal tidak didukung"
                } else if !available {
                    "Halaman tujuan tidak tersedia"
                } else {
                    &node.title
                };
                let label = node.title.clone();
                gpui::div()
                    .w_full()
                    .min_w_0()
                    .flex()
                    .items_start()
                    .pl(px((depth.min(8) * 12) as f32))
                    .when(children, |row| {
                        row.child(
                            Button::new(gpui::SharedString::from(format!(
                                "outline-expand-{}-{index}",
                                source.0
                            )))
                            .ghost()
                            .icon(if collapsed {
                                gpui::assets::IconName::ChevronRight
                            } else {
                                gpui::assets::IconName::ChevronDown
                            })
                            .accessibility_label(format!(
                                "{}: {}",
                                if collapsed { "Buka bab" } else { "Tutup bab" },
                                node.title
                            ))
                            .w(px(32.))
                            .h(px(32.))
                            .flex_shrink_0()
                            .on_click(cx.listener(
                                move |s, _, _, cx| {
                                    if !s.outline.collapsed.remove(&(source, index)) {
                                        s.outline.collapsed.insert((source, index));
                                    }
                                    cx.notify();
                                },
                            )),
                        )
                    })
                    .when(!children, |row| {
                        row.child(gpui::div().w(px(32.)).h(px(32.)).flex_shrink_0())
                    })
                    .child(
                        Button::new(gpui::SharedString::from(format!(
                            "outline-node-{}-{index}",
                            source.0
                        )))
                        .ghost()
                        .child(
                            gpui::div()
                                .w_full()
                                .min_w_0()
                                .whitespace_normal()
                                .text_left()
                                .line_height(gpui::relative(1.4))
                                .child(label.clone()),
                        )
                        .h_auto()
                        .min_h(px(32.))
                        .py_2()
                        .tooltip(reason.to_string())
                        .accessibility_label(label)
                        .selected(self.outline.selected == Some((source, index)))
                        .disabled(!available)
                        .flex_1()
                        .min_w_0()
                        .on_click(cx.listener(
                            move |s, _, window, cx| {
                                s.outline.selected = Some((source, index));
                                s.outline_go(source, node.page, node.point, cx);
                                s.focus.focus(window, cx);
                            },
                        )),
                    )
                    .into_any_element()
            }
            None => gpui::div().into_any_element(),
        }
    }
    fn outline_go(
        &mut self,
        source: SourceId,
        page: Option<u16>,
        point: Option<[f32; 2]>,
        cx: &mut Context<Self>,
    ) {
        let Some(serial) = page else {
            return;
        };
        let id = PageId {
            source,
            serial: u32::from(serial),
        };
        let target = self.workspace.as_ref().and_then(|w| {
            w.pages().iter().position(|p| p.id == id).and_then(|i| {
                w.geometry(i)
                    .ok()
                    .map(|g| (i, w.pages()[i].rotation_delta, g))
            })
        });
        let Some((index, delta, geometry)) = target else {
            return;
        };
        self.go_to(index as u16, cx);
        if let Some(point) = point {
            let point = pdf_domain::annotation::rotate(point, delta);
            let (width, height) = self.zoom.size(geometry, self.viewport);
            self.page_list.scroll_to(gpui::ListOffset {
                item_ix: index,
                offset_in_item: -px(24. + height * point[1]),
            });
            self.horizontal.set_offset(gpui::point(
                px((-width * point[0]).max(-(self.content_width() - self.viewport.0).max(0.))),
                px(0.),
            ));
        }
        self.report(cx);
        cx.notify();
    }
}
