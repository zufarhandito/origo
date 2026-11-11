# ADR-0004: adopt GPUI Kit for the desktop presentation

The owner requested GPUI Kit (https://gpui-kit.com/component/) for the complete UI.
Use `gpui-kit = 0.9.1` exactly, with the component/assets features supplied by default.
The facade selects the matching `gpui-pre-* = 0.3.12` snapshot. Remove the old direct
Zed git `gpui`/`gpui_platform` dependencies rather than mixing incompatible GPUI types.
Cargo.lock records actual registry checksums and the matching platform snapshot.
This supersedes ADR-0001's dependency selection; its original Windows investigation
remains historical evidence. Rust stays pinned at 1.98.1.

Inspect the published 0.9.1 sources and their `.cargo_vcs_info.json` (Kit source commit
`39ab849a93b5f15507b969164d49891db8d5b470`), not floating main examples. The facade provides
`application`, `init`, `open_window` and the Root provider. Its action macro is used;
an explicit crate alias lets GPUI macro paths resolve consistently through the facade.
The dependency tree has one GPUI implementation; no alternate Fast backend is enabled.

Presentation uses real Kit TitleBar, Toolbar, Button, Empty, Spinner and StatusBar
components with semantic theme colors and bundled Lucide icons. AllAssets is required
for the full icon names used here; the default reduced Assets bundle omitted several.
Theme switching is view-local and does not change PDF raster data. Library defaults
provide disabled states, tooltips, focus treatment and toolbar keyboard behavior.
Page navigation and PDF processing remain in their existing modules; no PDF work occurs
in render callbacks. Custom presentation lives in `apps/desktop/src/views.rs`.

The app defaults to Light and offers a sun/moon button; preference persistence is deferred.
Native file selection, page shortcuts and source safety are retained. No organizer
actions or nonfunctional placeholders are added to this UI change.

License texts: `LICENSES/GPUI-Kit-Apache-2.0.txt` from the published source revision and
`LICENSES/Lucide-ISC.txt` from gpui-kit-assets 0.9.1. A complete release dependency audit
is still required before redistribution. The app initiates no network operations.

Verification: workspace clippy/tests, debug build, real Rust native-engine smoke test,
actual Windows empty/document states, Ctrl+O native file selection, page navigation,
and light/dark switching with unchanged sampled PDF raster interior. Full release,
screen-reader, mixed-DPI and clean-machine packaging checks are not claimed here.
