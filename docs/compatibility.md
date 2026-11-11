# Current compatibility

M2 is a Windows x64 virtual page organizer using GPUI Kit 0.9.1. Multiple PDF sources,
page selection, reorder, rotate, delete, import and bounded Undo/Redo are implemented.
Edits are session-only: Save As and production exports remain M3. No form/signature
preservation claims are made. See organizer.md for controls, selection semantics and
aggregate limits. Baca mode is for navigation/text; Atur halaman exposes page operations.
Mode changes do not mark content dirty. Structural edits explicitly clear text selection and positional caches.

Text selection is available as a requested viewer addition before M2: single-click
clears without selecting, drag over text to select, double-click selects a sentence,
Ctrl+C or Salin to copy, Esc or a blank page click to clear. Selection survives zoom
and scroll. Drag may cross simultaneously visible pages; automatic edge scrolling,
separate word/paragraph selection modes, whole-document Select All and text search are not included.
Copy follows PDFium's extracted order, which can differ from intended reading order
for columns, rotated layouts, ligatures and unusual fonts. Image-only scans need OCR;
OCR is not implemented. See text-selection.md for bounds and current evidence.

Default adapter limits are 128 MiB input, 1024 pages, 8192 pixels per bitmap dimension
and 16 million bitmap pixels (64 MiB RGBA). ResourceLimits can be configured in Rust.
High zoom may display a capped-resolution raster. CPU page/thumbnail caches default
to 128/16 MiB; GPU textures retain only the viewport and adjacent pages.

Native pinch events zoom over the document viewport; Ctrl+scroll is also supported.
Ordinary two-finger scroll remains document navigation. Zoom keeps fractional gesture
deltas and preserves the current page's reading anchor, subject to scroll boundaries
and the 50-400% manual range. Pinch delivery depends on Windows precision-trackpad
support/driver configuration. Physical trackpad acceptance is still pending.

Missing dependencies, invalid/password-protected documents and resource violations
return typed errors. There is no password prompt. Source size/mtime changes detected
during a new native read invalidate the workspace; cached pages are not live-watched,
and fingerprints are not cryptographic hashes. Reopen after editing a source externally.

PDFium calls are serialized and have no hard timeout or process sandbox. A hung native
call can occupy the single worker; hostile-document containment remains future work.
The UI can navigate while normal rendering proceeds, but stale native calls cannot
be interrupted in-process. QPDF runs only for trusted synthetic smoke tests.

DPR layout/raster math is tested; physical mixed-monitor 100/150/200% acceptance is
pending. Broader complex-PDF/accessibility tests, runtime/license audit and clean-machine
portable packaging remain release gates. A successful optimized build is not a packaged
product release. No new toolchain installation was required for M1.

