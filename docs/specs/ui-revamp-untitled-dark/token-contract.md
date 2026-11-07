# Token contract — Untitled UI PRO v8.0 dark → DESIGN:OS Studio (specs A and B)

| Field | Value |
|---|---|
| Owners | Spec A (colours, token store, Rust colour names) · Spec B (type, spacing, radius, control sizes) |
| Code baseline | `8dfaee93` (`origin/main` with v0.7.0 merged, 2026-10-28; every line number below is `crates/designos-app/src/theme.rs` at that commit unless a path is given). `theme.rs` is byte-identical at `d0555c82` and `8dfaee93` (`git diff d0555c82 8dfaee93 -- crates/designos-app/src/theme.rs` is empty), so every theme line number, HEAD-value cell and the appendix held through the 215 app files that changed; every cite outside `theme.rs` was re-read at `8dfaee93`. Tools run on v0.7.0 (`plans/261025-1247-untitled-ui-dark-revamp/tools/designos-latest`); code is cited at `origin/main`, not at a release branch. |
| Drift seen | Since `ec47da26`: #1106 (`b598a499`) removed `RAIL_SEPARATOR_INSET`, `RAIL_ICON_CENTER_Y` and `RAIL_LABEL_CENTER_Y`, set `RAIL_WIDTH` to 48 and hand-edited `design/design.tokens.json` `spacing.rail-width`, which leaves the store unsealed: re-run at `8dfaee93` on 2026-10-28, `ui ds status` still exits 1 (`evidence/AB-fix-round-261028/1164-store-seal-at-8dfaee93.txt`); R12 fixes it in its own small PR and A and B start after it. `TOAST_REVEAL_SECONDS` is new. #1108 (`07b961c8`) added 8 constants after `PAGES_MAX_VISIBLE_ROWS` (appendix; its drag logic is `panels/left_sep.rs`). Between `d0555c82` and `8dfaee93` (v0.7.0): nothing in `theme.rs`, `scripts/design_os_self_evolve.py`, `scripts/non_colour_token_parity.py`, `scripts/check_spacing_relationships.py`, `scripts/spacing_baseline.json` or `design/` changed; `terminal.rs` (+2350 lines) moved the colour-pin test to `:10727-10745` and the provider marks to `:4781-4799`. No rule of this contract is broken by the drift. Re-anchor every line number against HEAD before editing. |
| Naming (R14) | Two kinds of name. **Untitled role names** (`bg-primary_hover`, `fg-quaternary`, the "Untitled token" columns below) are a review aid: a reviewer traces each value to the kit through them, and they appear only in `specs/`. **Shipped names** are Studio's own: Rust constants (the existing `theme.rs` names; every name this contract adds is marked "new name"), design-store keys, `ui ds change-token` reasons, registry text, `theme.rs` comments, soul and principles. None of them contains the word "Untitled" and none has to equal a kit role name; the only shipped mention is the credit line (spec A rule 22). The design store carries no kit file and no Untitled icon (R6, R14). Name audit: the constants that borrow a kit ramp word (`UTILITY_<HUE>_<STEP>`, N21–N32, N39–N46) are Studio constants for badge and tile colours; renaming the family is a find-and-replace across `theme.rs`, the tests and specs C–K that changes no value, and is spec T's call (open item in spec A). |
| Rulings | R1–R7, R8.1–R8.7 (owner answers to the seven board questions, 2026-10-25), R9 (read-only Figma check), R10.1–R10.7 (owner answers to board 3, 2026-10-25), R12 (store re-seal in its own PR), R14 (Studio's own names and values in the design store, credit Untitled UI) and the controller rulings C-T, C-F, C-K, C-R in `plans/261025-1247-untitled-ui-dark-revamp/plan.md` |

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
in this contract is verified. A second, independent check against the live kit in Figma (R9, read-only
`figma-agent exec-js`, file key `VEffelx01mDl5BJJanwxja`) is recorded in `evidence/figma-live-variable-check.md`: colour
variables 273/273 identical in Dark mode and 273/273 in Light mode, numeric variables (spacing, radius, widths,
containers) 76/76. No dark value in this contract is provisional. The DTCG overwrite did lose the local `6. Typography` (32 variables) to its one-variable
library copy; the REST export reads it: Font size text-xs 12, sm 14, md 16, lg 18, xl 20 (display 24 … 72); Line
height 18, 20, 24, 28, 30 (display 32 … 90); Font weight Regular, Medium, Semibold, Bold; family Inter. That makes
spec B's type scale a variable FACT, matching the Styles panel reading (and the live Figma check). That Studio's DTCG
export lacks 31 of the 32 `6. Typography` variables is spec P's to fix (#1140).

### Coverage (acceptance 4)

| Family | Grep (on `theme.rs` at `8dfaee93`) | In file | Covered here |
|---|---|---|---|
| All constants | `grep -cE '^pub const ' theme.rs` → 457 lines, of which 2 are `pub const fn` | 455 | 455 |
| Colour, `Color32` and `[Color32; N]` | `grep -cE '^pub const [A-Z0-9_]+: (Color32\|\[Color32; ?[0-9]+\]) =' theme.rs` | 116 | 116 in (a): 116 constants covered; alias 4, delete 5, keep 52, revalue 55; new 77 (N1–N13 spec A, N14–N23 spec C, N24–N37 contract gaps G-1..G-3, N38 spec C, N39–N46 R10.2, N47–N73 names sent by specs D–G: Dev hover, HUD pointer, playground dots, content-overlay constants, the 15 reader light names; N74–N77 spec C's error roles) |
| Colour, `u32` / `[u32; 8]` (document defaults, board) | `grep -nE '^pub const [A-Z0-9_]+: (u32\|\[u32; ?[0-9]+\]) =' theme.rs` | 7 | 7 in (e), unchanged |
| Numeric (`f32` 311, `f64` 2, `u8` 6, `usize` 4, `(f32, f32)` 2, `[f32; N]` 2) | `grep -cE '^pub const [A-Z0-9_]+: (f32\|f64\|u8\|usize\|\(f32, ?f32\)\|\[f32; ?[0-9]\]) =' theme.rs` | 327 | 327: 20 changed or re-pointed + 1 held by R10.1 (`RIGHT_TABS_H`) + 2 renamed + 2 legacy in (c), 302 unchanged in the appendix |
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
| 3 | `BG_SECONDARY` :24 | `rgb(0x1E1E22)` | revalue | `rgb(0x171717)` | bg-secondary | cards, egui `faint_bg_color` and inactive widget fill, hover on bg-primary; kit `_Nav item base` Current=True fill. Text fields are not bg-secondary: they take the kit `Input field` fill bg-primary (`BG`, ruling C-F) |
| 4 | `BG_TERTIARY` :25 | `rgb(0x27272A)` | revalue | `rgb(0x262626)` | bg-tertiary | chips, tracks, filled secondary buttons |
| 5 | `BG_HOVER` :26 | `rgb(0x232329)` | revalue | `rgb(0x171717)` | bg-primary_hover | hover on a bg-primary surface; kit menu item, tab and nav hover |
| 6 | `BG_PRESSED` :27 | `rgb(0x2E2E36)` | revalue | `rgb(0x404040)` | bg-quaternary | press state (egui `active`); kit has no pressed role; only text-primary/text-secondary may sit on it (AA, spec A rule 6) |
| 7 | `BG_MENU` :28 | `rgb(0x18181B)` | revalue | `rgb(0x0A0A0A)` | bg-primary | kit `Dropdown menu` > Menu fill |
| 8 | `BG_SELECTED` :31 | `BG_PRESSED` | revalue | `rgb(0x262626)` | bg-secondary_hover | selected tool, tab and row; was an alias of BG_PRESSED, now its own literal; decided by R8.2 (owner: "A · #262626") |
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
| 29 | `TEXT_BRAND` :54 | `rgb(0xFF9E66)` | revalue | `rgb(0xF97316)` | fg-brand-secondary (Studio orange) | accent text (New, Fix, undo-step marks); decided by R8.3 (owner: "A · cam #F97316") |
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
| 74 | `FIND_HIGHLIGHT` :318 | `rgba(0x0D99FF, 0x59)` | keep | unchanged | Studio-only: info family | no Untitled highlight role |
| 75 | `FIND_HIGHLIGHT_SELECTED` :319 | `rgba(0x7CC4F8, 0x66)` | keep | unchanged | Studio-only: info family | same |
| 76 | `BG_MENU_SELECTED` :429 | `BG_SELECTED` | alias | unchanged (alias) | = BG_SELECTED | alias kept |
| 77 | `MENU_SHORTCUT_HIGHLIGHT` :448 | `rgba(0xFFFFFF, 0xCC)` | revalue | `rgb(0xE5E5E5)` | text-secondary_hover | hovered menu text in the kit; spec J applies the kit shortcut anatomy (text-quaternary) |
| 78 | `COMMENT_PIN_RESOLVED` :477 | `rgb(0x757575)` | keep | unchanged | canvas overlay | comment pins sit on the canvas |
| 79 | `AVATAR_COLORS` :479 | `[rgb(0xF24822), rgb(0xFFA629), rgb(0x14AE5C), rgb(0x0C8CE9), rgb(0x06B6D4), rgb(0xEE46BC)]` | keep | unchanged | content (identity colours) | picked from the author's name |
| 80 | `RULER_TEXT` :535 | `rgba(0xFFFFFF, 0xB2)` | keep | unchanged | canvas overlay | the 9px ruler exception; the ruler strip fill is BG |
| 81 | `RULER_TICK` :536 | `rgba(0xFFFFFF, 0x66)` | keep | unchanged | canvas overlay | same |
| 82 | `RULER_SELECTION_FILL` :538 | `rgba(0x0D99FF, 0x4D)` | keep | unchanged | canvas overlay | same |
| 83 | `RULER_GUIDE` :540 | `rgb(0xF24822)` | keep | unchanged | canvas overlay | same |
| 84 | `PIXEL_GRID_LIGHT_LINES` :542 | `rgba(0xFFFFFF, 0x1A)` | keep | unchanged | canvas overlay | same |
| 85 | `PIXEL_GRID_DARK_LINES` :543 | `rgba(0x000000, 0x1A)` | keep | unchanged | canvas overlay | same |
| 86 | `TEXT_SELECTION` :550 | `Color32::from_rgba_premultiplied(4, 49, 81, 89)` | keep | unchanged | canvas overlay | selected text on the canvas |
| 87 | `FIND_CANVAS_MATCH` :552 | `TEXT_SELECTION` | keep | unchanged | canvas overlay (alias) | same |
| 88 | `FIND_CANVAS_MATCH_CURRENT` :554 | `BG_BRAND` | keep | unchanged | alias of BG_BRAND | brand unchanged |
| 89 | `SYNTAX_PLAIN` :562 | `TEXT` | alias | unchanged (alias) | = TEXT (text-primary) | follows TEXT |
| 90 | `SYNTAX_PROPERTY` :563 | `rgb(0x7CC4F8)` | revalue | `rgb(0x7DD3FC)` | utility-sky-700 | Dev code blocks: kit dark utility steps replace the Material palette; spec G may refine within utility roles |
| 91 | `SYNTAX_KEYWORD` :564 | `rgb(0xC792EA)` | revalue | `rgb(0xF472B6)` | utility-pink-600 | #C792EA is violet (hue 276 deg) and breaks the Purple Ban at HEAD; pink-400 is outside the banned band |
| 92 | `SYNTAX_STRING` :565 | `rgb(0x9CD67D)` | revalue | `rgb(0x4ADE80)` | utility-green-600 | kit dark utility step |
| 93 | `SYNTAX_NUMBER` :566 | `rgb(0xF2A365)` | revalue | `rgb(0xFB923C)` | utility-orange-600 | kit dark utility step |
| 94 | `SYNTAX_TYPE` :567 | `rgb(0xFFCB6B)` | revalue | `rgb(0xFACC15)` | utility-yellow-600 | kit dark utility step |
| 95 | `SYNTAX_FUNCTION` :568 | `rgb(0x82AAFF)` | revalue | `rgb(0x60A5FA)` | utility-blue-600 | kit dark utility step |
| 96 | `SYNTAX_COMMENT` :569 | `rgb(0x8A8A8A)` | revalue | `rgb(0xA3A3A3)` | text-quaternary | comments are quiet text; fg-quaternary #737373 would fail AA |
| 97 | `SYNTAX_PUNCT` :570 | `rgb(0xB3B3B3)` | revalue | `rgb(0xD4D4D4)` | text-secondary | punctuation one step below plain text |
| 98 | `DEV_READY` :572 | `rgb(0x14AE5C)` | keep | unchanged | Dev overlay | controller default: Dev ready emerald #14AE5C; R10.4 makes it the one Dev mode accent (Dev toggle, tab badge, Plugins row) with the dark label `TEXT_ON_BRAND` #0A0A0A, 6.82:1 (owner: "A · #14AE5C xanh Figma") |
| 99 | `DEV_COMPLETED` :573 | `rgb(0x06B6D4)` | keep | unchanged | Dev overlay | same |
| 100 | `DEV_CHANGED` :574 | `rgb(0xFFA629)` | keep | unchanged | Dev overlay | same |
| 101 | `STATUS_INFO_TEXT` :576 | `rgb(0x0D99FF)` | keep | unchanged | Studio-only: info family | agent Working badge, also drawn on the canvas |
| 102 | `STATUS_INFO_BG` :577 | `rgba(0x0D99FF, 0x24)` | keep | unchanged | Studio-only: info family | same |
| 103 | `STATUS_INFO_BORDER` :578 | `rgba(0x0D99FF, 0x59)` | keep | unchanged | Studio-only: info family | same |
| 104 | `BOX_MARGIN` :580 | `rgb(0xF9CB9C)` | keep | unchanged | Dev overlay | box-model diagram keeps Inspect parity |
| 105 | `BOX_PADDING` :581 | `rgb(0xB6D7A8)` | keep | unchanged | Dev overlay | same |
| 106 | `BOX_CONTENT` :582 | `rgb(0x9FC5E8)` | keep | unchanged | Dev overlay | same |
| 107 | `BOX_TEXT` :583 | `rgb(0x1E1E1E)` | keep | unchanged | Dev overlay | same |
| 108 | `ANNOTATION_BG` :585 | `rgb(0xFFE08A)` | keep | unchanged | canvas overlay | annotation callouts |
| 109 | `ANNOTATION_TEXT` :586 | `rgb(0x1E1E1E)` | keep | unchanged | canvas overlay | same |
| 110 | `ANNOTATION_LINE` :587 | `rgb(0xFFB800)` | keep | unchanged | canvas overlay | same |
| 111 | `MEASURE` :589 | `rgb(0xF24822)` | keep | unchanged | canvas overlay | measurement red |
| 112 | `PROTO_BLUE` :594 | `BG_BRAND` | keep | unchanged | alias of BG_BRAND (canvas noodles) | brand unchanged |
| 113 | `PRESENT_BG` :603 | `rgb(0x09090B)` | revalue | `rgb(0x0A0A0A)` | bg-primary | presentation stage around the frame |
| 114 | `HOME_THUMB_BG` :709 | `rgb(0x3A3A3A)` | revalue | `rgb(0x262626)` | bg-tertiary | thumbnail placeholder, one step above the bg-secondary card |
| 115 | `VAR_SCROLL_THUMB` :731 | `rgb(0x707070)` | revalue | `rgb(0x737373)` | fg-quaternary | non-text graphic, 3:1 needed: 3.78:1 on bg-secondary |
| 116 | `BOARD_DOT` :753 | `rgba(0x000000, 0x2E)` | keep | unchanged | board content | whiteboard canvas dots |

| # | New constant (new name) | Value | Untitled token | Reason |
|---|---|---|---|---|
| N1 | `BG_SECONDARY_HOVER` | `rgb(0x262626)` | bg-secondary_hover | hover on a bg-secondary surface (cards, list rows); egui `widgets.hovered` uses it because BG_HOVER now equals BG_SECONDARY |
| N2 | `TEXT_SECONDARY_HOVER` | `rgb(0xE5E5E5)` | text-secondary_hover | kit hover text for menu items, nav items and secondary buttons |
| N3 | `TEXT_TERTIARY_HOVER` | `rgb(0xD4D4D4)` | text-tertiary_hover | kit hover text for tertiary and link-gray buttons |
| N4 | `TEXT_QUATERNARY` | `rgb(0xA3A3A3)` | text-quaternary | kit non-current tab text and menu shortcuts |
| N5 | `TEXT_PLACEHOLDER` | `rgb(0x737373)` | text-placeholder (kit value, R8.1) | owner chose the kit value ("B · #737373 của kit"): 3.78:1 on bg-secondary, 4.18:1 on bg-primary; the one named exception to the AA text law; placeholder only on bg-primary or bg-secondary fields (3.19:1 on bg-tertiary, 2.19:1 on bg-quaternary are not allowed) |
| N6 | `ICON_QUATERNARY` | `rgb(0x737373)` | fg-quaternary | kit input leading icons and nav icons; non-text 3:1 holds up to bg-tertiary (3.19:1) |
| N7 | `TEXT_ERROR` | `rgb(0xF87171)` | text-error-primary | error text; 7.16:1 on bg-primary, passes on bg-tertiary |
| N8 | `TEXT_WARNING` | `rgb(0xFACC15)` | text-warning-primary | warning text |
| N9 | `TEXT_SUCCESS` | `rgb(0x4ADE80)` | text-success-primary | success text |
| N10 | `TEXT_ON_BRAND` | `rgb(0x0A0A0A)` | Neutral-950 (dark label, R4) | label on BG_BRAND, BG_BRAND_HOVER, BG_BRAND_PRESSED: 5.73 / 7.06 / 5.56:1 |
| N11 | `FOCUS_RING` | `rgb(0xF97316)` | focus-ring (R4: Orange-500) | focus ring for every focusable control (spec C draws it) |
| N12 | `BG_ERROR_SOLID` | `rgb(0xDC2626)` | bg-error-solid | destructive button fill; label TEXT_WHITE at 4.83:1 |
| N13 | `TEXT_WHITE` | `rgb(0xFFFFFF)` | text-white | label on bg-error-solid (kit destructive button) |
| N14 | `ICON_BRAND_PRIMARY` | `rgb(0xF97316)` | fg-brand-primary (R4 remap: kit `#9E77ED` Brand-500 → Orange-500) | spec C: menu check, toast dot, slider and progress fill, spinner arc; non-text 7.06:1 on bg-primary |
| N15 | `ICON_SECONDARY_HOVER` | `rgb(0xE5E5E5)` | fg-secondary_hover | spec C: selected icon |
| N16 | `ICON_QUATERNARY_HOVER` | `rgb(0xA3A3A3)` | fg-quaternary_hover | spec C: icon hover and pressed |
| N17 | `ICON_WHITE` | `rgb(0xFFFFFF)` | fg-white | spec C: toggle knob |
| N18 | `BG_PRIMARY_ALT` | `rgb(0x171717)` | bg-primary_alt (dark: alias of bg-secondary) | spec C: selected segment, toast fill |
| N19 | `BG_PRIMARY_SOLID` | `rgb(0x262626)` | bg-primary-solid (dark: alias of bg-tertiary) | spec C: tooltip fill; `TEXT` on it 14.50:1 |
| N20 | `BG_ERROR_SOLID_HOVER` | `rgb(0xB91C1C)` | bg-error-solid_hover, owner override R8.7 (`_Primitives` Red-700; the kit's dark value is Red-500 `#EF4444`, its light-mode value is Red-700) | destructive button hover; `TEXT_WHITE` on it 6.47:1 (kit `#EF4444` would be 3.76:1). Owner: "C · #B91C1C Red-700" |
| N21 | `UTILITY_NEUTRAL_50` | `rgb(0x171717)` | utility-neutral-50 (dark: Neutral-900) | spec C: count badge fill |
| N22 | `UTILITY_NEUTRAL_200` | `rgb(0x404040)` | utility-neutral-200 (dark: Neutral-700) | spec C: count badge border |
| N23 | `UTILITY_NEUTRAL_700` | `rgb(0xD4D4D4)` | utility-neutral-700 (dark: Neutral-300) | spec C: count badge text, 12.09:1 on `UTILITY_NEUTRAL_50` |
| N24 | `BG_SUCCESS_PRIMARY` | `rgb(0x052E16)` | bg-success-primary (= utility-green-50, Green-950) | G-1: success badge and banner fill. G-1 source (N24–N32): resolved export, matched 273/273 by `evidence/figma-live-variable-check.md`; badge anatomy (utility-<colour>-50 fill, -200 stroke, -700 text) from `designos "$KIT" --select "Badge" --export-spec` |
| N25 | `BG_WARNING_PRIMARY` | `rgb(0x422006)` | bg-warning-primary (= utility-yellow-50, Yellow-950) | G-1: warning badge and banner fill |
| N26 | `BG_ERROR_PRIMARY` | `rgb(0x450A0A)` | bg-error-primary (= utility-red-50, Red-950) | G-1: error badge and banner fill |
| N27 | `UTILITY_GREEN_200` | `rgb(0x166534)` | utility-green-200 (Green-800) | G-1: success badge border (kit `Badge` stroke); decorative, 2.09:1 on its fill |
| N28 | `UTILITY_YELLOW_200` | `rgb(0x854D0E)` | utility-yellow-200 (Yellow-800) | G-1: warning badge border; decorative, 2.13:1 |
| N29 | `UTILITY_RED_200` | `rgb(0x991B1B)` | utility-red-200 (Red-800) | G-1: error badge border; decorative, 1.94:1 |
| N30 | `UTILITY_GREEN_700` | `rgb(0x86EFAC)` | utility-green-700 (Green-300) | G-1: success badge text, 10.62:1 on `BG_SUCCESS_PRIMARY` |
| N31 | `UTILITY_YELLOW_700` | `rgb(0xFDE047)` | utility-yellow-700 (Yellow-300) | G-1: warning badge text, 11.06:1 on `BG_WARNING_PRIMARY` |
| N32 | `UTILITY_RED_700` | `rgb(0xFCA5A5)` | utility-red-700 (Red-300; not the Red-700 primitive of N20) | G-1: error badge text, 8.51:1 on `BG_ERROR_PRIMARY` |
| N33 | `BG_OVERLAY` | `rgba(0x262626, 0xB3)` | bg-overlay (Neutral-800) at the kit overlay's 70% layer opacity | G-3: modal scrim; the kit adds a background blur: Figma `BACKGROUND_BLUR` radius 16, which Figma's own CSS export writes as `backdrop-filter: blur(8px)` (CSS `blur()` takes the Gaussian standard deviation, so radius 16 = sigma 8; read-only `getCSSAsync` on 3 overlay nodes, R9). Specs I, J and K draw no blur, so the numbers are documentation only. Source: live Figma, read-only (R9): Modals page, node "Background overlay", fill bound to `Colors/Background/bg-overlay`, layer opacity 0.70, `BACKGROUND_BLUR` 16; the dark value of bg-overlay from the resolved export, matched 273/273 by `evidence/figma-live-variable-check.md` |
| N34 | `PROVIDER_CLAUDE` | `rgb(0xD97757)` | none: third-party identity (G-2, content) | provider mark in agent tabs (`terminal.rs:4781-4799` today) |
| N35 | `PROVIDER_CODEX` | `rgb(0xFAFAFA)` | none: identity, equals text-primary | Codex and OpenCode mark |
| N36 | `PROVIDER_GROK` | `rgb(0xE5E5E5)` | none: identity, Neutral-200 (was Zinc-200 `#E4E4E7`) | Grok mark |
| N37 | `PROVIDER_GEMINI` | `rgb(0x60A5FA)` | none: identity, equals utility-blue-600 (dark) | Antigravity and Gemini mark |
| N38 | `BG_ERROR_SOLID_PRESSED` | `rgb(0x991B1B)` | Red-800 (same value as N29 `UTILITY_RED_200`; kit has no pressed role) | spec C: destructive button pressed, one step past R8.7 hover; `TEXT_WHITE` on it 8.31:1 |
| N39 | `UTILITY_BLUE_50` | `rgb(0x172554)` | utility-blue-50 (Blue-950) | R10.2: Home category tile, design file (kit `Featured icon` tile) |
| N40 | `UTILITY_BLUE_600` | `rgb(0x60A5FA)` | utility-blue-600 (Blue-400) | R10.2: design-file glyph on `UTILITY_BLUE_50`, 5.78:1 (same value as `PROVIDER_GEMINI`) |
| N41 | `UTILITY_AMBER_50` | `rgb(0x451A03)` | utility-amber-50 (Amber-950) | R10.2: Home category tile, board |
| N42 | `UTILITY_AMBER_600` | `rgb(0xFBBF24)` | utility-amber-600 (Amber-400) | R10.2: board glyph on `UTILITY_AMBER_50`, 8.97:1 |
| N43 | `UTILITY_GREEN_50` | `rgb(0x052E16)` | utility-green-50 (Green-950) | R10.2: Home category tile, deck (same value as `BG_SUCCESS_PRIMARY`) |
| N44 | `UTILITY_GREEN_600` | `rgb(0x4ADE80)` | utility-green-600 (Green-400) | R10.2: deck glyph on `UTILITY_GREEN_50`, 8.55:1 (same value as `TEXT_SUCCESS`) |
| N45 | `UTILITY_ORANGE_50` | `rgb(0x431407)` | utility-orange-50 (Orange-950) | R10.2: Home category tile, spec bundle (same value as `BG_TEXT_SELECTION`) |
| N46 | `UTILITY_ORANGE_600` | `rgb(0xFB923C)` | utility-orange-600 (Orange-400) | R10.2: spec-bundle glyph on `UTILITY_ORANGE_50`, 6.92:1 (same value as `BORDER_SELECTED`) |
| N47 | `DEV_READY_HOVER` | `rgb(0x30B870)` | none: Studio Dev accent, hover | D toggle hover (G rule 2): `DEV_READY` mixed 12 % toward white; `TEXT_ON_BRAND` on it 7.75:1 (C-1.63) |
| N48 | `HUD_POINTER_FILL` | `rgb(0x111216)` | none: content (a cursor, not chrome) | D recording HUD pointer body; value unchanged from today |
| N49 | `PLAYGROUND_GRID_DOT` | `rgba(0xFAFAFA, 0x12)` | none: `TEXT` at 7 % alpha | G Dev playground preview stage dot grid (today white at alpha 18) |
| N50 | `SWATCH_PLACEHOLDER` | `rgb(0x404040)` | bg-quaternary | F placeholder swatch for an image or empty paint (replaces three greys); same value as `BG_PRESSED` |
| N51 | `TYPE_PREVIEW_BASELINE` | `rgba(0x0D99FF, 0x6E)` | none: content overlay (info blue at alpha 110) | F type-details baseline guide drawn over a type preview; value unchanged |
| N52 | `CONTENT_HANDLE_FILL` | `CANVAS_HANDLE_FILL` | none: content overlay (alias) | F gradient, picker and slider handles drawn over content; `#FFFFFF`, alias of `CANVAS_HANDLE_FILL` |
| N53 | `CONTENT_HANDLE_RING` | `rgba(0x000000, 0x4D)` | none: content overlay | F handle outline over content; value unchanged |
| N54 | `CONTENT_HANDLE_CASING` | `rgba(0x000000, 0x66)` | none: content overlay | F handle casing over content; value unchanged |
| N55 | `CONTENT_HANDLE_AXIS` | `rgba(0xFFFFFF, 0xB2)` | none: content overlay | F handle axis line over content; value unchanged |
| N56 | `CONTENT_HANDLE_STOP_IDLE` | `rgba(0xFFFFFF, 0xCC)` | none: content overlay | F idle gradient stop over content; value unchanged |
| N57 | `CONTENT_MARK_SHADOW` | `rgba(0x000000, 0x6E)` | none: content overlay | F measurement-pill and mark shadow over content; value unchanged |
| N58 | `CONTENT_MARK_LIGHT` | `rgba(0xFFFFFF, 0xDC)` | none: content overlay | F mark highlight over content; value unchanged |
| N59 | `READER_LIGHT_BG_PAGE` | `rgb(0xFAFAFA)` | kit Light mode value (G rule 4) | G spec reader light field: page behind the sheet |
| N60 | `READER_LIGHT_DOC_SHEET` | `rgb(0xFFFFFF)` | kit Light mode value (G rule 4) | G spec reader light field: document sheet |
| N61 | `READER_LIGHT_SURFACE` | `rgb(0xFFFFFF)` | kit Light mode value (G rule 4) | G spec reader light field: surface |
| N62 | `READER_LIGHT_SURFACE_2` | `rgb(0xF5F5F5)` | kit Light mode value (G rule 4) | G spec reader light field: second surface |
| N63 | `READER_LIGHT_SURFACE_3` | `rgb(0xFAFAFA)` | kit Light mode value (G rule 4) | G spec reader light field: third surface |
| N64 | `READER_LIGHT_CODE_BG` | `rgb(0xFAFAFA)` | kit Light mode value (G rule 4) | G spec reader light field: code block fill |
| N65 | `READER_LIGHT_BORDER` | `rgb(0xE5E5E5)` | kit Light mode value (G rule 4) | G spec reader light field: border |
| N66 | `READER_LIGHT_HAIRLINE` | `rgb(0xF5F5F5)` | kit Light mode value (G rule 4) | G spec reader light field: hairline |
| N67 | `READER_LIGHT_TEXT` | `rgb(0x171717)` | kit Light mode value (G rule 4) | G spec reader light field: text |
| N68 | `READER_LIGHT_TEXT_MUTED` | `rgb(0x404040)` | kit Light mode value (G rule 4) | G spec reader light field: muted text |
| N69 | `READER_LIGHT_TEXT_SUBTLE` | `rgb(0x525252)` | kit Light mode value (G rule 4) | G spec reader light field: subtle text; 7.17:1 on `READER_LIGHT_SURFACE_2` (C-1.64), the lowest light text pair |
| N70 | `READER_LIGHT_DOT_OK` | `rgb(0x16A34A)` | kit Light mode value (G rule 4) | G spec reader light field: OK status dot |
| N71 | `READER_LIGHT_DOT_WARN` | `rgb(0xCA8A04)` | kit Light mode value (G rule 4) | G spec reader light field: warning status dot; 2.94:1 on a white sheet, under the 3:1 non-text floor: the kit's own light value in a hidden reader, flagged by G, not changed |
| N72 | `READER_LIGHT_DOT_NEUTRAL` | `rgb(0xA3A3A3)` | kit Light mode value (G rule 4) | G spec reader light field: neutral status dot; 2.52:1 on a white sheet, under the 3:1 floor: same flag |
| N73 | `READER_LIGHT_HIGHLIGHT` | `rgb(0xFEFCE8)` | kit Light mode value (G rule 4) | G spec reader light field: highlight |
| N74 | `FOCUS_RING_ERROR` | `rgb(0xEF4444)` | focus-ring-error | spec C: the 2 px ring of the destructive buttons (kit `Focus rings/focus-ring-error`; R4's orange ring is for everything else); same value as `DANGER` |
| N75 | `BORDER_ERROR_SUBTLE` | `rgb(0xEF4444)` | border-error_subtle | spec C: resting and hover border of `Button/SecondaryDestructive`; same value as `DANGER` |
| N76 | `BORDER_ERROR` | `rgb(0xF87171)` | border-error | spec C: pressed border of `Button/SecondaryDestructive`; same value as `TEXT_ERROR` |
| N77 | `TEXT_ERROR_HOVER` | `rgb(0xFCA5A5)` | text-error-primary_hover | spec C: hover and pressed label and icon of `Button/SecondaryDestructive`; same value as `UTILITY_RED_700` |

Rules that follow from the AA run in spec A section 3:

- On `BG_PRESSED` (bg-quaternary `#404040`) only `TEXT` and `TEXT_SECONDARY` may sit (tertiary 4.11:1, accent 3.70:1, placeholder 2.19:1 fail).
- Placeholder (`TEXT_PLACEHOLDER` `#737373`, R8.1) is the one named exception to the AA text law (E-1): text fields take the kit `Input field` fill `bg-primary` (`BG`, ruling C-F), so the placeholder reads 4.18:1; a field inside a `bg-secondary` card reads 3.78:1. Never on `BG_TERTIARY` (3.19:1) or `BG_PRESSED` (2.19:1).
- Tooltip (controller ruling C-T, overrides the kit's `text-white` label): fill `BG_PRIMARY_SOLID` `#262626`, title `TEXT` `#FAFAFA` (14.50:1), supporting text `TEXT_SECONDARY` `#D4D4D4` (= tooltip-supporting-text, 10.21:1). No new constant. White text (`TEXT_WHITE`) sits only on `BG_ERROR_SOLID` and `BG_ERROR_SOLID_HOVER`.
- Status badges (G-1, kit `Badge` = utility-<colour>-50 fill, -200 stroke, -700 text, from `--select "Badge" --export-spec`): success `BG_SUCCESS_PRIMARY` / `UTILITY_GREEN_200` / `UTILITY_GREEN_700`, warning `BG_WARNING_PRIMARY` / `UTILITY_YELLOW_200` / `UTILITY_YELLOW_700`, error `BG_ERROR_PRIMARY` / `UTILITY_RED_200` / `UTILITY_RED_700`. Info badges keep the Studio-only `STATUS_INFO_*` family (the kit has no info role).
- Provider marks (G-2): identity content, like `AVATAR_COLORS` and the terminal's ANSI palette. They are exempt from Untitled role mapping but live in `theme.rs` (N34–N37) so the colour-literal gate does not count them in `terminal.rs`, and they obey the Purple Ban: a provider whose brand is violet gets a neutral mark.
- Home category tiles (R10.2, owner: "A · Ô màu utility"): kit `Featured icon` tiles, tile `UTILITY_<HUE>_50`, glyph `UTILITY_<HUE>_600`: design file blue, board amber, deck green, spec bundle orange (N39–N46). Glyph pairs are C-1.31–C-1.34.
- Dev mode accent (R10.4): `DEV_READY` `#14AE5C`, label `TEXT_ON_BRAND` `#0A0A0A` (C-1.35); "Ready" text in the accent on `BG` (C-1.36).
- Selected Home file card (R10.3): 2 px `BORDER_SELECTED` `#FB923C` (C-1.37, C-1.38).
- Resting control boundary (E-2): the kit's 1 px `border-primary`, `BORDER_STRONG` `#404040`, 1.91:1 on `BG`, 1.73:1 on `BG_SECONDARY`, on exactly these controls: unchecked checkbox and radio (owner R10.7: "B · #404040 của kit"), text field and select, and secondary button (controller ruling C-F, from R2 and R10.7, which the owner may reverse). Only on `bg-primary` or `bg-secondary`; never on `BG_TERTIARY` (1.46:1) or `BG_PRESSED` (same value). It is the second named exception, to the 3:1 non-text rule (the placeholder, R8.1, is the first, to the AA text rule). Its floors are E-2.a/b in (f), never C-1 passes.
- Inspector "Mixed" value (controller ruling C-R): `TEXT_TERTIARY` `#A3A3A3`, which passes AA on `BG` and `BG_SECONDARY` (C-1.7, C-1.8). "Mixed" is not a placeholder, so E-1 does not cover it: E-1 stays placeholder-only. Spec F's row that gives "Mixed" `TEXT_PLACEHOLDER` follows this rule (F's writer applies it).
- Drag dividers (controller ruling C-R: left-panel resize edge, #1108 Pages/Layers separator, ruler strips) keep their visible 1 px line and get a 24 px hit band centred on it, so no pointer target is below `TARGET_MIN` and no new exception exists. The #1108 constants keep today's values (appendix: `PAGES_SEPARATOR_HIT` 6 stays the core press band). The 24 px band is one new constant, `DRAG_DIVIDER_HIT` `24.0` (`= TARGET_MIN`, new name), declared by spec D and used by E for the separator; neither A nor B adds it, and no colour changes.
- Focus rings (R4 and the error-field ruling, 2026-10-28): every focus ring is `FOCUS_RING` `#F97316`, except a field in its error state and the destructive buttons, which take `FOCUS_RING_ERROR` `#EF4444` (N74, C-1.69).
- Secondary destructive button (spec C; controller ruling in plan.md, C/M rulings): hover and pressed fill is the kit tint `BG_ERROR_PRIMARY` `#450A0A` with the `TEXT_ERROR_HOVER` `#FCA5A5` label (C-1.65), not `BG_ERROR_SOLID_HOVER` Red-700 (R8.7 governs only the solid `Button/Destructive`); borders `BORDER_ERROR_SUBTLE` (resting, hover) and `BORDER_ERROR` (pressed), ring `FOCUS_RING_ERROR` (C-1.66–C-1.69).
- Error text uses `TEXT_ERROR` (`#F87171`); `DANGER` (`#EF4444`) is for icons, dots and strokes (4.02:1 on bg-tertiary).
- `ICON_QUATERNARY` and `VAR_SCROLL_THUMB` (`#737373`) are non-text graphics (3:1): never on `BG_PRESSED` (2.19:1).
- `BG_HOVER` equals `BG_SECONDARY` (`#171717`) in the kit; hover on a bg-secondary surface uses `BG_SECONDARY_HOVER` (new name).

### Shared values (generated)

One hex serves several roles. Generated by script from `theme.rs` at `8dfaee93` (identical at `d0555c82`) with (a) applied, N1–N77 included (aliases followed;
rgba constants grouped by hex and alpha). Two roles that share a value draw at 1.00:1 against each other: a `#262626`
hairline (`BORDER`, `BORDER_SUBTLE`, `BORDER_MENU`) on a selected or hovered `#262626` row, or a card edge against its
`HOME_THUMB_BG` thumbnail, is invisible. Surface specs must not rely on such a pair to separate two things.

| Value | Constants (`theme.rs` after spec A; alias = `= NAME` in code) | Count |
|---|---|---|
| `#262626` | `BG_TERTIARY`, `BG_SELECTED`, `BG_SELECTED_HOVER`, `BG_TOOLBAR_SELECTED` (alias), `BORDER`, `BORDER_SUBTLE`, `BORDER_MENU`, `ELEVATION_400_RING`, `INNER_HIGHLIGHT`, `BG_MENU_SELECTED` (alias), `HOME_THUMB_BG`, `BG_SECONDARY_HOVER`, `BG_PRIMARY_SOLID` | 13 |
| `#FFFFFF` | `ICON`, `ICON_SELECTED` (alias), `CHECKER_A`, `CANVAS_HANDLE_FILL`, `CANVAS_SIZE_BADGE_TEXT`, `GUIDE_TEXT`, `PEN_POINT`, `TEXT_WHITE`, `ICON_WHITE`, `CONTENT_HANDLE_FILL` (alias), `READER_LIGHT_DOC_SHEET`, `READER_LIGHT_SURFACE` | 12 |
| `#171717` | `BG_SECONDARY`, `BG_HOVER`, `BG_SELECTED_SECONDARY`, `BG_TOOLBAR_HOVER`, `BG_TOOLBAR_MODE_SWITCHER`, `BG_MENU_HOVER`, `BG_PRIMARY_ALT`, `UTILITY_NEUTRAL_50`, `READER_LIGHT_TEXT` | 9 |
| `#0A0A0A` | `BG`, `BG_PRIMARY`, `BG_MENU`, `BG_TOOLBAR`, `TEXT_ON_LIGHT_CANVAS`, `AGENT_LABEL_TEXT`, `PRESENT_BG`, `TEXT_ON_BRAND` | 8 |
| `#A3A3A3` | `TEXT_TERTIARY`, `ICON_TERTIARY`, `SYNTAX_COMMENT`, `TEXT_QUATERNARY`, `ICON_QUATERNARY_HOVER`, `READER_LIGHT_DOT_NEUTRAL` | 6 |
| `#FAFAFA` | `TEXT`, `SYNTAX_PLAIN` (alias), `PROVIDER_CODEX`, `READER_LIGHT_BG_PAGE`, `READER_LIGHT_SURFACE_3`, `READER_LIGHT_CODE_BG` | 6 |
| `#404040` | `BG_PRESSED`, `BORDER_STRONG`, `UTILITY_NEUTRAL_200`, `SWATCH_PLACEHOLDER`, `READER_LIGHT_TEXT_MUTED` | 5 |
| `#D4D4D4` | `TEXT_SECONDARY`, `ICON_SECONDARY`, `SYNTAX_PUNCT`, `TEXT_TERTIARY_HOVER`, `UTILITY_NEUTRAL_700` | 5 |
| `#E5E5E5` | `MENU_SHORTCUT_HIGHLIGHT`, `TEXT_SECONDARY_HOVER`, `ICON_SECONDARY_HOVER`, `PROVIDER_GROK`, `READER_LIGHT_BORDER` | 5 |
| `#E36323` | `BG_BRAND`, `DROP_INDICATOR`, `FIND_CANVAS_MATCH_CURRENT` (alias), `PROTO_BLUE` (alias) | 4 |
| `#F97316` | `BG_BRAND_HOVER`, `TEXT_BRAND`, `FOCUS_RING`, `ICON_BRAND_PRIMARY` | 4 |
| `#000000 at alpha 0x00` | `SHADOW_COLOR`, `SHADOW_COLOR_STRONG`, `ELEVATION_400_SHADOW` | 3 |
| `#06B6D4` | `COMPONENT`, `CANVAS_COMPONENT`, `DEV_COMPLETED` | 3 |
| `#4ADE80` | `SYNTAX_STRING`, `TEXT_SUCCESS`, `UTILITY_GREEN_600` | 3 |
| `#60A5FA` | `SYNTAX_FUNCTION`, `PROVIDER_GEMINI`, `UTILITY_BLUE_600` | 3 |
| `#737373` | `VAR_SCROLL_THUMB`, `TEXT_PLACEHOLDER`, `ICON_QUATERNARY` | 3 |
| `#EF4444` | `DANGER`, `FOCUS_RING_ERROR`, `BORDER_ERROR_SUBTLE` | 3 |
| `#FB923C` | `BORDER_SELECTED`, `SYNTAX_NUMBER`, `UTILITY_ORANGE_600` | 3 |
| `#FFFFFF at alpha 0x1A` | `BORDER_TRANSLUCENT`, `BG_TRANSPARENT_HOVER`, `PIXEL_GRID_LIGHT_LINES` | 3 |
| `#052E16` | `BG_SUCCESS_PRIMARY`, `UTILITY_GREEN_50` | 2 |
| `#0D99FF at alpha 0x59` | `FIND_HIGHLIGHT`, `STATUS_INFO_BORDER` | 2 |
| `#0D99FF` | `INFO`, `STATUS_INFO_TEXT` | 2 |
| `#14AE5C` | `AGENT_2`, `DEV_READY` | 2 |
| `#1E1E1E` | `BOX_TEXT`, `ANNOTATION_TEXT` | 2 |
| `#3582FF at alpha 0x33` | `BG_SELECTED_AREA`, `CANVAS_MARQUEE_FILL` (alias) | 2 |
| `#431407` | `BG_TEXT_SELECTION`, `UTILITY_ORANGE_50` | 2 |
| `#991B1B` | `UTILITY_RED_200`, `BG_ERROR_SOLID_PRESSED` | 2 |
| `#F24822` | `RULER_GUIDE`, `MEASURE` | 2 |
| `#F5F5F5` | `READER_LIGHT_SURFACE_2`, `READER_LIGHT_HAIRLINE` | 2 |
| `#F87171` | `TEXT_ERROR`, `BORDER_ERROR` | 2 |
| `#FACC15` | `SYNTAX_TYPE`, `TEXT_WARNING` | 2 |
| `#FCA5A5` | `UTILITY_RED_700`, `TEXT_ERROR_HOVER` | 2 |
| `#FFA629` | `AGENT_1`, `DEV_CHANGED` | 2 |
| `#FFFFFF at alpha 0xB2` | `RULER_TEXT`, `CONTENT_HANDLE_AXIS` | 2 |
| `Color32::from_rgba_premultiplied(4, 49, 81, 89)` | `TEXT_SELECTION`, `FIND_CANVAS_MATCH` (alias) | 2 |

## (b) `design/design.tokens.json` colour roles → new values

The store keeps its 48 role keys (no add or delete path in the sealed store; see spec A section 2). Each re-value
is one `ui ds change-token colors.<key> --value <hex> --reason "<reason>" --dir .` (reason from the tsv below, Studio's wording, R14), which keeps
`$extensions.design-os.role`, bumps the generation and appends a changelog entry.

| Token path | HEAD value | New value | Untitled token / reason (review aid, not shipped) |
|---|---|---|---|
| `colors.accent` | `#2b1d16` | `#431407` | utility-orange-50 (Orange-950): brand-tinted text-selection fill |
| `colors.accent-foreground` | `#ff9e66` | `#f97316` | fg-brand-secondary with Studio orange (Orange-500): accent text |
| `colors.agent-1` | `#ffa629` | unchanged | Studio-only role kept (see (e)) |
| `colors.agent-2` | `#14ae5c` | unchanged | Studio-only role kept (see (e)) |
| `colors.agent-3` | `#ee46bc` | unchanged | Studio-only role kept (see (e)) |
| `colors.agent-4` | `#38bdf8` | unchanged | Studio-only role kept (see (e)) |
| `colors.bg` | `#18181b` | `#0a0a0a` | bg-primary |
| `colors.bg-active` | `#2e2e36` | `#262626` | bg-secondary_hover: selected row (R8.2) |
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

The 41 re-values as a tab-separated list (key, value, reason) for the spec A loop. The reasons are Studio's own wording (R14): they land in the store's changelog, which ships, so they name no Untitled role:

```tsv
accent	#431407	brand-tinted text-selection fill (Orange-950)
accent-foreground	#f97316	accent text (Orange-500, R8.3)
bg	#0a0a0a	chrome base
bg-active	#262626	selected row fill (R8.2)
bg-foreground	#fafafa	primary text
bg-hover	#171717	hover on the chrome base
border	#262626	hairline between regions
border-focus	#fb923c	focused-field and selected-card outline (Orange-400)
border-strong	#404040	1px resting control boundary (E-2)
border-subtle	#262626	subtlest divider
canvas	#0a0a0a	chrome around the canvas
canvas-foreground	#fafafa	primary text
card	#171717	card fill
card-foreground	#fafafa	primary text
component-foreground	#0a0a0a	dark label on the component cyan (R4)
destructive	#dc2626	destructive button fill
destructive-foreground	#ffffff	label on the destructive fill
info-foreground	#0a0a0a	dark label (R4)
input	#0a0a0a	text field fill (C-F)
muted	#171717	muted surface
muted-foreground	#a3a3a3	tertiary text
neutral	#737373	neutral status glyph (Neutral-500)
neutral-foreground	#fafafa	primary text
popover	#0a0a0a	menu and popover fill
popover-foreground	#fafafa	primary text
primary-active	#ea580c	pressed brand (Orange-600)
primary-foreground	#0a0a0a	dark label on Studio orange (R4)
primary-hover	#f97316	brand hover (Orange-500)
ring	#f97316	focus ring (R4, Orange-500)
secondary	#262626	secondary button fill
secondary-active	#404040	secondary button pressed
secondary-foreground	#fafafa	primary text
secondary-hover	#404040	secondary button hover
success	#22c55e	success status
success-foreground	#0a0a0a	dark label (R4)
surface	#171717	surface fill
surface-active	#262626	selected surface (R8.2)
surface-foreground	#fafafa	primary text
surface-hover	#262626	hover on a surface
warning	#eab308	warning status
warning-foreground	#0a0a0a	dark label (R4)
```

## (c) Scale constants → Untitled step (R5)

Type sizes and weights come from the kit text styles; radii and spacing from the `2_ Radius` and `3_ Spacing`
collections; control heights from `Buttons/Button` and `Input field` sizes; badge heights from `Badge`.

| Constant (`theme.rs` line) | HEAD value | New value | Untitled step | Reason |
|---|---|---|---|---|
| `FONT_SIZE_CAPTION` :117 | `10.0` | `12.0` | text-xs | R5 labels: captions, sublabels, rail labels, counts |
| `FONT_SIZE_BODY` :120 | `11.0` | `12.0` | text-xs | R5 rows: body, controls, menu rows, values in editor panels |
| `FONT_SIZE_LARGE` :121 | `13.0` | `14.0` | text-sm | R5 panel titles (modal titles use the kit text-md 16, `FONT_SIZE_TEXT_MD`, spec C) |
| `LINE_BODY` :124 | `16.0` | `18.0` | text-xs line height | kit Text xs 12/18 |
| `LINE_CAPTION` :125 | `14.0` | `18.0` | text-xs line height | kit Text xs 12/18 |
| `UI_FONT_WEIGHT` :131 | `450.0` | `400.0` | Regular | kit weights 400/500/600/700 |
| `UI_FONT_WEIGHT_STRONG` :132 | `550.0` | `600.0` | Semibold | kit Semibold; 550 is not a kit weight |
| `RADIUS_SEGMENT` :192 | `4` | `6` | radius-sm | RADIUS_CONTROL 8 - INSET_SEGMENT 2 |
| `RADIUS_CONTROL` :194 | `6` | `8` | radius-md | R5 radius md 8 |
| `RADIUS_FRAME` :196 | `14` | `16` | radius-2xl | RADIUS_CONTROL 8 + INSET_FRAME 8 (concentric law); decided by R8.5 (owner: "A · 16, item 8, inset 8") |
| `PAGE_ROW_HEIGHT` :287 | `CONTROL_DENSE` | `CONTROL_XS` | button xs / row 32 | R5 row 32 (was CONTROL_DENSE 24) |
| `MENU_ITEM_HEIGHT` :424 | `CONTROL_DENSE` | `CONTROL_XS` | row 32 | R5 row 32 (was CONTROL_DENSE 24) |
| `RIGHT_TABS_H` :337 | `CONTROL_DENSE` | unchanged (`CONTROL_DENSE` 24) | none (held by R10.1) | R10.1 (owner, board 3 Q1: "A · Giữ như hiện tại"): editor tab heights stay, R1 wins over R5 here; the Inspector segmented strip stays 24 and the `theme.rs:854` assertion stays. File tabs (26 in the 32 bar, `tabs.rs:1007`) and session tabs (24, `terminal.rs:5804`) are local literals with no `theme.rs` row, so they keep their heights without a contract row |
| `FIELD_HEIGHT` :351 | `CONTROL_DENSE` | `CONTROL_XS` | control xs 32 | R5 small end; kit has no input below Input sm 36; decided by R8.4 (owner: "A · 32px") |
| `FIND_FIELD_H` :311 | `CONTROL_STANDARD` | `CONTROL_XS` | control xs 32 | was CONTROL_STANDARD 28 |
| `LABEL_BOX_H` :370 | `LINE_CAPTION` | `LINE_CAPTION` | text-xs line 18 | follows LINE_CAPTION 14 -> 18 |
| `SUBLABEL_HEIGHT` :367 | `16.0` | `LINE_CAPTION` | text-xs line 18 | was literal 16; a label box is one caption line |
| `FIND_GROUP_ROW_H` :316 | `28.0` | `CONTROL_XS` | row 32 | was literal 28; R5 row 32 |
| `STYLE_FOLDER_ROW_H` :399 | `24.0` | `CONTROL_XS` | row 32 | was literal 24; R5 row 32 |
| `VAR_PILL_H` :735 | `CONTROL_DENSE` | `BADGE_MD` | Badge md 24 | was CONTROL_DENSE 24; a pill is a badge, not a control |
| `CODE_LINE_HEIGHT` :590 | `LINE_BODY` | `LINE_BODY` | text-xs line 18 | follows LINE_BODY 16 -> 18 |
| `CONTROL_TOOL` :257 | `32.0` | renamed to `CONTROL_XS` (value kept) | Buttons/Button Size=xs height | mechanical rename, see (d) |
| `CONTROL_ROW` :258 | `40.0` | renamed to `CONTROL_MD` (value kept) | Buttons/Button Size=md, Input field Size=md | mechanical rename, see (d) |
| `CONTROL_DENSE` :255 | `24.0` | unchanged (legacy, see (d)) | none (24 = Badge md / TARGET_MIN; 28 = Badge lg) | call sites mix buttons, rows and chips; surface specs re-point each site; stays defined as the base of `RIGHT_TABS_H` (R10.1) |
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
| `FONT_SIZE_TEXT_MD` | `16.0` | Text md 16/24 | Styles panel; modal titles (spec C) |
| `FONT_SIZE_TEXT_LG` | `18.0` | Text lg 18/28 | Styles panel |
| `FONT_SIZE_TEXT_XL` | `20.0` | Text xl 20/30 | Styles panel |
| `LINE_TEXT_XS` | `18.0` | Text xs line | kit text style line height |
| `LINE_TEXT_SM` | `20.0` | Text sm line | kit text style line height |
| `LINE_TEXT_MD` | `24.0` | Text md line | kit text style line height |
| `LINE_TEXT_LG` | `28.0` | Text lg line | kit text style line height |
| `LINE_TEXT_XL` | `30.0` | Text xl line | kit text style line height |
| `RADIUS_XL` (`u8`) | `12` | radius-xl | G-4: cards and nested panels that hold controls at inset 4 (12 − 4 = 8 = `RADIUS_CONTROL`); chain `RADIUS_FRAME` 16 > `RADIUS_XL` 12 > `RADIUS_CONTROL` 8 > `RADIUS_SEGMENT` 6 > `RADIUS_MARK` 2 |

Constants declared by the surface specs D–G (not B's, and not counted in B's "15 added, 2 renamed"; listed so each name has exactly one value; each lives in `theme.rs` once its spec lands):

| New constant (new name) | Value | Owner and use |
|---|---|---|
| `DRAG_DIVIDER_HIT` | `24.0` | spec D declares it (value `= TARGET_MIN`): the hit band of every drag divider (left-panel resize strip, ruler strips); E uses it for the #1108 Pages/Layers separator (C-R). Neither A nor B adds it; `PAGES_SEPARATOR_HIT` 6 keeps its value as the core press band |
| `DEV_STATUS_TINT` | `0.22` | alpha of the status colour in the Dev status pill and the Dev count badge (E, G); today the literal `22 %` |
| `DEV_TINT_REST` | `0.16` | alpha of `DEV_READY` in E's "Mark selection ready" button at rest |
| `DEV_TINT_HOVER` | `0.24` | the same button on hover (E) |
| `TYPE_LABEL_COMPONENT_ALPHA` | `0.75` | alpha of `TEXT_COMPONENT` on a component-like type label in Layers (E) |

**Frozen constants that lose their last reader.** `PAGES_SEARCH_DX` (28, E) and `RIGHT_PRESENT_FROM_RIGHT` (56, F) keep their values and their appendix cells until D–G land. Once the surface specs remove their last non-test reader, `cargo clippy --workspace --all-targets -- -D warnings` (CONTRIBUTING.md:29) flags them as dead code (a test that reads one does not count). Spec A's rule: delete the constant, no `#[allow(dead_code)]`, in the PR that removes its last reader, together with its appendix cell here (D-1 in (g) reports a stale cell). `RIGHT_PRESENT_CHEVRON_W` is still read and stays.

Unchanged scale constants that already sit on the Untitled scale (listed in the appendix with their values):
`FONT_SIZE_TITLE` 20 (text-xl), `FONT_SIZE_CANVAS` 9 (canvas exception, not a kit size), `UI_FONT_WEIGHT_MEDIUM` 500
(Medium), `RADIUS_MARK` 2 (radius-xxs), `INSET_SEGMENT` 2 (spacing-xxs), the spacing ladder `SPACE_FACT` 4
(spacing-xs), `SPACE_ROW` 4, `SPACE_INLINE` 8 (spacing-md), `SPACE_GROUP` 12 (spacing-lg), `SPACE_BLOCK` 16
(spacing-xl), `SPACE_SECTION` 24 (spacing-3xl), `SPACE_REGION` 48 (spacing-6xl), the insets `INSET_CONTROL` 8,
`INSET_FRAME` 8, `INSET_BUTTON` 12, `INSET_PANEL` 16, `INSET_POPOVER` 16, `INSET_MODAL` 24, and `TARGET_MIN` 24
(WCAG 2.5.8 floor, not a kit size).

Token-store scale re-values (spec B), one `ui ds change-token <path> --value <v> --reason "<reason>" --dir .` each (reasons in Studio's wording, R14):

```tsv
typography.font-size-caption	12px	12px caption and label size (R5)
typography.font-size-body	12px	12px row and body size (R5)
typography.font-size-title	14px	14px Home and dialog text (R5)
typography.font-size-heading	14px	14px panel title (R5)
radius.radius-xs	4px	4px radius step
radius.radius-sm	6px	6px radius step
radius.radius-md	8px	8px control radius (R5)
radius.radius-lg	10px	10px radius step
radius.radius-xl	12px	12px card radius
radius.radius-pill	9999px	full pill radius
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
`BADGE_LG` 28) and icon hit boxes (→ `TARGET_MIN` 24), so a single rename would silently resize chips.
Tabs and segmented strips keep today's height (R10.1); a control inside a tab bar matches its tabs, so the
"Sessions" ghost button (`agent_open_flow.rs:418-419`) stays 24 beside the 24 session tabs (designer inference from R10.1 and R1). They stay
as legacy names with their values; each surface spec (D–K) re-points the sites it owns, and spec B adds a
reference-count ratchet (spec B rule 11) so the count only falls. When it reaches 0, the constant is deleted,
except `CONTROL_DENSE`, which stays as the base of `RIGHT_TABS_H` (R10.1).

No colour constant is renamed; 5 dead colour constants are deleted and 77 are added (table (a)).

### `scripts/design_os_self_evolve.py` (G01, colour parity)

| Edit | Before (`8dfaee93`; the script is unchanged through #1106, #1108 and v0.7.0) | After |
|---|---|---|
| `BANNED_PURPLES` (:109) | `#9747ff`, `#8755e9`, `#c4a1ff`, `#a855f7` | add `#a259ff`, `#c792ea`, `#7f56d9`, `#9e77ed`, `#b692f6`, `#6941c6` (kit brand purples and the HEAD syntax keyword) |
| `CORE_RUST_CONSTANTS` (:111-134) | 22 constants | add `BG_SELECTED`, `BG`, `TEXT`, `TEXT_TERTIARY`, `BG_BRAND_HOVER`, `BG_BRAND_PRESSED`, `TEXT_ON_BRAND`, `FOCUS_RING`, `BORDER_SELECTED`, `BG_TEXT_SELECTION`, `TEXT_BRAND`, `BG_MENU`, `BG_ERROR_SOLID` (35) |
| `RUST_TO_DESIGN_TOKEN_MAP` (:137-158) | `BG_PRESSED`→`bg-active`, `DEV_READY`→`success` (collides with `SUCCESS`→`success`) | `BG_PRESSED`→`secondary-active`, `BG_SELECTED`→`bg-active`, `BG`→`canvas`, `TEXT`→`bg-foreground`, `TEXT_TERTIARY`→`muted-foreground`, `BG_BRAND_HOVER`→`primary-hover`, `BG_BRAND_PRESSED`→`primary-active`, `TEXT_ON_BRAND`→`primary-foreground`, `FOCUS_RING`→`ring`, `BORDER_SELECTED`→`border-focus`, `BG_TEXT_SELECTION`→`accent`, `TEXT_BRAND`→`accent-foreground`, `BG_MENU`→`popover`, `BG_ERROR_SOLID`→`destructive`; `DEV_READY` row removed |
| `EXPECTED_STANDALONE_HEX` (:160-163) | — | add `DEV_READY: "#14ae5c"` (Dev overlay keeps Figma parity) |
| Purple Ban hue scan (new function `purple_band_constants`, new name; called from G01) | exact-hex check of `BANNED_PURPLES` on the 35 core constants and the store only | every `Color32` / `[Color32; N]` constant in `theme.rs` except `BOARD_*`: any 6-digit hex literal with hue 255–300° and HSL saturation > 0.25 is a Purple Ban violation and fails G01 (HSL, not HSV: `#D5C3FF` is S 1.00 in HSL and 0.24 in HSV). Red-first proof in spec A section 10 test 6 |
| G04 `check_ui_ds_a11y` (:365) | blocking: exit 1 when `ui ds a11y` finds a pair below AA | report-only: runs `ui ds a11y` (default pairs and `--pairs` from `design/a11y-chrome-pairs.txt`), prints the counts as "supplementary" and always returns `passed=True`; contrast acceptance is C-1 in (f) |

Every mapped constant stays a plain `rgb(0x……)` literal (the parser at :223-225 reads literals only), so
`BG_SELECTED` becomes a literal instead of `= BG_PRESSED`.

### `scripts/non_colour_token_parity.py` (non-colour parity)

`TOKEN_TO_RUST` after spec B:

| Token path | Rust constants |
|---|---|
| `radius.radius-sm` (6) | `RADIUS_SEGMENT` |
| `radius.radius-md` (8) | `RADIUS_CONTROL` |
| `radius.radius-xl` (12) | `RADIUS_XL` (new, G-4) |
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
nothing): `G01 ok = True`, "35/35 Rust theme.rs constants match design.tokens.json; 0 Purple Ban violations; 30
non-colour constants match" (with `RADIUS_XL`; unmapped, information only: `radius-lg`, `radius-pill`, `radius-xs`,
`ai-panel-width`, `font-size-title`). Without the edits it fails (map collision, `RADIUS_MARK` vs `radius-xs`, unguarded new
constants).

## (f) Contrast check C-1 (the one every spec cites)

`ui ds a11y` is not an acceptance gate: on a single-mode import it measured 0 pairs (false green, spec A section 2),
and on this store it checks role/foreground pairs, not where text actually sits. Specs cite **C-1** instead: the pairs
below, recomputed with WCAG 2.x relative luminance by the snippet under the tables. The snippet reads every name's
value from `theme.rs` (aliases followed), fails when the hex in a row differs from `theme.rs` or the name is missing,
and fails when a row does not parse (row-count guard), so the table and the code cannot drift. Translucent constants
are made opaque the way they render: `theme::rgba()` premultiplies with integer division and egui composites
source-over on `BG` `#0A0A0A` (C-1.46 `STATUS_INFO_BG` → `#0A1E2D`); a background written `NAME@RRGGBB` (C-1.40, C-1.72) is the constant composited over that solid instead, as straight alpha. The Rust test
`chrome_contrast_pairs` (spec A section 10) asserts the same rows. A spec that adds a text×surface pair adds a row here.
Ids C-1.25 and C-1.26 are retired: they were the placeholder pair, which is an exception, not a pass, and now sits in
the floor table as E-1.a and E-1.b. Acceptance: **70/70 C-1 rows pass and 4/4 exception floors hold, exit 0.**

| Id | Foreground | Background | Min | Use |
|---|---|---|---|---|
| C-1.1 | `TEXT` #FAFAFA | `BG` #0A0A0A | 4.5 | primary text on chrome |
| C-1.2 | `TEXT` #FAFAFA | `BG_SECONDARY` #171717 | 4.5 | text in cards |
| C-1.3 | `TEXT` #FAFAFA | `BG_SELECTED` #262626 | 4.5 | selected row (R8.2) |
| C-1.4 | `TEXT` #FAFAFA | `BG_PRESSED` #404040 | 4.5 | pressed control |
| C-1.5 | `TEXT_SECONDARY` #D4D4D4 | `BG` #0A0A0A | 4.5 | secondary text |
| C-1.6 | `TEXT_SECONDARY` #D4D4D4 | `BG_PRESSED` #404040 | 4.5 | pressed control |
| C-1.7 | `TEXT_TERTIARY` #A3A3A3 | `BG` #0A0A0A | 4.5 | captions, labels |
| C-1.8 | `TEXT_TERTIARY` #A3A3A3 | `BG_SECONDARY` #171717 | 4.5 | captions in cards |
| C-1.9 | `TEXT_TERTIARY` #A3A3A3 | `BG_SELECTED` #262626 | 4.5 | caption in a selected row |
| C-1.10 | `TEXT_BRAND` #F97316 | `BG` #0A0A0A | 4.5 | accent text (R8.3) |
| C-1.11 | `TEXT_BRAND` #F97316 | `BG_SECONDARY` #171717 | 4.5 | accent text on cards |
| C-1.12 | `TEXT_ERROR` #F87171 | `BG` #0A0A0A | 4.5 | error text; C: `Button/SecondaryDestructive` resting label and icon |
| C-1.13 | `TEXT_ERROR` #F87171 | `BG_SECONDARY` #171717 | 4.5 | error text in cards |
| C-1.14 | `TEXT_ON_BRAND` #0A0A0A | `BG_BRAND` #E36323 | 4.5 | primary button label (R4) |
| C-1.15 | `TEXT_ON_BRAND` #0A0A0A | `BG_BRAND_HOVER` #F97316 | 4.5 | primary hover |
| C-1.16 | `TEXT_ON_BRAND` #0A0A0A | `BG_BRAND_PRESSED` #EA580C | 4.5 | primary pressed |
| C-1.17 | `TEXT_WHITE` #FFFFFF | `BG_ERROR_SOLID` #DC2626 | 4.5 | destructive button |
| C-1.18 | `TEXT_WHITE` #FFFFFF | `BG_ERROR_SOLID_HOVER` #B91C1C | 4.5 | destructive hover (R8.7) |
| C-1.19 | `TEXT` #FAFAFA | `BG_PRIMARY_SOLID` #262626 | 4.5 | tooltip title |
| C-1.20 | `TEXT_SECONDARY` #D4D4D4 | `BG_PRIMARY_SOLID` #262626 | 4.5 | tooltip supporting text |
| C-1.21 | `UTILITY_GREEN_700` #86EFAC | `BG_SUCCESS_PRIMARY` #052E16 | 4.5 | success badge |
| C-1.22 | `UTILITY_YELLOW_700` #FDE047 | `BG_WARNING_PRIMARY` #422006 | 4.5 | warning badge |
| C-1.23 | `UTILITY_RED_700` #FCA5A5 | `BG_ERROR_PRIMARY` #450A0A | 4.5 | error badge |
| C-1.24 | `UTILITY_NEUTRAL_700` #D4D4D4 | `UTILITY_NEUTRAL_50` #171717 | 4.5 | count badge |
| C-1.27 | `ICON_TERTIARY` #A3A3A3 | `BG_SELECTED` #262626 | 3.0 | icon, non-text (WCAG 1.4.11) |
| C-1.28 | `ICON_QUATERNARY` #737373 | `BG_SECONDARY` #171717 | 3.0 | quiet icon, non-text |
| C-1.29 | `FOCUS_RING` #F97316 | `BG` #0A0A0A | 3.0 | focus ring, non-text |
| C-1.30 | `BORDER_SELECTED` #FB923C | `BG_SECONDARY` #171717 | 3.0 | focused outline on a card, non-text |
| C-1.31 | `UTILITY_BLUE_600` #60A5FA | `UTILITY_BLUE_50` #172554 | 4.5 | Home tile glyph, design file (R10.2) |
| C-1.32 | `UTILITY_AMBER_600` #FBBF24 | `UTILITY_AMBER_50` #451A03 | 4.5 | Home tile glyph, board (R10.2) |
| C-1.33 | `UTILITY_GREEN_600` #4ADE80 | `UTILITY_GREEN_50` #052E16 | 4.5 | Home tile glyph, deck (R10.2) |
| C-1.34 | `UTILITY_ORANGE_600` #FB923C | `UTILITY_ORANGE_50` #431407 | 4.5 | Home tile glyph, spec bundle (R10.2) |
| C-1.35 | `TEXT_ON_BRAND` #0A0A0A | `DEV_READY` #14AE5C | 4.5 | Dev toggle and tab badge label (R10.4) |
| C-1.36 | `DEV_READY` #14AE5C | `BG` #0A0A0A | 4.5 | Dev accent text, Plugins "Ready" (R10.4) |
| C-1.37 | `BORDER_SELECTED` #FB923C | `BG` #0A0A0A | 3.0 | selected Home card border on the page (R10.3); focused field outline on the `bg-primary` field fill |
| C-1.38 | `BORDER_SELECTED` #FB923C | `HOME_THUMB_BG` #262626 | 3.0 | selected Home card border on its thumbnail (R10.3) |
| C-1.39 | `TEXT_SECONDARY` #D4D4D4 | `BG_SECONDARY_HOVER` #262626 | 4.5 | H: secondary text on a hovered card or field |
| C-1.40 | `TEXT_WARNING` #FACC15 | `BG_OVERLAY@FFFFFF` #676767 | 3.0 | H: the starred star glyph on its backdrop over a white thumbnail, non-text (3.69:1); #676767 is `BG_OVERLAY` rgba(0x262626, 0xB3) over `#FFFFFF`, straight alpha (egui's premultiplied result is #666666, 3.75:1, so this row is the conservative one). Replaces the old text row: only the glyph sits on the backdrop, never text |
| C-1.41 | `AGENT_LABEL_TEXT` #0A0A0A | `AGENT_1` #FFA629 | 4.5 | I: agent label on its presence colour |
| C-1.42 | `AGENT_LABEL_TEXT` #0A0A0A | `AGENT_2` #14AE5C | 4.5 | I: agent label on its presence colour |
| C-1.43 | `AGENT_LABEL_TEXT` #0A0A0A | `AGENT_3` #EE46BC | 4.5 | I: agent label on its presence colour |
| C-1.44 | `AGENT_LABEL_TEXT` #0A0A0A | `AGENT_4` #38BDF8 | 4.5 | I: agent label on its presence colour |
| C-1.45 | `TEXT_QUATERNARY` #A3A3A3 | `BG_PRIMARY` #0A0A0A | 4.5 | I: non-current session tab text |
| C-1.46 | `STATUS_INFO_TEXT` #0D99FF | `STATUS_INFO_BG` #0A1E2D | 4.5 | I: Working badge; #0A1E2D is rgba(0x0D99FF, 0x24) made opaque on `BG` #0A0A0A (= `BG_PRIMARY`) |
| C-1.47 | `PROVIDER_CLAUDE` #D97757 | `BG_PRIMARY` #0A0A0A | 3.0 | I: provider mark on the tab bar, non-text |
| C-1.48 | `PROVIDER_CODEX` #FAFAFA | `BG_PRIMARY` #0A0A0A | 3.0 | I: provider mark on the tab bar, non-text |
| C-1.49 | `PROVIDER_GROK` #E5E5E5 | `BG_PRIMARY` #0A0A0A | 3.0 | I: provider mark on the tab bar, non-text |
| C-1.50 | `PROVIDER_GEMINI` #60A5FA | `BG_PRIMARY` #0A0A0A | 3.0 | I: provider mark on the tab bar, non-text |
| C-1.51 | `ICON_TERTIARY` #A3A3A3 | `BG_PRIMARY` #0A0A0A | 3.0 | I: tab-bar icon, non-text |
| C-1.52 | `TEXT_SECONDARY_HOVER` #E5E5E5 | `BG_MENU_HOVER` #171717 | 4.5 | J: hovered menu item label |
| C-1.53 | `TEXT_QUATERNARY` #A3A3A3 | `BG_MENU` #0A0A0A | 4.5 | J: menu shortcut (`Menu/Shortcut`) |
| C-1.54 | `TEXT` #FAFAFA | `BG_PRIMARY_ALT` #171717 | 4.5 | J: toast text, selected segment |
| C-1.55 | `TEXT_TERTIARY` #A3A3A3 | `BG_MENU` #0A0A0A | 4.5 | J: caption in a menu or popover |
| C-1.56 | `TEXT_ON_LIGHT_CANVAS` #0A0A0A | `CANVAS_COMPONENT` #06B6D4 | 4.5 | K: variants badge label on the component cyan |
| C-1.57 | `TEXT_SECONDARY` #D4D4D4 | `BG_MENU` #0A0A0A | 4.5 | K: secondary text in a dialog |
| C-1.58 | `TEXT_TERTIARY` #A3A3A3 | `BG_SECONDARY_HOVER` #262626 | 4.5 | K: caption in a hovered row or card |
| C-1.59 | `WARNING` #EAB308 | `BG_MENU` #0A0A0A | 3.0 | K: status icon in a dialog, non-text |
| C-1.60 | `DANGER` #EF4444 | `BG_MENU` #0A0A0A | 3.0 | K: status icon in a dialog, non-text |
| C-1.61 | `INFO` #0D99FF | `BG_MENU` #0A0A0A | 3.0 | K: status icon in a dialog, non-text |
| C-1.62 | `SUCCESS` #22C55E | `BG_MENU` #0A0A0A | 3.0 | K: status icon in a dialog, non-text |
| C-1.63 | `TEXT_ON_BRAND` #0A0A0A | `DEV_READY_HOVER` #30B870 | 4.5 | D, G: Dev toggle label on hover (7.75:1) |
| C-1.64 | `READER_LIGHT_TEXT_SUBTLE` #525252 | `READER_LIGHT_SURFACE_2` #F5F5F5 | 4.5 | G: the lowest light-reader text pair (7.17:1); the two kit light dots under 3:1 (`READER_LIGHT_DOT_WARN` 2.94, `READER_LIGHT_DOT_NEUTRAL` 2.52 on a white sheet) are not rows: they are flagged in spec G, not rulings |
| C-1.65 | `TEXT_ERROR_HOVER` #FCA5A5 | `BG_ERROR_PRIMARY` #450A0A | 4.5 | C: `Button/SecondaryDestructive` hover and pressed label and icon on the kit tint (8.51:1); the resting label and icon are C-1.12 (`TEXT_ERROR` on `BG`) |
| C-1.66 | `BORDER_ERROR` #F87171 | `BG_ERROR_PRIMARY` #450A0A | 3.0 | C: secondary destructive pressed border, non-text (5.84:1) |
| C-1.67 | `BORDER_ERROR_SUBTLE` #EF4444 | `BG` #0A0A0A | 3.0 | C: secondary destructive resting border, non-text (5.26:1) |
| C-1.68 | `BORDER_ERROR_SUBTLE` #EF4444 | `BG_ERROR_PRIMARY` #450A0A | 3.0 | C: secondary destructive hover border on the tint, non-text (4.29:1) |
| C-1.69 | `FOCUS_RING_ERROR` #EF4444 | `BG` #0A0A0A | 3.0 | C: destructive focus ring, non-text (5.26:1) |
| C-1.70 | `TEXT_ERROR` #F87171 | `BG_SECONDARY` #171717 | 4.5 | H: "File not found" in a file card subtitle (6.48:1); C-1.13 is the same pair for error text in cards |
| C-1.71 | `TEXT_ERROR` #F87171 | `BG_SELECTED` #262626 | 4.5 | H: the same label in a selected list row (5.47:1) |
| C-1.72 | `ICON_SECONDARY` #D4D4D4 | `BG_OVERLAY@FFFFFF` #676767 | 3.0 | H: the unstarred star glyph on its backdrop over a white thumbnail, non-text (3.82:1) |

C-1.39–C-1.62 are the pairs specs H–K paint (W4 round); C-1.63–C-1.64 come from the D–G round's new names and C-1.65–C-1.69 from spec C's secondary-destructive button (controller ruling in plan.md, C/M rulings: its hover follows the kit tint `BG_ERROR_PRIMARY` with the `TEXT_ERROR_HOVER` label, not Red-700, which governs only the solid `Button/Destructive`). C-1.40 and C-1.70–C-1.72 come from spec H (the star rows replace the old text row C-1.40; the `TEXT_ERROR` pairs on cards and selected rows). Ids C-1.73 and up are reserved for the pairs the D–G round still sends (its D-P, E-P, F-P and G-P rows).

**Exception floors** (owner and controller rulings, table "Named exceptions" below). The snippet checks that each
value is exactly the named hex in `theme.rs` and that its ratio does not fall under the floor, prints `EXC`, and never
counts these rows as passes.

| Id | Foreground | Background | Floor | Use |
|---|---|---|---|---|
| E-1.a | `TEXT_PLACEHOLDER` #737373 | `BG` #0A0A0A | 3.0 | placeholder in a field on `bg-primary` (the kit Input fill, ruling C-F); R8.1 |
| E-1.b | `TEXT_PLACEHOLDER` #737373 | `BG_SECONDARY` #171717 | 3.0 | placeholder in a field inside a `bg-secondary` card; R8.1 |
| E-2.a | `BORDER_STRONG` #404040 | `BG` #0A0A0A | 1.7 | 1 px resting control boundary on `bg-primary`; R10.7 and ruling C-F |
| E-2.b | `BORDER_STRONG` #404040 | `BG_SECONDARY` #171717 | 1.7 | 1 px resting control boundary on `bg-secondary`; R10.7 and ruling C-F |

```bash
# from this folder; THEME = the theme.rs to check (after spec A: ../../../crates/designos-app/src/theme.rs)
python3 - token-contract.md "$THEME" <<'PY'
import re, sys
md, theme = open(sys.argv[1]).read(), open(sys.argv[2]).read()
defs = dict(re.findall(r"^pub const ([A-Z0-9_]+): Color32 = (.+?);", theme, re.M))
def rgba_of(name):  # (hex, alpha) of a theme.rs constant, aliases followed
    e = defs[name]
    if m := re.fullmatch(r"rgb\(0x([0-9A-Fa-f]{6})\)", e): return m[1].upper(), 255
    if m := re.fullmatch(r"rgba\(0x([0-9A-Fa-f]{6}), 0x([0-9A-Fa-f]{2})\)", e): return m[1].upper(), int(m[2], 16)
    if re.fullmatch(r"[A-Z0-9_]+", e): return rgba_of(e)
    raise ValueError(f"{name} = {e}")
def opaque(name):  # theme::rgba premultiplies with integer division; egui composites source-over on BG
    if "@" in name:  # NAME@RRGGBB: the constant over a solid that is not BG, straight alpha (one level lighter than egui's premultiplied result, so conservative)
        base, under = name.split("@"); h, a = rgba_of(base)
        return "".join(f"{round(int(h[i:i + 2], 16) * a / 255 + int(under[i:i + 2], 16) * (255 - a) / 255):02X}" for i in (0, 2, 4))
    h, a = rgba_of(name)
    if a == 255: return h
    bg = rgba_of("BG")[0]
    return "".join(f"{round(int(h[i:i + 2], 16) * a // 255 + int(bg[i:i + 2], 16) * (255 - a) / 255):02X}" for i in (0, 2, 4))
def lum(h):
    c = [int(h[i:i + 2], 16) / 255 for i in (0, 2, 4)]
    c = [x / 12.92 if x <= 0.04045 else ((x + 0.055) / 1.055) ** 2.4 for x in c]
    return 0.2126 * c[0] + 0.7152 * c[1] + 0.0722 * c[2]
rows = re.findall(r"^\| ((?:C-1|E-[12])\.\w+) \| `(\w+)` #(\w{6}) \| `(\w+(?:@\w{6})?)` #(\w{6}) \| ([\d.]+) \|", md, re.M)
want = len(re.findall(r"^\| (?:C-1|E-[12])\.", md, re.M))
bad, n_pass, n_floor = int(len(rows) != want), 0, 0
if bad: print(f"row-count guard: parsed {len(rows)} of {want} rows")
for rid, fg, fh, bgn, bh, mn in rows:
    try: tf, tb = opaque(fg), opaque(bgn)
    except (KeyError, ValueError) as e: print(f"{rid:7} not in theme.rs or not a literal: {e}"); bad += 1; continue
    if (tf, tb) != (fh.upper(), bh.upper()):
        print(f"{rid:7} table #{fh}/#{bh} but theme.rs #{tf}/#{tb}"); bad += 1; continue
    hi, lo = sorted((lum(tf), lum(tb)), reverse=True)
    r = (hi + 0.05) / (lo + 0.05); ok = r >= float(mn); bad += not ok
    floor = rid.startswith("E-"); n_floor += ok and floor; n_pass += ok and not floor
    print(f"{rid:7} {r:5.2f}:1 (min {mn}) {('EXC floor held' if floor else 'ok') if ok else 'FAIL'}")
n_c = sum(r[0].startswith("C-1") for r in rows)
print(f"C-1: {n_pass}/{n_c} pass, {n_floor}/{len(rows) - n_c} exception floors held, {bad} failure(s)")
sys.exit(1 if bad else 0)
PY
```

**Named exceptions** (rulings; their floors are E-1.a/b and E-2.a/b above, never C-1 passes):

| Id | Foreground | Background | Ratio | Rule it breaks | Scope (only here) | Ruling |
|---|---|---|---|---|---|---|
| E-1 | `TEXT_PLACEHOLDER` #737373 | `BG` #0A0A0A / `BG_SECONDARY` #171717 | 4.18:1 / 3.78:1 | AA 4.5:1 for text | placeholder text inside a text field or select; the field sits on `bg-primary` or `bg-secondary`, never on `BG_TERTIARY` (3.19:1) or `BG_PRESSED` (2.19:1) | R8.1 (owner) |
| E-2 | `BORDER_STRONG` #404040, 1 px, kit `border-primary` | `BG` #0A0A0A / `BG_SECONDARY` #171717 | 1.91:1 / 1.73:1 | 3:1 for non-text UI components (WCAG 1.4.11) | the resting boundary of exactly these controls: unchecked checkbox and radio (owner R10.7), text field and select, secondary button (controller ruling C-F, from R2 and R10.7; the owner may reverse it); only on `bg-primary` or `bg-secondary`; never on `BG_TERTIARY` (1.46:1) or `BG_PRESSED` (1.00:1, same value) | R10.7 (owner), C-F (controller) |

## (e) Unchanged because they are canvas, document, terminal or board

- **Canvas overlays (controller default, Figma parity):** `BG_SELECTED_AREA`, `CANVAS_*`, `GUIDE_*`, `PEN_POINT`,
  `RULER_TEXT/TICK/SELECTION_FILL/GUIDE`, `PIXEL_GRID_*`, `TEXT_SELECTION`, `FIND_CANVAS_MATCH*`, `ANNOTATION_*`,
  `MEASURE`, `COMMENT_PIN_RESOLVED`, `PROTO_BLUE`, `DROP_INDICATOR`, `CHECKER_A/B` (rows in (a)).
- **Dev overlay:** `DEV_READY/COMPLETED/CHANGED`, `BOX_*`.
- **Studio-only roles with no Untitled role:** `INFO`, `STATUS_INFO_*`, `FIND_HIGHLIGHT*`, `COMPONENT`,
  `TEXT_COMPONENT`, `AGENT_1..4`, `AGENT_COLORS`, `AVATAR_COLORS`.
- **Document defaults (`u32`, content):** `DEFAULT_PAGE_BG` :616 `0x5A5A5A`, `DEFAULT_STROKE` :617 `0x000000`,
  `DEFAULT_FRAME_FILL` :618 `0xFFFFFF`, `DEFAULT_SHAPE_FILL` :619 `0xD9D9D9`.
- **Board content (`u32`):** `BOARD_PALETTE` :748, `BOARD_INK` :750, `BOARD_BG` :752 `0xF4F4F4`; `BOARD_DOT` (Color32) keeps.
- **Terminal:** the ANSI palette lives in `terminal.rs`, not `theme.rs`, and is content. The chrome around the
  terminal uses the chrome constants above. `terminal.rs:10727-10745` (`8dfaee93`), the test
  `theme_has_info_success_warning_in_token_parity`, pins `INFO`, `SUCCESS`, `WARNING` in `theme.rs` (`:10728-10730`) and
  `colors.info/success/warning` in the store (`:10743-10745`); the `SUCCESS` and `WARNING` pins move to `#22C55E` / `#EAB308` in both places with spec A.
- **Strings:** `FAMILY_STRONG`, `UI_FONT_FAMILY` (`"Inter"`, the kit typeface), `FAMILY_MEDIUM`, `UI_MONO_FAMILY`
  (`"Roboto Mono"`), `APP_TITLE`.
- **Layout (R1):** every width, x position and panel geometry constant in the appendix.

## (g) Drift check D-1 (re-anchoring)

Run it against the HEAD `theme.rs` before editing code: it proves that every line number, HEAD-value cell and appendix
cell in this file still describes `theme.rs`, so a PR that starts after more commits learns what moved. It fails (exit 1)
when a constant named in (a), (c) or the appendix is missing or sits on another line, when a HEAD-value cell differs
from the literal, when a numeric constant is in no table, or when a constant #1106 deleted is back or an #1108 one is gone.
It reads `theme.rs` only (the contract tables are the claim, the code is the fact).

```bash
# from this folder; THEME = the HEAD theme.rs (before spec A): ../../../crates/designos-app/src/theme.rs
python3 - token-contract.md "$THEME" <<'PY'
import re, sys
md, lines = open(sys.argv[1]).read(), open(sys.argv[2]).read().split("\n")
const = {}  # name -> (line, type, expression)
for i, l in enumerate(lines, 1):
    if m := re.match(r"^pub const ([A-Z0-9_]+): (.+?) = (.+?);", l): const[m[1]] = (i, m[2], m[3])
def sec(a, b): return md[md.index(a):md.index(b, md.index(a))]
bad, n_col, n_scale, n_app = [], 0, 0, 0
for row in sec("## (a) Colour constants", "### Shared values").splitlines():
    if m := re.match(r"\| (\d+) \| `(\w+)` :(\d+) \| `(.+?)` \|", row):
        n, name, line, head = m.groups()
        if name not in const: bad.append(f"(a) #{n} {name}: not in theme.rs"); continue
        if const[name][0] != int(line): bad.append(f"(a) #{n} {name}: cited :{line}, theme.rs :{const[name][0]}")
        if const[name][2].replace(" ", "") != head.replace(" ", ""): bad.append(f"(a) #{n} {name}: HEAD cell {head}, theme.rs {const[name][2]}")
        n_col += 1
cset = set()
for row in sec("## (c) Scale constants", "## (d)").splitlines():
    if m := re.match(r"\| `(\w+)` :(\d+) \| `([^`]+)` \|", row):
        name, line, head = m.groups(); cset.add(name)
        if name not in const: bad.append(f"(c) {name}: not in theme.rs"); continue
        if const[name][0] != int(line): bad.append(f"(c) {name}: cited :{line}, theme.rs :{const[name][0]}")
        if const[name][2] != head: bad.append(f"(c) {name}: HEAD cell {head}, theme.rs {const[name][2]}")
        n_scale += 1
app = md[md.index("## Appendix"):]
cells = {n: int(l) for n, l in re.findall(r"`([A-Z0-9_]+)` :(\d+) = ", app)}
for n, l in cells.items():
    if n not in const or const[n][0] != l: bad.append(f"appendix {n} :{l}: theme.rs {const.get(n, ('missing',))[0]}")
    n_app += 1
numeric = {n for n, (_, t, _) in const.items() if re.fullmatch(r"f32|f64|u8|usize|\(f32, ?f32\)|\[f32; ?\d\]", t)}
for n in sorted(numeric - set(cells) - cset): bad.append(f"numeric {n}: in no table")
for n in ("RAIL_SEPARATOR_INSET", "RAIL_ICON_CENTER_Y", "RAIL_LABEL_CENTER_Y"):
    if n in const: bad.append(f"{n}: #1106 deleted it but theme.rs has it")
for n in ("PAGES_SEPARATOR_HIT", "PAGES_SEPARATOR_HIT_HALF", "PAGES_SEPARATOR_HIGHLIGHT_STROKE", "PAGES_SEPARATOR_STEP", "PAGES_SEPARATOR_SHIFT_MULT", "LAYERS_MIN_HEIGHT", "LEFT_DEV_READY_SECTION_STEP", "LAYER_FILTER_ROW_HEIGHT"):
    if n not in cells and n not in cset: bad.append(f"{n}: #1108 constant missing from the tables")
print("\n".join(bad))
print(f"D-1: {n_col} colour rows, {n_scale} scale rows, {n_app} appendix cells, {len(numeric)} numeric constants, {len(bad)} drift line(s)")
sys.exit(1 if bad else 0)
PY
```

## Appendix — numeric constants that keep their value

Value after resolving aliases at `8dfaee93` (origin/main; `theme.rs` is identical at `d0555c82`); the name keeps its expression too (no edit). 302 constants (327 numeric minus the 25 rows of (c)); every name and line number is checked by D-1 in (g). Since `ec47da26`: #1106 removed `RAIL_SEPARATOR_INSET`, `RAIL_ICON_CENTER_Y` and `RAIL_LABEL_CENTER_Y` (not in this table: D-1 fails if one comes back) and set `RAIL_WIDTH` to 48; `TOAST_REVEAL_SECONDS` is new; #1108 added `PAGES_SEPARATOR_HIT` 6, `PAGES_SEPARATOR_HIT_HALF` 3, `PAGES_SEPARATOR_HIGHLIGHT_STROKE` 2, `PAGES_SEPARATOR_STEP` 8, `PAGES_SEPARATOR_SHIFT_MULT` 5, `LAYERS_MIN_HEIGHT` 120, `LEFT_DEV_READY_SECTION_STEP` 54 and `LAYER_FILTER_ROW_HEIGHT` = `CONTROL_TOOL` 32 (B's rename makes it `= CONTROL_XS`, value kept); all eight are in the table below with today's values. The separator's drag logic lives in `panels/left_sep.rs` (new file) and reads these constants; it declares no theme value of its own. Controller ruling C-R gives the separator (and every drag divider) a 24 px hit band without touching these values: `PAGES_SEPARATOR_HIT` 6 stays the core press band and the band is the new `DRAG_DIVIDER_HIT` of spec D (rules after (a)). `PAGE_ROW_HEIGHT`, `PANEL_ACTION_BUTTON` (24) and `PAGES_SEARCH_DX` (28) keep their rows: B changes neither of the last two (B rule 6).

| | | | |
|---|---|---|---|
| `LIGHT_BG_THRESHOLD` :113 = 0.5 | `FONT_SIZE_CANVAS` :119 = 9 | `FONT_SIZE_TITLE` :122 = 20 | `CHEVRON_SMALL` :126 = 10 |
| `FONT_SIZE_LABEL_CANVAS` :127 = 11 | `UI_FONT_WEIGHT_MEDIUM` :133 = 500 | `UI_MONO_WEIGHT` :137 = 400 | `UI_FONT_OPSZ` :139 = 14 |
| `RADIUS_MARK` :190 = 2 | `SPACE_FACT` :228 = 4 | `SPACE_ROW` :230 = 4 | `SPACE_INLINE` :232 = 8 |
| `SPACE_GROUP` :234 = 12 | `SPACE_BLOCK` :237 = 16 | `SPACE_SECTION` :240 = 24 | `SPACE_REGION` :243 = 48 |
| `INSET_SEGMENT` :246 = 2 | `INSET_CONTROL` :247 = 8 | `INSET_FRAME` :248 = 8 | `INSET_BUTTON` :249 = 12 |
| `INSET_PANEL` :250 = 16 | `INSET_POPOVER` :251 = 16 | `INSET_MODAL` :252 = 24 | `TARGET_MIN` :260 = 24 |
| `RAIL_WIDTH` :263 = 48 | `BORDER_WIDTH` :264 = 1 | `RAIL_MENU_BUTTON_X` :265 = 12 | `RAIL_MENU_BUTTON_Y` :266 = 8 |
| `RAIL_MENU_BUTTON_SIZE` :267 = 32 | `RAIL_ITEM_SIZE` :268 = 56 | `RAIL_FIRST_Y` :269 = 56 | `RAIL_GROUP_GAP` :270 = 16 |
| `RAIL_ICON_SIZE` :271 = 20 | `RAIL_SEPARATOR_Y` :273 = 48 | `RAIL_TILE` :274 = 32 | `LEFT_PANEL_WIDTH` :276 = 240 |
| `LEFT_HEADER_HEIGHT` :277 = 65 | `LEFT_FILE_NAME_Y` :278 = 16 | `LEFT_SUBLABEL_Y` :279 = 40 | `LEFT_ACTION_Y` :280 = 12 |
| `LEFT_SIDEBAR_BTN_RIGHT_PAD` :281 = 12 | `PAGES_SEARCH_DX` :282 = 28 | `PANEL_TITLE_ROW_HEIGHT` :283 = 40 | `PANEL_ACTION_BUTTON` :284 = 24 |
| `PANEL_ACTION_RIGHT_PAD` :285 = 8 | `PANEL_PADDING_X` :286 = 16 | `PAGE_ROW_INSET` :288 = 8 | `PAGE_ROW_TEXT_PAD` :289 = 8 |
| `PAGES_MAX_VISIBLE_ROWS` :290 = 5 | `PAGES_SEPARATOR_HIT` :291 = 6 | `PAGES_SEPARATOR_HIT_HALF` :292 = 3 | `PAGES_SEPARATOR_HIGHLIGHT_STROKE` :293 = 2 |
| `PAGES_SEPARATOR_STEP` :294 = 8 | `PAGES_SEPARATOR_SHIFT_MULT` :295 = 5 | `LAYERS_MIN_HEIGHT` :296 = 120 | `LEFT_DEV_READY_SECTION_STEP` :297 = 54 |
| `LAYER_FILTER_ROW_HEIGHT` :298 = 32 | `PAGE_MENU_WIDTH` :299 = 160 | `LAYER_HEADER_HEIGHT` :300 = 40 | `LAYER_ROW_HEIGHT` :301 = 32 |
| `LAYER_INDENT` :302 = 16 | `LAYER_CHEVRON_BOX` :303 = 24 | `LAYER_CHEVRON_PAD` :304 = 8 | `LAYER_ICON_SIZE` :305 = 16 |
| `LAYER_ICON_GAP` :306 = 0 | `LAYER_TOGGLE_SIZE` :307 = 24 | `LAYER_DROP_THICKNESS` :308 = 2 | `FIND_ROW_GAP` :312 = 8 |
| `FIND_TOGGLE_SIZE` :313 = 22 | `FIND_BUTTON_H` :314 = 24 | `FIND_STATUS_H` :315 = 32 | `FIND_EXCERPT_LEAD` :321 = 12 |
| `RIGHT_PANEL_WIDTH` :323 = 240 | `RIGHT_PRESENT_FROM_RIGHT` :325 = 56 | `PRESENT_ICON_W` :328 = 15 | `PRESENT_ICON_H` :330 = 16 |
| `PRESENT_ICON_STROKE` :332 = 1.2 | `RIGHT_BTN_Y` :333 = 8 | `RIGHT_BTN_SIZE` :334 = 32 | `RIGHT_PRESENT_CHEVRON_W` :335 = 16 |
| `RIGHT_TABS_Y` :336 = 48 | `RIGHT_TABS_X` :338 = 9 | `RIGHT_TAB_PAD_X` :339 = 8 | `RIGHT_ZOOM_FROM_RIGHT` :340 = 63 |
| `RIGHT_ZOOM_W` :341 = 55 | `RIGHT_CONTENT_TOP` :342 = 81 | `RIGHT_SELECTION_HEADER_Y` :343 = 93 | `RIGHT_NAME_FIELD_X` :344 = 9 |
| `RIGHT_NAME_FIELD_W` :345 = 136 | `RIGHT_NAME_ACTION_X` :346 = [153.0, 181.0, 209.0] | `SECTION_TITLE_HEIGHT` :347 = 40 | `SECTION_CONTENT_X` :348 = 17 |
| `SECTION_CONTENT_W` :349 = 184 | `SECTION_TRAILING_X` :350 = 209 | `FIELD_WIDTH_HALF` :352 = 88 | `FIELD_COL_X` :353 = [17.0, 113.0] |
| `FIELD_PREFIX_W` :354 = 24 | `FIELD_PREFIX_X` :356 = 8 | `FIELD_PREFIX_ICON` :358 = 16 | `FIELD_PREFIX_GLYPH` :361 = 12 |
| `FIELD_PLAIN_X` :363 = 8 | `FIELD_SUFFIX_GAP` :365 = 4 | `DISTRIBUTE_BTN_X` :366 = 209 | `SEGMENT_W` :371 = 29 |
| `SEGMENT_GROUP_GAP` :372 = 8 | `SWATCH_SIZE` :373 = 16 | `SWATCH_RADIUS` :374 = 2 | `SWATCH_ROW_X` :375 = 17 |
| `STYLE_ROW_H` :376 = 32 | `STYLE_SWATCH` :378 = 16 | `STYLE_SWATCH_CX` :379 = 16 | `STYLE_LABEL_X` :380 = 33 |
| `STYLES_LABEL_GAP` :382 = 8 | `STYLES_GROUP_GAP` :383 = 14 | `STYLES_BOTTOM_PAD` :384 = 13 | `STYLES_ROW_GAP` :386 = 9 |
| `PAGE_FIELD_DY` :388 = 4 | `PAGE_BOTTOM_PAD` :389 = 16 | `PAGE_SWATCH` :390 = 12 | `PAGE_SWATCH_CX` :391 = 12 |
| `PAGE_HEX_X` :392 = 24 | `HEX_FIELD_W` :394 = 72 | `PAGE_DIVIDER_X` :395 = 129 | `PAGE_OPACITY_PAD` :396 = 8 |
| `PAGE_PERCENT_W` :397 = 16 | `PAGE_PERCENT_RIGHT` :398 = 7 | `STYLE_INDENT` :400 = 12 | `OPACITY_FIELD_W` :401 = 48 |
| `TOOLBELT_HEIGHT` :403 = 48 | `TOOLBELT_BOTTOM_MARGIN` :404 = 12 | `TOOLBELT_PADDING` :405 = 8 | `TOOL_BUTTON` :406 = 32 |
| `TOOL_CHEVRON_W` :407 = 16 | `TOOL_CHEVRON_GAP` :408 = 0 | `TOOL_CHEVRON_ICON` :410 = 11 | `TOOL_CHEVRON_CENTER_DX` :412 = 11 |
| `TOOL_GROUP_GAP` :413 = 8 | `TOOL_ICON_SIZE` :414 = 20 | `MODE_SEGMENTS` :416 = 4 | `MODE_SWITCH_PAD` :417 = 2 |
| `MODE_SWITCH_W` :418 = 29.5 | `TOOLBAR_DIVIDER_GAP_BEFORE` :420 = 13 | `TOOLBAR_DIVIDER_GAP_AFTER` :421 = 8 | `MENU_WIDTH` :423 = 200 |
| `MENU_PADDING` :426 = 8 | `MENU_PILL_INSET` :428 = 8 | `MENU_CHECK_X` :431 = 12 | `MENU_CHECK_BOX` :432 = 16 |
| `MENU_ICON_X_CHECKED` :433 = 32 | `MENU_ICON_BOX` :434 = 24 | `MENU_LABEL_X_ICON` :435 = 60 | `MENU_LABEL_X_PLAIN` :436 = 16 |
| `MENU_LABEL_X_CHECK` :438 = 32 | `MENU_ICON_X_PLAIN` :440 = 12 | `MENU_ICON_LABEL_GAP` :441 = 4 | `MENU_SHORTCUT_PAD` :442 = 16 |
| `MENU_CHEVRON_BOX` :443 = 24 | `MENU_CHEVRON_PAD` :444 = 8 | `MENU_SEP_MARGIN` :445 = 8 | `MENU_LABEL_GAP` :447 = 16 |
| `MENU_ICON_SIZE` :449 = 16 | `MENU_CHEVRON_SIZE` :450 = 12 | `SUBMENU_OVERLAP` :452 = 0 | `TOOL_MENU_WIDTH` :454 = 232 |
| `TOOL_MENU_DX` :455 = 2.5 | `TOOL_MENU_OVERLAP` :456 = 4 | `MAIN_MENU_X` :458 = 12 | `MAIN_MENU_Y` :459 = 44 |
| `MAIN_MENU_WIDTH` :460 = 210 | `PALETTE_WIDTH` :462 = 320 | `PERMISSIONS_MODAL_WIDTH` :463 = 360 | `PALETTE_TOP` :465 = 64 |
| `TOAST_HEIGHT` :466 = 40 | `TOAST_BOTTOM` :467 = 72 | `TOAST_SECONDS` :468 = 4 | `TOAST_REVEAL_SECONDS` :470 = 10 |
| `COMMENT_PIN` :473 = 32 | `COMMENT_PIN_RING` :474 = 2 | `COMMENT_PIN_HIT_PAD` :475 = 2 | `COMMENT_AVATAR` :480 = 24 |
| `COMMENT_POP_W` :482 = 300 | `COMMENT_POP_GAP` :483 = 8 | `COMMENT_PAD` :484 = 12 | `COMMENT_HEADER_H` :485 = 36 |
| `COMMENT_BTN_H` :486 = 24 | `COMMENT_BOX_MIN_H` :487 = 56 | `COMMENT_BOX_MAX_H` :488 = 160 | `COMMENT_ICON_BTN` :489 = 20 |
| `COMMENT_PANEL_HEADER_H` :491 = 48 | `COMMENT_FILTER_H` :492 = 24 | `COMMENT_CARD_PAD` :493 = 12 | `HISTORY_ROW_H` :495 = 44 |
| `HISTORY_DAY_H` :496 = 28 | `HISTORY_BANNER_H` :497 = 40 | `CANVAS_OUTLINE_WIDTH` :500 = 1 | `CANVAS_HOVER_WIDTH` :501 = 1 |
| `CANVAS_HANDLE_SIZE` :502 = 7 | `CANVAS_HANDLE_BORDER` :503 = 1 | `CANVAS_ROTATE_ZONE` :504 = 12 | `CANVAS_HANDLE_HIT` :505 = 6 |
| `CANVAS_BADGE_OFFSET` :506 = 4 | `CANVAS_BADGE_PAD_X` :507 = 4 | `CANVAS_BADGE_HEIGHT` :508 = 16 | `CANVAS_BADGE_RADIUS` :509 = 2 |
| `CANVAS_LABEL_GAP` :510 = 4 | `CANVAS_LABEL_MAX_ZOOM_HIDE` :511 = 0.05 | `SNAP_THRESHOLD_PX` :512 = 5 | `GUIDE_WIDTH` :513 = 1 |
| `GUIDE_X_SIZE` :514 = 3 | `GUIDE_TEXT_PAD` :515 = 3 | `GUIDE_TEXT_HEIGHT` :516 = 14 | `PEN_POINT_SIZE` :517 = 4 |
| `PEN_LINE_WIDTH` :518 = 1.5 | `AL_INDICATOR_WIDTH` :520 = 2 | `VEDIT_POINT_SIZE` :522 = 6 | `VEDIT_HANDLE_RADIUS` :523 = 3 |
| `VEDIT_HIT` :524 = 6 | `VEDIT_SEGMENT_HIT` :525 = 5 | `DRAG_THRESHOLD_PX` :526 = 3 | `ZOOM_MIN` :527 = 0.02 |
| `ZOOM_MAX` :528 = 256 | `FIT_PADDING` :529 = 64 | `TEXT_CARET_WIDTH` :530 = 1 | `RULER_SIZE` :532 = 20 |
| `RULER_FONT_SIZE` :533 = 9 | `PIXEL_GRID_MIN_ZOOM` :545 = 8 | `FIND_CANVAS_MATCH_MAX` :556 = 600 | `NUDGE` :557 = 1 |
| `NUDGE_BIG` :558 = 10 | `PROTO_NOODLE_WIDTH` :595 = 2 | `PROTO_HOTSPOT_RADIUS` :596 = 6 | `PROTO_HOTSPOT_RING` :597 = 2 |
| `PROTO_HOTSPOT_HIT` :598 = 10 | `PROTO_FLAG_HEIGHT` :599 = 20 | `PROTO_FLAG_GAP` :600 = 4 | `PROTO_ROW_HEIGHT` :601 = 44 |
| `PROTO_POPOVER_WIDTH` :602 = 240 | `PRESENT_BAR_HEIGHT` :604 = 48 | `PRESENT_PADDING` :605 = 32 | `PRESENT_BEZEL` :607 = 10 |
| `PRESENT_MAX_SCALE` :608 = 4 | `DEFAULT_WINDOW` :611 = (1600.0, 1088.0) | `MIN_WINDOW` :612 = (900.0, 600.0) | `DEFAULT_SHAPE_SIZE` :614 = 100 |
| `DEFAULT_TEXT_SIZE` :615 = 16 | `AUTO_LAYOUT_SPACING` :620 = 10 | `AUTO_LAYOUT_PADDING` :621 = 10 | `PASTE_OFFSET` :622 = 10 |
| `HOME_SIDEBAR_W` :699 = 264 | `HOME_PAD` :700 = 32 | `HOME_HEADER_H` :701 = 64 | `HOME_CARD_W` :702 = 224 |
| `HOME_THUMB_H` :703 = 140 | `HOME_CARD_H` :705 = 198 | `HOME_CARD_GAP` :706 = 24 | `HOME_FOLDER_H` :707 = 48 |
| `HOME_LIST_ROW_H` :708 = 44 | `VAR_SIDEBAR_W` :713 = 240 | `VAR_BAR_H` :715 = 48 | `VAR_FLOAT_W` :717 = 642 |
| `VAR_FLOAT_H` :718 = 380 | `VAR_FLOAT_Y` :719 = 8 | `VAR_FLOAT_BAR_H` :720 = 40 | `VAR_HEAD_H` :721 = 40 |
| `VAR_ROW_H` :722 = 40 | `VAR_ROW_PITCH` :723 = 41 | `VAR_NAME_COL` :725 = 200 | `VAR_MODE_MIN` :726 = 140 |
| `VAR_MODE_MAX` :727 = 280 | `VAR_ADD_COL` :728 = 39 | `VAR_BOTTOM_H` :729 = 41 | `VAR_SCROLL_H` :730 = 11 |
| `VAR_SEARCH_W` :732 = 174 | `VAR_FILTER_W` :733 = 25 | `VAR_SIDE_ROW` :734 = 32 | `VAR_ICON_X` :736 = 16 |
| `VAR_NAME_X` :737 = 44 | `VAR_SWATCH` :738 = 16 | `VAR_HEX_X` :739 = 30 | `VAR_HEX_W` :740 = 146 |
| `VAR_OPACITY_W` :741 = 62 | `VAR_OPACITY_RIGHT` :742 = 74 | `VAR_FIELD_H` :743 = 24 | `VAR_POPOVER_W` :744 = 264 |
| `BOARD_DOT_SPACING` :755 = 24 | `BOARD_DOT_MIN_PX` :756 = 14 | `BOARD_MAGNET_RADIUS` :758 = 5 | `BOARD_MAGNET_GAP` :759 = 12 |
| `BOARD_MAGNET_HIT` :760 = 10 | `BOARD_SHAPE_W` :762 = 160 | `BOARD_SHAPE_H` :763 = 120 | `BOARD_STAMP_SIZE` :764 = 64 |
| `BOARD_STICKY_GAP` :765 = 24 | `BOARD_MARKER_WIDTH` :766 = 6 | `BOARD_HIGHLIGHTER_WIDTH` :767 = 24 | `BOARD_HIGHLIGHTER_ALPHA` :768 = 0.4 |
| `BOARD_BAR_H` :770 = 40 | `BOARD_BAR_PAD` :771 = 6 | `BOARD_BAR_GAP` :772 = 12 | `BOARD_BAR_BTN` :773 = 28 |
| `BOARD_SWATCH` :774 = 18 | `BOARD_BAR_SEP` :775 = 8 |  |  |

## Changelog (curator fix round, review `reviews/AB-contract-curator-review.md`; 2026-10-25, re-anchored and extended 2026-10-28)

One line per finding that touches this file; spec A's and B's changelogs carry the rest.

- F2: R10.1 in the header; the `RIGHT_TABS_H` row is held at `CONTROL_DENSE` 24 (cites `terminal.rs:5804`, `tabs.rs:1007`).
- F5: E-2 names every 1 px `border-primary` control boundary (unchecked checkbox and radio, text field and select, secondary button), bounded to bg-primary and bg-secondary, citing ruling C-F as one the owner may reverse; the soul and P2 say the same.
- F8: the input fill is `BG` (bg-primary) in (a) #3 and (b) `input`, as C-F rules.
- F13: C-1.25/26 are floor rows E-1.a/b, E-2.a/b are added, the snippet prints floors as EXC and never counts them as passes, and a row-count guard fails on a dropped row; acceptance now reads 70/70 and 4/4 (run 2026-10-28: exit 0).
- F14: "Shared values (generated)" after (a), regenerated 2026-10-28 with N1–N77.
- F15: re-anchored to `8dfaee93`; the appendix lists no #1106-deleted constant and holds the #1108 constants with today's values; new check D-1 in (g) proves it (116 colour rows, 25 scale rows, 302 appendix cells, exit 0 at `8dfaee93`; 328 drift lines, exit 1 on the theme before #1106).
- D–G review (a): the rail constants #1106 deleted are not in the appendix and D-1 fails if one returns; the eight #1108 constants are in it with today's values. (c) C-R: drag dividers keep their visible 1 px line and get a 24 px hit band (`DRAG_DIVIDER_HIT` 24.0, spec D declares it; `PAGES_SEPARATOR_HIT` 6 stays the core band); "Mixed" is `TEXT_TERTIARY`, E-1 stays placeholder-only.
- R14: the naming row separates Untitled role names (review aid, specs only) from shipped names (Studio's own); the tsv reasons in (b) and (c) are Studio wording; the name audit of the kit-ramp families is spec T's call.
- Tooltip ruling: cited as C-T (plan.md).
- D–G amendment (2026-10-28): N47–N73 (27 colour constants: `DEV_READY_HOVER`, `HUD_POINTER_FILL`, `PLAYGROUND_GRID_DOT`, `SWATCH_PLACEHOLDER`, `TYPE_PREVIEW_BASELINE`, the seven `CONTENT_HANDLE_*`/`CONTENT_MARK_*` names, the 15 `READER_LIGHT_*` names) with the values the specs gave; four numeric names and `DRAG_DIVIDER_HIT` in a new (c) table; C-1.63 (`DEV_READY_HOVER`, 7.75:1) and C-1.64 (`READER_LIGHT_TEXT_SUBTLE` on `READER_LIGHT_SURFACE_2`, 7.17:1); the two kit light dots under 3:1 are flagged, not ruled; the frozen constants `PAGES_SEARCH_DX` and `RIGHT_PRESENT_FROM_RIGHT` are deleted by the PR that removes their last reader.
- C amendment (2026-10-28): N74–N77 (`FOCUS_RING_ERROR`, `BORDER_ERROR_SUBTLE`, `BORDER_ERROR`, `TEXT_ERROR_HOVER`) and C-1.65–C-1.69 for the secondary-destructive button (kit tint hover, per the controller ruling); `C-1.12` also covers its resting label.
- H amendment (2026-10-28): C-1.40 is now the starred-star glyph over a white thumbnail (`TEXT_WARNING` on `BG_OVERLAY@FFFFFF`, 3.69:1; the snippet takes `NAME@RRGGBB` as a constant composited over a solid) with the unstarred pair as C-1.72 (3.82:1); C-1.70 and C-1.71 are `TEXT_ERROR` on `BG_SECONDARY` (6.48:1) and `BG_SELECTED` (5.47:1); `BORDER_ERROR_SUBTLE` and `TEXT_ERROR_HOVER` were already rows N75 and N77; 70 C-1 rows now.
- Error-state field ring (ruling, 2026-10-28): a field in its error state takes `FOCUS_RING_ERROR` (N74, `#EF4444`); every other focus ring stays `FOCUS_RING` `#F97316` (R4).
