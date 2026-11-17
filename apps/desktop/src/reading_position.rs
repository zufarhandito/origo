//! Reading-only bookmark mapped back to the primary source, never an edit backup.
use crate::*;

#[derive(Clone, Debug, PartialEq)]
pub(super) struct ReadingPosition {
    pub page: u16,
    pub zoom: Zoom,
    pub offset: f32,
    pub pan: f32,
}
impl Default for ReadingPosition {
    fn default() -> Self {
        Self {
            page: 0,
            zoom: Zoom::FitWidth,
            offset: 0.,
            pan: 0.,
        }
    }
}
impl DocumentSession {
    pub(super) fn reading_position(&self) -> ReadingPosition {
        if let Some(position) = &self.pending_reading {
            return position.clone();
        }
        let Some(workspace) = &self.workspace else {
            return ReadingPosition::default();
        };
        let top = self.page_list.logical_scroll_top();
        let Some(page) = workspace.pages().get(top.item_ix) else {
            return ReadingPosition::default();
        };
        if page.source_id != workspace.source().id {
            return ReadingPosition {
                zoom: self.zoom,
                ..Default::default()
            };
        }
        let height = workspace
            .geometry(top.item_ix)
            .ok()
            .map(|g| self.zoom.size(g, self.viewport).1 + 48.)
            .unwrap_or(1.);
        ReadingPosition {
            page: page.source_page_index,
            zoom: self.zoom,
            offset: (f32::from(top.offset_in_item) / height).clamp(-1., 1.),
            pan: (f32::from(self.horizontal.offset().x)
                / (self.content_width() - self.viewport.0).max(1.))
            .clamp(-1., 0.),
        }
    }
    pub(super) fn queue_reading_position(
        &mut self,
        position: ReadingPosition,
        cx: &mut Context<Self>,
    ) {
        self.zoom = position.zoom;
        self.navigation.current = position.page.min(self.navigation.count.saturating_sub(1));
        self.pending_reading = Some(position);
        self.reset_layout();
        cx.notify();
    }
    pub(super) fn apply_pending_reading(&mut self, cx: &mut Context<Self>) {
        let Some(position) = self.pending_reading.take() else {
            return;
        };
        let index = position.page.min(self.navigation.count.saturating_sub(1));
        self.go_to(index, cx);
        if let Some(height) = self
            .workspace
            .as_ref()
            .and_then(|w| w.geometry(usize::from(index)).ok())
            .map(|g| self.zoom.size(g, self.viewport).1 + 48.)
        {
            self.page_list.scroll_to(gpui::ListOffset {
                item_ix: usize::from(index),
                offset_in_item: px(position.offset.clamp(-1., 1.) * height),
            });
            self.horizontal.set_offset(gpui::point(
                px(position.pan.clamp(-1., 0.) * (self.content_width() - self.viewport.0).max(0.)),
                px(0.),
            ));
        }
    }
}
