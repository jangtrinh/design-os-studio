# H Restyle the Home workspace in Untitled UI dark (Home density)

| Field | Value |
|---|---|
| Issue | #1138 (epic #1118, spec key H) |
| Size | M. About two days: one live 2,906-line file (`home/mod.rs` at `205c72ef`) with 38 chrome colour sites and 12 derived tints, Home-density sizes (R5) on four control families, card and tile heights re-fitted to the new type, two dead files removed, new tests. |
| Depends on | A (colour roles, Studio brand override, constant names in `theme.rs`), B (type, spacing, radius and control-size scale; `HOME_*` constants), C (widget anatomy: `Buttons/Button`, `Input field`, `Dropdown menu`, `Badge`, `_Nav item base`, `Empty state`, `Featured icon`) and L (baseline capture and layout-unchanged gate) |
| Unblocks | M (docs and screenshot refresh) |
| Touches | `crates/designos-app/src/home/mod.rs` (drag ghost `:548-599`; inline `mod sidebar` `:872-1206`; inline `mod main_view` `:1211-2729`); deleted: `home/sidebar.rs`, `home/main_view.rs` (dead, never compiled); tests: `crates/designos-app/src/shell/tests/home.rs`, the shared helper file `crates/designos-app/src/shell/tests/chrome_census.rs` (new name, section 10) and its one-line `mod` registration in `shell/tests.rs` (L's file); `scripts/spacing_baseline.json` (lowered with `--write-baseline`, never raised). Reads only (owned by A and B): `theme.rs:691-701` (`HOME_*`). |

Code baseline: every `file:line` is at `origin/main@205c72ef`, read with `git show 205c72ef:<path>` (never a checkout). The first draft cited `ec47da26`; every citation was moved by a line diff of the two commits, and the few whose code changed between them were placed by hand (marked "placed by hand"). Since `ec47da26`, main changed `home/mod.rs` by 177 lines and `home/session.rs` by 178, and added `home/leave.rs`, `home/unsaved.rs` and `home/spec_bundle_hidden_tests.rs`. Re-anchor against the commit you branch from before writing code (search for the quoted code).

Implementer note: this spec is self-contained. Load the `es-designer` skill before writing UI code (section 7). Colours are named by their Untitled UI token name and hex; the Rust constant names come from the token contract (`specs/token-contract.md`, specs A and B), sizes from spec B, widget anatomy from spec C. Where A, B or C are merged before you start and differ from the expectations written here, A, B and C win; report the difference in the PR.

## 1. Summary and user story

As a designer who opens Studio, I land on Home and see it in the Untitled UI dark style: neutral surfaces (`bg-primary #0A0A0A`, `bg-secondary #171717`), hairline borders, Untitled buttons, inputs, menus and badges at the Home density of R5 (text-sm 14/20, button sm 36 / md 40), and the Studio orange `#E36323` with a dark `#0A0A0A` label on the one primary action ("+ Design file"), not today's blue. Home keeps its layout: the 264 px sidebar on the left, the header with title and action buttons, the filter/sort bar, the grid or list of files, the Trash at the bottom of the sidebar, and every section in today's order (R1).

## 2. Current state (FACT unless labelled)

**Files.** Measured with the census command below, plus `wc -l`.

| File | Lines | Compiled | `theme::` lines | Chrome colour sites | Site lines |
|---|---|---|---|---|---|
| `home/mod.rs` | 2,906 | yes | 182 | 38 | 561, 883-886, 888, 891, 909, 910, 1130, 1199, 1202, 1221-1228, 1419, 1569, 1598, 1733, 1734, 2015, 2035, 2190, 2196, 2434, 2440, 2513, 2536, 2550, 2556, 2565, 2566, 2596 |
| `home/thumb.rs` | 201 | yes | 1 | 0 | (`:153` builds a `ColorImage` from pixels: not a colour) |
| `home/time.rs` | 67 | yes | 0 | 0 | logic only |
| `home/session.rs` | 1,628 | yes | 0 | 0 | logic only (autosave, CLAUDE.md invariant 1) |
| `home/leave.rs`, `home/unsaved.rs` | 318, 144 | yes (`home/mod.rs:9`, `:15`) | 0 | 0 | logic only; not in the census list because they paint nothing |
| `home/sidebar.rs` | 214 | **no** | 20 | 0 | dead file |
| `home/main_view.rs` | 544 | **no** | 82 | 7 | dead file |

`home/sidebar.rs` and `home/main_view.rs` are dead: `home/mod.rs` declares `mod sidebar { … }` (`:872`) and `mod main_view { … }` (`:1211`) inline, so Rust never reads the two files, and nothing else references them (`grep -rnE 'sidebar\.rs|main_view\.rs|include!' crates/designos-app/src` finds nothing). They still have entries in `scripts/spacing_baseline.json` (`home/main_view.rs` 47 offset + 1 radius literals, `home/sidebar.rs` 14 offset literals). Editing them changes nothing on screen.

Census command (shared with specs D–G; run from the repo root; writes nothing). A *chrome site* is a `Color32` constructor with literal arguments, a `from_gray`/`from_black_alpha`/`from_white_alpha` call, or a named `Color32` colour other than `TRANSPARENT`, in non-test code (comments and strings blanked and `#[cfg(test)]` items dropped by `scripts/rust_literal_scan.clean`):

```bash
# REV=205c72ef reads that commit with `git show` (no checkout); drop it to read your working tree
REV=205c72ef PYTHONDONTWRITEBYTECODE=1 python3 - home/mod.rs home/thumb.rs home/time.rs home/session.rs home/sidebar.rs home/main_view.rs <<'PY'
import re, sys, os, subprocess
sys.path.insert(0, "scripts")
from rust_literal_scan import clean
REV = os.environ.get("REV")  # set: read that commit with `git show` (no checkout); unset: read the working tree
def read(rel):
    p = "crates/designos-app/src/" + rel
    if REV:
        return subprocess.run(["git", "show", f"{REV}:{p}"], capture_output=True, text=True, check=True).stdout
    return open(p, encoding="utf-8").read()
HEX = re.compile(r"Color32::from_rgba?(?:_unmultiplied|_premultiplied)?\(\s*(?:0x[0-9A-Fa-f]+|\d+)\s*,"
                 r"|Color32::from_gray\(\s*(?:0x[0-9A-Fa-f]+|\d+)\s*\)|Color32::from_(?:black|white)_alpha\("
                 r"|Color32::(?:WHITE|BLACK|GRAY|LIGHT_GRAY|DARK_GRAY|RED|GREEN|BLUE|YELLOW)\b")
CONV = re.compile(r"Color32::from_rgba?(?:_unmultiplied|_premultiplied)?\((?!\s*(?:0x[0-9A-Fa-f]+|\d+)\s*,)")
for rel in sys.argv[1:]:
    code = clean(read(rel)).split("\n")
    hexs = [i + 1 for i, l in enumerate(code) for _ in HEX.finditer(l)]
    conv = [i + 1 for i, l in enumerate(code) for _ in CONV.finditer(l)]
    print(rel, "theme_lines=%d" % sum("theme::" in l for l in code), "chrome_sites=%d" % len(hexs), sorted(set(hexs)), "computed=%s" % sorted(set(conv)))
PY
```

plan.md's figure for `home/mod.rs` is 19 (`grep -c 'Color32::from_rgb(…'`, test code included). It counts only `Color32::from_rgb` calls; the census above also counts named colours (`Color32::WHITE`) and alpha constructors, so it reads 38. Both are correct for their definition; this spec uses the census.

Derived tints: `home/mod.rs` has 12 `gamma_multiply(` calls on chrome colours (`grep -c 'gamma_multiply(' crates/designos-app/src/home/mod.rs` = 12, all in shipped code): accent tints on cards, banners and icon boxes, and `theme::DANGER.gamma_multiply(0.2)` for the danger hover (`:1428`).

**Why the primary action is blue.** P2 (`design/principles.md:10-19`) and the soul (`design/soul.md:15`, `:32`) say the one primary action per view is orange `#E36323`. Home ignores `theme::BG_BRAND` (`theme.rs:36`) because both inline modules define their own Figma-UI3 copies: `UI3_BLUE = #0D99FF` (`home/mod.rs:891`, `:1224`) and `UI3_BLUE_HOVER = #0B84E0` (`:1225`), and the header button kind is `Kind::PrimaryBlue` (`:1239`, used `:1262`, painted `:1416-1420` with a white label). The same `UI3_*` block defines Zinc copies of theme colours (`UI3_SIDEBAR_BG #18181B`, `UI3_SURFACE #1E1E22`, `UI3_PILL_ACTIVE`/`UI3_BORDER #27272A`, `UI3_PILL_HOVER #232329`, `UI3_MAIN_BG #121215`, `:883-891`, `:1221-1223`) and four category accents (`UI3_TEAL #14AE5C`, `UI3_AMBER #F59E0B`, `UI3_ORANGE #FF6A3D`, plus blue, `:1226-1228`).

**Regions** (all in `home/mod.rs`; screenshot `H-01` in section 10):

- **Frame** `show` (`:437-468`): fills `theme::BG`, splits sidebar `HOME_SIDEBAR_W` 264 (`theme.rs:691`) and main.
- **Sidebar** (`:893-1136`): fill `UI3_SIDEBAR_BG`, right hairline `UI3_BORDER` (`:895-896`).
  - Workspace header 32 tall (`:901-923`): avatar tile 20 with literal `#27272A` fill and `#3F3F46` stroke and an orange folder icon (`:908-911`); name "Personal Workspace" in a literal `FontId::new(12.0, …)` (`:912-913`); version pill 16 tall in mono with `ver_font.size = 10.0` (`:914-920`); "Workspace activity" icon button 24 (`:926-928`, C's `icon_button`).
  - Search field 32 (`:931-956`): `UI3_SEARCH_BG` + `UI3_BORDER`, icon `ICON_SECONDARY`, text `TEXT`, hint `TEXT_TERTIARY`.
  - Nav rows `ROW_H` 28 with 2/1 px gaps (`:879`, `:958-998`): Recents, Templates & Specs, divider, "FILES & PROJECTS" heading with a blue "P" disc (`:974-977`) and a `+` icon button 24 (`:978-982`), Drafts, All folders, Resources & Libraries.
  - Project tree (`:1005-1075`), scrolling, rows 28, "No projects yet" (`:1018`), chevrons `ICON_TERTIARY`.
  - Starred row with expander and up to 3 file rows 24 with blue file icons (`:1077-1116`).
  - Updater card (`:1118-1121`): `updater.rs` (spec J), hidden while `UPDATER_UI_ENABLED` is false (Z14). Not drawn today.
  - Trash row with a count badge 22×16 literal `#27272A` (`:1123-1135`).
  - Row painter `row` (`:1186-1205`): drop target `BG_SELECTED_HOVER`, selected `UI3_PILL_ACTIVE`, hover `UI3_PILL_HOVER`, radius `CornerRadius::same(6)`; selected icon and label `Color32::WHITE` (`:1199`, `:1202`), label `font_body_strong` when selected.
- **Main header** `HOME_HEADER_H` 64 (`:1250-1298`): breadcrumb in `font_title` 20 (`:1361-1404`); right-aligned buttons 32 tall, pill radius, `font_body_strong` (`button`, `:1406-1436`): "+ Design file" `PrimaryBlue`, "+ FigJam board", "+ Spec bundle", "Import" `Secondary` (`UI3_SURFACE` + `UI3_BORDER`), "+ New folder" in project sections, "Empty trash" `Danger` (transparent, `theme::DANGER` stroke and text) in Trash.
- **Filter bar** (`:1300-1343`): `Filter: …` and `Sort: …` pills 28 tall with chevrons (`:1305`, `:1319`), `List view`/`Grid view` icon buttons 28 (`:1332-1341`); band height 38 (`:1342`) then a hairline (`:1344`). Menus are `widgets::menu::popup_menu` (spec C, `:470-505`).
- **Content views**: Templates & Specs (`:1449-1643`: six 240×188 cards with accent banner, category pill, CTA 26 tall); All folders (`:1649-1896`: project cards with 2×2 previews, "+ Folder" mini button in black alpha 170 / blue, "+ New project" card, teal folder tiles); Resources & Libraries (`:1897-2115`: four hub cards with accent CTAs 26 tall and white labels `:2015`, `:2035`); files view (`:2223-2510`: Drafts quick-create strip of three 48-tall tiles with blue/amber/teal accents `:2269-2305`, folder tiles `HOME_FOLDER_H` 48, file grid, "Getting started" starter kits `:2453-2463`); Trash (`:2668-2729`: note line, rows 52 `:2675`, "Restore" secondary buttons).
- **File card** `card` (`:2603-2636`): 224 × (140 + 58) (`theme.rs:694-697`), radius 8, fill `UI3_SURFACE`, hover stroke `TEXT_TERTIARY`, selected 2 px `UI3_BLUE` (`home/mod.rs:2613-2619`); thumbnail stage literal `#18181B` (`:2556`) with the picture inset 6 and a white-alpha-18 frame (`:2565-2566`); board thumbnails on `BOARD_BG` with a "Board" badge (`:2532-2550`, `:2572-2581`); file-type badge 20×20 in blue/amber/teal/orange with a white glyph (`:2583-2601`); title `font_body_strong` 11 at `thumb.bottom() + 17`, meta `font_caption` 10 at `+16` (`:2626-2635`). Star overlay: black-alpha circles (`:2190`, `:2434`) and a `#FFCD29` star (`:2513`); hover scrim black alpha 90 (`:2196`, `:2440`).
- **List row** `list_row` (`:2643-2666`): `HOME_LIST_ROW_H` 44, selected `UI3_SURFACE` + 1 px `UI3_BLUE`, hover `BG_HOVER`, 48-wide thumbnail, columns from `list_columns` (`:2638-2641`).
- **Empty state** `empty_state` (`:2201-2221`): 32 px file icon `ICON_TERTIARY`, title `font_large_strong` 13, body `font_body` 11 wrapped at 380.
- **Drag ghost** (`:548-599`): 28-tall pill in `BG_BRAND` with a white label (`:559-561`). White on `#E36323` is 3.46:1 (plan.md facts), below 4.5.

**Gates today.** No gate counts colour literals. `scripts/check_spacing_relationships.py` ratchets literal geometry per file against `scripts/spacing_baseline.json` (`home/mod.rs`: 162 offset, 43 radius, 2 font literals; it passed on the first draft's `ec47da26`, exit 0; not re-run at `205c72ef` because that needs a checkout). `scripts/zero_fake_ui_gate.py` Z09 checks `home/mod.rs` (`Act::ExportFig` writes bytes; sections filter distinctly) and Z14 the hidden updater card. `.github/workflows/design-gates.yml` runs those Python gates as advisory (`continue-on-error: true`, `:4-5`, `:21`); only the macOS test, clippy and rustfmt jobs block a merge (CLAUDE.md, Git Workflow). Home tests: `home/mod.rs:2732-2906` and `shell/tests/home.rs`.

**Decided.** Home sidebar rows are 36 (plan.md, controller default from the 21:35 sweep: "Home sidebar rows 36 (R5 Home = sm) — H OQ1"). **UNKNOWN.** The exact Untitled `Input field` height at Size=sm (C and B measure it from the kit).

## 3. Behaviour rules

1. **Layout unchanged (R1).** Same regions, same order, same widths: sidebar 264, main pad `HOME_PAD` 32, header 64, card width 224 and gap 24, list columns. Heights grow only where R5 raises a control or type size, by the amounts in section 7 table 3. No new control, no removed control, no new copy.
2. **One brand action.** "+ Design file" is `Buttons/Button · Size=sm · Hierarchy=Primary`: fill `bg-brand-solid` with the Studio override `#E36323`, label `#0A0A0A` (R4, 5.73:1), hover `bg-brand-solid_hover` (Studio override; A names it). No other Home control uses the brand fill. `Kind::PrimaryBlue` and every `UI3_*` constant leave the file.
3. **No chrome literal.** After the change the census prints `chrome_sites=2` for `home/mod.rs`, both on the allowlist (`:2536` and `:2565`: `Color32::WHITE` as a texture tint inside `RectShape::filled(..).with_texture`, which multiplies the thumbnail by 1 and is not a colour role), and 0 for `home/thumb.rs`, `home/time.rs`, `home/session.rs`. Every other colour is a `theme::` constant from spec A. Derived tints (`gamma_multiply`) on chrome colours are replaced by the Untitled role that exists for that state (section 7 table 1); a tint stays only where Untitled has no role and A defines it as a named constant.
4. **Every white-on-brand label becomes dark.** The drag ghost label (`:561`) and any label painted on `bg-brand-solid` use the R4 dark label `#0A0A0A`.
5. **Neutral selection (P2).** The selected sidebar row and the selected list row use a neutral fill; the selected grid card uses a 2 px `BORDER_SELECTED` `#FB923C` border (R10.3), never `#0D99FF`.
6. **Dead files go.** Delete `home/sidebar.rs` and `home/main_view.rs` in the same PR (technical decision, section 12). The build proves they were dead: it compiles without them.
7. **Header never overlaps.** At every window width down to `MIN_WINDOW` 900×600 (`theme.rs:604`), the breadcrumb ends at least `spacing-lg` 12 px left of the leftmost header button; a longer breadcrumb elides its leading segments with "…" first, then the last segment. Today at 900 px there is about 60 px between "Recents" and "Import" (screenshot `H-03`); the 14 px type adds about 80 px to the button row, so without this rule the title and buttons collide.
8. **Behaviour unchanged.** Every click, drag, drop, context menu, keyboard path (Escape clears search `:454-463`), recorded widget id (`home-new-file`, `home-filter`, `home-sort`, `home-view-list`, `home-view-grid`, `home-search`, `home-<nav key>`, `home-trash`, `home-template-*`, `home-drafts-*`, `home-starter-*`, `home-res-*`) and every `Act` stay as they are. Z09 stays green.

## 4. Data model / schema changes

None. No document, library, settings or file-format change.

## 5. Contracts

- **Theme constants consumed** (token contract `specs/token-contract.md` names; Untitled role in brackets): `BG` (bg-primary), `BG_SECONDARY` (bg-secondary; also the kit's bg-primary_alt, same value `#171717`), `BG_HOVER` (bg-primary_hover), `BG_SECONDARY_HOVER` (bg-secondary_hover, new in the contract), `BG_TERTIARY` (bg-tertiary), `BG_SELECTED` (bg-secondary_hover), `BG_BRAND` / `BG_BRAND_HOVER` / `BG_BRAND_PRESSED` (bg-brand-solid, Studio override), `BG_ERROR_SOLID` (bg-error-solid), `BORDER` (border-secondary), `BORDER_STRONG` (border-primary), `BORDER_SELECTED` (border-brand, contract value Orange-400 `#FB923C`), `DANGER` (fg-error-primary; also the destructive outline, the kit's border-error_subtle has the same value `#EF4444`), `TEXT` (text-primary), `TEXT_SECONDARY`, `TEXT_SECONDARY_HOVER`, `TEXT_TERTIARY`, `TEXT_PLACEHOLDER` (kit `#737373`, R8.1), `TEXT_ERROR` (text-error-primary), `TEXT_WARNING` (star on; the kit's fg-warning-secondary has the same value `#FACC15`), `ICON_BRAND_PRIMARY` (fg-brand-primary: workspace avatar icon), `TEXT_ON_BRAND` (R4 dark label), `ICON_SECONDARY` (fg-secondary), `ICON_QUATERNARY_HOVER` (fg-quaternary_hover), `ICON_QUATERNARY` (fg-quaternary), `FOCUS_RING`, `BOARD_BG` / `BOARD_DOT` (content). Gray badges use `UTILITY_NEUTRAL_50` + `UTILITY_NEUTRAL_200` + `UTILITY_NEUTRAL_700` (contract N21–N23). Also from the contract: `BG_ERROR_PRIMARY` (N26, destructive hover) and `BG_OVERLAY` (N33, star backdrop and thumbnail hover scrim). Category colours (R10.2, contract N39–N46): tiles `UTILITY_BLUE_50`, `UTILITY_AMBER_50`, `UTILITY_GREEN_50`, `UTILITY_ORANGE_50`; glyphs `UTILITY_BLUE_600`, `UTILITY_AMBER_600`, `UTILITY_GREEN_600`, `UTILITY_ORANGE_600`.
- **Size constants consumed** (token contract (c) names; Untitled step in brackets): `FONT_SIZE_TEXT_XS` / `LINE_TEXT_XS` (text-xs 12/18), `FONT_SIZE_TEXT_SM` / `LINE_TEXT_SM` (text-sm 14/20), `FONT_SIZE_TEXT_MD` / `LINE_TEXT_MD` (text-md 16/24), `FONT_SIZE_TEXT_XL` / `LINE_TEXT_XL` (text-xl 20/30), `CONTROL_SM` (button sm 36), `SPACE_FACT` (spacing-xs 4), `SPACE_INLINE` (spacing-md 8), `SPACE_GROUP` (spacing-lg 12), `RADIUS_CONTROL` (radius-md 8), `theme::cr_pill` (radius-full). spacing-sm 6 has no contract constant; none of H's tables uses a 6 px step, so H needs none (if a later change does, spec B adds `SPACE_SM = 6.0`, new name; R5 lifts the 6 px ban). `HOME_*` constants (`theme.rs:691-701`) are A/B's file: the token contract keeps every `HOME_*` value (appendix, `HOME_CARD_H` 198), so H computes the taller card in `home/mod.rs` from a local `CARD_META_H = 62.0` (new name) as `HOME_THUMB_H + CARD_META_H` and stops reading `HOME_CARD_H` (section 7 table 3; decided, OQ5); `HOME_THUMB_BG` (`theme.rs:701`, unused here: `let _ = theme::HOME_THUMB_BG` at `home/mod.rs:2555`) is revalued by the contract to `#262626` and H leaves it alone.
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
| Nav row hover / selected / drop target | `#232329` / `#27272A` / `BG_SELECTED_HOVER` | `bg-secondary_hover` / `bg-tertiary` / `bg-tertiary` + 1 px `border-brand` (Studio override) | `#262626` / `#262626` / `#262626` + `#FB923C` |
| Nav label / selected label | `TEXT` / `WHITE` | `text-secondary` / `text-primary` (weight per B) | `#D4D4D4` / `#FAFAFA` |
| Nav icon / selected icon | `ICON_SECONDARY` / `WHITE` | `fg-quaternary` (hover `fg-quaternary_hover`) / `fg-secondary` | `#737373` (`#A3A3A3`) / `#D4D4D4` |
| Section heading "FILES & PROJECTS", chevrons | `TEXT_SECONDARY`, `ICON_TERTIARY` | `text-tertiary`, `fg-quaternary` | `#A3A3A3`, `#737373` |
| Primary button | `#0D99FF` / `#0B84E0`, white label | `bg-brand-solid` / `_hover` (Studio override), label R4 dark | `#E36323` / `#F97316`, `#0A0A0A` |
| Secondary button | `#1E1E22` + `#27272A`, hover `#27272A` | `bg-primary` + `border-primary`, label `text-secondary`; hover `bg-primary_hover`, label `text-secondary_hover` | `#0A0A0A` + `#404040`, `#D4D4D4`; `#171717`, `#E5E5E5` |
| Destructive button ("Empty trash") | transparent + `DANGER #FE5938` | `Buttons/Button destructive · Hierarchy=Secondary`: `border-error_subtle`, `text-error-primary`, hover `bg-error-primary` | `#EF4444`, `#F87171`, `#450A0A` |
| Filter/sort pills | `#1E1E22` + `#27272A` | secondary button (row above) with a trailing chevron in `fg-quaternary` | as above |
| Card, folder tile, quick-create tile, hub and template card | `#1E1E22`, stroke `#27272A`, hover `TEXT_TERTIARY` | `bg-primary_alt` + `border-secondary`; hover `border-primary` | `#171717` + `#262626`; `#404040` |
| Selected card | 2 px `#0D99FF` | 2 px `BORDER_SELECTED` (border-brand, Studio override; R10.3) | `#FB923C` |
| Thumbnail stage | `#18181B`, picture frame white α 18 | `bg-primary`, frame `border-secondary` | `#0A0A0A`, `#262626` |
| Card title / meta | `TEXT` / `TEXT_SECONDARY` | `text-primary` / `text-tertiary` | `#FAFAFA` / `#A3A3A3` |
| File-type badge, template accents, quick-create and hub accents, "P" disc, starred-file icon | `#0D99FF`, `#F59E0B`, `#14AE5C`, `#FF6A3D` + white glyph | `Featured icon` with utility colours (R10.2): design file and the "P" disc `UTILITY_BLUE_50` / `UTILITY_BLUE_600`, board `UTILITY_AMBER_50` / `UTILITY_AMBER_600`, deck `UTILITY_GREEN_50` / `UTILITY_GREEN_600`, spec bundle `UTILITY_ORANGE_50` / `UTILITY_ORANGE_600` (tile / glyph) | blue `#172554`/`#60A5FA`, amber `#451A03`/`#FBBF24`, green `#052E16`/`#4ADE80`, orange `#431407`/`#FB923C` |
| Star (on) | `#FFCD29` | `fg-warning-secondary` | `#FACC15` |
| Star backdrop, hover scrim | black α 80/150, α 90 | `BG_OVERLAY` (contract N33, bg-overlay at 70 %) for both the star backdrop and the hover scrim; technical call: one scrim role instead of two black alphas | `#262626` @ 70 % |
| Version pill, Trash count, "Board" badge | `#27272A` / `BG_MENU` + `TEXT_SECONDARY` | `Badge · Color=Gray`: `UTILITY_NEUTRAL_50` fill, `UTILITY_NEUTRAL_200` border, `UTILITY_NEUTRAL_700` text | `#171717`, `#404040`, `#D4D4D4` |
| Workspace avatar tile | `#27272A` + `#3F3F46`, orange icon | `bg-tertiary` + `border-primary`, icon `fg-brand-primary` → `ICON_BRAND_PRIMARY` (Studio override) | `#262626` + `#404040`, `#F97316` |
| Empty state | `ICON_TERTIARY`, `TEXT`, `TEXT_SECONDARY` | `Empty state`: `Featured icon · Color=Gray`, title `text-primary`, body `text-tertiary` | `#FAFAFA`, `#A3A3A3` |
| Drag ghost | `BG_BRAND` + white | `bg-brand-solid` + R4 dark label | `#E36323` + `#0A0A0A` |
| Breadcrumb current / parent / separator / hover | `TEXT` / `TEXT_SECONDARY` / `TEXT_TERTIARY` / `BG_HOVER` | `text-primary` / `text-tertiary` / `text-placeholder` / `bg-primary_hover` | `#FAFAFA` / `#A3A3A3` / `#737373` / `#171717` |
| Board thumbnail paper and dots | `BOARD_BG`, `BOARD_DOT` | unchanged (document content) | `#F4F4F4` |

Contrast: the pairs H uses are rows of token-contract.md section (f) "Contrast check C-1" (run its snippet; `ui ds a11y` is not a gate, it is a recorded false green): C-1.1, C-1.2, C-1.5, C-1.7, C-1.8 (text on chrome and cards), C-1.14 and C-1.15 (brand button label, 5.73:1 and 7.06:1), C-1.24 (gray badge), C-1.25 and C-1.26 (placeholder `#737373`, 4.18:1 and 3.78:1: the named exception, decided by R8.1), C-1.29 (focus ring), C-1.31 to C-1.34 (category glyph on its tile, R10.2), C-1.37 (selected card border on the page, R10.3; C-1.38 covers `HOME_THUMB_BG`, which H does not paint). Pairs H adds that C-1 does not list yet are handed to W1 in the report (not edited here): `TEXT_SECONDARY` on `BG_SECONDARY_HOVER` (nav label on a hovered row), `TEXT_ON_BRAND` on `BG_BRAND` for the drag ghost (same values as C-1.14), `TEXT_TERTIARY` on `BG_OVERLAY` over a thumbnail (depends on the picture: checked on the after-shots only).

**Table 2: type** (R5 Home density; weights and families per B; numbers stay in Roboto Mono, P5):

| Use | Today | After |
|---|---|---|
| Page title / breadcrumb | `font_title` 20 | `text-xl` 20/30 |
| Buttons, nav rows, card titles, list rows, search text, filter labels, trash rows, section headings in content | `font_body*` 11 | `text-sm` 14/20 |
| Card meta, template and hub descriptions, sidebar section heading, version pill, badges | `font_caption*` 10, literal 10/12 | `text-xs` 12/18 |
| Workspace name | literal 12 medium (`:912`) | `text-sm` 14/20 (weight per B) |
| Empty state title | `font_large_strong` 13 | `text-md` 16/24 |

**Table 3: sizes and their layout effect** (R1 quantified; "fits" means the new line height plus Untitled padding fits in today's box):

| Element | Line | Today | After | Δ height | Layout effect |
|---|---|---|---|---|---|
| Header buttons | `:1410` | 32 | button sm 36 | +4 | none: centred in the 64 header |
| Filter/sort pills, view toggles | `:1305`, `:1319`, `:1335` | 28 | button sm 36 | +8 | filter band 38 → 46 (`:1342`, same 4 / 6 margins); every content view starts 8 px lower |
| Sidebar search | `:932` | 32 | `Input field · Size=sm` (C; expected 36) | +4 | items below move 4 px down |
| Workspace header | `:902` | 32 | 36 | +4 | items below move 4 px down |
| Nav rows (8 fixed rows plus tree rows) | `:879` | 28 | 36 (decided: controller default, H OQ1) | +8 each | fixed rows push the project tree down 7 × 8 = 56 px plus the 8 px above; the tree list is a scroll area (`:1006-1009`) and absorbs it. Bottom block (Starred, Trash) keeps its anchor to the bottom edge. Sidebar width unchanged. |
| Starred file rows | `:1103` | 24 | 24 (text-xs 18 fits) | 0 | none |
| Trash count, version pill | `:1129`, `:918` | 16 | `Badge` sm 22 (C) | +6 | none (inside a 28-36 row) |
| File card meta block | `theme.rs:697`, `home/mod.rs:2626-2635` | 58 | 62 (12 + 20 + 18 + 12) | +4 per card row | grid rows 4 px taller; card height `HOME_THUMB_H` + local `CARD_META_H` 62 (contract keeps `HOME_CARD_H` 198) |
| List rows | `HOME_LIST_ROW_H` 44 | 44 | 44 (fits 20) | 0 | none |
| Folder tiles | `HOME_FOLDER_H` 48 | 48 | 48 (fits 20) | 0 | none |
| Trash rows | `:2675` | 52 | 52 (fits 20 + 18) | 0 | none |
| Drafts quick-create tiles | `:2278` | 48 | 50 (6 + 20 + 18 + 6) | +2 | strip 64 → 66 (`drafts_strip_h`, `:2238`) |
| Template cards | `:1522` (placed by hand), CTA `:1594` | 188, CTA 26 | 204, CTA button sm 36 | +16 | template grid rows 16 px taller (`template_rows`, `:1524`, placed by hand) |
| Hub card CTAs | `:2009-2010`, `:2031` | 26 | button sm 36 | +10 | hub card height grows 10 (`hub_rows_h`, `:1907`) |
| Empty state | `:2214-2220` | icon 32, 13 + 11 text | `Empty state · Size=sm` | about +20 | centred; no neighbour |
| Drag ghost | `:558` | 28 | 32 (text-sm 20 + 2 × 6) | +4 | follows the pointer |

Radii: every literal `CornerRadius::same(5|6|8)` and `cr_pill` on controls takes B's `radius-md` 8 (controls) or `radius-full` (badges, pills only where C's anatomy says so); cards take B's card radius (`radius-xl` 12 expected). Today's 43 radius literals in `home/mod.rs` (`spacing_baseline.json`) fall to the allowlisted remainder; nested shapes stay concentric (P4, `theme::concentric`).

**States** (C defines the anatomy; this spec lists which ones Home must show): buttons default, hover, pressed, disabled (none disabled today), focus (focus ring `#F97316`, R4); nav rows default, hover, selected, drop target, focus; inputs default, focus, filled; cards default, hover, selected, drag source; menus open/closed; empty state per section (`empty_state` texts unchanged).

## 8. Edge cases

1. Window at `MIN_WINDOW` 900×600: rule 7 (breadcrumb elides; no overlap). Project sections add "+ New folder", the widest case.
2. Long file and project names: elide with "…" at the new 14/12 sizes (`widgets::elide` today); never overlap the type badge or star.
3. Many projects: the tree scrolls; with 36-high rows the visible tree shrinks by 64 px at 1088 tall (about 2 rows).
4. Starred expanded with 3 files plus a 600 px window: the bottom reserve (`:1003`) must still leave the tree at least 64 px (`:1006`, `max(y + 64)`).
5. Search active: the header title is "Search results" style (`h.title()`); filter and sort still work; Escape clears (rule 8).
6. Trash empty / with items: "Empty trash" shows only with items (`:1255`).
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

Shared helper (new name) `chrome_colour_sites(src: &str) -> Vec<usize>`: the census rule of section 2 in Rust (strip from the first `#[cfg(test)]`, skip `//` comments, flag the same constructors and named colours, skip a named `Color32::WHITE` on a line containing `.image(` or `with_texture`). The gate's owner is spec A (D proposes `scripts/check_chrome_colour_literals.py`, new name); a Rust test is added as well because the Python design gates are advisory in CI and only `cargo test` on macOS blocks a merge. Add it as `crates/designos-app/src/shell/tests/chrome_census.rs` (new name, test-only, owned by H), registered with one `mod chrome_census;` line in `shell/tests.rs`; specs I, J and K reuse it. If I, J or K lands before H, that PR adds the file exactly as written here.

| Case (new names unless marked) | Where | Expected | Fails before |
|---|---|---|---|
| `home_sources_paint_no_chrome_colour_literals` | `shell/tests/home.rs` | `chrome_colour_sites(include_str!("../../home/mod.rs"))` returns only the two allowlisted tint lines; 0 for `home/thumb.rs` | yes (38) |
| `home_has_no_figma_blue_or_ui3_palette` | same | shipped `home/mod.rs` contains neither `UI3_` nor `PrimaryBlue` nor `0x0D, 0x99, 0xFF` | yes |
| `home_primary_button_is_brand_with_dark_label` | same | a `home_sim()` frame records `home-new-file`; the button's fill/label pair returned by `main_view::button_colours(Kind::Primary)` (new name, a pure fn the painter uses) equals A's brand fill and the R4 label | yes |
| `home_header_buttons_are_button_sm` | same | recorded `home-new-file`, `home-import` rects are 36 tall and vertically centred in the 64 header | yes (32) |
| `home_filter_bar_controls_are_button_sm` | same | `home-filter`, `home-sort`, `home-view-grid` rects 36 tall; content top = header 64 + 46 + 1 | yes (28, 38) |
| `home_layout_regions_unchanged` (guard) | same | sidebar right edge 264; header 64; first card x = sidebar + 32; card width 224 | guard |
| `home_breadcrumb_never_overlaps_header_buttons` | same | `Sim` at 900×600 in a project section with a long name: breadcrumb right edge + 12 ≤ leftmost button left | yes at 14 px type |
| `home_sidebar_nav_rows_follow_home_density` | same | `home-recents`, `home-trash` rects 36 tall | yes (28) |
| existing `home::tests::*` (`home/mod.rs:2751`, `:2827`, `:2868`) and `shell::tests::home::*` (marked) | — | pass unchanged | guard |
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

**Evidence commands.** Each was run once by the spec writer on 2026-10-25 against a build of the first draft's `ec47da26`; exit codes shown (before-shots; L re-captures the baseline on the branch base). `S` is a scratch folder; never point `--library` at the real library. Run the same list after the change and put both sets in the PR.

```bash
export PATH="$HOME/.cargo/bin:$PATH"; S="$(mktemp -d)"; OUT="$S/out"; mkdir -p "$OUT" "$S/first-run"
RUN() { cargo run -q --release -p designos-app --bin designos -- "$@"; echo "exit $?"; }
RUN --seed-library "$S/lib"                                                                                   # exit 0: writes 4 sample files (project "Marlow")
RUN --home --library "$S/lib" --screenshot "$OUT/H-01-home-recents.png" --size 1600x1088 --no-toast          # exit 0: sidebar, header, filter bar, 4 cards (incl. a board)
RUN --home --library "$S/first-run" --screenshot "$OUT/H-02-home-first-run.png" --size 1600x1088 --no-toast  # exit 0: first run seeds one "Welcome" file (an empty library is never empty)
RUN --home --library "$S/lib" --screenshot "$OUT/H-03-home-min-900x600.png" --size 900x600 --no-toast        # exit 0: minimum window, header crowding (rule 7)
```

Observed while capturing (all specs H–K, 37 screenshot runs under parallel load from other agents): 5 runs printed `designos: screenshot was not captured` and exited 1; every retry of the same command exited 0. If that happens, re-run the command once and note it in the PR. This is plan.md's unconfirmed dogfood gap, now reproduced as load-dependent.

Not capturable from the CLI today (no flag selects a Home section, the list view, a hover or a menu): Templates & Specs, Drafts, All folders, Resources & Libraries, Starred, Trash (with and without items), list view, filter and sort menus open, context menu, drag ghost, empty states, focus rings. The PR covers them with the tests above and manual window captures labelled manual. A `--home-section` flag would be a separate issue.

## 11. Performance budgets

- No new allocation per frame beyond today's (colour constants are `const`); no `doc.to_json()` (CLAUDE.md invariant 1); Home draws no document.
- Thumbnail loading budget unchanged (`budget = 4u32` textures per frame, `:1540` (placed by hand), `:1924`, `:2254`).
- Idle: Home requests no repaint when idle (unchanged).

## 12. Dependencies, ownership and build order

**Build order.** L captures the Home baseline (`H-01`..`H-03` above are also L's inputs) → A and B land the colour roles and Home sizes → C lands the widgets → this spec. If C is not merged, H paints the anatomy with A/B constants inside `home/mod.rs` and swaps to C's widgets in a follow-up only if C's API differs; do not wait on C for colours.

**Technical decisions recorded here** (the owner may reverse): (1) delete the two dead files rather than move the inline modules into them (smallest diff, no behaviour); (2) Home sidebar uses the editor left-panel surface `bg-secondary` so Home and editor read as one app (specs D/E use the same pairing); (3) the Rust literal test is added even though A owns the Python gate, because only `cargo test` blocks a merge; (4) hub, quick-create and template accents follow the same categorical mapping as the file-type badges (one rule, R10.2).

**File ownership across H–K.** Each file has exactly one owner; the assignments are the same as the table in I, J and K, and this one adds a note per file.

| File | Owner | Note |
|---|---|---|
| `home/mod.rs` | H | the updater card call (`:1121`) draws J's `updater.rs`; the menus call C's `widgets::menu` |
| `home/thumb.rs`, `home/time.rs`, `home/session.rs` | H | no visual change expected |
| `home/sidebar.rs`, `home/main_view.rs` | H | deleted (dead) |
| `shell/tests/home.rs` | H | tests only |
| `home/dialog.rs` | J | modal dialogs shared by Home and the editor (`dialog.rs:1-2`); Home density applies there too |
| `updater.rs` (module `crate::home::updater`, `home/mod.rs:16-17`) | J | the task brief assigns the updater dialog to J; spec D's not-mine list says H, J takes it |
| `terminal.rs`, `agent_*.rs`, `canvas/agent_*.rs`, `skills/*`, `image_jobs.rs`, `shell/tests/agent_chrome.rs` (new) | I | |
| `cmdk.rs`, `find/mod.rs`, `find/panel.rs`, `find/select.rs`, `comments/*`, `history/*`, `present.rs`, `panels/context.rs`, `shell/tests/overlays_chrome.rs` (new) | J | |
| `panels/assets.rs`, `variables/*`, `libraries/*`, `styles.rs`, `plugins/*`, `panels/check_designs.rs` (controller ruling), `shell/tests/assets_variables_libraries_chrome.rs` (new) | K | |
| `shell/tests/chrome_census.rs` (new) | H | the `chrome_colour_sites` helper; whichever of H–K lands first adds it as section 10 defines it |
| `home/leave.rs`, `home/unsaved.rs` (new on `origin/main`, 0 `theme::` lines) | H | no painting today; H owns them if they gain UI |

Not mine (D–G or the kit specs): `theme.rs`, `design/*` (A, B); `widgets/*` except `widgets/picker.rs`, `panels/mod.rs` (C); `tabs.rs`, `shell.rs` (including its toast, which D paints by J's rules), `panels/rail.rs`, `panels/toolbar.rs`, `panels/menus.rs`, `canvas/rulers.rs` (D); `panels/left.rs`, `panels/layers.rs`, `find/filter.rs` (E); `panels/right.rs`, the inspector files, `widgets/picker.rs`, `panels/style_picker.rs`, `panels/style_edit.rs`, `panels/style_edit_grid_rows.rs` (F); `dev/*`, `specs.rs` (G). `shell/tests.rs` is L's (`Sim`, `:69-146`; `Sim::rect_of` `:193`): each H–K PR only appends one `mod` line to its module list (`:1114-1180`). `scripts/spacing_baseline.json` is a shared ratchet: each surface PR may only lower its own files' counts with `--write-baseline`.

Unassigned in D–K (flag to the controller, recommended owner in brackets): `decks/ui.rs` [D], `board/*` [D], `canvas/draw.rs`, `canvas/grids.rs`, `canvas/tiles.rs`, `canvas/interact.rs` [D or L], `icons.rs` [C, R6], `lint.rs` [K].

**Open PRs touching H files** (`gh pr list --state open`, 2026-10-25, full file lists from `gh api repos/{owner}/{repo}/pulls/<n>/files`): stale #765, #768, #770 (opened 2026-10-19, `mergeable: CONFLICTING`) touch `home/mod.rs`, `home/dialog.rs`, `home/session.rs`; the PM decides their fate before H starts. No other open PR touches `home/`.

## 13. Owner rulings that apply

- R1: "sử dụng 100% style kiểu của Untitled UI nhưng vẫn giữ cấu trúc layout và bố cục hiện tại" (all of Untitled's style; keep layout and arrangement).
- R2: take Untitled spacing and typography as they are, adapted to Studio's layout.
- R3: Studio improves itself; no Figma. Evidence goes in the PR.
- R4: brand = Untitled anatomy with Studio orange `#E36323`, dark label `#0A0A0A`; focus ring `#F97316`; Purple Ban stays (every kit brand role resolves to the orange family, never the kit purple `#7F56D9`).
- R5: Home and dialogs use sm/md (text-sm 14/20, button sm 36 / md 40); the 2 px and 6 px spacing ban is lifted.
- R6: keep the icon set; restyle colour to Untitled fg roles.
- R7: spec-driven; the PM coordinates delivery.
- R8.1 placeholder `#737373` (named exception); R8.2 selected `#262626`; R8.6 button xs label text-sm 14 semibold (buttons only); R8.7 destructive hover `#B91C1C` applies to solid destructive buttons (`BG_ERROR_SOLID_HOVER`): H has none; its one destructive button ("Empty trash") is the kit's Secondary destructive (outline), whose hover is the error tint `BG_ERROR_PRIMARY`.
- R10.2: "Home category colours = kit Featured-icon utility tiles (blue, amber, green, orange; -50 tile, -600 glyph)"; the "P" disc takes the design blue (coordinator, board 3).
- R10.3: "Selected Home file card = 2 px `BORDER_SELECTED` `#FB923C`".
- Standing: `main` is protected (PR, green macOS test, clippy, rustfmt, squash); visual evidence in every PR (2026-09-22).

## Open questions (all decided)

Decided since the first draft (no longer owner questions):

- **OQ1 · Sidebar row height** — decided: 36. Source: plan.md, controller defaults from the 21:35 sweep: "Home sidebar rows 36 (R5 Home = sm) — H OQ1".
- **OQ4 · Placeholder contrast** — decided: kit `#737373` (`TEXT_PLACEHOLDER`). Source: R8.1 "Placeholder text = kit `text-placeholder` `#737373` … the one named exception to the AA text law" (owner: "B · #737373 của kit").
- **OQ5 · File card meta block** — decided: local `CARD_META_H` 62, cards 4 px taller. Source: plan.md, controller defaults: "file cards may grow 4 px (local `CARD_META_H` 62) as a consequence of R5 — H OQ5".

Decided by the owner on board 3 (plan.md R10.x):

- **OQ2 · Category colours** — decided: kit Featured-icon utility tiles. Source: R10.2 "Home category colours = kit Featured-icon utility tiles (blue, amber, green, orange; -50 tile, -600 glyph); needs 8 contract roles" (owner: "A · Ô màu utility"); the roles are contract N39–N46, and the "P" disc takes the design blue.
- **OQ3 · Selected file card** — decided: 2 px `BORDER_SELECTED` `#FB923C`. Source: R10.3 "Selected Home file card = 2 px `BORDER_SELECTED` `#FB923C`" (owner: "A · Cam #FB923C 2px").

No owner questions remain for H.
