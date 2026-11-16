# Split PDF

Open **Alat → Split PDF** from Baca or Atur halaman. Split captures the current
page order, imports, deletions and virtual rotations. It does not change the page
plan, selection, history or saved baseline. Thumbnail selection does not restrict
Split; use manual ranges to choose positions explicitly.

## Controls

- **Setiap N halaman**: enter 1 through the current page count. For 23 pages and N=10,
  the outputs contain 10, 10 and 3 pages. Default: min(10, page count).
- **Rentang manual**: semicolons separate files; commas combine ranges/pages in one
  file. `1-3,5; 6-8` produces two files with 4 and 3 pages. Positions start at 1 in
  the current workspace. Ranges must increase across the batch, without overlaps,
  repeated pages, reversed ranges or out-of-range positions. Omitted pages are not
  exported; the preview shows chosen and total page counts.
- **Nama folder baru** defaults to `hasil-split`. Use a new single folder name,
  at most 100 characters, without path separators, control characters, reserved
  Windows device names or invalid/trailing Windows filename characters.
- **Pilih folder induk** opens the native folder picker. The named child folder is
  published only after the whole batch succeeds. Existing files, directories
  (including empty directories) and links are refused, including publication races.

Names are `bagian-0001.pdf`, `bagian-0002.pdf`, and so on. Only PDFs appear in the
published folder; snapshots and QPDF work files remain in staging. Progress counts
validated parts. **Batal ekspor** cancels the whole batch; failures/cancellation
publish no final folder, including after earlier parts were validated.

## Safety and compatibility

Only referenced sources are copied, once into a shared private snapshot. Every
group passes existing Selected Extract preflight, QPDF write/check and PDFium
count/geometry/rotation validation. After all parts succeed, outputs are flushed,
originals/snapshots rechecked and the ready directory moved without replacement.

One batch-specific prompt combines detected tag/bookmark/metadata loss. Declining
publishes nothing. Tagged single-source groups may omit accessibility structure
after consent and preserve supported passive Ink. Active forms/signatures,
encryption and other unsupported content remain blocked. An entire unmodified
source in original order can retain the byte-identical whole-source Extract path,
including its narrow empty-form/tagged exception. Multi-source tagged restructuring
is not newly supported. See ADR-0003 and compatibility.md.

Split uses the normal per-window export slot and cancellation control. Editing can
continue against the live workspace after configuration; output represents the
captured revision. Close/Open waits for an active Split and rechecks dirty. Split
never acts as a full Save As receipt.

Limits remain 1024 aggregate source pages, 128 MiB aggregate source data, bounded
QPDF JSON/output and a cumulative 120-second QPDF child-process budget per job.
Manual input is capped at 16 KiB. Many-output jobs can hit the budget; no new
1000-part performance claim is made. PDFium remains in-process with cooperative
cancellation. Crash-orphan recovery, universal crash durability, overwrite and
non-Windows publication are outside this feature.

## Implementation and verification

Application `split` owns parsing, plans, consent aggregation and staged validation.
Platform `split` owns shared snapshots, flushing and publication. The distinct
SplitReceipt cannot advance the PDF clean revision. `PdfInspector::inspect_output`
releases native output handles before returning, enabling Windows directory moves
and cleanup. Normal viewer inspection retains its bounded native session reuse.
Preview parsing avoids constructing full per-part plans during rendering.

Windows publication uses pinned atomicwrites 0.4.4. Its safe `move_atomic` wrapper
calls MoveFileExW without replacement; std::fs::rename can replace an empty directory
on this toolchain/filesystem. See [the pinned implementation](https://docs.rs/crate/atomicwrites/0.4.4/source/src/lib.rs).
The MIT notice is LICENSES/Atomicwrites-MIT.txt. Other platforms fail publication
rather than falling back to a replacing rename.

Run `scripts/check.ps1`, `scripts/smoke-test.ps1` and
`scripts/split-ui-smoke.ps1 -Profile release`. Native smoke covers order/text/raster,
consent, changed sources, collision races, injected mid-batch failures and cancellation.
The headed suite uses owned synthetic fixtures and its own HWND, including the native
folder picker. See testing.md for actual results.
