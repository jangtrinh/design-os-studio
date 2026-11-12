# E12 Canvas, accessibility & text: triage and first slice (epic #124)

Written 2026-11-06 23:xx (Asia/Saigon) by the E12 planner. Read-only on GitHub: no issue, label or PR was edited.

- **Code baseline:** `origin/main@d22dad970` ("feat(app): live connectors in design files…", #1653). Every `file:line` below is cited at that commit. The local checkout is older (`ec47da26d`), so do not trust line numbers read from the working tree.
- **Epic state:** 78 sub-issues, 24 closed, **54 open**. All 54 are triaged below.
- **Open PRs:** none of the 65 open PRs names one of the 54 issues. Draft **#1667** overlaps by meaning: it makes MCP edits re-solve only what they touch, and it re-measures hug text on a collection mode change. That covers #417, part of #1404 and the area of #997 (see the rows).
- **Stale facts in this repo's docs:** CLAUDE.md says `egui 0.31`. `Cargo.lock` at the baseline pins `egui 0.35.0`, `eframe 0.35.0`, `accesskit 0.24.1` and `accesskit_macos 0.26.3`. The specs use the lockfile versions.

## First slice: "S1 Input you can trust: keyboard, pointer and screen reader"

**Outcome.** A press, a drag, a key and a screen reader all do what the user meant:
- drags are never lost or dropped in the wrong place;
- menus pick on release everywhere;
- Escape in a field only leaves the field;
- the keyboard can pick from a toolbar menu;
- the shortcuts list is complete and readable;
- VoiceOver and automation always find a focused element, and hear expanded and selected states.

**Why this slice:**
- Nine small-to-medium bugs with exact repros and acceptance criteria.
- Each fix extends a pattern main already has: `primary_gesture` (`widgets/menu.rs:151`), press-and-release drags (`panels/layers.rs:352-405`), `canvas_id()` (`agent_a11y.rs:68`) and `widgets::labelled` (`widgets/mod.rs:208`).
- It needs no hot-file slot and no GPU hot path. It unblocks the v0.39 Accessible Studio pack: #386 and #387 build on #1019, and #387's "shortcut reference" criterion is the #869 dialog.

| Issue | Spec | Size (builder h) | Tier | Depends on (file ownership) | Ready after owner answers? |
|---|---|---|---|---|---|
| #951 | [951-layers-two-drags-in-one-slow-frame.md](951-layers-two-drags-in-one-slow-frame.md) | 1 | Grok build-fast | none | Ready now |
| #969 | [969-layers-no-drop-indicator-where-the-drop-is-refused.md](969-layers-no-drop-indicator-where-the-drop-is-refused.md) | 3 | Argon | #951 (same builder, same file `panels/layers.rs`) | After Q5 |
| #915 | [915-pages-and-home-drags-start-from-the-press.md](915-pages-and-home-drags-start-from-the-press.md) | 3 | Argon | none | Ready now |
| #904 | [904-board-popovers-zoom-menu-and-layer-picker-pick-on-release.md](904-board-popovers-zoom-menu-and-layer-picker-pick-on-release.md) | 4 | Argon | none | Ready now |
| #935 | [935-escape-in-a-panel-field-keeps-the-selection.md](935-escape-in-a-panel-field-keeps-the-selection.md) | 4 | Claude Sonnet | none | Ready now |
| #828 | [828-screen-readers-hear-expanded-and-selected-states.md](828-screen-readers-hear-expanded-and-selected-states.md) | 3 | Argon | none | After Q7 |
| #944 | [944-keyboard-pick-in-toolbar-menus-and-slide-to-pick-in-popovers.md](944-keyboard-pick-in-toolbar-menus-and-slide-to-pick-in-popovers.md) | 5 | Claude Sonnet | #904 (`widgets/menu.rs`) | Ready now |
| #1019 | [1019-the-canvas-is-the-focused-element-when-no-widget-has-focus.md](1019-the-canvas-is-the-focused-element-when-no-widget-has-focus.md) | 8 | Claude Sonnet | #935 (`shell.rs`), #828 (`agent_a11y.rs`, `widgets/mod.rs`) | After Q6 |
| #869 | [869-keyboard-shortcuts-dialog-lists-every-binding.md](869-keyboard-shortcuts-dialog-lists-every-binding.md) | 8 | Claude Sonnet | #904 (`panels/menus.rs`), #944 (`widgets/menu.rs`, `panels/toolbar.rs`), #1019 and #828 (`agent_a11y.rs`) | After Q1–Q4 |

**Total:** about 39 builder hours.

**Tiers:**
- **Argon:** reversible, with a machine check.
- **Grok build-fast:** a one-line change with a local test. A Claude reviewer reviews it, never Grok.
- **Sonnet:** focus and key routing, shared field state, or UI copy and layout. Those need judgment.

### Waves (no two parallel legs touch the same file)

| Wave | Legs in parallel | Files owned |
|---|---|---|
| 1 | A: #951 then #969 (one builder) | `panels/layers.rs`, `ops.rs` (new pure predicate only), new `shell/tests/layers_drop_refusal.rs` |
| 1 | B: #915 | `panels/left.rs`, `home/main_view.rs`, `home/mod.rs`, new `shell/tests/pages_quick_drag.rs` |
| 1 | C: #904 | `board/toolbar.rs`, `panels/menus.rs` (`show_zoom_menu` only), `panels/context.rs`, `widgets/menu.rs` (visibility of `primary_gesture` only) |
| 1 | D: #935 | `shell.rs` (Escape arm only), `widgets/field.rs`, `widgets/field_tab.rs`, new `shell/tests/field_escape.rs` |
| 1 | E: #828 | `widgets/mod.rs` (new state helper), `panels/check_designs.rs`, `agent_a11y.rs` (`label_agents_panel` only) |
| 2 | F: #944, after C merges | `panels/toolbar.rs`, `widgets/menu.rs` |
| 2 | G: #1019, after D and E merge | `agent_a11y.rs`, `canvas/mod.rs`, `shell.rs`, the 9 focus-query sites listed in its spec |
| 3 | H: #869, after C, E, F and G merge | `home/dialog.rs`, `panels/menus.rs`, `canvas/tools.rs`, new `shortcuts.rs`, `agent_a11y.rs` (label constant only). With Q2 (a) or (b) also `widgets/menu.rs` (`MenuItem` shortcut text), `panels/toolbar.rs` (tooltips) and `cmdk.rs` (palette glyphs) |

**Conflict rule.** If leg F finds it must edit `shell.rs`, it rebases after G merges instead of running beside it.

**Gates for every leg:**
- `cargo test -p designos-app --lib` (narrow filter first).
- `cargo clippy -p designos-app -- -D warnings`, `cargo fmt --check`.
- `python3 scripts/zero_fake_ui_gate.py`.
- A 1440 desktop capture for any visible change (responsive: SKIPPED, owner default).
- UI legs load `es-designer`. Strings follow the PCP ux-writing rule: actions of 3 words or fewer, messages of about 8 words.

**Rollback:** each leg is one squash PR with no format or document change, so `git revert <sha>` is safe in any order. The exception is #869, which reads the #904 menu helpers: revert #869 first.

## Owner questions (7; AskUserQuestion, at most 4 per round; recommended option first)

Round 1: Q1, Q2, Q5, Q7 (they unblock the most legs). Round 2: Q3, Q4, Q6.

| # | Question | Options (recommended first) | Unblocks |
|---|---|---|---|
| Q1 | Where should the full shortcuts list live? | **(a) The current modal dialog, scrollable and grouped by menu.** Fixes the overlap, smallest change, and the dialog already exists (`home/dialog.rs:191`). (b) A Figma-style panel along the bottom, with category tabs, that stays open while you work (Figma help 360040328653). About twice the work. | #869 |
| Q2 | How should macOS show modifier keys? | **(a) Glyphs (⌘ ⇧ ⌥ ⌃) everywhere: dialog, menus, tooltips and the Cmd+K palette, through one formatter.** Matches macOS, Figma, the Zoom pill ("⌘+", `menus.rs:751-757`) and the palette ("⌥⌘D", `cmdk.rs:154`). Today every other menu shows "Ctrl+" on macOS, and the palette shows ⌘ even on Windows. (b) Words ("Cmd+Shift+S") everywhere. (c) Words in the dialog only, as the issue text says, with menus left unchanged. | #869 |
| Q3 | Should a key open the shortcuts list? | **(a) Yes: Ctrl+Shift+? (⌃⇧? on macOS), as in Figma.** (b) No: Help menu only, as today (`menus.rs:612` has no accelerator). | #869 |
| Q4 | Should the list include canvas-only keys (arrow nudge, Enter / Shift+Enter, Tab, Alt measure)? | **(a) Not in this slice: list only what menus and tools declare, so the list is generated and cannot drift.** Canvas keys come with #387. (b) Yes: add a hand-kept Canvas section, with a test that every listed key is bound. | #869 |
| Q5 | What do you see when a layer cannot be dropped where you are dragging it, for example a main into itself? | **(a) No drop line, and nothing happens on release, as today.** The issue asks only for this. (b) No drop line and a "not allowed" cursor. (c) (b) plus a toast on release that says why. | #969 |
| Q6 | What should VoiceOver say when the canvas has focus? | **(a) "Canvas".** Short, and the page name is already in the Pages list. (b) "Canvas, <page name>". | #1019 |
| Q7 | How should screen readers hear the Agents runtime pills and the Check designs Show/Hide button? | **(a) Pills as radio buttons ("Use Codex, radio button, selected"); the toggle named "Show ignored" / "Hide ignored".** A single choice reads as one, and a bare "Show" means nothing out of context. (b) Keep the pills as buttons with an on/off state ("toggle button, on") and keep the bare "Show"/"Hide". | #828 |

## Triage of all 54 open sub-issues

**Re-anchor method:**
- `git grep` at `origin/main@d22dad970`, plus a read of the cited lines.
- "Yes" means the code still shows the cause or the gap.
- "Partly" names what already landed.
- "UNKNOWN" means reading the code cannot decide it.

**Rank:** 1 = do first, by user impact against effort, across all 54.

**Size:** XS < 2 h, S 2–4 h, M 1 day, L several days.

**Groups:**
- **IN:** input and keyboard.
- **A11Y:** accessibility.
- **TXT:** text, fonts and text layout.
- **LAY:** auto-layout solver.
- **VIEW:** canvas rendering and view modes.
- **EXP:** export.
- **FILE:** files, tabs and app shell.
- **MEDIA:** video and colour picking.

| Rank | Issue | Title (short) | Group | Still real? (evidence at d22dad970) | Size |
|---|---|---|---|---|---|
| 1 | #951 | Two drags in one slow frame | IN | Yes. In `track_row_drag`, drag A's drop point is overwritten by B's release: `layers.rs:385-394` sets `frame.at` again while `drag` stays `Some` until the drop (`:764`). | XS |
| 2 | #935 | Escape in a field clears the selection | IN | Yes. The shell's Escape arm (`shell.rs:1590-1613`) runs before the panels draw, and egui has already dropped field focus. Only comments and dropdowns are exempt (`:1589`). | S |
| 3 | #904 | Board popovers, Zoom menu and layer picker pick on click | IN | Yes. `.clicked()` at `board/toolbar.rs:295,315`, `menus.rs:834` (`show_zoom_menu` `:767`) and `context.rs:272`. | S |
| 4 | #915 | Page drag may be lost | IN | Yes. `left.rs:315` uses `r.drag_started()`, and so do `home/main_view.rs:313` and `home/mod.rs:2406,2679`. Not reproduced yet. | S |
| 5 | #969 | Drop line on a row that refuses the drop | IN | Yes. The line draws for any `drop` (`layers.rs:742-758`); the refusal happens only inside `move_to_parent` (`ops.rs:2016-2021,2052-2061`). | S |
| 6 | #944 | Keyboard pick in toolbar menu; slide-to-pick in popover dropdowns | IN | Yes. Enter on the focused chevron reaches `cresp.clicked()` and toggles the menu shut (`toolbar.rs:276`). Dropdowns open on `r.clicked()` (`menu.rs:664`). | M |
| 7 | #828 | Expanded and selected states for screen readers | A11Y | Yes. The Check designs Show/Hide button sets no state (`check_designs.rs:493-505`). The runtime pills are plain `Role::Button` (`agent_a11y.rs:385-389`). | S |
| 8 | #1019 | macOS sees no focused element | A11Y | Yes. egui falls back to the root (`egui-0.35.0/src/context.rs:2604-2607`), and `accesskit_macos` drops `Role::Window` from focus (`node.rs:289`). | M |
| 9 | #869 | Shortcuts dialog incomplete, last row overlaps | IN | Yes. 13 hand-written rows (`dialog.rs:191-211`). 100 `leaf(` entries in `menus.rs`. Height has no cap (`dialog.rs:249-262`). | M |
| 10 | #1020 | Save as writes on the UI thread, panel opens in Documents | FILE | Yes (not changed since the issue; no commit cites it). Ranked after the slice: it is a save-path change under the session and data-loss invariants and needs its own Sonnet leg and review. | M |
| 11 | #1018 | Large files open on the UI thread (4 s) | FILE | Yes. `io.rs:83` parses inline, and `styles.rs:74` `style_entries_of` sorts per frame. Ranked here because it is L and touches the startup and session path (CLAUDE.md §4 invariant 1), so it is its own slice. | L |
| 12 | #940 | Fonts set by agents, undo, paste or styles are not fetched | TXT | Yes. Only some edit paths fill `wanted` (`fonts.rs:19`). There is no shared hook. | M |
| 13 | #980 | Redo after a font arrives brings back the stand-in box | TXT | Yes (no commit cites it). | S |
| 14 | #949 | Retry a failed face; round script-fallback weights | TXT | Yes. `scripts.rs:76-79` still inserts the unrounded weights. | S |
| 15 | #1404 | Root cause: hug and fill text keep a stale size (P1) | TXT/LAY | Partly. #1254 and #1332 merged; #1154, #1321, #1361 and #1362 are closed. **The task list in the issue is stale:** PRs #1367 and #1370 closed unmerged, and #1360 merged. Draft #1357 is still open. #1341 is open, and #1667 claims the mode-change re-measure. Not in S1: it waits for #1667 and for the umbrella spec `pm/specs/root-cause-umbrellas/spec.md`, which is outside this repo. | L |
| 16 | #962 | Wrapped text takes the height of its new width | LAY | Partly. #1360 merged ("re-measure wrapped Fill-width text height after a width change"). AC1 (0 layers moved on kit relayout) and AC2 (nested re-solve) are not verified. | M |
| 17 | #1004 | No visible way to rename a file | FILE | Yes. Rename is only `Intent::RenameFile` from the File panel header (`home/mod.rs:810`). `menus.rs` has no "Rename" leaf. Needs a UX decision. | S |
| 18 | #879 | Frame presets from the Frame tool dropdown | IN | Yes. Presets are only in the inspector (`right.rs:156`). | S |
| 19 | #870 | Reopen the tabs open at quit | FILE | Yes. No tab list is persisted (grep `open_tabs\|restore_tabs` finds only a test). | M |
| 20 | #1474 | Right-click menu parity for layers | IN | Yes (per issue, 2026-11-01). Context menu at `panels/context.rs`. | M |
| 21 | #895 | One writer for dialog-less saves | FILE | Partly. The Cmd+K label is already "Save .fig copy" (`cmdk.rs:261`). The two writers remain: `fig_export.rs:22` `export_new` and `io/fallback_save.rs:96` `write_new_in`. | S |
| 22 | #1582 | GUI `--select id:` differs from headless | FILE | Yes (2026-11-03, not investigated). | S |
| 23 | #1244 | 12-column grid shows as solid pink stripes | VIEW | **UNKNOWN.** The painter keeps alpha (`Color32::from_rgba_unmultiplied`, `grids.rs:494-497`) and honours the global toggle (`grids.rs:504`) and per-grid `visible` (`is_visible`, `grids.rs:591`). If the bug is real it sits upstream: the kiwi or REST colour import (`fig.rs:2235`, `rest_import.rs:357-359`) or stacked grids. Needs the owner's file or a red render test. | S? |
| 24 | #889 | UI polish from the 0.6.0 QA round (10 items) | VIEW/IN | Partly. F-34 copy and others not checked one by one. Split it before building. | M |
| 25 | #474 | Idle hot reload after undo; conflict policy | FILE | Yes (no commit cites it). Item 2 needs an owner policy. | M |
| 26 | #519 | Text layout drifts from Figma | TXT | Yes. Investigation bucket. The 10-28 comment shows the box geometry is exact and the shaping is +1–8 px. | L |
| 27 | #997 | `layout_node` and `relayout_subtree` disagree | LAY | Yes. Related to #1667, which changes what MCP edits re-solve. Re-measure after #1667 merges. | L |
| 28 | #417 | Rotated Hug children re-spaced after an MCP session | LAY | **Superseded-pending #1667.** The page-wide re-solve the issue blames is what #1667 removes. The rotated-bounds spacing is a separate solver gap. Verify after #1667 merges. | M |
| 29 | #1476 | Outline mode and pixel preview | VIEW | Yes. grep `outline_mode\|pixel_preview` finds nothing. Touches the GPU and CPU renderers (§4 invariant 3). | L |
| 30 | #1473 | Hyperlinks in text | TXT | Yes. The `hyperlink` field exists (`agent_busy_frames.rs:396`) with no editor UI. | M |
| 31 | #393 | Missing-fonts panel with replace | TXT | Yes. | M |
| 32 | #382 | REA study (accessible canvas, smart symbols, ExportSettings…) | A11Y | Yes. Unblocks #387, #396, #398, #400 and #402. | S |
| 33 | #387 | Full keyboard control of the canvas | A11Y | Partly. Enter / Shift+Enter navigation is done (`shell.rs:1615-`). There is no F6 region cycle (grep `Key::F6`: 0). The shortcut-reference criterion is #869. | M |
| 34 | #383 | Runtime palette with contrast mode | A11Y | Yes. `theme.rs` has 562 `pub const`. Needs a quiet merge window. | L |
| 35 | #384 | Enhanced-contrast mode | A11Y | Yes. Depends on #383. | M |
| 36 | #385 | Accessibility info on every widget | A11Y | Partly. The issue says AccessKit is called "zero times", which is stale: `widgets::labelled` (`widgets/mod.rs:208`) labels the Agents panel, rail and project tab. The properties panel is still unlabelled. | L |
| 37 | #386 | Canvas accessibility tree | A11Y | Yes. Builds on #1019. | L |
| 38 | #394 | Apple system fonts with optical sizes | TXT | Yes. SF Pro is still drawn with Inter (`fonts.rs:239`, `ensure.rs:104-107`). | M |
| 39 | #391 | Import and embed fonts (fsType) | TXT | Yes (grep `fsType`: 0). Depends on X3. | M |
| 40 | #392 | Team font folder | TXT | Yes. Depends on #391. | S |
| 41 | #395 | Every variable-font axis | TXT | Yes. The UI offers 4 axes (`typography.rs:1059-1080`). On the hot-file queue. | L |
| 42 | #397 | Text decoration style, thickness and colour | TXT | Yes (grep `decoration_style\|Wavy`: 0). On the hot-file queue. | M |
| 43 | #398 | Text parity audit | TXT | Yes. Depends on #382. | S |
| 44 | #396 | Smart symbols | TXT | Yes. Behaviour UNKNOWN until #382. | S |
| 45 | #1475 | Spell check in text editing | TXT | Yes (grep `spell`: comments only). OS integration. | L |
| 46 | #388 | Eyedropper anywhere on screen | MEDIA | Yes (grep `NSColorSampler`: 0). Depends on E1-7. | M |
| 47 | #400 | Export settings popover + .fig ExportSettings | EXP | Partly. `outline_text` exists with no UI (`svg.rs:30`). On the hot-file queue. | M |
| 48 | #401 | ICC profiles in PNG/JPG | EXP | Yes (grep `iccp`: 0). Depends on E1-7. | M |
| 49 | #402 | SVG simplify stroke, include id | EXP | Yes. Depends on #382 and #400. | M |
| 50 | #403 | Bulk export dialog | EXP | Yes. | M |
| 51 | #480 | Vector PDF follow-ups (P3) | EXP | Partly. The "raster images for now" line is now only in a historical CHANGELOG entry (`CHANGELOG.md:844`). The renderer follow-ups remain. | L |
| 52 | #389 | Video on the canvas (GPU texture slot) | MEDIA | Yes. GPU hot path (§4 invariant 4). | L |
| 53 | #390 | Stream large video (300 MB) | MEDIA | Yes. On the hot-file queue. | L |
| 54 | #404 | Tab muting, agent terminal window (P3) | FILE | Yes (grep `mute_tab`: 0). | M |

**Nothing is fully fixed.** No commit on main cites any of the 54 numbers. The only "obsolete" candidates are:
- #417, superseded-pending #1667;
- the stale task list inside #1404;
- the CHANGELOG sub-item of #480.

## Proposed later slices

1. **S2 Files that never freeze.** #1020, #1018, #870, #1004, #895, #474. Sonnet, because of the session and save invariants. Owner questions: the rename entry points (#1004) and the conflict policy (#474).
2. **S3 Fonts arrive on every path.** #940 (one shared request hook), #949, #980, then #393.
3. **S4 Text boxes that fit.** Starts after #1667 merges: #1404 (one shared re-measure step, invariant test), #962, #997, #417 (verify only), #519 (investigation).
4. **S5 Figma basics parity.** #1474, #879, #1473, #1476, #1582, #889 (split first), #1244 (after a repro file).
5. **S6 Accessible Studio (v0.39 pack).** #382, then #383 (quiet window), then #384, #385, #386 and #387. It builds on S1's #1019, #828 and #869.
6. **S7 Type system (v0.41).** #394, #395, #397, #391, #392, #396, #398, #1475. Hot-file queue positions 49 and 50.
7. **S8 Export settings (v0.42).** #400, #401, #402, #403, #480.
8. **S9 Media.** #388, #389, #390, #404.

## Assumptions (recorded, not asked)

| Assumption | Confidence | What would flip it |
|---|---|---|
| #915 is real (same egui rule as #855) | medium | The single-frame sim test over a page row passes on main. Then close #915 with that test as evidence. |
| The review's claim that right-panel dropdowns already slide-to-pick (#944) | low (UNVERIFIED) | The first test in the #944 spec shows the right panel is red too. Then the scope is "all dropdowns". |
| VoiceOver on macOS does not announce `expanded` (#828) | high: grep "expanded" finds no hit in `accesskit_macos-0.26.3/src` | A newer `accesskit_macos` in `Cargo.lock`. |
