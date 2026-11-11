# ADR-0002: pinned dynamic PDFium and QPDF sidecars

Use `pdfium-render = 0.9.5` with explicit `pdfium_8125` and `thread_safe` features;
disable floating `pdfium_latest`. Use the corresponding Chromium 8125 Windows x64
binary from bblanchon/pdfium-binaries. Use QPDF 12.4.2 MSVC x64 ZIP.

`sidecars.lock.json` records exact release URLs and SHA-256 digests obtained from GitHub
release metadata, not generated placeholders. Download scripts validate archives before
extraction. Binaries never download during application execution and never resolve via PATH.
Keep every DLL and notice from both archives, rather than guessing which runtime files
are optional. Native binaries remain ignored build inputs, not repository source.

Official release endpoints:
- https://github.com/bblanchon/pdfium-binaries/releases/tag/chromium%2F8125
- https://github.com/qpdf/qpdf/releases/tag/v12.4.2
- https://docs.rs/pdfium-render/0.9.5/pdfium_render/

QPDF is invoked through `Command` tokens with a 30-second timeout and bounded retained
stdout, no shell, no console window and no sensitive diagnostic output. The runtime
version probe requires exactly 12.4.2. PDFium uses the explicit bundled DLL path; missing
or incompatible bindings return an actionable error. Runtime binary integrity beyond
validated installation is still a release-hardening task, not an implemented guarantee.

QPDF is Apache-2.0; PDFium is BSD-style with third-party licenses. Preserve the complete
upstream license trees. Redistribution/transitive-license and clean-machine runtime
audits remain pending; M0 is not an approved distributable release.

