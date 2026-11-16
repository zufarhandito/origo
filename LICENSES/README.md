# Third-party notices and distribution evidence

This directory collects redistribution evidence; it is not legal clearance or a license
grant for PDF Studio. The project itself has no declared distribution license yet.

The verified download script preserves upstream distribution files:

- GPUI Kit 0.9.1: `GPUI-Kit-Apache-2.0.txt`; Lucide icon notice: `Lucide-ISC.txt`.
- atomicwrites 0.4.4 (Windows no-replace directory publication): `Atomicwrites-MIT.txt`.

- QPDF 12.4.2: `sidecars/qpdf/share/doc/` (including distribution notices) and all bundled DLLs.
- PDFium Chromium 8125: `sidecars/pdfium/LICENSE` and `sidecars/pdfium/licenses/`.
- Current locked GPUI Kit/base/pre crate metadata declares Apache-2.0. This is based on
  the actual Cargo graph, not a blanket assumption about every Zed repository component.
- [Supplemental notices](supplemental/README.md) provide version/commit/hash provenance
  for 18 crate versions whose Cargo archive omitted a license file.
- `scripts/audit-distribution.ps1` inventories the desktop normal/build graph, preserving
  notice files and the option-ext 0.2.0 MPL source archive. Runtime/build dependencies
  are a conservative workspace-feature superset, not a binary-level SBOM.

QPDF is Apache-2.0 and PDFium uses BSD-style notices with third-party obligations.
Before shipping a ZIP, inventory all Cargo/native dependencies, verify redistribution
terms and include their license texts. The internal portable candidate carries this
inventory, all native notices and app-local MSVC runtime evidence. Three Cargo notice
gaps, owner licensing/redistribution review and clean-machine/device acceptance remain
publication gates. See [distribution audit](../docs/distribution-audit.md).
