# Supplemental version-specific notices

Some published Cargo archives omit their repository's license text. `manifest.json`
maps each supplemental text to its crate version, upstream commit URL and SHA-256.
Commit IDs come from that published archive's `.cargo_vcs_info.json`; hashes verify
our stored copies. The distribution audit validates the hashes before copying them.
Git attributes preserve their upstream bytes across Windows checkouts; upstream
trailing spaces are retained so hashes and verbatim notices remain consistent.

For alternative licenses, the collected MIT text is used when available; otherwise
the actual upstream license file is preserved. Collection is evidence gathering,
not a blanket legal clearance or a choice of license for PDF Studio itself.

Unresolved evidence remains explicitly reported for mac 0.1.1, seahash 4.1.0 and
simd_helpers 0.1.0. Do not replace missing copyright/notice evidence with a generic
license template or treat package metadata alone as a completed review.
