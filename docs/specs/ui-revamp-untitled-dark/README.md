# UI revamp: Untitled UI dark theme — task specs

**Pack goal.** Studio's chrome adopts the Untitled UI PRO v8.0 **dark** design system. That covers colour roles, the type, spacing and radius scales, borders, focus rings and component states. The current layout stays as it is: the same regions, panel order, panel widths and control placement. Studio is used to improve itself. The kit is read, mocked and onboarded with Studio's own CLI and MCP. Figma is used only read-only, to check kit facts against the live kit (R9).

Epic: #1118. One spec per issue; the number in a file name is the issue number. Every spec follows the 13-section layout of the v0.6 and v0.7 specs. Each spec tells the worker to re-anchor cited lines against HEAD and report drift before writing code. Specs were written on `ec47da26` and re-anchored to `origin/main@205c72ef` (spec L to `7e15ddae`) during review.

## Owner rulings (Jang, 2026-10-25)

| # | Ruling |
|---|---|
| R1 | 100% Untitled UI style; keep the current layout and arrangement. |
| R2 | Take everything from Untitled (spacing and typography too) and adapt it to our layout. |
| R3 | Use Studio to improve Studio; do not build or onboard through Figma. |
| R4 | Brand = Untitled anatomy with Studio orange `#E36323`, label `#0A0A0A` (5.73:1). Focus ring is Untitled Orange-500 `#F97316`. The Purple Ban stays. |
| R5 | Editor panels use the small end of the scale: text-xs 12/18 for rows and labels, text-sm 14/20 for panel titles, button xs 32, row 32, radius md 8. Home and dialogs use text-sm 14/20 with button sm 36 or md 40. The full Untitled spacing scale applies; the old ban on 2px and 6px steps is lifted. |
| R6 | Keep the current icon set and restyle it to Untitled fg roles. Research the Untitled Icons licence separately (Q). |
| R7 | Spec-driven: everything is specified before work starts. The PM coordinates delivery. |
| R8.1 | Placeholder text = kit `#737373` (named exception E-1 to the AA text rule). |
| R8.2 | Selected row, tool and tab fill = `#262626`. |
| R8.3 | Accent text = Orange-500 `#F97316`. |
| R8.4 | Inspector fields are 32 high. |
| R8.5 | Menus, dropdowns and popovers: radius 16, items radius 8 at inset 8. |
| R8.6 | Button labels text-sm 14 Semibold; rows, labels and tabs stay text-xs 12. |
| R8.7 | Destructive hover = Red-700 `#B91C1C`. |
| R9 | The Ease Design Figma Agent may read the live kit to verify facts; Studio builds, renders and onboards. |
| R10.1 | Editor tab heights stay as today: file tabs 26 in the 32 bar, session tabs 24, Inspector strip 24. |
| R10.2 | Home category tiles use the kit's utility colours (blue, amber, green, orange). |
| R10.3 | The selected Home file card has a 2 px `#FB923C` border. |
| R10.4 | Dev mode accent = `#14AE5C`. |
| R10.5 | The always-on rail help dot is removed. |
| R10.6 | The website palette moves to the Untitled dark neutrals. |
| R10.7 | Unchecked checkbox and radio borders keep the kit `#404040` (named exception E-2). |

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

- `evidence/baseline-editor-with-kit.png`: Studio's editor today (0.6.x) with the kit open. The Home baseline is not published here, because the first capture showed a personal library; spec L captures every baseline from a seeded library instead.
- `evidence/brainstorm-compare-board@2x.png`: the board the owner chose from. It was built in Studio with `--apply-spec` and rendered by `designos_render_png`.
- `evidence/owner-questions-board-1@2x.png` and `evidence/owner-questions-board-3@2x.png` (with their generators): the open questions the owner answered (R8, R10), built and rendered in Studio. Labels sit off-centre because of #1154.
- `evidence/1139-widget-kit-specimen@2x.png` (with its generator): spec C's widget kit in every state.
- `evidence/figma-live-variable-check.md`: the kit values match live Figma (colour variables 273/273 per mode, numbers 76/76).
- `evidence/button-label-offset@3x.png`: the #1154 measurement.

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
| I | 1 | #1144 | [1144-surface-agent-terminal-and-agent-cards.md](1144-surface-agent-terminal-and-agent-cards.md) | L |
| J | 1 | #1146 | [1146-surface-popovers-menus-modals-toasts-cmdk-find-comments.md](1146-surface-popovers-menus-modals-toasts-cmdk-find-comments.md) | M |
| K | 1 | #1155 | [1155-surface-assets-variables-libraries-styles.md](1155-surface-assets-variables-libraries-styles.md) | M |
| M | 2 | #1125 | [1125-docs-and-screenshot-refresh.md](1125-docs-and-screenshot-refresh.md) | M |
| N | side | #1121 | [1121-mcp-imports-large-kit-files.md](1121-mcp-imports-large-kit-files.md) | S |
| O | side | #1127 | [1127-cli-addresses-nodes-by-id-and-figma-id.md](1127-cli-addresses-nodes-by-id-and-figma-id.md) | M |
| P | side | #1140 | [1140-dtcg-export-bundle-for-design-os-onboarding.md](1140-dtcg-export-bundle-for-design-os-onboarding.md) | M |
| Q | side | #1122 | [1122-research-untitled-icons-licence.md](1122-research-untitled-icons-licence.md) | S |
| R | side | #1141 | [1141-cli-screenshot-capture-flake.md](1141-cli-screenshot-capture-flake.md) | M |
| S | side | #1154 | [1154-nodespec-text-measured-not-estimated.md](1154-nodespec-text-measured-not-estimated.md) | M |

Sizes: S is under about a day, M is one to three days, L is more than three days, for one worker.

## Order

Phase 0 runs in this order: L (baseline captured before anything changes), then A, then B, then C. A and B both edit `theme.rs`, so they land as sequential PRs. The Phase 1 surfaces (D–K) start once C has merged, and run in parallel with disjoint file ownership; each spec's section 12 lists its files and its conflicts with open PRs. M runs last. N, O, P, Q, R and S are independent. The token contract (`token-contract.md`) names every colour and size; its section (f) holds contrast check C-1, which every spec cites.
