# Book viewer follow-up - 2026-11-09

Implemented after M1: persistent PDFium document sessions on a dedicated native thread,
512-pixel visible previews before rasters wider than 1024 pixels, and a default page
limit of 1024. The file limit remains 128 MiB. No new applications were installed.

A renderer and its clones share one bounded channel and one document. Source switches,
fingerprint mismatches and failed operations release the session. Idle sessions close
after 15 seconds; 64 render attempts trigger reopening before the next operation.
This amortizes PDF parsing without keeping native document caches indefinitely.
It bounds session lifetime, not total native memory or the duration of a native call.

The 32-entry UI queue prioritizes visible previews, visible full rasters, neighbor
rasters and thumbnails. Previews share the existing CPU byte budget, replace placeholders
until final rasters arrive and release GPU residency when superseded. Cached full rasters
skip preview work. This is progressive rendering, not a measured scroll-stop debounce;
an already-running full raster cannot be interrupted.

## Actual release measurements

These measurements predate the text-selection follow-up. That feature adds a separate
16 MiB text-data cache; these earlier working-set figures are not new-build guarantees.

Machine: Intel Core i7-13620H, about 15.7 GiB RAM, Windows build 10.0.26300.
These are generated fixtures, not scanned books from a scanner. Image-only fixtures
contain unique 1500x2100 grayscale simulated text-line images with RunLengthDecode.
Vector fixtures contain simple colored pages, labels and mixed geometry/rotation.

Native timings below exclude GPU upload, UI painting and fixture generation. Raster
width is 1200 pixels; each case makes 72 render requests including distant pages.
Times vary with machine load and PDF content; no speedup ratio against the old adapter
is claimed. Metadata inspection still visits every page, without eager rasterization.

| Fixture | Pages | File MiB | Inspect ms | First raster ms | Raster median ms | Raster p95 ms |
|---|---:|---:|---:|---:|---:|---:|
| Vector | 100 | 0.03 | 16 | 3.54 | 3.36 | 4.13 |
| Vector | 300 | 0.09 | 29 | 4.71 | 3.38 | 4.62 |
| Vector | 500 | 0.15 | 20 | 4.62 | 3.38 | 4.78 |
| Vector | 1000 | 0.30 | 50 | 3.75 | 3.40 | 4.25 |
| Simulated scan | 100 | 4.85 | 27 | 12.45 | 12.17 | 13.21 |
| Simulated scan | 300 | 14.55 | 39 | 12.08 | 12.32 | 52.75 |
| Simulated scan | 500 | 24.26 | 121 | 52.15 | 51.41 | 55.19 |
| Simulated scan | 1000 | 48.52 | 205 | 48.46 | 51.29 | 54.20 |

All eight cases passed: 576 renders, two native opens per 72 renders, pixel/background
checks, source bytes unchanged, stale-fingerprint invalidation and recovery. The last
case also verified session release after 16 idle seconds. Documents with 1025 pages
and input larger than 128 MiB were rejected. Benchmark process peak working set was
159.8 MiB, including generation buffers and native rendering, excluding GUI/GPU.

The headed release viewer passed with the 1000-page image-only fixture: lazy startup
(3 page/4 thumbnail attempts), jump to 1000, last-page boundary, thumbnail activation,
continuous scrolling, rapid 50-400% zoom and resize. Two passes visited 100 evenly-spaced
destinations across the book each, including the last page. This is 200 requested
destinations, not a claim that every page was read in order.

After each pass working set was 371.7/367.9 MiB. Final working set was 353.5 MiB and
peak 521.5 MiB (including high-zoom/transient allocations). Page/thumbnail caches stayed
within 128/16 MiB; final queue empty, process responsive, zero GPU-release errors.
There were 1346 native renders and 22 native document opens, consistent with 64-render
recycling. Cache budgets are not total process memory limits.

Evidence: ignored .research/book-benchmark/{results.csv,process.json} and
.research/m1-ui-24335a04409b41e89ec507eb6746c8cb. Initial UI attempts shared a fixture
with its generator and were invalidated by source-change detection; acceptance was
rerun on an isolated copy. The UI script now always copies its input into its own
evidence directory, checking both original and copy hashes afterward.

After correcting status-text encoding, both desktop profiles rebuilt and the latest
release UI passed the non-stress 1000-page acceptance again (4 initial page attempts,
4 thumbnail attempts). Evidence: .research/m1-ui-4c1fc7607b8241559e2d368525e4ee14;
this latest release window is left open. The earlier stress measurements remain the
evidence for two-pass residency bounds; only status strings changed afterward.

## Reproduce and remaining work

Build release and stage the existing sidecars, then run:

```powershell
./scripts/benchmark-books.ps1
./scripts/m1-ui-smoke.ps1 -Profile release -Fixture .research/book-benchmark/simulated-scan-1000.pdf -ExpectedPages 1000 -Stress -StressSamples 100
```

The native benchmark is also cargo run --release --locked -p pdf-smoke -- --benchmark-books.
CI includes it but has not run remotely. PowerShell wrapper records process peak and
logs; the headed UI script targets only its own process and leaves that viewer open.
Use -StressSamples 1000 to request all 1000 pages per pass; that longer test was not run.

Real JPEG/JPEG2000/high-resolution/color scans, complex fonts, large files beyond
128 MiB and physical mixed-monitor DPI still need representative testing. No real-book
performance guarantee follows from generated fixtures. OCR, text search, outline/page
labels, saved reading position and native process isolation are not implemented here.
