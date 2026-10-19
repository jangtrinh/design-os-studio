# #687 Agents panel accessibility: announce finished, needs-input and error states, and label the panel regions

| Field | Value |
|---|---|
| Issue | #687 (parent #585) |
| Size | M |
| Depends on | #683 (status changes), #686 (failure classes); soft: #689 (composer), #684 (Stop button), #654 (selection chip) |
| Unblocks | none |
| Touches | new `crates/designos-app/src/agent_a11y.rs` (announcement node, region and name helpers), `crates/designos-app/src/terminal.rs` (hot spot: call sites at the header, tab strip, viewport and composer), `crates/designos-app/src/widgets/mod.rs` (a `labelled` helper beside `record`, `:222`), `crates/designos-app/src/shell.rs` (focus shortcut), `crates/designos-app/src/shell/tests.rs` (capture of the AccessKit update), shortcuts list |

Code baseline: `origin/main@9f3f7a5`.

## 1. Summary and user story

As a screen-reader user, I want to hear when an agent finishes, needs my input or fails, and I want the panel's parts to have names, so that I can work with an agent without seeing its terminal.

## 2. Current state (FACT)

- Studio uses egui and eframe 0.35 (`crates/designos-app/Cargo.toml:38`, `:49`). `accesskit` 0.24.1 is in `Cargo.lock` (`:20-22`) through eframe's default `accesskit` feature (UNKNOWN until `cargo tree -p designos-app -e features | grep accesskit` confirms the feature is on).
- egui 0.35 offers `Context::enable_accesskit` (`context.rs:3599`), `Context::accesskit_node_builder(id, |node| ...)` (`:3582`) and returns the tree in `PlatformOutput.accesskit_update` (`data/output.rs:151`). `accesskit::Live` has a `Polite` value (`accesskit-0.24.1/src/lib.rs:584-586`).
- `designos-app/src` contains no AccessKit call and no `widget_info`. The panel draws with the painter and creates controls with `ui.interact(rect, Id, Sense)` (for example the tab close, `+` and send buttons, `terminal.rs:2543-2595`, `:3040-3085`), so they have no accessible name. `widgets::record` is a test-only hook (`widgets/mod.rs:221-227`).
- The `Sim` harness keeps the frame's cursor icon and shapes but not the AccessKit update (`shell/tests.rs:61-66`).
- The panel's regions today: header strip (`terminal.rs:2494-2515` in terminal view), tab strip, terminal viewport (`render_vt100_viewport` `:3614`), and, in chat mode only, the composer (`:2949-3100`).
- The state changes to announce come from #683 (`StatusChange` list) and #686 (failure class). Neither is on `main`.
- UNKNOWN: the AccessKit `Role` that suits a status region (`Status` or `Log`) and `Node::set_live`, and whether VoiceOver, NVDA and Orca speak a label change of a live node from egui's adapter. Check: a compile check of the calls, then an owner-manual run with each reader.
- UNKNOWN: which key chords are free. `shell.rs` already binds many `Cmd` chords (`:608-700`).

## 3. Behaviour rules

1. **Announcements.** For each status change of an agent tab, exactly one polite announcement:
   - turn ended: "<agent> finished";
   - needs input: "<agent> needs your input";
   - error: "<agent> failed: <class>" (the class in words: "rate limited", "login expired", "billing or quota", "provider server error", "context full", "no network", or "unknown error").
   `<agent>` is the same name the tab shows. Changes to running, stopped or idle are not announced.
2. **Mechanism.** One invisible node per Agents panel with `Live::Polite`. Its label is set to the announcement text in the frame the change is drained, and cleared two frames later. It has no visual and no pointer target. Announcements are queued, one per frame, so two changes in a frame are both spoken.
3. **Regions.** The panel, the tab strip, the terminal and the composer each carry an accessible role and name: "Agents panel", "Agent tabs", "Agent terminal", "Message composer". The terminal region's description names the active agent and its status.
4. **Icon buttons** get names: Send, Stop, New session (the `+`), Close tab, Selection chip (when #654 exists), plus the others in the panel (attach, runtime pills). The name states the action, never the glyph.
5. **Keyboard.** One shortcut moves focus to the composer, and the same shortcut moves it back to the canvas. In terminal view, where there is no composer today, it focuses the terminal and back. The chord is added to the in-app shortcuts list and to the docs. Recommended chord: `Cmd+Alt+J` (`Ctrl+Alt+J` elsewhere), after the check that it is free.
6. Names and announcements are text from real state only; nothing is announced that did not happen.
7. Tab order inside the panel follows the visual order: header controls, tab strip, terminal, composer.

## 4. Data model / schema changes

None. A small `Announcer { queue: VecDeque<String>, shown: Option<(String, frame)> }` in memory.

## 5. Contracts

- `agent_a11y::announce(&mut self, change: &StatusChange)` called with the drained list of #683. A `widgets::labelled(ui, id, role, name)` helper wraps `ui.interact` plus `ctx.accesskit_node_builder`. Without `accesskit` enabled these are no-ops (egui returns `None`).

## 6. File-format mapping

None.

## 7. UI

No visual change. Described in words: the shortcut appears as one more row in the shortcuts list with the same row height (`CONTROL_DENSE`) and mono key caps. Focus rings on the composer and terminal already follow the orange focus ring rule (P2); this task does not restyle them. Evidence: the PR carries a capture of the AccessKit tree dump from the shell test (terminal capture) and the shortcuts list at 1440. Responsive: SKIPPED (owner default). Load es-designer before touching any UI code.

## 8. Edge cases

1. Ten agents finish in the same frame: ten announcements, one per frame, in tab order.
2. The panel is closed when a turn ends: the announcement is still made; the label lives on the shell-level node, not the panel's.
3. A tab renamed: the next announcement uses the new name.
4. AccessKit is off (no assistive tool): all helpers do nothing and cost nothing.
5. The same state repeats (two `Stop` events): #683 drains one change; one announcement.
6. Window unfocused: announced anyway; the OS decides whether to speak it.
7. A very long failure class text: classes are a fixed short list.

## 9. Non-goals

- Making the terminal's text readable by a screen reader (the VT100 grid). Canvas accessibility. Localised strings. Sounds.

## 10. Test plan (red first)

Seams: extend `Sim` to enable AccessKit and keep `platform_output.accesskit_update`; pure tests for the text builder.

| Case | Expected | Fails before |
|---|---|---|
| `announcement_node_appears_when_a_stub_turn_ends` | the update holds one node with `Live::Polite` and the text "Claude finished" | yes |
| `needs_input_and_error_texts` | "Claude needs your input"; "Claude failed: rate limited" | yes |
| `running_and_stopped_are_not_announced` | no node text | yes |
| `two_changes_in_one_frame_are_both_spoken` | two texts in two frames | yes |
| `regions_have_roles_and_names` | four named regions in the tree | yes |
| `icon_buttons_have_names` | Send, Stop, New session, Close tab present | yes |
| `shortcut_moves_focus_to_the_composer_and_back` | focus id | yes |
| `no_accesskit_means_no_cost` | with AccessKit off the helpers return without locking the context | guard |

Run: `cargo test -p designos-app agent_a11y shell`. Evidence: red/green capture. Owner-manual: VoiceOver on macOS (the release platform) hears each of the three announcements; NVDA and Orca are flagged not run.

## 11. Performance budgets

- Per frame: one queue check; the node is created only while an announcement is live. Under 0.02 ms.
- No timer, no repaint request beyond the two frames that clear the label.

## 12. Dependencies and build order

Last among the panel UI tasks of part 4, because it labels what the others add (README-part4 merge order). `terminal.rs` gets call sites only.

## 13. Owner rulings

- Through egui's AccessKit integration, a turn end, a needs-input state and an error each produce one polite announcement with the given texts; the panel, tab strip, terminal and composer are labelled regions; icon buttons have accessible names; a shortcut moves focus to the composer and back and is documented; a shell test checks the AccessKit tree. (Issue text.)

## Open questions for the owner (recommended first)

1. The focus shortcut. Recommended: `Cmd+Alt+J` on macOS and `Ctrl+Alt+J` elsewhere, if free.
2. Terminal view has no composer. Recommended: the shortcut then focuses the terminal, and #689 decides whether the composer joins the terminal view.
