# Find text in the active PDF

The [competitor research and owner decisions](research/text-search.md) explain the
Find interaction and distinguish verified product behavior from local adaptations.

Open **Cari teks** in the reading toolbar, **Edit → Cari teks…**, or **Ctrl+F**.
The compact bar sits below the reading toolbar, inside the active document tab.
Each tab keeps its own query, options and current occurrence during the session.

- Default matching is **case-insensitive**. `hasil penelitian` matches the entire
  contiguous phrase, including `Hasil PENELITIAN`, but not `hasil lain penelitian`.
- Adjacent whitespace, including line breaks on the same page, counts as one space.
  Leading/trailing query whitespace is ignored. Matching does not cross page boundaries.
- The options menu offers **Bedakan huruf besar-kecil** and **Kata utuh**, both off
  initially. Whole-word boundaries use Unicode letters/numbers and underscore.
- Search updates after a 200 ms typing pause. Counts are progressive while **Mencari…**
  is shown. `3/18` means occurrence 3 out of 18, not page 3.
- Enter/F3 and the down arrow select the next occurrence; Shift+Enter/Shift+F3 and
  the up arrow select the previous one. Navigation wraps at either end.
- Matches on visible pages are yellow; the current match is orange. Navigation
  reveals its page and text location, including virtual/source rotations and zoom.
- Esc or × closes the bar and removes search highlights. Reopening retains the
  query. Search highlights are separate from drag selection and clipboard copying.

Search follows the current workspace order and includes imported pages. Rotate,
reorder, delete, import and Undo/Redo invalidate old results and rescan the current
plan. Search never changes content revisions, Undo history, saved status or PDF bytes.

## Execution and limits

The application matcher performs bounded literal phrase matching with Unicode
lowercasing and source-character offsets. This is not full linguistic case folding,
accent normalization, stemming, regex, dehyphenation or reconstructed reading order.
Columns/ligatures and unusual encodings inherit PDFium's text extraction limitations.
Scans need an existing text layer; no OCR or network operation is added.

One page per visible document is extracted/matched in background using the existing
shared PDFium worker. Search retains occurrence offsets, not a book of TextPages or
rasters. Visible highlights reuse the existing bounded text cache. Query length is
limited to 1024 Unicode characters; whitespace-only/oversized queries do no native work.
Results stop at 10,000 retained occurrences and show a limit notice (`10000+`).
The existing 200,000-character per-page extraction limit also applies.

Query/source tokens reject stale completions. Hiding a tab pauses incomplete scans;
showing it again restarts those scans. Both visible split panels may continue scanning
even when only one is focused. Completed compact results remain per tab. Closing
the bar/tab stops further scheduling; an already-running native call is cooperative,
not preemptible. Failed pages are counted explicitly as incomplete search. Source
changes invalidate the document under the existing size/mtime policy.
Diagnostics contain only counts/options/states, never the query or PDF text.

## Verification

```powershell
. ./scripts/use-build-environment.ps1
./scripts/check.ps1
cargo run --release --locked --offline -p pdf-smoke -- --search-smoke
cargo run --release --locked --offline -p pdf-smoke -- --write-search-fixtures
./scripts/search-ui-smoke.ps1 -Profile release
```

See [testing](testing.md) for actual runs and [text selection](text-selection.md) for
the shared extraction, character geometry and reading-order limitations.
