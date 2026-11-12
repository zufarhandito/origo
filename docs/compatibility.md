# Current compatibility

M3 is a Windows x64 PDF organizer/exporter using GPUI Kit 0.9.1. Multiple PDF sources,
page selection, reorder, rotate, delete, import and bounded Undo/Redo are implemented.
Save As exports the full plan; Extract exports selected pages in workspace order.
Both require a new .pdf destination and never overwrite existing files or sources.
Only a successful full export matching the current workspace instance/revision marks
that revision clean. Extract and older snapshots do not. Unsaved edits are discarded
on Open/close; there is no recovery or automatic save prompt. See organizer.md for controls and
aggregate limits. Baca mode is for navigation/text; Atur halaman exposes page operations.
Mode changes do not mark content dirty. Structural edits explicitly clear text selection and positional caches.

Konversi is an informational roadmap page with Akan datang cards. PDF-to-images,
Word/TXT/Excel/PowerPoint and conversion into PDF are not operational. No converter
runtime, OCR model or network upload was added. Save As/Extract produce PDFs only.
See m3-safe-export-plan.md and conversion-roadmap.md for scope and prerequisites.

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
be interrupted in-process. Export QPDF child processes support cancellation, bounded
stdout/stderr and a cumulative 120-second process-time budget (excluding user dialogs).
PDFium output validation is cooperative before/after the native call, without a hard timeout.

Export preflight requires clean QPDF 12.4.2 checks and recognized JSON v2 structure.
Encrypted, signed, AcroForm/XFA, tagged, annotation/link, attachment,
active-content, optional-content and unknown complex catalog/page constructs are blocked.
QPDF recovery warnings also block export. Ordinary Info/XMP document metadata may be
lost and requires explicit acknowledgement for that snapshot.
Ordinary navigation bookmarks (including nested outlines, local GoTo and named
destinations) are allowed only after explicit **Simpan tanpa bookmark** confirmation;
the output omits them. Detection validates the outline graph and local destinations.
Remote/JavaScript/chained/unknown bookmark actions remain blocked. If metadata is
also present, one confirmation describes both losses. Cancelling leaves no output
and preserves dirty status. This applies to Save As and Extract separately.
No implicit flattening,
decryption, signature stripping or rasterization is performed. Supported pages retain
PDF content/resources, but universal semantic preservation is not claimed.

Private source copies and staged output are owned by the job in the destination folder.
QPDF checks and PDFium count/geometry/orientation validation precede a flushed
non-clobber commit. Source size/mtime is rechecked, including before publication.
Failure/cancel cleans owned staging; crash-orphan recovery and universal crash durability
are not implemented. See ADR-0003 and m3-safe-export-plan.md.

DPR layout/raster math is tested; physical mixed-monitor 100/150/200% acceptance is
pending. Broader complex-PDF/accessibility tests, runtime/license audit and clean-machine
portable packaging remain release gates. A successful optimized build is not a packaged
product release. No new toolchain installation was required for M1.

