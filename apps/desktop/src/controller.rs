use crate::*;
use gpui_kit::{ListOffset, PathPromptOptions};
use pdf_application::{
    PageRenderer, RenderError, RenderRequest, SourceId, open_document,
    viewer::{RenderKind, RenderTicket},
};
use std::sync::atomic::Ordering;

impl Viewer {
    pub(super) fn release_images(&self, images: Vec<Arc<RenderImage>>, cx: &mut Context<Self>) {
        if images.is_empty() {
            return;
        }
        let handle = self.window_handle;
        let releases = self.gpu_releases.clone();
        let errors = self.gpu_release_errors.clone();
        let count = images.len() as u64;
        // Defer until the borrowed window has returned to App.windows, then pass
        // that window explicitly. This also runs when the window is minimized.
        cx.defer(move |cx| {
            let result = handle.update(cx, |_, window, cx| {
                for image in images {
                    cx.drop_image(image, Some(window));
                }
            });
            if result.is_ok() {
                releases.fetch_add(count, Ordering::SeqCst);
            } else {
                errors.fetch_add(count, Ordering::SeqCst);
            }
        });
    }
    pub(super) fn clear_bitmaps(&mut self, cx: &mut Context<Self>) {
        let gpu_images: Vec<_> = self
            .gpu_images
            .drain()
            .map(|(_, (image, _))| image)
            .collect();
        let images = self
            .pages
            .clear()
            .into_iter()
            .chain(self.thumbnails.clear())
            .chain(gpu_images)
            .collect();
        self.release_images(images, cx);
    }
    pub(super) fn trim_gpu(&mut self, cx: &mut Context<Self>) {
        let keys: Vec<_> = self
            .gpu_images
            .iter()
            .filter_map(|(key, (_, index))| {
                let visible = match key.kind {
                    RenderKind::Page => &self.visible,
                    RenderKind::Thumbnail => &self.thumbnail_visible,
                };
                let nearby = visible.start.saturating_sub(1)..visible.end.saturating_add(1);
                let current = self.key(*index, key.kind);
                let matches = current.as_ref().is_some_and(|full| {
                    full == key || (full.preview() == *key && !self.pages.contains(full))
                });
                (!nearby.contains(index) || !matches).then(|| key.clone())
            })
            .collect();
        let images = keys
            .into_iter()
            .filter_map(|key| self.gpu_images.remove(&key).map(|(image, _)| image))
            .collect();
        self.release_images(images, cx);
    }
    pub(super) fn invalidate_work(&mut self) {
        self.generation += 1;
        self.queue.reset(self.generation);
        self.failures.clear();
    }
    pub(super) fn load(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        if self.opening {
            return;
        }
        self.mode = ViewerMode::Read;
        self.page_dragging = false;
        self.gesture_zoom.reset();
        self.text = text_selection::TextState::default();
        self.invalidate_work();
        self.source_sequence += 1;
        self.page_anchor = None;
        let id = SourceId(self.source_sequence);
        let generation = self.generation;
        self.opening = true;
        self.workspace = None;
        self.navigation = Navigation::default();
        self.clear_bitmaps(cx);
        self.stats = RenderStats::default();
        self.page_list.reset(0);
        self.thumbnail_list.reset(0);
        self.error = None;
        self.status = "Memeriksa dokumen…".into();
        let renderer = self.renderer.clone();
        let task = cx
            .background_executor()
            .spawn(async move { open_document(&renderer, &path, id) });
        cx.spawn(async move |this, cx| {
            let result = task.await;
            let _ = this.update(cx, |this, cx| {
                this.opening = false;
                if this.generation != generation {
                    return;
                }
                match result {
                    Ok(workspace) => {
                        this.navigation.count = workspace.pages().len() as u16;
                        this.workspace = Some(workspace);
                        this.visible = 0..1;
                        this.reset_layout();
                        this.schedule_current(cx);
                    }
                    Err(error) => {
                        this.error = Some(error.to_string());
                        this.status = error.to_string();
                    }
                }
                this.report(cx);
                cx.notify();
            });
        })
        .detach();
        self.report(cx);
        cx.notify();
    }

    pub(super) fn reset_layout(&mut self) {
        let count = usize::from(self.navigation.count);
        let height = self
            .workspace
            .as_ref()
            .and_then(|w| w.geometry(0).ok())
            .map(|g| self.zoom.size(g, self.viewport).1 + 48.)
            .unwrap_or(800.);
        self.page_list.reset_with_uniform_height(count, px(height));
        self.page_list.scroll_to(ListOffset {
            item_ix: usize::from(self.navigation.current),
            offset_in_item: px(0.),
        });
        self.thumbnail_list
            .reset_with_uniform_height(count, px(228.));
        self.thumbnail_list
            .scroll_to_reveal_item(usize::from(self.navigation.current));
        self.horizontal.set_offset(gpui::point(px(0.), px(0.)));
    }

    pub(super) fn go_to(&mut self, index: u16, cx: &mut Context<Self>) {
        if index >= self.navigation.count {
            return;
        }
        self.navigation.current = index;
        self.visible = usize::from(index)..usize::from(index) + 1;
        self.trim_gpu(cx);
        self.invalidate_work();
        self.page_list.scroll_to(ListOffset {
            item_ix: usize::from(index),
            offset_in_item: px(0.),
        });
        self.thumbnail_list
            .scroll_to_reveal_item(usize::from(index));
        self.schedule_current(cx);
        self.report(cx);
        cx.notify();
    }
    pub(super) fn navigate(&mut self, direction: Direction, cx: &mut Context<Self>) {
        self.stats.navigation_commands += 1;
        if self.navigation.count > 0 {
            match direction {
                Direction::First => {
                    self.go_to(0, cx);
                    return;
                }
                Direction::Last => {
                    self.go_to(self.navigation.count - 1, cx);
                    return;
                }
                _ => {}
            }
        }
        if let Some(index) = self.navigation.target(direction) {
            self.go_to(index, cx);
        }
    }
    pub(super) fn jump_to(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let value = self.jump.read(cx).value();
        match value.trim().parse::<u16>() {
            Ok(page) if page >= 1 && page <= self.navigation.count => {
                self.go_to(page - 1, cx);
                self.focus.focus(window, cx);
            }
            _ => {
                self.status = format!("Masukkan nomor halaman 1–{}.", self.navigation.count);
                cx.notify();
            }
        }
    }
    pub(super) fn set_zoom(&mut self, zoom: Zoom, cx: &mut Context<Self>) {
        self.gesture_zoom.reset();
        if self.opening {
            self.zoom = zoom;
            return;
        }
        if self.zoom == zoom {
            return;
        }
        self.zoom = zoom;
        self.trim_gpu(cx);
        self.invalidate_work();
        self.reset_layout();
        self.schedule_current(cx);
        self.report(cx);
        cx.notify();
    }
    pub(super) fn zoom_step(&mut self, step: i16, cx: &mut Context<Self>) {
        let percent = self.zoom_percent() as i16;
        self.set_zoom(Zoom::Percent((percent + step).clamp(50, 400) as u16), cx);
    }
    pub(super) fn gesture_zoom(
        &mut self,
        delta: f32,
        position: gpui::Point<gpui::Pixels>,
        phase: gpui::TouchPhase,
        cx: &mut Context<Self>,
    ) {
        if phase == gpui::TouchPhase::Started {
            self.gesture_zoom.reset();
        }
        if phase == gpui::TouchPhase::Cancelled || self.opening {
            self.gesture_zoom.reset();
            return;
        }
        let top = self.page_list.logical_scroll_top();
        let Some(geometry) = self
            .workspace
            .as_ref()
            .and_then(|w| w.geometry(top.item_ix).ok())
        else {
            return;
        };
        let old_scale = self.zoom.scale(geometry, self.viewport);
        let Some(percent) = self.gesture_zoom.update(old_scale * 100., delta) else {
            if phase == gpui::TouchPhase::Ended {
                self.gesture_zoom.reset();
            }
            return;
        };
        let zoom = Zoom::Percent(percent);
        if zoom != self.zoom {
            let accumulator = std::mem::take(&mut self.gesture_zoom);
            let old_size = self.zoom.size(geometry, self.viewport);
            let new_size = zoom.size(geometry, self.viewport);
            let ratio = new_size.1 / old_size.1;
            let bounds = self.page_list.bounds_for_item(top.item_ix);
            let anchor_y = bounds
                .map(|b| f32::from(position.y - b.origin.y))
                .unwrap_or(32.);
            let offset_y = pdf_application::viewer::zoom_anchor_offset(
                f32::from(top.offset_in_item),
                anchor_y,
                old_size.1,
                ratio,
            );
            let horizontal = self.horizontal.bounds();
            let old_content = self.content_width();
            let old_left = f32::from(horizontal.origin.x + self.horizontal.offset().x)
                + (old_content - old_size.0) / 2.;
            let anchor_x = (f32::from(position.x) - old_left).clamp(0., old_size.0);
            self.navigation.current = top.item_ix as u16;
            self.set_zoom(zoom, cx);
            self.gesture_zoom = accumulator;
            self.page_list.scroll_to(ListOffset {
                item_ix: top.item_ix,
                offset_in_item: px(offset_y),
            });
            let content = self.content_width();
            let offset_x = f32::from(position.x - horizontal.origin.x)
                - (content - new_size.0) / 2.
                - anchor_x * ratio;
            self.horizontal.set_offset(gpui::point(
                px(offset_x.clamp(-(content - self.viewport.0).max(0.), 0.)),
                px(0.),
            ));
        }
        if phase == gpui::TouchPhase::Ended {
            self.gesture_zoom.reset();
        }
    }
    pub(super) fn content_width(&self) -> f32 {
        self.workspace
            .as_ref()
            .map(|workspace| {
                (0..workspace.pages().len())
                    .filter_map(|i| workspace.geometry(i).ok())
                    .map(|g| self.zoom.size(g, self.viewport).0 + 64.)
                    .fold(self.viewport.0, f32::max)
            })
            .unwrap_or(self.viewport.0)
    }
    pub(super) fn zoom_percent(&self) -> u16 {
        self.workspace
            .as_ref()
            .and_then(|w| w.geometry(usize::from(self.navigation.current)).ok())
            .map(|g| (self.zoom.scale(g, self.viewport) * 100.).round() as u16)
            .unwrap_or(100)
    }
    pub(super) fn viewport_changed(
        &mut self,
        viewport: (f32, f32),
        dpr: f32,
        cx: &mut Context<Self>,
    ) {
        if (viewport.0 - self.viewport.0).abs() < 2.
            && (viewport.1 - self.viewport.1).abs() < 2.
            && (dpr - self.dpr).abs() < 0.01
        {
            return;
        }
        self.viewport = viewport;
        self.dpr = dpr;
        if self.opening {
            return;
        }
        self.trim_gpu(cx);
        self.invalidate_work();
        self.reset_layout();
        self.schedule_current(cx);
        cx.notify();
    }
    pub(super) fn scrolled(&mut self, range: Range<usize>, cx: &mut Context<Self>) {
        if self.opening || self.navigation.count == 0 || range.is_empty() || range == self.visible {
            return;
        }
        self.visible = range.clone();
        self.trim_gpu(cx);
        self.navigation.current = range.start.min(usize::from(self.navigation.count) - 1) as u16;
        self.thumbnail_list
            .scroll_to_reveal_item(usize::from(self.navigation.current));
        self.invalidate_work();
        for index in range {
            self.ensure_render(index, RenderKind::Page, 0, cx);
        }
        self.schedule_current(cx);
        self.report(cx);
        cx.notify();
    }
    pub(super) fn key(&self, index: usize, kind: RenderKind) -> Option<RenderKey> {
        let workspace = self.workspace.as_ref()?;
        let geometry = workspace.geometry(index).ok()?;
        let width = if kind == RenderKind::Page {
            self.zoom.size(geometry, self.viewport).0
        } else {
            128.
        };
        Some(RenderKey {
            source: workspace.pages().get(index)?.source_id,
            fingerprint: workspace
                .resolve(workspace.pages().get(index)?.source_id)?
                .fingerprint,
            index: workspace.pages().get(index)?.source_page_index,
            rotation: geometry.quarter_turns,
            background_rgba: 0xffffffff,
            width: RenderKey::pixel_width(width, self.dpr),
            device_scale_milli: (self.dpr * 1000.) as u32,
            kind,
        })
    }
    pub(super) fn bitmap(&mut self, key: &RenderKey) -> Option<Arc<RenderImage>> {
        match key.kind {
            RenderKind::Page => self.pages.get(key),
            RenderKind::Thumbnail => self.thumbnails.get(key),
        }
    }
    pub(super) fn schedule_current(&mut self, cx: &mut Context<Self>) {
        let index = usize::from(self.navigation.current);
        self.ensure_render(index, RenderKind::Page, 0, cx);
        if index > 0 {
            self.ensure_render(index - 1, RenderKind::Page, 2, cx);
        }
        self.ensure_render(index + 1, RenderKind::Page, 2, cx);
        self.update_status();
    }
    pub(super) fn ensure_render(
        &mut self,
        index: usize,
        kind: RenderKind,
        priority: u8,
        cx: &mut Context<Self>,
    ) {
        let Some(key) = self.key(index, kind) else {
            return;
        };
        let cached = match kind {
            RenderKind::Page => self.pages.contains(&key),
            RenderKind::Thumbnail => self.thumbnails.contains(&key),
        };
        if cached
            || self.failures.contains_key(&key)
            || self
                .inflight
                .as_ref()
                .is_some_and(|job| job.key == key && job.generation == self.generation)
        {
            return;
        }
        if kind == RenderKind::Page && priority == 0 {
            let preview = key.preview();
            if preview != key
                && !self.pages.contains(&preview)
                && !self.failures.contains_key(&preview)
                && !self
                    .inflight
                    .as_ref()
                    .is_some_and(|job| job.key == preview && job.generation == self.generation)
            {
                self.queue.push(RenderTicket {
                    generation: self.generation,
                    key: preview,
                    priority: 0,
                });
            }
        }
        self.queue.push(RenderTicket {
            generation: self.generation,
            key,
            priority: if kind == RenderKind::Thumbnail {
                3
            } else {
                priority.max(1)
            },
        });
        self.pump(cx);
    }
    pub(super) fn pump(&mut self, cx: &mut Context<Self>) {
        if self.inflight.is_some() || self.opening {
            return;
        }
        let Some(job) = self.queue.pop() else {
            self.update_status();
            return;
        };
        let Some(workspace) = self.workspace.as_ref() else {
            return;
        };
        let Some(source) = workspace.resolve(job.key.source) else {
            return;
        };
        let request = RenderRequest {
            path: source.path.clone(),
            page_index: job.key.index,
            target_width: job.key.width,
            rotation_delta: (job.key.rotation + 4
                - source.geometries[usize::from(job.key.index)].quarter_turns)
                % 4,
            fingerprint: Some(job.key.fingerprint),
        };
        self.inflight = Some(job.clone());
        match job.key.kind {
            RenderKind::Page => self.stats.full += 1,
            RenderKind::Thumbnail => self.stats.thumbnails += 1,
        }
        let renderer = self.renderer.clone();
        let task = cx.background_executor().spawn(async move {
            let mut page = renderer.render_page(&request)?;
            let bytes = page.rgba.len();
            for pixel in page.rgba.as_chunks_mut::<4>().0 {
                pixel.swap(0, 2);
            }
            let pixels = image::RgbaImage::from_raw(page.width, page.height, page.rgba)
                .ok_or(RenderError::InvalidPdf)?;
            let frames: smallvec::SmallVec<[image::Frame; 1]> =
                smallvec::smallvec![image::Frame::new(pixels)];
            Ok::<_, RenderError>((Arc::new(RenderImage::new(frames)), bytes))
        });
        cx.spawn(async move |this, cx| {
            let result = task.await;
            let _ = this.update(cx, |this, cx| {
                this.inflight = None;
                if job.generation != this.generation {
                    this.stats.stale += 1;
                } else {
                    match result {
                        Ok((image, bytes)) => {
                            this.stats.max_bitmap_bytes = this.stats.max_bitmap_bytes.max(bytes);
                            let inserted = match job.key.kind {
                                RenderKind::Page => this.pages.insert(job.key, image, bytes),
                                RenderKind::Thumbnail => {
                                    this.thumbnails.insert(job.key, image, bytes)
                                }
                            };
                            this.release_images(inserted.evicted, cx);
                            this.stats.peak_cache_bytes = this
                                .stats
                                .peak_cache_bytes
                                .max(this.pages.bytes() + this.thumbnails.bytes());
                        }
                        Err(error) => {
                            this.failures.insert(job.key, error.to_string());
                            if matches!(error, RenderError::SourceChanged) {
                                this.text = text_selection::TextState::default();
                                this.invalidate_work();
                                this.clear_bitmaps(cx);
                                this.workspace = None;
                                this.navigation = Navigation::default();
                                this.error = Some(error.to_string());
                                this.status = error.to_string();
                            }
                        }
                    }
                }
                this.trim_gpu(cx);
                this.pump(cx);
                this.update_status();
                this.report(cx);
                cx.notify();
            });
        })
        .detach();
    }
    pub(super) fn update_status(&mut self) {
        if self.workspace.is_none() {
            return;
        }
        if self.navigation.count == 0 {
            self.status = "Tidak ada halaman - impor PDF atau Undo untuk memulihkan.".into();
            return;
        }
        let page = self.navigation.current + 1;
        let count = self.navigation.count;
        self.status =
            if let Some(key) = self.key(usize::from(self.navigation.current), RenderKind::Page) {
                if let Some(error) = self.failures.get(&key) {
                    error.clone()
                } else if self.pages.contains(&key) {
                    format!("Halaman {page} dari {count}")
                } else {
                    format!("Memuat halaman {page} dari {count}…")
                }
            } else {
                "Siap".into()
            };
    }
    pub(super) fn report(&self, cx: &mut Context<Self>) {
        self.defer_page_input(cx);
        let Some(path) = self.metrics.clone() else {
            return;
        };
        let sequence = self.metrics_sequence.clone();
        let number = sequence.fetch_add(1, Ordering::SeqCst) + 1;
        let lock = self.metrics_lock.clone();
        let mut json = format!(
            "{{\"pageCount\":{},\"activePage\":{},\"zoomPercent\":{},\"fullRenders\":{},\"thumbnailRenders\":{},\"staleResults\":{},\"queue\":{},\"rendering\":{},\"opening\":{},\"pageCacheBytes\":{},\"thumbnailCacheBytes\":{},\"peakCacheBytes\":{},\"maxBitmapBytes\":{},\"generation\":{},\"gpuReleases\":{},\"gpuReleaseErrors\":{},\"documentOpens\":{},\"nativeRenders\":{},\"textCacheBytes\":{},\"textLoads\":{},\"textLoading\":{},\"selectedChars\":{}}}",
            self.navigation.count,
            self.navigation.current + 1,
            self.zoom_percent(),
            self.stats.full,
            self.stats.thumbnails,
            self.stats.stale,
            self.queue.len(),
            self.inflight.is_some(),
            self.opening,
            self.pages.bytes(),
            self.thumbnails.bytes(),
            self.stats.peak_cache_bytes,
            self.stats.max_bitmap_bytes,
            self.generation,
            self.gpu_releases.load(Ordering::SeqCst),
            self.gpu_release_errors.load(Ordering::SeqCst),
            self.renderer.session_stats().document_opens,
            self.renderer.session_stats().renders,
            self.text.pages.bytes(),
            self.text.loads,
            self.text.busy,
            self.text
                .selection
                .filter(|s| s.anchor.page == s.focus.page)
                .map(|s| s.anchor.character.abs_diff(s.focus.character) + 1)
                .unwrap_or(0)
        );
        if let Some(w) = &self.workspace {
            json.pop();
            let plan = w
                .pages()
                .iter()
                .map(|p| {
                    format!(
                        "[{}, {}, {}]",
                        p.source_id.0, p.source_page_index, p.rotation_delta
                    )
                })
                .collect::<Vec<_>>()
                .join(",");
            json.push_str(&format!(",\"sourceCount\":{},\"selectedPages\":{},\"dirty\":{},\"revision\":{},\"pagePlan\":[{}]}}",w.sources().len(),w.selected().len(),w.is_dirty(),w.revision(),plan));
        }
        json.pop();
        json.push_str(&format!(
            ",\"organizing\":{},\"pageDragging\":{}}}",
            self.mode == ViewerMode::Organize,
            self.page_dragging
        ));
        json.pop();
        json.push_str(&format!(
            ",\"navCommands\":{}}}",
            self.stats.navigation_commands
        ));
        cx.background_executor()
            .spawn(async move {
                if let Ok(_guard) = lock.lock()
                    && sequence.load(Ordering::SeqCst) == number
                {
                    let _ = std::fs::write(path, json);
                }
            })
            .detach();
    }
    pub(super) fn open(&mut self, _: &Open, _: &mut Window, cx: &mut Context<Self>) {
        if self.opening {
            return;
        }
        let dialog = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some("Pilih PDF".into()),
        });
        cx.spawn(async move |this, cx| match dialog.await {
            Ok(Ok(Some(paths))) => {
                if let Some(path) = paths.into_iter().next() {
                    let _ = this.update(cx, |this, cx| this.load(path, cx));
                }
            }
            Ok(Ok(None)) => {}
            _ => {
                let _ = this.update(cx, |this, cx| {
                    this.status = "Dialog file gagal dibuka.".into();
                    cx.notify();
                });
            }
        })
        .detach();
    }
}
