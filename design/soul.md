---
status: ratified
---

# Design Soul — designos-studio

_Declared stance for `design:os studio`. Every `/ui:*` flow and studio agent reads this BEFORE picking personas, tokens, or layouts. Precedence: brief > soul (project > studio > factory) > memory prior > knowledge floors. Values live in `crates/designos-app/src/theme.rs` and `design/design.tokens.json`; this file names them and says when each one applies._

## Never

- Never choose a gap by eye: every gap names the relationship it separates and takes that rung of the spacing ladder (`SPACE_FACT` … `SPACE_REGION`); a gap that names no relationship is a defect — evidence: `crates/designos-app/src/theme.rs`
- Never let a label sit as close to the element above it as to the element it names; a name is always nearer its content than its neighbour — evidence: `crates/designos-app/src/theme.rs`
- Never nest a rounded shape off-curve: an inner radius is the outer radius minus the inset (`RADIUS_FRAME` 14 − `INSET_FRAME` 8 = `RADIUS_CONTROL` 6), pills inside pills included — evidence: `crates/designos-app/src/theme.rs`
- Never introduce a radius, spacing step, control height or type size that is not a theme role; a literal in a UI file is debt to retire, never a new value — evidence: `scripts/check_spacing_relationships.py`
- Never use purple or violet for chrome, badges, focus rings, gradients or syntax colours; components are the `component` cyan and the primary action is the `primary` orange — evidence: `crates/designos-app/src/theme.rs`
- Never hardcode inline colour values in panels or views outside the central token registry — evidence: `crates/designos-app/src/theme.rs`
- Never silently fall back to `Recents` when a Home section (`Starred`, `Trash`, `Templates`, `Design Systems`, `Drafts`) has zero matching items — evidence: `crates/designos-app/src/home/mod.rs`
- Never ship decorative stub buttons, dead search bars or unwired menu items (gate `Z01..Z14`) — evidence: `scripts/zero_fake_ui_gate.py`
- Never concatenate item counts into labels like `"Layers (4)"`; counts render in a separate `Badge/StatusCount` pill (`D85`) — evidence: `plans/reports/audit-261016-vsf-pcp-ui-craft.md`
- Never let single-key canvas shortcuts fire while the embedded terminal or a text input has keyboard focus — evidence: `crates/designos-app/src/terminal.rs`

## Always

- Always build space tight to loose along one ladder: parts of one fact, rows of one group, peers side by side, group to group, block to block, section to section, region to region; each rung is a named constant, and two relationships may share a value but never swap order — evidence: `crates/designos-app/src/theme.rs`
- Always set a group at least twice its own inner gap away from its neighbours, so grouping reads before any line or fill does — evidence: `design/principles.md`
- Always let a child inherit from its parent: its inset is no larger than the parent's inset, its gap no larger than the parent's gap, and its radius is the parent's radius minus the inset — evidence: `crates/designos-app/src/theme.rs`
- Always pay each boundary once: a gap belongs to the element that starts the new relationship, never to a container's padding and a child's margin together, and the last child pays nothing — evidence: `design/principles.md`
- Always separate inset from gap: insets come from the inset family (`INSET_CONTROL`, `INSET_BUTTON`, `INSET_FRAME`, `INSET_PANEL`, `INSET_POPOVER`, `INSET_MODAL`), gaps from the ladder — evidence: `crates/designos-app/src/theme.rs`
- Always draw a control at an explicit height from the control family (`CONTROL_DENSE` 24, `CONTROL_STANDARD` 28, `CONTROL_TOOL` 32, `CONTROL_ROW` 40; segmented tab strips are 24) and keep every pointer target at or above `TARGET_MIN` 24×24px (`WCAG 2.5.8 AA`) — evidence: `crates/designos-app/src/theme.rs`
- Always round chips, badges, status marks, filter chips and dialog and Home buttons into pills (radius = height ÷ 2); fields, tool buttons and inspector rows stay rectangular at `RADIUS_CONTROL` — evidence: `crates/designos-app/src/theme.rs`
- Always set type by role, never by size: caption 10, body 11, heading 13, title 20; the 9px canvas size is for canvas overlays only; weights are 450, 500 and 550 — evidence: `crates/designos-app/src/theme.rs`
- Always show which item is selected with a neutral fill (`bg-active`) first, and spend the `primary` orange on the one primary action per view and on live state; canvas selection stays Selection Blue — evidence: `crates/designos-app/src/theme.rs`
- Always put a dark label on a light or accent fill (`primary-foreground` on `primary`) — evidence: `crates/designos-app/src/theme.rs`
- Always enforce the Zinc dark surface hierarchy (`canvas` < panel chrome < `card` < hover) separated by 1px `border` hairlines — evidence: `crates/designos-app/src/theme.rs`
- Always render numeric coordinates, dimensions, hex codes and token paths in Roboto Mono (`theme::font_mono()`, `D6`), bundled with the app — evidence: `crates/designos-app/src/theme.rs`
- Always isolate embedded CLI agents per open document in `Panel/AgentTerminal`, with live status that tells idle, working, waiting and exited apart — evidence: `crates/designos-app/src/terminal.rs`
- Always preserve the 60fps viewport floor via `SceneIndex` culling and incremental GPU sync; chrome effects never touch the canvas frame cache — evidence: `plans/reports/perf-261016-figma-wasm-re.md`

## Voice

- Direct, technical, zero-filler studio nomenclature (`Frame`, `Component`, `Instance`, `Token`, `DirtyLog`, `Inspect`) with exact unit suffixes (`px`, `%`, `deg`, `ms`) — inherited: `crates/designos-app/src/theme.rs`
- Truthful empty and status states that name the exact section or action required, without marketing fluff — evidence: `crates/designos-app/src/home/mod.rs`

## Evidence pointers

- `crates/designos-app/src/theme.rs` — canonical colour, font, spacing, inset, control and radius constants; the invariant tests that pin their relationships
- `scripts/check_spacing_relationships.py` — the ratchet over literal geometry, with its baseline in `scripts/spacing_baseline.json`
- `crates/designos-app/src/home/mod.rs` — per-section Home filtering and card thumbnails
- `crates/designos-app/src/terminal.rs` — per-file PTY agent terminal and shortcut suppression
- `scripts/zero_fake_ui_gate.py` — `Z01..Z14` gate against dead controls
- `plans/reports/audit-261016-vsf-pcp-ui-craft.md` — 124-rule UI craft audit and its `D` rule ids
- `plans/reports/perf-261016-figma-wasm-re.md` — Figma C++/WASM reverse-engineering performance architecture report
