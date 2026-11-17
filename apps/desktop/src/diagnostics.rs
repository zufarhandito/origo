//! Numeric diagnostics only. Snapshots run outside render callbacks and include
//! every document, even after the active tab is closed or an inactive job finishes.
use crate::*;
use serde_json::{Value, json};
use std::sync::atomic::Ordering;

impl DocumentSession {
    pub(super) fn report(&self, cx: &mut Context<Self>) {
        if self.active {
            self.defer_page_input(cx);
        }
        if self.metrics.is_none() {
            return;
        }
        let owner = self.owner.clone();
        cx.defer(move |cx| {
            let _ = owner.update(cx, |shell, cx| shell.report(cx));
        });
    }
    pub(super) fn diagnostics(&self) -> Value {
        let native = self.renderer.session_stats();
        let mut result = json!({
            "pageCount": self.navigation.count, "activePage": self.navigation.current + 1,
            "zoomPercent": self.zoom_percent(), "fullRenders": self.stats.full,
            "thumbnailRenders": self.stats.thumbnails, "staleResults": self.stats.stale,
            "queue": self.queue.len(), "rendering": self.inflight.is_some(), "opening": self.opening,
            "pageCacheBytes": self.pages.bytes(), "thumbnailCacheBytes": self.thumbnails.bytes(),
            "peakCacheBytes": self.stats.peak_cache_bytes, "maxBitmapBytes": self.stats.max_bitmap_bytes,
            "generation": self.generation, "gpuReleases": self.gpu_releases.load(Ordering::SeqCst),
            "deviceScaleMilli": (self.dpr * 1000.).round() as u32,
            "viewportWidth": self.viewport.0, "viewportHeight": self.viewport.1,
            "gpuReleaseErrors": self.gpu_release_errors.load(Ordering::SeqCst),
            "gpuImages": self.gpu_images.len(),
            "documentOpens": native.document_opens, "nativeRenders": native.renders,
            "textCacheBytes": self.text.pages.bytes(), "textLoads": self.text.loads,
            "textLoading": self.text.busy, "copyingText": self.text.copying(),
            "searchOpen": self.search.open, "searchRunning": self.search.running,
            "searchMatches": self.search.matches.len(), "searchCurrent": self.search.current.map_or(0, |i| i + 1),
            "searchScannedPages": self.search.scanned, "searchFailedPages": self.search.errors,
            "searchCaseSensitive": self.search.options.case_sensitive, "searchWholeWords": self.search.options.whole_words,
            "searchCapped": self.search.capped,
            "copyBufferBytes": self.text.copy_bytes(),
            "selectedChars": self.text.selection.filter(|s| s.anchor.page == s.focus.page)
                .map_or(0, |s| s.anchor.character.abs_diff(s.focus.character) + 1),
            "organizing": self.mode == ViewerMode::Organize, "pageDragging": self.page_dragging,
            "navCommands": self.stats.navigation_commands, "tabId": self.tab_id.0, "active": self.active, "visible": self.visible_for_render,
            "viewId": self.view_id.0, "documentActor": self.document_owner.is_none(),
            "conversionHubVisible": self.mode == ViewerMode::Convert,
            "exporting": self.export.running, "exportsCompleted": self.export.completed,
            "exportSuccess": self.export.success, "exportPages": self.export.pages,
            "transition": format!("{:?}", self.transition.phase), "workspaceInstance": self.workspace_instance,
            "splitFiles": self.export.split_total, "splitDone": self.export.split_done.load(Ordering::SeqCst),
            "optimizing": self.export.optimizing,
            "pngExport": self.export.png, "pngPages": self.export.png_total,
            "pngDone": self.export.png_done.load(Ordering::SeqCst),
            "optimizeBeforeBytes": self.export.optimization.map_or(0, |s| s.baseline_bytes),
            "optimizeAfterBytes": self.export.optimization.map_or(0, |s| s.output_bytes),
        });
        if let Some(w) = self.workspace.as_ref() {
            result["sourceCount"] = json!(w.sources().len());
            result["selectedPages"] = json!(w.selected().len());
            result["dirty"] = json!(w.is_dirty());
            result["revision"] = json!(w.revision());
            result["pagePlan"] = json!(
                w.pages()
                    .iter()
                    .map(|p| (p.source_id.0, p.source_page_index, p.rotation_delta))
                    .collect::<Vec<_>>()
            );
        }
        result
    }
}
