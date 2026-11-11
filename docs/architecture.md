# Architecture - M1

```text
desktop (GPUI Kit) -> application -> domain
       |                  ^
       +-> pdf-engine ----+   (PDFium adapter)
smoke -> pdf-engine + qpdf-adapter
```

The domain owns a read-only single-source Workspace: source identity/fingerprint,
stable PageId, source index and validated page geometry. Opening inspects all page
metadata without rasterizing. Export support remains NotPreflighted; there are no
organizer commands or history yet. QPDF remains a trusted synthetic smoke adapter.

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

