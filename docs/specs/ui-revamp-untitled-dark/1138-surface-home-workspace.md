# H Restyle the Home workspace in Untitled UI dark (Home density)

| Field | Value |
|---|---|
| Issue | #1138 (epic #1118, spec key H) |
| Size | M. About two days: one live 2,830-line file (`home/mod.rs` at `8dfaee93`) with 38 chrome colour sites and 11 derived tints, Home-density sizes (R5) on four control families, card and tile heights re-fitted to the new type, two dead files removed, new tests. |
| Depends on | A (colour roles, Studio brand override, constant names in `theme.rs`), B (type, spacing, radius and control-size scale; `HOME_*` constants), C (widget anatomy under C's own names: `Button/Primary`, `Button/Secondary`, `Button/SecondaryDestructive` (new name; C adds it, plan.md C-K), `Button/Icon`, `Input/Search`, `Menu/*`, `List/Row`, `Badge/StatusCount`, `EmptyState/Default`, `Featured icon`; the kit's component names appear only as references, R14) and L (baseline capture, hermetic capture script and layout-unchanged gate) |
| Unblocks | M (docs and screenshot refresh) |
| Touches | `crates/designos-app/src/home/mod.rs` (drag ghost `:550-602`; inline `mod sidebar` `:857-1191`; inline `mod main_view` `:1196-2653`); deleted: `home/sidebar.rs`, `home/main_view.rs` (dead, never compiled); tests: `crates/designos-app/src/shell/tests/home.rs`, the shared helper file `crates/designos-app/src/shell/tests/chrome_census.rs` (new name, section 10) and its one-line `mod` registration in `shell/tests.rs` (L's file); `scripts/spacing_baseline.json` (lowered with `--write-baseline`, never raised). Reads only (owned by A and B): `theme.rs:699-709` (`HOME_*`). |

Code baseline: every `file:line` is at `origin/main@8dfaee93` (v0.7.0 merged), read with `git show 8dfaee93:<path>` (never a checkout). The earlier drafts cited `ec47da26` and then `205c72ef`; fix round 3 (2026-10-28) moved every citation by a line diff of `205c72ef..8dfaee93` (a line that did not change keeps its text and gets its new number), and the few cites whose code changed were placed by code text (marked "placed by hand"). The cross-check: the census at `8dfaee93` (section 2) prints exactly the mapped lines. Re-anchor against the commit you branch from before writing code (search for the quoted code, not the number).

**Drift note (`205c72ef` to `8dfaee93`).** `home/mod.rs` 2,906 to 2,830 lines (+87 / −163), `home/session.rs` 1,628 to 1,711, `home/dialog.rs` (J) +230, `theme.rs` +8 after `:290`; `thumb.rs`, `time.rs`, `leave.rs`, `unsaved.rs`, `sidebar.rs` and `main_view.rs` are unchanged in size. Home's colour census is the same 45 sites (38 live + 7 dead); every live site moved by +2 to −73 lines. What changed that H must style: (1) **Templates & Specs** shows blank-creation cards, not the six Marlow samples: `TemplateKind` and `Act::CreateFromTemplate` are gone; the cards are "Blank design", "Blank board", "Blank deck" and "DESIGN-OS Studio Starter" (`Act::OpenStarterSample`), plus the spec-bundle card while `SPEC_BUNDLE_ENTRY_ENABLED` (false today); the recorded ids `home-template-mobile`, `-web` and `-system` are gone, `home-template-design` and `-starter` are new. (2) The four **Resources hub cards** have one full-width CTA each ("Create Library Kit" and "New Token Sheet" went; the two-button branch has no caller). (3) The **"Getting started" starter-kit row** and its `home-starter-*` ids are gone. (4) A file whose disk file is missing draws **"File not found"** in `DANGER` and a `Kind::Danger` **"Remove"** button (`home-remove-<name>`, `Act::RemoveMissing`): new, restyled in section 7 (table 1, table 3). (5) `Import` ignores a double click (no visual change). Unchanged: sidebar, header, filter bar, list row, drag ghost, empty state, and the `scripts/spacing_baseline.json` entries for `home/mod.rs`, `home/main_view.rs` and `home/sidebar.rs`. Surfaces no spec covers yet (v0.7.0 added none in Home): see the unassigned list in section 12.

Implementer note: this spec is self-contained. Load the `es-designer` skill before writing UI code (section 7). Colours are named by their Untitled UI token name and hex; the Rust constant names come from the token contract (`specs/token-contract.md`, specs A and B), sizes from spec B, widget anatomy from spec C. Where A, B or C are merged before you start and differ from the expectations written here, A, B and C win; report the difference in the PR.

## 1. Summary and user story

As a designer who opens Studio, I land on Home and see it in the Untitled UI dark style: neutral surfaces (`bg-primary #0A0A0A`, `bg-secondary #171717`), hairline borders, Untitled buttons, inputs, menus and badges at the Home density of R5 (text-sm 14/20, button sm 36 / md 40), and the Studio orange `#E36323` with a dark `#0A0A0A` label on the one primary action ("+ Design file"), not today's blue. Home keeps its layout: the 264 px sidebar on the left, the header with title and action buttons, the filter/sort bar, the grid or list of files, the Trash at the bottom of the sidebar, and every section in today's order (R1).

## 2. Current state (FACT unless labelled)

**Files.** Measured with the census command below, plus `wc -l`.

| File | Lines | Compiled | `theme::` lines | Chrome colour sites | Site lines |
|---|---|---|---|---|---|
| `home/mod.rs` | 2,830 | yes | 179 | 38 | 563, 868-871, 873, 876, 894, 895, 1115, 1184, 1187, 1206-1213, 1404, 1535, 1564, 1699, 1700, 1974, 1994, 2157, 2163, 2407, 2413, 2440, 2463, 2477, 2483, 2492, 2493, 2523 |
| `home/thumb.rs` | 201 | yes | 1 | 0 | (`:153` builds a `ColorImage` from pixels: not a colour) |
| `home/time.rs` | 67 | yes | 0 | 0 | logic only |
| `home/session.rs` | 1,711 | yes | 0 | 0 | logic only (autosave, CLAUDE.md invariant 1) |
| `home/leave.rs`, `home/unsaved.rs` | 318, 144 | yes (`home/mod.rs:9`, `:15`) | 0 | 0 | logic only; not in the census list because they paint nothing |
| `home/sidebar.rs` | 214 | **no** | 20 | 0 | dead file |
| `home/main_view.rs` | 544 | **no** | 82 | 7 | dead file |

`home/sidebar.rs` and `home/main_view.rs` are dead: `home/mod.rs` declares `mod sidebar { … }` (`:857`) and `mod main_view { … }` (`:1196`) inline, so Rust never reads the two files, and nothing else references them (`grep -rnE 'sidebar\.rs|main_view\.rs|include!' crates/designos-app/src` finds nothing). They still have entries in `scripts/spacing_baseline.json` (`home/main_view.rs` 47 offset + 1 radius literals, `home/sidebar.rs` 14 offset literals). Editing them changes nothing on screen.

Census command (shared with specs D–G; run from the repo root; writes nothing). A *chrome site* is a `Color32` constructor with literal arguments, a `from_gray`/`from_black_alpha`/`from_white_alpha` call, or a named `Color32` colour other than `TRANSPARENT`, in non-test code (comments and strings blanked and `#[cfg(test)]` items dropped by `scripts/rust_literal_scan.clean`):

```bash
# REV=8dfaee93 reads that commit with `git show` (no checkout); drop it to read your working tree
REV=8dfaee93 PYTHONDONTWRITEBYTECODE=1 python3 - home/mod.rs home/thumb.rs home/time.rs home/session.rs home/sidebar.rs home/main_view.rs <<'PY'
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

Derived tints: `home/mod.rs` has 11 `gamma_multiply(` calls on chrome colours (`grep -c 'gamma_multiply(' crates/designos-app/src/home/mod.rs` = 11 at `8dfaee93`, all in shipped code): accent tints on cards, banners and icon boxes, and `theme::DANGER.gamma_multiply(0.2)` for the danger hover (`:1413`).

**Why the primary action is blue.** P2 (`design/principles.md:10-19`) and the soul (`design/soul.md:15`, `:32`) say the one primary action per view is orange `#E36323`. Home ignores `theme::BG_BRAND` (`theme.rs:36`) because both inline modules define their own Figma-UI3 copies: `UI3_BLUE = #0D99FF` (`home/mod.rs:876`, `:1209`) and `UI3_BLUE_HOVER = #0B84E0` (`:1210`), and the header button kind is `Kind::PrimaryBlue` (`:1224`, used `:1247`, painted `:1401-1405` with a white label). The same `UI3_*` block defines Zinc copies of theme colours (`UI3_SIDEBAR_BG #18181B`, `UI3_SURFACE #1E1E22`, `UI3_PILL_ACTIVE`/`UI3_BORDER #27272A`, `UI3_PILL_HOVER #232329`, `UI3_MAIN_BG #121215`, `:868-876`, `:1206-1208`) and four category accents (`UI3_TEAL #14AE5C`, `UI3_AMBER #F59E0B`, `UI3_ORANGE #FF6A3D`, plus blue, `:1211-1213`).

**Regions** (all in `home/mod.rs`; screenshot `H-01` in section 10):

- **Frame** `show` (`:432-470`): fills `theme::BG`, splits sidebar `HOME_SIDEBAR_W` 264 (`theme.rs:699`) and main.
- **Sidebar** (`:878-1121`): fill `UI3_SIDEBAR_BG`, right hairline `UI3_BORDER` (`:880-881`).
  - Workspace header 32 tall (`:886-908`): avatar tile 20 with literal `#27272A` fill and `#3F3F46` stroke and an orange folder icon (`:893-896`); name "Personal Workspace" in a literal `FontId::new(12.0, …)` (`:897-898`); version pill 16 tall in mono with `ver_font.size = 10.0` (`:899-905`); "Workspace activity" icon button 24 (`:911-913`, C's `icon_button`).
  - Search field 32 (`:916-941`): `UI3_SEARCH_BG` + `UI3_BORDER`, icon `ICON_SECONDARY`, text `TEXT`, hint `TEXT_TERTIARY`.
  - Nav rows `ROW_H` 28 with 2/1 px gaps (`:864`, `:943-983`): Recents, Templates & Specs, divider, "FILES & PROJECTS" heading with a blue "P" disc (`:959-962`) and a `+` icon button 24 (`:963-967`), Drafts, All folders, Resources & Libraries.
  - Project tree (`:990-1060`), scrolling, rows 28, "No projects yet" (`:1003`), chevrons `ICON_TERTIARY`.
  - Starred row with expander and up to 3 file rows 24 with blue file icons (`:1062-1101`).
  - Updater card (`:1103-1106`): `updater.rs` (spec J), hidden while `UPDATER_UI_ENABLED` is false (Z14). Not drawn today.
  - Trash row with a count badge 22×16 literal `#27272A` (`:1108-1120`).
  - Row painter `row` (`:1171-1190`): drop target `BG_SELECTED_HOVER`, selected `UI3_PILL_ACTIVE`, hover `UI3_PILL_HOVER`, radius `CornerRadius::same(6)`; selected icon and label `Color32::WHITE` (`:1184`, `:1187`), label `font_body_strong` when selected.
- **Main header** `HOME_HEADER_H` 64 (`:1235-1283`): breadcrumb in `font_title` 20 (`:1346-1389`); right-aligned buttons 32 tall, pill radius, `font_body_strong` (`button`, `:1391-1421`): "+ Design file" `PrimaryBlue`, "+ FigJam board", "+ Spec bundle", "Import" `Secondary` (`UI3_SURFACE` + `UI3_BORDER`), "+ New folder" in project sections, "Empty trash" `Danger` (transparent, `theme::DANGER` stroke and text) in Trash.
- **Filter bar** (`:1285-1328`): `Filter: …` and `Sort: …` pills 28 tall with chevrons (`:1290`, `:1304`), `List view`/`Grid view` icon buttons 28 (`:1317-1326`); band height 38 (`:1327`) then a hairline (`:1329`). Menus are `widgets::menu::popup_menu` (spec C, `:472-507`).
- **Content views**: Templates & Specs (`:1434-1609`: four 240×188 cards, "Blank design", "Blank board", "Blank deck" and "DESIGN-OS Studio Starter" (a fifth, spec-bundle card while `SPEC_BUNDLE_ENTRY_ENABLED`, false today), each with an accent banner, category pill and CTA 26 tall); All folders (`:1615-1862`: project cards with 2×2 previews, "+ Folder" mini button in black alpha 170 / blue, "+ New project" card, teal folder tiles); Resources & Libraries (`:1863-2074`: four hub cards, one full-width accent CTA 26 tall each and white labels `:1974`, `:1994` (the two-button branch `:1966-1988` has had no caller since v0.7.0)); files view (`:2190-2437`: Drafts quick-create strip of three 48-tall tiles with blue/amber/teal accents `:2234-2270`, folder tiles `HOME_FOLDER_H` 48, file grid; v0.7.0 removed the "Getting started" starter-kit row); Trash (`:2592-2653`: note line, rows 52 `:2599`, "Restore" secondary buttons).
- **File card** `card` (`:2530-2558`): 224 × (140 + 58) (`theme.rs:702-705`), radius 8, fill `UI3_SURFACE`, hover stroke `TEXT_TERTIARY`, selected 2 px `UI3_BLUE` (`home/mod.rs:2538-2539`, stroke call `:2545`); thumbnail stage literal `#18181B` (`:2483`, right after `let _ = theme::HOME_THUMB_BG;` at `:2482`) with the picture inset 6 and a white-alpha-18 frame (`:2492-2493`); board thumbnails on `BOARD_BG` with a "Board" badge (`:2459-2477`, `:2499-2508`); file-type badge 20×20 in blue/amber/teal/orange with a white glyph (`:2510-2528`); title `font_body_strong` 11 at `thumb.bottom() + 17`, meta `font_caption` 10 at `+16` (`:2553-2557`; a missing disk file reads "File not found" in `DANGER`, `:2555-2556`). Star overlay: black-alpha circles (`:2157`, `:2407`, alpha 80 when starred, 150 when the star is hovered) and a `#FFCD29` star (`:2440`); drag-source dim black alpha 90 over the dragged card (`if dragging && selected …`, `:2162-2163`, `:2412-2413`).
- **List row** `list_row` (`:2565-2590`; the modified column reads "File not found" in `DANGER` for a missing file, `:2586-2587`): `HOME_LIST_ROW_H` 44, selected `UI3_SURFACE` + 1 px `UI3_BLUE`, hover `BG_HOVER`, 48-wide thumbnail, columns from `list_columns` (`:2560-2563`).
- **Empty state** `empty_state` (`:2168-2188`): 32 px file icon `ICON_TERTIARY`, title `font_large_strong` 13, body `font_body` 11 wrapped at 380.
- **Drag ghost** (`:550-602`): 28-tall pill in `BG_BRAND` with a white label (`:561-563`). White on `#E36323` is 3.46:1 (plan.md facts), below 4.5.

**Gates today.** No gate counts colour literals. `scripts/check_spacing_relationships.py` ratchets literal geometry per file against `scripts/spacing_baseline.json` (`home/mod.rs`: 162 offset, 43 radius, 2 font literals; it passed on the first draft's `ec47da26`, exit 0; not re-run at `8dfaee93` because that needs a checkout; the three Home entries of the baseline file are identical at `205c72ef` and `8dfaee93`). `scripts/zero_fake_ui_gate.py` Z09 checks `home/mod.rs` (`Act::ExportFig` writes bytes; sections filter distinctly) and Z14 the hidden updater card. `.github/workflows/design-gates.yml` runs those Python gates as advisory (`continue-on-error: true`, `:4-5`, `:21`); only the macOS test, clippy and rustfmt jobs block a merge (CLAUDE.md, Git Workflow). Home tests: `home/mod.rs:2656-2830` and `shell/tests/home.rs`.

**Decided.** Home sidebar rows are 36 (plan.md, controller default from the 21:35 sweep: "Home sidebar rows 36 (R5 Home = sm) — H OQ1"). **UNKNOWN.** The exact Untitled `Input field` height at Size=sm (C and B measure it from the kit).

## 3. Behaviour rules

1. **Layout unchanged (R1).** Same regions, same order, same widths: sidebar 264, main pad `HOME_PAD` 32, header 64, card width 224 and gap 24, list columns. Heights grow only where R5 raises a control or type size, by the amounts in section 7 table 3. No new control, no removed control, no new copy.
2. **One brand action.** "+ Design file" is C `Button/Primary` sm: fill `BG_BRAND` `#E36323`, label `TEXT_ON_BRAND` `#0A0A0A` (R4, 5.73:1), hover `BG_BRAND_HOVER` (kit reference: `Buttons/Button · Size=sm · Hierarchy=Primary`, `bg-brand-solid` with the Studio override). No other Home control uses the brand fill. `Kind::PrimaryBlue` and every `UI3_*` constant leave the file.
3. **No chrome literal.** After the change the census prints `chrome_sites=2` for `home/mod.rs`, both on the allowlist (`:2463` and `:2492`: `Color32::WHITE` as a texture tint inside `RectShape::filled(..).with_texture`, which multiplies the thumbnail by 1 and is not a colour role), and 0 for `home/thumb.rs`, `home/time.rs`, `home/session.rs`. Every other colour is a `theme::` constant from spec A. Derived tints (`gamma_multiply`) on chrome colours are replaced by the Untitled role that exists for that state (section 7 table 1); a tint stays only where Untitled has no role and A defines it as a named constant.
4. **Every white-on-brand label becomes dark.** The drag ghost label (`:563`) and any label painted on `bg-brand-solid` use the R4 dark label `#0A0A0A`.
5. **Neutral selection (P2).** The selected sidebar row and the selected list row use a neutral fill; the selected grid card uses a 2 px `BORDER_SELECTED` `#FB923C` border (R10.3), never `#0D99FF`.
6. **Dead files go.** Delete `home/sidebar.rs` and `home/main_view.rs` in the same PR (technical decision, section 12). The build proves they were dead: it compiles without them.
7. **Header never overlaps.** At every window width down to `MIN_WINDOW` 900×600 (`theme.rs:612`), the breadcrumb ends at least `spacing-lg` 12 px left of the leftmost header button; a longer breadcrumb elides its leading segments with "…" first, then the last segment. Today at 900 px there is about 60 px between "Recents" and "Import" (screenshot `H-03`); the 14 px type adds about 80 px to the button row, so without this rule the title and buttons collide.
8. **Behaviour unchanged.** Every click, drag, drop, context menu, keyboard path (Escape clears search `:449-458`), recorded widget id (`home-new-file`, `home-filter`, `home-sort`, `home-view-list`, `home-view-grid`, `home-search`, `home-<nav key>`, `home-trash`, `home-template-*` (`-design`, `-board`, `-deck`, `-starter`), `home-drafts-*`, `home-res-*`, `home-remove-<name>`) and every `Act` stay as they are. Z09 stays green.

## 4. Data model / schema changes

None. No document, library, settings or file-format change.

## 5. Contracts

- **Theme constants consumed** (token contract `specs/token-contract.md` names are Studio's; the kit role is a reference in brackets, R14): `BG` (bg-primary), `BG_SECONDARY` (bg-secondary; also the kit's bg-primary_alt, same value `#171717`), `BG_HOVER` (bg-primary_hover), `BG_SECONDARY_HOVER` (bg-secondary_hover, new in the contract), `BG_TERTIARY` (bg-tertiary), `BG_SELECTED` (bg-secondary_hover), `BG_BRAND` / `BG_BRAND_HOVER` / `BG_BRAND_PRESSED` (bg-brand-solid, Studio override), `BORDER` (border-secondary), `BORDER_STRONG` (border-primary), `BORDER_SELECTED` (border-brand, contract value Orange-400 `#FB923C`), `DANGER` (fg-error-primary; also the secondary-destructive outline, the kit's border-error_subtle has the same value `#EF4444`, and C asks W1 for `BORDER_ERROR_SUBTLE`), `TEXT` (text-primary), `TEXT_SECONDARY`, `TEXT_SECONDARY_HOVER`, `TEXT_TERTIARY`, `TEXT_PLACEHOLDER` (kit `#737373`, R8.1), `TEXT_ERROR` (text-error-primary), `UTILITY_RED_700` (N32, `#FCA5A5`: the value of the kit's text-error-primary_hover, used as the secondary-destructive hover label until A adds C's `TEXT_ERROR_HOVER`), `TEXT_WARNING` (star on; the kit's fg-warning-secondary has the same value `#FACC15`), `ICON_BRAND_PRIMARY` (fg-brand-primary: workspace avatar icon), `TEXT_ON_BRAND` (R4 dark label), `ICON_SECONDARY` (fg-secondary), `ICON_QUATERNARY_HOVER` (fg-quaternary_hover), `ICON_QUATERNARY` (fg-quaternary), `FOCUS_RING`, `BOARD_BG` / `BOARD_DOT` (content). Gray badges use `UTILITY_NEUTRAL_50` + `UTILITY_NEUTRAL_200` + `UTILITY_NEUTRAL_700` (contract N21–N23). Also from the contract: `BG_ERROR_PRIMARY` (N26, destructive-secondary hover fill), `BG_OVERLAY` (N33, star backdrop only) and `HOME_THUMB_BG` (contract row 114, `#262626`, thumbnail stage). Category colours (R10.2, contract N39–N46): tiles `UTILITY_BLUE_50`, `UTILITY_AMBER_50`, `UTILITY_GREEN_50`, `UTILITY_ORANGE_50`; glyphs `UTILITY_BLUE_600`, `UTILITY_AMBER_600`, `UTILITY_GREEN_600`, `UTILITY_ORANGE_600`.
- **Size constants consumed** (token contract (c) names; Untitled step in brackets): `FONT_SIZE_TEXT_XS` / `LINE_TEXT_XS` (text-xs 12/18), `FONT_SIZE_TEXT_SM` / `LINE_TEXT_SM` (text-sm 14/20), `FONT_SIZE_TEXT_MD` / `LINE_TEXT_MD` (text-md 16/24), `FONT_SIZE_TEXT_XL` / `LINE_TEXT_XL` (text-xl 20/30), `CONTROL_SM` (button sm 36), `SPACE_FACT` (spacing-xs 4), `SPACE_INLINE` (spacing-md 8), `SPACE_GROUP` (spacing-lg 12), `RADIUS_CONTROL` (radius-md 8), `theme::cr_pill` (radius-full). spacing-sm 6 has no contract constant, and H adds no new 6 px step: the drag ghost and the Drafts quick-create tiles are computed on the 4 px ladder (table 3), and button padding lives inside C's `kit/` widgets. Two 6 px literals exist today and stay as they are (R1, layout unchanged): the filter band's bottom margin (`y = top + 4.0` at `:1287`, `top += 38.0` at `:1327`, so 4 above and 6 below the pills) and the thumbnail picture inset (`let pad = … 6.0`, `:2486`). If B adds `SPACE_SM = 6.0` (new name; R5 lifts the 6 px ban), H uses it for those two; otherwise they stay literal and the spacing ratchet counts them as today. `HOME_*` constants (`theme.rs:699-709`) are A/B's file: the token contract keeps every `HOME_*` value (appendix, `HOME_CARD_H` 198), so H computes the taller card in `home/mod.rs` from a local `CARD_META_H = 62.0` (new name) as `HOME_THUMB_H + CARD_META_H` and stops reading `HOME_CARD_H` (section 7 table 3; decided, OQ5); `HOME_THUMB_BG` (`theme.rs:709`) is revalued by the contract to `#262626` (row 114, "thumbnail placeholder, one step above the bg-secondary card"); H paints the stage with it at `home/mod.rs:2483` and removes the `let _ = theme::HOME_THUMB_BG;` line (`:2482`).
- **Widgets consumed** (spec C): `Button/Primary` and `Button/Secondary` sm (kit reference `Buttons/Button · Size=sm`), `Button/SecondaryDestructive` (new name in C, plan.md C-K; kit `Buttons/Button destructive · Hierarchy=Secondary`; anatomy in section 7), `Button/Icon` sm (icon-only; kit `Buttons/Button utility`), `Input/Search` sm (search; kit `Input field · Size=sm`), `Menu/*` (filter, sort and context menus; already `widgets::menu`), `List/Row` sm (sidebar rows; kit `_Nav item base`), `Badge/StatusCount` gray (version, Trash count, "Board"; kit `Badge · Size=sm · Color=Gray`), `Featured icon` (file-type badges, template and quick-create icons), `EmptyState/Default` (kit `Empty state · Size=sm`), `Tooltip/Default`. The kit names are references only (read from the live kit and `designos <kit> --inspect-json`, 2026-10-23); property names (`Size`, `Hierarchy`, `Color`) are spec C's to confirm.
- **Rust surface.** `main_view::Kind` keeps three variants renamed `Primary`, `Secondary`, `SecondaryDestructive` (new names, C's `Button/SecondaryDestructive`; "Empty trash" and the missing-file "Remove" both use the third); `button()` keeps its signature. No public API change outside `home/`.

## 6. File-format mapping

None.

## 7. UI

Load the `es-designer` skill first and complete its checklist; the project's design system wins over the skill's defaults. Read `design/principles.md`, `design/soul.md`, `design/ux-writing.md` before editing. Evidence goes in the PR (not Figma, R3). Responsive: SKIPPED (owner default): capture 1600×1088 plus the 900×600 minimum window of rule 7.

**Table 1: colour roles** (expected values; spec A's token contract is authoritative and the Studio constant is the name that goes into the design store and the code, R14; the "Untitled reference" column only says which kit role the value was read from, in dark mode, resolved from `--export-variables … --format dtcg` with `evidence/resolve-untitled-tokens.py`; "Studio override" means R4's orange family replaces the kit's purple brand):

| Element | Today | Studio constant (A, contract) | Untitled reference | Hex |
|---|---|---|---|---|
| Main area background | `UI3_MAIN_BG #121215` | `BG` | bg-primary | `#0A0A0A` |
| Sidebar background | `UI3_SIDEBAR_BG #18181B` | `BG` (the editor left panel is `BG` too, spec E table 1, contract row 1 "chrome base: panels … kit nav") | bg-primary | `#0A0A0A` |
| Sidebar right hairline, header bottom hairline, dividers | `UI3_BORDER #27272A` | `BORDER` | border-secondary | `#262626` |
| Search field | `#1E1E22` fill, `#27272A` border | C `Input/Search`: fill `BG`, border `BORDER_STRONG`, placeholder `TEXT_PLACEHOLDER`, focus ring `FOCUS_RING` (R4 `#F97316`) | kit Input: bg-primary, border-primary, text-placeholder, focus-ring | `#0A0A0A`, `#404040`, `#737373` |
| Nav row rest / hover / selected (C `List/Row` sm on a `BG` surface, C's site `home/mod.rs:1173-1190`) | none / `#232329` / `#27272A` | rest no fill; hover `BG_HOVER`; selected `BG_SELECTED` (R8.2) | bg-primary_hover; bg-secondary_hover | — / `#171717` / `#262626` |
| Nav row drop target | `BG_SELECTED_HOVER` | `BG_SELECTED` + 1 px `BORDER_SELECTED` | border-brand (Studio override) | `#262626` + `#FB923C` |
| Nav label rest / hover and selected | `TEXT` / `WHITE` | `TEXT_SECONDARY` / `TEXT_SECONDARY_HOVER` (C `List/Row`; weight per B) | text-secondary, `_hover` | `#D4D4D4` / `#E5E5E5` |
| Nav icon rest / hover and selected | `ICON_SECONDARY` / `WHITE` | `ICON_QUATERNARY` / `ICON_QUATERNARY_HOVER` (C `List/Row`) | fg-quaternary, `_hover` | `#737373` / `#A3A3A3` |
| Section heading "FILES & PROJECTS", chevrons | `TEXT_SECONDARY`, `ICON_TERTIARY` | `TEXT_TERTIARY`, `ICON_QUATERNARY` | text-tertiary, fg-quaternary | `#A3A3A3`, `#737373` |
| Primary button | `#0D99FF` / `#0B84E0`, white label | `BG_BRAND` / `BG_BRAND_HOVER`, label `TEXT_ON_BRAND` (R4) | bg-brand-solid, `_hover` (Studio override) | `#E36323` / `#F97316`, `#0A0A0A` |
| Secondary button | `#1E1E22` + `#27272A`, hover `#27272A` | C `Button/Secondary`: fill `BG`, border `BORDER_STRONG`, label `TEXT_SECONDARY`; hover fill `BG_HOVER`, label `TEXT_SECONDARY_HOVER` | bg-primary, border-primary, text-secondary | `#0A0A0A` + `#404040`, `#D4D4D4`; `#171717`, `#E5E5E5` |
| Secondary-destructive button ("Empty trash", missing-file "Remove") | transparent + `DANGER #FE5938` | C `Button/SecondaryDestructive` sm (new name in C; plan.md C-K): rest fill `BG`, 1 px `DANGER` border (C asks W1 for `BORDER_ERROR_SUBTLE`, same value), label `TEXT_ERROR`; hover fill `BG_ERROR_PRIMARY`, label `UTILITY_RED_700` (C asks W1 for `TEXT_ERROR_HOVER`, same value); focus ring `FOCUS_RING` `#F97316` per R4, while C's state table draws the kit's focus-ring-error `#EF4444` (`FOCUS_RING_ERROR`): the controller confirms which (R4 says the ring is `#F97316`; listed for C); disabled 50 % opacity (C S5); kit receipt below | kit `Buttons/Button destructive · Hierarchy=Secondary`: border-error_subtle, text-error-primary, `_hover` | `#0A0A0A` + `#EF4444` + `#F87171`; `#450A0A` + `#FCA5A5` |
| Template and hub card CTAs (white-on-accent labels `:1974`, `:1994` today) | accent fill + `Color32::WHITE` label | C `Button/Secondary` sm (one brand action per view stays "+ Design file", rule 2) | — | `#0A0A0A` + `#404040`, label `#D4D4D4` |
| Filter/sort pills | `#1E1E22` + `#27272A` | C `Button/Secondary` sm (row above) with a trailing chevron in `ICON_QUATERNARY` | fg-quaternary | as above |
| Card, folder tile, quick-create tile, hub and template card | `#1E1E22`, stroke `#27272A`, hover `TEXT_TERTIARY` | `BG_SECONDARY` + `BORDER`; hover border `BORDER_STRONG` | bg-primary_alt, border-secondary; border-primary | `#171717` + `#262626`; `#404040` |
| Selected card | 2 px `#0D99FF` | 2 px `BORDER_SELECTED` (R10.3) | border-brand (Studio override) | `#FB923C` |
| Thumbnail stage | `#18181B` (`:2483`), picture frame white α 18 (`:2493`) | `HOME_THUMB_BG` (contract row 114), frame `BORDER` | bg-tertiary | `#262626`, `#262626` |
| List row selected | `UI3_SURFACE` + 1 px `UI3_BLUE` | `BG_SELECTED` (R8.2), no border (the 1 px blue edge is removed) | bg-secondary_hover | `#262626` |
| Card title / meta | `TEXT` / `TEXT_SECONDARY` | `TEXT` / `TEXT_TERTIARY` | text-primary / text-tertiary | `#FAFAFA` / `#A3A3A3` |
| "File not found" (card subtitle `:2555-2556`, list-row modified column `:2586-2587`) | `DANGER` | `TEXT_ERROR` (the text role; `DANGER` stays the icon and border foreground): 6.48:1 on the `BG_SECONDARY` card, 5.47:1 on a `BG_SELECTED` row, 7.16:1 on `BG` (C-1.12) (computed here, listed for W1) | text-error-primary | `#F87171` |
| File-type badge, template accents, quick-create and hub accents, "P" disc, starred-file icon | `#0D99FF`, `#F59E0B`, `#14AE5C`, `#FF6A3D` + white glyph | `Featured icon` with utility colours (R10.2): design file and the "P" disc `UTILITY_BLUE_50` / `UTILITY_BLUE_600`, board `UTILITY_AMBER_50` / `UTILITY_AMBER_600`, deck `UTILITY_GREEN_50` / `UTILITY_GREEN_600`, spec bundle `UTILITY_ORANGE_50` / `UTILITY_ORANGE_600` (tile / glyph); size: each tile keeps today's box (file-type badge 20 × 20 at `:2510-2528`, template, hub and quick-create icon boxes as today; R1), radius `RADIUS_CONTROL` 8 for tiles of 20 and above. C defines no `Featured icon` size below its Empty-state tile 32, so this is H's call until C names one (listed for W2). The "DESIGN-OS Studio Starter" card (`Act::OpenStarterSample`) takes the orange pair, as the spec-bundle card does | kit Featured icon utility colours | blue `#172554`/`#60A5FA`, amber `#451A03`/`#FBBF24`, green `#052E16`/`#4ADE80`, orange `#431407`/`#FB923C` |
| Star (on) | `#FFCD29` | `TEXT_WARNING` | fg-warning-secondary | `#FACC15` |
| Star backdrop, drag-source dim | backdrop black α 80/150 (`:2157`, `:2407`); dim black α 90 (`:2163`, `:2413`) | backdrop `BG_OVERLAY` (contract N33); only the star glyph sits on it, never text: on (`TEXT_WARNING`) 3.69:1 and off (`ICON_SECONDARY`) 3.82:1 on its worst background, a white thumbnail (composite `#676767`); the drag source is shown by C rule S5 (the dragged card at 50 % opacity), not by a scrim, because `BG_OVERLAY` over the `#171717` card would lighten it | bg-overlay at 70 % | `#262626` @ 70 %; — |
| Version pill, Trash count, "Board" badge | `#27272A` / `BG_MENU` + `TEXT_SECONDARY` | C `Badge/StatusCount` gray: fill `UTILITY_NEUTRAL_50`, border `UTILITY_NEUTRAL_200`, text `UTILITY_NEUTRAL_700` | kit Badge · Color=Gray | `#171717`, `#404040`, `#D4D4D4` |
| Workspace avatar tile | `#27272A` + `#3F3F46`, orange icon | fill `BG_TERTIARY`, border `BORDER_STRONG`, icon `ICON_BRAND_PRIMARY` | bg-tertiary, border-primary, fg-brand-primary (Studio override) | `#262626` + `#404040`, `#F97316` |
| Empty state | `ICON_TERTIARY`, `TEXT`, `TEXT_SECONDARY` | C `EmptyState/Default`: gray `Featured icon`, title `TEXT`, body `TEXT_TERTIARY` | kit Empty state | `#FAFAFA`, `#A3A3A3` |
| Drag ghost | `BG_BRAND` + white | `BG_BRAND` + `TEXT_ON_BRAND` | bg-brand-solid | `#E36323` + `#0A0A0A` |
| Breadcrumb current / parent / separator / hover | `TEXT` / `TEXT_SECONDARY` / `TEXT_TERTIARY` / `BG_HOVER` | `TEXT` / `TEXT_TERTIARY` / `TEXT_PLACEHOLDER` / `BG_HOVER` | text-primary / text-tertiary / text-placeholder / bg-primary_hover | `#FAFAFA` / `#A3A3A3` / `#737373` / `#171717` |
| Board thumbnail paper and dots | `BOARD_BG`, `BOARD_DOT` | unchanged (document content) | — | `#F4F4F4` |

**Kit receipt for the secondary-destructive button** ("Empty trash", "Remove"; read-only, R9: `figma-agent context <node> --dir <scratch>`, fileKey `VEffelx01mDl5BJJanwxja`, Buttons page `1:1183`, 2026-10-25). Component set `Buttons/Button destructive` node `6218:85578`; variants `Size=sm, Hierarchy=Secondary, State=Default` `3287:429437`, `State=Hover` `3287:429521`, `State=Focused` `3287:429493`, `State=Disabled` `3287:429465`, each 36 high (padding 8 / 12 bound, gap 4 bound, corner radius bound to radius-md 8). Dark-mode values: Default fill bg-primary `#0A0A0A`, 1 px stroke border-error_subtle `#EF4444`, label text-error-primary `#F87171`, text-sm semibold 14/20; Hover fill bg-error-primary `#450A0A`, label text-error-primary_hover `#FCA5A5`; Focused draws focus-ring-error `#EF4444` (C's state table keeps it as `FOCUS_RING_ERROR`; R4 says `#F97316`, so Studio uses `FOCUS_RING` unless the controller confirms C); Disabled at 50 % opacity; the skeuomorphic shadows resolve to alpha 0 (contract T10). Contract names: `BG`, `DANGER`, `TEXT_ERROR`, `BG_ERROR_PRIMARY`, `UTILITY_RED_700` (same values); C's pending names: `BORDER_ERROR_SUBTLE`, `TEXT_ERROR_HOVER`, `FOCUS_RING_ERROR` (the last one only if the controller confirms C over R4).

Contrast: the pairs H paints are rows of token-contract.md section (f) "Contrast check C-1" (acceptance 8; run its snippet; `ui ds a11y` is not a gate, it is a recorded false green): C-1.1, 1.2, 1.5, 1.7, 1.8 (text on chrome and cards), C-1.3 and C-1.9 (selected nav and list row, R8.2), C-1.12 (destructive-secondary label `TEXT_ERROR` on `BG`, 7.16:1), C-1.23 (its hover label `UTILITY_RED_700` on `BG_ERROR_PRIMARY`, 8.51:1), C-1.14 and C-1.15 (brand button and drag-ghost label, 5.73:1 and 7.06:1), C-1.24 (gray badge), C-1.29 (focus ring), C-1.31 to C-1.34 (category glyph on its tile, R10.2), C-1.37 and C-1.38 (selected card border on the page and on `HOME_THUMB_BG`, R10.3), C-1.39 (`TEXT_SECONDARY` on a hovered card or field). Named exceptions, not C-1 passes: E-1 (placeholder `#737373` in the search field, R8.1) and E-2 (1 px `BORDER_STRONG` `#404040` boundary of the search field and the secondary buttons, R10.7 and plan.md C-F; the field fill is `BG`, the kit Input's bg-primary). Computed here, not yet C-1 rows (listed for W1): hovered or selected nav label `TEXT_SECONDARY_HOVER` on `BG_HOVER` 14.23:1 and on `BG_SELECTED` 12.01:1; nav icon `ICON_QUATERNARY` on `BG` 4.18:1 and `ICON_QUATERNARY_HOVER` on `BG_SELECTED` 6.00:1 (non-text, 3.0); destructive border `DANGER` on `BG` 5.26:1 (non-text); star on `BG_OVERLAY` over a white thumbnail (composite `#676767`): on 3.69:1, off 3.82:1 (non-text, 3.0), which should replace C-1.40 (`TEXT_TERTIARY` on `BG_OVERLAY`: H paints no text there, and over a white thumbnail that pair is 2.24:1). Any label that must sit over a thumbnail sits on an opaque chip, never on the overlay.

**Table 2: type** (R5 Home density; weights and families per B; numbers stay in Roboto Mono, P5):

| Use | Today | After |
|---|---|---|
| Page title / breadcrumb | `font_title` 20 | `text-xl` 20/30 |
| Buttons, nav rows, card titles, list rows, search text, filter labels, trash rows, section headings in content | `font_body*` 11 | `text-sm` 14/20 |
| Card meta, template and hub descriptions, sidebar section heading, version pill, badges | `font_caption*` 10, literal 10/12 | `text-xs` 12/18 |
| Workspace name | literal 12 medium (`:897`) | `text-sm` 14/20 (weight per B) |
| Empty state title | `font_large_strong` 13 | `text-md` 16/24 |

**Table 3: sizes and their layout effect** (R1 quantified; "fits" means the new line height plus Untitled padding fits in today's box):

| Element | Line | Today | After | Δ height | Layout effect |
|---|---|---|---|---|---|
| Header buttons | `:1395` | 32 | button sm 36 | +4 | none: centred in the 64 header |
| Filter/sort pills, view toggles | `:1290`, `:1304`, `:1320` | 28 | button sm 36 | +8 | filter band 38 → 46 (`:1327`; today's 4 above / 6 below margins kept, section 5); every content view starts 8 px lower |
| Sidebar search | `:917` | 32 | `Input field · Size=sm` (C; expected 36) | +4 | items below move 4 px down |
| Workspace header | `:887` | 32 | 36 | +4 | items below move 4 px down |
| Nav rows (8 fixed rows plus tree rows) | `:864` | 28 | 36 (decided: controller default, H OQ1) | +8 each | fixed rows push the project tree down 7 × 8 = 56 px plus the 8 px above; the tree list is a scroll area (`:991-994`) and absorbs it. Bottom block (Starred, Trash) keeps its anchor to the bottom edge. Sidebar width unchanged. |
| Starred file rows | `:1088` | 24 | 24 (text-xs 18 fits) | 0 | none |
| Trash count, version pill | `:1114`, `:903` | 16 | `Badge` sm 22 (C) | +6 | none (inside a 28-36 row) |
| File card meta block | `theme.rs:705`, `home/mod.rs:2553-2557` | 58 | 62 (12 + 20 + 18 + 12) | +4 per card row | grid rows 4 px taller; card height `HOME_THUMB_H` + local `CARD_META_H` 62 (contract keeps `HOME_CARD_H` 198) |
| List rows | `HOME_LIST_ROW_H` 44 | 44 | 44 (fits 20) | 0 | none |
| Folder tiles | `HOME_FOLDER_H` 48 | 48 | 48 (fits 20) | 0 | none |
| Trash rows | `:2599` | 52 | 52 (fits 20 + 18) | 0 | none |
| Drafts quick-create tiles | `:2243` | 48 | 48 (4 + 20 + 18 + 4 = 46 fits, centred) | 0 | none (`drafts_strip_h` 64, `:2204`, unchanged) |
| Template cards (four, five with the spec-bundle card) | `:1488` (placed by hand), CTA `:1560` | 188, CTA 26 | 204, CTA button sm 36 | +16 | template grid rows 16 px taller (`template_rows`, `:1490`, placed by hand) |
| Hub card CTAs (one per card since v0.7.0) | `:1968-1969` (the unused two-button branch), `:1990` | 26 | button sm 36 | +10 | hub card height grows 10 (`hub_rows_h`, `:1873`) |
| "Remove" on a missing file | `:2150`, `:2400` | 32 (`button()`) | `Button/SecondaryDestructive` sm 36 | +4 | none: it overlays the card meta block at the card's bottom right (centre `HOME_PAD` above the card bottom) and the list row's right edge, as today |
| Empty state | `:2181-2187` | icon 32, 13 + 11 text | `Empty state · Size=sm` | about +20 | centred; no neighbour |
| Drag ghost | `:560` | 28 | 28 (text-sm 20 + 2 × `SPACE_FACT` 4) | 0 | follows the pointer |

Radii: every literal `CornerRadius::same(5|6|8)` and `cr_pill` on controls takes B's `radius-md` 8 (controls) or `radius-full` (badges, pills only where C's anatomy says so); cards take B's card radius (`radius-xl` 12 expected). Today's 43 radius literals in `home/mod.rs` (`spacing_baseline.json`) fall to the allowlisted remainder; nested shapes stay concentric (P4, `theme::concentric`).

**States** (C defines the anatomy; this spec lists which ones Home must show): buttons default, hover, pressed, disabled (none disabled today), focus (focus ring `#F97316`, R4); nav rows default, hover, selected, drop target, focus; inputs default, focus, filled; cards default, hover, selected, drag source (the dragged card at 50 % opacity, C rule S5; no scrim); menus open/closed; empty state per section (`empty_state` texts unchanged).

## 8. Edge cases

1. Window at `MIN_WINDOW` 900×600: rule 7 (breadcrumb elides; no overlap). Project sections add "+ New folder", the widest case.
2. Long file and project names: elide with "…" at the new 14/12 sizes (`widgets::elide` today); never overlap the type badge or star.
3. Many projects: the tree scrolls; with 36-high rows the visible tree shrinks by 64 px at 1088 tall (about 2 rows).
4. Starred expanded with 3 files plus a 600 px window: the bottom reserve (`:988`) must still leave the tree at least 64 px (`:991`, `max(y + 64)`).
5. Search active: the header title is "Search results" style (`h.title()`); filter and sort still work; Escape clears (rule 8).
6. Trash empty / with items: "Empty trash" shows only with items (`:1240`).
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

**Shared helper** (new names), file `crates/designos-app/src/shell/tests/chrome_census.rs` (test-only, owned by H), registered with one `mod chrome_census;` line in `shell/tests.rs`; specs I, J and K reuse it, and if I, J or K lands before H, that PR adds the file exactly as written here. It is the section 2 census in Rust and must print what the Python census prints:

- `clean_rust(src: &str) -> String` ports `scripts/rust_literal_scan.py` `clean` exactly: first `strip_code` (`:52`): blank `//` comments to the end of the line; blank nested `/* … */` comments, keeping their newlines; replace a raw string `b?r#*"…"#*` (not preceded by an identifier character) and a normal string `"…"` (backslash escapes skipped) by `""` plus the newlines they spanned; replace a char literal (`'x'` or an escape such as `'\n'`, `'\u{..}'`) by `' '`, leaving lifetimes alone. Then `drop_cfg_test` (`:101`): repeatedly find the next `#[cfg(test)]`; scan forward from it: a `{` raises the depth and marks the item as started, a `}` lowers it and ends the item when started and back at depth 0, a `;` before any `{` ends the item; replace the attribute and the item with the same number of newlines. Line numbers never move. It does **not** stop at the first `#[cfg(test)]`: `home/mod.rs:11-12` (`#[cfg(test)] mod spec_bundle_hidden_tests;`), `cmdk.rs:86`, `present.rs:123`, `panels/assets.rs:52`, `terminal.rs:550` and `skills/mod.rs:503` all have one early, and everything after them is shipped code.
- `chrome_colour_sites(src: &str) -> Vec<usize>` runs `clean_rust`, then returns the 1-based line of every match (one entry per match, duplicates kept, ascending) of the section 2 `HEX` pattern, matched by hand because the crate has no `regex` dependency (do not add one for a test): `Color32::from_rgb(`, `from_rgba(`, `from_rgb_unmultiplied(`, `from_rgba_unmultiplied(`, `from_rgb_premultiplied(` or `from_rgba_premultiplied(` followed by optional whitespace, an integer literal (decimal or `0x…`), optional whitespace and `,`; `Color32::from_gray(` + integer literal + `)`; `Color32::from_black_alpha(`; `Color32::from_white_alpha(`; or `Color32::` + one of `WHITE`, `BLACK`, `GRAY`, `LIGHT_GRAY`, `DARK_GRAY`, `RED`, `GREEN`, `BLUE`, `YELLOW` not followed by an identifier character. `TRANSPARENT` and computed constructors (`from_rgb(r, g, b)`) are not sites. The helper applies **no** allowlist, so its output equals the Python census.
- `texture_tint_line(clean_line: &str) -> bool`: true for a cleaned line that holds `Color32::WHITE` and contains `.image(` or `with_texture`. Each surface test removes such lines, and the content lines its own spec lists, from the helper's output; the allowlist lives in the surface tests, not in the helper.
- `chrome_census_print` (`#[ignore]`): reads each path in the environment variable `CHROME_CENSUS_FILES` (space-separated, relative to `crates/designos-app/src/`) with `std::fs::read_to_string` and prints `<file> chrome_sites=<n> <sorted unique lines>` in the Python census's format, for the parity check (acceptance 9).

The colour-literal gate's owner is spec A (D proposes `scripts/check_chrome_colour_literals.py`, new name); the Rust helper is added as well because the Python design gates are advisory in CI and only `cargo test` on macOS blocks a merge.

| Case (new names unless marked) | Where | Expected | Fails before |
|---|---|---|---|
| `helper_blanks_only_the_cfg_test_item` | `shell/tests/chrome_census.rs` | fixture `"#[cfg(test)]\nmod x;\nlet c = Color32::from_rgb(1, 2, 3);\n"` → `[3]`; fixture `"#[cfg(test)]\nmod t {\n    fn f() { let _ = Color32::RED; }\n}\nlet g = Color32::from_gray(10);\n"` → `[5]` | yes (helper new) |
| `helper_skips_comments_and_strings` | same | fixture `"// Color32::WHITE\nlet s = \"Color32::WHITE\";\n/* Color32::BLACK\n */ let c = 'x';\n"` → `[]`; fixture `"a(Color32::WHITE, Color32::BLACK);\n"` → `[1, 1]`; `Color32::TRANSPARENT` and `Color32::from_rgb(r, g, b)` → `[]` | yes (helper new) |
| `home_sources_paint_no_chrome_colour_literals` | `shell/tests/home.rs` | `chrome_colour_sites(include_str!("../../home/mod.rs"))` minus `texture_tint_line` lines returns `[]` (the two tints at `:2463`, `:2492` today); `[]` for `home/thumb.rs`, `home/time.rs`, `home/session.rs` | yes (38 at `8dfaee93`, the same lines as section 2) |
| `home_nav_and_list_rows_use_list_row_look` | same | the sidebar fill is `theme::BG`; the nav-row and list-row painters take fill, label and icon from C's `kit::rows::row_look(State, on_surface)`: hover `BG_HOVER`, selected `BG_SELECTED`, label `TEXT_SECONDARY_HOVER`, icon `ICON_QUATERNARY_HOVER`; the selected list row draws no stroke | yes (`UI3_*`) |
| `home_thumb_stage_uses_home_thumb_bg` | same | shipped `home/mod.rs` paints the stage with `theme::HOME_THUMB_BG` and has no `let _ = theme::HOME_THUMB_BG` | yes |
| `home_empty_trash_is_destructive_secondary` | same | `main_view::button_colours(Kind::SecondaryDestructive)` (new name) at rest is `BG` / `DANGER` border / `TEXT_ERROR`, hovered `BG_ERROR_PRIMARY` / `UTILITY_RED_700`; it equals C's `Button/SecondaryDestructive` look, and `home-empty-trash` and `home-remove-<name>` both draw it | yes |
| `home_missing_file_uses_error_roles` | same | a library file whose disk file is absent (injected in `home_sim()`): the card subtitle and the list-row modified column read "File not found" in `TEXT_ERROR`, not `DANGER`; `home-remove-<name>` records a 36-high rect | yes (`DANGER`, 32) |
| `home_has_no_figma_blue_or_ui3_palette` | same | shipped `home/mod.rs` contains neither `UI3_` nor `PrimaryBlue` nor `0x0D, 0x99, 0xFF` | yes |
| `home_primary_button_is_brand_with_dark_label` | same | a `home_sim()` frame records `home-new-file`; the button's fill/label pair returned by `main_view::button_colours(Kind::Primary)` (new name, a pure fn the painter uses) equals A's brand fill and the R4 label | yes |
| `home_header_buttons_are_button_sm` | same | recorded `home-new-file`, `home-import` rects are 36 tall and vertically centred in the 64 header | yes (32) |
| `home_filter_bar_controls_are_button_sm` | same | `home-filter`, `home-sort`, `home-view-grid` rects 36 tall; content top = header 64 + 46 + 1 | yes (28, 38) |
| `home_layout_regions_unchanged` (guard) | same | sidebar right edge 264; header 64; first card x = sidebar + 32; card width 224 | guard |
| `home_breadcrumb_never_overlaps_header_buttons` | same | `Sim` at 900×600 in a project section with a long name: breadcrumb right edge + 12 ≤ leftmost button left. The code has only interact ids `("home-crumb", i)` (`:1374`) and records no rect, so H adds `widgets::record(ui, &format!("home-crumb-{i}"), r)` for each crumb (new record ids; behaviour unchanged) | yes at 14 px type |
| `home_sidebar_nav_rows_follow_home_density` | same | `home-recents`, `home-trash` rects 36 tall | yes (28) |
| existing `home::tests::*` (`home/mod.rs:2675`, `:2751`, `:2792`) and `shell::tests::home::*` (marked) | — | pass unchanged | guard |
| `scripts/zero_fake_ui_gate.py` Z09, Z14 (marked) | — | PASS | guard |
| `scripts/check_spacing_relationships.py` (marked) | — | PASS; then `--write-baseline` lowers `home/mod.rs` and removes the two dead entries; it refuses any rise | guard |
| A's theme tests and G01 parity (marked) | `theme.rs` tests, `design_os_self_evolve.py` | PASS | A's |

Run: `cargo test -p designos-app --bin designos chrome_census`, `cargo test -p designos-app --bin designos home`, `cargo test -p designos-app --bin designos shell::tests::home`, `python3 -B scripts/check_spacing_relationships.py`, `python3 -B scripts/zero_fake_ui_gate.py`, the section 2 census, `cargo fmt --all --check`, `cargo clippy -p designos-app --all-targets -- -D warnings`.

**Acceptance criteria.**

1. The section 2 census prints `chrome_sites=2` for `home/mod.rs` (lines of the two texture tints only) and 0 for the other live `home/` files; `home/sidebar.rs` and `home/main_view.rs` no longer exist and the crate builds. On the branch base, `home_sources_paint_no_chrome_colour_literals` fails with the 38 lines of section 2.
2. "+ Design file" renders `#E36323` with a `#0A0A0A` label; no `#0D99FF` pixel remains in Home chrome (selected card included) in the after screenshots.
3. Layout: L's gate passes for Home (same regions, widths and order); every height change is one listed in section 7 table 3.
4. Type: Home text uses `text-sm`/`text-xs`/`text-md`/`text-xl` from B; `home/mod.rs` has no font-size literal (`check_spacing_relationships.py --verbose` lists no `font_literals` for it).
5. Rule 7 holds at 900×600 (test and screenshot `H-03`).
6. All section 10 tests pass; Z09 and Z14 pass; the spacing ratchet passes and its baseline only went down.
7. The PR shows before and after for every capture below, side by side; states no capture reaches are listed as "not captured — covered by test <name>" (below).
8. Contrast: the `token-contract.md` section (f) snippet exits 0. H's rows: C-1.1, 1.2, 1.3, 1.5, 1.7, 1.8, 1.9, 1.12, 1.14, 1.15, 1.23, 1.24, 1.29, 1.31–1.34, 1.37, 1.38, 1.39. Named exceptions, which are not C-1 passes: E-1 (placeholder `#737373`, R8.1) and E-2 (1 px `#404040` search-field and secondary-button borders, R10.7 and plan.md C-F). `ui ds a11y` is not a gate. The pairs section 7 lists as "computed here" are shown in the PR with their ratios until W1 adds them to C-1.
9. Helper parity: on the branch base, `CHROME_CENSUS_FILES="<files>" cargo test -p designos-app --bin designos chrome_census_print -- --ignored --nocapture` prints, for every file in the H, I, J and K census lists, the same `chrome_sites` count and the same line list as that spec's section 2 Python census (`REV=<base>`); at `8dfaee93` the totals are H 45 (38 live + 7 dead), I 70, J 35, K 32 (re-run 2026-10-28, exit 0 for all four blocks). The PR pastes both outputs.

**Evidence captures** (Tier B PNGs; before = L's baseline blessed on the branch base or a build of that base, after = the PR build; both in the PR). Captures run only in L's hermetic mode, through L's capture script (`scripts/layout_baseline_capture.py`; H's case is `h-home`, `--only-spec H`) once L lands. Until then, or for a case L's script cannot express (H-03 needs `--size 900x600`, while the script passes `--size 1440x900`), run the binary by hand in the same isolation: `env -i`, scratch `HOME`, `TMPDIR` and `DESIGNOS_LIBRARY`, `PATH` without any agent CLI. Never point `--library` or `DESIGNOS_LIBRARY` at the owner's library: the public repo must not show a client file (the old `evidence/baseline-home.png` shows one and must not be used). Planning-round CLI runs use `tools/designos-latest` (v0.7.0, the controller's build for this round; the 0.6.2 pin of plan.md 22:40 is superseded); the implementer's before and after use the branch-base and PR builds.

```bash
BIN=<build dir>/release/designos                       # branch-base build (before) or PR build (after)
S="$(mktemp -d)"; mkdir -p "$S/home" "$S/tmp" "$S/stubs" "$S/out" "$S/first-run"; LIB="$S/lib"
RUN() { env -i HOME="$S/home" PATH=/usr/bin:/bin TMPDIR="$S/tmp" DESIGNOS_LIBRARY="$1" DESIGNOS_AGENT_CLI_DIR="$S/stubs" "$BIN" "${@:2}"; echo "exit $?"; }
RUN "$LIB" --seed-library "$LIB"                                                                        # 4 sample files (project "Marlow")
RUN "$LIB" --home --library "$LIB" --screenshot "$S/out/H-01-home-recents.png" --size 1600x1088 --no-toast        # sidebar, header, filter bar, 4 cards (incl. a board)
RUN "$S/first-run" --home --library "$S/first-run" --screenshot "$S/out/H-02-home-first-run.png" --size 1600x1088 --no-toast  # first run seeds one "Welcome" file
RUN "$LIB" --home --library "$LIB" --screenshot "$S/out/H-03-home-min-900x600.png" --size 900x600 --no-toast       # minimum window, header crowding (rule 7)
```

`DESIGNOS_AGENT_CLI_DIR` is honoured once L lands (an empty stub dir: Home starts no agent). The spec writer's earlier H-01..H-03 runs (2026-10-25, a build of `ec47da26`, exit 0) used a scratch `--library` but the host `PATH` and `HOME`, so they are not isolated by L's rules; their PNGs are not PR evidence. If a run prints `designos: screenshot was not captured` and exits 1 (seen 5 times in 37 runs under parallel load; spec R), re-run it once and note it in the PR.

Not capturable from the CLI today (no flag selects a Home section, the list view, a hover or a menu): Templates & Specs, Drafts, All folders, Resources & Libraries, Starred, Trash (with and without items), list view, filter and sort menus open, context menu, drag ghost, empty states, focus rings. Each is either rendered from injected state in L's `Sim` tier (same scratch library) or listed in the PR as "not captured — covered by test <name>" (section 10 tests). A manual window capture, if any, uses the same scratch library and is labelled manual. A `--home-section` flag would be a separate issue.

## 11. Performance budgets

- No new allocation per frame beyond today's (colour constants are `const`); no `doc.to_json()` (CLAUDE.md invariant 1); Home draws no document.
- Thumbnail loading budget unchanged (`budget = 4u32` textures per frame, `:1506` (placed by hand), `:1890`, `:2219`).
- Idle: Home requests no repaint when idle (unchanged).

## 12. Dependencies, ownership and build order

**Build order.** L captures the Home baseline in hermetic mode (`h-home`; `H-01`..`H-03` above) → A and B land the colour roles and Home sizes → C lands the widgets, including `Button/SecondaryDestructive` and `List/Row` → this spec. If C is not merged, H paints the anatomy with A/B constants inside `home/mod.rs` and swaps to C's widgets in a follow-up only if C's API differs; do not wait on C for colours.

**Technical decisions recorded here** (the owner may reverse): (1) delete the two dead files rather than move the inline modules into them (smallest diff, no behaviour); (2) Home sidebar = editor left panel = `BG` (spec E), so Home and editor read as one app, and nav rows use C's `List/Row` on that `bg-primary` surface; (3) the Rust literal test is added even though A owns the Python gate, because only `cargo test` blocks a merge; (4) hub, quick-create and template accents follow the same categorical mapping as the file-type badges (one rule, R10.2).

**File ownership across H–K.** Each file has exactly one owner; the assignments are the same as the table in I, J and K, and this one adds a note per file.

| File | Owner | Note |
|---|---|---|
| `home/mod.rs` | H | the updater card call (`:1106`) draws J's `updater.rs`; the menus call C's `widgets::menu`; v0.7.0 added the missing-file state and dropped the Marlow templates and starter kits (drift note) |
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

Not mine (D–G or the kit specs): `theme.rs`, `design/*` (A, B); `widgets/*` except `widgets/picker.rs`, `panels/mod.rs` (C); `tabs.rs`, `shell.rs` (including its toast, which D paints by J's rules), `panels/rail.rs`, `panels/toolbar.rs`, `panels/menus.rs`, `canvas/rulers.rs` (D); `panels/left.rs`, `panels/layers.rs`, `find/filter.rs` (E); `panels/right.rs`, the inspector files, `widgets/picker.rs`, `panels/style_picker.rs`, `panels/style_edit.rs`, `panels/style_edit_grid_rows.rs` (F); `dev/*`, `specs.rs` (G). `shell/tests.rs` is L's (`Sim`, `:69-154`; `Sim::rect_of` `:202`): each H–K PR only appends one `mod` line to its module list (`:1124-1201`). `scripts/spacing_baseline.json` is a shared ratchet: each surface PR may only lower its own files' counts with `--write-baseline`.

Assigned by the controller (plan.md, H–K check rulings, 2026-10-29 01:05: `lint.rs` K, `canvas/*` and `decks/ui.rs`, `board/*` D, `canvas/proto.rs` F, `icons.rs` C; were unassigned, recommendation in brackets): `decks/ui.rs` [D], `board/*` [D], `canvas/draw.rs`, `canvas/grids.rs`, `canvas/tiles.rs`, `canvas/interact.rs` [D or L], `canvas/proto.rs` (4 chrome sites, the prototype-flow overlay; named by no spec) [F or L], `icons.rs` [C, R6], `lint.rs` (`:297`, a non-chrome fallback) [K]. `shell/toast_layout.rs` is D's (D table); `panels/left_sep.rs` is E's; `dev/code_view.rs` is G's.

**Open PRs touching H files** (`gh pr list --state open`, 2026-10-28, 57 open; file lists from the same call): #1206 (draft; L's hermetic mode, touches `home/mod.rs` and `home/main_view.rs`), #1198 (draft; `home/leave.rs`, `home/session.rs`: library edits after a failed save); stale #765, #768, #770 (opened 2026-10-19, `home/mod.rs`, `home/session.rs`; #765 `CONFLICTING`) — the PM decides their fate before H starts. Re-run the census after each rebase.

## 13. Owner rulings that apply

- R1: "sử dụng 100% style kiểu của Untitled UI nhưng vẫn giữ cấu trúc layout và bố cục hiện tại" (all of Untitled's style; keep layout and arrangement).
- R2: take Untitled spacing and typography as they are, adapted to Studio's layout.
- R3: Studio improves itself; no Figma. Evidence goes in the PR.
- R14 (owner, 2026-10-28 23:05): Studio's design store (tokens, registry, soul) uses Studio's own names and values only. H follows it: every colour and widget is named by its Studio constant (contract) or C widget name; Untitled kit roles and component names appear only in the "Untitled reference" column and in the kit receipts, never as a name to create in the store.
- R4: brand = Untitled anatomy with Studio orange `#E36323`, dark label `#0A0A0A`; focus ring `#F97316`; Purple Ban stays (every kit brand role resolves to the orange family, never the kit purple `#7F56D9`).
- R5: Home and dialogs use sm/md (text-sm 14/20, button sm 36 / md 40); the 2 px and 6 px spacing ban is lifted.
- R6: keep the icon set; restyle colour to Untitled fg roles.
- R7: spec-driven; the PM coordinates delivery.
- R8.1 placeholder `#737373` (named exception); R8.2 selected `#262626`; R8.6 button xs label text-sm 14 semibold (buttons only); R8.7 destructive hover `#B91C1C` applies to solid destructive buttons (`BG_ERROR_SOLID_HOVER`): H has none; its one destructive button ("Empty trash") is the kit's secondary destructive (outline), C `Button/SecondaryDestructive`, whose hover is the error tint `BG_ERROR_PRIMARY` (plan.md C-K).
- R10.2: "Home category colours = kit Featured-icon utility tiles (blue, amber, green, orange; -50 tile, -600 glyph)"; the "P" disc takes the design blue (coordinator, board 3).
- R10.3: "Selected Home file card = 2 px `BORDER_SELECTED` `#FB923C`".
- plan.md C-F (controller, 22:30): "Field fill = kit Input `bg-primary`; E-2 covers every 1 px kit `border-primary` `#404040` control boundary: unchecked checkbox and radio (R10.7), text field and select, secondary button".
- plan.md C-K (controller, 22:35): "Radii and variants follow the live kit (R2), each with a recorded read-only receipt: dialogs/modals = kit Modals; command bar = kit Command menu; toasts = kit Notification; menus/dropdowns/popovers 16 (R8.5). Segmented controls outside R10.1's three strips keep today's height (R1). Controls inside a tab bar match its tabs (contract (d); agent header "Sessions", `+`, Skills, Stop stay 24). Home "Empty trash" uses the kit's secondary-destructive button (C adds it)". H's receipt is in section 7 ("Kit receipt for Empty trash").
- plan.md (owner, 22:40, superseded 2026-10-28): the planning round first pinned the 0.6.2 build; fix round 3 uses `tools/designos-latest` (v0.7.0), and code citations are at `origin/main@8dfaee93`.
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

## Changelog (fix round 2, curator review `reviews/HIJK-curator-review.md`)

- H-1 (blocker): section 10 helper rewritten as an exact port of `rust_literal_scan.clean` (`strip_code` + `drop_cfg_test`, every annotated item, not "from the first `#[cfg(test)]`"); helper returns raw sites, allowlist moved to the surface tests (`texture_tint_line`); fixture self-tests `helper_blanks_only_the_cfg_test_item` and `helper_skips_comments_and_strings`; `chrome_census_print` and acceptance 9 (helper/Python parity at the branch base, 45 / 71 / 35 / 32 at `205c72ef`). Covers I-4, J-5, K-2.
- H-2: sidebar fill `BG`; nav rows are C `List/Row` sm (hover `BG_HOVER`, selected `BG_SELECTED`, label `TEXT_SECONDARY` → `TEXT_SECONDARY_HOVER`, icon `ICON_QUATERNARY` → `ICON_QUATERNARY_HOVER`); decision (2) restated; test `home_nav_and_list_rows_use_list_row_look`.
- H-3: thumbnail stage `HOME_THUMB_BG` `#262626` (contract row 114), frame `BORDER`, `let _ =` line removed; C-1.38 cited; test `home_thumb_stage_uses_home_thumb_bg`.
- H-4 (plan.md C-K): "Empty trash" = C `Button/DestructiveSecondary` (new name, C adds it) with the kit receipt (set `6218:85578`, sm variants `3287:429437` / `429521` / `429493` / `429465`); hover label `UTILITY_RED_700` makes its pair C-1.23 (8.51:1), so no new contract row is needed; `Kind::Danger` → `Kind::DestructiveSecondary`; `BG_ERROR_SOLID` dropped from H's consumed list; test `home_empty_trash_is_destructive_secondary`.
- H-5: acceptance 8 (contrast rows, E-1 and E-2 with C-F, no `ui ds a11y`); contrast paragraph cites C-1.3, 1.9, 1.12, 1.23, 1.38, 1.39, drops "handed to W1" for rows that now exist and lists the remaining computed pairs.
- H-6: row renamed "Star backdrop, drag-source dim"; drag source = C S5 50 % opacity; star ratios over a white thumbnail (3.69 / 3.82) recorded; C-1.40 replacement listed for W1; opaque chip rule for any label over a thumbnail.
- H-7: "no 6 px step" claim replaced: drag ghost 28 and Drafts tiles 48 recomputed on the 4 px ladder (both Δ 0, strip unchanged); the filter band's 6 px bottom margin and the 6 px picture inset are today's literals, kept (R1).
- H-8: rows added for template and hub CTAs (`Button/Secondary` sm), list-row selected (`BG_SELECTED`, blue edge removed), Featured-icon size (today's boxes; C has no smaller size, listed for W2); nav label per C.
- H-9: stroke citation `:2613-2619` → `:2611-2612` / `:2618`; breadcrumb test records `home-crumb-<i>`.
- Captures: evidence block rewritten for L's hermetic mode (`env -i`, scratch `HOME` / `TMPDIR` / `DESIGNOS_LIBRARY`, `DESIGNOS_AGENT_CLI_DIR`); earlier non-isolated runs marked "not PR evidence"; owner-library ban and the `baseline-home.png` warning stated; uncapturable states go to `Sim` or "not captured — covered by test <name>".
- Coordinator amendment: 0.6.2 build for planning-round CLI runs (plan.md 22:40); C-F and C-K quoted in section 13.


## Changelog (fix round 3, 2026-10-28: re-anchor to `origin/main@8dfaee93`, plan.md C-K / C-R, R14)

Review re-check against `reviews/HIJK-curator-review.md` (each id read on the spec text, not on the round-2 changelog):

- H-1 (blocker): present and unchanged in substance: the section 10 helper is an exact port of `rust_literal_scan.clean`, the self-tests and acceptance 1 and 9 exist. Numbers re-run at `8dfaee93`: Python census exit 0, H 45 (38 live + 7 dead), I 70, J 35, K 32; for `home/mod.rs` the mapped lines equal the census output line for line (`main_view.rs` has no mapped list, H:33; its 7 hits are dead code).
- H-2: present: sidebar `BG`, nav rows C `List/Row` (hover `BG_HOVER`, selected `BG_SELECTED`), table 1 and test `home_nav_and_list_rows_use_list_row_look`.
- H-3: present: thumbnail stage `HOME_THUMB_BG` at `:2483`, the `let _ =` line (`:2482`) removed, test `home_thumb_stage_uses_home_thumb_bg`.
- H-4: present and renamed to C's final name: "Empty trash" is `Button/SecondaryDestructive` (plan.md C-K; C added it under that name, so `DestructiveSecondary` and `Kind::DestructiveSecondary` became `SecondaryDestructive`); the missing-file "Remove" button joins it.
- H-5: present: acceptance 8 lists the contrast rows and the E-1 / E-2 exceptions. The rows exist in the recorded C-1 run (`evidence/AB-fix-round-261028/c1-contrast-check-at-8dfaee93.txt` section 1, 70/70 pass, 4/4 floors held, exit 0); not re-run here because the snippet needs a theme built from contract (a).
- H-6: present: "Star backdrop, drag-source dim" row with C rule S5; the replacement of C-1.40 by the two star rows over `#676767` is still listed for W1 (the contract still holds C-1.40).
- H-7: present: no new 6 px step; the two 6 px literals (`:1287`/`:1327`, `:2486`) stay (R1).
- H-8: present: CTA, list-row selected and Featured-icon rows; nav label per C.
- H-9: present: stroke citation now `:2538-2539` / `:2545`; the breadcrumb test records `home-crumb-<i>`.
- X-1 (cross-spec: stale ownership text in D, E, F): D refreshed its ownership on 2026-10-28 (`updater.rs` and the files H–K own are in its not-mine list; `shell/toast_layout.rs` is D's own, D:9); E now says `image_jobs.rs` is I's and F says `check_designs.rs` is K's (both cite plan.md 22:20); the H–K ownership tables already matched.
- X-2 (contract (d) anchor): the contract now cites `agent_open_flow.rs:418-419` for the Sessions button (not `:396-397`); I cites `:414-420`; nothing to change in H–K.
- X-3 (`lint.rs` has no owner): still open; every H–K spec flags it to K (unassigned list; the non-chrome fallback is at `:297` at `8dfaee93`); the controller decides.
- X-4 (live-kit component receipts): answered in the specs, not in a file (this round writes specs only): node ids, bound variables and values are in J rules 2, 3 and 14, I rule 8 and H section 7; the controller may add `evidence/figma-live-component-radius.md`.
- X-5 (`evidence/baseline-home.png` shows an owner file name): the warning stays in H's capture paragraph; the file is still in `evidence/` and must stay out of public PRs (plan.md R13 purges it from PR #1123).
- Re-anchor: every citation moved from `205c72ef` to `8dfaee93` (line diff; changed cites placed by code text: `card` `:2530-2558`, meta `:2553-2557`, `list_row` `:2565-2590`); section 2 census table, tints (12 to 11) and file sizes refreshed; drift note added; the starter-kit row (`:2453-2463`) citation dropped because the row is gone.
- Drift that changed the surface: blank-creation Templates & Specs cards, one-CTA hub cards, no starter-kit row, missing-file state ("File not found" `TEXT_ERROR`, "Remove" `Button/SecondaryDestructive` sm), recorded ids (rule 8), table 3 rows, test `home_missing_file_uses_error_roles`.
- R14: table 1 now has a "Studio constant" column and a separate "Untitled reference" column; section 5 lists C's widget names first and the kit names as references; ruling added to section 13. The kit receipt keeps the kit names (it is evidence).
- plan.md C-K / C-R: "Empty trash" and "Remove" use C's secondary-destructive button, hover fill `BG_ERROR_PRIMARY`; no tab bar in Home, so the tab-bar rule does not apply; open PRs and the unassigned-surface list refreshed (`canvas/proto.rs` added).
