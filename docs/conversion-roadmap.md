# Conversion roadmap and extension boundaries

Status: future design, 11 November 2026. The owner explicitly requested a visible
Konversi page with Akan datang cards. That page is implemented; all conversion engines
below remain planned. M3 PDF Save As/Extract remains the next functional milestone.

## Product categories and order

Smallpdf and iLovePDF group tools into organization, optimization, conversion in both
directions, OCR/editing/security. We adopt the category distinction as product direction,
not a promise to reproduce their engines, quality, accounts or hosted services.

| Stage | Capability | Candidate path | Gate before enabling |
|---|---|---|---|
| C0 (now) | Konversi home with planned cards | GPUI presentation only | Clear Akan datang; no execution/upload |
| M3 (next) | PDF Save As + selected-page Extract | QPDF page plan + safe output | Preservation policy and failure acceptance |
| C1 | PDF pages to PNG/JPEG, PDF to TXT | PDFium owned raster/text + explicit encoders | Bounded batch export, dimensions/Unicode/order |
| M4 | Split, lossless optimize, release hardening | Existing blueprint | Cancellation/process/packaging gates |
| C2 | PNG/JPEG to PDF | Dedicated image-to-PDF writer/adapter | EXIF, DPI/page fit, colors, large inputs |
| C3 | DOCX/XLSX/PPTX to PDF | Benchmark a local Office engine such as headless LibreOffice | Fonts, pagination, profiles/process cleanup, redistribution |
| C4 | OCR + editable PDF to DOCX | Layout reconstruction pipeline and DOCX writer, or evaluated SDK | Indonesian/English text and layout quality corpus |
| C5 | PDF to XLSX/PPTX; later formats | Table/slide-specific reconstruction | Format-specific fidelity and editability gates |

C1 is deliberately ahead of Word because the current renderer/extractor supplies much
of its input. Office import/export, OCR and annotations remain distinct capabilities.
Conversion is an explicit future scope extension to the Level 1 blueprint, not a claim
that its former non-goals are already implemented. No new engine was installed here.

## Architectural place

Keep the source PDF/page-plan domain independent of DOCX/XLSX/PPTX or GPUI.
The current conversion.rs is a presentation-only catalog of planned cards, not a plugin
registry or backend capability assertion. It preserves the reader workspace when entered.
Cards are informational groups, have no click handler and are not file-picker actions.
Baca/Atur keep their current controls. Back/Esc returns to reading; Ctrl+G returns to the
reader page field. Reader page/zoom/history shortcuts do not mutate a hidden plan while
the hub is shown. Source loading still returns to Baca and import still enters Atur.

During C1, add only the first concrete conversion use case and the minimal interface
it consumes. Proposed application-side shapes, not implemented APIs:

- `ConversionRequest`: conversion kind, immutable workspace-instance/revision page
  snapshot or separate owned input files, format-specific options, destination intent.
- `ConversionCapabilities`: supported operation plus availability (planned, ready,
  dependency unavailable). Build this from the wired adapters; UI strings alone never
  authorize execution. Use a simple match/table; no dynamic engine discovery framework.
- `ConversionReceipt`: completed outputs, original snapshot identity, semantic losses
  or quality warnings, and per-file outcome for batches. A conversion never marks the
  PDF workspace clean. Only M3 full PDF Save As can do that.
- Shared application job phases and platform staging/process/cancellation mechanisms
  from M3. Keep format validation/preflight separate from the PDF exporter policy.

Separate adapters: PDF structural exporter (QPDF), raster-image writer (PDFium+encoders),
text writer, future reflow/document converter, and future OCR/Office workers. QPDF is
not a Word converter; sharing an output dialog does not imply sharing its engine.
Do not add unsupported variants, generic service traits or empty converter crates now.
When a card becomes ready, add its adapter, settings and tests in the same vertical slice.

All operations are local-first by default. No automatic upload, external account,
cloud fallback, runtime download or telemetry. An optional network provider would need
its own product decision and explicit per-document transfer consent. New runtimes,
models and packages should be placed on D: on this development machine; do not install
or download them as part of the current placeholder work.

## C1 - PDF to images and text

For PNG/JPEG, convert the captured current page plan, including reorder/delete and
virtual rotations. Default page scope is all; selected-page export uses current workspace
order. The renderer requests a width derived from PDF points and requested DPI; it does
not reuse a viewport-scaled GPUI preview as export-quality output. Encode the original
owned RGBA before the desktop's BGRA swap. JPEG requires explicit RGB/background handling.

- Defaults proposed for C1: PNG, 150 DPI, white background; JPEG quality 90 when chosen.
  PNG transparency is a later explicit option, not inferred from a viewer background.
- One image per output page, not embedded-image extraction. Names use current ordinal,
  e.g. page-0001.png with width max(4, digits(total)). Choose a new dedicated output folder.
  ZIP packaging and extracting original embedded images are separate later options.
- Compute dimensions and memory before allocation. Keep existing 8192-side/16 Mi-pixel
  limits until measured. A4 at 300 DPI is feasible, but oversized pages or higher DPI can
  exceed the cap. Reject or ask for lower DPI; never silently label a capped preview as
  an exact-DPI export. The present width-based renderer clamps raster sizes, so C1 needs
  a strict export sizing contract rather than using its viewer clamp blindly.
- Render/encode one page at a time in the background, cap queued work and stage outputs.
  Do not collect a book of RGBA images in memory or warm the entire viewer/GPU cache.
- Snapshot filenames/collisions before starting. Stage the batch in a unique sibling
  directory and publish only to a new folder; no overwrite/merging into an existing
  directory. If publication is not atomic on the target filesystem, report partial
  completion honestly and delete only files owned by this job. Cancellation cleans staging.
- Enable only the required PNG/JPEG features on the pinned image dependency and update
  the lockfile/notices when C1 is implemented. Current default-features=false is not
  evidence that image encoding is available.

TXT export uses the text extraction port in page-plan order, emits UTF-8 with explicit
page separators and warns about extracted reading order. Do not reuse the 1 MiB clipboard
limit as a whole-document export contract; stream with its own documented output cap.
A scan-only page produces a no-text/OCR-needed result, not invented content. Page text
storage order and visual rotation are distinct: never sort glyphs solely by rotated XY.

Raster/text conversion needs its own capability report. A policy refusing structural
PDF export does not automatically forbid every other format, but it never silently
bypasses encryption, signatures, rights or user-visible loss. Signed/form/tagged/annotated
inputs remain unsupported for initial C1 until explicit render/loss policies and tests
exist; do not present raster conversion as preservation of their interactive semantics.

## Word, OCR, Excel and PowerPoint

PDF is a positioned page representation; an editable Office file requires paragraphs,
reading order, fonts/styles, tables, images and structure that may not exist in the PDF.
Extracting Unicode or copying a page image into DOCX is not equivalent to recovering an
editable document. File appearance and natural editability are separate acceptance axes.

For C4, the default product target is editable text/paragraphs with clear warnings that
layout may change. A DOCX made of page screenshots must not be labeled editable Word.
A layout-preserving image document, if ever offered, is a separately named operation.
Scanned pages require OCR before reconstruction, with language settings and uncertainty
reporting. OCR alone does not reconstruct Word styles, tables or columns.

Candidate research lanes (no selected/purchased backend):

- Open pipeline: PDF text or OCR -> normalized layout representation -> DOCX writer.
  Tesseract is a possible OCR worker with positional outputs; package language data
  and benchmark Indonesian/English scans before adoption. It is not a direct DOCX engine.
- Structured extraction: evaluate Docling or another documented layout system as an
  optional worker, then assess whether a DOCX writer is still required. PDF/DOCX INPUT
  support is not proof of editable DOCX OUTPUT support. Audit model/runtime footprint,
  licenses and network/download behavior separately from the code package license.
- Specialized local SDK: benchmark an SDK explicitly supporting PDF-to-Office, such
  as Apryse's documented Structured Output module. Verify offline operation, Windows
  packaging, commercial terms and measured fidelity before proposing an adoption.
  No subscription, API key or installation is authorized by this analysis.

Do not treat LibreOffice command-line --convert-to as proof of high-fidelity PDF-to-Word.
Its documented headless/filter support is a candidate for the opposite direction,
Office-to-PDF. That worker needs a private user profile, bundled/pinned version, macro/
external-resource policy, font availability and process cleanup; never kill the user's
own running Office/LibreOffice instance or reuse its profile for automation.

Excel additionally requires table boundaries, cells, merges and type handling; untrusted
text must not become executable formulas by default. PowerPoint requires an explicit
choice of editable objects vs page-image slides. Neither is just changing an extension.
These formats stay planned until their specific product/fidelity contract is approved.

## Quality gates and later formats

Benchmark a licensed/generated corpus: text PDFs, image-only and mixed scans, Indonesian/
English, columns, headings, numbered lists, tables, figures, headers/footers, ligatures,
rotations, crop boxes and unusual fonts. Measure text correctness/reading order, table
structure, visual layout, actual editability, time, peak memory and errors separately.
Round-trip Office-to-PDF rendering helps visual comparison but cannot prove editability.

For images validate encoded file signatures, dimensions/DPI metadata, orientation, color,
page order, JPEG tolerance and PNG pixel equivalence against the intended render. Include
large books, collision/disk errors, cancellation, stale source and unchanged-source hashes.
For TXT test Unicode/order/newlines/scans with streaming output, without GPU dependence.

TIFF/WebP/HTML/SVG/RTF/PDF-A may be evaluated later with separate capability/quality gates.
SVG is vector extraction, not a renamed PNG. PDF/A is a conformance project, not a QPDF
optimization switch. Keep sign/redact/protect/unlock outside this conversion milestone.

## Reference sources

Checked 11 November 2026; product features are inspiration, not reproduced behavior.

- [Smallpdf official tools](https://smallpdf.com/pdf-tools)
- [iLovePDF official tool documentation](https://www.ilovepdf.com/help/documentation)
- [QPDF JSON: limitations on extracting/reflowing text](https://qpdf.readthedocs.io/en/12.4/json.html)
- [LibreOffice command-line/headless/profile documentation](https://help.libreoffice.org/latest/en-US/text/shared/guide/start_parameters.html)
- [Tesseract output documentation](https://tesseract-ocr.github.io/tessdoc/FAQ.html)
- [Docling supported formats](https://github.com/docling-project/docling/blob/main/docs/usage/supported_formats.md)
- [Apryse PDF-to-Office module documentation](https://docs.apryse.com/core/guides/features/office/convert-to-office/csharp)
