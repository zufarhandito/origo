# Architecture - desktop multi-tab

## Cetak dan navigasi outline (19 November 2026)

[Cetak](printing.md): snapshot immutable, raster PDFium dengan print intent 150 DPI,
managed helper Windows Forms terisolasi dan publikasi virtual melalui LocalOutput.
Print tidak mengubah baseline bersih. Microsoft Print to PDF menghasilkan gambar;
printer virtual file lain belum didukung. Printer fisik perlu acceptance perangkat.
[Daftar isi](outline.md): DTO bounded tanpa native handle, load async per sumber,
cache per view dan tujuan dipetakan melalui PageId. Aksi eksternal tidak dieksekusi.
Kemampuan membaca outline tidak mengubah consent/preservation ekspor bookmark.


One tab can retain two reading entities with distinct ViewIds and one shared
UI-thread Workspace handle. Cloning that handle does not clone document history.
The model also owns the monotonic import-source allocator and inspection busy flag.
Observers compare revision/dirty/selection/import signatures; sibling views refresh
their caches, text tokens and Find without clearing shared page selection. Each
view retains its PageId anchor and independent viewport/search/text selection.
Deleting that anchor falls back to the nearest surviving index.

The primary entity remains the canonical export/PNG/Split/close actor until the
document tab closes, even while its reading view is hidden. Secondary actions
forward before plan capture; only progress/control/notice are mirrored. The shared
model receives one receipt. Closing a pane hides its reading entity and releases
caches, while tab/window Close uses one transactional document guard. Session/reopen
metadata comes from the preferred reading view and still records one document.

Split comparison uses GPUI Kit DockArea/DockLayout with a locked two-panel layout;
resizing stays enabled and Dock's close/zoom controls cannot remove PDF sessions.
Uncached panel rendering preserves the native accessibility tree after modal/focus
changes observed in this integration. WindowShell separates focused (`active`) from
visible (`visible_for_render`) sessions. Both visible sessions schedule rendering,
text and Find; hidden ones release residency. Visible panels divide the window cache
and queue budgets equally. One external toolbar/organizer targets the focused PDF;
window-global canvas listeners also exclude the shared drawer/tab-picker overlays.

Reading session snapshots are versioned, bounded local JSON, separate from export
transactions and unsaved edits. WindowShell polls metadata changes every second,
coalescing writes to one running task plus the latest pending snapshot. Normal window
close captures pre-close reading state and waits for writes; individual last-tab close
persists empty state. Startup without an explicit PDF offers restore. Reinspection,
partial failure and fingerprint rejection apply before bookmark projection. Corrupt
metadata is preserved until explicit clear; session write failure during exit cancels
that exit once and reports how to retry without a session checkpoint.

Shell tab recovery keeps at most 20 metadata-only closed-tab entries in memory.
Only successful close batches record entries; reopening reinspects the primary source
and creates a fresh session, or focuses an existing duplicate. ReadingPosition maps
the viewport to the primary source page and defers offset/zoom projection until the
new viewport is measured. Changed fingerprints reset the reading position. Tab search
uses shell paths/status only and an overlay input, with a shared pointer-input guard
so window-global PDF canvas listeners cannot select text behind the overlay.

Find state belongs to each DocumentSession. A debounced query scans one workspace
page at a time on the shared PDFium worker; application/search implements bounded
literal phrase matching with source-character ranges. Only compact occurrences are
retained (10,000 maximum); visible highlights reuse TextPage geometry in the existing
window-wide text budget. Query/source tokens reject stale callbacks, hidden sessions pause
incomplete scans, and page edits restart against the current plan. Search diagnostics
exclude query/text content. See [Find behavior and limits](search.md).

PNG jobs capture a separate immutable
PngPlan and use an exact-size PDFium request, then background PNG encode/full-decode
validation. They reuse Split's private snapshots/no-replace folder publication, share
the window export slot and cannot mark PDF edits clean. PNG is the only active raster
converter; see [PNG](png.md).

WindowShell owns stable TabIds, active-tab/menu state, transactional multi-document
close and window-wide export/modal coordination. Each DocumentSession is a separate
GPUI reading entity; views of one tab share workspace/history and retain separate input/viewport. Async completions
target their originating weak entity; removed tabs cannot be confused with new indices.
Sessions share a cloned PDFium renderer/worker. Hidden sessions stop scheduling,
clear CPU raster/text caches and defer GPU release through the actual Window. Thus
cache budgets apply to visible sessions across the window, not eight independent
resident caches. Metrics writes also share a sequence/lock across sessions.
See [multi-tab](multitab.md) for lifecycle, layout and remaining verification limits.

Optimize adds ExportScope::Optimized and optional OptimizationSizes on the receipt.
QPDF builds an ordinary current-plan baseline in staging, then applies lossless
object/stream compression in a second pinned JSON job. OutputTransaction exposes
owned artifact sizes; the application reads them after validation and before commit.
The optimized receipt cannot advance the clean revision. Desktop exporting.rs reuses
the existing picker, consent, export slot, cancellation and Open/Close guard. See
optimize.md for comparison semantics and preservation limits.

The first M4 slice adds application/split: immutable per-N/manual plans, aggregate
consent and a distinct receipt that cannot advance the PDF clean revision.
platform/split owns one shared source snapshot and a ready directory. Parts reuse
finish_export validation before flushing and Windows no-replace publication.
PdfInspector adds inspect_output with transient validation handles; normal viewer
session reuse is unchanged. Desktop splitting.rs uses the existing export slot,
progress timer and Open/Close guard. See split.md.

```text
desktop (GPUI Kit) -> application -> domain
       |                  ^
       +-> pdf-engine ----+   (PDFium adapter)
smoke -> pdf-engine + qpdf-adapter
desktop -> qpdf-adapter + platform -> application
```

The domain owns a multi-source virtual Workspace: source identity/fingerprint, stable
PageId, source index, virtual rotation, authoritative page order, page selection and
bounded inverse-delta history. Commands validate before mutation; no-op operations
are omitted. Undo/redo restores state revisions and selection; dirty compares against
the opening or successfully exported baseline. Sources are retained until their import
is undone. An empty plan is valid, but export rejects it. QPDF production export is
separate from the trusted synthetic M0 probe.
See organizer.md for command semantics, limits and conservative cache invalidation.

Application ports expose inspected metadata and owned RGBA, without GPUI/native
handles. A dedicated adapter thread owns Pdfium and a borrowed document session;
cloned renderers share its bounded channel. No native handles cross the channel.
The same document is reused for inspection, page and thumbnail rendering, recycled
after 64 render attempts and released after 15 idle seconds, source changes/errors,
or renderer disconnection. Version 0.9.5 initializes the pinned DLL once and serializes
native operations/destruction through a process-wide mutex. Native reads verify
size/mtime before and after use.
RGBA is explicitly converted to GPUI BGRA before presentation.

The desktop virtualizes page and thumbnail lists. Metadata determines placeholder
sizes without measuring/rasterizing every page. One render worker drains a bounded
32-entry priority queue: visible previews, visible full rasters, neighbors, thumbnails. Generation changes
discard pending obsolete work and reject stale completed results. Errors are retained
per generation to avoid retry loops; changed sources clear the workspace for reopening.

Render keys include source/fingerprint, index, rotation, background, bucketed raster
width, DPR and page/thumbnail kind. Separate byte-counted LRU CPU caches default to
128 MiB and 16 MiB. PDF_STUDIO_PAGE_CACHE_MIB accepts 128-512 and
PDF_STUDIO_THUMBNAIL_CACHE_MIB accepts 16-64. Oversized bitmaps are not retained.
GPU residency is limited independently to visible pages/thumbnails plus neighbors.
Releases defer a window update and call drop_image with the actual Window: dropping
CPU Arc values or passing no current window alone does not free window atlas entries.

Fit layouts use PDF points at 96/72 logical pixels; raster keys incorporate actual
window DPR. Manual zoom is clamped to 50-400%. Large rasters are capped while keeping
aspect ratio. Visible requests wider than 1024 raster pixels first request a 512-pixel
preview. The preview shares the byte cache and is displayed until the final bitmap
arrives; obsolete generations still drop both stages. Smaller requests avoid duplicate
preview work. Already-running native work cannot be preempted. UI item callbacks read cache/layout and defer scheduling; PDF work and
metrics file writes run outside those callbacks. Optional metrics contain counters,
sizes and fixed operation states, without document paths or content.

See ADR-0005, books.md and testing.md for measured memory and remaining acceptance limits.

Trackpad zoom uses GPUI's native PinchEvent from Windows Direct Manipulation.
The document viewport handles pinch in bubble phase; a canvas registers Ctrl+scroll
in capture phase so the virtual list does not scroll before zoom consumes the event.
Unmodified scroll events pass through. GestureZoom retains fractional percentages
between events and resets on gesture boundaries or explicit zoom/document changes.
Gesture layout restores the current item's vertical offset and horizontal pan around
the pointer, excluding the fixed label/gap. Scroll boundaries may constrain the anchor.

The text port returns owned Unicode characters and normalized top-left rectangles.
PDFium converts native character boxes with its page-to-device mapping, including
intrinsic rotation; a fixed virtual coordinate surface does not allocate a bitmap.
Extraction runs on the existing document worker, capped before constructing the
character-index collection. Visible-page text is loaded independently of raster zoom
generations, accepted only for the current SourceId and stored in a 16 MiB byte LRU.
No native handles cross the port. One text request is in flight; thumbnails do not
load text. Selection stores page/character endpoints, and the view paints merged
rectangles on an explicitly top-left-positioned canvas over each page image.
Clipboard writes use the toolkit on an explicit copy action, capped to 1 MiB.


## Reader/organizer presentation

ViewerMode is UI-only. Baca restricts thumbnails to navigation; Atur halaman exposes
page commands. Both modes share the same viewer, document plan, history, render worker
and cache budgets. Header/toolbars and deferred current-page input projection live in
chrome.rs. The field is not mutated during render and focused drafts survive scrolling.

A global canvas mouse listener does not automatically inherit overflow clipping for
hit-testing. Text overlays capture the paint-time content mask and restrict down/move
to both page and clip bounds, preventing offscreen page regions from swallowing toolbar
or sidebar clicks. Mouse-up remains global for release outside the page/window viewport.

## Export and planned conversion boundaries

The Konversi home is a third UI-only mode with a presentation catalog in conversion.rs.
All cards are non-interactive groups marked Akan datang, with stable accessibility IDs.
The reader page plan, text cache and history remain shared; navigation/zoom/history
commands are guarded while the hub is displayed. Opening a PDF returns to Baca;
successful import still enters Atur halaman. No converter capability is implied by
presence in this catalog. PNG export is available separately through Alat and the
dedicated application/PNG pipeline; no plugin discovery or placeholder engine crates
have been added.

M3 application/export.rs owns immutable plans, preflight/export/output ports, phases,
cancellation and completion receipts. A workspace instance advances on successful replacement independently
of text invalidation tokens. Only a full committed receipt matching instance and revision
can update the clean baseline. Selected exports and stale jobs cannot.

The platform crate owns private snapshots, same-parent staging, flushed non-clobber
commit and bounded hidden child processes. qpdf-adapter owns version checks, strict
JSON v2 feature preflight and QPDFJob compilation. Consecutive source runs preserve
workspace order; relative rotations use final output positions. Owned prepared
transactions keep the report bound to the copies it inspected; there is no preflight cache.
The UI captures before the native save picker, runs native work off-thread, asks for
bookmark/metadata consent when needed and publishes sanitized notices. Reading/editing can
continue against the live workspace; one export runs per window. Policy v2 separates
bookmark omission from metadata loss, validates ordinary local outline/name trees,
and requests specific consent before dropping navigation bookmarks. Complex actions
remain blocked. Closing during a job waits for export, then checks unsaved changes.
Failed/cancelled exports abandon the pending close. PDFium validation remains in-process.

Historically, transitions.rs owned a single Open/Close intent, candidate inspection, the unsaved
decision and receipt-gated continuation. Replacement is installed only after inspecting
the candidate and rechecking its fingerprint. Failed/cancelled Open retains the old
plan/history/reading state. Page edits/history/import and additional exports are
guarded while leaving; the modal shares the error popup's global input guard.
That Open-replacement flow has been superseded: shell.rs opens new sessions and owns
close batches; transitions.rs retains each document's receipt-gated unsaved close.
See unsaved-changes.md. No domain or export preservation contracts change.

feedback.rs centralizes modal operational errors using GPUI Kit AlertDialog and a
deferred window update. A per-viewer guard prevents dialog stacking and background
text selection; on_close restores interaction. Export cancellations remain non-modal.
Catalog preflight reports feature-specific reasons or a bounded sanitized unknown
attribute name, without exposing object contents or raw QPDF diagnostics.

ExportPlan::is_whole_source_extract recognizes only complete, original-order,
unrotated selections from a single source. ExportPreflight::preflight_plan defaults
to normal source preflight; QPDF additionally validates a restricted copy-only mode.
The writer copies the staged snapshot with bounded buffers/cancellation, then uses
the same validation/transaction/receipt pipeline. Plan fields and the prepared plan
are private to keep the chosen mode immutable. Copy-only preflight preserves tagged
structure, viewer preferences and verified empty AcroForm instead of rebuilding them.

Policy v4 adds a separate tags_loss flag to the prepared report. Single-source
Extract may use a tagged-input preflight only with the same closed checks for
encryption/forms/signatures/active content. Passive Ink objects and annotation arrays
are validated before bypassing generic annotation rejection; page dictionaries are
still scanned recursively with only the validated Annots property omitted.

After consent, structure.rs builds a bounded JSON v2 update for the staged output's
current object IDs. It removes accessibility references, including stream-dictionary
StructParents, without replacing stream data or Ink geometry. A fresh inspection
checks cleanup before the existing validation/commit pipeline. Whole-source copy
returns earlier and preserves tags; ordinary full/multi-source exports are unchanged.

See m3-safe-export-plan.md and ADR-0003 for policy and known limits.
conversion-roadmap.md reserves separate
structural-PDF, raster, text, reflow/Office and OCR adapter boundaries, sharing snapshots
and output transactions when implemented. Only complete PDF Save As can advance a clean
revision; a derived conversion output will not mark the virtual PDF plan saved.

## Annotation authoring (17 November 2026)

Workspace owns bounded per-PageId annotation lists, baseline source lists and a shared
ID allocator. Atomic AnnotateMany history transactions preserve multi-page selection
and shared-view revisions. View-local tool/draft/selection state never enters session
restore. Export/Png snapshots carry annotation lists with page identity.

PDFium reads the versioned model from annotation NM. During editor rendering it
hides only recognized owned objects, renders the original page, restores flags even
on failure, then applies the bounded raster compositor. Unknown annotations remain
native. QPDF patches page Annots after assembly with standard Ink/Highlight objects
and vector AP streams; Optimize runs afterwards. Preflight validates the passive
closed schema before generic exemptions, including inline annotation dictionaries.
Output inspection checks annotation round-trip equality before publication. Policy
version is 5. See [behavior and limitations](annotations.md).


## Visual signature Stamp (policy v6)

An optional Arc<SignatureImage> on Annotation carries bounded RGBA data and two
opposite normalized corners. History/snapshots share asset allocation; each placed
object has its own ID/corners. Aggregate signature count is eight across Workspace,
including deleted-page data retained for Undo. Inspection also caps assets at eight.
Draw/import and placement state are per view and are not saved in session metadata.

QPDF writes owned passive Stamp annotations, Form appearances, RGB image XObjects and
8-bit grayscale soft masks. Versioned NM metadata retains bounded asset/corners for
reopen/edit. This is a visual image, not a cryptographic signature field. PDFium hides
owned Stamp objects during editor rendering and the compositor applies alpha once;
native PDF output uses the Form appearance. Export validation compares the full model.
Image input is decoded locally with format/byte/dimension/allocation limits and EXIF
orientation; transparent borders are cropped, with reduction to 512x256 if necessary.
No permanent template storage or external service was added. See [signatures](signatures.md).
