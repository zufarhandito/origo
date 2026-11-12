# Architecture - M3

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
metrics file writes run outside those callbacks. Optional metrics are numeric only.

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
presence in this catalog. No converter has been registered or implemented; no plugin discovery/placeholder
engine crates have been added.

M3 application/export.rs owns immutable plans, preflight/export/output ports, phases,
cancellation and completion receipts. A workspace instance advances on Open independently
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
remain blocked. Closing during a job
requests cancellation and waits for owned cleanup. PDFium validation remains in-process.

See m3-safe-export-plan.md and ADR-0003 for policy and known limits.
conversion-roadmap.md reserves separate
structural-PDF, raster, text, reflow/Office and OCR adapter boundaries, sharing snapshots
and output transactions when implemented. Only complete PDF Save As can advance a clean
revision; a derived conversion output will not mark the virtual PDF plan saved.
