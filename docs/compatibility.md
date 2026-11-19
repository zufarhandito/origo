# Current compatibility

## Cetak dan navigasi outline (19 November 2026)

[Cetak](printing.md): snapshot immutable, raster PDFium dengan print intent 150 DPI,
managed helper Windows Forms terisolasi dan publikasi virtual melalui LocalOutput.
Print tidak mengubah baseline bersih. Microsoft Print to PDF menghasilkan gambar;
printer virtual file lain belum didukung. Printer fisik perlu acceptance perangkat.
[Daftar isi](outline.md): DTO bounded tanpa native handle, load async per sumber,
cache per view dan tujuan dipetakan melalui PageId. Aksi eksternal tidak dieksekusi.
Kemampuan membaca outline tidak mengubah consent/preservation ekspor bookmark.


Text highlights, freehand pen/marker, colors, select/delete and Undo/Redo are available
under **Anotasi**. Standard PDF Ink/Highlight objects persist through safe Save As;
PDF Studio metadata enables reopening/editing its own annotations. Generic third-party
annotation editing is not supported. Policy v5 admits this bounded passive subset,
with action rejection and round-trip validation. Other export restrictions remain.
Visual signatures support drawing or PNG/JPEG import, placement, drag/resize and
reopen/edit of PDF Studio-owned passive Stamp annotations. Policy v6 extends the
owned subset; certificate signatures remain unsupported. See [signatures](signatures.md). See [annotations](annotations.md) and
[competitor research](research/pdf-annotations-and-signatures.md).

Two different open PDFs can be shown side by side with the GPUI Kit Dock splitter.
Both visible panes render; focus selects the shared toolbar/organizer target.
Hidden documents retain edits and reading state while releasing caches. The window
cache budgets and safe-export policies remain unchanged. See [multi-tab](multitab.md).
One PDF can also have two views with independent reading/search/text selection and
a shared page plan, selection, edit history and export actor. Tab capacity still
counts documents; session restore/reopen stores one preferred reading position per PDF.

Ctrl+F searches the active workspace with a compact in-tab bar, counts, highlights
and next/previous navigation. Default matching is case-insensitive and treats a phrase
as a contiguous sequence; same-page whitespace/linebreaks are normalized. Optional case
and whole-word matching are available. Search is bounded, progressive and per tab;
scans require text layers, and extraction failures are reported as incomplete results.
See [search](search.md). Historical statements below that text search is unavailable
are superseded by this addition.

Desktop navigation now uses grouped menus and up to eight independent document tabs.
Open creates/focuses a tab; unsaved protection applies to tab/window Close, with
transactional cancellation across all tabs. See [multi-tab](multitab.md). Historical
single-workspace Open-replacement and conversion-page descriptions below are superseded.

PNG export is under **Alat → Ekspor PNG…**. It captures all or selected workspace
pages at 150/300 DPI to a new folder, including source/virtual rotations. The raster
is opaque white, limited to 8192 pixels/side and 16 Mi-pixels; oversized pages are
rejected rather than downscaled. Encoded batch output is capped at 1 GiB. PNG uses
normal conservative source preflight; image conversion omits interactive semantics.
It never marks PDF edits clean. See [PNG](png.md) for limits and folder safety.

Optimize PDF (lossless) is under Alat. It exports the entire captured page plan
to a new PDF, comparing an ordinary export of that same plan with the final bytes.
Object/stream recompression does not rasterize or lower image quality; valid output
may grow. It follows full Save As compatibility/consent, and never marks edits saved.
See optimize.md. No wider form/signature/tag preservation support is implied.

Split PDF is available under Alat. It divides the captured plan per N pages or
increasing, non-overlapping manual ranges into a new Windows output folder. All
parts pass the existing Extract policy and validation before publication. Failure
or cancellation publishes no batch; existing directories/files are refused and
Split never marks edits saved. See split.md for controls, consent and limits.

M3 is a Windows x64 PDF organizer/exporter using GPUI Kit 0.9.1. Multiple PDF sources,
page selection, reorder, rotate, delete, import and bounded Undo/Redo are implemented.
Save As exports the full plan; Extract exports selected pages in workspace order.
Both require a new .pdf destination and never overwrite existing files or sources.
Only a successful full export matching the current workspace instance/revision marks
that revision clean. Extract and older snapshots do not. Unsaved edits prompt for
Save As, discard or cancel on Open/close. Failed/cancelled Open keeps the old workspace.
Reading-only session restore is available; unsaved edits and export staging have no
autosave/crash recovery. See multitab.md and unsaved-changes.md. See organizer.md for controls and
aggregate limits. Baca mode is for navigation/text; Atur halaman exposes page operations.
Mode changes do not mark content dirty. Structural edits explicitly clear text selection and positional caches.

The old Konversi catalog is informational. PNG export is now operational separately
under Alat; JPEG, TXT, Word/Excel/PowerPoint and conversion into PDF remain deferred.
The pinned renderer supplies PNG rasters; no OCR model or network upload was added.
Save As/Extract continue to produce PDFs only.
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

Operational failures appear in a GPUI Kit modal error dialog with a specific reason
and Tutup, including export, Open and import. Error text supports drag selection through
GPUI Kit SelectableText; Salin pesan copies the complete original message without closing
the dialog. Cancellation is normal feedback rather
than an error popup. Background failures do not stack dialogs. Catalog export blockers
name the detected attribute instead of listing unrelated possibilities; unsupported
structures remain blocked pending validated preservation support.

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

Policy v3 adds a narrow whole-source Extract path: selecting every page of one source
in its original order with no virtual rotation produces a byte-identical copy from
the private snapshot. It can preserve tags, viewer preferences and an AcroForm that
contains only an empty Fields array, without dropping metadata/bookmarks or requesting
loss consent. Active fields, XFA, signatures, encryption and active content remain
blocked. This is not support for restructuring tagged/form PDFs: partial selection,
rotation/reorder and Save As still use the conservative structural-export policy.

Policy v4 permits tagged single-source Extract with explicit **Simpan tanpa tag**
consent. Accessibility tags/reading-order structure are omitted; the warning also
lists bookmark/metadata loss when detected. The selected pages remain PDF content,
and validated passive Ink annotations are preserved rather than flattened. Other
annotation types, annotation actions/extra appearances, active forms/XFA, signatures
and encryption remain blocked. Whole-source unmodified extraction still copies all
original bytes without loss or a tag-removal warning. Full Save As and multi-source
tagged restructuring remain outside this exception.

Private source copies and staged output are owned by the job in the destination folder.
QPDF checks and PDFium count/geometry/orientation validation precede a flushed
non-clobber commit. Source size/mtime is rechecked, including before publication.
Failure/cancel cleans owned staging; crash-orphan recovery and universal crash durability
are not implemented. See ADR-0003 and m3-safe-export-plan.md.

DPR layout/raster math and actual connected-display DPI 96/100% with UIAutomation/
targeted input checks are tested. Physical 150/200%, mixed-monitor, trackpad and
screen-reader/modifier-key acceptance remain pending; see [device matrix](device-acceptance.md).
An internal portable candidate and runtime/notice inventory are now available; see
[distribution audit](distribution-audit.md). Broader complex-PDF/accessibility tests,
license/security review and clean-machine portable acceptance remain release gates.
A successful candidate build is not a published
product release. No new toolchain installation was required for M1.

