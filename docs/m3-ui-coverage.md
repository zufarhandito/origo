# M3 UI migration: coverage map

Baseline `582cd67`. The original script is preserved byte-for-byte in
`scripts/archive/m3-single-workspace-ui.ps1.txt`; it is historical and not runnable
acceptance. Current entry point stays `scripts/m3-ui-smoke.ps1`.

## Lifecycle mapping

| Original single-workspace case | Current multi-tab check |
|---|---|
| Cancel/invalid Open preserves dirty workspace | Cancel/invalid Open keeps A; valid Open creates independent B; switching back preserves A's identity, revision, selection and plan |
| Unsaved Open prompt Cancel/Esc/focus/edit locks/repeated intent | Same assertions on dirty tab Close; repeated window-close cannot replace the current tab-close intent |
| Changed replacement candidate | Changed B is invalidated independently; A remains intact; B can reopen with a fresh TabId. Native source before/after checks remain in native smoke |
| Cancel Save As before leaving | Cancel Save As before tab Close retains both tabs and dirty source |
| Empty edited plan cannot save | Retained on tab Close, then Undo restores plan and selection |
| Save-before-Open only continues after current receipt | Save-before-tab-Close removes only its origin after committed current receipt; other document remains |
| Explicit window discard/restart | Retained real window Close/Discard and restart; identify only the owned process by executable and fixture GUID |
| Failed Save before replacement | Failed Save before tab Close preserves dirty origin and another already-open tab |
| Final window Close Save/consent refusal/success | Retained, including validation and cleanup before owned process exit |

TabId is now part of `Assert-SameWorkspace`; equal revision numbers from different
documents cannot accidentally satisfy the assertion. Export cases explicitly close
their previous tab before opening an isolated input instead of assuming Open replaces it.

## Export assertions retained

- Multi-source import/reorder/delete/rotation, exact seven-page order/rotation through
  QPDF JSON, validation, Undo/Redo to the clean saved revision.
- Selected Extract count and dirty baseline; cancelled native Save As picker.
- Metadata refusal/acceptance, close-wait and cancelling only the close intent.
- Ordinary/named/bookmark+metadata refusal/acceptance, omission, rotation and source hash.
- Byte-identical whole-source tagged/empty-form Extract.
- Partial tagged Extract refusal/acceptance, tag cleanup, passive Ink objects/geometry
  and source hash; Extract completion cannot authorize discarding dirty edits.
- Forms/tagged/page-label/unknown-catalog blockers, selectable error message and
  clipboard copy. Expected text includes the new document-name prefix.
- Invalid import/Open preserve session; failed pending Save preserves document.
- All source hashes, existing-output protection and owned staging cleanup.

Each completed case prints a named PASS and writes `cases.json` in its owned
`.research/m3-ui-*` directory. Only completed cases count as evidence; a partially
successful run is not full acceptance. Clipboard is restored and cleanup targets
only the process matching the fixture's path/GUID. See [testing](testing.md) for runs.

The migrated release run completed **25 distinct cases**. Existing-destination
acceptance handles IFileDialog's native overwrite confirmation using native Button
enumeration filtered to the owned PID; the application must still reject the path
and preserve the destination hash. This avoids querying transient native UIA surfaces
without skipping the no-overwrite assertion.
