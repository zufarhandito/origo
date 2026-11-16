PDF Studio - internal Windows x64 portable test candidate

Extract the entire ZIP to a writable folder. Double-click pdf-studio.exe.
Keep sidecars/, the app-local runtime DLLs and LICENSES/ together.
No Rust, Visual Studio, QPDF installation, administrator access or runtime download
is needed to launch this candidate on a compatible Windows installation.

Target for acceptance: Windows 10 22H2 and Windows 11 x64, with OS ICU/UCRT,
Direct3D 11 and a working graphics driver. Clean-machine acceptance is pending.
Open PDF through Berkas > Buka PDF. Changes are virtual until Simpan sebagai.
Existing destinations and source PDFs are never overwritten. Complex PDFs may
be viewable but conservatively blocked for export. There is no OCR/converter.

This is a test candidate, not an approved public release. See build-info.json,
RELEASE-CHECKLIST.md and LICENSES/dependencies/cargo-inventory.json for open gates.
The project license, third-party evidence gaps, source/notice obligations and
Microsoft redistribution entitlement still require owner/reviewer sign-off.
Collected notices and declared licenses are evidence, not legal clearance.

Physical mixed-DPI/trackpad/keyboard/Narrator acceptance remains pending.
App-local MSVC runtimes must be serviced by rebuilding the candidate when updated;
they do not receive central-runtime servicing automatically.

files.sha256.json lists every packaged file except itself; the adjacent ZIP
.sha256 file checks transfer integrity, not publisher authenticity or signing.
No application signing, installer or automatic updater is supplied.
