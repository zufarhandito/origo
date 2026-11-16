# Windows portable candidate acceptance

An engineering candidate can be built and tested while publication gates remain open.
No public release, installer, release tag or binary upload is implied by a Git push.

| Gate | Evidence required | Current status |
|---|---|---|
| Pinned inputs | Matching archive SHA-256, Cargo.lock, source revision/dirty marker | Automated by package script |
| Runtime closure | x64 standard/delay imports resolved to packaged files or explicit Windows components | Automated by package verifier |
| Transfer integrity | ZIP SHA-256, exhaustive path/size/SHA manifest after extraction | Automated by package verifier |
| Relocated UI/export | Read, edit, Save As, source identity and local DLL paths from extracted package with restricted PATH | See current testing evidence |
| Project license | Owner chooses distribution terms for first-party code | Pending owner decision; no license invented |
| Third-party notices | Review inventory, missing evidence, native static dependencies and bundled assets | Pending final redistribution review |
| MPL source | Unmodified option-ext crate source archive and notice accompany candidate | Collected; reviewer validates obligations |
| Microsoft runtime | Unmodified x64 release CRT, version/hash; entitlement under installed VS terms | Technical inventory collected; entitlement review pending |
| Security | Review advisories for pinned Rust and native versions; record findings | Cargo OSV scan done; anyhow patched, four unmaintained notices/native review remain |
| Clean Windows | Repeat on clean Windows 10 22H2 and Windows 11 x64 without toolchain or external engines | Pending separate machines/VMs |
| Physical input/a11y | Complete device-acceptance matrix with actual observations | Pending human/hardware |

Clean-machine procedure:

1. Transfer the ZIP and its checksum through the chosen delivery channel; verify SHA-256.
2. Extract to a path with spaces/Unicode; keep all directories and notices intact.
3. Launch as a standard user without installing Rust, VS, QPDF or PDFium. Record OS,
   graphics driver, actual DPI, runtime errors and SmartScreen/signing behavior.
4. Open synthetic A/B, switch tabs, import, reorder/delete/rotate, Undo/Redo, Save As
   to a new PDF. Reopen and compare order/orientation/text/raster; hash sources.
5. Exercise Split/Optimize, cancellation and existing-destination refusal. Check owned
   staging cleanup and the unsupported form/signature path. Do not use private PDFs.
6. Run the physical device matrix including Narrator. Record each pass/fail separately.

Passing on the developer workstation with a restricted PATH is relocated-package
evidence. It does not prove a clean OS, graphics-driver compatibility or human input.
