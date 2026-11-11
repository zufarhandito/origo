# Text selection before M2

This is an owner-requested extension to the original M1 viewer scope. Raster preview
remains the visual page. A separate PDFium extraction port provides owned characters
and character rectangles; an overlay handles selection and highlights without drawing
the text a second time. Source files remain read-only.

## Use

- A single click clears selection without selecting a character. Hold and drag to
  select after moving at least 4 logical pixels to another character.
- Double-click selects a sentence. Dragging after that extends by whole sentences.
  Labels without sentence punctuation fall back to the extracted line.
- Ctrl+C or the Salin button copies the selection as plain text.
- Esc or clicking blank page space clears the selection.
- Selection survives zoom and scrolling. Source rotations of 0/90/180/270 degrees
  are mapped with PDFium's own coordinate conversion.
- Drag can cross pages that are simultaneously visible. There is no automatic
  scrolling at viewport edges yet. Separate word/paragraph selection modes, Select All,
  search and OCR are not part of this slice.

Image-only scans contain no text to select. Clicking such a page reports that OCR is
needed; the application does not infer or invent text from the image. Unusual encodings,
ligatures, columns, rotated content and missing Unicode mappings can affect copied
text and reading order. Sentence boundaries use punctuation/blank lines, with decimal
numbers handled; abbreviations and language-specific segmentation remain approximate.
PDFium may return lines in a different order after source rotation. Highlight rectangles
are axis-aligned character boxes, not glyph outlines.

## Loading and bounds

Visible pages lazily request text on the existing dedicated native worker. One text
request is in flight, independent of the bounded raster queue. Text results survive
raster zoom-generation changes but are accepted only for the same current SourceId.
Opening another source or detecting a changed source clears text/selection state.

The adapter checks a 200,000-character per-page cap before creating the character
index vector. Character data uses a separate 16 MiB LRU; bitmap budgets remain
128/16 MiB. These are data budgets, not total process memory caps. Character rectangle
coordinates are normalized to the displayed page using a virtual 16384-square render
configuration for coordinate conversion only; no bitmap is allocated for extraction.
Zoom transforms cached rectangles rather than extracting text again. Adjacent boxes
are merged for highlight painting where their rows/columns align.

Copy is limited to 1 MiB UTF-8 text. A selection whose earlier pages have been evicted
must be narrowed/reselected; copy reports the missing cache instead of silently omitting
pages. Clipboard changes happen only on the user's copy action. Native calls retain
the existing serialization, source fingerprint checks, idle release and session recycling;
text operations also count toward the 64-operation recycle threshold. Native parsing
still has no hard timeout/process sandbox.

## Actually verified

- Workspace unit tests: 16 passed. Tests cover Unicode/newline retention, reverse
  multi-page ranges, hit testing without text, highlight rectangle merging, and sentence
  boundaries with wrapped lines, decimal numbers and unpunctuated labels.
- Workspace denied-warning Clippy and formatting: passed. Debug/release builds passed.
- Real release native smoke: exact generated text/newlines, all four source rotations,
  every visible glyph rectangle overlapping actual raster ink, empty image-only scan,
  and changed-source text rejection passed. This does not establish complex-layout
  reading order or all-font Unicode compatibility.
- Headed release UI: forward/reverse drag, exact clipboard text, four rotated pages,
  selection after zoom with no extra text loads, drag/copy across two visible pages,
  blank-click clearing, disabled copy with empty selection and unchanged fixture hashes
  passed. Text cache contained 1824 bytes after four fixture pages, loaded once each.
- Synthetic screenshot confirmed the highlight is painted over the selected text.
  An initial failed run found an absolute canvas below the image; explicit top/left
  positioning fixed it. The viewport's Ctrl+scroll probe was aligned the same way.

Final selection evidence: ignored .research/text-ui-4225477efef6436e95f5ae2a5c267bbd.
The later click-behavior regression passed in
.research/text-ui-9beba3501152498bad944cc76a8d252f: single click and 1px jitter do not
select, native double-click selects the full sentence/label, and forward/reverse drag,
hover updates during a press, four rotations, zoom reuse and cross-page copying pass.
Selection starts after 4px movement to another character. A press ends on mouse-up
even outside page bounds or on window deactivation; buttonless hover/repaint events
do not cancel it. Earlier intermediate drag runs timed out before this state fix;
only the final pass is counted. This latest release test window is left open.
Final debug empty-start/navigation/ordinary-scroll/50-400%-zoom/resize regression also
passed: .research/m1-ui-13f9daebe00440249c16a5de48130d1f. The debug test window was
closed and the tested release selection window left open.
Keyboard shortcut delivery, physical pinch, mixed-monitor DPI and complex real-document
selection remain manual acceptance; the clipboard smoke invokes the shared copy action
through its UI button and uses targeted HWND mouse messages, not global input.

## Reproduce

Build the workspace and stage sidecars, then:

```powershell
cargo run --locked -p pdf-smoke -- --write-fixtures
cargo run --locked -p pdf-smoke
./scripts/text-selection-smoke.ps1 -Profile release
```

The native smoke writes numeric character-position probes for its own generated
fixture to .research/text-selection/probe.json. The UI smoke copies that generated
fixture into its own directory, saves/restores existing clipboard formats without
printing their contents, and leaves the test viewer open. It requires an interactive
Windows GPU desktop. Optional application metrics add textCacheBytes, textLoads,
textLoading and selectedChars (single-page count only); no text content is logged.
