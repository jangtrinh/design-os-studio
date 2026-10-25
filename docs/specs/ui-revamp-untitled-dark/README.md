# UI revamp: Untitled UI dark theme — task specs

**Pack goal.** Studio's chrome adopts the Untitled UI PRO v8.0 **dark** design system. That covers colour roles, the type, spacing and radius scales, borders, focus rings and component states. The current layout stays as it is: the same regions, panel order, panel widths and control placement. Studio is used to improve itself. The kit is read, mocked and onboarded with Studio's own CLI and MCP, and Figma is not used.

Epic: #1118. One spec per issue; the number in a file name is the issue number. Every spec follows the 13-section layout of the v0.6 and v0.7 specs. Each spec tells the worker to re-anchor cited lines against HEAD and report drift before writing code. Specs were written on `ec47da26`; `origin/main` was `35a65796` (0.6.1) when they were published.

## Owner rulings (Jang, 2026-10-25)

| # | Ruling |
|---|---|
| R1 | 100% Untitled UI style; keep the current layout and arrangement. |
| R2 | Take everything from Untitled (spacing and typography too) and adapt it to our layout. |
| R3 | Use Studio to improve Studio; no Figma. |
| R4 | Brand = Untitled anatomy with Studio orange `#E36323`, label `#0A0A0A` (5.76:1). Focus ring is Untitled Orange-500 `#F97316`. The Purple Ban stays. |
| R5 | Editor panels use the small end of the scale: text-xs 12/18 for rows and labels, text-sm 14/20 for panel titles, button xs 32, row 32, radius md 8. Home and dialogs use text-sm 14/20 with button sm 36 or md 40. The full Untitled spacing scale applies; the old ban on 2px and 6px steps is lifted. |
| R6 | Keep the current icon set and restyle it to Untitled fg roles. Research the Untitled Icons licence separately (Q). |
| R7 | Spec-driven: everything is specified before work starts. The PM coordinates delivery. |

Controller defaults (the owner may reverse them):
- Canvas and Dev overlays keep their Figma-parity colours, because Untitled has no canvas roles. These are selection blue `#0D99FF`/`#3582FF`, component cyan `#06B6D4`, guides `#F24822`/`#FE5938` and Dev ready `#14AE5C`.
- Chrome status colours move to Untitled.
- Document content does not change: the page background and default fills.
- The terminal's ANSI palette is content and stays.

## Dark palette (the kit's own variables, exported by Studio)

| Role | Value |
|---|---|
| Background | bg-primary `#0A0A0A`, bg-secondary `#171717`, bg-tertiary `#262626`, bg-quaternary `#404040` |
| Border | border-primary `#404040`, border-secondary `#262626` |
| Text | text-primary `#FAFAFA`, text-secondary `#D4D4D4`, text-tertiary `#A3A3A3`, text-placeholder `#737373` |
| Status | error `#EF4444` (text `#F87171`), warning `#EAB308`, success `#22C55E` |
| Shadows | none in dark mode: every dark shadow token is transparent, so hairlines separate surfaces |
| Radius | xxs 2, xs 4, sm 6, md 8, lg 10, xl 12, 2xl 16 |
| Spacing | xxs 2, xs 4, sm 6, md 8, lg 12, xl 16, 2xl 20, 3xl 24, 4xl 32, 6xl 48 |
| Buttons | xs 32, sm 36, md 40, lg 44, xl 48 |

How the values were read: `designos <kit.fig> --export-variables <dir> --format dtcg`, then alias resolution. The raw export is not committed, because the kit is a paid product and this repo is public. Regenerate it locally from the kit file.

## Evidence

- `evidence/baseline-home.png`, `evidence/baseline-editor-with-kit.png`: Studio today, 0.6.x.
- `evidence/brainstorm-compare-board@2x.png`: the board the owner chose from. It was built in Studio with `--apply-spec` and rendered by `designos_render_png`.

## Index

| Key | Phase | Issue | Spec | Size |
|---|---|---|---|---|
| L | 0, first | #1126 | [1126-layout-unchanged-gate-baseline-capture.md](1126-layout-unchanged-gate-baseline-capture.md) | M |
| A | 0 | #1142 | [1142-onboard-untitled-dark-colour-roles-and-soul.md](1142-onboard-untitled-dark-colour-roles-and-soul.md) | M |
| B | 0 | #1143 | [1143-untitled-type-spacing-radius-control-scale.md](1143-untitled-type-spacing-radius-control-scale.md) | M |
| C | 0 | #1139 | [1139-widget-kit-untitled-anatomy.md](1139-widget-kit-untitled-anatomy.md) | L |
| D | 1 | #1119 | [1119-surface-app-shell-tabs-rail-toolbar-rulers-menus.md](1119-surface-app-shell-tabs-rail-toolbar-rulers-menus.md) | M |
| E | 1 | #1120 | [1120-surface-left-panel-pages-layers.md](1120-surface-left-panel-pages-layers.md) | M |
| F | 1 | #1124 | [1124-surface-right-panel-inspector-design-prototype.md](1124-surface-right-panel-inspector-design-prototype.md) | L |
| G | 1 | #1128 | [1128-surface-dev-mode-and-spec-reader.md](1128-surface-dev-mode-and-spec-reader.md) | M |
| H | 1 | #1138 | [1138-surface-home-workspace.md](1138-surface-home-workspace.md) | M |
| I | 1 | — | Agent terminal panel and agent cards (spec in progress) | — |
| J | 1 | — | Popovers, menus, modals, toasts, command palette, Find, Comments (spec in progress) | — |
| K | 1 | — | Assets, Variables, Libraries and Styles panels (spec in progress) | — |
| M | 2 | #1125 | [1125-docs-and-screenshot-refresh.md](1125-docs-and-screenshot-refresh.md) | M |
| N | side | #1121 | [1121-mcp-imports-large-kit-files.md](1121-mcp-imports-large-kit-files.md) | S |
| O | side | #1127 | [1127-cli-addresses-nodes-by-id-and-figma-id.md](1127-cli-addresses-nodes-by-id-and-figma-id.md) | M |
| P | side | #1140 | [1140-dtcg-export-bundle-for-design-os-onboarding.md](1140-dtcg-export-bundle-for-design-os-onboarding.md) | M |
| Q | side | #1122 | [1122-research-untitled-icons-licence.md](1122-research-untitled-icons-licence.md) | S |

Sizes: S is under about a day, M is one to three days, L is more than three days, for one worker.

## Order

Phase 0 runs in this order: L (baseline captured before anything changes), then A, then B, then C. A and B both edit `theme.rs`, so they land as sequential PRs. The Phase 1 surfaces (D–K) start once C has merged, and run in parallel with disjoint file ownership; each spec's section 12 lists its files and its conflicts with open PRs. M runs last. N, O, P and Q are independent.
