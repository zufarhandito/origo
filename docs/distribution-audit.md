# Distribution audit and portable candidate

Follow-up after checkpoint `06c2a39`, 16 November 2026. The previous checklist only
named priorities 6/7 without recording their titles. This work treats them as runtime/
notice inventory, portable packaging, package verification and release readiness.
No format converter, installer, public binary upload or release tag is added.

## Repeatable engineering checks

From the existing build environment:

```powershell
. ./scripts/use-build-environment.ps1
$zip = ./scripts/package-portable.ps1
$package = ./scripts/test-portable.ps1 -ZipPath $zip
./scripts/portable-ui-smoke.ps1 -PackageDirectory $package
# Optional network audit; sends public crate names/versions, never PDF contents:
./scripts/audit-advisories.ps1 -InventoryPath "$package/LICENSES/dependencies/cargo-inventory.json"
```

The candidate and adjacent ZIP checksum stay under ignored `dist/`. The extracted
test package and UI evidence stay under ignored `.research/`. Scripts do not install
toolchains or change global PATH/DPI. The UI harness temporarily restricts the child
PATH to Windows directories and restores its parent environment after launch.
`-SkipBuild` packages the existing release executable; build it first when source
changes. `build-info.json` records the revision and dirty state, not a claim that an
unstaged source tree exactly produced an older executable.

The builder:

- Re-extracts QPDF 12.4.2/PDFium 8125 from SHA-256-verified pinned archives. It never
  trusts mutable `target/*/sidecars` or copies a developer directory wholesale.
- Includes the native distributions' runtime binaries/docs/notices, Cargo.lock,
  collected Cargo notice files, supplemental provenance, and option-ext's unmodified
  source archive. It excludes toolchains, user PDFs, private research and old executables.
- Copies x64 release CRT from the installed VS redist folder to the application and
  QPDF directories, recording versions/hashes. No runtime is copied from System32.
  `-RuntimeDirectory` accepts another licensed x64 Microsoft.VC143.CRT folder.
- Resolves x64 standard/delay import tables against packaged files or an explicit
  Windows DLL/API-set list. This checks static import closure; PDFium is also loaded
  dynamically in the real UI smoke. Arbitrary DLLs on the developer PC cannot satisfy it.
- Creates a file manifest and ZIP checksum. ZIP timestamps are normalized only on
  the owned package copy because upstream files may predate ZIP's 1980 epoch.

The verifier checks ZIP checksum, entry paths/duplicates, exact file membership and
hashes, required files, import evidence and pinned QPDF under restricted PATH. It
relocates extraction to a new path with spaces, Unicode and `&`. Negative tests alter
only this owned extraction, reject modified contents/missing runtime, restore it,
then require the original checks to pass again. Checksums are transfer integrity,
not a publisher signature. Run scripts only on trusted candidate ZIPs.

The UI smoke requires an interactive Windows desktop: render/text load, local
PDFium/MSVC module paths, virtual rotate, Save As, QPDF page count/rotation, reopen
in another tab, source hash and staging cleanup. CI runs headless packaging/integrity
tests only; it does not claim UI/GPU or clean-machine acceptance.

## Notice inventory and remaining decisions

The Windows desktop normal/build graph has 512 records, including seven project crates.
It conservatively includes workspace-unified feature dependencies, so it is a superset
of code linked into the desktop executable, not a perfect runtime SBOM.

The actual GPUI crate-family metadata declares Apache-2.0. The audit preserves notice
files across the graph rather than assuming a license from the Zed project name.
Nineteen crates with omitted published notice texts (including `velopack 1.2.161`,
pinned to the release commit recorded in `vpk.nuspec`) have supplemental files fetched
from the exact upstream commit; the checked-in manifest pins URLs and SHA-256. The
builder validates these stored files offline.

Three evidence gaps remain: **mac 0.1.1, seahash 4.1.0, simd_helpers 0.1.0**. Metadata
declarations alone do not close them. `option-ext 0.2.0` declares MPL-2.0; its license
and corresponding source archive are carried for review. PDF Studio's own license
is unset. No license is invented on the owner's behalf; public redistribution remains
blocked on owner/third-party review, including native transitive notices and Microsoft
entitlement. Technical collection does not certify legal compliance.

Microsoft documents app-local DLL deployment and restricts redistribution to the
applicable license terms: [DLL selection](https://learn.microsoft.com/en-us/cpp/windows/determining-which-dlls-to-redistribute?view=msvc-170),
[VS 2022 redistribution list](https://learn.microsoft.com/en-us/visualstudio/releases/2022/redistribution).
App-local runtime servicing is the application's responsibility. The executable also
imports OS ICU; target acceptance is Windows 10 22H2 and Windows 11 x64 with working
D3D11, rather than claiming arbitrary Windows versions. See [Windows ICU](https://learn.microsoft.com/en-us/windows/win32/intl/international-components-for-unicode--icu-).

## Advisory review

`cargo audit` is not installed; no toolchain installation was needed. The explicit
OSV script checks registry names/versions in batches, validates response counts and
follows pagination. API failure is an error, never a clean result. See [OSV API](https://google.github.io/osv.dev/post-v1-querybatch/).

Initial scan: 463 registry versions, five matches. `anyhow 1.0.100` was affected by
[RUSTSEC-2026-0190](https://rustsec.org/advisories/RUSTSEC-2026-0190.html); the exact
workspace pin/lock is upgraded to **1.0.103**, the patched boundary. No direct
`downcast_mut` calls were found in application/workspace sources; the dependency was
updated rather than assuming transitive callers are safe.

The remaining four matches are unmaintained notices, with no patched version:

| Crate | Advisory | Desktop dependency path |
|---|---|---|
| instant 0.1.13 | [RUSTSEC-2024-0384](https://rustsec.org/advisories/RUSTSEC-2024-0384.html) | GPUI base/component |
| paste 1.0.15 | [RUSTSEC-2024-0436](https://rustsec.org/advisories/RUSTSEC-2024-0436.html) | Transitive macro dependency |
| rustybuzz 0.20.1 | [RUSTSEC-2026-0206](https://rustsec.org/advisories/RUSTSEC-2026-0206.html) | GPUI SVG shaping |
| ttf-parser 0.25.1 | [RUSTSEC-2026-0192](https://rustsec.org/advisories/RUSTSEC-2026-0192.html) | GPUI/fontdb/SVG shaping |

These are not silently suppressed. Replacing the pinned UI/font stack requires a
separate compatibility change. OSV registry matching does not audit native PDFium,
QPDF, Windows, MSVC or exploit reachability and is not a vulnerability-free claim.

## Completion boundary

The packaging/verification workflow is implemented. **Production release remains open**:
notice/owner review, native/dependency security disposition, separate clean Windows
machines and the physical input/DPI/Narrator matrix. See [release checklist](release-checklist.md),
[device acceptance](device-acceptance.md) and actual [test evidence](testing.md).
Git commit/push publishes reviewed source changes; it does not publish the ignored ZIP.
