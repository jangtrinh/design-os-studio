# #654 Live selection chip in the Agents panel; click to send

| Field | Value |
|---|---|
| Issue | #654 (owner ruling 2026-10-19; audit experience 4) |
| Size / priority | M / P1 |
| Depends on | none (independent of the session work) |
| Unblocks | #655 (shares the reference format for layers) |
| Touches | new `crates/designos-app/src/agent_selection_chip.rs` (summary and chip painting), `crates/designos-app/src/terminal.rs` (hot spot: the context strip in `show_left` `:2602-2610`, the dock bar in `render_terminal_surface` `:3467-3481`, `export_selection_and_comments_to_pty` `:2413-2465`, `TerminalAction`), `crates/designos-app/src/panels/left.rs` (`:17-21`), `crates/designos-app/src/shell.rs` (`:1190`, `:1331-1337`), tests |

Code baseline: `origin/main@01d48f2` (0.5.0). The issue says "in the terminal header"; the panel's header strip is the
20 px "doc · page" strip, and the dock's header is its tab bar. Both are covered here.

## 1. Summary and user story

As a designer, I always see what is selected, in the agent panel header, as `page · N selected · first names`.
It follows my selection live. When I want the agent to look at it, I click the chip and the selection goes to the
agent. Nothing is sent when I merely select things, so it costs no tokens until I click.

## 2. Current state (FACT)

- The panel header strip shows `{doc_name} · {current_page_name}` as a 20 px strip under the tab bar
  (`terminal.rs:2602-2610`); it has no selection and no click.
- The selection exists in the app as `app.selection: Vec<NodeId>` with `app.page`; node names via
  `doc.get(id).name`. The panel painters receive only `TerminalState`
  (`show_left(state, ui, rect, file_path)`, `:2471`); the caller `panels/left.rs:17-21` has `app`; the dock caller is
  `shell.rs:1190`.
- The terminal mirrors the selection into the session folder for the agent: `sync_live_file_context` writes
  `context.json` with `selected_node_ids` and a scoped `canvas.designos` whenever the document epoch, page or
  selection changes (`terminal.rs:800-855`). That is a file the agent may read, not text pushed into the prompt.
- The prompt hook reply carries the page only: the test at `terminal.rs:5607` asserts that the context reply
  never contains `selected_node_ids` ("owner ruling: the selection is never auto-injected").
- Explicit sending exists: `TerminalAction::ExportSelectionToTerminal` (`terminal.rs:338-353`), raised by the dock
  bar's "Send Selection & Comments" pill (`:3467-3481`), the composer `+` (`:2979-2996`) and the "Send Selection" prompt
  card (`:2904-2922`). It calls `export_selection_and_comments_to_pty` (`:2413-2465`), which:
  - exports only the **first** selected node (`selection.first()`, else the first top-level frame of the page when nothing
    is selected) with `doc.export_spec`, plus every unresolved comment of the document, into `selection.spec.json`;
  - pastes one fixed sentence ("Read the design:os selection at <path> ...") into the active agent tab with a bracketed
    paste, without Enter (`paste_into_agent_tab`, `:1444-1452`; a shell tab or a dead agent receives nothing and the
    status line says so, `:2458-2463`).
- `designos_get_node` and `designos_export_spec` exist as MCP tools (`designos-mcp/src/engine.rs:80`, `:271`).
- Repaint cadence of the panel: 50 ms when the terminal is focused, 500 ms otherwise (`terminal.rs:2625-2626`); a
  selection change comes from input or from a live patch in the same update (`live_patch_apply.rs:94-99`).

## 3. Behaviour rules

1. **The chip** shows `page · N selected · first names`, where `page` is the current page name, `N` is
   `app.selection.len()`, and `first names` is the names of the first K selected layers in selection order. It is
   computed from `app` at paint time and passed to the painters (a `SelectionSummary { page, count, names }` argument), so
   it changes in the same frame as the selection and involves no file round trip. A change of selection, page or layer
   name is visible on the next painted frame.
2. **No tokens until clicked.** Computing or painting the chip writes nothing to the PTY. The existing file mirroring
   (`context.json`) is unchanged and is not a prompt.
3. **Click sends.** Clicking the chip raises a new action `TerminalAction::SendSelectionToAgent`. It writes
   `selection.spec.json` and pastes one fixed sentence into the active **agent** tab. This text assumes the recommended answer to open question 5:
   no Enter, like today's export (`paste_into_agent_tab`); the user can add a request and press Enter.
4. **What the file contains.** For the whole selection, not only the first layer:
   `{ "page": {id, name}, "count": N, "nodes": [ {node_id, name, kind, spec?} ] }`, one entry per selected layer in selection
   order. `spec` (the `doc.export_spec` NodeSpec) is included for the first 10 layers; for the rest only
   `node_id`, `name`, `kind`, and the sentence tells the agent to read the others with `designos_export_spec`. A file
   larger than 256 KB drops `spec` from the remaining layers the same way. (Both numbers are initial values; the worker
   confirms them on the synthetic large fixture and records the sizes.)
5. **Where the tab is missing.** If there is no live agent tab, nothing is written to a PTY; the status line says
   "No agent tab open - nothing sent" (the existing wording style, `:2462`).
6. **Empty selection.** The chip is shown with `none selected` and is inert (no click action). Today's fallback of
   exporting the first top-level frame is not used by the chip. (Presentation: open question 1.)
7. **Shell tabs and dead agents** receive nothing (existing `accepts_agent_input`).
8. The existing "Send Selection & Comments" pill and the composer `+` keep their behaviour; whether the pill is removed
   or kept beside the chip is open question 3.
9. Applies to every CLI tab (Claude, Codex, Grok, Antigravity): the paste is the same text for all.

## 4. Data model / schema changes

- `SelectionSummary` (in memory).
- `selection.spec.json` gains the `nodes` array shape of rule 4; the old single-spec shape produced by
  `export_selection_and_comments_to_pty` stays for the existing pill until open question 3 is answered.

## 5. Contracts

- Agent-facing: a one-sentence paste plus `selection.spec.json`. The `designos-studio` skill text
  (`provision_session_agent_files`, `terminal.rs:917-945`) gets one line describing the new file shape and
  naming `designos_export_spec` for layers without an inline spec.
- No MCP tool changes.

## 6. File-format mapping

None.

## 7. UI

Described in words. Checked against `design/principles.md` (P1 to P10) and `design/soul.md`; the rules used, with the `theme.rs` constants, are recapped in README-part3 under "UI rules checked for part 3". In short: every colour, gap, inset, control height and radius below is a `crates/designos-app/src/theme.rs` constant (the panel's own `TERM_*` / `STATUS_*` constants at `terminal.rs:29-55` stay for the terminal viewport and the existing tab status dots and are not extended); selected and active rows use the neutral fill `BG_SELECTED`; the brand orange is the view's one primary action; counts sit in their own pill, not in a label.

The issue fixes the chip text and the click behaviour; the rest is a proposal for the owner's confirmation:

- A pill `CONTROL_DENSE` (24) high with `radius_full(24)`, in the panel's header strip (the strip's height changes from today's 20 px literal at `terminal.rs:2603` to `CONTROL_DENSE`, a move onto the control family) and, in the dock, at the left of the bar's right-hand button group, `SPACE_INLINE` from its neighbours. Caption type (`font_caption`), `TEXT` on a neutral `BG_SELECTED` fill with no outline when there is a selection (selection is a neutral fill, P2; the pill is not blue); `TEXT_SECONDARY` on `BG_SECONDARY` with a `BORDER` outline when nothing is selected. Hover steps the fill to `BG_SELECTED_HOVER` or `BG_HOVER`. The 24 px pill is its own pointer target.
- Long text is elided with `widgets::elide` to the strip width; the names part shrinks first, then the page name.
- A tooltip: "Send the selection to the agent" (selection) or "Select layers to send them" (none).
- No animation, no timers (idle CPU invariant). Zero fake UI (P6): the chip renders only from the real selection.
- Desktop captures at 1440 for: one layer, three layers, many layers, none selected, elided; responsive SKIPPED (owner default). Load es-designer before UI code; update the Figma frames from the captures first (owner rule).

## 8. Edge cases

1. 10,000 layers selected (select all): `count` is `len()`, only K names are read; the click writes at most 10 specs.
2. A selected layer is deleted by the agent between paint and click: the file lists only layers that still exist at click time.
3. Selection includes a page node or a hidden layer: listed like any layer.
4. A layer name with newlines or very long text: the chip shows its first line, elided; the file keeps the full name.
5. RTL or non-Latin names: painted by egui's text layout; elision works on characters, not bytes.
6. Two quick clicks: two pastes (the user's action); each rewrites the file atomically (`atomic_write_session_file`).
7. Selection changes while the agent is mid-turn: the chip follows live; the file is written at click time, never earlier.
8. Multi-window: each document tab has its own `TerminalState` and chip.

## 9. Non-goals

- Auto-injecting the selection into prompts (ruled out by the owner).
- Sending a screenshot of the selection (belongs to E15-16 / T7.1).
- @-mentions (#655).

## 10. Test plan (red first)

Seams: pure-function tests for the chip text; `Sim` harness tests for the live update and the click, with a stub agent
(`sh -c 'cat > $out'`) recording what reaches the PTY.

| Case | Expected (fails before) |
|---|---|
| `chip_text_formats` | `selection_chip_text("Page 1", 3, ["A","B","C"])` equals the ruled shape `Page 1 · 3 selected · A, B, C` once the owner fixes K and the overflow style; `0` gives the none-selected form |
| `chip_updates_in_the_same_frame` | change `app.selection`, run one frame: the chip's recorded rect/text reflects it (no second frame) |
| `selection_change_writes_nothing_to_the_pty` | select, change, clear for N frames with a stub agent: zero bytes received (the token-cost guard) |
| `click_pastes_one_sentence_without_enter` | click: the stub receives one bracketed paste, no trailing `\r` |
| `file_covers_the_whole_selection` | select 3 layers: `selection.spec.json.nodes.len() == 3`, ids and names match |
| `large_selection_is_capped` | 200 layers: specs for the first 10, ids for all, file under 256 KB |
| `empty_selection_chip_is_inert` | no action raised, nothing written |
| `no_agent_tab_sends_nothing` | status line text, no PTY write |
| `context_reply_still_excludes_the_selection` | the existing assertion (`terminal.rs:5607`) stays green |
| `chip_elides_to_width` | narrow strip: text ends with an ellipsis, names shrink first |

Run: `cargo test -p designos-app terminal` and the `Sim` suites. Evidence: red/green capture plus the five 1440 captures; the
token-cost guard test output pasted in the PR.

## 11. Performance budgets

- Per frame: one pass over at most K selected ids (K small) and one string format; under 0.05 ms; nothing allocated when the
  selection, page and names are unchanged (cache the last summary by `(selection hash, page, doc mutation epoch)`).
- Click: writing the file for 10 specs is O(10 layers) `export_spec`; budget under 20 ms on the large synthetic fixture,
  measured and recorded. No UI-thread `to_json` of the whole document (CLAUDE.md invariant 1): only the selected layers are
  serialized.
- Idle: no timer and no repaint request added.

## 12. Dependencies and build order

Independent of #651-#653. `terminal.rs` is the hot spot: the chip and the action live in the new file; `terminal.rs` gets the
call sites. Serialize with the other agent-panel PRs (README-part3).

## 13. Owner rulings

- Show a live chip, `page · N selected · first names`, in the terminal header; it updates within one frame of a selection
  change; clicking it sends the selection; the selection is not auto-injected into every prompt. (Issue text.)
- No token cost unless the chip is clicked.
- Related decided ruling, not changed here: MCP tools default to the editor selection when attached, configurable per user and session as selection / require `node_id` / page
  (owner decision 4 in `plans/261017-epics-full-scope/owner-decisions-and-cross-check.md`). The chip controls what is pasted into the prompt; it does not replace the tool default.
- Design standard rulings of 2026-10-18 (`plans/reports/design-std-580-evidence/owner-rulings.md`): R3 chips are pills, R4 selection is a neutral fill, R10 control heights 24/28/32/40; the chip follows them.
- Skip-permissions stays the default; Codex tabs keep the heuristic status.

## Open questions for the owner

1. **Chip with nothing selected.**
   - A (recommended): show `page · none selected`, greyed and not clickable, so the chip's place never jumps.
   - B: show only the page name, no selection part, not clickable.
   - C: hide the chip.
2. **The existing `doc · page` strip.**
   - A (recommended): the chip replaces it; the document name stays visible in the tab and the file header.
   - B: keep the strip and put the chip beside it, which uses more width in a narrow panel.
3. **The dock's "Send Selection & Comments" pill.**
   - A (recommended): keep it as is; it also sends open comments, which the chip does not.
   - B: remove it; the chip is the one way to send the selection.
   - C: fold the comments into the chip's click as well (selection plus comments pinned to the selected layers).
4. **How many names, and how an overflow reads.**
   - A (recommended): the first 2 names, joined with commas, then `+N`.
   - B: the first 3 names, then `+N`.
   - C: only the first name.
5. **Does the click press Enter?**
   - A (recommended): no; the sentence is pasted and the user adds a request and presses Enter, as the existing export does.
   - B: yes; the agent starts reading at once.
