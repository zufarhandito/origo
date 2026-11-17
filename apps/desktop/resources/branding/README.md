# Origo provisional icon

Owner-supplied raster mark, approved for installation on 17 November 2026.
`origo-source.png` preserves the supplied 233x203 image with opaque white background.
The mark is centered on a square without stretching. No vector master is available;
256px is an upscale, not additional detail.

- `origo.ico`: 16,20,24,32,40,48,64,128,256 px; Windows icon resource ID 1.
- `origo-32.png`: embedded title-bar image displayed at 16 logical px.
- `origo.rc`: icon + Origo product/version metadata; leaves GPUI manifest unchanged.

To replace: update the source PNG, run `python scripts/update-origo-icon.py` with
Pillow installed, then rebuild. Review both light/dark title bars and verify native
icon resources with `scripts/branding-ui-smoke.ps1`. ICO/PNG are embedded in the exe;
portable packages do not need an additional external icon file.

Display name is Origo. Binary `pdf-studio.exe`, state folder, environment variables
and internal crate/package names retain their existing names for compatibility.
Installer/Store artwork, SVG and a complete brand system remain future work.
