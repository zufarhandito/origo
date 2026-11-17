# PDF to PNG

Use **Alat → Ekspor PNG…** on the active tab. Choose **Semua halaman** or
**Halaman terpilih**, 150 DPI (default) or 300 DPI, a new folder name, then its
parent folder. Selection is captured from the organizer; an empty selection disables
the selected-only option. Default output folder name: `hasil-png`.

Each captured output page becomes `page-0001.png`, `page-0002.png`, etc. Numbering
starts at one for this batch, in workspace order, including imported sources and
virtual rotations. The image background is opaque white. Text/links/bookmarks are
pixels rather than interactive PDF elements. Source PDF bytes and the workspace's
dirty/Undo baseline are unchanged. JPEG, TXT, OCR, ZIP export, transparency and
embedded-image extraction are outside this feature.

## Resolution and bounded execution

Pixel dimensions are `round(displayed_points × DPI / 72)` after source/virtual rotation,
independent of window DPI, viewer zoom and preview quality. The exact export request
never uses the viewer's automatic downscaling or 32-pixel minimum. A page exceeding
8192 pixels on either side or 16 Mi-pixels is rejected before native allocation;
choose 150 DPI or fewer/smaller pages. DPI controls raster dimensions; this first
encoder does not write a physical-print `pHYs` DPI metadata tag.

The plan is immutable from the configuration dialog. Sources are privately snapshotted
and checked before/after reads and before publication. Only referenced sources are
included. Normal conservative preflight is used, without whole-source/tagged Extract
exceptions: ordinary metadata/bookmarks may be converted, while forms, signatures,
tags, annotations, encrypted, active and unsupported complex inputs remain blocked.
The dialog explains the conversion's loss of interactive semantics before submission.

One page is rendered, encoded and decoded for validation at a time, in background
work. Original RGBA is encoded before the viewer's BGRA conversion. Decoded pixels
must match exactly. The writer checks cancellation and counts bytes while streaming;
the whole batch is capped at 1 GiB of encoded PNG data. Native rendering itself is
not preemptible; cancellation is observed after the current render and during writes.
PNG jobs have a 30-minute accumulated work budget. No document contents/paths are
added to numeric diagnostics.

## Folder publication and multiple tabs

Results stay in an owned sibling staging directory until every PNG passes validation.
Shared Split snapshot/publication primitives flush files and move the result directory
without replacing an existing destination. Existing files/directories, including a
destination created during export, are rejected. Failure/cancel removes owned staging;
there is no partial user-visible batch. Crash-orphan recovery is not implemented.

PNG shares the window's single export slot and progress/cancel controls. Users can
switch tabs; the job remains owned by its source tab, and the window header can cancel
it from another tab. Closing waits for the job; a successful PNG export cannot authorize
discarding dirty PDF edits. Picker cancellation and failed PNG export retain the document.

## Verification

```powershell
. ./scripts/use-build-environment.ps1
./scripts/check.ps1
cargo run --release --locked -p pdf-smoke -- --png-smoke
./scripts/png-ui-smoke.ps1 -Profile release
```

Native checks cover dimensions/pixels, source/virtual rotations, multi-source order,
selection, preflight, limits, cancel/failure/races/source changes and cleanup. UI checks
use only owned synthetic fixtures, processes and native dialogs. See [testing](testing.md)
for runs and [portable audit](distribution-audit.md) for remaining release gates.
