# Token contract — Untitled UI PRO v8.0 dark → DESIGN:OS Studio (specs A and B)

| Field | Value |
|---|---|
| Owners | Spec A (colours, token store, Rust colour names) · Spec B (type, spacing, radius, control sizes) |
| Code baseline | `ec47da26` (every line number below is `crates/designos-app/src/theme.rs` at that commit unless a path is given) |
| Drift seen | `origin/main@35a65796` adds `pub const TOAST_REVEAL_SECONDS: f64 = 10.0;` after line 463 (+2 lines, unchanged by this contract). Re-anchor every line number against HEAD before editing. |
| Naming | Untitled token names as the kit names them (`bg-primary_hover`); the design:os store writes `_` as `-` (`bg-primary-hover`) on import. Rust names are the existing `theme.rs` names; every name this contract adds is marked "new name". |
| Rulings | R1–R7 and the controller defaults in `plans/261025-1247-untitled-ui-dark-revamp/plan.md` |

Other specs refer to Untitled names only; this file is the single place that maps them to Rust constants and
`design/design.tokens.json` paths (plan.md, "Shared naming contract").

## 0. Sources and coverage

### Kit values (every hex and size below comes from one of these commands)

| What | Command (run from the repo root; the kit is a paid file, its raw export is never committed) |
|---|---|
| Colour, radius, spacing variables, dark mode, aliases resolved | `cargo run -q --release -p designos-app --bin designos -- "$KIT" --export-variables <dir> --format dtcg`, then `python3 plans/261025-1247-untitled-ui-dark-revamp/evidence/resolve-untitled-tokens.py <dir>` |
| Text styles (size / line height) | `cargo run -q --release -p designos-app --bin designos -- "$KIT" --select none --screenshot <out>.png --size 1600x7000 --no-toast`, Styles panel read from the right 240 px (Text xs 12/18, sm 14/20, md 16/24, lg 18/28, xl 20/30, Display xs 24/32 … 2xl 72/90) |
| Font weights | `… --select "_Type scale base" --export-spec <out>.json` (frame labels "Font weight: 400 / 500 / 600 / 700"; typeface Inter) |
| Component anatomy (buttons, inputs, menus, tabs, nav items, badges) | `… --select "<component set name>" --export-spec <out>.json` (e.g. `Buttons/Button`, `_Nav item base`, `Badge`, `Dropdown menu`) |

`$KIT` = `~/Downloads/❖ Untitled UI Figma – PRO VARIABLES (v8.0) gKXp7LqFY2NV (Copy).fig`.

**Verification of the dark values (FACT, 2026-10-25).** `--export-variables --format dtcg` writes one file per collection
name and overwrites same-named collections (spec P, #1140): the kit has 10 variable collections, three names twice
(`_Primitives`, `1. Color modes`, `6. Typography`, each a local copy and a library copy). Check run:
`cargo run -q --release -p designos-app --bin designos -- "$KIT" --export-variables all.json --format figma-rest`
(one file, collections keyed by id, nothing overwritten), then `verify_dark_values_rest.py` (resolve every variable of
both copies in Dark mode / Style through the alias chain and diff against the DTCG-resolved values used here):
"compared 575 identical 575 diffs 0". The library copies hold only 6 variables (`_Primitives`: Neutral/400 `#A3A3A3`,
Neutral/600, Base/transparent; `1. Color modes`: text-tertiary, shadow-xs; `6. Typography`: text-sm 14), each with
an identical local twin; the local copies (343 and 273 variables) are the ones the DTCG files kept. So every colour
in this contract is verified. The DTCG overwrite did lose the local `6. Typography` (32 variables) to its one-variable
library copy; the REST export reads it: Font size text-xs 12, sm 14, md 16, lg 18, xl 20 (display 24 … 72); Line
height 18, 20, 24, 28, 30 (display 32 … 90); Font weight Regular, Medium, Semibold, Bold; family Inter. That makes
spec B's type scale a variable FACT, matching the Styles panel reading.

### Coverage (acceptance 4)

| Family | Grep (on `theme.rs` at `ec47da26`) | In file | Covered here |
|---|---|---|---|
| All constants | `grep -cE '^pub const ' theme.rs` → 451 lines, of which 2 are `pub const fn` | 449 | 449 |
| Colour, `Color32` and `[Color32; N]` | `grep -cE '^pub const [A-Z0-9_]+: (Color32\|\[Color32; ?[0-9]+\]) =' theme.rs` | 116 | 116 in (a): 116 constants covered; alias 4, delete 5, keep 52, revalue 55; new 23 (N1–N13 from spec A, N14–N23 for spec C) |
| Colour, `u32` / `[u32; 8]` (document defaults, board) | `grep -nE '^pub const [A-Z0-9_]+: (u32\|\[u32; ?[0-9]+\]) =' theme.rs` | 7 | 7 in (e), unchanged |
| Numeric (`f32` 306, `f64` 1, `u8` 6, `usize` 4, `(f32, f32)` 2, `[f32; N]` 2) | `grep -cE '^pub const [A-Z0-9_]+: (f32\|f64\|u8\|usize\|\(f32, ?f32\)\|\[f32; ?[0-9]\]) =' theme.rs` | 321 | 321: 21 changed or re-pointed + 2 renamed + 2 legacy in (c), 296 unchanged in the appendix |
| Strings (`&str`: font families, `APP_TITLE`) | `grep -cE '^pub const [A-Z0-9_]+: &str =' theme.rs` | 5 | 5 in (e), unchanged |

The numeric values were evaluated (aliases and expressions resolved) for HEAD and for a scratch copy of
`theme.rs` with this contract applied; no constant outside (c) changes value (0 implicit changes).

## (a) Colour constants in `theme.rs` → Untitled token

Actions: **revalue** (new literal), **keep** (value unchanged; canvas, Dev overlay, document or Studio-only role),
**alias** (stays an alias of the named constant), **delete** (0 references outside `theme.rs`, checked with
`grep -rnw <NAME> crates/designos-app/src --exclude=theme.rs`).

| # | Constant (`theme.rs` line) | HEAD value | Action | New value | Untitled token | Reason |
|---|---|---|---|---|---|---|
| 1 | `BG` :22 | `rgb(0x121215)` | revalue | `rgb(0x0A0A0A)` | bg-primary | chrome base: panels, rail, ruler strips, egui `panel_fill`; kit nav, menu and input fills are bg-primary |
| 2 | `BG_PRIMARY` :23 | `rgb(0x18181B)` | revalue | `rgb(0x0A0A0A)` | bg-primary | tab bar and terminal context row; kit header navigation is bg-primary; stays a literal (G01 parses literals only) |
| 3 | `BG_SECONDARY` :24 | `rgb(0x1E1E22)` | revalue | `rgb(0x171717)` | bg-secondary | fields, cards, egui inactive/extreme fill; kit `_Nav item base` Current=True fill |
| 4 | `BG_TERTIARY` :25 | `rgb(0x27272A)` | revalue | `rgb(0x262626)` | bg-tertiary | chips, tracks, filled secondary buttons |
| 5 | `BG_HOVER` :26 | `rgb(0x232329)` | revalue | `rgb(0x171717)` | bg-primary_hover | hover on a bg-primary surface; kit menu item, tab and nav hover |
| 6 | `BG_PRESSED` :27 | `rgb(0x2E2E36)` | revalue | `rgb(0x404040)` | bg-quaternary | press state (egui `active`); kit has no pressed role; only text-primary/text-secondary may sit on it (AA, spec A rule 6) |
| 7 | `BG_MENU` :28 | `rgb(0x18181B)` | revalue | `rgb(0x0A0A0A)` | bg-primary | kit `Dropdown menu` > Menu fill |
| 8 | `BG_SELECTED` :31 | `BG_PRESSED` | revalue | `rgb(0x262626)` | bg-secondary_hover | selected tool, tab and row; was an alias of BG_PRESSED, now its own literal; owner question OQ-2 (kit-literal alternative bg-secondary #171717) |
| 9 | `BG_SELECTED_HOVER` :32 | `rgb(0x383840)` | revalue | `rgb(0x262626)` | bg-secondary_hover | kit keeps the current fill under the pointer (`_Tab button base` Current=True State=Hover = Current=True State=Default) |
| 10 | `BG_SELECTED_SECONDARY` :33 | `rgb(0x24242A)` | revalue | `rgb(0x171717)` | bg-secondary | rows inside a selected parent (Layers); kit current-item fill |
| 11 | `BG_TEXT_SELECTION` :35 | `rgb(0x2B1D16)` | revalue | `rgb(0x431407)` | utility-orange-50 | selected text in a field; kit has no selection role, the brand-tinted dark utility step keeps text-primary at 14.98:1 |
| 12 | `BG_BRAND` :36 | `rgb(0xE36323)` | keep | unchanged | bg-brand-solid | R4: Studio orange replaces the kit's purple brand-solid #7F56D9 |
| 13 | `BG_BRAND_HOVER` :37 | `rgb(0xF06D2B)` | revalue | `rgb(0xF97316)` | bg-brand-solid_hover | kit dark: brand-solid (600) hovers to brand-500, one step lighter; with R4 orange that is Orange-500 |
| 14 | `BG_BRAND_PRESSED` :38 | `rgb(0xC85217)` | revalue | `rgb(0xEA580C)` | Orange-600 (primitive) | kit has no pressed role; Orange-600 keeps the dark label at 5.56:1 (Orange-700 would be 3.82:1) |
| 15 | `BG_TOOLBAR` :39 | `rgb(0x18181B)` | revalue | `rgb(0x0A0A0A)` | bg-primary | floating toolbars; kit dark draws no shadow, the bar is bg-primary with a border-secondary_alt hairline |
| 16 | `BG_TOOLBAR_HOVER` :40 | `rgb(0x27272A)` | revalue | `rgb(0x171717)` | bg-primary_hover | hover on the bg-primary toolbar |
| 17 | `BG_TOOLBAR_SELECTED` :41 | `BG_SELECTED` | alias | unchanged (alias) | = BG_SELECTED | alias kept |
| 18 | `BG_TOOLBAR_MODE_SWITCHER` :42 | `rgb(0x27272A)` | revalue | `rgb(0x171717)` | bg-secondary | segmented track; the selected tile is BG_SELECTED |
| 19 | `BG_SELECTED_AREA` :43 | `rgba(0x3582FF, 0x33)` | keep | unchanged | canvas overlay | marquee fill; canvas keeps Figma-parity blue (controller default) |
| 20 | `BORDER` :44 | `rgb(0x27272A)` | revalue | `rgb(0x262626)` | border-secondary | hairlines between regions and sections; kit dividers and card borders |
| 21 | `BORDER_SUBTLE` :46 | `rgb(0x222226)` | revalue | `rgb(0x262626)` | border-tertiary | subtlest divider; equals border-secondary in dark mode |
| 22 | `BORDER_MENU` :47 | `rgb(0x2E2E34)` | revalue | `rgb(0x262626)` | border-secondary_alt | kit `Dropdown menu` > Menu stroke |
| 23 | `BORDER_SELECTED` :48 | `rgb(0xE36323)` | revalue | `rgb(0xFB923C)` | border-brand | focused field and selected-card outline; kit border-brand is Brand-400, with R4 orange that is Orange-400 |
| 24 | `BORDER_TRANSLUCENT` :49 | `rgba(0xFFFFFF, 0x16)` | revalue | `rgba(0xFFFFFF, 0x1A)` | alpha-black-10 | swatch and thumbnail outlines over any fill; in dark mode alpha-black-10 resolves to white at 10% |
| 25 | `TEXT` :50 | `rgb(0xF4F4F5)` | revalue | `rgb(0xFAFAFA)` | text-primary | primary text |
| 26 | `TEXT_SECONDARY` :51 | `rgba(0xF4F4F5, 0xB2)` | revalue | `rgb(0xD4D4D4)` | text-secondary | was white at 70% alpha; now the opaque kit step |
| 27 | `TEXT_TERTIARY` :52 | `rgba(0xF4F4F5, 0x66)` | revalue | `rgb(0xA3A3A3)` | text-tertiary | was white at 40% alpha (about 3.9:1 on BG); the kit step passes AA on every surface up to bg-tertiary |
| 28 | `TEXT_DISABLED` :53 | `rgba(0xF4F4F5, 0x55)` | revalue | `rgba(0xD4D4D4, 0x80)` | text-secondary at opacity 0.5 | v8 has no disabled token; kit `Buttons/Button` State=Disabled sets `opacity: 0.5`; disabled text is exempt from WCAG 1.4.3 |
| 29 | `TEXT_BRAND` :54 | `rgb(0xFF9E66)` | revalue | `rgb(0xF97316)` | fg-brand-secondary (Studio orange) | accent text (New, Fix, undo-step marks); owner question OQ-3 (kit-literal alternative text-brand-secondary #D4D4D4, which is neutral in dark mode) |
| 30 | `TEXT_COMPONENT` :55 | `rgb(0x22D3EE)` | keep | unchanged | Studio-only: component (Cyan-400 primitive) | Untitled has no component role; Layers component names keep cyan |
| 31 | `ICON` :56 | `rgb(0xF4F4F5)` | revalue | `rgb(0xFFFFFF)` | fg-primary | highest-emphasis icon |
| 32 | `ICON_SECONDARY` :57 | `rgba(0xF4F4F5, 0xB2)` | revalue | `rgb(0xD4D4D4)` | fg-secondary | was white at 70% |
| 33 | `ICON_TERTIARY` :58 | `rgba(0xF4F4F5, 0x66)` | revalue | `rgb(0xA3A3A3)` | fg-tertiary | was white at 40% |
| 34 | `ICON_SELECTED` :59 | `ICON` | alias | unchanged (alias) | = ICON | alias kept |
| 35 | `DANGER` :60 | `rgb(0xFE5938)` | revalue | `rgb(0xEF4444)` | fg-error-primary | error icons, dots and strokes; text uses the new TEXT_ERROR (fg-error-primary is 4.02:1 on bg-tertiary) |
| 36 | `INFO` :61 | `rgb(0x0D99FF)` | keep | unchanged | Studio-only: info | Untitled v8 dark has no info role; agent Working state and selection blue stay |
| 37 | `SUCCESS` :62 | `rgb(0x14AE5C)` | revalue | `rgb(0x22C55E)` | fg-success-primary | chrome status (controller default); terminal.rs test pin moves with it |
| 38 | `WARNING` :63 | `rgb(0xF59E0B)` | revalue | `rgb(0xEAB308)` | fg-warning-primary | chrome status; terminal.rs test pin moves with it |
| 39 | `BG_ELEVATED` :65 | `rgb(0x18181B)` | delete | deleted | - | 0 references; kept only for parity with the retired ui-spec.json |
| 40 | `BG_MENU_HOVER` :67 | `rgb(0x27272A)` | revalue | `rgb(0x171717)` | bg-primary_hover | kit `_Dropdown menu list item` State=Hover/active fill |
| 41 | `BG_DISABLED` :69 | `rgb(0x71717A)` | delete | deleted | - | 0 references; ui-spec.json parity only |
| 42 | `BG_TRANSPARENT_HOVER` :71 | `rgba(0xFFFFFF, 0x0D)` | revalue | `rgba(0xFFFFFF, 0x1A)` | alpha-black-10 | hover over a surface of unknown fill (Find chips); kit alpha steps are 10% apart |
| 43 | `BORDER_STRONG` :73 | `rgb(0x3F3F46)` | revalue | `rgb(0x404040)` | border-primary | active tab outline, picker hover; kit inputs and secondary buttons |
| 44 | `COMPONENT` :75 | `rgb(0x06B6D4)` | keep | unchanged | Studio-only: component (Cyan-500 primitive) | Untitled has no component role |
| 45 | `TEXT_ON_LIGHT_CANVAS` :77 | `rgba(0x000000, 0xE5)` | revalue | `rgb(0x0A0A0A)` | Neutral-950 (dark label, R4) | dark label on a light or accent fill |
| 46 | `TEXT_ON_LIGHT_CANVAS_SECONDARY` :79 | `rgba(0x000000, 0x4D)` | delete | deleted | - | 0 references |
| 47 | `TEXT_ON_DARK_CANVAS` :81 | `rgba(0xFFFFFF, 0xE5)` | delete | deleted | - | 0 references |
| 48 | `SHADOW_COLOR` :82 | `rgba(0x000000, 0x66)` | revalue | `rgba(0x000000, 0x00)` | shadow-* (alpha 0 in dark) | every kit dark-mode shadow resolves to alpha 0; surfaces drop the draw call in their own spec |
| 49 | `SHADOW_COLOR_STRONG` :83 | `rgba(0x000000, 0x8C)` | revalue | `rgba(0x000000, 0x00)` | shadow-* (alpha 0 in dark) | same |
| 50 | `ELEVATION_400_SHADOW` :85 | `rgba(0x000000, 0x66)` | revalue | `rgba(0x000000, 0x00)` | shadow-* (alpha 0 in dark) | same |
| 51 | `ELEVATION_400_RING` :86 | `rgba(0xFFFFFF, 0x16)` | revalue | `rgb(0x262626)` | border-secondary_alt | the popover outline becomes the kit menu hairline |
| 52 | `ELEVATION_400_TOP` :87 | `rgba(0xFFFFFF, 0x0C)` | revalue | `rgba(0xFFFFFF, 0x00)` | none (alpha 0) | no top highlight in the kit |
| 53 | `INNER_HIGHLIGHT` :88 | `rgba(0xFFFFFF, 0x16)` | revalue | `rgb(0x262626)` | border-secondary_alt | the toolbar inner ring becomes the kit hairline |
| 54 | `LOGO_GRADIENT_A` :90 | `rgb(0xE36323)` | delete | deleted | - | 0 references |
| 55 | `CHECKER_A` :91 | `rgb(0xFFFFFF)` | keep | unchanged | content | transparency checker under document colours |
| 56 | `CHECKER_B` :92 | `rgb(0xCCCCCC)` | keep | unchanged | content | same |
| 57 | `CANVAS_SELECTION` :95 | `rgb(0x3582FF)` | keep | unchanged | canvas overlay | controller default: canvas keeps Figma parity |
| 58 | `CANVAS_COMPONENT` :96 | `rgb(0x06B6D4)` | keep | unchanged | canvas overlay | same |
| 59 | `CANVAS_HANDLE_FILL` :97 | `rgb(0xFFFFFF)` | keep | unchanged | canvas overlay | same |
| 60 | `CANVAS_SIZE_BADGE_TEXT` :98 | `rgb(0xFFFFFF)` | keep | unchanged | canvas overlay | same |
| 61 | `CANVAS_LABEL` :99 | `rgba(0x000000, 0x80)` | keep | unchanged | canvas overlay | same |
| 62 | `CANVAS_LABEL_DARK` :100 | `rgba(0xFFFFFF, 0x99)` | keep | unchanged | canvas overlay | same |
| 63 | `CANVAS_MARQUEE_FILL` :101 | `BG_SELECTED_AREA` | keep | unchanged | canvas overlay (alias) | same |
| 64 | `GUIDE_COLOR` :102 | `rgb(0xFE5938)` | keep | unchanged | canvas overlay | guides #FE5938 (controller default) |
| 65 | `GUIDE_TEXT` :103 | `rgb(0xFFFFFF)` | keep | unchanged | canvas overlay | same |
| 66 | `PEN_POINT` :104 | `rgb(0xFFFFFF)` | keep | unchanged | canvas overlay | same |
| 67 | `DROP_INDICATOR` :105 | `rgb(0xE36323)` | keep | unchanged | bg-brand-solid (R4) | insertion line in Layers and on the canvas; brand value unchanged |
| 68 | `AGENT_1` :107 | `rgb(0xFFA629)` | keep | unchanged | Studio-only: agent presence | canvas presence colours (controller default) |
| 69 | `AGENT_2` :108 | `rgb(0x14AE5C)` | keep | unchanged | Studio-only: agent presence | same |
| 70 | `AGENT_3` :109 | `rgb(0xEE46BC)` | keep | unchanged | Studio-only: agent presence | same |
| 71 | `AGENT_4` :110 | `rgb(0x38BDF8)` | keep | unchanged | Studio-only: agent presence | same |
| 72 | `AGENT_COLORS` :111 | `[AGENT_1, AGENT_2, AGENT_3, AGENT_4]` | keep | unchanged | array of AGENT_1..4 | same |
| 73 | `AGENT_LABEL_TEXT` :112 | `rgb(0x09090B)` | revalue | `rgb(0x0A0A0A)` | Neutral-950 (dark label, R4) | one dark label value for every accent fill |
| 74 | `FIND_HIGHLIGHT` :313 | `rgba(0x0D99FF, 0x59)` | keep | unchanged | Studio-only: info family | no Untitled highlight role |
| 75 | `FIND_HIGHLIGHT_SELECTED` :314 | `rgba(0x7CC4F8, 0x66)` | keep | unchanged | Studio-only: info family | same |
| 76 | `BG_MENU_SELECTED` :424 | `BG_SELECTED` | alias | unchanged (alias) | = BG_SELECTED | alias kept |
| 77 | `MENU_SHORTCUT_HIGHLIGHT` :443 | `rgba(0xFFFFFF, 0xCC)` | revalue | `rgb(0xE5E5E5)` | text-secondary_hover | hovered menu text in the kit; spec J applies the kit shortcut anatomy (text-quaternary) |
| 78 | `COMMENT_PIN_RESOLVED` :470 | `rgb(0x757575)` | keep | unchanged | canvas overlay | comment pins sit on the canvas |
| 79 | `AVATAR_COLORS` :472 | `[rgb(0xF24822), rgb(0xFFA629), rgb(0x14AE5C), rgb(0x0C8CE9), rgb(0x06B6D4), rgb(0xEE46BC)]` | keep | unchanged | content (identity colours) | picked from the author's name |
| 80 | `RULER_TEXT` :528 | `rgba(0xFFFFFF, 0xB2)` | keep | unchanged | canvas overlay | the 9px ruler exception; the ruler strip fill is BG |
| 81 | `RULER_TICK` :529 | `rgba(0xFFFFFF, 0x66)` | keep | unchanged | canvas overlay | same |
| 82 | `RULER_SELECTION_FILL` :531 | `rgba(0x0D99FF, 0x4D)` | keep | unchanged | canvas overlay | same |
| 83 | `RULER_GUIDE` :533 | `rgb(0xF24822)` | keep | unchanged | canvas overlay | same |
| 84 | `PIXEL_GRID_LIGHT_LINES` :535 | `rgba(0xFFFFFF, 0x1A)` | keep | unchanged | canvas overlay | same |
| 85 | `PIXEL_GRID_DARK_LINES` :536 | `rgba(0x000000, 0x1A)` | keep | unchanged | canvas overlay | same |
| 86 | `TEXT_SELECTION` :543 | `Color32::from_rgba_premultiplied(4, 49, 81, 89)` | keep | unchanged | canvas overlay | selected text on the canvas |
| 87 | `FIND_CANVAS_MATCH` :545 | `TEXT_SELECTION` | keep | unchanged | canvas overlay (alias) | same |
| 88 | `FIND_CANVAS_MATCH_CURRENT` :547 | `BG_BRAND` | keep | unchanged | alias of BG_BRAND | brand unchanged |
| 89 | `SYNTAX_PLAIN` :555 | `TEXT` | alias | unchanged (alias) | = TEXT (text-primary) | follows TEXT |
| 90 | `SYNTAX_PROPERTY` :556 | `rgb(0x7CC4F8)` | revalue | `rgb(0x7DD3FC)` | utility-sky-700 | Dev code blocks: kit dark utility steps replace the Material palette; spec G may refine within utility roles |
| 91 | `SYNTAX_KEYWORD` :557 | `rgb(0xC792EA)` | revalue | `rgb(0xF472B6)` | utility-pink-600 | #C792EA is violet (hue 276 deg) and breaks the Purple Ban at HEAD; pink-400 is outside the banned band |
| 92 | `SYNTAX_STRING` :558 | `rgb(0x9CD67D)` | revalue | `rgb(0x4ADE80)` | utility-green-600 | kit dark utility step |
| 93 | `SYNTAX_NUMBER` :559 | `rgb(0xF2A365)` | revalue | `rgb(0xFB923C)` | utility-orange-600 | kit dark utility step |
| 94 | `SYNTAX_TYPE` :560 | `rgb(0xFFCB6B)` | revalue | `rgb(0xFACC15)` | utility-yellow-600 | kit dark utility step |
| 95 | `SYNTAX_FUNCTION` :561 | `rgb(0x82AAFF)` | revalue | `rgb(0x60A5FA)` | utility-blue-600 | kit dark utility step |
| 96 | `SYNTAX_COMMENT` :562 | `rgb(0x8A8A8A)` | revalue | `rgb(0xA3A3A3)` | text-quaternary | comments are quiet text; fg-quaternary #737373 would fail AA |
| 97 | `SYNTAX_PUNCT` :563 | `rgb(0xB3B3B3)` | revalue | `rgb(0xD4D4D4)` | text-secondary | punctuation one step below plain text |
| 98 | `DEV_READY` :565 | `rgb(0x14AE5C)` | keep | unchanged | Dev overlay | controller default: Dev ready emerald #14AE5C |
| 99 | `DEV_COMPLETED` :566 | `rgb(0x06B6D4)` | keep | unchanged | Dev overlay | same |
| 100 | `DEV_CHANGED` :567 | `rgb(0xFFA629)` | keep | unchanged | Dev overlay | same |
| 101 | `STATUS_INFO_TEXT` :569 | `rgb(0x0D99FF)` | keep | unchanged | Studio-only: info family | agent Working badge, also drawn on the canvas |
| 102 | `STATUS_INFO_BG` :570 | `rgba(0x0D99FF, 0x24)` | keep | unchanged | Studio-only: info family | same |
| 103 | `STATUS_INFO_BORDER` :571 | `rgba(0x0D99FF, 0x59)` | keep | unchanged | Studio-only: info family | same |
| 104 | `BOX_MARGIN` :573 | `rgb(0xF9CB9C)` | keep | unchanged | Dev overlay | box-model diagram keeps Inspect parity |
| 105 | `BOX_PADDING` :574 | `rgb(0xB6D7A8)` | keep | unchanged | Dev overlay | same |
| 106 | `BOX_CONTENT` :575 | `rgb(0x9FC5E8)` | keep | unchanged | Dev overlay | same |
| 107 | `BOX_TEXT` :576 | `rgb(0x1E1E1E)` | keep | unchanged | Dev overlay | same |
| 108 | `ANNOTATION_BG` :578 | `rgb(0xFFE08A)` | keep | unchanged | canvas overlay | annotation callouts |
| 109 | `ANNOTATION_TEXT` :579 | `rgb(0x1E1E1E)` | keep | unchanged | canvas overlay | same |
| 110 | `ANNOTATION_LINE` :580 | `rgb(0xFFB800)` | keep | unchanged | canvas overlay | same |
| 111 | `MEASURE` :582 | `rgb(0xF24822)` | keep | unchanged | canvas overlay | measurement red |
| 112 | `PROTO_BLUE` :587 | `BG_BRAND` | keep | unchanged | alias of BG_BRAND (canvas noodles) | brand unchanged |
| 113 | `PRESENT_BG` :596 | `rgb(0x09090B)` | revalue | `rgb(0x0A0A0A)` | bg-primary | presentation stage around the frame |
| 114 | `HOME_THUMB_BG` :702 | `rgb(0x3A3A3A)` | revalue | `rgb(0x262626)` | bg-tertiary | thumbnail placeholder, one step above the bg-secondary card |
| 115 | `VAR_SCROLL_THUMB` :724 | `rgb(0x707070)` | revalue | `rgb(0x737373)` | fg-quaternary | non-text graphic, 3:1 needed: 3.78:1 on bg-secondary |
| 116 | `BOARD_DOT` :746 | `rgba(0x000000, 0x2E)` | keep | unchanged | board content | whiteboard canvas dots |

| # | New constant (new name) | Value | Untitled token | Reason |
|---|---|---|---|---|
| N1 | `BG_SECONDARY_HOVER` | `rgb(0x262626)` | bg-secondary_hover | hover on a bg-secondary surface (fields, cards); egui `widgets.hovered` uses it because BG_HOVER now equals BG_SECONDARY |
| N2 | `TEXT_SECONDARY_HOVER` | `rgb(0xE5E5E5)` | text-secondary_hover | kit hover text for menu items, nav items and secondary buttons |
| N3 | `TEXT_TERTIARY_HOVER` | `rgb(0xD4D4D4)` | text-tertiary_hover | kit hover text for tertiary and link-gray buttons |
| N4 | `TEXT_QUATERNARY` | `rgb(0xA3A3A3)` | text-quaternary | kit non-current tab text and menu shortcuts |
| N5 | `TEXT_PLACEHOLDER` | `rgb(0xA3A3A3)` | text-placeholder (AA-adjusted, OQ-1) | kit #737373 is 3.78:1 on bg-secondary; recommended value is the AA-passing Neutral-400 |
| N6 | `ICON_QUATERNARY` | `rgb(0x737373)` | fg-quaternary | kit input leading icons and nav icons; non-text 3:1 holds up to bg-tertiary (3.19:1) |
| N7 | `TEXT_ERROR` | `rgb(0xF87171)` | text-error-primary | error text; 7.16:1 on bg-primary, passes on bg-tertiary |
| N8 | `TEXT_WARNING` | `rgb(0xFACC15)` | text-warning-primary | warning text |
| N9 | `TEXT_SUCCESS` | `rgb(0x4ADE80)` | text-success-primary | success text |
| N10 | `TEXT_ON_BRAND` | `rgb(0x0A0A0A)` | Neutral-950 (dark label, R4) | label on BG_BRAND, BG_BRAND_HOVER, BG_BRAND_PRESSED: 5.76 / 7.06 / 5.56:1 |
| N11 | `FOCUS_RING` | `rgb(0xF97316)` | focus-ring (R4: Orange-500) | focus ring for every focusable control (spec C draws it) |
| N12 | `BG_ERROR_SOLID` | `rgb(0xDC2626)` | bg-error-solid | destructive button fill; label TEXT_WHITE at 4.83:1 |
| N13 | `TEXT_WHITE` | `rgb(0xFFFFFF)` | text-white | label on bg-error-solid (kit destructive button) |
| N14 | `ICON_BRAND_PRIMARY` | `rgb(0xF97316)` | fg-brand-primary (R4 remap: kit `#9E77ED` Brand-500 → Orange-500) | spec C: menu check, toast dot, slider and progress fill, spinner arc; non-text 7.06:1 on bg-primary |
| N15 | `ICON_SECONDARY_HOVER` | `rgb(0xE5E5E5)` | fg-secondary_hover | spec C: selected icon |
| N16 | `ICON_QUATERNARY_HOVER` | `rgb(0xA3A3A3)` | fg-quaternary_hover | spec C: icon hover and pressed |
| N17 | `ICON_WHITE` | `rgb(0xFFFFFF)` | fg-white | spec C: toggle knob |
| N18 | `BG_PRIMARY_ALT` | `rgb(0x171717)` | bg-primary_alt (dark: alias of bg-secondary) | spec C: selected segment, toast fill |
| N19 | `BG_PRIMARY_SOLID` | `rgb(0x262626)` | bg-primary-solid (dark: alias of bg-tertiary) | spec C: tooltip fill; `TEXT` on it 14.50:1 |
| N20 | `BG_ERROR_SOLID_HOVER` | `rgb(0xEF4444)` | bg-error-solid_hover (Red-500) | spec C: destructive hover. Kit FACT, but `TEXT_WHITE` on it is 3.76:1 (below AA for 14 px text); spec C owns the call |
| N21 | `UTILITY_NEUTRAL_50` | `rgb(0x171717)` | utility-neutral-50 (dark: Neutral-900) | spec C: count badge fill |
| N22 | `UTILITY_NEUTRAL_200` | `rgb(0x404040)` | utility-neutral-200 (dark: Neutral-700) | spec C: count badge border |
| N23 | `UTILITY_NEUTRAL_700` | `rgb(0xD4D4D4)` | utility-neutral-700 (dark: Neutral-300) | spec C: count badge text, 12.09:1 on `UTILITY_NEUTRAL_50` |

Rules that follow from the AA run in spec A section 3:

- On `BG_PRESSED` (bg-quaternary `#404040`) only `TEXT` and `TEXT_SECONDARY` may sit (tertiary 4.11:1, accent 3.70:1, placeholder 2.19:1 fail).
- Error text uses `TEXT_ERROR` (`#F87171`); `DANGER` (`#EF4444`) is for icons, dots and strokes (4.02:1 on bg-tertiary).
- `ICON_QUATERNARY` and `VAR_SCROLL_THUMB` (`#737373`) are non-text graphics (3:1): never on `BG_PRESSED` (2.19:1).
- `BG_HOVER` equals `BG_SECONDARY` (`#171717`) in the kit; hover on a bg-secondary surface uses `BG_SECONDARY_HOVER` (new name).

## (b) `design/design.tokens.json` colour roles → new values

The store keeps its 48 role keys (no add or delete path in the sealed store; see spec A section 2). Each re-value
is one `ui ds change-token colors.<key> --value <hex> --reason "<Untitled token>: …" --dir .`, which keeps
`$extensions.design-os.role`, bumps the generation and appends a changelog entry.

| Token path | HEAD value | New value | Untitled token / reason |
|---|---|---|---|
| `colors.accent` | `#2b1d16` | `#431407` | utility-orange-50 (Orange-950): brand-tinted text-selection fill |
| `colors.accent-foreground` | `#ff9e66` | `#f97316` | fg-brand-secondary with Studio orange (Orange-500): accent text |
| `colors.agent-1` | `#ffa629` | unchanged | Studio-only role kept (see (e)) |
| `colors.agent-2` | `#14ae5c` | unchanged | Studio-only role kept (see (e)) |
| `colors.agent-3` | `#ee46bc` | unchanged | Studio-only role kept (see (e)) |
| `colors.agent-4` | `#38bdf8` | unchanged | Studio-only role kept (see (e)) |
| `colors.bg` | `#18181b` | `#0a0a0a` | bg-primary |
| `colors.bg-active` | `#2e2e36` | `#262626` | bg-secondary_hover: selected row (owner question OQ-2) |
| `colors.bg-foreground` | `#f5f5f7` | `#fafafa` | text-primary |
| `colors.bg-hover` | `#232329` | `#171717` | bg-primary_hover |
| `colors.border` | `#27272a` | `#262626` | border-secondary |
| `colors.border-focus` | `#e36323` | `#fb923c` | border-brand with Studio orange (Orange-400) |
| `colors.border-strong` | `#3f3f46` | `#404040` | border-primary |
| `colors.border-subtle` | `#222226` | `#262626` | border-tertiary |
| `colors.canvas` | `#121215` | `#0a0a0a` | bg-primary (chrome around the canvas) |
| `colors.canvas-foreground` | `#f5f5f7` | `#fafafa` | text-primary |
| `colors.card` | `#1e1e22` | `#171717` | bg-secondary |
| `colors.card-foreground` | `#f5f5f7` | `#fafafa` | text-primary |
| `colors.component` | `#06b6d4` | unchanged | Studio-only role kept (see (e)) |
| `colors.component-foreground` | `#09090b` | `#0a0a0a` | Neutral-950 dark label (R4) |
| `colors.destructive` | `#f24822` | `#dc2626` | bg-error-solid |
| `colors.destructive-foreground` | `#09090b` | `#ffffff` | text-white (kit destructive label) |
| `colors.info` | `#0d99ff` | unchanged | Studio-only role kept (see (e)) |
| `colors.info-foreground` | `#09090b` | `#0a0a0a` | Neutral-950 dark label (R4) |
| `colors.input` | `#121215` | `#0a0a0a` | bg-primary (kit Input field fill) |
| `colors.muted` | `#222226` | `#171717` | bg-secondary |
| `colors.muted-foreground` | `#a8a8b2` | `#a3a3a3` | text-tertiary |
| `colors.neutral` | `#6e6e77` | `#737373` | fg-quaternary (Neutral-500) |
| `colors.neutral-foreground` | `#f5f5f7` | `#fafafa` | text-primary |
| `colors.popover` | `#18181b` | `#0a0a0a` | bg-primary (kit Dropdown menu fill) |
| `colors.popover-foreground` | `#f5f5f7` | `#fafafa` | text-primary |
| `colors.primary` | `#e36323` | unchanged | Studio-only role kept (see (e)) |
| `colors.primary-active` | `#ff9e66` | `#ea580c` | Orange-600: pressed brand (no Untitled pressed role) |
| `colors.primary-foreground` | `#09090b` | `#0a0a0a` | Neutral-950 dark label on Studio orange (R4) |
| `colors.primary-hover` | `#f07436` | `#f97316` | bg-brand-solid_hover with Studio orange (Orange-500) |
| `colors.ring` | `#e36323` | `#f97316` | focus-ring Orange-500 (R4) |
| `colors.secondary` | `#27272a` | `#262626` | bg-tertiary |
| `colors.secondary-active` | `#3f3f46` | `#404040` | bg-quaternary |
| `colors.secondary-foreground` | `#f5f5f7` | `#fafafa` | text-primary |
| `colors.secondary-hover` | `#2e2e34` | `#404040` | bg-quaternary |
| `colors.success` | `#14ae5c` | `#22c55e` | fg-success-primary |
| `colors.success-foreground` | `#09090b` | `#0a0a0a` | Neutral-950 dark label |
| `colors.surface` | `#1e1e22` | `#171717` | bg-secondary |
| `colors.surface-active` | `#2e2e36` | `#262626` | bg-secondary_hover (selected) |
| `colors.surface-foreground` | `#f5f5f7` | `#fafafa` | text-primary |
| `colors.surface-hover` | `#232329` | `#262626` | bg-secondary_hover |
| `colors.warning` | `#f59e0b` | `#eab308` | fg-warning-primary |
| `colors.warning-foreground` | `#09090b` | `#0a0a0a` | Neutral-950 dark label |

Counts: 48 colour tokens; 41 re-valued; 7 unchanged

The 41 re-values as a tab-separated list (key, value, reason) for the spec A loop:

```tsv
accent	#431407	utility-orange-50 (Orange-950): brand-tinted text-selection fill
accent-foreground	#f97316	fg-brand-secondary with Studio orange (Orange-500): accent text
bg	#0a0a0a	bg-primary
bg-active	#262626	bg-secondary_hover: selected row (owner question OQ-2)
bg-foreground	#fafafa	text-primary
bg-hover	#171717	bg-primary_hover
border	#262626	border-secondary
border-focus	#fb923c	border-brand with Studio orange (Orange-400)
border-strong	#404040	border-primary
border-subtle	#262626	border-tertiary
canvas	#0a0a0a	bg-primary (chrome around the canvas)
canvas-foreground	#fafafa	text-primary
card	#171717	bg-secondary
card-foreground	#fafafa	text-primary
component-foreground	#0a0a0a	Neutral-950 dark label (R4)
destructive	#dc2626	bg-error-solid
destructive-foreground	#ffffff	text-white (kit destructive label)
info-foreground	#0a0a0a	Neutral-950 dark label (R4)
input	#0a0a0a	bg-primary (kit Input field fill)
muted	#171717	bg-secondary
muted-foreground	#a3a3a3	text-tertiary
neutral	#737373	fg-quaternary (Neutral-500)
neutral-foreground	#fafafa	text-primary
popover	#0a0a0a	bg-primary (kit Dropdown menu fill)
popover-foreground	#fafafa	text-primary
primary-active	#ea580c	Orange-600: pressed brand (no Untitled pressed role)
primary-foreground	#0a0a0a	Neutral-950 dark label on Studio orange (R4)
primary-hover	#f97316	bg-brand-solid_hover with Studio orange (Orange-500)
ring	#f97316	focus-ring Orange-500 (R4)
secondary	#262626	bg-tertiary
secondary-active	#404040	bg-quaternary
secondary-foreground	#fafafa	text-primary
secondary-hover	#404040	bg-quaternary
success	#22c55e	fg-success-primary
success-foreground	#0a0a0a	Neutral-950 dark label
surface	#171717	bg-secondary
surface-active	#262626	bg-secondary_hover (selected)
surface-foreground	#fafafa	text-primary
surface-hover	#262626	bg-secondary_hover
warning	#eab308	fg-warning-primary
warning-foreground	#0a0a0a	Neutral-950 dark label
```

## (c) Scale constants → Untitled step (R5)

Type sizes and weights come from the kit text styles; radii and spacing from the `2_ Radius` and `3_ Spacing`
collections; control heights from `Buttons/Button` and `Input field` sizes; badge heights from `Badge`.

| Constant (`theme.rs` line) | HEAD value | New value | Untitled step | Reason |
|---|---|---|---|---|
| `FONT_SIZE_CAPTION` :117 | `10.0` | `12.0` | text-xs | R5 labels: captions, sublabels, rail labels, counts |
| `FONT_SIZE_BODY` :120 | `11.0` | `12.0` | text-xs | R5 rows: body, controls, menu rows, values in editor panels |
| `FONT_SIZE_LARGE` :121 | `13.0` | `14.0` | text-sm | R5 panel titles; also modal titles |
| `LINE_BODY` :124 | `16.0` | `18.0` | text-xs line height | kit Text xs 12/18 |
| `LINE_CAPTION` :125 | `14.0` | `18.0` | text-xs line height | kit Text xs 12/18 |
| `UI_FONT_WEIGHT` :131 | `450.0` | `400.0` | Regular | kit weights 400/500/600/700 |
| `UI_FONT_WEIGHT_STRONG` :132 | `550.0` | `600.0` | Semibold | kit Semibold; 550 is not a kit weight |
| `RADIUS_SEGMENT` :192 | `4` | `6` | radius-sm | RADIUS_CONTROL 8 - INSET_SEGMENT 2 |
| `RADIUS_CONTROL` :194 | `6` | `8` | radius-md | R5 radius md 8 |
| `RADIUS_FRAME` :196 | `14` | `16` | radius-2xl | RADIUS_CONTROL 8 + INSET_FRAME 8 (concentric law); owner question OQ-5 |
| `PAGE_ROW_HEIGHT` :290 | `CONTROL_DENSE` | `CONTROL_XS` | button xs / row 32 | R5 row 32 (was CONTROL_DENSE 24) |
| `MENU_ITEM_HEIGHT` :419 | `CONTROL_DENSE` | `CONTROL_XS` | row 32 | R5 row 32 (was CONTROL_DENSE 24) |
| `RIGHT_TABS_H` :332 | `CONTROL_DENSE` | `CONTROL_XS` | row 32 | R5 row 32 (was CONTROL_DENSE 24); RIGHT_TABS_Y 48 + 32 = 80 stays above RIGHT_CONTENT_TOP 81 |
| `FIELD_HEIGHT` :346 | `CONTROL_DENSE` | `CONTROL_XS` | control xs 32 | R5 small end; kit has no input below Input sm 36; owner question OQ-4 |
| `FIND_FIELD_H` :306 | `CONTROL_STANDARD` | `CONTROL_XS` | control xs 32 | was CONTROL_STANDARD 28 |
| `LABEL_BOX_H` :365 | `LINE_CAPTION` | `LINE_CAPTION` | text-xs line 18 | follows LINE_CAPTION 14 -> 18 |
| `SUBLABEL_HEIGHT` :362 | `16.0` | `LINE_CAPTION` | text-xs line 18 | was literal 16; a label box is one caption line |
| `FIND_GROUP_ROW_H` :311 | `28.0` | `CONTROL_XS` | row 32 | was literal 28; R5 row 32 |
| `STYLE_FOLDER_ROW_H` :394 | `24.0` | `CONTROL_XS` | row 32 | was literal 24; R5 row 32 |
| `VAR_PILL_H` :728 | `CONTROL_DENSE` | `BADGE_MD` | Badge md 24 | was CONTROL_DENSE 24; a pill is a badge, not a control |
| `CODE_LINE_HEIGHT` :583 | `LINE_BODY` | `LINE_BODY` | text-xs line 18 | follows LINE_BODY 16 -> 18 |
| `CONTROL_TOOL` :257 | `32.0` | renamed to `CONTROL_XS` (value kept) | Buttons/Button Size=xs height | mechanical rename, see (d) |
| `CONTROL_ROW` :258 | `40.0` | renamed to `CONTROL_MD` (value kept) | Buttons/Button Size=md, Input field Size=md | mechanical rename, see (d) |
| `CONTROL_DENSE` :255 | `24.0` | unchanged (legacy, see (d)) | none (24 = Badge md / TARGET_MIN; 28 = Badge lg) | call sites mix buttons, rows and chips; surface specs re-point each site |
| `CONTROL_STANDARD` :256 | `28.0` | unchanged (legacy, see (d)) | none (24 = Badge md / TARGET_MIN; 28 = Badge lg) | call sites mix buttons, rows and chips; surface specs re-point each site |

| New constant (new name) | Value | Untitled step | Reason |
|---|---|---|---|
| `CONTROL_XS` | `32.0` | Buttons/Button Size=xs height | R5 editor buttons and rows |
| `CONTROL_SM` | `36.0` | Buttons/Button Size=sm, Input field Size=sm | R5 Home and dialogs |
| `CONTROL_MD` | `40.0` | Buttons/Button Size=md, Input field Size=md | R5 Home and dialogs; panel title rows |
| `BADGE_SM` | `22.0` | Badge Size=sm | kit badge heights |
| `BADGE_MD` | `24.0` | Badge Size=md | kit badge heights |
| `BADGE_LG` | `28.0` | Badge Size=lg | kit badge heights |
| `FONT_SIZE_TEXT_XS` | `12.0` | Text xs 12/18 | Styles panel |
| `FONT_SIZE_TEXT_SM` | `14.0` | Text sm 14/20 | Styles panel |
| `FONT_SIZE_TEXT_MD` | `16.0` | Text md 16/24 | Styles panel |
| `FONT_SIZE_TEXT_LG` | `18.0` | Text lg 18/28 | Styles panel |
| `FONT_SIZE_TEXT_XL` | `20.0` | Text xl 20/30 | Styles panel |
| `LINE_TEXT_XS` | `18.0` | Text xs line | kit text style line height |
| `LINE_TEXT_SM` | `20.0` | Text sm line | kit text style line height |
| `LINE_TEXT_MD` | `24.0` | Text md line | kit text style line height |
| `LINE_TEXT_LG` | `28.0` | Text lg line | kit text style line height |
| `LINE_TEXT_XL` | `30.0` | Text xl line | kit text style line height |

Unchanged scale constants that already sit on the Untitled scale (listed in the appendix with their values):
`FONT_SIZE_TITLE` 20 (text-xl), `FONT_SIZE_CANVAS` 9 (canvas exception, not a kit size), `UI_FONT_WEIGHT_MEDIUM` 500
(Medium), `RADIUS_MARK` 2 (radius-xxs), `INSET_SEGMENT` 2 (spacing-xxs), the spacing ladder `SPACE_FACT` 4
(spacing-xs), `SPACE_ROW` 4, `SPACE_INLINE` 8 (spacing-md), `SPACE_GROUP` 12 (spacing-lg), `SPACE_BLOCK` 16
(spacing-xl), `SPACE_SECTION` 24 (spacing-3xl), `SPACE_REGION` 48 (spacing-6xl), the insets `INSET_CONTROL` 8,
`INSET_FRAME` 8, `INSET_BUTTON` 12, `INSET_PANEL` 16, `INSET_POPOVER` 16, `INSET_MODAL` 24, and `TARGET_MIN` 24
(WCAG 2.5.8 floor, not a kit size).

Token-store scale re-values (spec B), one `ui ds change-token <path> --value <v> --reason "<Untitled step>" --dir .` each:

```tsv
typography.font-size-caption	12px	Untitled text-xs (R5 labels)
typography.font-size-body	12px	Untitled text-xs (R5 rows)
typography.font-size-title	14px	Untitled text-sm (Home and dialogs, R5)
typography.font-size-heading	14px	Untitled text-sm (R5 panel titles)
radius.radius-xs	4px	Untitled radius-xs
radius.radius-sm	6px	Untitled radius-sm
radius.radius-md	8px	Untitled radius-md (R5)
radius.radius-lg	10px	Untitled radius-lg
radius.radius-xl	12px	Untitled radius-xl
radius.radius-pill	9999px	Untitled radius-full
```

## (d) Rust names, token paths and renames

### Renames (only where every call site migrates mechanically)

| Old name | New name | Value | Why mechanical | Command |
|---|---|---|---|---|
| `CONTROL_TOOL` (:257) | `CONTROL_XS` (new name) | 32 → 32 | value kept, so no call site changes size | see below |
| `CONTROL_ROW` (:258) | `CONTROL_MD` (new name) | 40 → 40 | value kept | see below |

```bash
# from the repo root, after theme.rs declares CONTROL_XS and CONTROL_MD and drops the two old lines
grep -rlwE 'CONTROL_(TOOL|ROW)' crates scripts | xargs perl -pi -e 's/\bCONTROL_TOOL\b/CONTROL_XS/g; s/\bCONTROL_ROW\b/CONTROL_MD/g'
grep -rnwE 'CONTROL_(TOOL|ROW)' crates scripts   # must print nothing
```

Measured on a scratch copy of `crates/designos-app/src`: 42 references outside `theme.rs` (3 `CONTROL_TOOL`, 39 `CONTROL_ROW`) renamed, 0 old names left. Shipped specs under `docs/specs/v0.6/` keep the old names: they are records of what shipped.

Not renamed: `CONTROL_DENSE` (24, 55 references outside `theme.rs` in 13 files) and `CONTROL_STANDARD` (28, 47
references in 15 files; `grep -rnow <NAME> crates/designos-app/src --exclude=theme.rs | wc -l`). Their call sites mix buttons (→ `CONTROL_XS` 32 under R5), rows, chips and pills (→ `BADGE_MD` 24 or
`BADGE_LG` 28) and icon hit boxes (→ `TARGET_MIN` 24), so a single rename would silently resize chips. They stay
as legacy names with their values; each surface spec (D–K) re-points the sites it owns, and spec B adds a
reference-count ratchet (spec B section 10) so the count only falls. When it reaches 0, the constant is deleted.

No colour constant is renamed; 5 dead colour constants are deleted and 23 are added (table (a)).

### `scripts/design_os_self_evolve.py` (G01, colour parity)

| Edit | Before (`ec47da26`) | After |
|---|---|---|
| `BANNED_PURPLES` (:109) | `#9747ff`, `#8755e9`, `#c4a1ff`, `#a855f7` | add `#a259ff`, `#c792ea`, `#7f56d9`, `#9e77ed`, `#b692f6`, `#6941c6` (kit brand purples and the HEAD syntax keyword) |
| `CORE_RUST_CONSTANTS` (:111-134) | 22 constants | add `BG_SELECTED`, `BG`, `TEXT`, `TEXT_TERTIARY`, `BG_BRAND_HOVER`, `BG_BRAND_PRESSED`, `TEXT_ON_BRAND`, `FOCUS_RING`, `BORDER_SELECTED`, `BG_TEXT_SELECTION`, `TEXT_BRAND`, `BG_MENU`, `BG_ERROR_SOLID` (35) |
| `RUST_TO_DESIGN_TOKEN_MAP` (:137-158) | `BG_PRESSED`→`bg-active`, `DEV_READY`→`success` (collides with `SUCCESS`→`success`) | `BG_PRESSED`→`secondary-active`, `BG_SELECTED`→`bg-active`, `BG`→`canvas`, `TEXT`→`bg-foreground`, `TEXT_TERTIARY`→`muted-foreground`, `BG_BRAND_HOVER`→`primary-hover`, `BG_BRAND_PRESSED`→`primary-active`, `TEXT_ON_BRAND`→`primary-foreground`, `FOCUS_RING`→`ring`, `BORDER_SELECTED`→`border-focus`, `BG_TEXT_SELECTION`→`accent`, `TEXT_BRAND`→`accent-foreground`, `BG_MENU`→`popover`, `BG_ERROR_SOLID`→`destructive`; `DEV_READY` row removed |
| `EXPECTED_STANDALONE_HEX` (:160-163) | — | add `DEV_READY: "#14ae5c"` (Dev overlay keeps Figma parity) |

Every mapped constant stays a plain `rgb(0x……)` literal (the parser at :223-225 reads literals only), so
`BG_SELECTED` becomes a literal instead of `= BG_PRESSED`.

### `scripts/non_colour_token_parity.py` (non-colour parity)

`TOKEN_TO_RUST` after spec B:

| Token path | Rust constants |
|---|---|
| `radius.radius-sm` (6) | `RADIUS_SEGMENT` |
| `radius.radius-md` (8) | `RADIUS_CONTROL` |
| `typography.font-size-small` (9) | `FONT_SIZE_CANVAS` |
| `typography.font-size-caption` (12) | `FONT_SIZE_CAPTION`, `FONT_SIZE_TEXT_XS` |
| `typography.font-size-body` (12) | `FONT_SIZE_BODY` |
| `typography.font-size-heading` (14) | `FONT_SIZE_LARGE`, `FONT_SIZE_TEXT_SM` |
| `typography.font-size-hero` (20) | `FONT_SIZE_TITLE`, `FONT_SIZE_TEXT_XL` |
| `spacing.rail-width`, `spacing.panel-width` | unchanged rows |
| `spacing.space-1/2/3/4/6/12` | unchanged rows |
| `spacing.space-7` (28) | `CONTROL_STANDARD` (unchanged row) |
| `spacing.space-8` (32) | `CONTROL_XS` (was `CONTROL_TOOL`) |
| `spacing.space-10` (40) | `CONTROL_MD` (was `CONTROL_ROW`) |

`radius.radius-xs` leaves the map: the token becomes 4 (kit radius-xs) while `RADIUS_MARK` stays 2 (kit
radius-xxs). `RUST_ONLY` gains `RADIUS_MARK` (radius-xxs 2, no token), `CONTROL_SM` (36, no token),
`FONT_SIZE_TEXT_MD` (16) and `FONT_SIZE_TEXT_LG` (18); `RADIUS_FRAME`'s reason becomes "16 px radius-2xl has no
token". `CONTROL_DENSE` keeps its row. `scripts/tests/test_non_colour_token_parity.py:81` swaps the name
`CONTROL_ROW` for `CONTROL_MD`.

Harness result on the scratch theme and scratch store with exactly these edits (imports the repo scripts, writes
nothing): `G01 ok = True`, "35/35 Rust theme.rs constants match design.tokens.json; 0 Purple Ban violations; 29
non-colour constants match". Without the edits it fails (map collision, `RADIUS_MARK` vs `radius-xs`, unguarded new
constants).

## (e) Unchanged because they are canvas, document, terminal or board

- **Canvas overlays (controller default, Figma parity):** `BG_SELECTED_AREA`, `CANVAS_*`, `GUIDE_*`, `PEN_POINT`,
  `RULER_TEXT/TICK/SELECTION_FILL/GUIDE`, `PIXEL_GRID_*`, `TEXT_SELECTION`, `FIND_CANVAS_MATCH*`, `ANNOTATION_*`,
  `MEASURE`, `COMMENT_PIN_RESOLVED`, `PROTO_BLUE`, `DROP_INDICATOR`, `CHECKER_A/B` (rows in (a)).
- **Dev overlay:** `DEV_READY/COMPLETED/CHANGED`, `BOX_*`.
- **Studio-only roles with no Untitled role:** `INFO`, `STATUS_INFO_*`, `FIND_HIGHLIGHT*`, `COMPONENT`,
  `TEXT_COMPONENT`, `AGENT_1..4`, `AGENT_COLORS`, `AVATAR_COLORS`.
- **Document defaults (`u32`, content):** `DEFAULT_PAGE_BG` :609 `0x5A5A5A`, `DEFAULT_STROKE` :610 `0x000000`,
  `DEFAULT_FRAME_FILL` :611 `0xFFFFFF`, `DEFAULT_SHAPE_FILL` :612 `0xD9D9D9`.
- **Board content (`u32`):** `BOARD_PALETTE` :741, `BOARD_INK` :743, `BOARD_BG` :745 `0xF4F4F4`; `BOARD_DOT` (Color32) keeps.
- **Terminal:** the ANSI palette lives in `terminal.rs`, not `theme.rs`, and is content. The chrome around the
  terminal uses the chrome constants above. `terminal.rs:10019-10021` pins `INFO`, `SUCCESS`, `WARNING`; the
  `SUCCESS` and `WARNING` pins move to `#22C55E` / `#EAB308` with spec A (at `origin/main` the pins sit at 9613-9615).
- **Strings:** `FAMILY_STRONG`, `UI_FONT_FAMILY` (`"Inter"`, the kit typeface), `FAMILY_MEDIUM`, `UI_MONO_FAMILY`
  (`"Roboto Mono"`), `APP_TITLE`.
- **Layout (R1):** every width, x position and panel geometry constant in the appendix.

## Appendix — numeric constants that keep their value

Value after resolving aliases at `ec47da26`; the name keeps its expression too (no edit).

| | | | |
|---|---|---|---|
| `LIGHT_BG_THRESHOLD` :113 = 0.5 | `FONT_SIZE_CANVAS` :119 = 9 | `FONT_SIZE_TITLE` :122 = 20 | `CHEVRON_SMALL` :126 = 10 |
| `FONT_SIZE_LABEL_CANVAS` :127 = 11 | `UI_FONT_WEIGHT_MEDIUM` :133 = 500 | `UI_MONO_WEIGHT` :137 = 400 | `UI_FONT_OPSZ` :139 = 14 |
| `RADIUS_MARK` :190 = 2 | `SPACE_FACT` :228 = 4 | `SPACE_ROW` :230 = 4 | `SPACE_INLINE` :232 = 8 |
| `SPACE_GROUP` :234 = 12 | `SPACE_BLOCK` :237 = 16 | `SPACE_SECTION` :240 = 24 | `SPACE_REGION` :243 = 48 |
| `INSET_SEGMENT` :246 = 2 | `INSET_CONTROL` :247 = 8 | `INSET_FRAME` :248 = 8 | `INSET_BUTTON` :249 = 12 |
| `INSET_PANEL` :250 = 16 | `INSET_POPOVER` :251 = 16 | `INSET_MODAL` :252 = 24 | `TARGET_MIN` :260 = 24 |
| `RAIL_WIDTH` :263 = 56 | `BORDER_WIDTH` :264 = 1 | `RAIL_MENU_BUTTON_X` :265 = 12 | `RAIL_MENU_BUTTON_Y` :266 = 8 |
| `RAIL_MENU_BUTTON_SIZE` :267 = 32 | `RAIL_ITEM_SIZE` :268 = 56 | `RAIL_FIRST_Y` :269 = 56 | `RAIL_GROUP_GAP` :270 = 16 |
| `RAIL_ICON_SIZE` :271 = 20 | `RAIL_SEPARATOR_Y` :273 = 48 | `RAIL_SEPARATOR_INSET` :274 = 20 | `RAIL_TILE` :275 = 32 |
| `RAIL_ICON_CENTER_Y` :276 = 20 | `RAIL_LABEL_CENTER_Y` :277 = 44 | `LEFT_PANEL_WIDTH` :279 = 240 | `LEFT_HEADER_HEIGHT` :280 = 65 |
| `LEFT_FILE_NAME_Y` :281 = 16 | `LEFT_SUBLABEL_Y` :282 = 40 | `LEFT_ACTION_Y` :283 = 12 | `LEFT_SIDEBAR_BTN_RIGHT_PAD` :284 = 12 |
| `PAGES_SEARCH_DX` :285 = 28 | `PANEL_TITLE_ROW_HEIGHT` :286 = 40 | `PANEL_ACTION_BUTTON` :287 = 24 | `PANEL_ACTION_RIGHT_PAD` :288 = 8 |
| `PANEL_PADDING_X` :289 = 16 | `PAGE_ROW_INSET` :291 = 8 | `PAGE_ROW_TEXT_PAD` :292 = 8 | `PAGES_MAX_VISIBLE_ROWS` :293 = 5 |
| `PAGE_MENU_WIDTH` :294 = 160 | `LAYER_HEADER_HEIGHT` :295 = 40 | `LAYER_ROW_HEIGHT` :296 = 32 | `LAYER_INDENT` :297 = 16 |
| `LAYER_CHEVRON_BOX` :298 = 24 | `LAYER_CHEVRON_PAD` :299 = 8 | `LAYER_ICON_SIZE` :300 = 16 | `LAYER_ICON_GAP` :301 = 0 |
| `LAYER_TOGGLE_SIZE` :302 = 24 | `LAYER_DROP_THICKNESS` :303 = 2 | `FIND_ROW_GAP` :307 = 8 | `FIND_TOGGLE_SIZE` :308 = 22 |
| `FIND_BUTTON_H` :309 = 24 | `FIND_STATUS_H` :310 = 32 | `FIND_EXCERPT_LEAD` :316 = 12 | `RIGHT_PANEL_WIDTH` :318 = 240 |
| `RIGHT_PRESENT_FROM_RIGHT` :320 = 56 | `PRESENT_ICON_W` :323 = 15 | `PRESENT_ICON_H` :325 = 16 | `PRESENT_ICON_STROKE` :327 = 1.2 |
| `RIGHT_BTN_Y` :328 = 8 | `RIGHT_BTN_SIZE` :329 = 32 | `RIGHT_PRESENT_CHEVRON_W` :330 = 16 | `RIGHT_TABS_Y` :331 = 48 |
| `RIGHT_TABS_X` :333 = 9 | `RIGHT_TAB_PAD_X` :334 = 8 | `RIGHT_ZOOM_FROM_RIGHT` :335 = 63 | `RIGHT_ZOOM_W` :336 = 55 |
| `RIGHT_CONTENT_TOP` :337 = 81 | `RIGHT_SELECTION_HEADER_Y` :338 = 93 | `RIGHT_NAME_FIELD_X` :339 = 9 | `RIGHT_NAME_FIELD_W` :340 = 136 |
| `RIGHT_NAME_ACTION_X` :341 = [153.0, 181.0, 209.0] | `SECTION_TITLE_HEIGHT` :342 = 40 | `SECTION_CONTENT_X` :343 = 17 | `SECTION_CONTENT_W` :344 = 184 |
| `SECTION_TRAILING_X` :345 = 209 | `FIELD_WIDTH_HALF` :347 = 88 | `FIELD_COL_X` :348 = [17.0, 113.0] | `FIELD_PREFIX_W` :349 = 24 |
| `FIELD_PREFIX_X` :351 = 8 | `FIELD_PREFIX_ICON` :353 = 16 | `FIELD_PREFIX_GLYPH` :356 = 12 | `FIELD_PLAIN_X` :358 = 8 |
| `FIELD_SUFFIX_GAP` :360 = 4 | `DISTRIBUTE_BTN_X` :361 = 209 | `SEGMENT_W` :366 = 29 | `SEGMENT_GROUP_GAP` :367 = 8 |
| `SWATCH_SIZE` :368 = 16 | `SWATCH_RADIUS` :369 = 2 | `SWATCH_ROW_X` :370 = 17 | `STYLE_ROW_H` :371 = 32 |
| `STYLE_SWATCH` :373 = 16 | `STYLE_SWATCH_CX` :374 = 16 | `STYLE_LABEL_X` :375 = 33 | `STYLES_LABEL_GAP` :377 = 8 |
| `STYLES_GROUP_GAP` :378 = 14 | `STYLES_BOTTOM_PAD` :379 = 13 | `STYLES_ROW_GAP` :381 = 9 | `PAGE_FIELD_DY` :383 = 4 |
| `PAGE_BOTTOM_PAD` :384 = 16 | `PAGE_SWATCH` :385 = 12 | `PAGE_SWATCH_CX` :386 = 12 | `PAGE_HEX_X` :387 = 24 |
| `HEX_FIELD_W` :389 = 72 | `PAGE_DIVIDER_X` :390 = 129 | `PAGE_OPACITY_PAD` :391 = 8 | `PAGE_PERCENT_W` :392 = 16 |
| `PAGE_PERCENT_RIGHT` :393 = 7 | `STYLE_INDENT` :395 = 12 | `OPACITY_FIELD_W` :396 = 48 | `TOOLBELT_HEIGHT` :398 = 48 |
| `TOOLBELT_BOTTOM_MARGIN` :399 = 12 | `TOOLBELT_PADDING` :400 = 8 | `TOOL_BUTTON` :401 = 32 | `TOOL_CHEVRON_W` :402 = 16 |
| `TOOL_CHEVRON_GAP` :403 = 0 | `TOOL_CHEVRON_ICON` :405 = 11 | `TOOL_CHEVRON_CENTER_DX` :407 = 11 | `TOOL_GROUP_GAP` :408 = 8 |
| `TOOL_ICON_SIZE` :409 = 20 | `MODE_SEGMENTS` :411 = 4 | `MODE_SWITCH_PAD` :412 = 2 | `MODE_SWITCH_W` :413 = 29.5 |
| `TOOLBAR_DIVIDER_GAP_BEFORE` :415 = 13 | `TOOLBAR_DIVIDER_GAP_AFTER` :416 = 8 | `MENU_WIDTH` :418 = 200 | `MENU_PADDING` :421 = 8 |
| `MENU_PILL_INSET` :423 = 8 | `MENU_CHECK_X` :426 = 12 | `MENU_CHECK_BOX` :427 = 16 | `MENU_ICON_X_CHECKED` :428 = 32 |
| `MENU_ICON_BOX` :429 = 24 | `MENU_LABEL_X_ICON` :430 = 60 | `MENU_LABEL_X_PLAIN` :431 = 16 | `MENU_LABEL_X_CHECK` :433 = 32 |
| `MENU_ICON_X_PLAIN` :435 = 12 | `MENU_ICON_LABEL_GAP` :436 = 4 | `MENU_SHORTCUT_PAD` :437 = 16 | `MENU_CHEVRON_BOX` :438 = 24 |
| `MENU_CHEVRON_PAD` :439 = 8 | `MENU_SEP_MARGIN` :440 = 8 | `MENU_LABEL_GAP` :442 = 16 | `MENU_ICON_SIZE` :444 = 16 |
| `MENU_CHEVRON_SIZE` :445 = 12 | `SUBMENU_OVERLAP` :447 = 0 | `TOOL_MENU_WIDTH` :449 = 232 | `TOOL_MENU_DX` :450 = 2.5 |
| `TOOL_MENU_OVERLAP` :451 = 4 | `MAIN_MENU_X` :453 = 12 | `MAIN_MENU_Y` :454 = 44 | `MAIN_MENU_WIDTH` :455 = 210 |
| `PALETTE_WIDTH` :457 = 320 | `PERMISSIONS_MODAL_WIDTH` :458 = 360 | `PALETTE_TOP` :460 = 64 | `TOAST_HEIGHT` :461 = 40 |
| `TOAST_BOTTOM` :462 = 72 | `TOAST_SECONDS` :463 = 4 | `COMMENT_PIN` :466 = 32 | `COMMENT_PIN_RING` :467 = 2 |
| `COMMENT_PIN_HIT_PAD` :468 = 2 | `COMMENT_AVATAR` :473 = 24 | `COMMENT_POP_W` :475 = 300 | `COMMENT_POP_GAP` :476 = 8 |
| `COMMENT_PAD` :477 = 12 | `COMMENT_HEADER_H` :478 = 36 | `COMMENT_BTN_H` :479 = 24 | `COMMENT_BOX_MIN_H` :480 = 56 |
| `COMMENT_BOX_MAX_H` :481 = 160 | `COMMENT_ICON_BTN` :482 = 20 | `COMMENT_PANEL_HEADER_H` :484 = 48 | `COMMENT_FILTER_H` :485 = 24 |
| `COMMENT_CARD_PAD` :486 = 12 | `HISTORY_ROW_H` :488 = 44 | `HISTORY_DAY_H` :489 = 28 | `HISTORY_BANNER_H` :490 = 40 |
| `CANVAS_OUTLINE_WIDTH` :493 = 1 | `CANVAS_HOVER_WIDTH` :494 = 1 | `CANVAS_HANDLE_SIZE` :495 = 7 | `CANVAS_HANDLE_BORDER` :496 = 1 |
| `CANVAS_ROTATE_ZONE` :497 = 12 | `CANVAS_HANDLE_HIT` :498 = 6 | `CANVAS_BADGE_OFFSET` :499 = 4 | `CANVAS_BADGE_PAD_X` :500 = 4 |
| `CANVAS_BADGE_HEIGHT` :501 = 16 | `CANVAS_BADGE_RADIUS` :502 = 2 | `CANVAS_LABEL_GAP` :503 = 4 | `CANVAS_LABEL_MAX_ZOOM_HIDE` :504 = 0.05 |
| `SNAP_THRESHOLD_PX` :505 = 5 | `GUIDE_WIDTH` :506 = 1 | `GUIDE_X_SIZE` :507 = 3 | `GUIDE_TEXT_PAD` :508 = 3 |
| `GUIDE_TEXT_HEIGHT` :509 = 14 | `PEN_POINT_SIZE` :510 = 4 | `PEN_LINE_WIDTH` :511 = 1.5 | `AL_INDICATOR_WIDTH` :513 = 2 |
| `VEDIT_POINT_SIZE` :515 = 6 | `VEDIT_HANDLE_RADIUS` :516 = 3 | `VEDIT_HIT` :517 = 6 | `VEDIT_SEGMENT_HIT` :518 = 5 |
| `DRAG_THRESHOLD_PX` :519 = 3 | `ZOOM_MIN` :520 = 0.02 | `ZOOM_MAX` :521 = 256 | `FIT_PADDING` :522 = 64 |
| `TEXT_CARET_WIDTH` :523 = 1 | `RULER_SIZE` :525 = 20 | `RULER_FONT_SIZE` :526 = 9 | `PIXEL_GRID_MIN_ZOOM` :538 = 8 |
| `FIND_CANVAS_MATCH_MAX` :549 = 600 | `NUDGE` :550 = 1 | `NUDGE_BIG` :551 = 10 | `PROTO_NOODLE_WIDTH` :588 = 2 |
| `PROTO_HOTSPOT_RADIUS` :589 = 6 | `PROTO_HOTSPOT_RING` :590 = 2 | `PROTO_HOTSPOT_HIT` :591 = 10 | `PROTO_FLAG_HEIGHT` :592 = 20 |
| `PROTO_FLAG_GAP` :593 = 4 | `PROTO_ROW_HEIGHT` :594 = 44 | `PROTO_POPOVER_WIDTH` :595 = 240 | `PRESENT_BAR_HEIGHT` :597 = 48 |
| `PRESENT_PADDING` :598 = 32 | `PRESENT_BEZEL` :600 = 10 | `PRESENT_MAX_SCALE` :601 = 4 | `DEFAULT_WINDOW` :604 = (1600.0, 1088.0) |
| `MIN_WINDOW` :605 = (900.0, 600.0) | `DEFAULT_SHAPE_SIZE` :607 = 100 | `DEFAULT_TEXT_SIZE` :608 = 16 | `AUTO_LAYOUT_SPACING` :613 = 10 |
| `AUTO_LAYOUT_PADDING` :614 = 10 | `PASTE_OFFSET` :615 = 10 | `HOME_SIDEBAR_W` :692 = 264 | `HOME_PAD` :693 = 32 |
| `HOME_HEADER_H` :694 = 64 | `HOME_CARD_W` :695 = 224 | `HOME_THUMB_H` :696 = 140 | `HOME_CARD_H` :698 = 198 |
| `HOME_CARD_GAP` :699 = 24 | `HOME_FOLDER_H` :700 = 48 | `HOME_LIST_ROW_H` :701 = 44 | `VAR_SIDEBAR_W` :706 = 240 |
| `VAR_BAR_H` :708 = 48 | `VAR_FLOAT_W` :710 = 642 | `VAR_FLOAT_H` :711 = 380 | `VAR_FLOAT_Y` :712 = 8 |
| `VAR_FLOAT_BAR_H` :713 = 40 | `VAR_HEAD_H` :714 = 40 | `VAR_ROW_H` :715 = 40 | `VAR_ROW_PITCH` :716 = 41 |
| `VAR_NAME_COL` :718 = 200 | `VAR_MODE_MIN` :719 = 140 | `VAR_MODE_MAX` :720 = 280 | `VAR_ADD_COL` :721 = 39 |
| `VAR_BOTTOM_H` :722 = 41 | `VAR_SCROLL_H` :723 = 11 | `VAR_SEARCH_W` :725 = 174 | `VAR_FILTER_W` :726 = 25 |
| `VAR_SIDE_ROW` :727 = 32 | `VAR_ICON_X` :729 = 16 | `VAR_NAME_X` :730 = 44 | `VAR_SWATCH` :731 = 16 |
| `VAR_HEX_X` :732 = 30 | `VAR_HEX_W` :733 = 146 | `VAR_OPACITY_W` :734 = 62 | `VAR_OPACITY_RIGHT` :735 = 74 |
| `VAR_FIELD_H` :736 = 24 | `VAR_POPOVER_W` :737 = 264 | `BOARD_DOT_SPACING` :748 = 24 | `BOARD_DOT_MIN_PX` :749 = 14 |
| `BOARD_MAGNET_RADIUS` :751 = 5 | `BOARD_MAGNET_GAP` :752 = 12 | `BOARD_MAGNET_HIT` :753 = 10 | `BOARD_SHAPE_W` :755 = 160 |
| `BOARD_SHAPE_H` :756 = 120 | `BOARD_STAMP_SIZE` :757 = 64 | `BOARD_STICKY_GAP` :758 = 24 | `BOARD_MARKER_WIDTH` :759 = 6 |
| `BOARD_HIGHLIGHTER_WIDTH` :760 = 24 | `BOARD_HIGHLIGHTER_ALPHA` :761 = 0.4 | `BOARD_BAR_H` :763 = 40 | `BOARD_BAR_PAD` :764 = 6 |
| `BOARD_BAR_GAP` :765 = 12 | `BOARD_BAR_BTN` :766 = 28 | `BOARD_SWATCH` :767 = 18 | `BOARD_BAR_SEP` :768 = 8 |

296 constants.
