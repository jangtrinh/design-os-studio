# #1472 Create multiple components (and Figma's single-component wrap)

| Field | Value |
|---|---|
| Issue | #1472 (epic #1487; part of #16) |
| Code baseline | `origin/main@d22dad970`. Re-anchor before coding. |
| Verdict | **Real, but the issue's description of Studio is inverted.** Studio's "Create component" already makes one component per selected layer. The gap is the reverse: no "Create multiple components" command, and no Figma-style "wrap the whole selection into one component". |
| Size | M: 5–7 h |
| Tier | Claude Sonnet (changes the meaning of an existing shortcut; menus are rendered UI so `es-designer` loads first) |
| Touches | `crates/designos-app/src/comp_ops.rs`, `crates/designos-app/src/panels/menus.rs`, `crates/designos-app/src/panels/context.rs`, `crates/designos-app/src/shell.rs` (only if a shortcut changes), UX-writing baseline `scripts/ux_writing_baseline.json` if the gate lists menu strings |
| Depends on | Owner Q1. No open PR touches these files (checked `gh pr list` 2026-11-06: #1684 touches only `shell/tests/leave_menu_quit.rs`). |

## 1. Still real? (FACT)

- `App::create_component` (`crates/designos-app/src/comp_ops.rs:259-276`) loops over `top_selection()` and calls `designos_doc::create_component` per layer inside one `Tx` (one undo), toast "Created components" for N > 1. So N selected layers already give N components, one undo.
- Doc `create_component` (`crates/designos-doc/src/components.rs:576-611`): Frame → Component in place; Group/Section → Component; Instance → detach as component; other leaves → wrapped in a new Component node.
- Menus: Object menu "Create component" `panels/menus.rs:559`; a second menu list `:721`; canvas context menu `panels/context.rs:139`. Shortcut Ctrl+Alt+K → `Action::CreateComponent` (`shell.rs:1446`). No "Create multiple components" item anywhere (grep: 0 hits).
- Frame selection already wraps a same-parent selection in one container: `ops.rs:1458-1469` (`group_selection(frame: true)` → `wrap_in_container`), Ctrl+Alt+G.

## 2. Figma-parity facts (FACT, Figma Help "Create components to reuse in designs", help.figma.com/hc/en-us/articles/360038663154, fetched 2026-11-06)

- Create component with several layers selected: "Figma will nest the layers within a special component frame" (one component).
- "Create multiple components": select layers, open the Create component options menu beside the selection's name in the right sidebar, choose "Create multiple components". "Figma will create a component for each frame, group, boolean operation, or path."
- Shortcut: Ctrl+Alt+K / ⌥⌘K (single component).

## 3. User-visible outcome (assuming Q1 recommended answer)

- Object menu and canvas context menu show "Create component" (Ctrl+Alt+K) and, below it, "Create multiple components".
- With one layer selected both behave as today. With N layers selected: "Create component" wraps them in one new component (like Frame selection, then convert); "Create multiple components" makes N components.
- Each command is one undo step. Selection after: the new component(s). Toast "Created component" / "Created {n} components".

## 4. Behaviour rules (decided)

- "Create multiple components" = today's `App::create_component` body, unchanged (rename to `create_components_per_layer`).
- "Create component" with ≥ 2 top layers: `same_parent_tops()` (`ops.rs`, used by Frame selection) → `wrap_in_container(&mut tx, &ids, true)` → `create_component` on the wrapper, in the same `Tx`. Name: "Component 1"-style default the doc uses for new components (`[UNVERIFIED]` exact default naming; builder checks `create_component`'s wrapper naming at `components.rs:593-596`, which copies the leaf's name; for a multi-wrap use the Frame selection's default frame name then convert).
- Enablement: "Create multiple components" enabled when ≥ 2 top layers are selected and at least one can become a component (same predicate as `can_create` at `context.rs`), else disabled (Zero Fake UI: never an enabled no-op).
- Layers that are already components: skipped by "Create multiple components" (doc returns `nothing()`), counted in the toast only by what was made.

## 5. Acceptance criteria (red on `d22dad970`)

- AC-1 Menu: Object menu and context menu contain "Create multiple components" right after "Create component"; disabled with 0–1 layer selected. App test reads both menu models.
- AC-2 Per-layer: select 3 rectangles → "Create multiple components" → 3 Component nodes, one undo restores 3 rectangles.
- AC-3 Wrap (per Q1): select 3 rectangles → Ctrl+Alt+K → 1 Component holding 3 rectangles at unchanged page positions; one undo restores.
- AC-4 Mixed selection: frame + instance + text → per-layer: frame becomes component in place, instance detaches into a component, text gets wrapped (existing doc rules); wrap: one component holding all three.
- AC-5 Cross-parent selection: wrap uses only same-parent layers (Frame selection rule); per-layer handles all tops.
- AC-6 UX-writing gate and Zero Fake UI gate pass: `python3 scripts/zero_fake_ui_gate.py`, ux-writing check used by CI (`scripts/ux_writing_baseline.json`).
- AC-7 Screenshot of both menus with the new item (desktop 1440 capture), attached to the PR.

## 6. Test steps

1. Red tests in `comp_ops.rs` tests module for AC-1..AC-5.
2. Implement; `cargo test -p designos-app --lib comp_ops`.
3. Gates: zero fake UI, design self-evolve parity (`python3 scripts/design_os_self_evolve.py`).
4. Build, open Studio, select 3 layers, open Object menu → screenshot; run both commands → screenshot layers panel.

## 7. Edge cases

- Selection inside an instance (non-top layers): today `top_selection` decides; keep.
- Selection containing a Page / Section: Section → component (doc rule); Page → skipped.
- Layers inside auto layout: wrap inserts the component at the first layer's index (Frame selection behaviour); per-layer keeps each in place.
- A layer whose wrapping would make a main hold itself (layer inside a main and it is an instance of it): doc refuses; toast counts only made ones.

## 8. Non-goals

The right-sidebar "Create component options" dropdown (Figma's location); "Create component set" (exists as Combine as variants, Ctrl+Shift+K).

## 9. Undecided user-visible details → README owner questions

- Q1: does Ctrl+Alt+K switch to Figma's wrap-into-one for multi-selection (and the per-layer behaviour move to the new item)?
- Toast wording: decided "Created {n} components" (PCP ux-writing: ≤ 8 words).
- Whether "Create multiple components" gets a shortcut: decided none (Figma has none).

## 10. Builder tier

Claude Sonnet.

## Epistemic ledger

- FACT: per-layer behaviour exists today (`comp_ops.rs:259-276`); issue text says the opposite.
- FACT: Figma wraps on Create component and per-layer on Create multiple components (help article).
- ASSUMPTION (medium): owner wants Figma parity for Ctrl+Alt+K. Flips on Q1.
