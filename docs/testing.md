# Latest verification - unified tabs, hardening and memory profile, 16 November 2026

- Final hardening re-run also passed after the shared harness was strengthened to
  identify its own PID by executable path and unique fixture GUID:
  `.research/tabs-hardening-0d2ae36c34e84b52a6ec5e73e38ed533`.
- Additional three-round high-zoom profile passed with 8 loaded books, 400% zoom/fit
  on both source kinds each round, 61 samples and the same resource guards:
  `.research/tabs-memory-723a63a16ec146108b0826cc2c276314`.
  Peak working set **509.99 MiB**, sampled private maximum **410.25 MiB**, GPU shared
  maximum **185.26 MiB**, warmed private growth **0.875 MiB**, GPU growth **0 MiB**.
  Page-cache sampled maximum **120.31 MiB** stayed below 128 MiB. Inactive residency,
  GPU release errors and empty-idle native documents remained zero. Driver pools
  retain a warmed baseline; handle counts 744/758/764 stayed within the documented
  guard, not claimed perfectly constant or proof against unbounded-session leaks.
- Final long-filename containment checks passed in both themes/four widths;
  × stays inside its TabItem: `.research/chrome-ui-1ccd5db683e141a99d4eaafbde08f8d2`.
  Release M1 empty startup, lazy 100-page startup, boundary navigation, thumbnail,
  continuous scroll, rapid 50–400% zoom/stale rejection and resize/fit passed:
  `.research/m1-ui-49d288ac2dc04c3cbdfa1e3589cab7b1`. Its owned window was closed normally.
- Release Split and Optimize regressions also passed on this implementation:
  `.research/split-ui-1074a25c42ea4325b5a80c82eea3373a`,
  `.research/optimize-ui-7654370ba1ff4f29bd57a3680dbc7f29`.

- `scripts/check.ps1` passed fmt, workspace/all-target Clippy with denied warnings,
  **47 unit tests** and release workspace build. Debug desktop build also passed.
  New copy-buffer coverage checks Unicode byte limits, separators and rejection
  without partial append. Full native M1/text/M3/tagged/whole-source/Split/Optimize
  smoke passed, including source identity, consent/blockers, failure/cancel/races
  and owned transaction cleanup.
- Unified GPUI Kit folder tabs put the × suffix inside the TabItem. Release hardening
  checked actual accessibility bounds and targeted HWND close clicks, including
  closing an inactive clean tab without switching the active document.
- Release hardening passed offscreen selected-text copy after cache eviction,
  repeated fast switch/close, isolated source change/reopen, Save As completion
  while another tab is active, cancellation and the cross-tab export-slot/dirty-close
  cancellation race. Clipboard was materialized/restored and fixtures were owned.
  Evidence: `.research/tabs-hardening-acd78bfb87b146d49bc7612a780f88a3`.
  The earlier successful debug run was
  `.research/tabs-hardening-ffa63e19c38f423ea62985ddbcf08f5c`.
  The integrated tab/list-control screenshot was visually inspected.
- Release memory profile passed **3 identical rounds / 8 sources x 1000 pages**,
  four generated vector and four simulated scan books, with two sweeps per round,
  close-to-empty, native idle timeout and unchanged hashes. 55 samples recorded
  process private/working set, handles/threads/GDI/User objects and real Windows GPU
  process-memory counters, filtered to the test PID.
  Evidence: `.research/tabs-memory-27c7c4d8de0d4a1dab31cbfe93da46cb`.
  Peak working set **209.53 MiB**, sampled maximum private **162.81 MiB**, GPU shared
  maximum **70.50 MiB**, dedicated **0 MiB**. Warmed endpoint private growth was
  **0.438 MiB**, GPU growth **-0.035 MiB**; handles **785 -> 786 -> 792**, GDI/User
  objects stable **77/37**. CPU caches/GPU residency/native documents were all zero
  at every empty-idle endpoint; native documents never exceeded one and GPU release
  errors were zero. Driver/allocator baseline is retained, not claimed as zero.
  See [protocol and limits](multitab-hardening.md); this is not a universal leak,
  physical-memory cap or PDFium-only heap measurement.
- Existing release multi-tab suite passed:
  `.research/tabs-ui-edd0fd023b424750bb7ecb6363274f7f`.
  Release light/dark/four-width drawer/Enter/hit-testing/Esc chrome passed:
  `.research/chrome-ui-6197f6a4142c408ba26eeb3596c2d4f1`.
- Release text selection passed single/double-click, jitter, forward/reverse drag,
  zoom coordinate reuse, reader/organizer selection preservation, all four intrinsic
  rotations, cross-page copy, clear, byte budget and source hashes:
  `.research/text-ui-27903420cb0249b4996e1959bef876cb`.
  Its historical mode labels were updated to Panel alat/Kembali membaca; the removed
  Konversi placeholder is now asserted absent rather than treated as navigable.
- An intermediate Windows PowerShell run interpreted a BOM-less literal bullet as
  ANSI in a test selector; the harness now constructs that marker with its Unicode
  code point. It was a failed intermediate run, not acceptance. The owned pending
  job was cancelled and its synthetic viewer closed normally before retrying.
- No user PDF, global input, remote CI/fetch/push or portable release claim. Existing
  user process was preserved while the old release EXE was renamed within target
  for rebuilding. Broader corpus/duration, physical mixed-DPI/trackpad and native
  isolation remain separate gates. The original M3 single-workspace UI suite remains
  intact and was not claimed as migrated or passing.

---
# Latest verification - desktop UI and multi-tab, 16 November 2026

- `scripts/check.ps1` passed fmt, denied-warning workspace/all-target Clippy,
  **46 unit tests**, and release workspace build. Debug desktop/workspace builds
  also passed during implementation. No domain/export preservation contract changed.
- Final full native smoke passed M1/text/M3/tagged and whole-source Extract/Split/
  Optimize, including consent/blockers, source/raster/text identity, no-clobber races,
  cancellation/failure and owned cleanup.
- Final release multi-tab UI passed: last-tab close to empty state and reopen,
  isolated edit/history/viewport, duplicate Open preserving state, mixed valid/corrupt
  multi-file Open, capacity refusal and eight tabs, Save As cleaning only its owner,
  Optimize retaining dirty, cancelled save picker, discard-then-cancel retaining all
  tabs, save-then-cancel retaining earlier clean receipt, final window close,
  active cache bounds, zero GPU-release errors and unchanged source hashes.
  Evidence: `.research/tabs-ui-81d435fb01574f91a955fd2c061a7fd2`.
- Debug chrome passed light/dark at 860/1000/1200/1600 px, non-overlapping fixed
  toolbar, focused page draft/Enter, actual HWND click on drawer Rotate and Esc
  returning to reading. Evidence: `.research/chrome-ui-7e012d11ea8c4e7693594601e6b7eb00`.
  Physical DPI was 96; this is not mixed-monitor acceptance. Dark narrow-drawer and
  light eight-tab screenshots were visually inspected.
- Final release chrome, including actual drawer hit-testing/Esc, passed with settled
  mode/accessibility checks: `.research/chrome-ui-ce32122af63e4c07b5e16faa37d53137`.
  Final release Split passed validation/preview, per-N/manual, native folder picker,
  order/rotation, collisions/cancel, batch loss consent/Ink, close-wait, hashes and
  cleanup: `.research/split-ui-b5a4775bedaa4a77aa63704914828e27`.
- Final release Optimize passed picker cancellation, edited multi-source plan,
  exact size reporting, unchanged dirty/history, loss consent, close-wait guard,
  blockers/JPEG, hashes and cleanup:
  `.research/optimize-ui-90c0f46c82214b50993f2d404ac13a0d`.
- Debug M2 UI passed drag, rotate/move/delete, empty/history, native import,
  multi-source order and unchanged-source checks:
  `.research/m2-ui-cec52a4635594f499cb8429698b62d6b`.
  Debug M1 passed no-argument startup/close, lazy 100-page startup, boundary jump,
  thumbnail/continuous scroll, rapid 50–400% zoom with stale-result rejection and
  resize/fit: `.research/m1-ui-503dddfaf2734145be1be4f51a04cb69`.
  Empty desktop chrome also passed:
  `.research/chrome-ui-139c3484698e421786381f09e560bdc5`.
- Debug M2 drag/rotate/move/delete/empty/history/native import/multi-source/source
  hashes passed: `.research/m2-ui-cec52a4635594f499cb8429698b62d6b`. Debug M1 empty
  startup/100-page navigation/thumbnail/scroll/rapid 50-400% zoom/resize passed:
  `.research/m1-ui-503dddfaf2734145be1be4f51a04cb69`. These ran before the final
  drawer hit-test guard; the added guard was then checked by the chrome suite above.
- Final release Chrome/Split/Optimize evidence is recorded in the follow-up below.
- Intermediate UI runs found a close guard blocking the shell's own close request,
  duplicate control labels confusing test actions, and stale UIAutomation projections.
  The final code fixes the guard/labels. Chrome acceptance waits for both numeric
  mode state and accessibility projection instead of assuming a fixed delay.
  Mouse-based save activation during dialog animation was replaced with the existing
  UIAutomation Invoke pattern. Failed intermediate attempts are not acceptance.
- The old `m3-ui-smoke.ps1` remains intact, with historical single-workspace
  Open-replacement assertions. It was not claimed as passing. Automatic approval
  review rejected replacing that suite because it would remove existing coverage;
  the safe alternative adds `tabs-ui-smoke.ps1` and preserves the older test.
- Original user viewer was preserved. To rebuild release without closing it, its
  old executable was moved within `target/release` to `pdf-studio.previous-*.exe`;
  `run.cmd` starts the newly built release. Owned passing test windows were closed.
- No push/fetch/remote CI, real user PDF, converter, OCR, portable certification or
  physical mixed-DPI/trackpad claim. Full native/GPU/process memory profiling and
  broader corpus/accessibility acceptance remain release gates. CPU cache bounds
  are not a total process-memory limit. See [multi-tab](multitab.md).

---
# Latest verification - lossless Optimize, 16 November 2026

- scripts/check.ps1 passed formatting, workspace/all-target denied-warning Clippy,
  **45 unit tests** and release workspace build. Workspace debug build also passed.
  New tests cover optimized capture/receipt never marking saved, smaller/equal/larger
  results and zero-size rejection before commit, plus exact lossless-only QPDF options.
- Full debug native smoke passed existing M1/text/M3/Split and new Optimize checks:
  multi-source edited order/rotations/deletion, exact same-plan before/after byte sizes,
  source text and exact 512px RGBA versus an ordinary export of the same plan; blank,
  simulated scan and JPEG cases; original JPEG encoded bytes retained; unchanged
  dirty revision; metadata/bookmark consent and blocked forms/signatures/tags/active
  content; cancellation/failure after writing; destination race and changed sources;
  owned baseline/staging cleanup. No new general performance/fidelity claim.
- Release Optimize UI passed cancelled picker, transformed multi-source page plan,
  exact size receipt and accessible result text, Undo/Redo, metadata refusal/acceptance,
  Close waiting then retaining unsaved edits, conservative blockers, blank/JPEG output,
  unchanged source hashes and cleanup. Evidence:
  .research/optimize-ui-5b6e871cd8b14f77bdbb69626adf3a7a.
  optimize-result.png was visually inspected. A valid tiny blank PDF grew from its
  431-byte ordinary-export baseline to 521 bytes; the UI correctly reported growth.
  Small JPEG output also grew (1571 -> 1615 bytes), while the edited multi-source case
  shrank (3019 -> 1892 bytes). These are generated fixtures, not reduction guarantees.
- An intermediate blank fixture lacked a Resources dictionary, so clean QPDF preflight
  correctly blocked repair warnings. The fixture was corrected; production preflight
  was not loosened. The earlier interrupted suite is not counted as acceptance.
- Final regressions passed on the Optimize build: release M3/unsaved guard
  (.research/m3-ui-612af32f2ad1469cbf4aef08de67390b), release Split
  (.research/split-ui-1fc72d1242b74d978835dd0613cab04a), and debug M1 empty startup,
  100-page navigation/thumbnail/scroll, rapid 50-400% zoom and resize
  (.research/m1-ui-93de63cbfd69453991750c92596dc122). The owned final viewer was
  closed normally afterward; existing user sessions were preserved.
- No new dependency/runtime/sidecar or real user PDF. The tiny JPEG is a generated
  color pattern (scripts/new-optimize-fixture.ps1), committed for repeatable tests.
  No portable release, remote CI result, overwrite, image downsampling, OCR or broader
  form/tag/signature preservation is claimed. See optimize.md for current limits.

---
# Latest verification - Split PDF, 15 November 2026

- Final scripts/check.ps1 passed formatting, workspace/all-target Clippy with denied
  warnings, **42 unit tests** and the release workspace build. The three new tests
  cover per-N/manual parsing and invalid input, folder-name safety, and actual Windows
  directory no-replace behavior for existing empty directories/files and new targets.
  The workspace debug build also passed.
- Full debug native smoke passed existing M1/text/M3 checks plus Split: multi-source
  order/rotations/text/sample raster identity; per-N/manual groups; progress; unchanged
  dirty baseline; refusal/acceptance of tag-loss consent; passive Ink/page appearance;
  byte-identical whole-source empty-form/tagged copy; a blocked later signed part;
  unused-source exclusion; existing destination/race refusal; changed originals;
  injected failure on the second writer call; cancellation after the first validated
  part; no partial final folder and owned staging cleanup.
- Release Split UI passed validation/preview, per-N and manual groups, real native
  folder picker, deterministic names, QPDF count/rotation checks, unchanged workspace,
  existing-output hashes, cancelled picker/consent, accepted batch tag loss/Ink,
  Close waiting for Split followed by unsaved guard, source hashes and staging cleanup.
  Evidence: .research/split-ui-16cd253457374468a719b5847afcfb4e.
  split-dialog.png was visually inspected. Only owned synthetic fixtures/windows used.
- Release M3 + unsaved-guard regression passed on the same final build:
  .research/m3-ui-cb2003c199b84fffada56484845c959a. Final debug M1 UI passed empty
  startup/clean close, lazy startup, 100-page navigation, thumbnail/scroll, rapid zoom
  and resize: .research/m1-ui-1c2c841922ad4edea931c984d30e105f. Its fixture window
  was closed normally afterward; the existing user viewer remained untouched.
- Early checks exposed replacing std::fs::rename behavior, retained PDFium output
  handles preventing directory moves, an error cleared on input blur, and the native
  folder filename control differing from the file picker (Edit 1152). These were fixed
  and final checks above passed. A negative fixture initially used an empty AcroForm,
  which is correctly allowed by whole-source copy; the blocked-part test now uses a
  synthetic signature structure. Intermediate attempts are not passing acceptance.
- Added pinned Windows-only atomicwrites 0.4.4 and its MIT notice; no new toolchain,
  native sidecar or external conversion runtime. Viewer session reuse is unchanged;
  inspect_output validation releases its document before publication/cleanup.
- No real user PDFs, remote CI, push, portable release, large-batch performance claim,
  process isolation or crash-recovery claim. See split.md for current limits.

---
# Latest verification - unsaved-change guards, 15 November 2026

- Added a single desktop Open/Close transition with candidate inspection, a final
  fingerprint recheck, explicit Save As/discard/cancel and exact-receipt continuation.
  Page mutations/history/import/additional exports are locked while leaving. No new
  domain/export interfaces, dependencies or broader PDF preservation claims.
- Formatting, workspace/all-target Clippy with denied warnings and **39 unit tests**
  passed. The two new tests cover export completion/current-receipt requirements and
  cancellation tokens. Existing source/snapshot/receipt/no-clobber tests remain green.
  Final scripts/check.ps1 and workspace debug build passed, producing both profiles.
- Full debug native smoke passed after generating synthetic export fixtures: M1,
  text/16 rotations, M3, tagged extraction and byte-identical whole-source extraction.
- Release M3 UI passed cancellation/invalid Open, candidate changed after inspection,
  Batal focus, Escape, repeated close, disabled mutations, empty-plan save disabled,
  cancelled Save As, successful save-before-Open, explicit discard/close, pending
  export wait/cancel-intent, Extract remaining dirty, failed save keeping the old
  workspace, consent refusal and successful save-before-close. Existing organizer,
  export/popup/select-copy/source-hash/staging regressions passed too. Evidence:
  .research/m3-ui-d31dabaa7f3d421896d095761ccad295 (final executable).
  The unsaved-prompt screenshot was visually inspected. An earlier attempt exposed
  missing accessible title/ambiguous save controls; a later stronger assertion was
  corrected to expect Undo to restore the selection immediately before Delete.
  These intermediate attempts are not counted as final acceptance.
- Release M2 drag/history/import/multi-source UI passed, including dirty-window cleanup:
  .research/m2-ui-d3733ea6ea414e4196ec75c063662db6.
  Release chrome UI passed both themes, four widths and focused page entry:
  .research/chrome-ui-74577653e9854d4c9a586b1a8f7afbc0.
- Final debug M1 UI passed empty/no-argument startup and clean close, lazy 100-page
  startup, navigation/thumbnail/scroll, rapid 50-400% zoom and resize:
  .research/m1-ui-1b8b478590194c9ea5e455ec7010ca4b. Its owned fixture window was
  closed normally after acceptance. No new 1000-page stress/physical-DPI test is claimed.
- All UI input/captures used only owned synthetic fixtures and test HWNDs. No real
  user PDFs, global SendKeys, new installations, remote CI or push were involved.
  Autosave, crash recovery, forced termination and external-source recovery remain
  outside this feature; see unsaved-changes.md.

---
# Latest verification - partial tagged Extract, 15 November 2026

- Formatting, workspace/all-target denied-warning Clippy and **37 unit tests** passed.
  New tests cover the passive Ink closed schema and selective accessibility JSON
  patching, preserving annotation data and stream dictionaries. Existing unsafe/
  schema/identity/snapshot/no-clobber tests remain green.
- Full debug native smoke passed. New three-page synthetic fixture has tagged content,
  metadata/viewer settings and passive Ink on page 3. Partial pages 1 and 3 require
  consent; without consent no output exists. With consent, page count/text and full
  512px RGBA rasters match. Whole-source byte-copy and existing M1/text/M3 checks passed.
- Explicit debug AND release checks of the user's supplied three-page document passed
  for both page 1 and annotated page 3: QPDF/PDFium validation, exact full-raster equality,
  text, consent refusal and unchanged source bytes. Original source SHA-256 remains
  unchanged. No actual document text, screenshots or PDF copies enter tracked fixtures.
  Temporary verification outputs were owned/removed.
- Release workspace build passed. Expanded M3 UI passed Batal/Simpan tanpa tag, tag
  removal from catalog/page objects, preserved Ink subtype/InkList on extracted page 3,
  unchanged source hash and the existing organizer/Save As/Extract/history/metadata/
  bookmark/popup/select-copy/import/Open/cleanup checks. Evidence:
  .research/m3-ui-fd8b8252153d4570bed5b349905dea15.
- No new dependency/runtime/installation, user-file overwrite, remote CI or push.
  Existing user sessions were preserved. Tag preservation/remapping, other annotation
  types/actions/appearances, active forms/signatures and broader fidelity are not
  newly claimed; see ADR-0003 and compatibility for this deliberately scoped policy.

---
# Latest verification - whole-source Extract, 15 November 2026

- Read-only inspection of the user's supplied one-page invoice found an empty
  AcroForm (only Fields, zero field summaries, no XFA), tags, viewer preferences and
  page-level StructParents/Tabs. QPDF check returned 0. No user contents or file copy
  were added to tracked fixtures; outputs lived only in owned temporary directories.
- Workspace/all-target denied-warning Clippy, formatting and **35 unit tests** passed.
  New cases cover original-order/full-selection eligibility, rejection after partial
  selection/reorder/rotation, copy-only tag acceptance and rejection of active fields,
  XFA, signature flags, encryption and document actions. Immutable plan fields prevent
  strategy changes after the source preflight.
- Full debug native suite passed existing M1/text/M3 checks plus a matching synthetic
  tagged/empty-form whole-source Extract. Output equals every original byte, and
  rotated tagged extraction remains blocked rather than stripping structures.
- Explicit debug and release verification against the supplied invoice passed:
  identical output bytes, unchanged source bytes, QPDF/PDFium checks, page text and
  sampled raster comparison. Temporary outputs were removed. Source SHA-256 checked
  independently before/after and remained unchanged. No actual invoice screenshot or
  contents were captured in acceptance evidence.
- Release workspace build and expanded headed M3 suite passed, including UI selection
  and Extract of the matching one-page fixture, identical output/source hashes, and
  existing Save As/Extract/history/consent/error-popup/copy/recovery/cleanup checks.
  Evidence: .research/m3-ui-e453bedfe89b411cab9338b41a136ca6.
- Existing user processes were retained during rebuild. No installation, new native
  engine, format converter, remote CI or push occurred. This is a narrow complete-
  source extraction path, not general form/tag-preserving structural editing.

---
# Latest verification - selectable error text, 15 November 2026

- Popup error body now uses GPUI Kit SelectableText for plain literal text. The
  Salin pesan button copies the complete original reason without dismissing the
  dialog. Tutup has a direct accessible callback and restores the modal guard.
- Desktop all-target Clippy with denied warnings and the release build passed.
  No backend/domain/dependency changes were made; full unit/native-only suites
  were not repeated for this UI-only fix.
- Final release M3 UI suite passed: drag selection over the error body, visible
  highlight (screenshot inspected), exact clipboard text after Salin pesan, popup
  remaining open, Tutup/repeated error recovery, plus existing Save As/Extract/
  history/bookmark consent/catalog/import/Open/source-hash/staging checks.
  Evidence: .research/m3-ui-dbb145fcc2394193b732bb34be6e2fb5/error-popup.png.
  Original clipboard formats were restored. Input targeted only the test's HWND.
- Tests wait for modal entrance/hover before dragging and modal exit before further
  input. Open waits for a new workspace generation/source identity, including when
  recovering from an empty state. Intermediate failed attempts exposed an accessible
  footer callback issue and stale/empty-state assumptions in the harness; they are
  not counted as passing acceptance. The final rerun above completed successfully.
- Existing user windows were preserved by an in-workspace executable rename before
  rebuilding. run.cmd launches the new release; existing windows keep old code.
  This change does not alter export preservation policy or resolve the user's
  separate unknown-catalog PDF. No remote CI, installation or push was performed.

---
# Latest verification - GPUI Kit error popups, 12 November 2026

- scripts/check.ps1 passed fmt, workspace/all-target denied-warning Clippy,
  **33 unit tests** and release workspace build. After adding the modal text-input
  guard, desktop Clippy plus debug/release builds passed again.
- Release native smoke passed the complete existing M1/text/M3 suite. Catalog
  diagnostics now name actual known blockers; a unit case checks tags, MarkInfo,
  page labels, viewer preferences, layout, color intents and unknown attribute names.
  No unsupported features were newly allowed or removed.
- Expanded release M3 UI passed GPUI Kit error popups for export forms, tagged
  catalog, page labels, unknown catalog, failed import and failed Open, including
  exact reasons, Tutup and recovery. Existing organizer/Save As/Extract/history,
  metadata/bookmark consent, source-hash and owned-staging checks also passed.
  Final evidence: .research/m3-ui-84551693d1c14fcc91d903420c224a9d.
  error-popup.png was visually inspected: centered modal, icon/title, reason,
  dimmed document and Tutup. Earlier pre-guard run passed as well:
  .research/m3-ui-ae36ecd010e444b386ea4f8ba22cce3b.
- Final release text selection regression passed single-click/jitter, double-click,
  forward/reverse drag/copy, zoom reuse, mode/conversion selection preservation,
  four rotations, cross-page copy, clear/cache bounds and source hashes. Clipboard
  restored. Evidence: .research/text-ui-eae2e837b2c547ff8611b739a495889c.
- One intermediate headed attempt failed because Windows created an unowned Save
  picker while the test app was inactive. The harness now also finds top-level
  #32770 pickers using the test's own PID; it never targets another app's dialog.
  The failed attempt is not counted as acceptance. Only its own synthetic process
  was cleaned up. Production handling was not weakened to work around automation.

The user's PDF/path was requested but has not been supplied. Its exact catalog
attribute and appropriate export support remain unconfirmed. This change implements
popup feedback and precise diagnostics; it does not claim to fix export of that
specific document. All acceptance used synthetic files. No installation, converter,
remote CI or GitHub push occurred. Existing user windows/edits were preserved.

---
# Latest verification - bookmark export confirmation, 12 November 2026

This supersedes the blanket bookmark rejection in the earlier M3 records.

- scripts/check.ps1 passed: formatting, workspace/all-target denied-warning Clippy,
  **32 unit tests**, and release workspace build. Current debug desktop also built.
- Debug and release native smoke passed. Bookmark cases include direct destinations,
  local GoTo actions, legacy and name-tree destinations, nested/sibling outlines and
  combined Info metadata. Without consent, publication is refused and staging cleans
  up. With consent, output actually has no bookmark structures, retains page content/
  sampled raster and passes QPDF/PDFium validation. Rotated selected-page Extract
  requires its own consent and does not advance the clean baseline.
- JavaScript, remote and chained bookmark actions remain blocked. Unit cases also
  cover invalid parents/targets, cycles, unknown keys, missing summary items, unrelated
  annotations and unvalidated orphan actions. Existing source-change/no-clobber and
  other unsafe-PDF checks remain in the native suite.
- Headed release scripts/m3-ui-smoke.ps1 passed the existing two-source organizer/
  Save As/Extract/history/native-cancel/metadata/form checks plus bookmark-only,
  named-bookmark and combined bookmark+metadata dialogs. It asserts warning text,
  Batal retaining dirty with no output, Simpan tanpa bookmark producing a clean full
  export, omitted outlines, expected 90-degree rotation, unchanged source hashes and
  staging cleanup. Evidence: .research/m3-ui-4acc3841c85344269a4cd64f4be11b84.
- PowerShell parsing and whitespace checks passed. The interrupted attempt was not
  counted as completed release/UI evidence; the resumed checks above finished.

Only synthetic fixtures were used. The user's specific PDF was not supplied or
inspected, and complex bookmark actions/other unsupported document features may
still block export. Bookmark preservation/remapping is not implemented: policy v2
allows omission only after explicit snapshot-specific consent. Existing reader/text/
chrome regression results below are historical and were not rerun for this change.
No new engine/runtime, installation, remote CI, converter or push was performed.

---
# Latest verification - M3 safe PDF export, 11 November 2026

This supersedes status claims in historical sections below. Current implementation:
M1 viewer, M2 organizer, M3 PDF Save As/Extract and presentation-only conversion home.

- scripts/check.ps1 passed formatting, workspace/all-target denied-warning Clippy,
  **30 unit tests**, and the release workspace build. Debug workspace build also
  passed. Additional native regression cases were then added; formatting/Clippy and
  debug/release builds passed again. No product logic changed after release UI checks.
- Debug and release native smoke passed: existing M0/M1/text cases plus production M3
  two-source reorder/rotate/delete Save As and selected Extract. It compares source
  text and sampled rasters, tests all 16 intrinsic/virtual rotation combinations,
  Unicode/spaces/ampersand/leading-hyphen paths, metadata detection and consent,
  form/signature-marker/tag/bookmark/annotation/JavaScript/attachment/label/unknown
  rejection, real encrypted input, malformed input, actual QPDF recovery exit 3,
  unused-source filtering, source changes before commit and staging cleanup.
- Unit failure coverage includes no-clobber of an existing hardlink, a competing
  destination created after staging, source changes, no metadata consent, fake engine
  failure, cancellation and output validation mismatch. Real child-process fixtures
  exercise capture overflow, a 100 ms timeout, cancellation and exit code 7. QPDF
  missing/version mismatch and JSON schema/cycle/count/unsafe-object cases are tested.
  The synthetic signature marker is not a cryptographically valid signature fixture.
- Release M3 UI passed: import two sources, move/rotate/delete, Save As seven pages,
  QPDF count/rotation verification, clean baseline plus Undo/Redo, Extract one page
  retaining dirty status, native save cancellation, metadata decline/accept, blocked
  form export retaining a readable viewer, source hashes and staging cleanup.
  Evidence: .research/m3-ui-7bbee9c601fc4ffaaf510ca304f3d33c.
  Debug M3 UI also passed: .research/m3-ui-7d6625f4d15e4e8e9670fc10d9fff22d.
- Release chrome passed Baca/Atur/Konversi, both themes, long filename,
  860/1000/1200/1600 widths, navigation/zoom non-overlap and focused input/Enter.
  Evidence: .research/chrome-ui-2e567cbc4cbf4b9f9879823d4cbf8475.
  Conversion without a document passed after readiness was made explicit:
  .research/chrome-ui-48846d95a2cf4538ae237726790301a4.
- Release M2 UI passed drag/cancel, rotation/move/delete/empty/history/native import,
  exact multi-source mapping and source hashes:
  .research/m2-ui-bf41773522f842cdb9cfbecc49532035.
- Release M1 UI passed empty startup, lazy 100 pages, navigation/boundaries/thumbnails,
  scrolling, rapid 50-400% zoom, fit/resize and unchanged sources:
  .research/m1-ui-010a6ccb74954136ae5bbcb09cf79c32.
- Release text UI passed single-click/jitter, double-click, forward/reverse drag/copy,
  zoom reuse, mode/conversion selection preservation, four rotations, cross-page copy,
  clear/cache bounds and source hashes. Clipboard restored:
  .research/text-ui-a031084328064388a4efd606e56f873f.
- PowerShell parsing and git diff whitespace checks passed. CI includes expanded native
  smoke automatically, but no remote CI execution is claimed.

Harness fixes: native Save filename is Edit ID 1001, Open is Edit ID 1148; do not
match the address toolbar or Search Box. TaskDialog's CCPushButton HWND control ID
is not its logical answer ID; use posted TDM_CLICK_BUTTON with pinned GPUI's negative
answer indices. Chrome waits for accessible readiness. Rapid zoom can disable its
button between UIA IsEnabled and Invoke; the test handles that race and still requires
exact final numeric bounds. Earlier failed attempts are not passing evidence.

All data was synthetic and UI input targeted the test's own HWNDs; no global SendKeys.
Physical mixed-DPI/trackpad/modifiers, broader accessibility, real complex PDF fidelity,
ACL-specific errors, hard native cancellation/isolation, crash recovery and portable
clean-machine/license acceptance remain manual or future gates. No 2000+ page or
universal preservation claim is made. Sources remain read-only; converters remain
Akan datang. See ADR-0003 and m3-safe-export-plan.md for exact export limits.

---

# Historical verification records
# Verification log

Results below were accumulated during the implementation sessions of 9–10 November
2026. See [arsip handoff](archive/PROJECT_HANDOFF_HISTORY.md), recorded 11 November 2026,
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

## M3 analysis + planned conversion home - 2026-11-11

Scope: analyze safe PDF exporting and future formats; implement only the owner's
explicitly requested Konversi/Akan datang page. M3 Save As/Extract and all conversion
backends remain unimplemented. No engine/model/codec feature/dependency was installed.

- `scripts/check.ps1` passed: fmt, denied-warning workspace/all-target Clippy,
  21 unit tests and workspace release build.
- `scripts/chrome-ui-smoke.ps1 -Profile release -NoDocument` passed: Konversi opens
  without a PDF, all planned cards are present without InvokePattern, and return works.
  Evidence `.research/chrome-ui-fe2186433c9f48ec9b06cd3492d7aed6`.
- `scripts/chrome-ui-smoke.ps1 -Profile release` passed: Baca/Atur/Konversi, light/dark,
  long filename, 860/1000/1200/1600 widths, placeholder-only cards, preserved workspace
  and focused reader input. Evidence `.research/chrome-ui-ce0a270006bb422b9282b55f2a58b833`
  includes 24 theme/width/mode screenshots. Actual DPI remains 96; physical mixed DPI
  is not claimed. Grid uses two/three/four columns as width permits.
- Release M2 UI smoke passed including mode semantics, drag/cancel, history, empty,
  native import, multi-source mapping and unchanged source hashes:
  `.research/m2-ui-b4876d03447d44e0b93905cc1e37da4e`.
- Release M1 viewer UI smoke passed (empty startup, 100 pages, navigation/scroll,
  50-400% zoom, resize): `.research/m1-ui-bc1f43c00fa241f39d2a07147fedc49e`.
- Release text-selection UI smoke passed, now also checking that Konversi hides Copy
  and returning preserves the previous selection, followed by four rotations and
  cross-page copy: `.research/text-ui-e3072931751841588c32518d76a4c1d7`. Clipboard restored.

The analysis also ran bundled QPDF 12.4.2 --job-json-help and structured inspection
on a generated fixture. A synthetic QPDFJob JSON selected A:2,A:1,B:4 and applied
relative rotations, produced three pages passing --check, and retained both source
hashes. Evidence `.research/m3-analysis-289cb38e568e41a597167ce18ae30c7c`. This proves
job-file invocation only; no production export/preflight/atomic-output gate is claimed.
Official reference sources and implementation/quality decisions are recorded in
m3-safe-export-plan.md and conversion-roadmap.md.

A running user release session was preserved by renaming its loaded executable to
`target/release/pdf-studio.previous-dcd498672e83487b849f8cde642b55f9.exe` before building
the new default executable. Acceptance viewers were separate synthetic instances.
