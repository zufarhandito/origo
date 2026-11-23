use super::*;
use pdf_ocr::{BatchPlan, Progress};

pub(super) struct Batch {
    tab: shell::TabId,
    marker: PageKey,
    total: usize,
    plan: Option<BatchPlan>,
    loading: bool,
    paused: bool,
    // A tab/window close preserves the resume marker. An explicit cancel clears it.
    stop: Option<bool>,
    updating_marker: bool,
    marker_pending: bool,
    samples: u32,
    spent: Duration,
    error: Option<String>,
}

impl Batch {
    fn progress(&self) -> Progress {
        self.plan.as_ref().map_or(
            Progress {
                total: self.total,
                ..Progress::default()
            },
            BatchPlan::progress,
        )
    }

    fn busy(&self) -> bool {
        self.loading
            || self.updating_marker
            || self.stop.is_some()
            || self
                .plan
                .as_ref()
                .is_some_and(|p| p.progress().remaining() > 0)
    }

    fn label(&self) -> String {
        if self.loading {
            return "Menyiapkan OCR dokumen…".into();
        }
        if self.stop.is_some() {
            return "Menghentikan OCR…".into();
        }
        if self.plan.is_none()
            && let Some(error) = &self.error
        {
            return error.clone();
        }
        let p = self.progress();
        let mut status = format!("{} / {} halaman siap dicari", p.ready, p.total);
        if p.empty > 0 {
            status.push_str(&format!(" · {} tanpa teks", p.empty));
        }
        if p.failed > 0 {
            status.push_str(&format!(" · {} gagal", p.failed));
        }
        if p.remaining() > 0 {
            status.push_str(&format!(" · {} tersisa", p.remaining()));
            if self.paused {
                status.push_str(" · Dijeda");
            } else if self.samples >= 5 {
                let seconds = (self.spent.as_secs_f64() / f64::from(self.samples)
                    * p.remaining() as f64)
                    .ceil() as u64;
                status.push_str(&format!(
                    " · kira-kira {} m {:02} dtk",
                    seconds / 60,
                    seconds % 60
                ));
            }
        }
        if let Some(error) = &self.error {
            status.push_str(&format!(" · {error}"));
        }
        status
    }
}

impl OcrJobs {
    pub fn has_batch(&self) -> bool {
        self.batch.as_ref().is_some_and(Batch::busy)
    }

    pub fn batch_label(&self) -> Option<String> {
        self.batch.as_ref().map(Batch::label)
    }

    pub fn batch_diagnostics(&self) -> serde_json::Value {
        self.batch.as_ref().map_or(serde_json::Value::Null, |b| {
            let p = b.progress();
            serde_json::json!({
                "ready":p.ready, "empty":p.empty, "failed":p.failed, "total":p.total,
                "remaining":p.remaining(), "paused":b.paused, "loading":b.loading,
                "working":self.running.as_ref().is_some_and(|run| run.batch.is_some()),
                "samples":b.samples
            })
        })
    }

    pub fn batch_paused(&self) -> bool {
        self.batch.as_ref().is_some_and(|b| b.paused)
    }

    pub fn batch_can_toggle(&self) -> bool {
        self.batch.as_ref().is_some_and(|b| {
            !b.loading
                && !b.updating_marker
                && b.stop.is_none()
                && b.plan.is_some()
                && b.progress().remaining() > 0
        })
    }

    fn start_batch(
        &mut self,
        tab: shell::TabId,
        marker: PageKey,
        pages: Vec<DocumentPage>,
        cx: &mut Context<Self>,
    ) {
        if self.clearing || self.has_batch() {
            return;
        }
        self.batch = Some(Batch {
            tab,
            marker: marker.clone(),
            total: pages.len(),
            plan: None,
            loading: true,
            paused: false,
            stop: None,
            updating_marker: false,
            marker_pending: false,
            samples: 0,
            spent: Duration::ZERO,
            error: None,
        });
        let store = self.store.clone();
        let task = cx.background_executor().spawn(async move {
            let plan = BatchPlan::load(pages, &store)?;
            let pending = plan.progress().remaining() > 0 || plan.progress().failed > 0;
            store.set_checkpoint(&marker, pending)?;
            Ok::<_, pdf_ocr::DocumentError>((plan, pending))
        });
        cx.spawn(async move |this, cx| {
            let result = task.await;
            let _ = this.update(cx, |this, cx| {
                if let Some(batch) = &mut this.batch {
                    batch.loading = false;
                    match result {
                        Ok((plan, pending)) => {
                            batch.plan = Some(plan);
                            batch.marker_pending = pending;
                        }
                        Err(error) => batch.error = Some(error.to_string()),
                    }
                }
                this.pump(cx);
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    pub fn toggle_batch(&mut self, cx: &mut Context<Self>) {
        if self.batch_can_toggle()
            && let Some(batch) = &mut self.batch
        {
            batch.paused = !batch.paused;
            if !batch.paused {
                batch.error = None;
            }
            self.pump(cx);
            cx.notify();
        }
    }

    fn retry_batch(&mut self, cx: &mut Context<Self>) {
        if let Some(batch) = &mut self.batch
            && batch.stop.is_none()
            && !batch.updating_marker
            && let Some(plan) = &mut batch.plan
        {
            plan.retry_failed();
            batch.paused = false;
            batch.error = None;
            self.pump(cx);
            cx.notify();
        }
    }

    fn cancel_batch(&mut self, cx: &mut Context<Self>) {
        if let Some(batch) = &mut self.batch {
            batch.stop = Some(false);
        }
        if let Some(work) = &self.running
            && work.batch.is_some()
        {
            work.control.cancel();
        }
        self.pump(cx);
        cx.notify();
    }

    pub(super) fn stop_batch_tabs(
        &mut self,
        tabs: &[shell::TabId],
        cx: &mut Context<Self>,
    ) -> bool {
        if let Some(batch) = &mut self.batch
            && tabs.contains(&batch.tab)
        {
            batch.stop = Some(true);
            self.pump(cx);
        }
        self.batch
            .as_ref()
            .is_some_and(|batch| tabs.contains(&batch.tab))
    }

    pub(super) fn take_next(&mut self, cx: &mut Context<Self>) -> Option<(Job, Option<usize>)> {
        // Always settle stop/checkpoint transitions before dispatching more work.
        if let Some(batch) = &mut self.batch
            && !batch.loading
            && !batch.updating_marker
        {
            if let Some(keep_checkpoint) = batch.stop {
                if keep_checkpoint || !batch.marker_pending {
                    self.batch = None;
                } else {
                    self.update_marker(false, cx);
                }
            } else if let Some(plan) = &batch.plan {
                let p = plan.progress();
                if p.remaining() == 0 && p.failed == 0 && batch.marker_pending {
                    self.update_marker(false, cx);
                }
            }
        }
        if let Some(job) = self.queue.pop_front() {
            let batch_index = self
                .batch
                .as_mut()
                .filter(|b| b.tab == job.tab && b.stop.is_none() && !b.updating_marker)
                .and_then(|b| b.plan.as_mut())
                .and_then(|plan| plan.claim_priority(&job.key));
            return Some((job, batch_index));
        }
        let batch = self
            .batch
            .as_mut()
            .filter(|b| !b.paused && b.stop.is_none() && !b.updating_marker)?;
        let (index, page) = batch.plan.as_mut()?.claim_next()?;
        Some((
            Job {
                tab: batch.tab,
                key: page.key(),
                page,
                manual: false,
            },
            Some(index),
        ))
    }

    fn update_marker(&mut self, pending: bool, cx: &mut Context<Self>) {
        let Some(batch) = &mut self.batch else {
            return;
        };
        batch.updating_marker = true;
        let key = batch.marker.clone();
        let store = self.store.clone();
        let task = cx
            .background_executor()
            .spawn(async move { store.set_checkpoint(&key, pending) });
        cx.spawn(async move |this, cx| {
            let result = task.await;
            let _ = this.update(cx, |this, cx| {
                if let Some(batch) = &mut this.batch {
                    batch.updating_marker = false;
                    batch.marker_pending = pending;
                    if let Err(e) = result {
                        cx.emit(Change::Notice(e.to_string()));
                    }
                }
                this.pump(cx);
                cx.notify();
            });
        })
        .detach();
    }

    pub(super) fn update_batch(
        &mut self,
        work: &Running,
        result: &Result<Cached, pdf_ocr::DocumentError>,
    ) {
        use pdf_ocr::PageStatus;
        let Some(id) = work.batch else {
            return;
        };
        let Some(batch) = &mut self.batch else {
            return;
        };
        let Some(plan) = &mut batch.plan else {
            return;
        };
        let state = match result {
            Ok(entry) => PageStatus::from(entry),
            Err(_) if work.control.checkpoint().is_err() => PageStatus::Pending,
            Err(error) if systemic(error) => {
                batch.error = Some(error.to_string());
                batch.paused = true;
                PageStatus::Pending
            }
            Err(_) => PageStatus::Failed,
        };
        plan.finish(id, state);
        if state != PageStatus::Pending {
            batch.samples += 1;
            batch.spent += work.started.elapsed();
        }
    }
}

pub(super) fn systemic(error: &pdf_ocr::DocumentError) -> bool {
    matches!(
        error,
        pdf_ocr::DocumentError::Cache(_)
            | pdf_ocr::DocumentError::Ocr(
                pdf_ocr::OcrError::EngineUnavailable | pdf_ocr::OcrError::ModelMissing(_)
            )
            | pdf_ocr::DocumentError::Render(
                pdf_application::RenderError::SourceChanged
                    | pdf_application::RenderError::DependencyUnavailable(_)
            )
    )
}

impl DocumentSession {
    fn ocr_batch_marker(&self) -> Option<PageKey> {
        let w = self.workspace.as_ref()?;
        Some(PageKey::new(
            &TextRequest {
                path: w.source().path.clone(),
                fingerprint: w.source().fingerprint,
                page_index: 0,
            },
            Language::default(),
        ))
    }

    pub(crate) fn check_ocr_resume(&mut self, cx: &mut Context<Self>) {
        self.ocr_resume = false;
        let Some(marker) = self.ocr_batch_marker() else {
            return;
        };
        let store = self.ocr.read(cx).store.clone();
        let task = cx
            .background_executor()
            .spawn(async move { store.has_checkpoint(&marker).unwrap_or(false) });
        let instance = self.workspace_instance;
        cx.spawn(async move |this, cx| {
            let pending = task.await;
            let _ = this.update(cx, |this, cx| {
                if this.workspace_instance == instance {
                    this.ocr_resume = pending;
                    cx.notify();
                }
            });
        })
        .detach();
    }

    pub(crate) fn start_ocr_batch(&mut self, cx: &mut Context<Self>) {
        if self.edits_locked() {
            return;
        }
        let Some(marker) = self.ocr_batch_marker() else {
            return;
        };
        let pages = (0..usize::from(self.navigation.count))
            .filter_map(|i| self.ocr_page(i))
            .collect();
        let tab = self.tab_id;
        self.ocr_resume = false;
        self.ocr
            .update(cx, |ocr, cx| ocr.start_batch(tab, marker, pages, cx));
    }

    pub(crate) fn ocr_resume_pending(&self) -> bool {
        self.ocr_resume
    }

    pub(crate) fn ocr_batch_controls(&self, cx: &mut Context<Self>) -> Option<gpui::AnyElement> {
        let jobs = self.ocr.read(cx);
        let batch = jobs.batch.as_ref().filter(|b| b.tab == self.tab_id)?;
        let p = batch.progress();
        let locked = self.edits_locked();
        let failed_pages = batch
            .plan
            .as_ref()
            .map_or_else(Vec::new, BatchPlan::failed_pages);
        let toggle = jobs.batch_can_toggle();
        let stop = batch.busy() && batch.stop.is_none();
        let label = jobs.batch_label()?;
        let paused = jobs.batch_paused();
        let busy = batch.updating_marker;
        Some(
            gpui::div()
                .id("ocr-batch-status")
                .role(gpui::Role::Group)
                .aria_label(label.clone())
                .flex()
                .flex_wrap()
                .items_center()
                .gap_2()
                .px_3()
                .py_1()
                .flex_shrink_0()
                .text_xs()
                .child(label)
                .when(toggle, |d| {
                    d.child(
                        Button::new("ocr-batch-toggle")
                            .outline()
                            .xsmall()
                            .label(if paused { "Lanjutkan OCR" } else { "Jeda OCR" })
                            .disabled(locked)
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.ocr.update(cx, |ocr, cx| ocr.toggle_batch(cx))
                            })),
                    )
                })
                .when(stop, |d| {
                    d.child(
                        Button::new("ocr-batch-cancel")
                            .outline()
                            .xsmall()
                            .label("Batalkan batch OCR")
                            .disabled(locked)
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.ocr.update(cx, |ocr, cx| ocr.cancel_batch(cx))
                            })),
                    )
                })
                .when(p.failed > 0, |d| {
                    d.child(
                        Button::new("ocr-batch-retry")
                            .outline()
                            .xsmall()
                            .label("Coba ulang halaman gagal")
                            .tooltip(format!(
                                "Halaman {}",
                                failed_pages
                                    .iter()
                                    .map(usize::to_string)
                                    .collect::<Vec<_>>()
                                    .join(", ")
                            ))
                            .disabled(locked || busy)
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.ocr.update(cx, |ocr, cx| ocr.retry_batch(cx))
                            })),
                    )
                })
                .into_any_element(),
        )
    }
}
