---
status: ratified
---

# Design Soul — designos-studio

_Declared stance for `design:os studio`. Every `/ui:*` flow and studio agent reads this BEFORE picking personas, tokens, or layouts. Precedence: brief > soul (project > studio > factory) > memory prior > knowledge floors. The chrome speaks the Untitled UI PRO v8.0 dark language with Studio orange as its brand; the layout and arrangement stay Studio's own. Values live in `crates/designos-app/src/theme.rs` and `design/design.tokens.json`; this file names them by their Untitled token and says when each one applies._

## Never

- Never choose a gap by eye: every gap names the relationship it separates and takes that rung of the spacing ladder (`SPACE_FACT` … `SPACE_REGION`), and every value sits on the Untitled spacing scale (2, 4, 6, 8, 12, 16, 20, 24, 32, 40, 48 …); a gap that names no relationship is a defect — evidence: `crates/designos-app/src/theme.rs`
- Never let a label sit as close to the element above it as to the element it names; a name is always nearer its content than its neighbour — evidence: `crates/designos-app/src/theme.rs`
- Never nest a rounded shape off-curve: an inner radius is the outer radius minus the inset (`RADIUS_FRAME` 16 − `INSET_FRAME` 8 = `RADIUS_CONTROL` 8; `RADIUS_CONTROL` 8 − `INSET_SEGMENT` 2 = `RADIUS_SEGMENT` 6), pills inside pills included — evidence: `crates/designos-app/src/theme.rs`
- Never introduce a radius, spacing step, control height or type size that is not a theme role on the Untitled scale; a literal in a UI file is debt to retire, never a new value — evidence: `scripts/check_spacing_relationships.py`
- Never use purple or violet for chrome, badges, focus rings, gradients or syntax colours, the kit's own purple brand (`#7F56D9`, `#9E77ED`, `#B692F6`) included; components are the `component` cyan and the primary action is Studio orange `#E36323` — evidence: `scripts/design_os_self_evolve.py`
- Never hardcode inline colour values in panels or views outside the central token registry — evidence: `crates/designos-app/src/theme.rs`
- Never draw a drop shadow on dark chrome: every Untitled dark shadow resolves to alpha 0, so surfaces separate by fill step and a 1px hairline only — evidence: `crates/designos-app/src/theme.rs`
- Never put tertiary, placeholder, accent or error text on the pressed fill (`bg-quaternary` `#404040`); only `text-primary` and `text-secondary` pass AA there — evidence: `design/design.tokens.json`
- Never silently fall back to `Recents` when a Home section (`Starred`, `Trash`, `Templates`, `Design Systems`, `Drafts`) has zero matching items — evidence: `crates/designos-app/src/home/mod.rs`
- Never ship decorative stub buttons, dead search bars or unwired menu items (gate `Z01..Z14`) — evidence: `scripts/zero_fake_ui_gate.py`
- Never concatenate item counts into labels like `"Layers (4)"`; counts render in a separate `Badge/StatusCount` pill (`D85`) — evidence: `plans/reports/audit-261016-vsf-pcp-ui-craft.md`
- Never let single-key canvas shortcuts fire while the embedded terminal or a text input has keyboard focus — evidence: `crates/designos-app/src/terminal.rs`

## Always

- Always build space tight to loose along one ladder: parts of one fact, rows of one group, peers side by side, group to group, block to block, section to section, region to region; each rung is a named constant on the Untitled spacing scale, two relationships may share a value but never swap order, and the 2px and 6px steps are allowed where a relationship needs them — evidence: `crates/designos-app/src/theme.rs`
- Always set a group at least twice its own inner gap away from its neighbours, so grouping reads before any line or fill does — evidence: `design/principles.md`
- Always let a child inherit from its parent: its inset is no larger than the parent's inset, its gap no larger than the parent's gap, and its radius is the parent's radius minus the inset — evidence: `crates/designos-app/src/theme.rs`
- Always pay each boundary once: a gap belongs to the element that starts the new relationship, never to a container's padding and a child's margin together, and the last child pays nothing — evidence: `design/principles.md`
- Always separate inset from gap: insets come from the inset family (`INSET_SEGMENT`, `INSET_CONTROL`, `INSET_FRAME`, `INSET_BUTTON`, `INSET_PANEL`, `INSET_POPOVER`, `INSET_MODAL`), gaps from the ladder — evidence: `crates/designos-app/src/theme.rs`
- Always draw a control at an Untitled size: editor panels use the small end (`CONTROL_XS` 32 for buttons, rows and fields), Home and dialogs use `CONTROL_SM` 36 and `CONTROL_MD` 40, badges are 22, 24 or 28, and every pointer target stays at or above `TARGET_MIN` 24×24px (`WCAG 2.5.8 AA`) — evidence: `crates/designos-app/src/theme.rs`
- Always shape by role: buttons, fields, menus, tabs and rows take `radius-md` 8 (`RADIUS_CONTROL`) at every size, Home and dialog buttons included; floating chrome takes `RADIUS_FRAME` 16; pills (radius = height ÷ 2) are kept for pill badges, count pills, filter chips, toggles and avatars — evidence: `crates/designos-app/src/theme.rs`
- Always set type by role on the Untitled scale: `text-xs` 12/18 for editor rows, labels, captions and counts, `text-sm` 14/20 for panel titles and for Home and dialog text, `text-xl` 20/30 for page titles and the Home hero; the 9px size is for canvas overlays only; weights are Regular 400, Medium 500 and Semibold 600 in Inter — evidence: `crates/designos-app/src/theme.rs`
- Always enforce the Untitled Neutral dark hierarchy: `bg-primary` `#0A0A0A` (chrome base) < `bg-secondary` `#171717` (fields, cards) < `bg-tertiary` `#262626` (chips, selected) < `bg-quaternary` `#404040` (pressed), separated by 1px `border-secondary` `#262626` hairlines, with `border-primary` `#404040` for inputs and active outlines — evidence: `design/design.tokens.json`
- Always set text on the Untitled steps: `text-primary` `#FAFAFA`, `text-secondary` `#D4D4D4`, `text-tertiary` `#A3A3A3`; disabled is the control at opacity 0.5, never a separate grey — evidence: `crates/designos-app/src/theme.rs`
- Always show which item is selected with a neutral fill first, and spend Studio orange on the one primary action per view, on live state and on the focus ring (`focus-ring` `#F97316`); canvas selection stays Selection Blue — evidence: `crates/designos-app/src/theme.rs`
- Always put the dark label `#0A0A0A` on an orange or accent fill (5.76:1 on `#E36323`); white text sits only on `bg-error-solid` — evidence: `crates/designos-app/src/theme.rs`
- Always render numeric coordinates, dimensions, hex codes and token paths in Roboto Mono (`theme::font_mono()`, `D6`), bundled with the app — evidence: `crates/designos-app/src/theme.rs`
- Always isolate embedded CLI agents per open document in `Panel/AgentTerminal`, with live status that tells idle, working, waiting and exited apart — evidence: `crates/designos-app/src/terminal.rs`
- Always preserve the 60fps viewport floor via `SceneIndex` culling and incremental GPU sync; chrome effects never touch the canvas frame cache — evidence: `plans/reports/perf-261016-figma-wasm-re.md`

## Voice

- Direct, technical, zero-filler studio nomenclature (`Frame`, `Component`, `Instance`, `Token`, `DirtyLog`, `Inspect`) with exact unit suffixes (`px`, `%`, `deg`, `ms`) — inherited: `crates/designos-app/src/theme.rs`
- Truthful empty and status states that name the exact section or action required, without marketing fluff — evidence: `crates/designos-app/src/home/mod.rs`

## Evidence pointers

- `crates/designos-app/src/theme.rs` — canonical colour, font, spacing, inset, control and radius constants, each named by its Untitled token; the invariant tests that pin their relationships
- `design/design.tokens.json` — the sealed token store, re-valued to the Untitled dark roles with one changelog entry per token
- `scripts/design_os_self_evolve.py` — G01 parity between `theme.rs` and the token store, and the Purple Ban list
- `scripts/check_spacing_relationships.py` — the ratchet over literal geometry, with its baseline in `scripts/spacing_baseline.json`
- `crates/designos-app/src/home/mod.rs` — per-section Home filtering and card thumbnails
- `crates/designos-app/src/terminal.rs` — per-file PTY agent terminal and shortcut suppression
- `scripts/zero_fake_ui_gate.py` — `Z01..Z14` gate against dead controls
- `plans/reports/audit-261016-vsf-pcp-ui-craft.md` — 124-rule UI craft audit and its `D` rule ids
- `plans/reports/perf-261016-figma-wasm-re.md` — Figma C++/WASM reverse-engineering performance architecture report
