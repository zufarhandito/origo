# M3 - Safe PDF export: analysis and implementation contract

Status: design only, 11 November 2026. The conversion hub is presentation-only.
No production Save As, Extract or converter was implemented by this analysis.
This extends the approved blueprint and keeps its conservative preservation policy.

## Current gap

The multi-source workspace already has stable PageIds, original indices, virtual
rotations and unique content revisions. PDFium renders/extracts text in a bounded
native worker. QPDF 12.4.2 exists locally but QpdfProbe only handles trusted fixtures.
There is no ExportPlan, preflight report, output transaction or saved-revision update.
The image dependency currently disables default features, so PNG/JPEG encoding must
not be assumed available merely because RGBA images are displayed in GPUI.

The current reader/organizer UI is kept. Save As will be an explicit PDF operation;
format conversion has a separate Konversi home and separate completion semantics.

## M3 behavior and contracts

1. Capture an immutable `ExportPlan` in application/export.rs: workspace-instance
   token, content revision, ordered page entries, and only the source documents
   referenced by those entries. A new instance token is assigned on Open; the
   existing source_sequence is unsuitable because edits also increment it.
2. Full Save As uses all current pages. Extract uses selected PageIds in current
   workspace order, regardless of HashSet iteration or click order. Reject zero
   pages, missing IDs/sources and invalid rotations before I/O. No duplicate pages.
3. Add application ports `ExportPreflight`, `PdfExporter` (writes a staged PDF),
   and `AtomicOutput` (owns staging/commit). Keep document rules independent of
   native engines and filesystem. Implement process/output helpers in a small
   platform adapter crate during M3; do not create an exporter plugin registry.
4. Add `Workspace::mark_exported(revision)` guarded by current revision equality.
   The application additionally checks the captured instance token. Mark clean
   only after successful full-workspace PDF commit, never after Extract, image,
   TXT, DOCX or an older snapshot's completion. Undo/Redo back to a marked baseline
   must report clean. Preserve the original source references and edit history;
   Save As does not silently replace/rebase the active workspace onto its output.
5. Save As opens a native destination picker, defaulting to a new derived .pdf name.
   Ctrl+Shift+S and a labeled Simpan sebagai control enter the same use case.
   Extract is available from Atur halaman when there is selection. No Save-over-
   original path exists. Keep Baca/Atur compact; add output actions as one group.
6. One output job per window. Navigation/reading continues; edits/Open may change
   the active plan while the captured snapshot exports. A stale successful receipt
   reports which snapshot completed and never marks the new/current document clean.
   Phase progress is explicit; QPDF phases are indeterminate, not invented percent.

## Preservation policy and preflight

Preflight runs against private source snapshots used by the job, not cached viewer
rasters. Only referenced sources are inspected; deleting every page of an unsupported
source must not block export of remaining ordinary sources.

- Require an unencrypted input and clean QPDF structure check. Treat warnings as
  unsupported/review-needed in v1, rather than turning exit 3 into silent success.
  Encryption probes have different exit-code meanings from normal QPDF jobs.
- Inspect structured JSON v2 with stream data disabled. Resolve indirect objects,
  the catalog, page tree, AcroForm and signatures; summaries alone cannot establish
  the absence of every unsafe feature. Missing/unknown required JSON schema fails closed.
- Block signed, encrypted, AcroForm/XFA, tagged/StructTreeRoot, malformed/recovered,
  portfolio/attachment, JavaScript/action, optional-content or unknown-complex
  documents. Initially also block outlines/bookmarks, annotations and nontrivial
  page-label constructs until dedicated preservation fixtures pass. Viewer support
  remains independent: a blocked export can still be read.
- Ordinary Info/XMP metadata is detected and handled by an explicit warning: the
  initial --empty strategy does not promise to retain document metadata. Require
  acknowledgement for that plan/report; rejection cancels cleanly. If detection
  or warning coverage is unreliable, block that case rather than silently losing it.
- No implicit flattening, decryption, signature stripping or page rasterization.
  Preserve supported page content/resources as PDF objects, with relative rotation.
- Preflight report contains typed reasons/warnings, source fingerprints and policy
  version. Cache only by all of these; no global "PDF is safe" boolean. Source size
  and mtime are the existing baseline, not a cryptographic identity guarantee.

## Output transaction and QPDF compilation

Capture plan -> choose destination -> prepare an owned unique staging directory in
its parent -> copy referenced sources -> preflight -> acknowledge warnings -> write
staged PDF -> validate -> commit new output -> publish receipt -> cleanup.

- Validate fingerprints before/after snapshot copying and again before committing.
  Export from the private copies so a later file replacement cannot change the bytes
  QPDF consumes. A file that changes while exporting is rejected conservatively.
  Same-size/mtime-preserving changes can evade the current baseline; strict hashing
  at import belongs to a separate strengthening, not a claim made by M3.
- Refuse existing destinations, including aliases/symlinks/hardlinks to sources.
  Recheck with a non-clobber commit primitive, not exists()+ordinary rename. Flush
  output before commit; validate Windows/NTFS behavior in tests. No universal
  crash-safety claim. Cleanup only staging objects owned by this job. Crash-orphan
  recovery is separate from normal error/cancel cleanup.
- Use QPDFJob JSON generated with typed serialization and UTF-8 without BOM. This
  is job configuration, distinct from inspection JSON. Compile stable source page
  references to one-based ranges; group adjacent runs without reordering. Relative
  +90/+180/+270 rotations address final output positions. Set --empty explicitly.
  Verify every used job field against the pinned 12.4.2 help/schema.
- Spawn bundled QPDF with separate Command arguments, stdin null, hidden Windows
  process, no shell. Passing one job-file argument avoids a large per-page Windows
  command line. Enforce a 16 MiB job/inspection-JSON budget; exceeding a cap returns
  a resource error. Never truncate JSON and then interpret the prefix as a report.
- Production process handling must replace the probe's 64 KiB JSON-unsuitable
  capture with bounded streaming capture: 16 MiB structured output, 64 KiB diagnostic
  text, drain/terminate safely on overflow. Do not show raw document diagnostics or
  encryption secrets in production logs.
- Default overall QPDF phase deadline: 120 seconds, with cancellation checked while
  polling. On cancel/timeout kill and reap the child, then clean owned staging.
  PDFium validation is still an in-process native call: cancellation there is
  cooperative before/after the call, not a hard native timeout. Do not claim process
  isolation until the separate native-worker hardening is implemented.
- Validation: QPDF --check must succeed cleanly; reopen via PDFium, inspect every
  output page count/geometry/orientation against the plan. Integration tests compare
  representative content rasters. Production metadata/spot checks are not proof of
  byte identity or universal semantic preservation. Only commit after validation.
- Metadata warning acknowledgement is tied to this plan and preflight policy. A
  different snapshot or changed report requires a new decision. Cancellation and
  every failure leave existing user files untouched; a destination race is an error.

## Delivery slices and acceptance

1. Pure ExportPlan compilation + revision/instance receipt tests, including source
   runs, selected order, empty/invalid references and all intrinsic/virtual rotations.
2. Structured preflight on generated plain/metadata/bookmark/form/signature/tagged/
   encrypted/annotation/malformed fixtures, including missing JSON and resource caps.
3. Native QPDFJob adapter + safe staging/no-clobber process handling, tested on Windows.
4. Application orchestration with fake ports: cancellation, process failure, old
   revisions, new document with same numeric revision, destination races and cleanup.
5. Save As/Extract UI: import A+B, reorder/rotate/delete, export new PDF, reopen and
   compare order/orientation/content; source hashes unchanged. Test Unicode, spaces,
   leading hyphens and long/large job plans. Exercise access denied, missing sidecar,
   QPDF warning/nonzero, output validation failure, timeout and failed commit.
6. Run fmt, denied-warning Clippy, workspace tests, native fixtures and headed
   reader/organizer/text/output smoke; update compatibility and preservation ADR.

## Feasibility checked in this analysis

Pinned qpdf --version returned 12.4.2. --job-json-help exposes empty/inputFile/outputFile,
pages[file,password,range], and rotate[]. A generated job selected A:2,A:1,B:4, applied
relative output rotations and produced three pages passing --check; both source
fixture hashes remained unchanged. Evidence (ignored local data):
`.research/m3-analysis-289cb38e568e41a597167ce18ae30c7c`.
This is a synthetic CLI feasibility probe, not a production preflight/export acceptance.

References (checked 11 November 2026):

- [Pinned QPDF CLI: job JSON, exit codes, page-selection limitations](https://qpdf.readthedocs.io/en/12.4/cli.html)
- [QPDF inspection JSON v2](https://qpdf.readthedocs.io/en/12.4/json.html)
- Existing PDF_EDITOR_CODEX_BLUEPRINT.md and ADR-0003 remain the safety baseline.
