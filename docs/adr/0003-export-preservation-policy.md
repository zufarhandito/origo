# ADR-0003: conservative structural PDF export

Status: implemented in M3, 11 November 2026. This supersedes the M0-only deferral.

Save As and selected-page Extract create a new PDF with pinned QPDF 12.4.2.
Use QPDFJob JSON with empty, consecutive source/page runs in workspace order and
relative rotations addressed by final output position. Supported page content and
resources stay PDF objects; there is no rasterization or image recompression.
The empty-document strategy avoids importing stale document-level references from
an arbitrary primary source. It does not establish universal preservation.

Preflight examines private copies of every referenced source. Require an unencrypted
input, clean QPDF checks (warnings/recovery are errors), recognized JSON v2 schema,
resolved catalog/page tree and agreement with the inspected page count/order.
Catalog/page dictionaries use conservative allowlists, with a depth-bounded page
tree and recursive inspection of returned object values for known unsafe features.
Block forms/XFA, signature markers, tags, outlines/bookmarks, page labels, annotations
and links, attachments, active/optional content and unknown complex constructs.
Viewing remains available independently of export compatibility. Do not decrypt,
flatten, strip signatures or pretend that unsupported features are preserved.

Ordinary trailer Info and catalog XMP metadata are detected, their references are
resolved and the user must explicitly acknowledge possible document-metadata loss.
Consent applies to the owned prepared plan/report, not a remembered global choice.
Declining drops that job's staging. Metadata preservation is not promised. The
catalog also permits Lang/Version without a promise of retaining those document hints.

Capture only referenced sources; deleted-source registry entries retained for Undo
cannot block the remaining plan. Check source size/mtime before/after copying and
before commit, and export from the private copies. This detects ordinary changes,
but is not cryptographic identity protection against size/mtime-preserving edits.

Write only inside an owned unique directory in the destination parent. Refuse all
existing destinations, including hardlinks/symlinks. Validate with clean QPDF checks
and PDFium page count, dimensions and rotation before flushing and persist_noclobber.
Windows tests cover an existing hardlink and a destination created during the job.
Failure/cancel cleans only owned staging. Crash-orphan recovery, universal crash
durability, a hostile-filesystem sandbox and Save-over-original remain outside M3.

QPDF uses hidden shell-free child processes, bounded output, cancellation and a
cumulative 120-second process-time budget. Kill/reap on timeout, overflow or cancel.
Do not publish raw diagnostics or paths in production logs. PDFium validation is
in-process and only cooperatively cancellable before/after its native call.

Only a committed full Save As receipt matching workspace instance and current
revision advances the saved baseline. Extract and stale results cannot mark current
edits saved. Saving retains source references and history; it does not rebase them
onto the generated file. Opening/closing still discards unsaved edits without a prompt.

Acceptance includes ordinary two-source edits, all 16 intrinsic/virtual rotations,
source text and sampled raster identity, metadata consent, structural unsafe fixtures,
recovery warnings, source changes, process failures and no-clobber cleanup. A synthetic
signature marker is a structural rejection test, not a cryptographic signature audit.
See [testing](../testing.md) and [implementation contract](../m3-safe-export-plan.md).

Reference: [QPDF 12.4 page selection](https://qpdf.readthedocs.io/en/12.4/cli.html#page-selection).
