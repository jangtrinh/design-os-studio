---
status: ratified
---

# Design Soul — designos-studio

_Declared stance for `design:os studio`. Every `/ui:*` flow and studio agent reads this BEFORE picking personas, tokens, or layouts. Precedence: brief > soul (project > studio > factory) > memory prior > knowledge floors. The chrome is a neutral dark system with Studio orange as its brand; the layout and arrangement stay Studio's own. Values live in `crates/designos-app/src/theme.rs` and `design/design.tokens.json`; this file names them by their `theme.rs` constants and says when each one applies. Credit: the chrome's colour, type, spacing and radius steps are adapted from the Untitled UI design system (https://www.untitledui.com); this repository ships no Untitled UI file, icon or kit export._

## Never

- Never choose a gap by eye: every gap names the relationship it separates and takes that rung of the spacing ladder (`SPACE_FACT` … `SPACE_REGION`), and every value sits on the spacing scale (2, 4, 6, 8, 12, 16, 20, 24, 32, 40, 48 …); a gap that names no relationship is a defect — evidence: `crates/designos-app/src/theme.rs`
- Never let a label sit as close to the element above it as to the element it names; a name is always nearer its content than its neighbour — evidence: `crates/designos-app/src/theme.rs`
- Never nest a rounded shape off-curve: an inner radius is the outer radius minus the inset (`RADIUS_FRAME` 16 − `INSET_FRAME` 8 = `RADIUS_CONTROL` 8; `RADIUS_CONTROL` 8 − `INSET_SEGMENT` 2 = `RADIUS_SEGMENT` 6), pills inside pills included — evidence: `crates/designos-app/src/theme.rs`
- Never introduce a radius, spacing step, control height or type size that is not a theme role; a literal in a UI file is debt to retire, never a new value; the one carve-out is the editor tab heights the owner kept (R10.1: file tabs 26 in the 32 bar, session tabs and the Inspector segmented strip 24) — evidence: `scripts/check_spacing_relationships.py`
- Never use purple or violet for chrome, badges, focus rings, gradients or syntax colours, generic AI violet (`#9747FF`, `#8755E9`, `#A259FF`, `#C4A1FF`, `#A855F7`) and the purples `#7F56D9`, `#9E77ED`, `#B692F6` included; components are the `component` cyan and the primary action is Studio orange `#E36323` — evidence: `scripts/design_os_self_evolve.py`
- Never hardcode inline colour values in panels or views outside the central token registry — evidence: `crates/designos-app/src/theme.rs`
- Never draw a drop shadow on dark chrome: every shadow colour is alpha 0, so surfaces separate by fill step and a 1px hairline only — evidence: `crates/designos-app/src/theme.rs`
- Never put tertiary, placeholder, accent or error text on the pressed fill (`BG_PRESSED` `#404040`); only `TEXT` and `TEXT_SECONDARY` pass AA there — evidence: `design/design.tokens.json`
- Never silently fall back to `Recents` when a Home section (`Starred`, `Trash`, `Templates`, `Design Systems`, `Drafts`) has zero matching items — evidence: `crates/designos-app/src/home/mod.rs`
- Never ship decorative stub buttons, dead search bars or unwired menu items (gate `Z01..Z14`) — evidence: `scripts/zero_fake_ui_gate.py`
- Never concatenate item counts into labels like `"Layers (4)"`; counts render in a separate `Badge/StatusCount` pill (`D85`) — evidence: `plans/reports/audit-261016-vsf-pcp-ui-craft.md`
- Never let single-key canvas shortcuts fire while the embedded terminal or a text input has keyboard focus — evidence: `crates/designos-app/src/terminal.rs`
- Never set a tag, badge or chip in all caps: only the acronyms `AI`, `API`, `CLI`, `CSS`, `MCP`, `PRD`, `SVG`, `PNG`, `JPG`, `PDF`, `URL`, `ID`, `UI`, `AA` and `AAA` are upper case there, and tabs are not covered — evidence: `scripts/check_ds_ontology_parity.py` (P7 pins every upper-case site that remains)
- Never fill a selected tab, segment or row with the brand colour: selected is the neutral `BG_SELECTED`, and Studio orange stays for the primary action, live state and the focus ring — evidence: `scripts/check_ds_ontology_parity.py` (P7 pins the one segment that still does)
- Never put a brand-filled primary button in a table cell, list row or row item: row actions are `Button/Secondary`, `Button/Tertiary` or `Button/Icon` — evidence: `design/component-registry.json` (the `Table/Data` and `List/Row` composition); no gate enforces it until the chrome fingerprint test lands
- Never fill a control with a gradient (button, tab, row or bar); headers and cards are not covered — evidence: `scripts/check_ds_ontology_parity.py` (P7 pins the mesh constructors outside the canvas and board)

## Always

- Always build space tight to loose along one ladder: parts of one fact, rows of one group, peers side by side, group to group, block to block, section to section, region to region; each rung is a named constant on the spacing scale, two relationships may share a value but never swap order, and the 2px and 6px steps are allowed where a relationship needs them — evidence: `crates/designos-app/src/theme.rs`
- Always set a group at least twice its own inner gap away from its neighbours, so grouping reads before any line or fill does — evidence: `design/principles.md`
- Always let a child inherit from its parent: its inset is no larger than the parent's inset, its gap no larger than the parent's gap, and its radius is the parent's radius minus the inset — evidence: `crates/designos-app/src/theme.rs`
- Always pay each boundary once: a gap belongs to the element that starts the new relationship, never to a container's padding and a child's margin together, and the last child pays nothing — evidence: `design/principles.md`
- Always separate inset from gap: insets come from the inset family (`INSET_SEGMENT`, `INSET_CONTROL`, `INSET_FRAME`, `INSET_BUTTON`, `INSET_PANEL`, `INSET_POPOVER`, `INSET_MODAL`), gaps from the ladder — evidence: `crates/designos-app/src/theme.rs`
- Always draw a control at a set size: editor panels use the small end (`CONTROL_XS` 32 for buttons, rows and fields), Home and dialogs use `CONTROL_SM` 36 and `CONTROL_MD` 40, badges are 22, 24 or 28, tabs keep today's heights by owner ruling R10.1 (file tabs 26 in the 32 bar, session tabs and the Inspector segmented strip 24), and every pointer target stays at or above `TARGET_MIN` 24×24px (`WCAG 2.5.8 AA`), drag dividers included: a divider keeps its visible 1px line and takes a 24px hit band centred on it — evidence: `crates/designos-app/src/theme.rs`
- Always shape by role: buttons, fields, tabs and rows take `RADIUS_CONTROL` 8 at every size, Home and dialog buttons included; menus, dropdowns and popovers take `RADIUS_FRAME` 16 with items of radius 8 at inset 8; cards take `RADIUS_XL` 12; pills (radius = height ÷ 2) are kept for pill badges, count pills, filter chips, toggles and avatars — evidence: `crates/designos-app/src/theme.rs`
- Always set type by role: 12/18 (`FONT_SIZE_CAPTION`, `FONT_SIZE_BODY`) for editor rows, labels, captions and counts, 14/20 (`FONT_SIZE_LARGE`) for panel titles and for Home and dialog text, 16/24 (`FONT_SIZE_TEXT_MD`) for modal titles only, 20/30 (`FONT_SIZE_TITLE`) for page titles and the Home hero; button labels are 14 Semibold at every button size, while rows, labels and tabs stay 12; the 9px size is for canvas overlays only; weights are Regular 400, Medium 500 and Semibold 600 in Inter — evidence: `crates/designos-app/src/theme.rs`
- Always enforce the dark surface hierarchy: `BG` `#0A0A0A` (chrome base and text fields) < `BG_SECONDARY` `#171717` (cards, hover on `BG`) < `BG_TERTIARY` `#262626` (chips, selected) < `BG_PRESSED` `#404040` (pressed), separated by 1px `BORDER` `#262626` hairlines, with `BORDER_STRONG` `#404040` for the resting boundary of text fields, selects, secondary buttons, checkboxes and radios, and for active outlines — evidence: `design/design.tokens.json`
- Always keep chrome text at WCAG AA 4.5:1 on its surface, set on `TEXT` `#FAFAFA`, `TEXT_SECONDARY` `#D4D4D4`, `TEXT_TERTIARY` `#A3A3A3`; the one named exception is placeholder text, `TEXT_PLACEHOLDER` `#737373` (3.78:1 on `BG_SECONDARY`, 4.18:1 on `BG`, owner ruling 2026-10-25), which sits only in text fields: on their `BG` fill, or in a field inside a `BG_SECONDARY` card; the Inspector's "Mixed" value is not a placeholder and uses `TEXT_TERTIARY`; disabled is the control at opacity 0.5, and text-only disabled uses `TEXT_DISABLED` (`TEXT_SECONDARY` at 50%), never a new grey — evidence: `crates/designos-app/src/theme.rs`
- Always keep icons, focus rings and control boundaries at 3:1 against their surface (WCAG 1.4.11); the one named exception is `BORDER_STRONG` `#404040`, 1px (1.91:1 on `BG`, 1.73:1 on `BG_SECONDARY`), as the resting boundary of exactly these controls: unchecked checkboxes and radios (owner ruling R10.7), text fields, selects and secondary buttons (controller ruling C-F, which the owner may reverse), and only on `BG` or `BG_SECONDARY`; it is the second named exception after the placeholder, and these two are the only ones — evidence: `design/design.tokens.json`
- Always show which item is selected with a neutral fill first (`BG_SELECTED` `#262626`, one step above hover), and spend Studio orange on the one primary action per view, on live state and on the focus ring (`FOCUS_RING` `#F97316`, except a field in its error state and destructive buttons, which take the error ring `FOCUS_RING_ERROR` `#EF4444`); canvas selection stays Selection Blue — evidence: `crates/designos-app/src/theme.rs`
- Always put the dark label `TEXT_ON_BRAND` `#0A0A0A` on an orange or accent fill (5.73:1 on `#E36323`); white text (`TEXT_WHITE`) sits only on `BG_ERROR_SOLID` `#DC2626` and its hover `#B91C1C`, and tooltips use `TEXT` on `BG_PRIMARY_SOLID` — evidence: `crates/designos-app/src/theme.rs`
- Always render numeric coordinates, dimensions, hex codes and token paths in Roboto Mono (`theme::font_mono()`, `D6`), bundled with the app — evidence: `crates/designos-app/src/theme.rs`
- Always isolate embedded CLI agents per open document in `Panel/AgentTerminal`, with live status that tells idle, working, waiting and exited apart — evidence: `crates/designos-app/src/terminal.rs`
- Always preserve the 60fps viewport floor via `SceneIndex` culling and incremental GPU sync; chrome effects never touch the canvas frame cache — evidence: `plans/reports/perf-261016-figma-wasm-re.md`

## Voice

- Direct, technical, zero-filler studio nomenclature (`Frame`, `Component`, `Instance`, `Token`, `DirtyLog`, `Inspect`) with exact unit suffixes (`px`, `%`, `deg`, `ms`) — inherited: `crates/designos-app/src/theme.rs`
- Truthful empty and status states that name the exact section or action required, without marketing fluff — evidence: `crates/designos-app/src/home/mod.rs`

## Evidence pointers

- `crates/designos-app/src/theme.rs` — canonical colour, font, spacing, inset, control and radius constants; the invariant tests that pin their relationships
- `design/design.tokens.json` — the sealed token store, re-valued to the dark roles with one changelog entry per token
- `scripts/design_os_self_evolve.py` — G01 parity between `theme.rs` and the token store, and the Purple Ban list
- `scripts/check_spacing_relationships.py` — the ratchet over literal geometry, with its baseline in `scripts/spacing_baseline.json`
- `scripts/check_ds_ontology_parity.py` — the registry, Rust kit and token store parity gate, and the pinned sites of the absences above
- `design/component-registry.json` — the closed list of 78 chrome records, element to screen, with tier, composition, tokens and Rust symbol
- `crates/designos-app/src/home/mod.rs` — per-section Home filtering and card thumbnails
- `crates/designos-app/src/terminal.rs` — per-file PTY agent terminal and shortcut suppression
- `scripts/zero_fake_ui_gate.py` — `Z01..Z14` gate against dead controls
- `plans/reports/audit-261016-vsf-pcp-ui-craft.md` — 124-rule UI craft audit and its `D` rule ids
- `plans/reports/perf-261016-figma-wasm-re.md` — Figma C++/WASM reverse-engineering performance architecture report
