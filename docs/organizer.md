# M2 page organizer

The workspace owns multiple sources and one authoritative ordered page plan. Page IDs
stay stable through move/rotate/delete/undo. Source PDFs are never written by organizer
operations. Save As and Extract remain M3; edits currently live only in this session.
Opening another PDF or closing the app discards this virtual plan.

The app opens in Baca mode. Its header contains the document name, Baca / Atur halaman,
Buka PDF and Lainnya (import and theme choices). Hover the name for full name/page size.
The fixed reading toolbar groups sidebar/history, page navigation and zoom. Enter in
the page field replaces the old Ke button; while focused, its draft is not overwritten
by scrolling. The Zoom dropdown contains fit width/page and fixed 50-400% presets.
At narrow widths first/last navigation moves into Navigasi halaman. Copy appears only
with a text selection. Status/footer shows source count and session changes, not controls.

In Baca, thumbnail clicks navigate without selecting pages and page dragging is disabled.
Atur halaman opens the sidebar and selects the current page when no pages are selected.
Returning to Baca clears page selection/anchor/drag; text selection survives mode changes.
Mode changes never change content revision or dirty status. A successful import enters
Atur halaman; failure leaves the current mode/page plan intact.

In Atur halaman, use Ctrl-toggle or Shift-range on thumbnails. In the thumbnail panel:
Up/Down selects, Shift+Up/Down extends, Ctrl+Up/Down moves the reading position without
changing selection, Space toggles, Ctrl+A selects all pages, Esc clears page selection
and cancels drag. Delete removes selected pages; Ctrl+R / Ctrl+Shift+R rotates
clockwise/counterclockwise. Alt+Up/Down or Pindahkan > Lebih awal / Lebih akhir moves
selection as an ordered block. Seleksi contains Pilih semua and Bersihkan pilihan.
Toolbar icons expose accessible names and shortcuts in tooltips. Page actions without
a selection are disabled. Undo/Redo remain in the reading toolbar in both modes.

Drag selected thumbnails onto another thumbnail to insert before it. A line marks the
drop target; the end drop zone appears only while page dragging. One drop commits one
command; Escape/cancel commits none. Dragging an unselected thumbnail moves only it.
Ctrl+Shift+O imports one complete PDF at a time, appended in import order.
Ctrl+Z / Ctrl+Y undo/redo; the toolbar exposes the same operations.

Move preserves the original relative order of selected pages, including non-contiguous
selections. Its target is an insertion boundary in the pre-removal list. Delete clears
removed IDs from selection. Import selects imported pages, move selects moved pages,
and rotate preserves selection. Undo/redo restores the operation's before/after selection.
Selection alone does not change the content revision. Returning to the opening revision
via Undo clears dirty; a new edit after Undo drops redo. No-op commands add no history.

History retains at most 128 compact deltas (affected entries/positions, selections,
revision IDs and imported source metadata), rather than complete workspace snapshots.
An empty workspace is valid and supports import/undo. Import is inspected in the native
background worker and only committed after validation; failure preserves existing pages.
Duplicate source paths/IDs are refused. Default aggregate limits are 1024 source pages
(including deleted pages of registered sources) and 128 MiB of source files. Deleted
sources remain registered for undo; undoing import unregisters that source.

Rendering resolves the source for every page, composes intrinsic and virtual rotations,
and rotates owned RGBA off the UI thread. Text boxes rotate in normalized coordinates.
For correctness, every edit clears positional text/cache/selection and raster residency,
and advances a token rejecting old asynchronous text results. This conservative policy
may rerender unchanged pages after edits; preserving caches across edits is deferred.
Zoom and navigation continue to reuse caches normally. The single bounded native session
may reopen frequently for alternating sources; there is no unbounded document pool.

Checks: workspace tests include deterministic edit/undo, seeded move/rotate sequences,
invalid commands, empty state, multi-source import, no-ops and bounded history. Native
smoke checks glyph/raster overlap for 16 intrinsic/virtual rotation combinations.
`scripts/m2-ui-smoke.ps1` exercises real UI commands and a native import dialog using
only its own window, verifies numeric page-plan metrics and hashes both source copies.
Physical modifier-key gestures and broader drag/drop cases still require the manual matrix below
unless explicitly recorded as automated evidence in testing.md.

Manual acceptance: select non-adjacent thumbnails with Ctrl; Shift-range both ways;
keyboard select/toggle/clear; drag several selected pages before/after their original
positions and to the end; Escape/cancel a drag; undo/redo each action; repeat with mixed
portrait/landscape and two sources. Rotate text pages and select/copy at each orientation.
Check that text fields retain normal editing keys. No export-preservation claims apply.
