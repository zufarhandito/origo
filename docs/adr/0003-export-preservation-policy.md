# ADR-0003: defer production export until preflight exists

There is no user-document export in M0. The QPDF probe operates on a generated two-page
fixture without forms, encryption, signatures, outlines, tags or catalog extensions.
It uses `--empty --pages` with two source files, selects page 2 then page 1, and applies
relative +90 rotation to output page 1. It writes a fresh temporary output and runs
QPDF `--check`; the Rust smoke binary reopens it through PDFium and checks count, rotation,
page identity and unchanged source bytes.

This is a feasibility experiment, not evidence of universal catalog preservation. Before
M3 exposes Save As, implement conservative preflight, changed-source detection, staged
same-directory output, no-overwrite commit and output validation. Block encrypted,
signed, form-bearing, malformed, tagged or unknown-complex documents. Metadata/bookmarks
need dedicated detection and an explicit acknowledgment policy, or blocking.

Reference: https://qpdf.readthedocs.io/en/12.4/cli.html#page-selection

