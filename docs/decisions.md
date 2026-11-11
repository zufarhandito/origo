# Implementation decisions

- M0 established actual Windows native rendering and the D:-local toolchain.
- GPUI Kit 0.9.1 supplies the native UI and matching GPUI family (ADR-0004).
- M1 introduces the pure domain workspace, metadata inspection, virtualized viewer,
  generation-aware rendering and independent CPU/GPU lifetime bounds (ADR-0005).
- M2 implements page commands, multi-source plans and bounded history. M3 implements
  conservative Save As/Extract with private snapshots and validated non-clobber output.
- Book follow-up retains native sessions on a dedicated thread, recycles them after
  64 renders/15 idle seconds and adds progressive previews. Eight generated cases up
  to 1000 pages support raising the default page cap to 1024; the file cap stays 128 MiB.
  See books.md for measured results and real-scan acceptance still pending.
- Sources remain read-only. Production export uses strict structural preflight;
  ordinary metadata loss requires explicit consent and complex documents are blocked
  under ADR-0003. Format converters remain informational roadmap entries.
- The owner requested text selection/copy before M2, extending the blueprint's original
  viewer scope. This adds native text extraction and a selection overlay; raster
  rendering remains unchanged. OCR, text editing and search are separate capabilities.
- Rust, MSVC, SDK payload and build output remain under D:. Some mandatory Microsoft
  servicing/runtime files remain on C:. Do not remove registered `.tools` installations.
- Portable publication waits for native license/runtime audit and clean-machine checks.
