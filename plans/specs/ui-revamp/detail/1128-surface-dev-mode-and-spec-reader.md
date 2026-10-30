---
id: detail-1128-surface-dev-mode-and-spec-reader
type: note
title: "G Dev mode (status, sections, code, playground, Compare changes, annotations) and the spec reader in Untitled UI dark"
updated: "2026-10-25"
status: draft
---

## Nguồn
Bản sao từ nhánh `docs/specs-untitled-ui-dark` (`docs/specs/ui-revamp-untitled-dark/1128-surface-dev-mode-and-spec-reader.md`), lấy ngày 25/10/2026 để pack tự đủ. Nguồn chuẩn chi tiết cho US tương ứng.


| Field | Value |
|---|---|
| Issue | #1128 (epic #1118, spec key G) |
| Size | M. Three days: 11 owned files, 52 literal colour sites (42 in `specs.rs`, 10 in `dev/`), the syntax palette (contract rows 89–97), the Dev accent `DEV_READY` in G's files (R10.4), R5 sizes on the Dev buttons, badges, toggle and Compare footer. The spec reader part is half a day of it and can ship separately (G2) because the reader is not reachable in the app today (section 2). |
| Depends on | A (colour constants, `specs/token-contract.md`, contrast check C-1, colour-literal gate, Purple Ban check), B (type, spacing, radius and control-size scale), C (widget anatomy and states, `specs/C-widget-kit-untitled-anatomy.md`) and L (layout-unchanged gate and baselines). |
| Unblocks | M (docs and screenshot refresh). D, E and F no longer wait on G: R10.4 fixed the Dev accent value (`DEV_READY` `#14AE5C`). |
| Touches | `crates/designos-app/src/dev/sections.rs` (`:16-34` status pill, `:36-42` primary button, `:45-99` status header, `:151-330` annotations and links, `:332-380` box model, `:386-419` code block, `:421-468` code section, `:485-683` variables and assets); `dev/highlight.rs` (`:33-45` token colours); `dev/list.rs` (`:40-77` tabs, `:81-168` Ready for dev list); `dev/compare.rs` (`:53-70` pane, `:83-195` Compare changes window); `dev/playground.rs` (`:315-335` toggle switch, `:451-577` playground section); `dev/annot.rs` (`:65-161` canvas callouts, colours unchanged); `dev/mod.rs`, `dev/color.rs`, `dev/data.rs`, `dev/preset.rs` (no colour or size change; listed for ownership); `crates/designos-app/src/specs.rs` (`:29-91` reader palette, `:2287-2641` left explorer, `:2720-2918` reader, `:2919-3776` right column and role views). Reads only: `theme.rs:554-582` (`SYNTAX_*`, `DEV_*`, `STATUS_INFO_*`, `BOX_*`, `ANNOTATION_*`, `MEASURE`, `CODE_LINE_HEIGHT`; A writes them). |

Code baseline: `origin/main` `205c72ef` (checked with `git show 205c72ef:<path>` and a `git archive` of that ref, no checkout). The spec was written on `ec47da26` and re-anchored to `35a65796` first. Between `35a65796` and `205c72ef`, of the files cited here only `panels/rail.rs` changed (the cited `:10-18` did not move) and `theme.rs` lost 3 lines after `:263` (#1106); every `theme.rs` cite here sits after it and was re-anchored. The section 2 census gives the same output at `35a65796` and `205c72ef` (exit 0 at both).

Drift after `205c72ef` (measured 2026-10-25 against `origin/main` `649da124`): none of G's owned files changed. Cites into other specs' files moved: `panels/left.rs` (#1108) `:93` → `:97`, `:97` → `:102`, `:160` → `:165`, `:289` → `:295`; `shell.rs` +4 lines after `:102` (#1108) and +4 more between `:1001` and `:1033` (#1148), +8 in all after `:1029`, so `:318-322` → `:322-326`, `:483` → `:487`, `:1509` → `:1517`, `:1512` → `:1520`, `:1578` → `:1586`, `:1606` → `:1614`; `theme.rs` +8 lines after `:290` (#1108), so every `theme.rs` cite here sits 8 lines lower. `649da124` itself (#1151) touches only `crates/designos-mcp`. Re-anchor against the HEAD you branch from and report drift before writing code.

Naming follows the plan's contract: a colour is named by the token-contract Rust constant, its Untitled token name and the dark hex. A name the contract does not have is marked "new name" and is a request to A. Widgets are named by spec C's anatomy names.

## 1. Summary and user story

As a developer in Dev mode, I see Studio's Inspect panel, the Ready for dev list, the Compare changes window and the generated code in Untitled UI dark: neutral surfaces, Untitled badges and buttons, a syntax palette from Untitled's utility colours with no purple, and the one Dev accent `DEV_READY` wherever Dev mode is shown. Canvas callouts, box-model tints and measurements keep the colours developers know from Figma. The spec reader stays hidden; its palette and accents move to the same Untitled roles so it is ready when it is shown again.

## 2. Current state (FACT unless labelled)

- **Where Dev mode is drawn.** Toolbar Dev toggle and tab badge (spec D), Dev section of the left panel (`panels/left.rs:93-160`, spec E, calling `dev/list.rs`), Share and the Plugins row (spec F), Inspect panel shell (`panels/inspect.rs`, spec F) calling `dev/sections.rs` (`inspect.rs:403-406`, `:593-597`, `:702`) and `dev/playground.rs`, Compare changes window (`dev/compare.rs::show`, `shell.rs:1606`), canvas callouts (`dev/annot.rs::overlay`, `canvas/mod.rs:408`), canvas status badges and measurements (`canvas/draw.rs`, not owned by D–K).
- **Dev accent today is four greens.** Theme `DEV_READY` `#14AE5C` (`theme.rs:564`) for the Ready status; literal `#1BC47D` for the Dev-mode accent in `tabs.rs:1046`, `panels/toolbar.rs:302`, `:342`, `panels/right.rs:66`, `:203`, `:274`, `panels/left.rs:97`, `:289`; `#22D489` for the toolbar toggle hover (`toolbar.rs:465`); `#34D399` text on a `#14AE5C` alpha-38 tint for the Dev list count badge (`dev/list.rs:68-69`). The same `#1BC47D` literal also draws two non-Dev marks: the Update button dot (`tabs.rs:1134`, spec D maps it to `SUCCESS`) and `updater.rs:43` `UI3_EMERALD`. White on `#1BC47D` is 2.27:1 and white on `#14AE5C` 2.90:1.
- **Status pill** (`sections.rs:25-34`): 20 px pill, status colour at 22 % (`gamma_multiply(0.22)`), 3.5 px dot, `TEXT` label in `font_caption_medium` 10. Colours `DEV_READY` `#14AE5C`, `DEV_COMPLETED` `#06B6D4`, `DEV_CHANGED` `#FFA629` (`theme.rs:564-566`). On the target surfaces a `#FAFAFA` label reaches 12.68:1 or more over `BG` and 10.97:1 or more over `BG_SECONDARY` on every tint (computed this session, section 10).
- **Primary buttons** ("Ready for dev", "Mark completed", "Compare changes", "Back to ready", "Save", "Add link"; `sections.rs:36-42`, `:62-93`, `:165-170`, `:266-270`): 28 px pills in `BG_BRAND` `#E36323` (hover `BG_BRAND_HOVER`) with a `TEXT` label in `font_body_strong` 11, 3.14:1 today.
- **Code block** (`sections.rs:386-419`): `BG_SECONDARY` fill, `RADIUS_CONTROL`, `font_mono` 11, line height `CODE_LINE_HEIGHT` = `LINE_BODY` 16 (`theme.rs:124`, `:582`); token colours from `highlight.rs:33-45` mapping to `SYNTAX_*` (`theme.rs:554-562`): plain `TEXT`, property `#7CC4F8`, keyword `#C792EA` (violet, against `design/soul.md` line 15: "Never use purple or violet for chrome, badges, focus rings, gradients or syntax colours"), string `#9CD67D`, number `#F2A365`, type `#FFCB6B`, function `#82AAFF`, comment `#8A8A8A`, punctuation `#B3B3B3`.
- **Box model** (`sections.rs:332-380`): devtools tints `BOX_MARGIN` `#F9CB9C`, `BOX_PADDING` `#B6D7A8`, `BOX_CONTENT` `#9FC5E8` with `BOX_TEXT` `#1E1E1E` (9.23:1 or more).
- **Ready for dev list** (`list.rs`): tabs 24 px (`:51`), selected fill `BG_SECONDARY` with `TEXT`, hover `BG_HOVER`, labels `TEXT_SECONDARY`; rows 32 px (`:78`); count badge `#14AE5C` at alpha 38 with `#34D399` text (`:68-69`).
- **Compare changes** (`compare.rs`): window 820 wide (`:20`) drawn by `widgets::menu_panel` with `RADIUS_FRAME` 14 (`:101`); full-window scrim `from_black_alpha(140)` (`:97`); two image panes on `BG_SECONDARY` (`:53-70`; `:62` passes `Color32::WHITE` as the image tint); change list with the old value in `DANGER` and the new one in `DEV_READY` (`:163`, `:165`); close icon button 28 (`:111-112`); footer "Close" (`small_button`) and "Mark as ready for dev again" (a 190 px `BG_BRAND` pill with a `TEXT` label), both 32 tall (`:134`, `:174-183`).
- **Playground** (`playground.rs`): switch track 28×16 (`:318`), off `BG_SECONDARY`, hover `BG_TERTIARY`, on `BG_BRAND`, knob literal white (`:332`); preview stage `#1E1E1E` (`:470`), dot grid white at alpha 18 (`:478`), preview image tint white (`:499`).
- **Canvas callouts** (`annot.rs:65-161`): `ANNOTATION_BG` `#FFE08A`, `ANNOTATION_TEXT` `#1E1E1E`, `ANNOTATION_LINE` `#FFB800` (`theme.rs:577-579`); no literals.
- **Spec reader** (`specs.rs`). Its own palette, not the theme: ten dark constants `TOKEN_*` (`:32-41`), three status dots `DOT_*` (`:44-46`), a reader light theme (`:66-75`, toggled by `specs-theme-toggle`, `:2750-2754`) and the dark sheet colour (`:80`). Accent literal `#0D99FF` in 12 sites: active-row bars (`:2440`, `:2502`, `:2976`), tab underline (`:2823`), ask-agent sparkle and send button (`:2858`, `:2862`; the same line holds the `#25A8FF` hover, and `:2864-2866` draw the arrow in `Color32::WHITE`), "Reference @spec in Agent" outline (`:3109`), status pill outline and text (`:3239`, `:3241`), live canvas embed outline and text (`:3297`, `:3303`), user-story quote bar (`:3475`). Highlight `#FEF08A` / `#F5C542` at alpha 46 (`:3280`). 20 literal font sizes (`FontId::proportional(15|16|20|22)`).
- **The spec reader is not reachable in the app (FACT by code reading; capture G-05 below).** `shell.rs:1509` hard-codes `let specs_reader_open = false;`, so `specs::show_reader` and `specs::show_right` (`shell.rs:1512`, `:1578`) never run; `specs::show_left` is called only from tests (`specs.rs:4032`, `:4078`); `RailItem::Specs` is left out of the rail (`panels/rail.rs:10-18`). The Home "+ Spec bundle" button and template card (`home/mod.rs:1273-1275`, `:1517`, handled at `:702-705`) and `--panel specs` (`shell.rs:318-322`) still set `RailItem::Specs`. Tests that draw the reader: `specs::tests::renders_specs_reader_and_left_panel_without_lint_issues` (`specs.rs:4015`) and `specs::tests::specs_interactive_controls_mutate_state_and_dispatch_to_agent` (`:4053`).

Measured colour census (non-test code, `scripts/rust_literal_scan.clean`; definitions as in spec D section 2):

| File | `theme::` lines | Literal sites | Site lines | Computed sites |
|---|---|---|---|---|
| `specs.rs` | 128 | 42 | 32-41, 44-46, 66-75, 80, 2440, 2502, 2823, 2858, 2862 (×2), 2864, 2865, 2866, 2976, 3109, 3239, 3241, 3280 (×2), 3297, 3303, 3475 | none |
| `dev/playground.rs` | 32 | 4 | 332, 470, 478, 499 | none |
| `dev/compare.rs` | 24 | 2 | 62, 97 | none |
| `dev/list.rs` | 23 | 2 | 68, 69 | none |
| `dev/sections.rs` | 73 | 0 | | 481 (document colour) |
| `dev/annot.rs`, `dev/highlight.rs` | 13, 9 | 0 | | none |
| `dev/mod.rs`, `dev/color.rs`, `dev/data.rs`, `dev/preset.rs` | 0 | 0 | | none |
| **Total** | 302 | **52** | | 1 |

Command (run from the repo root, or from the root of a `git archive` of the ref; writes nothing):

```bash
PYTHONDONTWRITEBYTECODE=1 python3 - dev/mod.rs dev/annot.rs dev/color.rs dev/compare.rs dev/data.rs dev/highlight.rs dev/list.rs dev/playground.rs dev/preset.rs dev/sections.rs specs.rs <<'PY'
import re, sys
sys.path.insert(0, "scripts")
from rust_literal_scan import clean
HEX = re.compile(r"Color32::from_rgba?(?:_unmultiplied|_premultiplied)?\(\s*(?:0x[0-9A-Fa-f]+|\d+)\s*,"
                 r"|Color32::from_gray\(\s*(?:0x[0-9A-Fa-f]+|\d+)\s*\)|Color32::from_(?:black|white)_alpha\("
                 r"|Color32::(?:WHITE|BLACK|GRAY|LIGHT_GRAY|DARK_GRAY|RED|GREEN|BLUE|YELLOW)\b")
CONV = re.compile(r"Color32::from_rgba?(?:_unmultiplied|_premultiplied)?\((?!\s*(?:0x[0-9A-Fa-f]+|\d+)\s*,)")
for rel in sys.argv[1:]:
    src = open("crates/designos-app/src/" + rel, encoding="utf-8").read()
    code = clean(src).split("\n")
    hexs = [i + 1 for i, l in enumerate(code) for _ in HEX.finditer(l)]
    conv = [i + 1 for i, l in enumerate(code) for _ in CONV.finditer(l)]
    print(rel, "theme_lines=%d" % sum("theme::" in l for l in code), "chrome_sites=%d" % len(hexs), sorted(set(hexs)), "computed=%s" % sorted(set(conv)))
PY
```

Output at `205c72ef` (exit 0, pasted from the run):

```text
dev/mod.rs theme_lines=0 chrome_sites=0 [] computed=[]
dev/annot.rs theme_lines=13 chrome_sites=0 [] computed=[]
dev/color.rs theme_lines=0 chrome_sites=0 [] computed=[]
dev/compare.rs theme_lines=24 chrome_sites=2 [62, 97] computed=[]
dev/data.rs theme_lines=0 chrome_sites=0 [] computed=[]
dev/highlight.rs theme_lines=9 chrome_sites=0 [] computed=[]
dev/list.rs theme_lines=23 chrome_sites=2 [68, 69] computed=[]
dev/playground.rs theme_lines=32 chrome_sites=4 [332, 470, 478, 499] computed=[]
dev/preset.rs theme_lines=0 chrome_sites=0 [] computed=[]
dev/sections.rs theme_lines=73 chrome_sites=0 [] computed=[481]
specs.rs theme_lines=128 chrome_sites=42 [32, 33, 34, 35, 36, 37, 38, 39, 40, 41, 44, 45, 46, 66, 67, 68, 69, 70, 71, 72, 73, 74, 75, 80, 2440, 2502, 2823, 2858, 2862, 2864, 2865, 2866, 2976, 3109, 3239, 3241, 3280, 3297, 3303, 3475] computed=[]
```

The plan's line method (`grep -cE 'Color32::from_rgba?(_unmultiplied|_premultiplied)?\(|Color32::from_gray\(' crates/designos-app/src/specs.rs`, tests included) reports `specs.rs` 37, which is the plan's "37 colour literals"; the census adds the 5 named `Color32::WHITE` sites.

Classification of the 52 sites: 2 identity image tints (`compare.rs:62`, `playground.rs:499`; allowlisted) and 50 chrome colour choices. The one computed site (`sections.rs:481`) converts a document colour for a variable swatch and stays.

Gates today: no colour-literal gate (spec A rule 13 adds `scripts/check_chrome_colour_literals.py`, new name). `scripts/check_spacing_relationships.py` ratchets geometry and font literals (`specs.rs` and `dev/*` have baseline entries). `scripts/zero_fake_ui_gate.py` Z05–Z07 cover `specs.rs`; Z11 covers the Dev actions in the Inspect panel. `scripts/design_os_self_evolve.py` G01 checks 15 theme colours against `design/design.tokens.json` and the banned purples (`#9747FF`, `#8755E9`, `#C4A1FF`, `#A855F7`); `SYNTAX_KEYWORD` `#C792EA` is violet but not on that list, so no gate catches it today.

## 3. Behaviour rules

1. **Chrome or content.** Syntax highlighting is chrome: it colours code Studio generates, in a Studio panel, and the soul's purple rule names syntax colours. Box-model tints, canvas callouts (`ANNOTATION_*`), measurements (`MEASURE`), the Dev status colours (`DEV_READY`, `DEV_COMPLETED`, `DEV_CHANGED`) and the canvas Dev badges are Dev overlays: they keep their values (contract rows 98–107, controller default). The spec reader is chrome.
2. **One Dev accent (R10.4).** Every place that says "you are in Dev mode" uses `DEV_READY` `#14AE5C` (contract row 98, G's `dev-accent` role): toolbar toggle (D), the Dev tab badge (D), the Dev section icon and count in the left panel (E), the Plugins row (F) and the Ready for dev count badge (G). A label or glyph on a filled Dev accent is `TEXT_ON_BRAND` `#0A0A0A` (C-1.35, 6.82:1); accent text on `BG` is `DEV_READY` (C-1.36). The toolbar toggle hover is a new name A derives (`DEV_READY_HOVER`, new name; today `#22D489`). `#1BC47D`, `#22D489` and `#34D399` leave Dev chrome. Untitled success (`#16A34A`) is not used for Dev mode.
3. **Role map** (token-contract constants; widget states are C's, rule 5):

| Element | State | Constant (Untitled role, hex) | Today |
|---|---|---|---|
| Section titles in the Dev sections (Code, Variables, Assets, Annotations, Links, Playground) | default | C `Section/Header` via F's `section_title`: `TEXT` `#FAFAFA`, text-sm 14/20 semibold | `TEXT`, 11 |
| Captions, dates, token notes | default | `TEXT_TERTIARY` `#A3A3A3` (C-1.7) | `TEXT_TERTIARY` (`#F4F4F5` at 40 %) |
| Status pill | Ready / Completed / Changed | C `Badge/StatusCount` anatomy with a dot (height 22, radius-full, text-xs medium): fill = status colour at 22 %, dot = status colour, label `TEXT`; status colours stay `DEV_READY` `#14AE5C`, `DEV_COMPLETED` `#06B6D4`, `DEV_CHANGED` `#FFA629` (decision 2; G-P.9–G-P.11) | same colours, 20 px pill |
| Dev primary buttons (Ready for dev, Mark completed, Compare changes, Back to ready, Save, Add link) | default, hover, pressed, focused, disabled | C `Button/Primary` xs 32: `BG_BRAND` `#E36323` → `BG_BRAND_HOVER` `#F97316` → `BG_BRAND_PRESSED` `#EA580C`, label `TEXT_ON_BRAND` `#0A0A0A` text-sm 14 Semibold (R4, R8.6; C-1.14–C-1.16), radius-md 8, `FOCUS_RING` | `BG_BRAND` + `TEXT` (3.14:1), pill |
| Secondary actions ("Remove status", "Mark ready", copy, download; F's `small_button`) | default, hover, pressed | C `Button/Secondary` xs 32 (F owns `small_button`) | `BG_SECONDARY` |
| Annotation and link editors (text fields, property chips) | default, hover, focused, selected chip | C `Input/Field` xs 32 (R8.4: inspector fields are 32), placeholder `TEXT_PLACEHOLDER` (R8.1, E-1); chips C `Tag/Chip`, selected `BG_SELECTED` (R8.2) | `BG_SECONDARY`, `BG_SELECTED`, `BORDER_SELECTED` |
| Code block | default | no C widget (G anatomy): fill `BG_SECONDARY` `#171717`, border `BORDER` `#262626`, radius `RADIUS_CONTROL` 8, Roboto Mono (P5, D6) | `BG_SECONDARY`, no border |
| Syntax: plain | default | `SYNTAX_PLAIN` = `TEXT` `#FAFAFA` (row 89; C-1.2) | `TEXT` |
| Syntax: property | default | `SYNTAX_PROPERTY` utility-sky-700 `#7DD3FC` (row 90; G-P.1) | `#7CC4F8` |
| Syntax: keyword | default | `SYNTAX_KEYWORD` utility-pink-600 `#F472B6` (row 91, hue 329°, outside violet; decision 3; G-P.2) | `#C792EA` violet |
| Syntax: string | default | `SYNTAX_STRING` utility-green-600 `#4ADE80` (row 92; G-P.3) | `#9CD67D` |
| Syntax: number | default | `SYNTAX_NUMBER` utility-orange-600 `#FB923C` (row 93; G-P.4) | `#F2A365` |
| Syntax: type | default | `SYNTAX_TYPE` utility-yellow-600 `#FACC15` (row 94; G-P.5) | `#FFCB6B` |
| Syntax: function | default | `SYNTAX_FUNCTION` utility-blue-600 `#60A5FA` (row 95; G-P.6) | `#82AAFF` |
| Syntax: comment | default | `SYNTAX_COMMENT` text-quaternary `#A3A3A3` (row 96; G-P.7) | `#8A8A8A` |
| Syntax: punctuation | default | `SYNTAX_PUNCT` text-secondary `#D4D4D4` (row 97; G-P.8) | `#B3B3B3` |
| Box-model diagram | default | Dev overlay: `BOX_*` tints unchanged; band labels `BOX_TEXT` unchanged | same |
| Ready for dev list tabs | default, hover, selected | C `Tabs/Pill` at the surface's current height 24 (C rule after R10.1): label `TEXT_QUATERNARY`; hover `BG_HOVER` + `TEXT_SECONDARY`; selected `BG_SELECTED` + `TEXT_SECONDARY` (as F's strip) | 24 px, `BG_SECONDARY` |
| Ready for dev list rows | default, hover, selected | E's `Tree/Row` roles (C row anatomy, row 32) | `BG_HOVER`, `BG_SELECTED` |
| Ready for dev count badge | count > 0 / 0 | C `Badge/StatusCount` anatomy: fill `DEV_READY` at 22 %, label `TEXT`, mono digits (the rule E uses for its Dev section count); 0 hides the badge as today | `#14AE5C` α38 + `#34D399` |
| Compare changes scrim | open | `BG_OVERLAY` (contract N33, G-3: bg-overlay `#262626` at 70 %) | `from_black_alpha(140)` |
| Compare changes window | default | C `Dialog/Modal`: fill `BG_MENU` `#0A0A0A`, radius `RADIUS_FRAME` 16 (kit Modals, R8.5), no border; title `TEXT` text-md 16 semibold; close C `Button/Close` sm 36; panes `BG_SECONDARY` with `BORDER`; captions `TEXT_TERTIARY` | `menu_panel`, radius 14, close 28 |
| Compare change list values | before / after | before `TEXT_ERROR` `#F87171` (C-1.12; error text is `TEXT_ERROR`, never `DANGER`), after `TEXT_SUCCESS` `#4ADE80` (contract N9; G-P.12; decision 7), arrow `TEXT_TERTIARY` | `DANGER`, `DEV_READY` |
| Compare footer | default, hover | C `Button/Secondary` "Close" and C `Button/Primary` "Mark as ready for dev again", both md 40 (controller default 21:35: dialog footer md 40), label `TEXT_ON_BRAND` on the primary | 32, `BG_BRAND` pill + `TEXT` |
| Playground switch | off, hover, on | C `Toggle/Switch` (track 36×20, knob 16, hit box at least `TARGET_MIN`): off track `BG_TERTIARY` `#262626`, on `BG_BRAND`; knob `ICON_WHITE` (N17; G-P.13, G-P.14) | 28×16, `BG_SECONDARY` off, white knob |
| Playground preview stage | default | `BG_SECONDARY` `#171717` with the dot grid as a new-name constant (`PLAYGROUND_GRID_DOT`, `TEXT` at 7 % alpha; today white at alpha 18) | `#1E1E1E`, white α18 |
| Canvas callouts, measurements, canvas Dev badges | any | Dev overlay constants unchanged (`ANNOTATION_*`, `MEASURE`, `DEV_*`) | same |
| Tooltips | hover | C `Tooltip/Default` (`BG_PRIMARY_SOLID` + `TEXT`, C-1.19) | egui default |
| Image tints (`compare.rs:62`, `playground.rs:499`) | n/a | identity tint, allowlisted | `Color32::WHITE` |

4. **Spec reader roles (restyle only; the reader stays hidden, controller default 21:35).** The reader keeps its two themes (dark default, light toggle) and its layout. Every `ReaderPalette` field maps to a contract constant in dark mode; the light values are the kit's Light mode and are new names (A adds a light role set, `READER_LIGHT_*` or a light contract column):

| `ReaderPalette` field | Constant (Untitled role) | Dark | Light (new names) | Today (dark / light) |
|---|---|---|---|---|
| `bg_page` | `BG` (bg-primary; equals bg-secondary_alt in dark) | `#0A0A0A` | `#FAFAFA` | `#141518` / `#F6F6F7` |
| `doc_sheet` | `BG_PRIMARY_ALT` (N18) | `#171717` | `#FFFFFF` | `#17181C` / `#FFFFFF` |
| `surface` | `BG` (bg-primary) | `#0A0A0A` | `#FFFFFF` | `#191A1E` / `#FAFAFB` |
| `surface_2` (active row, table head) | `BG_TERTIARY` | `#262626` | `#F5F5F5` | `#22242A` / `#F1F2F4` |
| `TOKEN_SURFACE_3` (hover, callout) | `BG_HOVER` (bg-primary_hover) | `#171717` | `#FAFAFA` | `#1D1E23` / none |
| `code_bg` | `BG_SECONDARY` | `#171717` | `#FAFAFA` | `#111215` / `#F4F4F6` |
| `border` | `BORDER` (border-secondary) | `#262626` | `#E5E5E5` | `#282A32` / `#E4E5E9` |
| `hairline` | `BORDER_SUBTLE` (border-tertiary) | `#262626` | `#F5F5F5` | `#22242B` / `#ECEDF0` |
| `text` | `TEXT` | `#FAFAFA` | `#171717` | `#EDEEF0` / `#18181B` |
| `text_muted` | `TEXT_SECONDARY` | `#D4D4D4` | `#404040` | `#B8BCC4` / `#3F3F46` |
| `text_subtle` | `TEXT_TERTIARY` | `#A3A3A3` | `#525252` | `#7E838E` / `#71717A` |
| `DOT_OK` / `DOT_WARN` / `DOT_NEUTRAL` | `SUCCESS` / `WARNING` / `ICON_QUATERNARY` (fg-success-primary / fg-warning-primary / fg-quaternary) | `#22C55E` / `#EAB308` / `#737373` | `#16A34A` / `#CA8A04` / `#A3A3A3` | `#3ECF8E` / `#E5A93B` / `#7E838E` |

The 12 `#0D99FF` accents (with the `#25A8FF` hover and the white arrow): active-row bars and the tab underline take `ICON_BRAND_PRIMARY` `#F97316` (N14, fg-brand-primary; G-P.17); the send button is C `Button/Primary` icon-only xs 32 (`BG_BRAND` with a `TEXT_ON_BRAND` arrow, C-1.14); the sparkle is the input's leading icon `ICON_QUATERNARY`; "Reference @spec in Agent" is C `Button/Secondary`; the status pill is C `Badge/StatusCount` gray (`UTILITY_NEUTRAL_*`, C-1.24); the canvas-embed outline and label take `TEXT_BRAND` (R8.3, as F's MCP URI; C-1.11 on the sheet); the user-story bar is `BORDER_SELECTED` (border-brand). The highlight (`:3280`) takes `BG_WARNING_PRIMARY` `#422006` (N25; light `#FEFCE8`, new name) under `TEXT` (G-P.16). Reader font literals (15, 16, 20, 22) move to B's text roles (text-md 16/24, text-xl 20/30, display-xs 24/32; B decides the mapping).

5. **Widgets and states (spec C).** Buttons (`Button/Primary`, `Button/Secondary`, `Button/Close`), `Badge/StatusCount`, `Toggle/Switch`, `Input/Field`, `Tag/Chip`, `Tabs/Pill`, `Dialog/Modal` with the `BG_OVERLAY` scrim, `Select/Trigger` (code language) and `Tooltip/Default` come from C; the code block is G's own surface. Buttons: C's Default, Hover, Pressed, Focused, Disabled. Status pills: one per Dev status, no hover. Reader rows: Default, Hover, Active. No state is removed; Pressed comes with C's buttons.
6. **Sizes (R5, R8.4, R8.6) and their layout effect.** The Inspect panel keeps its 240 px width and section order; the reader keeps its three columns (R1).

| Element | Code | Today | Target | Δ | Layout effect |
|---|---|---|---|---|---|
| Dev primary buttons | `sections.rs:61-62`, `:165`, `:266` | 28, pill | `Button/Primary` xs 32, radius 8, label 14 Semibold | +4 each | the status header grows 4 px (one button row); the annotation and link editor rows grow 4 px each; labels fit the 184 px content width (INFERENCE: "Compare changes" is the longest, measured in the PR) |
| Status pill | `sections.rs:29` | 20, caption 10 | `Badge/StatusCount` 22, text-xs 12 medium | +2 | the status row stays 28 px tall (`sections.rs:60`); the pill sits 2 px taller inside it |
| Code text | `font_mono` 11, `CODE_LINE_HEIGHT` (= `LINE_BODY` 16, `theme.rs:124`, `:582`) | 11 / 16 | text-xs 12/18 mono | +2 per line | a 30-line CSS block grows about 60 px; the Inspect panel already scrolls; fewer columns per line, so `highlight::wrap` wraps more |
| Annotation and link fields | `sections.rs:153`, `:252` (and the URL field below it) | 28 | `Input/Field` xs 32 (R8.4) | +4 | the editors grow 4 px per field row |
| Ready for dev tabs | `list.rs:51` | 24 | 24 (C `Tabs/Pill` at the surface's height, R10.1) | 0 | none |
| Ready for dev rows | `list.rs:78` | 32 | row 32 | 0 | none |
| Playground switch | `playground.rs:318` (`CONTROL_STANDARD` 28 from the right) | 28×16 | `Toggle/Switch` 36×20 | +8 / +4 | the switch grows 8 px to the left of the right edge; its label column loses 8 px |
| Compare footer buttons | `compare.rs:134`, `:174-176` | 32 | md 40 (controller default 21:35) | +8 | the change list ends 8 px higher inside the same clamped window (`:90`, 380–640 tall) |
| Compare close | `compare.rs:111` | 28 | `Button/Close` sm 36 | +8 | the box grows around the same centre |
| Reader text | `specs.rs` font literals | 15, 16, 20, 22 | B's roles | +1 / 0 / 0 / +2 | reader prose reflows inside the same columns |

7. **No behaviour change.** Widget ids (`dev-ready`, `dev-complete`, `dev-compare`, `dev-reopen`, `dev-annot-save`, `dev-link-add`, `dev-playground-preview`, `compare-close`, `compare-done`, `specs-*`, `hero-copy-ai`, `spec-inline-canvas-embed`), the code generators, copy actions, Compare changes, Z05–Z07 and Z11 stay. Re-enabling the spec reader is out of scope (controller default 21:35).

## 4. Data model / schema changes

None. Dev status, annotations and links in the document are unchanged.

## 5. Contracts

**File ownership across D–K.** G owns:

| File | Owner | Shared surface and rule |
|---|---|---|
| `dev/sections.rs`, `dev/playground.rs` | G | drawn inside F's Inspect panel; F owns the panel shell, `section_title`, `small_button`, `copy`, `download` (`panels/inspect.rs`) |
| `dev/highlight.rs` | G | the `SYNTAX_*` values are contract rows 89–97; the constants live in `theme.rs` (A writes them) |
| `dev/list.rs` | G | drawn inside E's left panel (`panels/left.rs:93-160`); E owns the panel and its Dev-section literals (`left.rs:97`, `:289`), which take `DEV_READY` |
| `dev/compare.rs` | G | the window and scrim use C's `Dialog/Modal` anatomy |
| `dev/annot.rs` | G | canvas overlay; colours unchanged |
| `dev/mod.rs`, `dev/color.rs`, `dev/data.rs`, `dev/preset.rs` | G | no change expected |
| `specs.rs` | G | the reader's `show_left` would draw into E's left-panel slot if re-enabled; still G's file |

Not mine: `theme.rs`, `design/*` (A, B); `widgets/*` and `panels/mod.rs` (C; `widgets/picker.rs` is F's); `tabs.rs`, `shell.rs`, `panels/rail.rs`, `panels/toolbar.rs`, `panels/menus.rs`, `canvas/rulers.rs` (D); `panels/left.rs`, `panels/layers.rs`, `find/filter.rs` (E); `panels/inspect.rs`, `panels/right.rs` and the other inspector files (F); `home/*` (H); `terminal.rs`, `agent_*`, `skills/*`, `cmdk.rs` (I or J; they use `STATUS_INFO_*`, which this spec does not change); `canvas/draw.rs` (canvas, not in D–K: it draws the Dev badges and box tints with the overlay constants); `updater.rs` (`UI3_EMERALD`, not Dev; spec J).

**Dev accent contract.** The value is `DEV_READY` `#14AE5C` (R10.4, contract row 98) with the label `TEXT_ON_BRAND` (C-1.35); A adds the derived hover (new name). D, E and F replace their `#1BC47D` and `#22D489` literals with it; G replaces `#34D399` and the alpha-38 tint in `list.rs`.

**Colour-literal gate.** Spec A rule 13 adds `scripts/check_chrome_colour_literals.py` (new name). G's allowlist: `dev/compare.rs:62` and `dev/playground.rs:499` (identity image tints). Until the gate exists, the section 2 command must print `chrome_sites=0` for every owned file except those two (1 each).

**Purple Ban gate.** A extends `BANNED_PURPLES` or adds a hue check in `scripts/design_os_self_evolve.py` G01 so a violet `SYNTAX_*` value fails (A owns the script change; G only states the need).

## 6. File-format mapping

None. Generated code text is unchanged; only its colours change.

## 7. UI

- Read the in-repo design sources first: `design/principles.md`, `design/soul.md`, `specs/token-contract.md` (A), spec B's scale and spec C's widget kit; they win over generic defaults. The Purple Ban stays (R4) and covers the syntax palette. Contributors who use the project's agent kit may also load its `es-designer` skill; it is not required.
- One primary action per view: in the Inspect panel the Dev primary button is the only `Button/Primary` besides Share (F); both are orange. Dev mode is shown by `DEV_READY`, never by recolouring the primary action.
- Code, token paths and hex values stay in Roboto Mono (P5, D6).
- Copy is unchanged; Z16 stays green.
- Evidence goes into the PR body (not Figma, R3). Responsive: SKIPPED (owner default).

## 8. Edge cases

1. Long generated code lines at 12 px: `highlight::wrap` wraps by column count; continuations keep their indent (test `dev::highlight::tests::wrapping_keeps_every_character_and_indents_continuations`).
2. A layer with no Dev status: no pill; the "Ready for dev" primary button shows.
3. A Changed layer: amber pill and "Compare changes"; the Compare window opens over the scrim and closes on Escape as today.
4. Very long annotation text and many property chips: wrap inside the 240 px panel; chips wrap to new rows.
5. Light documents in the Compare panes: the panes keep a `BORDER` frame so a white layer is still bounded on the modal fill.
6. Playground with no boolean or variant props: the section shows its empty text in `TEXT_TERTIARY`.
7. Spec reader light mode: every field resolves to the light column; no dark constant leaks (test below).
8. Kotlin, Swift, CSS, Tailwind, HTML and UIKit code: all token kinds map to a `SYNTAX_*` constant; no `Tok` falls back to a literal.
9. Annotation fields with no text show the placeholder in `TEXT_PLACEHOLDER` `#737373` (R8.1, named exception E-1), only on `BG` or `BG_SECONDARY` fills.

## 9. Non-goals

- Re-enabling the spec reader in the rail or the canvas (`shell.rs:1509`, `rail.rs:10`), reader layout, new role views.
- Canvas overlay colours (callouts, measurements, canvas Dev badges, box-model tints), `canvas/draw.rs`.
- Code generation, `dev/color.rs` value strings, Dev data model.
- `STATUS_INFO_*` users in agent surfaces (I, J).

## 10. Test plan (red first)

Acceptance criteria:

1. The section 2 command (or A's gate) prints `chrome_sites=0` for every owned file except `dev/compare.rs` and `dev/playground.rs` (1 each, the allowlisted image tints).
2. No `#1BC47D`, `#22D489` or `#34D399` remains in Dev chrome (`grep -rnE '0x1B, 0xC4, 0x7D|0x22, 0xD4, 0x89|0x34, 0xD3, 0x99' crates/designos-app/src --include='*.rs'` prints only `updater.rs:43`, which is not Dev, unless J changes it; D's Update dot at `tabs.rs:1134` moves to `SUCCESS`).
3. Every `SYNTAX_*` value equals contract rows 89–97, is non-violet and reaches 4.5:1 on the code block fill; `design_os_self_evolve.py` G01 fails if a violet value is put back.
4. `python3 scripts/check_spacing_relationships.py` passes; font and radius literal counts for `specs.rs` and `dev/*` fall or stay.
5. Spec L's layout gate shows the same regions, widths and order as the baseline for the Dev-mode captures; height changes match the rule 6 table.
6. `python3 scripts/zero_fake_ui_gate.py` (Z05–Z07, Z11) and `python3 scripts/design_os_self_evolve.py` pass.
7. **Contrast.** `ui ds a11y` is not accepted: on this store it measured 0 pairs (false green, plan.md and contract (f)). The check is three parts:
   - (a) The contract's C-1 check (token-contract (f)) exits 0. Run 2026-10-25 from the specs folder against the contract as last edited at 22:15 (62 rows, C-1.1 to C-1.62): exit 0, 62 rows ok, 0 FAIL. G relies on C-1.1, C-1.2 (plain syntax and reader text on `#171717`), C-1.3, C-1.5, C-1.7, C-1.8, C-1.11 (reader canvas-embed label), C-1.12 (Compare "before" values, `TEXT_ERROR` on `BG_MENU` `#0A0A0A`), C-1.55 (Compare captions, `TEXT_TERTIARY` on `BG_MENU`), C-1.14–C-1.16 (Dev primary buttons, reader send button), C-1.19 (tooltips), C-1.24 (reader status pill), C-1.25–C-1.26 (placeholder, E-1), C-1.28 (reader sparkle), C-1.29 (focus ring) and C-1.35–C-1.36 (Dev accent, R10.4).
   - (b) The G-P rows below, in the C-1 row shape, pass the contract's snippet with only the id prefix and the file changed. Run 2026-10-25 from the specs folder: exit 0, 18 rows ok (lowest: G-P.13 3.46:1 non-text; lowest text row G-P.18 6.00:1). The `*_PILL_ON_BG` backgrounds are composites (the status colour at 0.22 over `BG`), not constants; over `BG_SECONDARY` the lowest is 10.97:1 (`DEV_CHANGED`).
   - (c) A adds the G-P rows to C-1 and to the `chrome_contrast_pairs` test (spec A section 10). The reader's light-mode pairs join them when A adds the light role set; computed this session, the lowest is `text_subtle` `#525252` on `surface_2` `#F5F5F5`, 7.17:1. Until then G's PR runs (b).
8. Before/after captures of every command below are in the PR with exit codes, plus the reader rendered by its test (the reader is not reachable from the CLI).

G-P rows (foreground on background, WCAG 2.x, the contract's luminance formula):

| Id | Foreground | Background | Min | Use |
|---|---|---|---|---|
| G-P.1 | `SYNTAX_PROPERTY` #7DD3FC | `BG_SECONDARY` #171717 | 4.5 | code: property |
| G-P.2 | `SYNTAX_KEYWORD` #F472B6 | `BG_SECONDARY` #171717 | 4.5 | code: keyword |
| G-P.3 | `SYNTAX_STRING` #4ADE80 | `BG_SECONDARY` #171717 | 4.5 | code: string |
| G-P.4 | `SYNTAX_NUMBER` #FB923C | `BG_SECONDARY` #171717 | 4.5 | code: number |
| G-P.5 | `SYNTAX_TYPE` #FACC15 | `BG_SECONDARY` #171717 | 4.5 | code: type |
| G-P.6 | `SYNTAX_FUNCTION` #60A5FA | `BG_SECONDARY` #171717 | 4.5 | code: function |
| G-P.7 | `SYNTAX_COMMENT` #A3A3A3 | `BG_SECONDARY` #171717 | 4.5 | code: comment |
| G-P.8 | `SYNTAX_PUNCT` #D4D4D4 | `BG_SECONDARY` #171717 | 4.5 | code: punctuation |
| G-P.9 | `TEXT` #FAFAFA | `DEV_READY_PILL_ON_BG` #0C2E1C | 4.5 | Ready pill and Ready for dev count badge label |
| G-P.10 | `TEXT` #FAFAFA | `DEV_COMPLETED_PILL_ON_BG` #093036 | 4.5 | Completed pill label |
| G-P.11 | `TEXT` #FAFAFA | `DEV_CHANGED_PILL_ON_BG` #402C11 | 4.5 | Changed pill label |
| G-P.12 | `TEXT_SUCCESS` #4ADE80 | `BG_MENU` #0A0A0A | 4.5 | Compare "after" values |
| G-P.13 | `ICON_WHITE` #FFFFFF | `BG_BRAND` #E36323 | 3.0 | playground switch knob on (non-text) |
| G-P.14 | `ICON_WHITE` #FFFFFF | `BG_TERTIARY` #262626 | 3.0 | playground switch knob off |
| G-P.15 | `TEXT_SECONDARY` #D4D4D4 | `BG_PRIMARY_ALT` #171717 | 4.5 | reader muted text on the sheet |
| G-P.16 | `TEXT` #FAFAFA | `BG_WARNING_PRIMARY` #422006 | 4.5 | reader highlight |
| G-P.17 | `ICON_BRAND_PRIMARY` #F97316 | `BG_PRIMARY_ALT` #171717 | 3.0 | reader active-row bar and tab underline |
| G-P.18 | `TEXT_TERTIARY` #A3A3A3 | `BG_TERTIARY` #262626 | 4.5 | reader subtle text on the active row and table head |

```bash
# from the specs folder; the contract's C-1 snippet with the id prefix and the file changed; exits 1 on a failure
python3 - G-surface-dev-mode-and-spec-reader.md <<'PY'
import re, sys
def lum(h):
    c = [int(h[i:i + 2], 16) / 255 for i in (0, 2, 4)]
    c = [x / 12.92 if x <= 0.04045 else ((x + 0.055) / 1.055) ** 2.4 for x in c]
    return 0.2126 * c[0] + 0.7152 * c[1] + 0.0722 * c[2]
bad = 0
for m in re.finditer(r"^\| (G-P\.\d+) \| `\w+` #(\w{6}) \| `\w+` #(\w{6}) \| ([\d.]+) \|", open(sys.argv[1]).read(), re.M):
    hi, lo = sorted((lum(m[2]), lum(m[3])), reverse=True)
    r = (hi + 0.05) / (lo + 0.05)
    ok = r >= float(m[4]); bad += not ok
    print(f"{m[1]:7} {r:5.2f}:1 (min {m[4]}) {'ok' if ok else 'FAIL'}")
sys.exit(1 if bad else 0)
PY
```

| Case | Expected | Fails before |
|---|---|---|
| census of the owned files | 0 (2 allowlisted) | yes (52 sites) |
| new `dev::highlight::tests::token_colours_are_untitled_utility_roles` (new name) | every `Tok` maps to the `SYNTAX_*` constant in rule 3 with the contract value; none is violet (hue 260°–300°) | yes (`#C792EA`, hue 276°) |
| new `dev::highlight::tests::token_colours_reach_aa_on_the_code_fill` (new name) | each token colour is at least 4.5:1 on the code block fill | guard (passes today on `#1E1E22`, lowest `#8A8A8A` 4.81:1; must keep passing on `#171717`) |
| new `dev::sections::tests::primary_button_label_is_dark_on_brand` (new name) | the Dev primary button label is `TEXT_ON_BRAND` | yes (`TEXT`, 3.14:1) |
| new `dev::list::tests::count_badge_uses_the_dev_accent` (new name) | the count badge fill is `DEV_READY` at 22 % and its label `TEXT` | yes (`#34D399`) |
| new `dev::playground::tests::switch_hit_box_meets_the_target_floor` (new name) | the switch's interact rect is at least `TARGET_MIN` tall and wide | guard (the hit box is the 24 px control rect, `playground.rs:520-523`; the 36×20 track must not shrink it) |
| new `specs::tests::reader_palette_has_no_literals_in_either_mode` (new name) | `ReaderPalette::for_mode(false)` and `(true)` equal the theme role sets | yes |
| `specs::tests::renders_specs_reader_and_left_panel_without_lint_issues`, `specs::tests::specs_interactive_controls_mutate_state_and_dispatch_to_agent` | pass | guard |
| `dev::*` tests (`annot`, `color`, `highlight`, `list`, `playground`), `shell::tests::dev::*` | pass | guard |

Run: `cargo test -p designos-app --bin designos dev::`, `cargo test -p designos-app --bin designos specs::`, `cargo test -p designos-app --bin designos shell::tests::dev`, `cargo test -p designos-app --bin designos theme::`, `python3 scripts/check_spacing_relationships.py`, `python3 scripts/zero_fake_ui_gate.py`, `python3 scripts/design_os_self_evolve.py`, `cargo fmt --all --check`, `cargo clippy -p designos-app --all-targets -- -D warnings`.

**Evidence commands.** `KIT` is a scratch copy of `~/Downloads/❖ Untitled UI Figma – PRO VARIABLES (v8.0) gKXp7LqFY2NV (Copy).fig` (never the original); `FIX` is the fixture built by spec E's `E-00` command.

```bash
cd "$(git rev-parse --show-toplevel)"   # repo root: cargo run -p needs it
export PATH="$HOME/.cargo/bin:$PATH"; S="$(mktemp -d)"; mkdir -p "$S/home" "$S/shots"; OUT="$S/shots"
KIT="$S/untitled-ui-kit-copy.fig"; cp "$HOME/Downloads/❖ Untitled UI Figma – PRO VARIABLES (v8.0) gKXp7LqFY2NV (Copy).fig" "$KIT"   # a copy, never the original
FIX="$S/e-layers.designos"   # create it first with spec E's fixture JSON and its E-00 command
RUN() { env HOME="$S/home" TMPDIR="$S/" CARGO_HOME="$HOME/.cargo" RUSTUP_HOME="$HOME/.rustup" cargo run -q --release -p designos-app --bin designos -- "$@"; echo "exit $?"; }
RUN "$KIT" --screenshot "$OUT/D-09-dev-css.png" --size 1600x1088 --no-toast --dev css --select "Welcome to Untitled UI"                # Dev toolbar, Inspect panel, CSS code block (shared with D and F)
RUN "$KIT" --screenshot "$OUT/E-04-dev-left.png" --size 1600x1088 --no-toast --dev css --dev-demo --select "Welcome to Untitled UI"   # Ready status pill, annotation, link, Ready for dev list (shared with E)
RUN "$FIX" --screenshot "$OUT/G-03-dev-compare.png" --size 1600x1088 --no-toast --panel dev-compare --select "Revamp fixture"         # Compare changes window and scrim, SwiftUI code
RUN "$FIX" --screenshot "$OUT/G-04-dev-annotate.png" --size 1600x1088 --no-toast --panel dev-annotate --select "Revamp fixture"       # annotation editor, canvas callout
RUN "$KIT" --screenshot "$OUT/G-05-panel-specs.png" --size 1600x1088 --no-toast --panel specs                                          # what --panel specs shows today (the reader is not drawn)
cargo test -p designos-app --bin designos specs::tests::renders_specs_reader_and_left_panel_without_lint_issues                       # the reader's only render path today
```

Recorded exit codes, with the ref and machine state of each run (the spec writer ran the same arguments through a wrapper in its scratch folder, not in the repo, with an isolated `HOME`):

| Capture | Exit | Ref and conditions |
|---|---|---|
| G-03, G-04, G-05 | 0 | spec writer, local build of `ec47da26`, 2026-10-25, many concurrent cargo builds from other agents |
| reader render test | 0 (1 passed) | same build |
| D-09 | 1 twice ("designos: screenshot was not captured") | same build and load; not rerun |
| E-04 | 1 once (same message) | same build and load; not rerun |

No capture was rerun at `205c72ef`. The cause of the exit-1 runs is not diagnosed (a `--screenshot` run gives up after 240 frames, `shell.rs:483`). The implementer reruns D-09 and E-04 on an idle machine and records the exit codes in the PR.

Not capturable from the CLI today: the spec reader itself (section 2), hover and focus states, the playground (needs a component instance with properties selected in Dev mode), the reader's light mode. The PR covers the reader with the palette test above and the rest with paint tests and a manual window capture labelled manual.

## 11. Performance budgets

- Syntax colours stay `const`; `highlight::color` stays a `match` with no allocation.
- The Dev code cache (`DEV_CODE_CACHE`, `sections.rs`) is untouched; larger code text adds no regeneration.
- No `request_repaint` added; idle CPU stays 0 % (CLAUDE.md section 4).

## 12. Dependencies and build order

- A, B, C first; L's baselines before the first PR. Build order: `highlight.rs` and its tests (contract rows 89–97); `sections.rs` (pill, primary button, code block, editors); `list.rs`; `compare.rs`; `playground.rs`; then `specs.rs` (G2); then R5/R8.4 sizes; then evidence.
- **Sent to A:** the derived Dev toggle hover (`DEV_READY_HOVER`, new name); `PLAYGROUND_GRID_DOT` (new name); the reader's light role set (new names); the G-P rows for C-1 and `chrome_contrast_pairs`; the Purple Ban hue check in G01.
- **Sent to D:** `tabs.rs:1134` is the Update button dot, not a Dev accent (an earlier draft of this spec listed it as Dev); D maps it to `SUCCESS`.
- **Open PRs touching G files** (`gh pr list --state open`, 2026-10-25): stale #770 `task/572-parallel-agents-frame-ownership`, #768 `task/599-auto-fix-settings-and-report-ui`, #765 `task/598-engine-auto-fix` change `dev/data.rs`, `dev/highlight.rs` and `dev/mod.rs`. No open PR touches `specs.rs`. #1106, #1108, #1148 and #1151 touch no G file.

## 13. Owner rulings that apply

- R1 (layout unchanged), R2 (Untitled spacing and type), R3 (no Figma), R4 (Studio orange, dark label `#0A0A0A` 5.73:1, focus ring Orange-500 `#F97316`, Purple Ban), R5 (text-xs 12/18 rows and labels, text-sm 14/20 titles, button xs 32, row 32, radius-md 8), R6 (keep icons), R7 (spec-driven).
- R8.1 placeholder `#737373` (annotation fields); R8.2 selected `#262626`; R8.3 accent text `#F97316` (reader canvas-embed link); R8.4 inspector fields 32 (annotation and link editors); R8.5 floating chrome 16 (Compare window); R8.6 button labels 14 Semibold, tab and row text 12. R8.7 does not apply (no destructive button in G's files). R9: figma-agent read-only is allowed; not used here.
- R10.1 tab heights stay as today (C's `Tabs/Pill` rule: the Ready for dev tabs stay 24); R10.4 Dev accent `DEV_READY` `#14AE5C` with the dark label (decides open question 1).
- Controller defaults: canvas and Dev overlays keep Figma-parity colours (Dev ready `#14AE5C`, component cyan `#06B6D4`, guides `#F24822`); chrome status colours move to Untitled. 21:35 sweep: dialog footer buttons md 40; dialog radius follows the kit Modals, floating chrome 16; status badges are Untitled tinted badges (contract N24–N37); "Spec reader: restyle palette only, stays hidden (G OQ2)" (decides open question 2).
- `design/soul.md`: no purple or violet in syntax colours. P2: one primary action per view. P5: code in Roboto Mono.
- 2026-09-22: visual evidence in every PR. Standing: protected `main`. 2026-10-10: responsive SKIPPED by default.

## Decisions recorded by the spec writer (technical; owner may reverse)

1. Syntax highlighting is chrome and takes the contract's `SYNTAX_*` values (rows 89–97, Untitled utility steps), all at 6.7:1 or more on `#171717`.
2. Dev statuses keep their three colours in the panel and on the canvas, so one state has one colour everywhere; only the pill anatomy changes to C's `Badge/StatusCount`.
3. Keyword is utility-pink-600 `#F472B6` (hue 329°): the only utility hue left that is distinct from the other six token colours and is not violet (260°–300°), not red (read as an error) and not indigo (`#818CF8`, hue 234°, too close to function blue `#60A5FA`). `#C792EA` today is hue 276°.
4. Dev primary buttons stay brand orange (P2, soul: "the primary action is the primary orange"); `DEV_READY` marks Dev mode, not actions.
5. The spec reader is restyled in this spec even though it is hidden, because its 42 literals would otherwise be the largest exception to A's literal gate; it ships as G2 and can wait.
6. The reader's light theme maps to Untitled's Light mode values from the kit export; A holds them as a separate light role set (new names).
7. The Compare change list uses diff colours: removed values in `TEXT_ERROR`, new values in `TEXT_SUCCESS` (not `DANGER`, which is for icons, and not `DEV_READY`, which marks Dev mode).

## Open questions for the owner

None. The Dev accent is decided by R10.4 (`DEV_READY` `#14AE5C`; Untitled success `#16A34A` is dropped) and the spec reader by the 21:35 controller default (restyle the palette only; it stays hidden).

## Changelog (pass of 2026-10-25 after board 3)

| Item | Resolution |
|---|---|
| Re-anchor to `205c72ef` | Only `panels/rail.rs` (cited lines unmoved) and `theme.rs` (−3 after `:263`) changed among cited files; census identical at `35a65796` and `205c72ef` (exit 0, pasted); drift to `649da124` listed (`left.rs`, `shell.rs`, `theme.rs`). |
| Open question 1 (Dev accent) | Decided by R10.4: `DEV_READY` `#14AE5C` with `TEXT_ON_BRAND`; the `#16A34A` alternative is dropped. |
| Open question 2 (spec reader) | Decided by the 21:35 controller default: restyle only, stays hidden (G2). |
| `on-brand-label` | Replaced by the contract constant `TEXT_ON_BRAND` (C-1.14–C-1.16, C-1.35). |
| `dev-accent` | Kept as G's role name; its value is `DEV_READY` (contract row 98). |
| Contract names | Provisional Untitled names replaced by contract constants: `SYNTAX_*` rows 89–97 (property follows row 90, `#7DD3FC` utility-sky-700, instead of this spec's earlier `#38BDF8`), `BG_OVERLAY` N33, `BG_PRIMARY_ALT` N18, `ICON_BRAND_PRIMARY` N14, `ICON_WHITE` N17, `BG_WARNING_PRIMARY` N25, `UTILITY_NEUTRAL_*` N21–N23, `TEXT_SUCCESS` N9, `SUCCESS`, `WARNING`. |
| Widget names | C names: `Button/Primary`, `Button/Secondary`, `Button/Close`, `Badge/StatusCount`, `Input/Field`, `Tag/Chip`, `Tabs/Pill`, `Toggle/Switch`, `Dialog/Modal`, `Select/Trigger`, `Tooltip/Default`. |
| Contrast | `ui ds a11y` not accepted; C-1 plus 18 G-P rows (exit 0). |
| `tabs.rs:1134` | Corrected: it is the Update dot (D, `SUCCESS`), not a Dev accent. |
| R8 and 21:35 defaults | R8.1, R8.2, R8.3, R8.4, R8.5, R8.6 applied; Compare footer md 40 and radius 16; status badges per contract. |
| Colour-literal gate | Cited as spec A rule 13 (was "spec D section 5 proposes"). |
