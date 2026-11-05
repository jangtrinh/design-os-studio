# Spike 1526: Notion-style block editor in egui

Branch `spike/1526-block-editor` (base 53860a64, not pushed). Code: `spikes/block-editor/` (THROWAWAY, 1,759 lines incl. 11 unit tests and the scripted scenario). Captures: `plans/reports/spike-1526-evidence/`. Epic #1522, phase 04.

## NOT verified (read first)

1. **Typing with a human keyboard.** Real macOS Vietnamese Simple Telex was driven by synthetic HID key events (Swift `CGEventPost`), not by a person. Another app (`Grok Bot`) took focus several times; I guarded every key on the frontmost app. Three runs, two with usable traces. Composed text landed correctly in both backends, but the word-ending space never reached egui (see d, finding D4), and a final "thẻ" composition was lost to my own Cmd+Shift+P. Owner should type "Thanh toán bằng thẻ" once by hand in each mode.
2. **Accessibility (AccessKit) of the custom surface.** Not built, not tested. Spec §13 needs accessible names.
3. **Large pages.** No test above 9 blocks. The spike re-lays out every block every frame and clones the whole doc for undo.
4. **Mouse selection, multi-click, drag across blocks.** Custom backend has click + drag inside a block, untested by script. Selection across blocks does not exist in either backend.
5. **Everything in "Not built"** at the end (tables, embeds, toolbar, drag, shortcuts, save/conflict).
6. `cargo test --workspace --lib` on the main workspace: **skipped**. `git diff HEAD -- crates Cargo.toml Cargo.lock` is empty (0 files), so the workspace is byte-identical to the base; a full workspace build would need a fresh multi-GB target for no signal. `cargo metadata` confirms 13 workspace members, spike not one of them.
7. The 8-10 d estimate below is INFERENCE, not measured.

## Conclusion

Build the editor on **one custom editor surface over egui galleys (backend B)**, not one `TextEdit` per block (backend A). Both pass (a)-(c), (e), (f) with synthetic events and look identical. The difference is in what each one costs to make correct:

| | A: `TextEdit` per block | B: custom surface over galleys |
|---|---|---|
| IME composition kept out of the document (d1) | **FAIL**: preedit is written into the block string | PASS: kept in `App::preedit`, drawn underlined |
| Know whether a composition is open | Cannot: `TextEditState::cursor_purpose` is `pub(crate)`; must infer from raw `Event::Ime` | Own state |
| Structural keys (Enter, Backspace at 0, arrows at a row edge) | Must delete events from the input queue before the widget runs (`take_key`); `consume_key(NONE, ..)` also eats Shift+Arrow | Own key handler |
| Inline runs follow edits | Diff old/new string (prefix/suffix) per frame, then shift runs | Exact: edit ops carry the range |
| Undo | Widget has its own undoer: must be bypassed | Single doc-level log |
| Caret crossing blocks | New widget gets focus: IME session restarts each crossing | Same `Id` keeps focus; `should_interrupt_composition` flag exists for deliberate cuts |
| Selection across blocks (spec §3.1 Shift+arrow, §3.4 Esc/Cmd+A) | Not possible without a second layer | One selection model |
| Free from egui | multi-click, word jump, clipboard, accessibility | none: must be written or reused (below) |

Weakest link: B gives up AccessKit and multi-click for free. Whether that cost is acceptable is an owner call (open question 1).

## Evidence for (a)-(f)

Labels: FACT = observed this session. Scripted = synthetic `egui::Event` injected through eframe `raw_input_hook` into the real window (same path as OS events from egui down; skips winit). Run: `cargo run -- --mode custom|textedit --script --shots DIR`. Result: custom **22/22 PASS**; textedit **21/22 PASS**, only d1 fails.

**(a) Caret across blocks.** FACT, both modes. Checks a0-a7: ArrowUp on the first row goes to the previous block; ArrowDown inside a wrapped block stays in it (row 0 to 1, 2 rows); ArrowDown on the last row enters the next block; ArrowUp enters a wrapped block on its **last** row; Left at start / Right at end cross to the previous block end / next block start. Bug found and fixed on the way: after ArrowDown the caret was stored as a bare char index, so a soft-wrap boundary (one offset, two rows) put it back on row 0. egui's `CCursor::prefer_next_row` must be stored with the caret. Studio's canvas text hit the same ambiguity and solved it with `hint_x` (`canvas/textsession.rs:38`).

**(b) Split / merge.** FACT, both. Enter at char 31 of the intro paragraph splits it, caret at 0 of the tail (b1); Backspace at start merges it back to the identical text with the caret at 31 (b2); heading/list Backspace first turns into Text (unit test). Captures: `custom/1-split.png`, `custom/2-merge.png` (and `textedit/`).

**(c) `/` menu.** FACT, both. `/` opens it under the caret (c1); `head` filters to the 3 headings, accent-insensitive (c2); Down + Enter inserts Heading 2 and removes the `/head` text (c3); Esc handling (keeps the typed text, does not reopen) is coded but NOT covered by the script. Captures: `custom/3-slash-menu.png` (unfiltered, list clipped mid-row at 328 px as in m04b), `custom/4-slash-filtered.png`. Not done: second and third sections (Design, Data) are rows without action.

**(d) Vietnamese Telex.** Two kinds of evidence, kept apart.
- Scripted synthetic `Event::Ime` Preedit/Commit, words "Thanh toán bằng thẻ": text equals `Thanh toán bằng thẻ` in both modes (d2). Capture `custom/5-vietnamese.png`.
- **Real macOS IME** (input source `com.apple.inputmethod.VietnameseIM.VietnameseSimpleTelex`, driven by HID key events into the packaged spike window, `--log-ime` trace). FACT, both modes: egui receives `Preedit{text:"t"}`, `"to"`, `"toa"`, `"toan"`, `"toán"` (tone key `s` rewrites the preedit), then `Preedit{""}` + `Commit("toán")` + `Preedit{""}`; `bằng` goes `b, ba, bă, bằ, bằn, bằng`. Commits are NFC precomposed (U+00E1, U+1EB1). Composed text reached the document in custom (`aThanhtoánbằng`) and textedit (`toánbằng`): `real-ime/real-ime-1.png`, `real-ime-textedit/real-ime-1.png`.
- FACT d1 (the decisive one): while "toán" is still composing, the TextEdit backend's document (FACT: its Markdown dump; INFERENCE: so would autosave and undo snapshots) contains "toán"; custom does not.
- Findings D1-D4 (FACT unless marked):
  - D1 `Event::Ime` only flows after the frame that sets `PlatformOutput::ime`. A key pressed before that arrives as plain `Event::Text`. The custom surface must set `ime` every frame it is focused (as `canvas/draw.rs:1267` does).
  - D2 egui drops widget focus while the window is inactive; the custom surface must re-request it, otherwise `ime` is never set and Telex falls back to raw Latin letters. FACT: my first real run showed only `Event::Text` and zero `Event::Ime`; INFERENCE: the cause was lost focus, because re-requesting focus fixed it.
  - D3 Cmd+Shift+P during a composition cancels it (`Preedit{""}`) and no `Key` event arrives. Shortcuts cannot be assumed to work mid-composition.
  - D4 The space that ends a word never reaches egui as `Text` and is not inside the `Commit` (words glued: `Thanhtoánbằng`), identically in both backends. So it is upstream of our code (macOS, winit or egui-winit) or an artefact of synthetic HID keys. UNKNOWN which. Needs a human-keyboard check before the build.
  - D5 Typing faster than ~35 ms per key stalled the IME after two words (twice); at 90 ms it ran through. Likely an artefact of synthetic input.

**(e) Undo of one user action.** FACT, both. Cmd+Z undoes the merge (e1), then the split (e2); Cmd+Shift+Z redoes (e3); typing `abc` as three events inside one second is one entry and one Cmd+Z empties the block (e4, e5). Entries are doc snapshots recorded per action; typing and deleting coalesce on a 1 s pause like `textedit.rs::UndoLog`. Bug I introduced and fixed: an undo run in the same frame as the doc-diff recorder was recorded as a new edit, which cleared redo.

**(f) Bold, both approaches.** FACT, both. Select "Thanh" with Shift+Right x5, Cmd+B: bold range 0..5, Markdown `**Thanh** toán bằng thẻ` (f1). Bold is a different `FontId` (Inter variable-font instance at wght 600, same recipe as `theme.rs::install_fonts`), not synthetic bold. Captures `custom/6-bold.png`, `textedit/6-bold.png` look the same. Costs differ:
- A needs a `layouter` closure plus a string diff per frame to move runs, and the diff also sees preedit churn.
- B builds the `LayoutJob` itself and applies edits with exact ranges (`Block::replace`, unit-tested including typing inside a bold run).
- Both need the same extra for "bold with no selection" (pending style at the caret): not built (open question 3).

**Idle CPU** (CLAUDE.md invariant; debug build, M-series Mac, window open and focused, `ps -o time=` over two 20 s windows): custom 0.27 s to 0.27 s to 0.28 s; textedit 0.25 to 0.35 s (first window) then 0.35 to 0.35 s. Cursor blink is off (`visuals.text_cursor.blink = false`); with it on, egui schedules a repaint twice a second.

## Reuse in the real build

Correction to the plan: `agent_composer_text.rs` (47 lines) holds composer placeholder strings and caption logic only. It has no caret or IME code. The reusable code is:

| What | Where | Use |
|---|---|---|
| Grapheme / word / paragraph navigation, `clean_input` | `crates/designos-app/src/textedit.rs:8-91` | Backspace/Delete by grapheme (spike deletes by char: wrong for decomposed input), Option+arrows, double/triple click, paste |
| Undo log with 1 s burst coalescing | `textedit.rs:94-175` (`UndoLog`, `starts_new_word`) | Same semantics; it is concrete over `TextProps`, so make it generic over the snapshot type |
| IME preedit/commit/key loop, caret rect for the candidate window | `canvas/textsession.rs:703-740` (preedit, commit), `:743-781` (`text_keys`), `:831` (`caret_screen_rect`), `canvas/draw.rs:1267-1269` (`o.ime = IMEOutput`) | Shape of the loop; the canvas version splices the preedit into the content, the block editor should not (d1) |
| Rich-text run algebra | `crates/designos-app/src/richtext.rs:20-229` (`segments`, `splice`, `apply_style`), `:339-399` (lists: `toggle_list`, `change_level`, `exit_empty_item`, `backspace_at_item_start`) | Candidate to replace `Block.bold: Vec<Range>` with `TextProps` runs, so bold/italic/code/link share the canvas model. Cost: it is over bytes, egui cursors are chars |
| Cosmic-text caret and hit testing | `crates/designos-app/src/textmeasure.rs:125` (`caret_rect_hint`), `:210` (`hit_stop`), `:285-307` (`move_visual`, `move_vertical`) | Use instead of egui galleys if blocks must match canvas text rendering or need RTL. egui galleys were enough for the spike |
| Stock `TextEdit` precedent | `crates/designos-app/src/agent_composer.rs:1117` | Keep `TextEdit` for single-style fields (Mermaid source, link field, search) |

Facts that contradict the plan or CLAUDE.md: `crates/designos-app/Cargo.toml` pins egui/eframe **0.35**, not 0.31 (so `ImeEvent::Preedit` has `active_range_chars`, `IMEOutput` has `should_interrupt_composition`). `designos-app` is bin-only (no `[lib]`), which is irrelevant for the build (editor lives inside it) but is why the spike copied theme constants.

## Risks

1. **Accessibility** of the custom surface (above). Mitigation: emit AccessKit text-run nodes per block; budget 1-2 d.
2. **Performance on long pages.** Per-frame layout of every block and `Doc` clone per undo entry will not hold at hundreds of blocks (and Studio's 8 ms frame budget). Needs galley cache keyed by (block rev, width), height cache with viewport culling, and structural-sharing or op-based undo.
3. **IME**: D4 (space), D3 (shortcuts mid-composition), D1/D2. Mitigated by owner hand test and a `ime` output set every focused frame.
4. **Soft-wrap caret ambiguity** (`prefer_next_row`) is easy to get wrong; keep a unit test per entry path.
5. **Mixed models**: if runs move to `TextProps` (byte offsets) while egui cursors are chars, every boundary needs conversion; pick one index type at the editor boundary.
6. **Round-trip to Markdown** was only exercised as a serializer (`**bold**`, lists, code fence) in unit tests; parsing and phase 01's exact output were not available to compare.

## Estimate for the build

Plan: 8-10 d. Revised **10-12 d** with the custom surface (INFERENCE). The spike's custom core already covers: surface and focus, caret and selection in a block, preedit, split/merge, slash menu, doc-level undo, bold, list/heading/callout/code painting, ~2 d of the original 8-10. Added by what the spike showed: AccessKit 1-2 d, galley/height cache and culling 1-2 d, cross-block selection 1 d. Remaining plan items untouched by the spike: tables with cell navigation and horizontal scroll (2 d), inline toolbar and link popover (1 d), block handle drag and menu (1 d), Markdown shortcuts and list indent (0.5 d), Markdown round trip with phase 01 (1 d), save debounce and conflict banner (1 d), embeds and Mermaid hooks (0.5 d), tests and captures (1.5 d). With backend A the same list is about 8-10 d (no a11y or selection work) but carries the d1 workaround permanently and cannot do cross-block selection; I do not recommend it.

## Open questions for the owner

1. Is losing free AccessKit and multi-click selection acceptable for a custom surface, with an a11y task of 1-2 d inside phase 04 (spec §13 requires accessible names)?
2. Hand test needed: type "toans bawfng thẻ" with a real keyboard in `--mode custom` and `--mode textedit`. Do spaces after composed words appear (D4)?
3. Cmd+B with a collapsed caret (typing in bold): required in v1? The spec lists the shortcut but not the behaviour.
4. Is Shift+arrow selection across blocks (spec §3.1) and Cmd+A twice (§3.4) required in v1? It decides A vs B on its own.
5. Should undo history survive switching pages, and how do agent steps (phase 06, one step per tool call) interleave with typing in one log?
6. Keep inline model as the canvas `TextProps` runs (reuse, byte offsets) or a small block-local run list (spike)?

## Not built in the spike

Block handle drag and block menu (the `+` button inserts a block below and opens the menu; the grip is painted only), selection toolbar and link popover, Markdown shortcuts (`# `, `- `), list indent/outdent, tables, embeds, Mermaid, divider/frame/state/table slash actions, multi-click selection, cross-block selection, copy of blocks, save/conflict, agent co-editing, accessibility, narrow-window behaviour.

## Reproduce

```bash
export PATH="$HOME/.cargo/bin:$PATH"
cd spikes/block-editor
cargo build            # exit 0 (last run: Finished dev profile in 0.12 s, 1 dead-code warning)
cargo test             # 11 passed
cargo run --release -- --mode custom --script --shots /tmp/shots    # RESULT pass=22 fail=0
cargo run --release -- --mode textedit --script --shots /tmp/shots  # RESULT pass=21 fail=1 (d1)
cargo run --release -- --mode custom --log-ime                      # interactive, real IME trace
```

Captures (2x retina, 2360x1640, all under 300 KB): `custom/{1-split,2-merge,3-slash-menu,4-slash-filtered,5-vietnamese,6-bold}.png`, the same six under `textedit/`, `real-ime/real-ime-1.png` and `real-ime-textedit/real-ime-1.png` (1180x820, real IME text).
