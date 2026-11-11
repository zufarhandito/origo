//! Pure layout, scheduling and cache policies, independently testable without GPUI.
use crate::{PageGeometry, SourceFingerprint, SourceId};
use std::{collections::HashMap, hash::Hash};

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum Zoom {
    Percent(u16),
    #[default]
    FitWidth,
    FitPage,
}
/// Preserve sub-percent gesture deltas; rounding every event loses slow pinches.
#[derive(Default)]
pub struct GestureZoom(Option<f32>);
impl GestureZoom {
    pub fn reset(&mut self) {
        self.0 = None;
    }
    pub fn update(&mut self, current: f32, delta: f32) -> Option<u16> {
        if !delta.is_finite() || delta <= -1. || delta == 0. || !current.is_finite() {
            return None;
        }
        let percent = (self.0.unwrap_or(current) * (1. + delta)).clamp(50., 400.);
        self.0 = Some(percent);
        Some(percent.round() as u16)
    }
}

/// Zoom around a point inside the page, excluding the 24px label and 8px gap.
pub fn zoom_anchor_offset(old_offset: f32, point_in_item: f32, old_height: f32, ratio: f32) -> f32 {
    (old_offset + (point_in_item - 32.).clamp(0., old_height) * (ratio - 1.)).max(0.)
}
impl Zoom {
    pub fn scale(self, page: PageGeometry, viewport: (f32, f32)) -> f32 {
        let width = page.width_points * 96. / 72.;
        let height = page.height_points * 96. / 72.;
        match self {
            Self::Percent(n) => f32::from(n.clamp(50, 400)) / 100.,
            Self::FitWidth => ((viewport.0 - 64.).max(32.) / width).clamp(0.01, 4.),
            Self::FitPage => (((viewport.0 - 64.).max(32.) / width)
                .min((viewport.1 - 64.).max(32.) / height))
            .clamp(0.01, 4.),
        }
    }
    pub fn size(self, page: PageGeometry, viewport: (f32, f32)) -> (f32, f32) {
        let scale = self.scale(page, viewport) * 96. / 72.;
        (page.width_points * scale, page.height_points * scale)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum RenderKind {
    Page,
    Thumbnail,
}
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct RenderKey {
    pub source: SourceId,
    pub fingerprint: SourceFingerprint,
    pub index: u16,
    pub rotation: u8,
    pub background_rgba: u32,
    pub width: u32,
    pub device_scale_milli: u32,
    pub kind: RenderKind,
}
impl RenderKey {
    pub fn preview(&self) -> Self {
        let mut key = self.clone();
        if key.kind == RenderKind::Page && key.width > 1024 {
            key.width = 512;
        }
        key
    }
    pub fn pixel_width(display_width: f32, dpr: f32) -> u32 {
        let pixels = (display_width * dpr.clamp(0.5, 4.))
            .ceil()
            .clamp(32., 8192.) as u32;
        pixels.div_ceil(64) * 64
    }
}

pub struct CacheInsert<V> {
    pub retained: bool,
    pub evicted: Vec<V>,
}
pub struct ByteCache<K, V> {
    entries: HashMap<K, (V, usize, u64)>,
    budget: usize,
    used: usize,
    clock: u64,
}
impl<K: Eq + Hash + Clone, V: Clone> ByteCache<K, V> {
    pub fn new(budget: usize) -> Self {
        Self {
            entries: HashMap::new(),
            budget,
            used: 0,
            clock: 0,
        }
    }
    pub fn get(&mut self, key: &K) -> Option<V> {
        self.clock = self.clock.wrapping_add(1);
        self.entries.get_mut(key).map(|(value, _, touched)| {
            *touched = self.clock;
            value.clone()
        })
    }
    pub fn contains(&self, key: &K) -> bool {
        self.entries.contains_key(key)
    }
    pub fn insert(&mut self, key: K, value: V, bytes: usize) -> CacheInsert<V> {
        if bytes > self.budget {
            return CacheInsert {
                retained: false,
                evicted: vec![value],
            };
        }
        let mut evicted = Vec::new();
        if let Some((value, old, _)) = self.entries.remove(&key) {
            self.used -= old;
            evicted.push(value);
        }
        while self.used + bytes > self.budget {
            let Some(oldest) = self
                .entries
                .iter()
                .min_by_key(|(_, (_, _, time))| *time)
                .map(|(key, _)| key.clone())
            else {
                break;
            };
            if let Some((value, old, _)) = self.entries.remove(&oldest) {
                self.used -= old;
                evicted.push(value);
            }
        }
        self.clock = self.clock.wrapping_add(1);
        self.entries.insert(key, (value, bytes, self.clock));
        self.used += bytes;
        CacheInsert {
            retained: true,
            evicted,
        }
    }
    pub fn clear(&mut self) -> Vec<V> {
        let evicted = self
            .entries
            .drain()
            .map(|(_, (value, _, _))| value)
            .collect();
        self.used = 0;
        evicted
    }
    pub fn bytes(&self) -> usize {
        self.used
    }
    pub fn len(&self) -> usize {
        self.entries.len()
    }
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

#[derive(Clone, Debug)]
pub struct RenderTicket {
    pub generation: u64,
    pub key: RenderKey,
    pub priority: u8,
}
pub struct RenderQueue {
    generation: u64,
    pending: Vec<RenderTicket>,
    capacity: usize,
}
impl RenderQueue {
    pub fn new(capacity: usize) -> Self {
        Self {
            generation: 0,
            pending: Vec::new(),
            capacity,
        }
    }
    pub fn reset(&mut self, generation: u64) {
        self.generation = generation;
        self.pending.clear();
    }
    pub fn push(&mut self, ticket: RenderTicket) {
        if ticket.generation != self.generation || self.capacity == 0 {
            return;
        }
        if let Some(existing) = self
            .pending
            .iter_mut()
            .find(|queued| queued.key == ticket.key)
        {
            existing.priority = existing.priority.min(ticket.priority);
            return;
        }
        if self.pending.len() == self.capacity
            && let Some((worst, priority)) = self
                .pending
                .iter()
                .enumerate()
                .max_by_key(|(_, entry)| entry.priority)
                .map(|(i, e)| (i, e.priority))
        {
            if ticket.priority >= priority {
                return;
            }
            self.pending.remove(worst);
        }
        self.pending.push(ticket);
    }
    pub fn pop(&mut self) -> Option<RenderTicket> {
        let index = self
            .pending
            .iter()
            .enumerate()
            .min_by_key(|(_, entry)| entry.priority)
            .map(|(i, _)| i)?;
        Some(self.pending.remove(index))
    }
    pub fn len(&self) -> usize {
        self.pending.len()
    }
    pub fn is_empty(&self) -> bool {
        self.pending.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn key(index: u16) -> RenderKey {
        RenderKey {
            source: SourceId(1),
            fingerprint: SourceFingerprint {
                bytes: 42,
                modified_ns: 1,
            },
            index,
            rotation: 0,
            background_rgba: 0xffffffff,
            width: 800,
            device_scale_milli: 1000,
            kind: RenderKind::Page,
        }
    }
    #[test]
    fn cache_is_byte_bounded_lru_and_does_not_retain_oversized_bitmaps() {
        let mut cache = ByteCache::new(100);
        cache.insert(1, "a", 40);
        cache.insert(2, "b", 40);
        assert_eq!(cache.get(&1), Some("a"));
        let evicted = cache.insert(3, "c", 40);
        assert_eq!(evicted.evicted, vec!["b"]);
        assert!(!cache.contains(&2));
        assert!(cache.contains(&1));
        assert!(!cache.insert(4, "huge", 101).retained);
        assert_eq!(cache.bytes(), 80);
        cache.clear();
        assert_eq!(cache.bytes(), 0);
    }
    #[test]
    fn queue_prioritizes_visible_and_discards_stale_zoom_work() {
        let mut queue = RenderQueue::new(3);
        queue.reset(7);
        for index in 0..20 {
            queue.push(RenderTicket {
                generation: 7,
                key: key(index),
                priority: 2,
            });
        }
        assert_eq!(queue.len(), 3);
        queue.push(RenderTicket {
            generation: 7,
            key: key(19),
            priority: 0,
        });
        assert_eq!(queue.pop().map(|job| job.key.index), Some(19));
        queue.reset(8);
        queue.push(RenderTicket {
            generation: 7,
            key: key(2),
            priority: 0,
        });
        assert!(queue.pop().is_none());
    }
    #[test]
    fn gesture_accumulates_small_deltas_clamps_and_resets_between_interactions() {
        let mut gesture = GestureZoom::default();
        for _ in 0..100 {
            gesture.update(100., 0.001);
        }
        assert_eq!(gesture.update(100., 0.001), Some(111));
        gesture.reset();
        assert_eq!(gesture.update(100., 0.1), Some(110));
        assert_eq!(gesture.update(110., -0.5), Some(55));
        assert_eq!(gesture.update(55., -0.9), Some(50));
        assert_eq!(gesture.update(50., 10.), Some(400));
        assert_eq!(gesture.update(400., -0.01), Some(396));
        assert_eq!(gesture.update(396., f32::NAN), None);
        assert_eq!(gesture.update(396., -1.), None);
        gesture.reset();
        assert_eq!(gesture.update(75., 0.1), Some(83));
    }
    #[test]
    fn zoom_keeps_reading_anchor_and_fixed_page_label_at_same_viewport_position() {
        // Old top offset 200, pointer at viewport y=300 -> page-local y=500.
        let enlarged = zoom_anchor_offset(200., 500., 1200., 2.);
        assert_eq!(enlarged, 668.);
        assert_eq!(32. + (500. - 32.) * 2. - enlarged, 300.);
        assert_eq!(zoom_anchor_offset(enlarged, 968., 2400., 0.5), 200.);
        assert_eq!(zoom_anchor_offset(0., 32., 1200., 2.), 0.);
        assert_eq!(zoom_anchor_offset(0., 600., 1200., 0.5), 0.);
    }
    #[test]
    fn preview_preserves_source_identity_and_dpi_but_avoids_small_raster_duplication() {
        let mut full = key(19);
        full.width = 4096;
        full.device_scale_milli = 2000;
        let preview = full.preview();
        assert_eq!(preview.width, 512);
        assert_eq!(preview.index, full.index);
        assert_eq!(preview.source, full.source);
        assert_eq!(preview.fingerprint, full.fingerprint);
        assert_eq!(preview.device_scale_milli, 2000);
        assert_eq!(preview.preview(), preview);
        full.width = 1024;
        assert_eq!(full.preview(), full);
        full.width = 4096;
        full.kind = RenderKind::Thumbnail;
        assert_eq!(full.preview(), full);
    }
    #[test]
    fn fit_zoom_and_dpi_preserve_aspect_and_limit_manual_zoom() {
        let page = PageGeometry {
            width_points: 600.,
            height_points: 900.,
            quarter_turns: 0,
        };
        let size = Zoom::FitPage.size(page, (800., 600.));
        assert!(size.0 <= 736. && size.1 <= 536.1);
        assert!((size.0 / size.1 - 2. / 3.).abs() < 0.001);
        assert_eq!(Zoom::Percent(1).scale(page, (800., 600.)), 0.5);
        assert_eq!(Zoom::Percent(900).scale(page, (800., 600.)), 4.);
        assert!(RenderKey::pixel_width(size.0, 2.) > RenderKey::pixel_width(size.0, 1.));
        assert_eq!(RenderKey::pixel_width(50000., 4.), 8192);
    }
}
