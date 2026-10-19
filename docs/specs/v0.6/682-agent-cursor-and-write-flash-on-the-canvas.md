# #682 Show each agent on the canvas: a named cursor that moves to what it edits, and a flash on every write

| Field | Value |
|---|---|
| Issue | #682 (parent #585; owner ruling 2026-10-19: default cursor size Small, labelled with the agent name) |
| Size | L |
| Depends on | #592 (in-progress flag), #572 (agent identity per run); soft: #653 (session identity), #683 (tab status carries the tab id) |
| Unblocks | #695 (the chip takes the agent colour), #693 (names the agent) |
| Touches | new `crates/designos-app/src/agent_presence.rs` (state, follow and envelope math), new `crates/designos-app/src/canvas/agent_overlay.rs` (painting), `crates/designos-doc/src/live_patch/replay.rs` (report of touched ids), `crates/designos-app/src/live_patch_apply.rs` (`apply_live_patches` `:81-110`), `crates/designos-app/src/canvas/mod.rs` (one call after `draw::overlays`, `:387`), `crates/designos-app/src/comments/settings.rs` (one choice value), `crates/designos-app/src/theme.rs` (agent colours, if ratified) |

Code baseline: `origin/main@9f3f7a5`.

## 1. Summary and user story

As a designer, I want to see that an agent is working and what it just changed, without reading the terminal. Each agent shows as a small named cursor that travels to the layer it edits, and the layer's outline flashes in the agent's colour on every write.

## 2. Current state (FACT)

- The only signals are a status line "Applied N live MCP edit(s) from <path>" (`live_patch_apply.rs:60-62`), a toast such as "Applied agent edit: 2 new layers" (`:164-179`), and the selection moving to inserted roots (`:95-96`).
- The agent's writes arrive as `LivePatch { v, base, ops }` (`designos-doc/src/live_patch/mod.rs:27-35`). A patch carries no agent or run id. The ops are `Insert`, `Update`, `Reparent`, `Remove` and `Comments` (`mod.rs:134-172`). There is no `Replace` op: a replace arrives as `Update` or as `Remove` plus `Insert`.
- Replay reports only `inserted_roots`, `needs_layout`, `applied`, `duplicates`, `skipped` (`replay.rs:61-73`). Ids of updated, moved or removed nodes are not reported.
- One log `doc-patch.jsonl` belongs to one file's session folder (`terminal.rs:704-708`, `live_patch` at `:719`). Several agent tabs in one file share it, so today a write cannot be tied to a tab.
- The canvas draws editor overlays through `canvas::draw::overlays(app, ui, rect)` (`canvas/mod.rs:387`, `draw.rs:774`) with the egui painter. `View::to_screen` maps page to screen (`app.rs:29`).
- `theme.rs` has no agent colour and no `info` or `success` constant (only `DANGER`, `:60`). `design/design.tokens.json` has `info` `#0d99ff` (Selection Blue, canvas selection) and `success` `#14ae5c`. `AVATAR_COLORS` (`theme.rs:461`) holds six comment-avatar colours, one of them cyan.
- Design laws that bind the colours (`design/principles.md` P2): cyan `#06B6D4` is for components only; orange `#E36323` is the primary action; purple is banned.
- Idle invariant (CLAUDE.md section 4): viewport-only work must not call `app.changed()` or bump `TileView::rev`; the idle frame is 0.00% CPU.
- Settings store: `load_flag` and `save_flag` hold booleans (`comments/settings.rs:34-44`); the author name is a string (`:29`, `:46`).
- UNKNOWN: whether a node can be drawn at fractional opacity for a fade-in without recompiling the scene (GPU `SceneCompiler`, CPU tile path). Check: spike S1 in rule 7.

## 3. Behaviour rules

1. **Identity.** Each agent session has a colour index and a label. A session is one agent tab of the file (#653 and #572 give the id; before #572 lands, the file has one session). Colours are assigned in order of first write and kept for the tab's life. The label is the agent name (`Claude`, `Codex`, `Grok`, `Antigravity`), or agent and model when the model is known.
2. **Touched ids.** Replay returns `touched: Vec<(NodeId, TouchKind)>` with `TouchKind` = `Inserted | Updated | Moved | Removed`, in op order, in editor ids (after remap). `apply_live_patches` hands each committed patch's list to `agent_presence`. #677's ledger and #695 read the same list; whichever task lands first adds it and the others reuse it.
3. **Cursor.**
   - It appears when the session's first write lands. For each later write it targets the written node. It settles near the middle of the node's bounds, at the nearest point inside the canvas rect (inset `INSET_FRAME`).
   - Motion is an exponential follow: each frame `pos += (target - pos) * (1 - exp(-2.5 * dt))`. Position is kept in page space, so panning moves the cursor with the layer.
   - Fade-in 200 ms with ease-out-cubic. After 5 s with no write it fades out over 1.5 s and the session's cursor is gone.
   - Size setting: Hidden, Small 24 px, Medium 40 px, Large 56 px. Default Small, labelled with the agent name. Hidden draws nothing and requests no frame.
4. **Write flash.** Every Insert, Update, Move or replace flashes the node's outline in the session colour. Envelope: attack 200 ms, hold 400 ms, release 300 ms. Stroke 2 px, 1 px for inserts. Top-level frames are not flashed. If the cursor had to travel, the flash starts 300 ms later. A removed node cannot flash; its parent flashes instead.
5. **Several agents.** Each session has its own colour and cursor. Cursors never share a colour while fewer than four sessions are active.
6. **Idle.** A frame is requested only while a cursor or flash is moving (`request_repaint()`), plus one one-shot `request_repaint_after` for the start of the fade-out at last write plus 5 s. There is no periodic timer. Nothing calls `app.changed()` or bumps `TileView::rev`.
7. **Fade-in of new top-level nodes** from opacity 0 with decay 5/s. Spike S1 first: if a runtime-only opacity multiplier cannot be animated without a recompile per frame or a `rev` bump, stop, report the measurement, and ask the owner (open question 3). Do not fake the fade with a cover shape.
8. **Headless.** Renders and exports (CPU, GPU, `--screenshot`, MCP export) never draw cursors or flashes: they live only in `canvas/agent_overlay.rs`.
9. A flash list is capped at 64 entries; the oldest drops first. A write to a node that no longer exists draws nothing.

## 4. Data model / schema changes

- `AgentPresence { sessions: Vec<Session> }` in memory. `Session { id, colour_index, label, cursor_page: Vec2, target_page: Vec2, last_write_s, fade, flashes: Vec<Flash> }`. Nothing is saved in the file.
- `LivePatchReport.touched` (rule 2) in `designos-doc`.
- One persisted choice, `agent_cursor_size` (`hidden|small|medium|large`), through a new `load_choice` / `save_choice` pair beside `load_flag` in `comments/settings.rs`.

## 5. Contracts

- No log, MCP or file format change. The patch line format is unchanged (`v: 1`).
- A session id on the patch comes from #572's `run_id` when it lands; until then `agent_presence` attributes every patch to the file's single session.

## 6. File-format mapping

None.

## 7. UI

Described in words; checked against `design/principles.md` and `design/soul.md`.

- The cursor is an arrow glyph in the session colour with a label pill under-right of it. The pill is `CONTROL_DENSE` (24 px) high, `radius_full`, caption type, a dark label on the colour fill (soul: dark label on an accent fill). The label elides with `widgets::elide`.
- The flash is a 2 px (1 px for inserts) outline of the node's world bounds, drawn above the canvas, below the cursor. It is not clickable and takes no input.
- Colours. The agent colours must not be cyan (components), purple (banned), orange (primary action), `info` blue (selection) or the guide red (`GUIDE_COLOR`, `theme.rs:99`). Recommended set of four: amber `#FFA629`, emerald `#14AE5C`, pink `#EE46BC` (all already in `AVATAR_COLORS`) and a fourth chosen by the owner. They become tokens in `theme.rs` and `design/design.tokens.json` (P1). The panel's "running" dot (#683) uses the `info` colour, which is a different job.
- The size setting lives with the Agents settings of #656 (open question 2).
- Desktop capture at 1440: one cursor mid-travel with its flash, two agents in two colours, Hidden. Plus a short screen recording. Responsive: SKIPPED (owner default). Load es-designer before UI code; update the Figma frames from the captures first.

## 8. Edge cases

1. The target node is off screen: the cursor clamps to the nearest canvas edge; #695's chip is the way to see it.
2. The user pans or zooms while the cursor moves: the cursor follows the layer (page space). The overlay never moves the view.
3. A write that changes nothing visible (a duplicate op): `duplicates` count, no flash.
4. A burst of 300 patches in one poll: one flash per touched node, capped at 64; the cursor goes to the last node.
5. The agent switches document (`DocEpoch`): sessions reset; no cursor on the new document until its first write.
6. A comment-only patch: no cursor movement.
7. Two agents write the same node in one frame: both flashes draw, in two colours.
8. Window unfocused: animations still end; no extra wake-ups.

## 9. Non-goals

- Cursors for the user's own edits or for other people.
- Moving the view (#695 owns the chip) and the busy-frame banner (#693).
- Agent attribution in the file (#572).

## 10. Test plan (red first)

Seams: pure functions in `agent_presence.rs` for follow, fade and envelope; the `Sim` harness with a stub agent that appends three patches to `doc-patch.jsonl`.

| Case | Expected | Fails before |
|---|---|---|
| `follow_reaches_target_with_rate_2_5` | after 1 s the gap is `exp(-2.5)` of the start | yes (no module) |
| `flash_envelope_is_200_400_300` | alpha at 100, 400, 700, 950 ms matches the envelope | yes |
| `three_writes_show_one_cursor_and_three_flashes` | recorded rects: one cursor, three flash outlines in the session colour | yes |
| `flash_waits_300_ms_when_the_cursor_travels` | start time of the flash | yes |
| `top_level_frames_do_not_flash` | no outline for a frame insert | yes |
| `two_agents_two_colours` | two session colours differ | yes |
| `cursor_is_gone_five_seconds_after_the_last_write` | no cursor at t = 5 s + 1.5 s | yes |
| `hidden_draws_nothing_and_asks_for_no_frame` | no rects; no repaint request | yes |
| `overlay_never_bumps_the_tile_revision` | `canvas.tiles.rev()` equal before and after the animation | yes |
| `cpu_export_is_pixel_identical_with_and_without_an_active_agent` | equal RGBA | guard |
| idle measurement | 20 s window after the cursor is gone: `ps -p <pid> -o time=` delta 0.00 s | yes (method of CLAUDE.md section 4) |

Run: `cargo test -p designos-app agent_presence`, `cargo test -p designos-doc live_patch`. Evidence: red/green capture, the 1440 captures, the recording, the idle measurement output. Owner-manual: a run with a real agent tab.

## 11. Performance budgets

- Per frame while animating: O(sessions + flashes) painter calls, under 0.2 ms with 64 flashes.
- No document walk: bounds come from `fast_world_aabb` for the touched ids only.
- Idle: 0.00% CPU. Settled state requests no frame.

## 12. Dependencies and build order

After #592 and #572 (part 1, E15-7 and RD-3). The `touched` field lands with whichever of #677, #682 or #695 is first. New files hold all logic; `canvas/mod.rs`, `live_patch_apply.rs` and `replay.rs` get call sites only. Serialize `replay.rs` with part 3's #677 and part 1's RD-3.

## 13. Owner rulings

- Owner ruling 2026-10-19: four agent colours, added as tokens `agent-1`…`agent-4`: amber, emerald, pink, sky. They avoid purple (Purple Ban), cyan (components) and orange (primary actions). A fifth concurrent agent reuses `agent-1` and is told apart by its label.

- Default cursor size Small, labelled with the agent name (2026-10-19). The behaviour numbers in the issue are the owner's and are used verbatim.
- Headless renders and exports never contain cursors or flashes; idle CPU returns to 0%.

## Open questions for the owner (recommended first)

(Answered by the owner 2026-10-19, see §13.) 1. The four agent colours. Recommended: amber, emerald, pink and one more warm neutral, as tokens, so no agent looks like a component, a selection or a guide.
2. Where the size setting lives. Recommended: the Agents settings of #656, with the value stored beside the other panel settings.
3. If the fade-in cannot be done without a per-frame recompile. Recommended: ship cursor and flash, replace the fade by the 1 px insert flash, and file the fade as a render task.
