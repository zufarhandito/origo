# Native PDF Editor (Level 1) — Architecture & Implementation Blueprint for Codex

**Status:** Approved architecture proposal; implementation not yet started  
**Last reviewed:** 2026-11-09  
**Target:** Windows 10/11 x86_64 (MSVC)  
**Working language:** Rust; communicate with the product owner in Indonesian  
**Project working name:** `pdf-studio` (renameable)  
**Deliverable:** maintainable, testable, native Windows PDF organizer; *not* a full content editor

> **CODEX — READ THIS FIRST**
>
> Treat this document as the project's working product requirements and engineering contract. If an existing repository is present, inspect it before making changes; preserve compatible work and report conflicts with this blueprint. Implement in small, verifiable milestones and do not claim a feature is finished without tests or a concrete manual verification procedure. Verify current GPUI and Rust APIs against the *pinned dependency revision* before writing integration code. Favor boring, readable, testable software over premature abstraction. Do not silently skip requirements. If an external dependency or Windows environment is unavailable, document the blocker and continue with independently testable code; never invent successful builds or tests.

---

## 0. Executive decision / Architecture Decision Record (ADR-000)

We **approve Level 1 first**. Build a native desktop PDF viewer and page organizer using **Rust + GPUI**, with a **hexagonal/ports-and-adapters design** and pragmatic modularity. Keep document rules independent of GPUI, PDFium, and QPDF. This protects future work on Level 2 annotations and Level 3 content editing without paying the cost of implementing them now.

**Engine decisions:**

1. **GPUI:** native desktop presentation and input. Current GPUI is pre-1.0 and its API can break. Pin GPUI and `gpui_platform` to mutually compatible versions or an exact commit; document rationale. Windows support must be proven with a real spike.
2. **PDFium via `pdfium-render`:** reading, document/page information, and rasterizing visible PDF pages. Package a **matching pinned `pdfium.dll`** with the Windows app. A rendering adapter must hide the actual API.
3. **QPDF CLI sidecar:** PDF page structure manipulation on export (merge, extract, split, re-order, rotate, delete, lossless optimization). For the first release, bundle `qpdf.exe` plus required runtime libraries. Use `std::process::Command` with separate argument tokens **never a shell command string**. Do not rely on `qpdf` being installed in PATH. Put the executable location and version check behind an adapter.
4. **Do not introduce `lopdf` as a second modification engine** in v1 merely because it is Rust-native. QPDF is designed for content-preserving structural transforms, while `lopdf` would require us to own complicated reference/bookmark/form handling. We can revisit after measured evidence.
5. **Non-destructive editing:** represent edits as a virtual ordered page list, and export to new output bytes/files only when the user explicitly saves. The source is never modified in-place during editing.

**QPDF integration trade-off:** a separate process is easier to build, audit, and replace than C++ FFI in the first iteration; it costs process startup, sidecar distribution, and explicit error handling. Export is coarse-grained and infrequent enough to justify the trade-off. A later `QpdfLibraryAdapter` may replace `QpdfCliAdapter` without changing application/domain code.

**Architectural style:** Hexagonal architecture *at dependency boundaries*, combined with vertical use cases. **Do not** create interfaces for every struct, a plugin registry, an event bus, a database, or a distributed system.

**Core dependency direction:**

```text
GPUI Desktop (presentation adapter)
           |
           v
Application (use cases, orchestration, ports)
           |
           v
Domain (pure Rust models, rules, commands)

Infrastructure adapters implement Application ports:
  PDFiumRenderer    -> PdfReader / PageRenderer
  QpdfCliAdapter    -> PdfExporter / PdfInspector
  LocalFilesystem   -> FileStore / AtomicOutput
  NativeDialogs     -> FilePicker (presentation boundary)
```

Dependency rule: domain knows nothing about application, GPUI, PDFium, QPDF, Windows dialogs, or filesystem. Application knows domain and **port contracts**, not concrete technology. Adapters depend inward and are wired up in a composition root.

---

## 1. Product scope

### 1.1 Who is this for?

Windows users who routinely open, organize, combine, extract, split, rotate, and optimize PDFs locally. The application should be offline-first, fast to launch, visually minimal, and safe for important documents.

### 1.2 Level 1 — MVP requirements

| ID | Capability | Expected behavior | Priority |
|---|---|---|---|
| F01 | Open PDF | Native file chooser; show page count, page sizes, basic status; friendly errors | P0 |
| F02 | Render PDF | Show pages, thumbnails, scroll, page navigation; scale correctly on HiDPI | P0 |
| F03 | Zoom | Fit width, fit page, 50–400% zoom; no rendering on GPUI UI thread | P0 |
| F04 | Select pages | Single, range (Shift), and multi-select (Ctrl); keyboard accessible | P0 |
| F05 | Reorder pages | Drag-and-drop + keyboard move; maintain stable page identity | P0 |
| F06 | Rotate pages | 90° clockwise / counter-clockwise; preview immediately | P0 |
| F07 | Delete pages | Remove selected pages from workspace; recover with Undo | P0 |
| F08 | Merge documents | Append/import pages from multiple PDFs in chosen order | P0 |
| F09 | Extract pages | Save selected pages as a new PDF, optionally chosen order | P0 |
| F10 | Split PDF | Split by page ranges or every N pages, with predictable names | P1 |
| F11 | Undo / redo | Deterministic reversible page operations; dirty indicator | P0 |
| F12 | Export / Save As | Export edited virtual page order; safe output and explicit errors | P0 |
| F13 | Recent files | Optional local recent-file history without file contents or telemetry | P1 |
| F14 | Optimize (lossless) | QPDF structural/stream optimization, show before/after sizes; output **may grow** | P1 |
| F15 | Progress and cancellation | Long-running operations show progress/state; cancellation is best effort and safe | P1 |
| F16 | Drag PDF into app | Drop files into window; works for multiple files | P1 |
| F17 | Windows distribution | Portable ZIP first, pinned sidecars; installer after stable smoke tests | P0 release gate |

Clarifications:

- **Merge** adds page references to the virtual workspace. **Reorder / rotate / delete** changes that workspace, never the source bytes.
- **Extract** creates one output from the current page selection. **Split** creates multiple files with deterministic non-colliding names. **Export** writes the full current workspace.
- The user can open a PDF in a viewer and then modify its pages. Multiple inputs are presented as a single **workspace** of ordered page references.
- **Save As** is the default and required first release. "Save over original" is disabled until the tested Windows atomic-replace/backup workflow exists.
- No actual text editing, highlight annotation, text selection/search, OCR, signatures, or print workflow in Level 1. These are separate capabilities.

### 1.3 Explicit non-goals for Level 1

- Edit existing text or images inside page content streams.
- Add text, handwritten marks, stamps, annotations or digital signatures.
- Convert Word/Excel to PDF, PDF to Office, OCR, image rasterizing compression, or cloud processing.
- PDF form editing, password entry/decryption, redaction, repair, and PDF/A guarantees.
- Preserving arbitrary PDF JavaScript, dynamic forms, signatures, tagged-document semantics, and every catalog extension through cross-document merge.
- A database, account system, updater service, analytics SDK, or network backend.
- Multi-platform release before Windows x64 release is reliable.

---

## 2. Document safety and compatibility policy (NON-NEGOTIABLE)

PDFs are not simple collections of images. Page operations can invalidate catalog-level references, bookmarks, outlines, form fields, accessibility tags, embedded files, and digital signatures. QPDF has documented limitations for page selection and merging. Do not promise universal lossless preservation.

**Preflight behavior:**

1. Open and display a PDF when PDFium can render it, **even if export is unsupported**. Differentiate viewer support from editing/export support.
2. Before structural exports, inspect at minimum: parse validity, encryption, digital signatures, AcroForm/XFA, outlines/bookmarks, and tagged/accessibility structure where detectable. Prefer QPDF `--check` and structured inspection plus PDFium metadata/capabilities; verify actual CLI/API signatures.
3. For v1, **block structural export** of encrypted, digitally signed, form-bearing (AcroForm/XFA), or structurally malformed PDFs. Explain the reason and never silently flatten/strip content. Also block known unsupported dynamic, tagged, or otherwise complex PDF constructs unless preservation is validated by dedicated regression tests.
4. For ordinary PDFs with metadata/bookmarks that cannot be guaranteed after page selection, show a specific warning and require explicit acknowledgment before export; never assert bookmarks are preserved. For v1 it is acceptable to **block** instead of using a warning if reliable feature detection is not yet implemented.
5. Design the exporter to preserve page appearance/content where supported. Validate by reopening the generated output and comparing ordered page count, orientation, and selected page content rendering.
6. Detect source files changed since import (size / modified time; optional hash for strict mode). Prompt to reload or reject export. **Never silently export from a different underlying file** than the one displayed.
7. Reject exports with zero pages; handle unreadable files, duplicate source paths, and missing media gracefully.
8. Preserve source PDFs and clean temporary outputs when operations fail. Do not delete or overwrite a user file implicitly.
9. Do not log document contents, raw passwords, or personally identifying filenames in production diagnostics unless explicitly opted into detailed logging.
10. Enforce file-size/page-count/resource limits with clear errors, but make them configurable and backed by measurements. Treat malformed/untrusted PDFs as hostile inputs to native libraries.

**Conservative export strategy:** initial exports can use `qpdf --empty --pages ... -- output.pdf` when intentional metadata semantics are understood and communicated. For a single-source transform, retaining the primary input may better preserve some document metadata, but can leave stale references when selecting pages. Choose based on tests—not guesswork. Record the choice in ADR-003 and document exactly what is preserved/dropped.

**QPDF-specific caveat:** the CLI's `--pages` operation documents incomplete handling of document-level data such as outlines and forms. Duplicate-page insertion is **out of scope** for v1 because repeated page objects may share resources in ways that complicate later edits.

**Optimization naming:** call it **"Optimize PDF (lossless)"** rather than promising "Compress 80%". File size may be unchanged or larger, especially for PDFs whose images are already compressed. Recompressing/rasterizing images is outside this MVP.

---

## 3. Workspace/domain model

The domain represents a **virtual page plan**, not a mutable PDF blob.

```rust
// Conceptual domain pseudocode; adapt names/types to idiomatic compiling Rust.
struct SourceId(Uuid);
struct PageId(Uuid);

struct SourceDocument {
    id: SourceId,
    path: PathBuf,
    fingerprint: SourceFingerprint,
    page_count: usize,
    capabilities: DocumentCapabilities,
}

struct PageEntry {
    id: PageId,                 // Stable identity for selection and UI.
    source_id: SourceId,
    source_page_index: usize,  // Zero-based internally; exporter translates to 1-based.
    rotation_delta: QuarterTurns,
}

struct Workspace {
    sources: HashMap<SourceId, SourceDocument>,
    pages: Vec<PageEntry>,      // AUTHORITATIVE output order.
    selected_pages: HashSet<PageId>,
    revision: u64,
    saved_revision: Option<u64>,
}
```

**Invariants:**

- `PageId` is stable across reorder and rotation; it does not equal page position.
- Every `PageEntry.source_id` resolves to a loaded source and its index is valid.
- `rotation_delta` is one of 0, 90, 180, 270 degrees; derive effective display rotation from source orientation + delta.
- Page order is authoritative; UI widgets do not maintain an independent competing copy.
- After deletion, selection cannot point at nonexistent pages.
- Undo/redo restores the same page order, IDs, rotations, and selection semantics as specified per operation.
- A workspace may temporarily contain zero pages, but cannot be exported until non-empty.
- Changes to `zoom`, scroll, thumbnail visibility, or selection alone do **not** mark document content dirty.
- No native PDF object pointers or PDFium lifetime-bound values may escape an infrastructure adapter.

### 3.1 Commands + undo/redo

Implement domain operations with explicit validated commands, for example:

```text
AddSourcePages { source_id, insert_at }
MovePages      { page_ids, target_index }
RotatePages    { page_ids, quarter_turns }
DeletePages    { page_ids }
RestorePages   { removed_entries_with_original_positions }
```

For every command, return a corresponding inverse operation or a compact before/after delta. Prefer a **bounded command history** over cloning an entire large workspace every time. On a new command after Undo, clear the redo stack. Make all pure domain commands testable without I/O. A drag operation should commit **one command**, not one command per mouse-move event.

Defer document-level editing concepts (text objects, annotation objects) to future feature modules, rather than forcing Level 1 page operations into an over-generalized graphics object model.

### 3.2 Cross-document page references

A merged workspace contains entries from several source files. Rendering resolves each entry to its original source and page; exporting transforms ordered references to one or more QPDF source/range specifications. Validate that alternate source paths, spaces, Unicode, and repeated references to the same source do not cause argument parsing problems. Do not support duplicate copies of **the same page** until semantics are tested.

---

## 4. Practical Rust workspace layout

Keep the number of crates small enough to navigate, but isolate **stable** technical and domain boundaries.

```text
pdf-studio/
├─ Cargo.toml                 # Workspace deps, lints, profiles
├─ Cargo.lock                 # COMMITTED for reproducible desktop builds
├─ rust-toolchain.toml        # Pinned and documented
├─ README.md
├─ LICENSES/                  # Third-party notices, license texts
├─ docs/
│  ├─ architecture.md         # Diagram + dependency rules
│  ├─ adr/
│  │  ├─ 0001-gpui-windows.md
│  │  ├─ 0002-pdfium-and-qpdf.md
│  │  └─ 0003-export-preservation-policy.md
│  ├─ compatibility.md        # Unsupported document types / limitations
│  ├─ testing.md
│  └─ decisions.md
├─ apps/
│  └─ desktop/                # executable, GPUI composition root and views
│     └─ src/
│        ├─ main.rs
│        ├─ app.rs
│        ├─ actions.rs
│        ├─ state.rs
│        ├─ views/
│        │  ├─ shell.rs
│        │  ├─ toolbar.rs
│        │  ├─ page_canvas.rs
│        │  ├─ thumbnail_sidebar.rs
│        │  └─ dialogs.rs
│        └─ controllers/
├─ crates/
│  ├─ domain/                 # PURE Rust business rules, models, commands
│  ├─ application/            # Use cases, ports, orchestrators
│  ├─ pdf-engine/             # PDFium renderer/inspector adapters
│  ├─ qpdf-adapter/           # QPDF CLI adapter, arg generation, error mapping
│  └─ platform/               # filesystem, temp, atomic output, Windows specifics
├─ tests/
│  ├─ fixtures/               # Generated synthetic PDFs only
│  ├─ integration/            # End-to-end engine tests
│  └─ golden/                 # Expected page arrangement / render references
├─ scripts/
│  ├─ check.ps1
│  ├─ fetch-sidecars.ps1      # pinned versions + checksums; never floating URLs
│  ├─ build-portable.ps1
│  └─ smoke-test.ps1
└─ .github/workflows/
   └─ ci.yml
```

Adjust workspace layout if Cargo test discovery is easier with `tests/` local to crates; tests should be runnable by `cargo test --workspace` and/or well-named separate integration commands. Do not create an unneeded `ui` library crate until real shared components justify it.

**Explicit direction:**

- `domain`: no GPUI, PDFium, QPDF, dialogs, async runtime, OS syscalls.
- `application`: depends on domain and defines *minimal* outbound ports.
- `pdf-engine`, `qpdf-adapter`, `platform`: implement outward ports; can depend on application/domain, never vice versa.
- `desktop`: depends on application and adapters, wires dependencies and maps view events to use cases.
- Tests: use fakes for pure orchestration; real PDFium and QPDF only in integration tests.

---

## 5. Port contracts (define behavior before implementation)

These are **design sketches**, not assumed real APIs. Use `Result` with typed errors, `Send`/`Sync` only where justified, and concrete lifetimes/ownership that compile on pinned versions.

```rust
trait PdfInspector {
    fn inspect(&self, source: &Path) -> Result<InspectedDocument, PdfError>;
    fn preflight_export(&self, sources: &[SourceDocument])
        -> Result<ExportCompatibilityReport, PdfError>;
}

trait PageRenderer {
    fn render_page(&self, request: RenderRequest)
        -> Result<RenderedPage, RenderError>;
}

trait PdfExporter {
    fn export_pages(&self, plan: &ExportPlan, destination: &Path)
        -> Result<ExportReceipt, ExportError>;
    fn optimize_lossless(&self, input: &Path, destination: &Path)
        -> Result<ExportReceipt, ExportError>;
}

trait AtomicOutput {
    fn prepare(&self, target: &Path) -> Result<TempOutput, IoError>;
    fn commit_new(&self, temporary: TempOutput, target: &Path)
        -> Result<(), IoError>;
}
```

Avoid a giant `PdfService` that reads, renders, exports, manages UI state, and writes files. Define each port from the use case's actual needs. `FilePicker` can be a desktop UI capability instead of an application port unless necessary for testing; path selection is an input, not domain logic.

### 5.1 Initial use cases

- `OpenDocument` — validate input, inspect, add sources/pages, clear/reset workspace as applicable.
- `ImportDocument` — append pages to an existing workspace.
- `MovePages`, `RotatePages`, `DeletePages`, `Undo`, `Redo` — pure commands.
- `RenderVisiblePages` — schedule page renders with caching and stale-request protection.
- `ExportWorkspace` — validate sources, preflight, build export plan, stage output, run QPDF, reopen and verify, then commit.
- `ExtractSelectedPages` — same engine but selected page plan.
- `SplitWorkspace` — derive multiple export plans, validate all destinations, then export safely with partial-failure reporting.
- `OptimizeOutput` — separate optional workflow; do not mix silently with export defaults.

### 5.2 Typed errors and observability

Use enums with actionable categories: `InvalidPdf`, `EncryptedPdfUnsupported`, `SignedPdfUnsupported`, `FormPdfUnsupported`, `UnsupportedCatalogFeature`, `DependencyUnavailable`, `SourceChanged`, `RenderFailed`, `ExportFailed`, `VerificationFailed`, `DestinationExists`, `PermissionDenied`, `Cancelled`, etc. Preserve underlying errors internally but show concise messages + recoverable user actions in UI. Use `tracing`/`tracing-subscriber` and redact file paths/content as appropriate. Do not `unwrap()` on user input or background errors.

---

## 6. GPUI presentation and interaction

### 6.1 Desired window layout

```text
┌───────────────────────────────────────────────────────────────────┐
│ Title / document name                 [Open] [Import] [Export]    │
├───────────────────────────────────────────────────────────────────┤
│ [Undo] [Redo]  [Rotate L/R] [Delete] [Extract] [Split]  [Zoom]    │
├─────────────────┬─────────────────────────────────────────────────┤
│  Thumbnail      │                                                 │
│  sidebar        │              PDF PAGE CANVAS                    │
│  (virtualized)  │    Visible pages + selection + drag reorder    │
│                 │                                                 │
├─────────────────┴─────────────────────────────────────────────────┤
│ Ready / Processing | Page X of Y | Zoom N% | Modified •           │
└───────────────────────────────────────────────────────────────────┘
```

Minimal native visual language: quiet neutral surfaces, clear focus rings, readable typography, Lucide-like icons if license/build integration is suitable; accessible contrast and keyboard labels. No Electron, Tauri, browser runtime, or local HTTP server.

### 6.2 Single source of truth and threading

- `Workspace` is authoritative domain state; GPUI entity stores view-local state and a read model/projection, not a duplicate writable page list.
- UI events call application use cases. Mutations happen in a controlled sequence; notify GPUI entities with minimal updates.
- `Render` must remain cheap: **never** parse PDFs, rasterize pages, invoke QPDF, or do file I/O in GPUI's render callback.
- Use GPUI task/background mechanisms available in the pinned version; do not hold GPUI contexts or references into native PDFium objects across inappropriate thread/lifetime boundaries.
- Start with a bounded rendering queue (PDFium calls are effectively serialized under `pdfium-render`'s thread-safe mutex). Additional threads do not automatically mean parallel PDFium throughput.
- Prioritize visible pages, then adjacent pages, then thumbnails. Cancel/ignore stale requests after rapid zoom, scroll, source changes, or workspace revision changes.

### 6.3 Render pipeline

```text
Visible viewport -> visible PageIds -> RenderRequest(source,index,scale,rotation)
     -> bounded worker -> PDFium raster bitmap -> RenderedPage pixels
     -> cache by RenderKey -> GPUI-compatible image -> screen
```

Use a cache key containing at minimum `source fingerprint / SourceId`, `source page index`, `effective rotation`, `scale bucket / target pixel dimensions`, `device pixel ratio`, and any render background/color settings. Use a bounded **LRU cache**, e.g. initially target 256 MiB full-page + 64 MiB thumbnail budgets, then profile and tune. Budget by actual pixel-buffer bytes rather than number of pages. If GPUI uploads a second GPU copy, measure both CPU and GPU memory separately.

Validate pixel channel order and stride explicitly. GPUI supports image sources backed by rendered image data, but use the actual pinned API rather than assuming `image::DynamicImage` directly implements a view element. Render dimensions should be controlled to prevent giant allocations on extreme zoom. Previews of virtual rotations should match final QPDF output. Preserve correct aspect ratios for mixed portrait/landscape/media boxes.

### 6.4 Keyboard and UX minimum

- `Ctrl+O`: Open; `Ctrl+Shift+O`: Import; `Ctrl+Shift+S`: Export / Save As.
- `Ctrl+Z` / `Ctrl+Y` (and optionally `Ctrl+Shift+Z`): Undo / Redo.
- `Delete`: Delete selected pages; `Esc`: clear selection / dismiss dialog.
- `Ctrl+A`: select all pages in the active page list.
- Arrow keys navigate selection; Shift extends selection; Ctrl toggles selection.
- Screen reader labels, keyboard focus, and high-DPI behavior must be considered from the start even if advanced accessibility auditing is deferred.
- Show empty state, loading state, unsupported-file notice, non-destructive confirmation, and export success/failure clearly.
- A page operation should be visible immediately without waiting for QPDF to reconstruct a file.

---

## 7. QPDF export adapter and file safety

### 7.1 QPDF command construction

For a conceptual merged output:

```text
qpdf --empty --pages <src-A.pdf> 1-3 <src-B.pdf> 2,4 <src-A.pdf> 5 -- <output.pdf>
```

This is illustrative; **validate actual CLI syntax for the pinned QPDF version**, including repeated sources, page range encoding, paths that begin with a hyphen, and rotations. QPDF page specifications are generally one-based while domain indices are zero-based. Use the documented `--file=<path>` / `--range=<range>` form where it improves unambiguous argument parsing. Consider grouping contiguous page runs without changing output order. Never concatenate untrusted filenames into command-line text or execute through `cmd.exe`/PowerShell.

For rotations, generate supported QPDF `--rotate=+90:<output-page-range>` options after constructing final output ordering. Verify whether rotation commands apply to the intended output page positions; test with source pages already rotated and mixed page sizes.

When the output would exceed Windows command-line argument length limits, avoid silently failing: generate smaller verified batches through temporary intermediate PDFs (with a bounded safe plan) or return a specific `ExportPlanTooLarge` error until an alternate library interface is implemented. Never silently reorder/omit pages to fit the command line.

### 7.2 Lifecycle of `ExportWorkspace`

1. Capture immutable export plan at workspace revision `R`.
2. Validate non-empty page list, source existence/fingerprints, permissions and destination.
3. Perform preflight, displaying explicit warnings / unsupported reasons.
4. Choose a unique temp file in the **destination directory**, never a path that equals an input file.
5. Spawn bundled QPDF with a deterministic argument vector; capture exit status and bounded stdout/stderr. Enforce a timeout and offer cancellation via process termination where safe.
6. Require success exit code and existence of a non-zero output file.
7. Run QPDF validation and reopen using PDFium; verify page count, orientation and selected-page structural expectations. Render spot-check samples as appropriate for integration tests.
8. Only then commit temporary output to the user's requested destination. If destination exists, require explicit confirmation, but default MVP to non-overwrite Save As.
9. On error/cancel, remove only the temporary output under our control. Keep user source/destination untouched. Report failure and allow retry.
10. If workspace revision changed during export, do **not** mark current workspace "saved"; the exported file represents revision `R`.

For replacing an existing output in a later milestone, implement and test a Windows-specific same-volume replace/backup strategy. Do not rely on a sequence of renames being universally atomic or crash-safe. If that cannot be guaranteed, disable overwrite instead of gambling with user data.

### 7.3 Split/export-all safety

Generate all filenames before starting, validate duplicates/collisions and free space where possible, write each output to staging, and clearly report partial completion if the batch fails. Never delete pre-existing unrelated files. Give the user an actionable failure log without sensitive document contents.

### 7.4 Versioned sidecars

- Document **pinned** QPDF and PDFium versions and their expected SHA-256 hashes in a machine-readable manifest (hash values must be genuine, not invented).
- Bundle `qpdf.exe`, its required DLLs, `pdfium.dll`, and license/notice files in predictable paths relative to the executable.
- At startup or first use, validate existence and compatibility/version before attempting the operation. Error helpfully if a dependency is missing.
- Build/release scripts download from approved official release endpoints with checksum validation, or use checked-in manifest paths; never download arbitrary latest binaries at runtime.
- Audit third-party licensing and binary redistribution obligations before shipping. QPDF is Apache-2.0; Pdfium is BSD-style. Verify transitive dependencies and bundle notices.

---

## 8. Testing strategy — must be built alongside functionality

### 8.1 Testing pyramid

| Layer | Scope | Mechanism | CI requirement |
|---|---|---|---|
| Domain unit | Page identity/order/rotation/selection/invariants | `cargo test`, pure Rust | Required each PR |
| Property-based | Arbitrary sequences of add/move/rotate/delete/undo/redo | `proptest` optional; seeded repeatable tests | Required after command engine |
| Application | Use-case orchestration / export preflight / failures | Fake renderer/exporter/FS ports | Required each PR |
| Adapter integration | Real PDFium render, QPDF input/output, version mismatch | Synthetic PDFs + pinned sidecars | Required on Windows CI |
| GPUI interaction | Focus, selection, actions, basic view state | GPUI's `#[gpui::test]` / `TestAppContext`, where supported | Targeted smoke tests |
| Windows E2E | Open/modify/export/verify UI on real Windows GPU | Manual checklist first, automate later | Required for releases |
| Security / robustness | Corrupt inputs, giant counts, odd filenames | Regression corpus / fuzz where practical | Before public release |

GPUI officially demonstrates `#[gpui::test]`, `TestAppContext`, and window-based tests; use those examples and match the pinned GPUI version. Native/GPU behavior must still be verified on actual Windows hardware.

### 8.2 Mandatory domain test cases

- Reorder one or multiple non-contiguous pages; source content mapping remains correct.
- Rotate same page four times by +90 => original effective orientation.
- Delete selection then Undo then Redo => consistent state, stable page IDs.
- New command after Undo clears redo stack.
- Page selection remains coherent across reorder/delete/import.
- Adding source pages and deleting all pages leaves a valid empty workspace with export disabled.
- Invalid page/source references and illegal move indices return typed errors; no panic.
- Workspace dirty revision changes only for edit commands, not navigation/zoom.
- `ExportPlan` reproduces exact logical page order including cross-source merges.
- `PageId` never used as a surrogate for 1-based/0-based page index.

### 8.3 Mandatory adapter/export tests

- PDF with 1 page, 2 pages, 100 pages; mixed A4/Letter, portrait/landscape, rotated MediaBox.
- Synthetic PDF with embedded images and text; visual appearance remains after reorder/merge/rotate.
- QPDF export from two sources: order, count, and rotation verified by PDFium output inspection.
- Zero-page output refused; source missing after opening; source modified while workspace active.
- Encrypted, digitally signed, form-bearing, tagged, and corrupted PDFs exercise safe reject/warning policies.
- Unicode/spaces/quotes/symbols/leading-hyphen filenames; no command injection or wrong file interpretation.
- Destination exists, access denied, disk full simulation where feasible, QPDF non-zero exit, missing DLL, kill/cancel, interrupted export; originals preserved.
- QPDF version drift and mismatched PDFium binary fail visibly with actionable errors.
- Optimize-lossless roundtrip: final PDF opens, page count and rendering remain valid even if file size grows.
- Split every N pages: correct files, ranges, naming and collision handling.

Fixtures must be generated or licensed for tests; do not commit sensitive customer PDFs. Prefer programmatically generated tiny synthetic files and a small curated regression corpus. Tests should make comparisons by parsed/rendered output rather than byte-for-byte equality because PDF object ordering may change legitimately.

### 8.4 Quality gates / commands

Baseline developer commands (adjust package options to actual workspace):

```powershell
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo build --workspace --release
```

Add `cargo audit` (if available) and dependency review, and record `cargo tree` to monitor native dependency size. CI uses a Windows MSVC runner; pure-domain tests should also be runnable on Linux where practical, but cross-platform compilation of GPUI desktop is **not** an MVP requirement. Unit tests must not require an actual UI or bundled native binaries; integration tests may require sidecars with documented skip reasons in local development, but **must not silently skip** on the release CI job.

---

## 9. Performance, reliability and security budgets

Start with measured baselines rather than inflated claims. Record build machine CPU/RAM/GPU, PDF fixture properties, wall clock, average and peak memory, UI responsiveness. Document performance regressions.

**Working targets, subject to measurement and adjustment:**

- Load a normal 100-page document without rasterizing all pages eagerly.
- Restrict active render/cache memory with a budget and eviction policy; no unbounded retention of full page bitmaps.
- Scrolling and zooming remain interactive while a background page render or QPDF export is pending.
- Cancellations and rapid zoom changes cannot display an older request as the newest image.
- No unhandled panic or source mutation on malformed documents or export failures.
- Respect Windows HiDPI scaling and large/small window resizes.
- Benchmark several realistic documents before setting hard launch-time/latency thresholds.

**Security principles:** no shell execution, no network dependency, explicit input limits, bounded stderr, temp output ownership, safe permissions, dependency version pinning, sensitive-logging controls, no telemetry by default. Be mindful that PDFium/QPDF parse untrusted native inputs; prioritize dependency updates and a future sandbox threat-model review.

---

## 10. Implementation plan and deliverables

**Important:** deliver in vertical slices. Each milestone must compile, have relevant tests, and leave the repository in a usable state. Start with architectural seams, but do not spend weeks writing unused interfaces.

### M0 — Windows feasibility spike (**first executable milestone**)

Tasks:

1. Determine current working GPUI + `gpui_platform` pairing; pin the versions or exact commit in `Cargo.lock` and ADR.
2. Compile and launch a GPUI window on Windows x64; test keyboard input, window resizing, native dialog proof-of-concept, HiDPI.
3. Bundle/load a matching PDFium binary; render the first page of a synthetic PDF to a bitmap and **actually display it in GPUI**.
4. Verify `qpdf --version` and a minimal synthetic merge/rotate/export from Rust `Command` using packaged path.
5. Confirm binary packaging and licenses can be satisfied.
6. Record concrete commands, observed behavior, and blockers in `docs/adr/0001...` and `README.md`.

**M0 acceptance:** executable window displays a real PDF page from PDFium; a Rust test/smoke command invokes bundled QPDF; no PDF parsing/rendering in a GPUI render callback; sample fixtures and setup instructions exist. **Stop feature expansion and report blockers if this fails.**

### M1 — Core foundation and viewer

- Create domain workspace + stable IDs, source inspection, typed errors.
- Implement `PdfInspector`, `PageRenderer`, and basic `OpenDocument` use case with dependency injection.
- Viewer page list, scroll, zoom, fit width, basic visible-page rendering queue, bounded cache, thumbnail sidebar.
- Test domain invariants and PDFium adapter integration.

**M1 acceptance:** opens and navigates a 100-page synthetic PDF without rasterizing every page eagerly; unit + real adapter tests pass.

### M2 — Page organizer (first value milestone)

- Selection model; reorder, rotate, delete; commands/undo/redo.
- Import another PDF into current workspace (merge concept).
- Live page preview using original source references and virtual rotations.
- Keyboard shortcuts and well-defined empty/dirty states.

**M2 acceptance:** multi-source page plan behaves correctly; deterministic edit/undo unit tests and UI smoke tests pass.

### M3 — Safe exporting

- QPDF CLI adapter, `ExportPlan` compiler, preflight policy, staged outputs, validation, Save As.
- Extract selected pages; source fingerprint checks; non-overwrite semantics.
- Error handling for QPDF/version/path/permission failures; packaging of sidecars.

**M3 acceptance:** imports 2 PDFs, reorders/rotates/deletes pages, exports, reopens generated PDF, and verifies exact order/orientation; source files unchanged; failure tests pass.

### M4 — Split, optimize and release readiness

- Split every N pages and selected ranges with collision-safe output naming.
- Lossless optimization with honest size reporting and correctness tests.
- Progress/cancellation behavior, drag-and-drop, recent files (optional), accessibility polish.
- Portable Windows ZIP, install-from-clean-machine smoke test, licenses/notices, README and compatibility matrix.

**M4 acceptance:** documented Level 1 release checklist passes on clean Windows 10/11 hardware; installer can be planned separately.

### M5 (future, **NOT** in this scope)

- Annotations, highlight, signatures/stamps (Level 2) via independent feature modules + supported PDF engine.
- True content editing and layout reconstruction (Level 3) only after separate feasibility research.
- Native binding replacing QPDF CLI if profiling/distribution justifies it.

---

## 11. Definition of Done (applies to each feature)

A feature is **not done** until:

- Domain behavior and failure cases are specified.
- Implementation respects layer dependencies and has no hidden business logic in GPUI callbacks.
- Correct unit and integration tests exist and actually ran, with results documented.
- UI handles loading, unsupported inputs, errors, and cancellation where applicable.
- No unsafe overwrite or silent data loss; source PDF remains unmodified.
- Windows packaging is exercised when native sidecars change.
- The README/compatibility list is updated if user-visible behavior changes.
- Code is formatted, clippy-clean, and uses consistent error/result handling.
- No gratuitous `unwrap`, wildcard dependency versions, or environment-specific absolute paths.
- Any newly discovered limitation is recorded as an issue or an ADR rather than concealed.

**Suggested release acceptance scenario:**

1. On a clean Windows x64 PC, launch the portable ZIP without manually installing QPDF/PDFium.
2. Open a 10-page synthetic source A and a 4-page synthetic source B.
3. Import selected pages from B; reorder, delete one, rotate another; undo then redo.
4. Export to a **new** PDF path, reopen and visually verify ordering, page count, rotation and representative content.
5. Verify original A/B file hashes are identical before/after.
6. Test an unsupported signed/form PDF and confirm editing/export is safely blocked with a clear message.
7. Simulate failed export and ensure no damaged or partially overwritten destination.

---

## 12. Codex operating instructions (execute in this order)

1. **Inspect environment and repository.** Determine OS, Rust toolchain, existing code, GPUI status, current API docs, and available native libraries. If not on Windows, implement testable core components but explicitly defer/mark GUI verification for Windows; never pretend it ran.
2. **Write `docs/architecture.md` and ADR-0001/0002/0003** explaining dependency graph, chosen revisions, version/license packaging, preservation trade-offs and immediate risks. Keep them concise and rooted in actual findings.
3. **Start M0.** Prove GPUI + PDFium bitmap display and QPDF sidecar execution. Do not begin with a giant mock UI or a hypothetical PDF renderer.
4. **Implement M1–M4 incrementally.** At each step, add tests alongside code and keep the full project buildable.
5. **Report after each milestone:** files changed, commands run, tests passing/failing, architectural deviations with rationale, currently unsupported scenarios, next vertical slice. Show real command output when helpful; don't say "all tests pass" if they were not executed.
6. **Ask the owner only when a decision is genuinely blocking.** Otherwise use this blueprint's conservative defaults and document the assumption.
7. **Do not implement Level 2/3 until the owner explicitly authorizes it.**

### First response expected from Codex

Before changing significant code, provide:

- Short environment/repo assessment.
- Working GPUI dependency strategy for Windows and any blocker.
- M0 file/task plan (not a multi-month roadmap rehash).
- Initial verification commands and how to run the resulting executable.

Then **proceed with M0 implementation** if the environment permits. A plan alone is not completion.

---

## 13. Verified reference entry points (recheck against pinned versions)

- GPUI README and platform setup: https://github.com/zed-industries/zed/tree/main/crates/gpui
- GPUI examples index: https://github.com/zed-industries/zed/blob/main/crates/gpui/examples/README.md
- GPUI `testing.rs` example: https://github.com/zed-industries/zed/blob/main/crates/gpui/examples/testing.rs
- GPUI Contexts: https://github.com/zed-industries/zed/blob/main/crates/gpui/docs/contexts.md
- PDFium Rust wrapper (recent documentation): https://docs.rs/pdfium-render/latest/pdfium_render/
- PDFium binary distribution: https://github.com/bblanchon/pdfium-binaries/releases
- QPDF manual: https://qpdf.readthedocs.io/en/latest/
- QPDF CLI page-selection and limitations: https://qpdf.readthedocs.io/en/latest/cli.html
- QPDF Windows binary and redistribution information: https://github.com/qpdf/qpdf/blob/main/README-what-to-download.md
- QPDF source and license: https://github.com/qpdf/qpdf

**Reference observation as of 2026-11-09:** `pdfium-render` docs show a 0.9.5 release and QPDF has a 12.4.2 release. These are *research observations*, not an authorization to use wildcard versions or mismatched native binaries. Verify exact compatibility, pin selected versions/commit and SHA-256 values, and document how to update them.

---

## 14. Final engineering principle

**Prioritize correctness and user-document safety over feature count.** A native PDF organizer with reliable undo/redo and exports is more valuable than a large interface that silently damages complex PDFs. Build the smallest tested vertical slice, profile with real fixtures, and extend only across well-defined boundaries.
