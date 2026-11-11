# Verification log

Results below were accumulated during the implementation sessions of 9–10 November
2026. See [PROJECT_HANDOFF.md](../PROJECT_HANDOFF.md), recorded 11 November 2026,
for the consolidated status. Earlier sections retain historical test counts and
“left open” observations; these do not establish current process state. No suites
were rerun for the documentation-only handoff.

Current implementation: M1 viewer. Earlier M0 and GPUI Kit rendering checks were
superseded by the checks below; broader product-release acceptance remains pending.

## Selection click behavior (newest results)

Single click no longer selects one character. Drag activation requires movement to
another character beyond 4 logical pixels. Double click selects a sentence, falling
back to the extracted line for unpunctuated labels. Whole-sentence drag extension is
wired; punctuation/decimal/wrapped-line logic has unit coverage.

Workspace tests: 16 passed. Workspace/all-target denied-warning Clippy and formatting
passed; desktop debug/release builds updated. The headed release selection test passed
single click, jitter, double click/copy, pressed-drag hover resilience, reverse drag,
zoom reuse, four source rotations, cross-page copy and clearing. Evidence:
.research/text-ui-9beba3501152498bad944cc76a8d252f. Existing clipboard formats restored.
The test viewer is left open. Sentence segmentation is heuristic; abbreviations and
complex extracted reading order remain documented limitations.

## Text selection follow-up (newest results)

Workspace tests now total 15, all passed; denied-warning workspace/all-target Clippy,
formatting and debug/release builds passed. Real release native smoke includes text
content/newlines, four rotations and raster alignment, empty scan and changed-source
rejection. Headed release selection/copy/zoom/cross-page checks passed; see
[text-selection.md](text-selection.md) for exact scope, evidence and manual gates.

## Trackpad zoom follow-up

The pinned GPUI Windows source exposes native PinchEvent through Direct Manipulation.
The viewport now consumes those events and Ctrl+scroll (capture phase); unmodified
scroll remains navigation. Fractional accumulation, limits, reset/invalid input and
vertical reading-anchor math have unit coverage. Workspace tests passed (12 total),
denied-warning Clippy passed, and current debug/release desktop builds succeeded.
The anchor padding was corrected to include both the 24px label and 8px gap; the
application tests passed again after that correction.

Headed release regression passed: empty startup, lazy rendering, page 100/boundaries,
thumbnail activation, ordinary continuous wheel scrolling, 50-400% button zoom and
resize. Evidence: .research/m1-ui-e96429e64ad84083bbd9b33afaac9fd2. Latest viewer left open.

Physical trackpad pinch and Ctrl+scroll acceptance remain manual: automated checks
do not inject global keyboard input or claim that hardware gestures were exercised.

## Startup fix (newest check)

The no-argument debug executable reproduced a main-thread stack overflow before its
empty window appeared. Document-only release tests had missed this startup path.
The desktop build script now reserves 8 MiB Windows MSVC main-thread stack (committed
on demand). Both debug/release PE headers were checked for this reserve, and both
executables rebuilt. Formatting and desktop all-target Clippy with denied warnings passed.

The headed debug UI test now first opens/closes the empty viewer, then runs its normal
PDF/navigation/zoom/resize checks: PASS. Evidence:
.research/m1-ui-0b2ce65ff7a14524b79e70ac9a34e479. The UI script retains this empty-start
regression check for future runs. All PowerShell scripts parsed successfully.

run.cmd now prefers release, falls back to debug, sets the project working directory
and forwards arguments. Invoking the actual batch file without arguments from a different
working directory produced a responsive release window: PASS. That release window is
left open. Existing book stress results below remain applicable to viewer behavior.

## Book follow-up (newest results)

- Current debug/release builds, formatting, denied-warning Clippy and workspace tests:
  PASS (now 10 unit tests, including preview key isolation).
- Current real release PDFium/QPDF smoke: PASS.
- Eight generated native book cases (100/300/500/1000 vector and image-only pages),
  576 renders, session reuse/recycle/idle/source protection and admission limits: PASS.
- Current release UI with a 1000-page image-only fixture: PASS, all navigation/zoom/
  resize checks and two passes through 100 evenly-spaced destinations each.
- See [books.md](books.md) for exact measurements, reproducible commands and limitations.
  The newer UI evidence uses an isolated fixture copy; its release viewer is left open.

## Original M1 checks before the book follow-up

- Rust 1.98.1, Windows x64 MSVC, GPUI Kit 0.9.1, PDFium Chromium8125, QPDF 12.4.2.
- cargo fmt --all; Clippy workspace/all targets with denied warnings: PASS.
- cargo test --locked --workspace: PASS, nine tests, none ignored. Covers stable
  identities/geometry, metadata-only opening, byte-LRU ownership/eviction, scheduler
  priority/generation/cap, fit/DPR math, navigation bounds and QPDF argument tokens.
- Current debug executable and optimized release workspace build: PASS.
- Real release pdf-smoke.exe: PASS, QPDF selection/order/rotation, pixel identity,
  Unicode/spaces/ampersand paths and unchanged source bytes.
- M1 native integration: PASS, 100-page metadata/workspace, mixed sizes and rotation,
  selected first/last/middle renders, max dimensions/pixels, configured page limits,
  invalid index and modified-source rejection. Synthetic native probe took 73 ms in
  the release run; this is not a general PDF performance claim.
- Headed debug GPU UI: PASS, lazy startup (3 page/4 thumbnail attempts), jump to 100,
  boundary disabled state, thumbnail activation, continuous wheel scrolling, rapid
  50-400% zoom, stale-result rejection and fit recomputation on resize.
- Two debug passes requesting all 100 pages: PASS, bounded 128/16 MiB CPU caches.
  Working set 296.8/302.9 MiB after each pass; final 295.5 MiB, peak 303.8 MiB.
  Final shared GPU counter about 48 MiB, zero GPU-release errors, empty work queue.
  Shared GPU accounting can overlap working set; do not add these figures.
- Changed-source UI probe on its own copied synthetic file: pageCount became zero,
  both caches cleared and rendering stopped. The original fixture was untouched.
- Final headed release UI and two-pass 100-page stress: PASS, including resize.
  Working set after the two passes was 303.6/314.7 MiB; final 306.1 MiB,
  peak 318.9 MiB. Both CPU caches stayed within 128/16 MiB. Evidence:
  .research/m1-ui-093513d7c18f42f3888d2442dedf797f. The release viewer is left open.

Machine: Intel Core i7-13620H, about 15.7 GiB RAM, Windows build 10.0.26300.
Synthetic debug stress evidence: ignored
.research/m1-ui-dd2f60912f2f4aeb98f286e19a7c20b1.

GPU retention was measured during implementation: CPU-only eviction left window
atlas textures resident. Explicit viewport/neighbor GPU lifetimes and deferred releases
with the current Window resolved the growth. See ADR-0005.

## Reproduce

Activate the local toolchains and run scripts/check.ps1 and scripts/smoke-test.ps1.
The native smoke never silently skips missing sidecars. Generate synthetic fixtures
with cargo run --locked -p pdf-smoke -- --write-fixtures. The committed fixtures contain
only generated colored pages and labels, including 100-page and mixed-geometry PDFs.

For an interactive Windows desktop with GPU, built binaries and staged sidecars:

```powershell
./scripts/m1-ui-smoke.ps1 -Profile release -Stress
```

The script targets its own synthetic process via UIAutomation and HWND messages,
never global keyboard injection. It verifies original fixture hashes and leaves its
viewer open. Evidence is numeric JSON and captures of generated fixture pages in
ignored .research. Optional --metrics writes numeric counters asynchronously; it
contains no document path/content. Its parent directory must already exist.

An early release UI run exposed test races at disabled zoom boundaries and a resize
to an already-saved size. The script handles disabled Invoke and selects a different
resize width. These failed runs are not counted as acceptance passes.

## Remaining acceptance

Physical mixed-monitor 100/150/200% DPI is not tested; pure layout/raster DPR math is
covered at 1x/2x. M1 key bindings are wired, but the current UI test exercises native
controls through UIAutomation rather than OS keyboard injection. Earlier M0 keyboard
checks do not substitute for the complete M1 keyboard/accessibility matrix.

Password/complex/hostile PDF breadth, hard native-call cancellation/process containment,
form/signature export preflight, clean-machine portable runtime/license audit and
packaging remain pending. The CI workflow is written but has not run remotely;
headed GPU acceptance is local and is not claimed on GitHub hosted runners.


## M2 organizer - 2026-11-11

Implemented multi-source virtual commands, page selection, reorder/drag, rotate/delete,
import, bounded Undo/Redo, revision/dirty and empty states. See organizer.md for exact
semantics and limitations. Save As/Extract remain M3; sources are never rewritten.

Final checks actually run locally:

- `scripts/check.ps1`: fmt, workspace/all-target Clippy with denied warnings,
  **21 unit tests**, and workspace release build passed.
- `cargo run --locked --offline -p pdf-smoke`: real QPDF/PDFium smoke passed;
  metadata/render/resource/source-change checks passed. Text checks now also cover
  **16 intrinsic/virtual rotation combinations** against actual glyph raster ink.
- `scripts/m2-ui-smoke.ps1 -Profile debug`: native import and organizer UI passed
  before drag coverage was added; evidence `.research/m2-ui-4da7cfa4f77b4ec9a997c78cff6f1176`.
- `scripts/m2-ui-smoke.ps1 -Profile release`: drag/drop, rotate, move, delete,
  Undo/Redo, delete-all/restore, native import, two-source page plan and unchanged
  source hashes passed. Evidence `.research/m2-ui-31aaa601d905498f8c39cdda04d6f371`.
- `scripts/m1-ui-smoke.ps1 -Profile release`: empty startup, lazy 100-page viewing,
  navigation, scrolling, rapid zoom and resize passed. Evidence
  `.research/m1-ui-41a98684860c4b2895595eb58e49e7ce`.
- `scripts/text-selection-smoke.ps1 -Profile release`: click/jitter, double-click,
  forward/reverse, hover while pressed, four source rotations, zoom reuse, cross-page
  copy, clear, cache bound and unchanged source hash passed. Evidence
  `.research/text-ui-3422b39a1186498d8a8443ffd7de6bd7`.

Harness corrections during this session: hidden launch had no usable MainWindowHandle;
visible headed test launch is necessary. UIA import-button invocation can block on the
modal picker. HWND-posted click avoids this. UIA filename traversal incorrectly found
Search Box and Open-button lookup could target another control, causing a wrong fixture
to be selected. Final harness targets native filename Edit ID 1148 and native button ID 1,
and verifies the resulting exact page mapping; failed attempts are not passing evidence.
Only synthetic fixture copies and the test's own window/process handles were used.

The release UI runs preceded the final additional seeded mixed-operation test and
smoke output wording change; production behavior did not change afterward. Final
quality checks and native smoke were rerun after those changes. Test windows were
closed; an earlier hidden/stuck test process required targeted cleanup. No installation,
source rewrite, sidecar upgrade, remote CI run or 2000+ page benchmark was performed.
Physical modifier keys, multi-selection drag/cancel matrix and physical mixed-DPI
acceptance remain manual checks documented in organizer.md.

## Compact Baca / Atur halaman UX - 2026-11-11

Implemented the approved grouped toolbar plan. Header is 44 logical pixels, reading
bar 40, and contextual organizer bar 40 only in Atur halaman. Baca is the default;
mode changes are presentation-only. Theme/import live under Lainnya; Enter replaces
Ke; zoom presets/fit choices live under Zoom; copy appears only for text selection.
Navigation shortcuts and page commands retain their previous domain semantics.

Final local verification:

- `scripts/check.ps1`: fmt, workspace/all-target Clippy with denied warnings,
  **21 unit tests**, and workspace release build passed.
- Native smoke passed earlier in this UX session: pinned QPDF/PDFium, order/rotation,
  resources/source changes and 16 source/virtual rotation glyph alignments. No engine
  or domain behavior changed in the subsequent presentation fixes.
- `scripts/chrome-ui-smoke.ps1 -Profile release` passed: Baca/Atur, both themes,
  long filename, widths 860/1000/1200/1600, navigation/zoom on one non-overlapping row,
  focused page draft surviving scroll and Enter navigation without dirtying content.
  Evidence: `.research/chrome-ui-855a944943694c628b61aa806c636800` (16 mode/theme/width
  screenshots, plus initial views and acceptance.json). Actual test DPI was **96/100%**.
- `scripts/m2-ui-smoke.ps1 -Profile release` passed: mode/selection semantics,
  drag and Escape cancellation without an edit, rotate/move/delete, history,
  empty/restore, native import, two-source mapping and unchanged source hashes.
  Evidence: `.research/m2-ui-3405e25e53f14a6bb6e8b0022b2f3074`.
- `scripts/m1-ui-smoke.ps1 -Profile release` passed: empty startup, lazy 100-page
  opening, Enter navigation, boundary buttons/thumbnail, continuous scrolling,
  rapid 50-400% zoom, resize and unchanged sources.
  Evidence: `.research/m1-ui-d9efab4b715a4d198d96436b3f475e56`.
- `scripts/text-selection-smoke.ps1 -Profile release` passed: single-click/jitter,
  double-click, forward/reverse drag, zoom reuse, mode switch preserving selection,
  all four intrinsic rotations, cross-page copy, clear, bounded cache and source hashes.
  Clipboard was restored. Evidence: `.research/text-ui-383da9ad9f65460d956d07b54e5761a5`.
- Debug viewer regression also passed after the clipping correction:
  `.research/m1-ui-92ea059dfe66490687492eff48630c5a`.

Corrections discovered during validation:

- Raw Button.hover duplicates the Kit's hover style and panics; use the supported
  ButtonCustomVariant instead. Earlier black captures/missing controls were failed
  render evidence, not successful acceptance screenshots.
- Centered navigation exposed an existing text-overlay bug: a scrolled-off page's
  global mouse listener could overlap the toolbar and swallow clicks. Restricting
  down/move to the paint-time content mask fixes this. First/previous navigation,
  scrolling and text-selection regressions passed after the fix. Unrestricted global
  mouse-up is retained. Navigation target math and domain commands were unchanged.
- Page-number projection is deferred outside render, respects focused drafts and
  refreshes on Blur. UIA readiness is awaited; key-down tests use HWND PostMessage
  because GPUI pretranslates queued key messages. Menus without InvokePattern use
  targeted HWND-relative clicks; no global SendKeys/input was introduced.
- The old release executable was in use. Its loaded file was renamed to a unique
  previous-version filename inside target/release, preserving the user's running
  session while the new default executable was built. It was not force-closed.

Physical 150/200% and mixed-monitor DPI checks remain manual; the width/theme matrix
above is not a claim that those physical configurations were tested. Save As remains
M3 and edits remain session-only. Documentation and PROJECT_HANDOFF.md were updated.
