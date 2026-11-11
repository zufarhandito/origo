# ADR-0001: one Zed revision for GPUI and the Windows platform

Dependency decision superseded by [ADR-0004](0004-gpui-kit.md) following the owner's
request for GPUI Kit. The original findings and toolchain setup below are historical.

Decision: pin both git dependencies to `ec237216460f8a34a4ac7c6b068e838593976ede`.
`gpui_platform` is absent from crates.io; mixing crates.io GPUI 0.2.2 with current git
platform code would risk incompatible types. Use Rust 1.98.1, matching this revision's
toolchain. The pairing now passes workspace check, clippy, tests and debug build;
the resulting native window has displayed an actual PDF page on this machine.

Inspected the revision's `gpui_platform/Cargo.toml`, hello-world example, `assets.rs`,
`elements/img.rs`, `app/context.rs`, dialog options and application methods. The platform
crate enables the native Windows backend and Windows manifest. `RenderImage` consumes
BGRA frames; `prompt_for_paths` returns a nested result through a oneshot channel.

Source: https://github.com/zed-industries/zed/tree/ec237216460f8a34a4ac7c6b068e838593976ede/crates

Environment: Windows x64, initially no Rust, Cargo, MSVC compiler or Visual Studio
installer. Rust was installed locally under ignored `.tools` without changing system
PATH. With the owner's explicit authorization, Build Tools 2022 17.14.44 was installed
under `.tools/vs2022`, shared tools/cache under `.tools/vs-shared` and `.tools/vs-cache`.
Windows SDK 10.0.26100 was relocated to `.tools/windows-sdk` with a junction at its registered
Windows Kits path. Installer exit code and relocation exit code were both 0.
The real GPU window responds and displays a PDF. Separate native-dialog/keyboard/resize
and 100%/150%/200% DPI acceptance checks remain pending.

