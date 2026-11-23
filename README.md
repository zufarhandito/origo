# PDF Studio

For the next development chat, start with the [documentation index](docs/README.md)
and [current handoff](docs/handoff/PROJECT_HANDOFF.md). A [ready-to-copy prompt](docs/handoff/NEXT_CHAT_PROMPT.md)
records progress, verification, toolchain precautions and the latest Git checkpoint.
The [progress summary](docs/PROGRESS.md) maps completed work to commits and separates
the remaining backlog from release gates.
Read [agent instructions](AGENTS.md) and the [feature workflow](docs/feature-development.md)
for competitor-based research and Conventional Commit/push checkpoints after each feature.
The [desktop UI and multi-tab design](docs/plans/UI_REDESIGN_MULTITAB_PLAN.md) is implemented;
see [current multi-tab behavior](docs/multitab.md) and [organizer](docs/organizer.md).
Unified document tabs, offscreen-copy hardening and the measured eight-book process/
GPU profile are documented in [multi-tab hardening](docs/multitab-hardening.md).
Internal Windows portable candidates, runtime/notice inventory and package verification
are described in [distribution audit](docs/distribution-audit.md). Public release still
requires the [release gates](docs/release-checklist.md), including physical acceptance.

Native Windows PDF organizer following [the original blueprint](docs/blueprints/PDF_EDITOR_CODEX_BLUEPRINT.md).
**Alat → Ekspor PNG…** exports all or selected pages to a new folder at 150/300 DPI,
following the current page order and rotation. See [PDF to PNG](docs/png.md).
Current slice: **desktop UI and multi-tab**, retaining M3 safe Save As/Extract,
using GPUI Kit 0.9.1 and pinned PDFium/QPDF.
**Tampilan → Bandingkan dua PDF…** shows two open PDFs in a resizable Dock layout,
with one focused toolbar/organizer and shared window resource budgets. See
[multi-tab behavior](docs/multitab.md); reading-session restore is also available.
**Tampilan → Tampilan kedua PDF ini** opens two reading views of one shared document,
with independent navigation/search and shared page edits/history. Closing a pane keeps
the document; closing its tab retains the unsaved/export guard.
The first M4 feature is **Split PDF**, under **Alat → Split PDF**: split every N
pages or by manual ranges into a new output folder. All parts are validated before
the folder is published; Split does not mark edits saved. See [Split](docs/split.md).
**Alat → Optimize PDF (lossless)** optimizes the current page plan to a new PDF
and reports before/after sizes. It does not lower image quality or mark edits saved;
the result may stay the same size or grow. See [Optimize](docs/optimize.md).

- Continuous virtualized scrolling and clickable thumbnail sidebar.
- First/previous/next/last navigation and direct page-number entry.
- Zoom 50-400%, fit width/page, horizontal panning, light/dark appearance.
- Background rendering, bounded CPU caches and viewport-scoped GPU textures.
- Metadata-only document opening, stable page identities and source-change detection.
- Drag-select text, copy with Ctrl+C or Salin, and clear with Esc. Selection follows
  zoom/scroll and intrinsic page rotation.
  Single-click clears without selecting; double-click selects a sentence.
- Ctrl+F searches the active PDF with phrase matching, default case-insensitive,
  occurrence highlights/counts and previous/next navigation. The Find bar lives
  below the reading toolbar; see [text search](docs/search.md).
- Book viewer follow-up: up to 1024 pages, reused native document sessions and lightweight
  previews before larger rasters. The input-file limit remains 128 MiB.
- Local Indonesian/English OCR supports active pages, resumable document batches,
  the Windows notification area and searchable copies. See [OCR](docs/ocr.md).

Page selection, reorder, virtual rotate/delete, import and bounded Undo/Redo are implemented.
Use **Simpan sebagai** or **Ctrl+Shift+S** to save the current page plan to a new PDF.
In **Atur halaman**, **Ekstrak** saves selected pages in their workspace order.
Existing files are never overwritten; source PDFs remain untouched. Save As marks the
exported current revision clean, while Extract keeps the workspace's saved status.
Opening another PDF creates or focuses a tab. Closing a dirty tab/window asks to save,
discard or cancel; cancelled/failed Open preserves existing tabs. See
[unsaved changes](docs/unsaved-changes.md).
See [organizer controls and limits](docs/organizer.md).
See [testing](docs/testing.md), [compatibility](docs/compatibility.md) and
[architecture](docs/architecture.md) for evidence and remaining limits.
## Export and conversion direction

The previous **Konversi** catalog reserves a direction for PDF-to-PNG/JPEG, Word, text, Excel,
PowerPoint, images-to-PDF and Office-to-PDF. Every card currently says **Akan datang**:
there is no converter, upload or format-selection execution behind these cards.
It is excluded from the new desktop navigation. PNG export is available separately
under **Alat → Ekspor PNG…**; other format converters remain deferred.

[M3 safe PDF Save As/Extract](docs/m3-safe-export-plan.md) is implemented for ordinary,
unencrypted PDFs passing conservative structural preflight. Complex documents such as
forms, signatures, annotations and tagged PDFs are blocked for export; reading
remains independent. Ordinary local bookmarks can be exported with explicit
**Simpan tanpa bookmark** consent: navigation bookmarks are omitted from the output.
Info/XMP metadata also requires consent because it may be lost. Complex bookmark
actions remain blocked. See [export policy](docs/adr/0003-export-preservation-policy.md).
Whole-source Extract has a narrow preservation path: selecting every page of one PDF
without reorder/rotation copies its snapshot byte-for-byte, including supported
empty-form/tagged documents. Active forms/XFA and structural edits remain restricted.
Partial single-source tagged Extract now offers **Simpan tanpa tag**: accessibility
structure is omitted only after confirmation, while page appearance and supported
passive Ink annotations are retained. Original PDFs remain unchanged.
The [conversion roadmap](docs/conversion-roadmap.md) separates raster export, text,
Office layout reconstruction and OCR. It does not assume that the same engine can
provide every format or that editable Word output will exactly reproduce the PDF.

## Windows setup

Install Visual Studio 2022 Build Tools with **Desktop development with C++**, x64 MSVC
compiler, Windows 10/11 SDK, and CMake tools. Use its Developer PowerShell. Install Rust
through rustup; `rust-toolchain.toml` pins 1.98.1 with rustfmt/clippy. Git and tar must be available.

On this working machine Rust is already installed in ignored `.tools`; use the local
environment in each PowerShell session (this does not install MSVC):

```powershell
. ./scripts/use-local-rust.ps1
```

If using the Build Tools installed for this workspace on D:, activate both toolchains:

```powershell
. ./scripts/use-build-environment.ps1
```

This also keeps Cargo output and build temporary files on D:.

The Microsoft Build Tools installation is in `.tools/vs2022`; Windows SDK data is in
`.tools/windows-sdk`, with a junction from its registered Windows Kits path on C:.
Some Microsoft installer/runtime and servicing-cache files remain on C:. **Do not delete
or move `.tools`/this workspace without uninstalling or reconfiguring these toolchains.**

Prepare fixture and exact pinned binaries (fetch once into a fresh sidecars directory):

```powershell
./scripts/new-fixture.ps1
./scripts/fetch-sidecars.ps1
./scripts/test-sidecars.ps1
```

The download script checks genuine archive hashes and keeps DLLs and upstream notices.
No network access is required at application runtime. `test-sidecars.ps1` independently
checks both native binaries using a PowerShell/C# test harness, even before MSVC is
available. It does not validate the Rust integration or GPUI.

For OCR, build the local Tesseract runtime with `./scripts/build-ocr-sidecar.ps1`.
See [OCR setup and limits](docs/ocr.md).

## Verify and run

Double-click `run.cmd` to launch the staged release application without a build shell.
It falls back to debug when release is unavailable and forwards optional PDF arguments.
To rebuild on this machine:

```powershell
. ./scripts/use-build-environment.ps1
./scripts/check.ps1
./scripts/smoke-test.ps1
./scripts/run-desktop.ps1 -Pdf ./tests/fixtures/hundred-pages.pdf
```

The app opens in reading mode with a fixed toolbar. Choose **Alat → Atur halaman**
or **Panel alat** to select, rotate, delete, move or import pages in the right panel.
Kembali membaca clears page selection. Undo/Redo are available in both modes.
Theme choices live under **Tampilan**; Split and Optimize live under **Alat**. The Copy
icon appears when text is selected. See [organizer controls](docs/organizer.md).

Open with Buka PDF or Ctrl+O. PgUp/PgDn and Left/Right move between pages;
Ctrl+Home/End select the first/last page. Ctrl+G focuses the page field; Enter
jumps to that page. Ctrl+plus/minus adjusts zoom, Ctrl+0 fits the page, Ctrl+1 fits
width. The percentage opens fit options and zoom presets. Scroll vertically across pages and
pan horizontally at higher zoom. The sidebar and theme can be toggled.
Over the PDF viewport, spread/pinch two fingers on a supported precision trackpad
to zoom in/out; Ctrl+scroll also zooms. Ordinary two-finger scrolling still pans the
document. Gesture zoom preserves the reading offset instead of returning to page top
and stays within 50-400%. Trackpad/driver delivery of native pinch events varies.

The real native smoke verifies QPDF order/rotation and PDFium rendering, then M1
100-page metadata, mixed geometry, resource limits and changed-source rejection.
It also exercises production M3 preflight, Save As/Extract and safe-output failures.
For multi-tab, Save As and unsaved-close UI acceptance, run:

```powershell
cargo run --locked -p pdf-smoke -- --write-export-fixtures
./scripts/tabs-ui-smoke.ps1 -Profile release
./scripts/m3-ui-smoke.ps1 -Profile release
```

M3's original single-workspace assertions are archived and mapped to current tab/
window behavior in [the coverage map](docs/m3-ui-coverage.md). Device/input acceptance
and the remaining human hardware checks are tracked in [the device matrix](docs/device-acceptance.md).

Original documents are read-only. For interactive Windows GPU acceptance:

```powershell
./scripts/m1-ui-smoke.ps1 -Profile release -Stress
```

This requires already-built executables and sidecars staged beside them. It targets
only its own synthetic viewer using UIAutomation/HWND messages, leaves that window
open and writes evidence under ignored `.research`. Physical mixed-monitor DPI and
clean-machine portable packaging remain release gates.

Optional numeric diagnostics (parent directory must exist):

```powershell
./target/debug/pdf-studio.exe ./tests/fixtures/hundred-pages.pdf --metrics ./.research/metrics.json
```

Metrics contain counters, sizes and fixed operation states, no document paths or content. See
[ADR-0005](docs/adr/0005-m1-viewer.md) for cache and scheduler decisions.
Text extraction and selection details are in [text-selection.md](docs/text-selection.md).

Generated book benchmarks (100/300/500/1000 vector and image-only pages):

```powershell
./scripts/benchmark-books.ps1
./scripts/m1-ui-smoke.ps1 -Profile release -Fixture .research/book-benchmark/simulated-scan-1000.pdf -ExpectedPages 1000 -Stress
```

See [book viewer measurements and limits](docs/books.md). Generated scan-like images
do not establish compatibility or performance for arbitrary real scanned books.
