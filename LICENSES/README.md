# Third-party notices — M0 preparation

This directory is not a completed redistribution audit or a license grant for this project.

The verified download script preserves upstream distribution files:

- GPUI Kit 0.9.1: `GPUI-Kit-Apache-2.0.txt`; Lucide icon notice: `Lucide-ISC.txt`.

- QPDF 12.4.2: `sidecars/qpdf/share/doc/` (including distribution notices) and all bundled DLLs.
- PDFium Chromium 8125: `sidecars/pdfium/LICENSE` and `sidecars/pdfium/licenses/`.
- GPUI/Zed source revision: Apache-2.0/GPL obligations must be reviewed at the actual
  crate boundary and for its complete transitive dependency graph before distribution.

QPDF is Apache-2.0 and PDFium uses BSD-style notices with third-party obligations.
Before shipping a ZIP, inventory all Cargo/native dependencies, verify redistribution
terms and include their license texts. No production release has been packaged or approved.
