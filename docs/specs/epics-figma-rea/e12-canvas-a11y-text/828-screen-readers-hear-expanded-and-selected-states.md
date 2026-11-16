# #828 Agents and Check designs expose expanded and selected states to screen readers

| Field | Value |
|---|---|
| Issue | #828 (epic #124; label accessibility; no milestone) |
| Slice | S1, wave 1, leg E |
| Size | 3 builder hours |
| Tier | Argon (AccessKit tree assertions in existing `Sim` harness) |
| Depends on | Owner answer **Q7** (spoken names and pill role). **Blocks** #1019 (shares `agent_a11y.rs` and `widgets/mod.rs`) |
| Touches | `crates/designos-app/src/widgets/mod.rs` (new helper next to `labelled` `:208`), `crates/designos-app/src/panels/check_designs.rs` (Show/Hide toggle `:493-505`), `crates/designos-app/src/agent_a11y.rs` (`label_agents_panel` `:346-389` only), tests in `crates/designos-app/src/shell/tests.rs` (AccessKit helpers `:1638-1680`) or a new `shell/tests/a11y_states.rs` |

**Code baseline:** `origin/main@d22dad970`. Re-anchor against HEAD first.

## User-visible outcome (screen-reader users)

- The Check designs "Show"/"Hide" ignored-findings button reports collapsed or expanded. So does the Agents panel's AI-chat toggle and terminal toggle.
- The runtime pills in the Agents panel (Claude Code, Codex, Antigravity, Grok) report which one is chosen.

## Current state (FACT)

1. `widgets::labelled(ui, id, role, name)` sets only role and label (`widgets/mod.rs:208-216`). It returns early when AccessKit is off (`agent_a11y::is_accesskit_active`, `:44`).
2. Check designs: the ignored-findings toggle is a `kit::button` labelled "Show"/"Hide" (`check_designs.rs:493-505`). It has no AccessKit state.
3. Agents panel (`agent_a11y.rs:363-389`):
   - `agents-header-ai` is `Role::Button` named "Show AI chat"/"Hide AI chat" (`:368`);
   - `agents-runtime-pty-toggle` is `Role::Button` "Show terminal" (`:389`);
   - the four runtime pills are `Role::Button` "Use Claude Code" … "Use Grok" (`:385-388`), with no selected state.

   The active runtime is `state.active_runtime` (`:358`).
4. Platform mapping (FACT, `accesskit_macos-0.26.3`):
   - `expanded` is not mapped on macOS (no "expanded" in `src/`).
   - `selected` is exposed only for item-like selectable roles (`node.rs:1031-1037`) and as the value of `Role::Tab` (`node.rs:326-330`).
   - `toggled` is exposed as the value of any node (`node.rs:324-325`).
   - `Role::RadioButton` maps to the AX radio button role (`node.rs:57`).
   - Windows UIA and Linux AT-SPI (through `accesskit_windows` / `accesskit_unix`) do map expanded. UNVERIFIED in this run; read their sources if a test needs it.
5. The tests already read the tree. `Sim.accesskit_update` (`shell/tests.rs:43`, `:153`) and `find_accesskit_node(update, role, name)` (`:1655`).

## Scope

1. Add `widgets::labelled_state(ui, id, role, name, state: A11yState)` with `A11yState { expanded: Option<bool>, selected: Option<bool>, toggled: Option<bool> }`. It is a no-op when AccessKit is off, like `labelled`. `labelled` calls it with the default state, so there is one code path.
2. Check designs toggle: set `expanded = show_ignored` on `check-designs-toggle-ignored`. Keep the visible "Show"/"Hide" label.
   - Q7 (a), the default: name it for screen readers as "Show ignored" / "Hide ignored". The current bare "Show" says nothing out of context, and this stays within the ux-writing rule of at most 3 words.
3. Agents panel:
   - `agents-header-ai` gets `expanded = state.chat_mode`.
   - `agents-runtime-pty-toggle` gets `expanded = false`: it is drawn only in chat mode, where the terminal is hidden.
4. Runtime pills, Q7 (a), the default: role `Role::RadioButton` inside a `Role::RadioGroup` region named "Agent runtime".
   - Set `selected` and `toggled` from `state.active_runtime == <pill runtime>`.
   - `selected` meets the issue text. `toggled` is what macOS speaks (FACT 4).
   - The names stay "Use Claude Code" and the rest.

## Acceptance criteria

- **AC-1** With AccessKit enabled and Check designs open with 1 or more ignored findings: the toggle node has `is_expanded() == Some(false)`. After a click it has `Some(true)`, and its name is "Hide ignored".
- **AC-2** Agents panel in chat mode: `agents-header-ai` has `expanded == Some(true)`. After toggling chat off it has `Some(false)`.
- **AC-3** Exactly one runtime pill node has `selected == Some(true)` and `toggled == Some(Toggled::True)`, and it is the active runtime. After choosing another runtime, the flags move to that pill.
- **AC-4** With AccessKit disabled, the helper does not touch the context. `labelled_state` returns before `accesskit_node_builder`, the same as `labelled`; the existing perf assumption at `widgets/mod.rs:205-207` holds. Assert with a unit test on the early return (no node in the update).
- **AC-5** The existing AccessKit tests stay green. For example the Agents panel tests at `shell/tests.rs:1664-1700` and `agent_simplified_panel.rs:835`. Update the expected roles of the pills there.

## Test steps

1. Add the tests next to the existing AccessKit tests (`shell/tests.rs:1638-1700`). Use `open_agents_panel_with_accesskit` and `find_accesskit_node`.
2. `cargo test -p designos-app --lib accesskit agents check_designs`
3. Manual VoiceOver check on macOS: Cmd+F5, then move to a pill. It should say "Use Codex, radio button, selected" or the equivalent. Record a screen capture for the PR, owner-manual if the builder cannot run VoiceOver.

## Edge cases

- No agent tab and no active runtime: the default runtime is shown as selected (the current visual).
- The panel is in terminal mode (`side_panel_pty_open`): pills are not drawn and no nodes are emitted (current rule, `:374-383`).

## Non-goals

- Labelling the other properties-panel widgets. That is #385.
- macOS `expanded` speech: AccessKit 0.26 does not map it. The flipping "Show"/"Hide" label already carries the state on macOS.

## Parity facts

- AccessKit node flags as above, with file:line in `~/.cargo/registry/src/*/accesskit_macos-0.26.3/src/node.rs`.

## Decisions

- **Pending Q7 (recommended default shown).**
  - Pills use radio semantics ("Use Codex, radio button, selected"): they are a single choice. Tab semantics would claim tab panels that do not exist.
  - The Check designs toggle's spoken name becomes "Show ignored" / "Hide ignored". The visible label is unchanged.
- Q7 (b) keeps `Role::Button` with `selected` and `toggled` set (macOS speaks "toggle button, on"), and keeps the bare "Show"/"Hide" name.

## Rules

- Open one PR that references #828. Use "Fixes #828" only when every acceptance criterion is met. Do not merge, tag or close.
- Re-anchor every cited line against HEAD before writing code, and report drift first.
- Work on a branch or worktree from `main`, never on `main`.
- Merge with squash only, after `gh pr update-branch <n> --rebase`.
- Blocking CI is `Test (macos-14)`, `Clippy` and `Rustfmt`. Ubuntu and Windows CI are advisory (CLAUDE.md, Git Workflow).
- Test first: each new test is red on the base commit, and that red output goes in the PR body.
- Visual evidence goes in the PR: an app capture, or a terminal render for test-only proof.
- Keep plan ids, issue numbers and finding codes out of code comments and test names. Use conventional commits with no AI trailers.
