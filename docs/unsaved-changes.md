# Unsaved changes: Open and Close

The desktop now uses [multi-tab sessions](multitab.md). Open adds/focuses a tab;
it does not replace a dirty document. Closing a tab retains the receipt-gated
Save As/discard/cancel flow. Window close checks tabs left to right and removes none
until the entire batch succeeds. Cancel preserves all tabs; prior saves remain clean.

Tab close and normal window close (X/Alt+F4) protect virtual page edits. The existing
revision/clean-baseline model is authoritative; navigation, zoom, text/page selection
and mode changes do not dirty a document. Undo to the saved revision is clean.

## Open

The shell inspects selected files off-thread, reserves up to eight loading/loaded
tabs, rechecks fingerprints and publishes successful DocumentSessions. Existing
tabs survive partial failure/cancellation. Duplicate primary paths focus the
existing tab without resetting its history or edits; Open never asks to discard it.
Failures are summarized in the shell. Source changes invalidate only the affected
session when a fresh native read detects them; cached pages are not live-watched.
Fingerprints use size/mtime and are not cryptographic identity protection.

## Unsaved decision

If a tab being closed is dirty, show **Simpan perubahan sebelum melanjutkan?**:

- **Simpan sebagai** exports the full current plan to a new PDF. Continue only when
  the committed receipt matches the workspace instance and revision. Native save
  cancellation, declined loss consent and export failure cancel the pending action.
- **Buang perubahan** continues without saving the old virtual plan.
- **Batal**, Escape or the dialog close button keeps the workspace. The backdrop
  does not dismiss the alert. Initial keyboard focus is on Batal.

An empty edited workspace still prompts, but saving is disabled with an explanation
that at least one page is required. Cancel allows Undo/import to restore pages.

## Active work and interaction

Only one close batch exists per window. Repeated Open/Close requests do not stack
dialogs or replace an in-flight intent. Page mutations, import, history and additional
exports are blocked during the transition, including keyboard and drag handlers.
Reading/zoom remain available outside the modal. Active imports must finish before
a close batch is accepted. Open can add independent sessions without replacing the
session that owns a background operation.

If export is already running, wait for completion without cancelling it. **Tetap di
aplikasi** cancels only the pending Close batch; **Batal ekspor** still cancels the job.
After success, recheck dirty: Extract, Split, Optimize and stale snapshots cannot mark current edits
saved. Failed/cancelled jobs never close the viewer or replace its workspace.

The GPUI Kit alert shares the operational-modal guard, including global text-canvas
listeners. Successful Save As keeps original source references and Undo/Redo history.
Sources and existing destinations remain protected by the existing export policy.

This feature covers ordinary tab/window close, not autosave, crash recovery,
forced process termination, OS shutdown or recovery after external source changes.
It adds no dependencies, format converters or broader PDF export compatibility.

## Verification

Transition unit tests cover successful/current receipts, failed/cancelled jobs and
async token invalidation. `scripts/tabs-ui-smoke.ps1 -Profile release` covers the
current multi-document lifecycle using owned synthetic fixtures and test HWNDs.
`scripts/m3-ui-smoke.ps1` now checks Open isolation, tab/window Close, failed/cancelled
saves, receipt-gated continuation and the existing native export/consent assertions.
The original single-workspace script is archived; see [coverage map](m3-ui-coverage.md).
See testing.md for actual executions and evidence.
