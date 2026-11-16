# Unsaved changes: Open and Close

The desktop now uses [multi-tab sessions](multitab.md). Open adds/focuses a tab;
it does not replace a dirty document. Closing a tab retains the receipt-gated
Save As/discard/cancel flow. Window close checks tabs left to right and removes none
until the entire batch succeeds. Cancel preserves all tabs; prior saves remain clean.

Tab close and normal window close (X/Alt+F4) protect virtual page edits. The existing
revision/clean-baseline model is authoritative; navigation, zoom, text/page selection
and mode changes do not dirty a document. Undo to the saved revision is clean.

## Open (historical replacement flow)

The following describes the older single-workspace implementation. The current
shell instead inspects new files off-thread, reserves up to eight loading/loaded
tabs, rechecks fingerprints and publishes successful DocumentSessions. Existing tabs
survive partial failure/cancellation. Duplicate primary paths focus the existing tab.

Choose the replacement PDF first. Inspect it in the background as a candidate,
without clearing the current workspace or advancing its identity. Cancelling the
picker or failing inspection preserves the current page plan, history and reading
state. Errors use the existing selectable GPUI Kit error popup.

Before installing the candidate, recheck its size/mtime in the background. If it has
changed or disappeared, keep the old workspace and ask the user to choose again.
Successful replacement resets the view and invalidates old asynchronous results once.
This is ordinary source-change detection, not cryptographic identity protection.

## Unsaved decision

If the old workspace is dirty, show **Simpan perubahan sebelum melanjutkan?**:

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
Reading/zoom remain available outside the modal. An active import must finish before
another Open/Close request is accepted.

If export is already running, wait for completion without cancelling it. **Tetap di
aplikasi** cancels only the pending Open/Close; **Batal ekspor** still cancels the job.
After success, recheck dirty: Extract, Split, Optimize and stale snapshots cannot mark current edits
saved. Failed/cancelled jobs never close the viewer or replace its workspace.

The GPUI Kit alert shares the operational-modal guard, including global text-canvas
listeners. Successful Save As keeps original source references and Undo/Redo history.
Sources and existing destinations remain protected by the existing export policy.

This feature covers ordinary Open/window close, not autosave, crash recovery,
forced process termination, OS shutdown or recovery after external source changes.
It adds no dependencies, format converters or broader PDF export compatibility.

## Verification

Transition unit tests cover successful/current receipts, failed/cancelled jobs and
async token invalidation. `scripts/tabs-ui-smoke.ps1 -Profile release` covers the
current multi-document lifecycle using owned synthetic fixtures and test HWNDs.
The older `m3-ui-smoke.ps1` Open-replacement scenarios remain historical and need migration.
See testing.md for actual executions and evidence.
