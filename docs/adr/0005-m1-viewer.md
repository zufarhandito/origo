# ADR-0005: M1 metadata, virtualized rendering and bounded residency

Status: accepted for M1, 2026-11-09.

Opening a document should not rasterize every page. A pure domain workspace stores
stable source/page identities and inspected geometry; application ports keep native
handles away from presentation. Commands/history and export are deferred.

Use virtual page/thumbnail lists, one background worker and a 32-entry priority queue.
Visible pages outrank neighbors and thumbnails. Generations remove obsolete pending
work and discard stale completions; native calls are serialized and not interruptible.

Use source/fingerprint, geometry-affecting state, raster bucket and DPR in cache keys.
Separate byte-budgeted CPU LRUs default to 128 MiB pages and 16 MiB thumbnails, large
enough for visible maximum-resolution pages without constant eviction/rerender loops.
Native bitmap limits independently cap dimensions and pixels.

CPU LRU eviction alone did not bound GPU memory in actual Windows stress testing:
the window atlas retained uploaded images, including images still cached on CPU.
Track GPU residency separately for viewport plus adjacent items and explicitly release
textures via a deferred update using the current Window. Keep CPU cache hits available
for re-upload. The final two-pass 100-page debug test used about 296-303 MiB working
set and about 48 MiB shared GPU, with bounded CPU caches and zero release errors.
Shared GPU accounting can overlap process working set; these figures are not additive.

Size/mtime fingerprints detect source changes on native reads without hashing entire
files. Cached pages have no live file watcher. Source changes require reopening.
Physical mixed-monitor DPI, hostile native-call containment and portable packaging
remain later acceptance work; see testing.md.
