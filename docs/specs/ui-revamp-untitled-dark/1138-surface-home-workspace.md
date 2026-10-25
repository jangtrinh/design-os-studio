# H Restyle the Home workspace in Untitled UI dark (Home density)

| Field | Value |
|---|---|
| Issue | #1138 (epic #1118, spec key H) |
| Size | M. About two days: one live 2,893-line file (`home/mod.rs`) with 38 chrome colour sites and 12 derived tints, Home-density sizes (R5) on four control families, card and tile heights re-fitted to the new type, two dead files removed, new tests. |
| Depends on | A (colour roles, Studio brand override, constant names in `theme.rs`), B (type, spacing, radius and control-size scale; `HOME_*` constants), C (widget anatomy: `Buttons/Button`, `Input field`, `Dropdown menu`, `Badge`, `_Nav item base`, `Empty state`, `Featured icon`) and L (baseline capture and layout-unchanged gate) |
| Unblocks | M (docs and screenshot refresh) |
| Touches | `crates/designos-app/src/home/mod.rs` (drag ghost `:539-560`; inline `mod sidebar` `:873-1207`; inline `mod main_view` `:1212-2716`); deleted: `home/sidebar.rs`, `home/main_view.rs` (dead, never compiled); tests: `crates/designos-app/src/shell/tests/home.rs`, one shared helper in `crates/designos-app/src/shell/tests.rs` if no gate owner has added it (section 10); `scripts/spacing_baseline.json` (lowered with `--write-baseline`, never raised). Reads only (owned by A and B): `theme.rs:692-702` (`HOME_*`). |

Code baseline: HEAD `ec47da26` (local `main` this spec was written on). Re-anchor every cited line against the HEAD you branch from and report drift before writing code. Every measurement below was taken on `ec47da26` on 2026-10-25.

Implementer note: this spec is self-contained. Load the `es-designer` skill before writing UI code (section 7). Colours are named by their Untitled UI token name and hex; the Rust constant names come from spec A, sizes from spec B, widget anatomy from spec C. Where A, B or C are merged before you start and differ from the expectations written here, A, B and C win; report the difference in the PR.

## 1. Summary and user story

As a designer who opens Studio, I land on Home and see it in the Untitled UI dark style: neutral surfaces (`bg-primary #0A0A0A`, `bg-secondary #171717`), hairline borders, Untitled buttons, inputs, menus and badges at the Home density of R5 (text-sm 14/20, button sm 36 / md 40), and the Studio orange `#E36323` with a dark `#0A0A0A` label on the one primary action ("+ Design file"), not today's blue. Home keeps its layout: the 264 px sidebar on the left, the header with title and action buttons, the filter/sort bar, the grid or list of files, the Trash at the bottom of the sidebar, and every section in today's order (R1).

## 2. Current state (FACT unless labelled)

**Files.** Measured with the census command below, plus `wc -l`.

| File | Lines | Compiled | `theme::` lines | Chrome colour sites | Site lines |
|---|---|---|---|---|---|
| `home/mod.rs` | 2,893 | yes | 182 | 38 | 552, 884-887, 889, 892, 910, 911, 1131, 1200, 1203, 1222-1229, 1411, 1559, 1588, 1723, 1724, 2005, 2023, 2177, 2183, 2421, 2427, 2500, 2523, 2537, 2543, 2552, 2553, 2583 |
| `home/thumb.rs` | 201 | yes | 1 | 0 | (`:153` builds a `ColorImage` from pixels: not a colour) |
| `home/time.rs` | 67 | yes | 0 | 0 | logic only |
| `home/session.rs` | 1,484 | yes | 0 | 0 | logic only (autosave, CLAUDE.md invariant 1) |
| `home/sidebar.rs` | 214 | **no** | 20 | 0 | dead file |
| `home/main_view.rs` | 544 | **no** | 82 | 7 | dead file |

`home/sidebar.rs` and `home/main_view.rs` are dead: `home/mod.rs` declares `mod sidebar { … }` (`:873`) and `mod main_view { … }` (`:1212`) inline, so Rust never reads the two files, and nothing else references them (`grep -rnE 'sidebar\.rs|main_view\.rs|include!' crates/designos-app/src` finds nothing). They still have entries in `scripts/spacing_baseline.json` (`home/main_view.rs` 47 offset + 1 radius literals, `home/sidebar.rs` 14 offset literals). Editing them changes nothing on screen.

Census command (shared with specs D–G; run from the repo root; writes nothing). A *chrome site* is a `Color32` constructor with literal arguments, a `from_gray`/`from_black_alpha`/`from_white_alpha` call, or a named `Color32` colour other than `TRANSPARENT`, in non-test code (comments and strings blanked and `#[cfg(test)]` items dropped by `scripts/rust_literal_scan.clean`):

```bash
PYTHONDONTWRITEBYTECODE=1 python3 - home/mod.rs home/thumb.rs home/time.rs home/session.rs home/sidebar.rs home/main_view.rs <<'PY'
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

plan.md's figure for `home/mod.rs` is 19 (`grep -c 'Color32::from_rgb(…'`, test code included). It counts only `Color32::from_rgb` calls; the census above also counts named colours (`Color32::WHITE`) and alpha constructors, so it reads 38. Both are correct for their definition; this spec uses the census.

Derived tints: `home/mod.rs` has 12 `gamma_multiply(` calls on chrome colours (`grep -c 'gamma_multiply(' crates/designos-app/src/home/mod.rs` = 12, all in shipped code): accent tints on cards, banners and icon boxes, and `theme::DANGER.gamma_multiply(0.2)` for the danger hover (`:1420`).

**Why the primary action is blue.** P2 (`design/principles.md:10-19`) and the soul (`design/soul.md:15`, `:32`) say the one primary action per view is orange `#E36323`. Home ignores `theme::BG_BRAND` (`theme.rs:36`) because both inline modules define their own Figma-UI3 copies: `UI3_BLUE = #0D99FF` (`:892`, `:1225`) and `UI3_BLUE_HOVER = #0B84E0` (`:1226`), and the header button kind is `Kind::PrimaryBlue` (`:1233`, used `:1256`, painted `:1408-1412` with a white label). The same `UI3_*` block defines Zinc copies of theme colours (`UI3_SIDEBAR_BG #18181B`, `UI3_SURFACE #1E1E22`, `UI3_PILL_ACTIVE`/`UI3_BORDER #27272A`, `UI3_PILL_HOVER #232329`, `UI3_MAIN_BG #121215`, `:884-892`, `:1222-1224`) and four category accents (`UI3_TEAL #14AE5C`, `UI3_AMBER #F59E0B`, `UI3_ORANGE #FF6A3D`, plus blue, `:1227-1229`).

**Regions** (all in `home/mod.rs`; screenshot `H-01` in section 10):

- **Frame** `show` (`:428-459`): fills `theme::BG`, splits sidebar `HOME_SIDEBAR_W` 264 (`theme.rs:692`) and main.
- **Sidebar** (`:894-1137`): fill `UI3_SIDEBAR_BG`, right hairline `UI3_BORDER` (`:896-897`).
  - Workspace header 32 tall (`:902-924`): avatar tile 20 with literal `#27272A` fill and `#3F3F46` stroke and an orange folder icon (`:909-912`); name "Personal Workspace" in a literal `FontId::new(12.0, …)` (`:913-914`); version pill 16 tall in mono with `ver_font.size = 10.0` (`:915-921`); "Workspace activity" icon button 24 (`:927-929`, C's `icon_button`).
  - Search field 32 (`:932-957`): `UI3_SEARCH_BG` + `UI3_BORDER`, icon `ICON_SECONDARY`, text `TEXT`, hint `TEXT_TERTIARY`.
  - Nav rows `ROW_H` 28 with 2/1 px gaps (`:880`, `:959-999`): Recents, Templates & Specs, divider, "FILES & PROJECTS" heading with a blue "P" disc (`:975-978`) and a `+` icon button 24 (`:979-983`), Drafts, All folders, Resources & Libraries.
  - Project tree (`:1006-1076`), scrolling, rows 28, "No projects yet" (`:1019`), chevrons `ICON_TERTIARY`.
  - Starred row with expander and up to 3 file rows 24 with blue file icons (`:1078-1117`).
  - Updater card (`:1119-1122`): `updater.rs` (spec J), hidden while `UPDATER_UI_ENABLED` is false (Z14). Not drawn today.
  - Trash row with a count badge 22×16 literal `#27272A` (`:1124-1136`).
  - Row painter `row` (`:1187-1206`): drop target `BG_SELECTED_HOVER`, selected `UI3_PILL_ACTIVE`, hover `UI3_PILL_HOVER`, radius `CornerRadius::same(6)`; selected icon and label `Color32::WHITE` (`:1200`, `:1203`), label `font_body_strong` when selected.
- **Main header** `HOME_HEADER_H` 64 (`:1244-1290`): breadcrumb in `font_title` 20 (`:1353-1396`); right-aligned buttons 32 tall, pill radius, `font_body_strong` (`button`, `:1398-1428`): "+ Design file" `PrimaryBlue`, "+ FigJam board", "+ Spec bundle", "Import" `Secondary` (`UI3_SURFACE` + `UI3_BORDER`), "+ New folder" in project sections, "Empty trash" `Danger` (transparent, `theme::DANGER` stroke and text) in Trash.
- **Filter bar** (`:1292-1335`): `Filter: …` and `Sort: …` pills 28 tall with chevrons (`:1297`, `:1311`), `List view`/`Grid view` icon buttons 28 (`:1324-1333`); band height 38 (`:1334`) then a hairline (`:1336`). Menus are `widgets::menu::popup_menu` (spec C, `:461-496`).
- **Content views**: Templates & Specs (`:1441-1633`: six 240×188 cards with accent banner, category pill, CTA 26 tall); All folders (`:1639-1886`: project cards with 2×2 previews, "+ Folder" mini button in black alpha 170 / blue, "+ New project" card, teal folder tiles); Resources & Libraries (`:1887-2102`: four hub cards with accent CTAs 26 tall and white labels `:2005`, `:2023`); files view (`:2210-2497`: Drafts quick-create strip of three 48-tall tiles with blue/amber/teal accents `:2256-2292`, folder tiles `HOME_FOLDER_H` 48, file grid, "Getting started" starter kits `:2440-2450`); Trash (`:2655-2716`: note line, rows 52 `:2662`, "Restore" secondary buttons).
- **File card** `card` (`:2590-2623`): 224 × (140 + 58) (`theme.rs:695-698`), radius 8, fill `UI3_SURFACE`, hover stroke `TEXT_TERTIARY`, selected 2 px `UI3_BLUE` (`:2600-2606`); thumbnail stage literal `#18181B` (`:2543`) with the picture inset 6 and a white-alpha-18 frame (`:2552-2553`); board thumbnails on `BOARD_BG` with a "Board" badge (`:2519-2537`, `:2559-2568`); file-type badge 20×20 in blue/amber/teal/orange with a white glyph (`:2570-2588`); title `font_body_strong` 11 at `thumb.bottom() + 17`, meta `font_caption` 10 at `+16` (`:2613-2622`). Star overlay: black-alpha circles (`:2177`, `:2421`) and a `#FFCD29` star (`:2500`); hover scrim black alpha 90 (`:2183`, `:2427`).
- **List row** `list_row` (`:2630-2653`): `HOME_LIST_ROW_H` 44, selected `UI3_SURFACE` + 1 px `UI3_BLUE`, hover `BG_HOVER`, 48-wide thumbnail, columns from `list_columns` (`:2625-2628`).
- **Empty state** `empty_state` (`:2188-2208`): 32 px file icon `ICON_TERTIARY`, title `font_large_strong` 13, body `font_body` 11 wrapped at 380.
- **Drag ghost** (`:539-560`): 28-tall pill in `BG_BRAND` with a white label (`:550-552`). White on `#E36323` is 3.46:1 (plan.md facts), below 4.5.

**Gates today.** No gate counts colour literals. `scripts/check_spacing_relationships.py` ratchets literal geometry per file against `scripts/spacing_baseline.json` (`home/mod.rs`: 162 offset, 43 radius, 2 font literals; it passes on `ec47da26`, exit 0). `scripts/zero_fake_ui_gate.py` Z09 checks `home/mod.rs` (`Act::ExportFig` writes bytes; sections filter distinctly) and Z14 the hidden updater card. `.github/workflows/design-gates.yml` runs those Python gates as advisory (`continue-on-error: true`, `:4-5`, `:21`); only the macOS test, clippy and rustfmt jobs block a merge (CLAUDE.md, Git Workflow). Home tests: `home/mod.rs:2719-2893` and `shell/tests/home.rs`.

**UNKNOWN.** Whether spec B maps Home row height to 36 or keeps 28 (Open question 1); the exact Untitled `Input field` height at Size=sm (C and B measure it from the kit).

## 3. Behaviour rules

1. **Layout unchanged (R1).** Same regions, same order, same widths: sidebar 264, main pad `HOME_PAD` 32, header 64, card width 224 and gap 24, list columns. Heights grow only where R5 raises a control or type size, by the amounts in section 7 table 3. No new control, no removed control, no new copy.
2. **One brand action.** "+ Design file" is `Buttons/Button · Size=sm · Hierarchy=Primary`: fill `bg-brand-solid` with the Studio override `#E36323`, label `#0A0A0A` (R4, 5.76:1), hover `bg-brand-solid_hover` (Studio override; A names it). No other Home control uses the brand fill. `Kind::PrimaryBlue` and every `UI3_*` constant leave the file.
3. **No chrome literal.** After the change the census prints `chrome_sites=2` for `home/mod.rs`, both on the allowlist (`:2523` and `:2552`: `Color32::WHITE` as a texture tint inside `RectShape::filled(..).with_texture`, which multiplies the thumbnail by 1 and is not a colour role), and 0 for `home/thumb.rs`, `home/time.rs`, `home/session.rs`. Every other colour is a `theme::` constant from spec A. Derived tints (`gamma_multiply`) on chrome colours are replaced by the Untitled role that exists for that state (section 7 table 1); a tint stays only where Untitled has no role and A defines it as a named constant.
4. **Every white-on-brand label becomes dark.** The drag ghost label (`:552`) and any label painted on `bg-brand-solid` use the R4 dark label `#0A0A0A`.
5. **Neutral selection (P2).** The selected sidebar row and the selected list row use a neutral fill; the selected grid card uses a 2 px brand border (Open question 3 recommends it), never `#0D99FF`.
6. **Dead files go.** Delete `home/sidebar.rs` and `home/main_view.rs` in the same PR (technical decision, section 12). The build proves they were dead: it compiles without them.
7. **Header never overlaps.** At every window width down to `MIN_WINDOW` 900×600 (`theme.rs:605`), the breadcrumb ends at least `spacing-lg` 12 px left of the leftmost header button; a longer breadcrumb elides its leading segments with "…" first, then the last segment. Today at 900 px there is about 60 px between "Recents" and "Import" (screenshot `H-03`); the 14 px type adds about 80 px to the button row, so without this rule the title and buttons collide.
8. **Behaviour unchanged.** Every click, drag, drop, context menu, keyboard path (Escape clears search `:445-454`), recorded widget id (`home-new-file`, `home-filter`, `home-sort`, `home-view-list`, `home-view-grid`, `home-search`, `home-<nav key>`, `home-trash`, `home-template-*`, `home-drafts-*`, `home-starter-*`, `home-res-*`) and every `Act` stay as they are. Z09 stays green.

## 4. Data model / schema changes

None. No document, library, settings or file-format change.

## 5. Contracts

- **Theme constants consumed** (names are spec A's; these are the Untitled roles this spec needs): `bg-primary`, `bg-primary_alt`, `bg-primary_hover`, `bg-secondary`, `bg-secondary_hover`, `bg-tertiary`, `bg-brand-solid` and `_hover` (Studio override), `bg-error-primary`, `border-primary`, `border-secondary`, `border-brand` (Studio override), `border-error_subtle`, `text-primary`, `text-secondary`, `text-tertiary`, `text-placeholder`, `text-error-primary`, `fg-secondary`, `fg-quaternary`, `fg-quaternary_hover`, `fg-warning-secondary`, the on-brand dark label `#0A0A0A` (R4), utility colours for the category accents if Open question 2 is accepted, and alpha scrims (`alpha-white-*`, which in the kit's dark mode resolve to `#0A0A0A` at the given alpha).
- **Size constants consumed** (spec B): `text-xs` 12/18, `text-sm` 14/20, `text-md` 16/24, `text-xl` 20/30, button sm 36, `spacing-xs` 4, `spacing-sm` 6, `spacing-md` 8, `spacing-lg` 12, `radius-md` 8, `radius-full`. `HOME_*` constants (`theme.rs:692-702`) are A/B's file: this spec asks B for one change, `HOME_CARD_H = HOME_THUMB_H + 62` (section 7 table 3); `HOME_THUMB_BG` (`:702`, unused: `let _ = theme::HOME_THUMB_BG` at `:2542`) can be removed by A.
- **Widgets consumed** (spec C): `Buttons/Button · Size=sm` (Primary, Secondary, Destructive), `Buttons/Button utility · Size=sm` (icon-only), `Input field · Size=sm` (search), `Dropdown menu` (filter, sort and context menus; already `widgets::menu`), `_Nav item base` (sidebar rows), `Badge · Size=sm · Color=Gray` (version, Trash count, "Board"), `Featured icon` (file-type badges, template and quick-create icons), `Empty state · Size=sm`, `Tooltip`. Component-set names come from the kit (`designos <kit> --inspect-json`, page/screen listing, 2026-10-23); property names (`Size`, `Hierarchy`, `Color`) are spec C's to confirm.
- **Rust surface.** `main_view::Kind` keeps three variants renamed `Primary`, `Secondary`, `Destructive` (new names); `button()` keeps its signature. No public API change outside `home/`.

## 6. File-format mapping

None.

## 7. UI

Load the `es-designer` skill first and complete its checklist; the project's design system wins over the skill's defaults. Read `design/principles.md`, `design/soul.md`, `design/ux-writing.md` before editing. Evidence goes in the PR (not Figma, R3). Responsive: SKIPPED (owner default): capture 1600×1088 plus the 900×600 minimum window of rule 7.

**Table 1: colour roles** (expected values; spec A's token contract is authoritative; hexes are the kit's dark mode, resolved this session from `--export-variables … --format dtcg` with `evidence/resolve-untitled-tokens.py`; "Studio override" means R4's orange family replaces the kit's purple brand):

| Element | Today | Untitled role | Hex |
|---|---|---|---|
| Main area background | `UI3_MAIN_BG #121215` | `bg-primary` | `#0A0A0A` |
| Sidebar background | `UI3_SIDEBAR_BG #18181B` | `bg-secondary` (same as the editor left panel, specs D/E) | `#171717` |
| Sidebar right hairline, header bottom hairline, dividers | `UI3_BORDER #27272A` | `border-secondary` | `#262626` |
| Search field | `#1E1E22` fill, `#27272A` border | `Input field`: `bg-primary`, `border-primary`, placeholder `text-placeholder`, focus ring `focus-ring` (R4 `#F97316`) | `#0A0A0A`, `#404040`, `#737373` |
| Nav row hover / selected / drop target | `#232329` / `#27272A` / `BG_SELECTED_HOVER` | `bg-secondary_hover` / `bg-tertiary` / `bg-tertiary` + 1 px `border-brand` (Studio override) | `#262626` / `#262626` / `#262626` + `#E36323` |
| Nav label / selected label | `TEXT` / `WHITE` | `text-secondary` / `text-primary` (weight per B) | `#D4D4D4` / `#FAFAFA` |
| Nav icon / selected icon | `ICON_SECONDARY` / `WHITE` | `fg-quaternary` (hover `fg-quaternary_hover`) / `fg-secondary` | `#737373` (`#A3A3A3`) / `#D4D4D4` |
| Section heading "FILES & PROJECTS", chevrons | `TEXT_SECONDARY`, `ICON_TERTIARY` | `text-tertiary`, `fg-quaternary` | `#A3A3A3`, `#737373` |
| Primary button | `#0D99FF` / `#0B84E0`, white label | `bg-brand-solid` / `_hover` (Studio override), label R4 dark | `#E36323` / A's hover, `#0A0A0A` |
| Secondary button | `#1E1E22` + `#27272A`, hover `#27272A` | `bg-primary` + `border-primary`, label `text-secondary`; hover `bg-primary_hover`, label `text-secondary_hover` | `#0A0A0A` + `#404040`, `#D4D4D4`; `#171717`, `#E5E5E5` |
| Destructive button ("Empty trash") | transparent + `DANGER #FE5938` | `Buttons/Button destructive · Hierarchy=Secondary`: `border-error_subtle`, `text-error-primary`, hover `bg-error-primary` | `#EF4444`, `#F87171`, `#450A0A` |
| Filter/sort pills | `#1E1E22` + `#27272A` | secondary button (row above) with a trailing chevron in `fg-quaternary` | as above |
| Card, folder tile, quick-create tile, hub and template card | `#1E1E22`, stroke `#27272A`, hover `TEXT_TERTIARY` | `bg-primary_alt` + `border-secondary`; hover `border-primary` | `#171717` + `#262626`; `#404040` |
| Selected card | 2 px `#0D99FF` | 2 px `border-brand` (Studio override; Open question 3) | `#E36323` |
| Thumbnail stage | `#18181B`, picture frame white α 18 | `bg-primary`, frame `border-secondary` | `#0A0A0A`, `#262626` |
| Card title / meta | `TEXT` / `TEXT_SECONDARY` | `text-primary` / `text-tertiary` | `#FAFAFA` / `#A3A3A3` |
| File-type badge, template accents, quick-create and hub accents, "P" disc, starred-file icon | `#0D99FF`, `#F59E0B`, `#14AE5C`, `#FF6A3D` + white glyph | `Featured icon` with utility colours (Open question 2): design file `utility-blue`, board `utility-amber`, deck `utility-green`, spec bundle `utility-orange`; tile `-50`, glyph `-600` | blue `#172554`/`#60A5FA`, amber `#451A03`/`#FBBF24`, green `#052E16`/`#4ADE80`, orange `#431407`/`#FB923C` |
| Star (on) | `#FFCD29` | `fg-warning-secondary` | `#FACC15` |
| Star backdrop, hover scrim | black α 80/150, α 90 | `alpha-white-*` scrims (dark mode `#0A0A0A` at α): star `-60`/`-30`, scrim `-30` (A names them) | `#0A0A0A` @ 60 % / 30 % |
| Version pill, Trash count, "Board" badge | `#27272A` / `BG_MENU` + `TEXT_SECONDARY` | `Badge · Color=Gray`: `utility-neutral-50` fill, `utility-neutral-200` border, `utility-neutral-700` text | `#171717`, `#404040`, `#D4D4D4` |
| Workspace avatar tile | `#27272A` + `#3F3F46`, orange icon | `bg-tertiary` + `border-primary`, icon `fg-brand-primary` (Studio override) | `#262626` + `#404040`, `#E36323` |
| Empty state | `ICON_TERTIARY`, `TEXT`, `TEXT_SECONDARY` | `Empty state`: `Featured icon · Color=Gray`, title `text-primary`, body `text-tertiary` | `#FAFAFA`, `#A3A3A3` |
| Drag ghost | `BG_BRAND` + white | `bg-brand-solid` + R4 dark label | `#E36323` + `#0A0A0A` |
| Breadcrumb current / parent / separator / hover | `TEXT` / `TEXT_SECONDARY` / `TEXT_TERTIARY` / `BG_HOVER` | `text-primary` / `text-tertiary` / `text-placeholder` / `bg-primary_hover` | `#FAFAFA` / `#A3A3A3` / `#737373` / `#171717` |
| Board thumbnail paper and dots | `BOARD_BG`, `BOARD_DOT` | unchanged (document content) | `#F4F4F4` |

Contrast to check in the PR (A's a11y gate): `text-placeholder #737373` on `bg-primary #0A0A0A` is 4.18:1 and on `#171717` 3.78:1 (plan.md facts); the sidebar search sits on `bg-primary` inside a `bg-secondary` panel, so the hint reads 4.18:1. It is the kit's own placeholder role; A decides (Open question 4).

**Table 2: type** (R5 Home density; weights and families per B; numbers stay in Roboto Mono, P5):

| Use | Today | After |
|---|---|---|
| Page title / breadcrumb | `font_title` 20 | `text-xl` 20/30 |
| Buttons, nav rows, card titles, list rows, search text, filter labels, trash rows, section headings in content | `font_body*` 11 | `text-sm` 14/20 |
| Card meta, template and hub descriptions, sidebar section heading, version pill, badges | `font_caption*` 10, literal 10/12 | `text-xs` 12/18 |
| Workspace name | literal 12 medium (`:913`) | `text-sm` 14/20 (weight per B) |
| Empty state title | `font_large_strong` 13 | `text-md` 16/24 |

**Table 3: sizes and their layout effect** (R1 quantified; "fits" means the new line height plus Untitled padding fits in today's box):

| Element | Line | Today | After | Δ height | Layout effect |
|---|---|---|---|---|---|
| Header buttons | `:1402` | 32 | button sm 36 | +4 | none: centred in the 64 header |
| Filter/sort pills, view toggles | `:1297`, `:1311`, `:1327` | 28 | button sm 36 | +8 | filter band 38 → 46 (`:1334`, same 4 / 6 margins); every content view starts 8 px lower |
| Sidebar search | `:933` | 32 | `Input field · Size=sm` (C; expected 36) | +4 | items below move 4 px down |
| Workspace header | `:903` | 32 | 36 | +4 | items below move 4 px down |
| Nav rows (8 fixed rows plus tree rows) | `:880` | 28 | 36 if Open question 1 = yes, else 28 | +8 each | fixed rows push the project tree down 7 × 8 = 56 px plus the 8 px above; the tree list is a scroll area (`:1007-1010`) and absorbs it. Bottom block (Starred, Trash) keeps its anchor to the bottom edge. Sidebar width unchanged. |
| Starred file rows | `:1104` | 24 | 24 (text-xs 18 fits) | 0 | none |
| Trash count, version pill | `:1130`, `:919` | 16 | `Badge` sm 22 (C) | +6 | none (inside a 28-36 row) |
| File card meta block | `theme.rs:698`, `:2613-2622` | 58 | 62 (12 + 20 + 18 + 12) | +4 per card row | grid rows 4 px taller; `HOME_CARD_H` = 140 + 62 |
| List rows | `HOME_LIST_ROW_H` 44 | 44 | 44 (fits 20) | 0 | none |
| Folder tiles | `HOME_FOLDER_H` 48 | 48 | 48 (fits 20) | 0 | none |
| Trash rows | `:2662` | 52 | 52 (fits 20 + 18) | 0 | none |
| Drafts quick-create tiles | `:2265` | 48 | 50 (6 + 20 + 18 + 6) | +2 | strip 64 → 66 (`drafts_strip_h`, `:2225`) |
| Template cards | `:1447`, CTA `:1584` | 188, CTA 26 | 204, CTA button sm 36 | +16 | template grid rows 16 px taller (`template_rows`, `:1449`) |
| Hub card CTAs | `:1999-2000`, `:2019` | 26 | button sm 36 | +10 | hub card height grows 10 (`hub_rows_h`, `:1897`) |
| Empty state | `:2201-2207` | icon 32, 13 + 11 text | `Empty state · Size=sm` | about +20 | centred; no neighbour |
| Drag ghost | `:549` | 28 | 32 (text-sm 20 + 2 × 6) | +4 | follows the pointer |

Radii: every literal `CornerRadius::same(5|6|8)` and `cr_pill` on controls takes B's `radius-md` 8 (controls) or `radius-full` (badges, pills only where C's anatomy says so); cards take B's card radius (`radius-xl` 12 expected). Today's 43 radius literals in `home/mod.rs` (`spacing_baseline.json`) fall to the allowlisted remainder; nested shapes stay concentric (P4, `theme::concentric`).

**States** (C defines the anatomy; this spec lists which ones Home must show): buttons default, hover, pressed, disabled (none disabled today), focus (focus ring `#F97316`, R4); nav rows default, hover, selected, drop target, focus; inputs default, focus, filled; cards default, hover, selected, drag source; menus open/closed; empty state per section (`empty_state` texts unchanged).

## 8. Edge cases

1. Window at `MIN_WINDOW` 900×600: rule 7 (breadcrumb elides; no overlap). Project sections add "+ New folder", the widest case.
2. Long file and project names: elide with "…" at the new 14/12 sizes (`widgets::elide` today); never overlap the type badge or star.
3. Many projects: the tree scrolls; with Open question 1 = yes the visible tree shrinks by 64 px at 1088 tall (about 2 rows).
4. Starred expanded with 3 files plus a 600 px window: the bottom reserve (`:1004`) must still leave the tree at least 64 px (`:1007`, `max(y + 64)`).
5. Search active: the header title is "Search results" style (`h.title()`); filter and sort still work; Escape clears (rule 8).
6. Trash empty / with items: "Empty trash" shows only with items (`:1249`).
7. Board files: thumbnail keeps its paper colour and dot grid (content); only the "Board" badge restyles.
8. Thumbnail missing (`tex = None`): stage plus a `fg-quaternary` file icon.
9. Selected and hovered card at once: selected border wins (2 px brand), hover adds nothing.
10. Updater card: hidden by Z14 in every build; Home never draws it (spec J owns its style). Do not change `UPDATER_UI_ENABLED`.
11. Drag ghost over a non-drop area: unchanged behaviour.

## 9. Non-goals

- No copy changes (labels such as "+ FigJam board" stay; UX-writing fixes are separate issues).
- No new Home sections, sorting, filters or CLI flags; no change to `designos-library`.
- No file split of `home/mod.rs` beyond deleting the two dead files; moving the inline modules into their own files is a follow-up.
- No change to `updater.rs` (J), `home/dialog.rs` (J), `widgets/*` (C), `panels/mod.rs` scrollbar (C), tab bar (D).
- No icon set change (R6): icons are restyled by colour only.
- Responsive captures beyond 1600×1088 and 900×600 (owner default).

## 10. Test plan (red first)

Shared helper (new name) `chrome_colour_sites(src: &str) -> Vec<usize>`: the census rule of section 2 in Rust (strip from the first `#[cfg(test)]`, skip `//` comments, flag the same constructors and named colours, skip a named `Color32::WHITE` on a line containing `.image(` or `with_texture`). The gate's owner is spec A (D proposes `scripts/check_chrome_colour_literals.py`, new name); a Rust test is added as well because the Python design gates are advisory in CI and only `cargo test` on macOS blocks a merge. If no helper exists when this PR starts, add it to `crates/designos-app/src/shell/tests.rs` (one function, test-only); specs I, J and K reuse it.

| Case (new names unless marked) | Where | Expected | Fails before |
|---|---|---|---|
| `home_sources_paint_no_chrome_colour_literals` | `shell/tests/home.rs` | `chrome_colour_sites(include_str!("../../home/mod.rs"))` returns only the two allowlisted tint lines; 0 for `home/thumb.rs` | yes (38) |
| `home_has_no_figma_blue_or_ui3_palette` | same | shipped `home/mod.rs` contains neither `UI3_` nor `PrimaryBlue` nor `0x0D, 0x99, 0xFF` | yes |
| `home_primary_button_is_brand_with_dark_label` | same | a `home_sim()` frame records `home-new-file`; the button's fill/label pair returned by `main_view::button_colours(Kind::Primary)` (new name, a pure fn the painter uses) equals A's brand fill and the R4 label | yes |
| `home_header_buttons_are_button_sm` | same | recorded `home-new-file`, `home-import` rects are 36 tall and vertically centred in the 64 header | yes (32) |
| `home_filter_bar_controls_are_button_sm` | same | `home-filter`, `home-sort`, `home-view-grid` rects 36 tall; content top = header 64 + 46 + 1 | yes (28, 38) |
| `home_layout_regions_unchanged` (guard) | same | sidebar right edge 264; header 64; first card x = sidebar + 32; card width 224 | guard |
| `home_breadcrumb_never_overlaps_header_buttons` | same | `Sim` at 900×600 in a project section with a long name: breadcrumb right edge + 12 ≤ leftmost button left | yes at 14 px type |
| `home_sidebar_nav_rows_follow_home_density` (only if Open question 1 = yes) | same | `home-recents`, `home-trash` rects 36 tall | yes (28) |
| existing `home::tests::*` (`home/mod.rs:2738`, `:2814`, `:2855`) and `shell::tests::home::*` (marked) | — | pass unchanged | guard |
| `scripts/zero_fake_ui_gate.py` Z09, Z14 (marked) | — | PASS | guard |
| `scripts/check_spacing_relationships.py` (marked) | — | PASS; then `--write-baseline` lowers `home/mod.rs` and removes the two dead entries; it refuses any rise | guard |
| A's theme tests and G01 parity (marked) | `theme.rs` tests, `design_os_self_evolve.py` | PASS | A's |

Run: `cargo test -p designos-app --bin designos home`, `cargo test -p designos-app --bin designos shell::tests::home`, `python3 -B scripts/check_spacing_relationships.py`, `python3 -B scripts/zero_fake_ui_gate.py`, the section 2 census, `cargo fmt --all --check`, `cargo clippy -p designos-app --all-targets -- -D warnings`.

**Acceptance criteria.**

1. The section 2 census prints `chrome_sites=2` for `home/mod.rs` (lines of the two texture tints only) and 0 for the other live `home/` files; `home/sidebar.rs` and `home/main_view.rs` no longer exist and the crate builds.
2. "+ Design file" renders `#E36323` with a `#0A0A0A` label; no `#0D99FF` pixel remains in Home chrome (selected card included) in the after screenshots.
3. Layout: L's gate passes for Home (same regions, widths and order); every height change is one listed in section 7 table 3.
4. Type: Home text uses `text-sm`/`text-xs`/`text-md`/`text-xl` from B; `home/mod.rs` has no font-size literal (`check_spacing_relationships.py --verbose` lists no `font_literals` for it).
5. Rule 7 holds at 900×600 (test and screenshot `H-03`).
6. All section 10 tests pass; Z09 and Z14 pass; the spacing ratchet passes and its baseline only went down.
7. The PR shows before and after for every command below, side by side, plus manual captures (labelled manual) of the states the CLI cannot reach.

**Evidence commands.** Each was run once on `ec47da26` by the spec writer on 2026-10-25; exit codes shown. `S` is a scratch folder; never point `--library` at the real library. Run the same list after the change and put both sets in the PR.

```bash
export PATH="$HOME/.cargo/bin:$PATH"; S="$(mktemp -d)"; OUT="$S/out"; mkdir -p "$OUT" "$S/first-run"
RUN() { cargo run -q --release -p designos-app --bin designos -- "$@"; echo "exit $?"; }
RUN --seed-library "$S/lib"                                                                                   # exit 0: writes 4 sample files (project "Marlow")
RUN --home --library "$S/lib" --screenshot "$OUT/H-01-home-recents.png" --size 1600x1088 --no-toast          # exit 0: sidebar, header, filter bar, 4 cards (incl. a board)
RUN --home --library "$S/first-run" --screenshot "$OUT/H-02-home-first-run.png" --size 1600x1088 --no-toast  # exit 0: first run seeds one "Welcome" file (an empty library is never empty)
RUN --home --library "$S/lib" --screenshot "$OUT/H-03-home-min-900x600.png" --size 900x600 --no-toast        # exit 0: minimum window, header crowding (rule 7)
```

Observed while capturing (all specs H–K, 27 runs under parallel load from other agents): 5 runs printed `designos: screenshot was not captured` and exited 1; every retry of the same command exited 0. If that happens, re-run the command once and note it in the PR. This is plan.md's unconfirmed dogfood gap, now reproduced as load-dependent.

Not capturable from the CLI today (no flag selects a Home section, the list view, a hover or a menu): Templates & Specs, Drafts, All folders, Resources & Libraries, Starred, Trash (with and without items), list view, filter and sort menus open, context menu, drag ghost, empty states, focus rings. The PR covers them with the tests above and manual window captures labelled manual. A `--home-section` flag would be a separate issue.

## 11. Performance budgets

- No new allocation per frame beyond today's (colour constants are `const`); no `doc.to_json()` (CLAUDE.md invariant 1); Home draws no document.
- Thumbnail loading budget unchanged (`budget = 4u32` textures per frame, `:1465`, `:1914`, `:2241`).
- Idle: Home requests no repaint when idle (unchanged).

## 12. Dependencies, ownership and build order

**Build order.** L captures the Home baseline (`H-01`..`H-03` above are also L's inputs) → A and B land the colour roles and Home sizes → C lands the widgets → this spec. If C is not merged, H paints the anatomy with A/B constants inside `home/mod.rs` and swaps to C's widgets in a follow-up only if C's API differs; do not wait on C for colours.

**Technical decisions recorded here** (the owner may reverse): (1) delete the two dead files rather than move the inline modules into them (smallest diff, no behaviour); (2) Home sidebar uses the editor left-panel surface `bg-secondary` so Home and editor read as one app (specs D/E use the same pairing); (3) the Rust literal test is added even though A owns the Python gate, because only `cargo test` blocks a merge; (4) hub, quick-create and template accents follow the same categorical mapping as the file-type badges (one rule, Open question 2).

**File ownership across H–K.** Each file has exactly one owner.

| File | Owner | Note |
|---|---|---|
| `home/mod.rs` | H | the updater card call (`:1122`) draws J's `updater.rs`; the menus call C's `widgets::menu` |
| `home/thumb.rs`, `home/time.rs`, `home/session.rs` | H | no visual change expected |
| `home/sidebar.rs`, `home/main_view.rs` | H | deleted (dead) |
| `shell/tests/home.rs` | H | tests only |
| `home/dialog.rs` | J | modal dialogs shared by Home and the editor (`dialog.rs:1-2`); Home density applies there too |
| `updater.rs` (module `crate::home::updater`, `home/mod.rs:12-13`) | J | the task brief assigns the updater dialog to J; spec D's not-mine list says H, J takes it |
| `terminal.rs`, `agent_*.rs`, `canvas/agent_*.rs`, `skills/*`, `image_jobs.rs` | I | |
| `cmdk.rs`, `find/mod.rs`, `find/panel.rs`, `find/select.rs`, `comments/*`, `history/*`, `present.rs`, `panels/context.rs` | J | |
| `panels/assets.rs`, `variables/*`, `libraries/*`, `styles.rs`, `plugins/*` | K | |

Not mine (D–G or the kit specs): `theme.rs`, `design/*` (A, B); `widgets/*` except `widgets/picker.rs`, `panels/mod.rs` (C); `tabs.rs`, `shell.rs` (including its toast, which D paints by J's rules), `panels/rail.rs`, `panels/toolbar.rs`, `panels/menus.rs`, `canvas/rulers.rs` (D); `panels/left.rs`, `panels/layers.rs`, `find/filter.rs` (E); `panels/right.rs`, the inspector files, `widgets/picker.rs`, `panels/style_picker.rs`, `panels/style_edit.rs`, `panels/style_edit_grid_rows.rs` (F); `dev/*`, `specs.rs` (G).

Unassigned in D–K (flag to the controller, recommended owner in brackets): `panels/check_designs.rs` [K], `decks/ui.rs` [D], `board/*` [D], `canvas/draw.rs`, `canvas/grids.rs`, `canvas/tiles.rs`, `canvas/interact.rs` [D or L], `icons.rs` [C, R6], `lint.rs` [K].

**Open PRs touching H files** (`gh pr list --state open`, 2026-10-25, full file lists from `gh api repos/{owner}/{repo}/pulls/<n>/files`): stale #765, #768, #770 (opened 2026-10-19, `mergeable: CONFLICTING`) touch `home/mod.rs`, `home/dialog.rs`, `home/session.rs`; the PM decides their fate before H starts. No other open PR touches `home/`.

## 13. Owner rulings that apply

- R1: "sử dụng 100% style kiểu của Untitled UI nhưng vẫn giữ cấu trúc layout và bố cục hiện tại" (all of Untitled's style; keep layout and arrangement).
- R2: take Untitled spacing and typography as they are, adapted to Studio's layout.
- R3: Studio improves itself; no Figma. Evidence goes in the PR.
- R4: brand = Untitled anatomy with Studio orange `#E36323`, dark label `#0A0A0A`; focus ring `#F97316`; Purple Ban stays (every kit brand role resolves to the orange family, never the kit purple `#7F56D9`).
- R5: Home and dialogs use sm/md (text-sm 14/20, button sm 36 / md 40); the 2 px and 6 px spacing ban is lifted.
- R6: keep the icon set; restyle colour to Untitled fg roles.
- R7: spec-driven; the PM coordinates delivery.
- Standing: `main` is protected (PR, green macOS test, clippy, rustfmt, squash); visual evidence in every PR (2026-09-22).

## Open questions for the owner

1. **Sidebar row height.** R5 sets Home buttons to sm 36 but does not name the sidebar row. Recommended: 36 (same as button sm, Untitled `_Nav item base` at the small size); the project tree scrolls and absorbs the 64 px. Alternative: keep 28 and let only the type grow (14/20 fits).
2. **Category colours** (file-type badges, template, hub and quick-create accents, the "P" disc). Recommended: Untitled `Featured icon` tiles in utility colours (design `utility-blue`, board `utility-amber`, deck `utility-green`, spec bundle `utility-orange`), so file kinds stay recognisable without solid saturated tiles. Alternative: one neutral gray featured icon for every kind (maximum restraint, kinds told apart by glyph only).
3. **Selected file card.** Recommended: 2 px `border-brand` in Studio orange `#E36323` (Untitled's selected-card pattern). Alternative: neutral `border-primary #404040` 2 px (P2's neutral selection, weaker on dark thumbnails).
4. **Placeholder contrast.** The kit's `text-placeholder #737373` reads 4.18:1 on `bg-primary`. Recommended: keep the kit value (placeholders are not required text under WCAG 1.4.3 when a label exists; the search field has an icon and is self-explanatory). Alternative: `text-tertiary #A3A3A3` for placeholders. Spec A decides this once for every input.
