# #133 Bind variables to min/max size, font family/style and paragraph spacing/indent (E1-5b)

| Field | Value |
|---|---|
| Issue | #133 (E1-5b, P1). Milestone on GitHub: Backlog; body says v0.8 |
| Still real? | Partly. Font family/style and paragraph spacing/indent binds shipped in #1452 (`e4efa94e2`). Open: min/max binding, the font request on a mode switch, and node-level `.fig` round-trip tests |
| Size | Leg A (doc + .fig tests): S, about 4 h. Leg B (font requests on a mode switch): S, about 3 h. Leg C (min/max inspector fields + bind icon): M, about 6 h, only if the owner says yes (Q1) |
| Tier | Leg A: Argon. Leg B: Claude Sonnet (UI-thread performance path). Leg C: Claude Sonnet with `es-designer` |
| Depends on | Leg A: #1664 merged (hot file, test-only touch in `designos-fig/tests`; the code touch is doc-only). Legs B and C: none |
| Ready after owner answers? | Legs A and B are ready now. Leg C waits on Q1 |

Code baseline: `origin/main@d22dad970`.

## 1. Outcome

- A frame's min or max width or height can be bound to a number variable. Switching the mode re-solves auto layout with the bound limit.
- Switching a mode that changes a bound font family to a face not yet loaded fetches that face, so the text renders in it instead of the fallback.
- These bindings survive `.fig` save and reopen. Tests prove it.

## 2. Re-anchor (origin/main)

- FACT: `property_kind` has no `minWidth|maxWidth|minHeight|maxHeight` (`crates/designos-doc/src/variables_apply.rs:21-44`). Nor do `scope_allows` (`:48-86`) and `set_bound_property` (`:282-475`). Bound min/max keys therefore resolve to nothing.
- FACT: `.fig` import maps `MIN_WIDTH`/`MAX_WIDTH`/`MIN_HEIGHT`/`MAX_HEIGHT` to the keys `minWidth`/... (`crates/designos-fig/src/fig.rs:1971-1974`), and export maps them back (`crates/designos-fig/src/fig_export.rs:1873-1876`). So bindings are carried as data but never applied.
- FACT: limits live on `LayoutChild.min_w/max_w/min_h/max_h` (`crates/designos-doc/src/node.rs:1110-1113`). The solver reads them through `SizeLimits::from(&LayoutChild)` (`layout_solver_types.rs:33-37`) on a resolved node: `crate::resolve_node` at `crates/designos-doc/src/layout_apply.rs:88` and `:159`. Once `set_bound_property` writes the limits, the solver sees them.
- FACT: the inspector has no min/max fields. The only app writer is the Truncate path that clears `max_h` (`crates/designos-app/src/panels/typography.rs:670-679`). The canvas only draws a "Max H" label (`canvas/draw.rs:1136-1144`). A "bind entry point" therefore means new UI (Q1).
- FACT: font requests read stored values only. `requests_for` walks `TextStyle`s and node text styles, with a cap of 8,192 nodes when the document has more than 25,000 (`crates/designos-fonts/src/request.rs:41-58`). A family bound by variable is not in the stored style, so a mode switch never requests it.
- FACT: the editor's mode-switch hook already computes changed variables (`changed_variables`, `crates/designos-app/src/variables/mod.rs:559-575`).
- FACT: Figma's scope for min/max is `WIDTH_HEIGHT`. This is INFERENCE from the existing `"width" | "height" => has(&["WIDTH_HEIGHT"])` (`variables_apply.rs:69`) and Figma's public scope list, which has no separate MIN/MAX scope. The schema check is the `VariableScope` enum in `crates/designos-fig/src/figma_schema.txt` (from line 2759).

## 3. Scope and rules

**Leg A (doc + tests)**
1. Add `minWidth`, `maxWidth`, `minHeight` and `maxHeight` to `property_kind` as `Number`, and to `scope_allows` under `WIDTH_HEIGHT`. Add `set_bound_property` arms that write `layout_child.min_w`/`max_w`/`min_h`/`max_h = Some(x.max(0.0))`.
2. Rule: a bound max below the bound min clamps as the solver does today (min wins; `clamp_opt` applies min, then max). Builder decision: keep the solver's order and add a test documenting it.
3. Node-level `.fig` round-trip tests for `fontFamily`, `paragraphSpacing`, `paragraphIndent` and each of the four min/max keys.

**Leg B (font requests)**
4. After a mode switch, a variable edit or a collection mode change, request the faces of text nodes whose `fontFamily`/`fontStyle`/`fontWeight` resolve to a face not yet requested. Use the node set `changed_variables` returns, resolved through `resolve_node`. This must stay off the per-frame path: run once per switch, never per frame, and allocate nothing when no font binding changed (CLAUDE.md §4).
5. Respect the 25k-node cap in `requests_for`. The new path visits only bound text nodes; it does not walk the whole document.

**Leg C (only if Q1 = yes)**
6. Add min/max fields to the W/H section the way Figma does: a W/H dropdown with "Add min width…", and so on. Each field gets the always-visible bind icon (#876 rule), with a pill once bound. Follow the `design/principles.md` P3 and P11 copy and spacing rules.

## 4. Files likely touched

- Leg A: `crates/designos-doc/src/variables_apply.rs`; new `crates/designos-doc/tests/` or in-module tests; new `crates/designos-fig/tests/fig_node_typography_and_size_limit_bindings_round_trip.rs`
- Leg B: `crates/designos-app/src/variables/mod.rs` (mode-switch path), `crates/designos-app/src/fonts.rs` (`ensure_font_requests_with` call at `:127`)
- Leg C: `crates/designos-app/src/panels/properties.rs` (W/H rows), `crates/designos-app/src/panels/var_bind.rs`

## 5. Acceptance criteria

- **AC-1** (A) A child of an auto-layout row has `maxWidth` bound to a variable with modes 120 / 240 and Fill width in a 400 px row. Under mode 1 its solved width is 120; switch the mode and it is 240. Red on main (the width is 400 in both).
- **AC-2** (A) `scope_allows(["WIDTH_HEIGHT"], "minWidth", Frame)` is true; with `["GAP"]` it is false.
- **AC-3** (A) A `.fig` round trip keeps node bindings for `fontFamily`, `paragraphSpacing`, `paragraphIndent`, `minWidth`, `maxWidth`, `minHeight` and `maxHeight`. Each resolves per mode after reopen.
- **AC-4** (B) A text bound to a string variable whose values are "Inter" / "Lora", with Lora not cached: switching to the Lora mode issues exactly one `FontRequest { family: "Lora", .. }`. Doing nothing issues none. The test uses an `is_available` stub and is red on main.
- **AC-5** (B) No per-frame cost: a test drives 100 idle frames after the switch and sees zero new requests.
- **AC-6** (C, if yes) Desktop capture at 1440: min/max fields, the bind icon visible at rest, and a bound pill. `python3 scripts/zero_fake_ui_gate.py` passes.

## 6. Test steps

1. `cargo test -p designos-doc variables_apply`, `cargo test -p designos-fig --test fig_node_typography_and_size_limit_bindings_round_trip`, `cargo test -p designos-app --bin designos variables::`
2. Workspace gates: `cargo test --workspace --lib`, clippy, fmt.
3. Evidence: terminal captures for A and B. Leg C adds a 1440 capture (responsive: SKIPPED, owner default).

## 7. Edge cases

- `maxWidth` bound on a node that is not an auto-layout child: the value is stored, and the solver ignores it (as for a literal value today).
- A variable value of 0 for `minWidth`: Figma writes 0 for "unset" (`fig.rs:1785-1786`). The bound 0 is a real 0 and does not unset. Builder records this in the test.
- A font family the fetcher cannot find: the existing "Font unavailable" path applies, with no retry loop.
- An instance child whose limit comes from the main: the resolved value follows the instance's own binding first (`resolve_node` order).

## 8. Non-goals

- New scopes beyond `WIDTH_HEIGHT`.
- Min/max on grid tracks.
- Changing the solver's clamp order.

## 9. Figma parity

- Checked in code and schema: MIN/MAX field names in the kiwi `VariableField` enum (as mapped at `fig.rs:1971-1974`).
- Needs a live Figma check (owner's scratch file, Leg C only): the exact W/H dropdown labels and whether the bind icon sits on the min/max field row.

## 10. Builder rules

One PR per leg. `Refs #133` on A and B; the last leg merged uses `Closes #133`. Leg A touches `designos-fig` tests only, but it still waits for #1664 so the fixture helpers do not conflict.
