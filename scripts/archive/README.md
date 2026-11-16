# Historical UI acceptance snapshot

`m3-single-workspace-ui.ps1.txt` is a byte-identical copy of the M3 UI suite from
baseline commit `582cd67`, captured before multi-tab migration. The `.txt` extension
is intentional: its Open-replacement assumptions and old control labels are
historical, so it must not be run as current acceptance.

Run `../m3-ui-smoke.ps1` for current behavior. The old-to-new assertion map is in
[`docs/m3-ui-coverage.md`](../../docs/m3-ui-coverage.md). Git history and this snapshot
retain the original assertions for auditing; the current suite continues the native
export, consent, output inspection, failure, source-hash and cleanup checks.
