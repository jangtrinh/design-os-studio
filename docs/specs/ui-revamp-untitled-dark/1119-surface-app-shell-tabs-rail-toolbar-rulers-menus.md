# D App shell: title bar, tab bar, left rail, bottom toolbar, rulers and the shell menus in Untitled UI dark

| Field | Value |
|---|---|
| Issue | #1119 (epic #1118, spec key D) |
| Size | M. About two to three days: 6 owned files, 39 chrome colour sites, the R5 type swap on four bars, the menu row height from spec C. No new behaviour. |
| Depends on | A (colour roles and the `theme.rs` colour constants), B (type, spacing, radius and control-size scale), C (widget anatomy and states, `specs/C-widget-kit-untitled-anatomy.md`) and L (layout-unchanged gate and the baseline captures). Also #1106 (icon-only rail, in flight), see section 12 and Open question 2. |
| Unblocks | M (docs and screenshot refresh). J reads section 5: the command palette inside `panels/menus.rs` is restyled here, not in J. |
| Touches | `crates/designos-app/src/tabs.rs` (`:19-29`, `:48-54`, `:895-1212`); `crates/designos-app/src/shell.rs` (`:71`, `:989-1016`, `:1180-1182`, `:1319-1345`, `:1382`, `:1457-1509`, `:3403-3490`); `crates/designos-app/src/panels/rail.rs` (`:36-212`); `crates/designos-app/src/panels/toolbar.rs` (`:109-444`); `crates/designos-app/src/panels/menus.rs` (`:417-842`, `:879-1057`); `crates/designos-app/src/canvas/rulers.rs` (`:373-498`). Reads only (owned by A and B): the shell constants in `crates/designos-app/src/theme.rs` (`:22-47`, `:263-277`, `:398-455`, `:524-541`). |

Code baseline: HEAD `ec47da26` (the local `main` this spec was written on). `origin/main` is `35a65796`, four commits ahead (release 0.6.1). It changes five of this spec's files: `shell.rs` 308, `panels/menus.rs` 104, `tabs.rs` 93, `panels/toolbar.rs` 82 and `panels/rail.rs` 34 changed lines (`git diff --stat ec47da26 35a65796 -- <files>`; `canvas/rulers.rs` unchanged). The colour-site counts in section 2 are identical at `35a65796` (re-measured on a `git archive` export); only line numbers move. Re-anchor every cited line against the HEAD you branch from and report drift before writing code.

Colour names follow the plan's naming contract: Untitled token name plus the dark-mode hex, resolved this session from the kit export (`--export-variables --format dtcg`, then `evidence/resolve-untitled-tokens.py`). Brand roles carry the R4 remap. Where spec A or C is not yet written, a role is marked provisional and A's token contract wins on conflict. Names that Untitled does not have are marked "new name".

## 1. Summary and user story

As a designer working in Studio, I see the window chrome (title bar, file tabs, left rail, floating toolbar, rulers and the shell menus) in Untitled UI dark: the same neutral greys, borders, type scale, radii, states and focus rings as the rest of the revamped app, with Studio orange as the only brand colour. Nothing moves: every region, width and control order stays where it is today (R1). Heights change only where R5 raises a control or a type size, and section 3 lists each one.

## 2. Current state (FACT unless labelled)

Inventory of the surface (HEAD `ec47da26`):

- **Title bar** (`shell.rs:1319-1345`). Height `TOP_TITLE_BAR_HEIGHT` 28 (`shell.rs:71`). Fill `#18181B` and bottom hairline `#27272A` are colour literals (`:1324`, `:1325`). Logo in `theme::BG_BRAND` (`:1334`), wordmark "design" strong + ":os studio" regular, 11 px (`:1328-1337`). It is not drawn in `--screenshot` mode (`:1320`, `self.shot.is_none()`), so no CLI capture of it exists.
- **Tab bar** (`tabs.rs:895-1212`). Height `TOP_TAB_BAR_HEIGHT` 32 (`tabs.rs:19`). File-local colour aliases (`:22-29`): bar `BG_PRIMARY`, border `BORDER`, active tab `BG_SECONDARY` + `BORDER_STRONG`, hover `BG_HOVER`, and three literal badge colours `#0D99FF` (design), `#F59E0B` (board, the constant is misnamed `BADGE_BOARD_PURPLE`), `#FF6A3D` (deck/spec) used by `badge_color` (`:48-54`). Home button 72×24 with literal radius `cr(6)` (`:925-939`); divider (`:947`); project tab 26 tall, 112–240 wide (`:954-964`); close box 18 (`:971`); active tab top radius `RADIUS_CONTROL`/`RADIUS_SEGMENT` (`:985-998`); kind badge 14×14, Dev badge `#1BC47D` with icon `#0D1F16` (`:1001-1010`), other badges a 38-alpha tint of the badge colour (`:1008`); unsaved dot in `#0D99FF` (`:1028`); close hover fill `#E11D48` with a white glyph (`:1032-1044`); `+` button 24 (`:1064-1078`); Update button with a `#1BC47D` dot (`:1082-1113`), hidden while `UPDATER_UI_ENABLED` is false; the two tab menus use `widgets::menu::popup_menu` (`:1116-1179`).
- **Left rail** (`panels/rail.rs:36-212`). Width `RAIL_WIDTH` 56 (`theme.rs:263`), cells 56, tile 32, icon 20, labels in `font_caption*` 10 px `TEXT` (`rail.rs:88-89`), selected tile `BG_SELECTED`, hover `BG_HOVER` (`:76-80`), separators `BORDER` (`:200-203`), main-menu button (`:43-55`), Agent Terminal button (`:163-175`), help button with a literal `#0D99FF` dot (`:177-185`).
- **Bottom toolbar** (`panels/toolbar.rs:109-444`). Bar `TOOLBELT_HEIGHT` 48, radius `RADIUS_FRAME` 14, fill `BG_TOOLBAR`, inner ring `INNER_HIGHLIGHT`, two drop shadows (`:170-173`). Tool buttons 32 (`TOOL_BUTTON`), selected `BG_TOOLBAR_SELECTED` with a white icon (`:191-201`), chevrons 16 wide (`:208-224`). Actions button fills literal `#0D99FF` when open (`:114`, `:233-238`). Mode switcher: 3 segments (`:249-310`), Dev segment fills literal `#1BC47D` (`:272-276`), the selected Draw/Design icon is `#0D99FF` (`:283`). Contextual floating bar 128×34 with 28 px buttons, radius `cr(8)`, border `#383838` (`:121-168`). Dev-mode toolbar (`:314-444`): Re-center pill 118×28 in `#2C2C2C`/`#363636`/`#383838` (`:333-352`), tool icons white when active (`:395`), Dev toggle `#1BC47D`, hover `#22D489`, white icon (`:435-439`).
- **Shell menus** (`panels/menus.rs`). Main menu tree (`:417-558`), tool-group menus (`:560-569`), the CLI preset (`:577-609`), file menu (`:611-651`), zoom menu (`:653-769`), dispatcher (`:776-842`). All of them are drawn by `widgets::menu::popup_tree`/`popup_menu` (spec C), so they carry no colours of their own. Rows are `MENU_ITEM_HEIGHT` 24 (`theme.rs:419`). The menu widget clamps a level taller than the screen and scrolls it (`widgets/menu.rs:174`, test `:661`). The "Actions…" command palette is drawn here (`menus.rs:879-1057`): search row 38, tab row 34, section row 22, 8 rows of 24, width 520 (`:879-883`), with literals `#2C2C2C` surface, `#383838` border and active tab (`:934-936`) and a white active-tab label (`:982`).
- **Rulers** (`canvas/rulers.rs:373-498`). 20 px strips (`RULER_SIZE`, `theme.rs:525`) filled `BG` with a `BORDER` hairline (`:427-432`); ticks `RULER_TICK` white 40 % and labels `RULER_TEXT` white 70 % at the 9 px canvas size (`theme.rs:526-529`, `rulers.rs:436-462`); selection band `RULER_SELECTION_FILL` (`#0D99FF` 30 %) with `CANVAS_SELECTION` labels (`:474-493`); guides `RULER_GUIDE` `#F24822` and a guide badge with white text (`:405-422`).
- **Other shell paint in `shell.rs`.** Window background `theme::BG` (`:1382`); dock and left-panel resize handles highlight in `BG_BRAND` 2 px (`:1457-1464`, `:1491-1498`); collapsed-sidebar button (`:1472-1477`); toast (`:989-1016`, spec J's surface, see section 5); `clear_color` returns the float literal `[0.17, 0.17, 0.17, 1.0]` (`:1180-1182`), which the colour regex below does not see; the recording HUD for `--panel micro-*:seq` (`:3403-3490`, literals at `:3406`, `:3433`, `:3451`, `:3453`, `:3466`, `:3467`, `:3473`, `:3479`).

Measured colour census (non-test code: comments and strings blanked and `#[cfg(test)]` items dropped by `scripts/rust_literal_scan.clean`). A *chrome site* is a `Color32` constructor with literal arguments, a `from_gray`/`from_black_alpha`/`from_white_alpha` call, or a named `Color32` colour other than `TRANSPARENT`. A *computed site* builds a colour from variables (a document colour or a derived tint).

| File | `theme::` lines | Chrome sites | Site lines | Computed sites |
|---|---|---|---|---|
| `tabs.rs` | 26 | 8 | 27, 28, 29, 1003, 1006, 1034, 1037, 1090 | 1008 (badge tint) |
| `shell.rs` | 55 | 12 | 1324, 1325, 3406 (×2), 3433 (×2), 3451, 3453, 3466, 3467 (×2), 3473, 3479 | none |
| `panels/rail.rs` | 28 | 1 | 185 | none |
| `panels/toolbar.rs` | 99 | 13 | 114, 135, 201, 238, 272, 281, 315, 337 (×2), 338, 395, 438, 439 | none |
| `panels/menus.rs` | 43 | 4 | 934, 935, 936, 982 | none |
| `canvas/rulers.rs` | 24 | 1 | 422 | none |
| **Total** | 275 | **39** | | 1 |

Command (run from the repo root; it writes nothing, `PYTHONDONTWRITEBYTECODE=1` keeps `scripts/` clean):

```bash
PYTHONDONTWRITEBYTECODE=1 python3 - tabs.rs shell.rs panels/rail.rs panels/toolbar.rs panels/menus.rs canvas/rulers.rs <<'PY'
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

The plan's own method (`grep -cE 'Color32::from_rgba?(_unmultiplied|_premultiplied)?\(|Color32::from_gray\(' <file>`, lines, tests included) gives `tabs.rs` 8, `shell.rs` 8, `panels/toolbar.rs` 7, `panels/menus.rs` 3, `panels/rail.rs` 1, `canvas/rulers.rs` 0. It misses named colours (`Color32::WHITE`) and counts computed constructors, so this spec uses the census above.

Gates today: no gate counts colour literals. `scripts/check_spacing_relationships.py` ratchets geometry and font-size literals against `scripts/spacing_baseline.json`; `scripts/design_os_self_evolve.py` G01 compares 15 `theme.rs` colour constants with `design/design.tokens.json`; `scripts/zero_fake_ui_gate.py` checks behaviour (Z08 the contextual bar buttons, Z13 the zoom menu rows, Z14 the hidden Update button). Theme tests (`theme.rs:770-913`) pin the control family (24/28/32/40), `RIGHT_TABS_H`, the radius chain and `selection_fills_are_neutral`.

## 3. Behaviour rules

1. **Colours come from roles only.** After this change the 39 chrome sites in section 2 are gone from the owned files. Each one becomes a `theme::` constant from spec A. The one computed site (`tabs.rs:1008`) derives a tint from a role; it stays computed but takes its alpha from a theme constant (A names it; "new name" if A has none).
2. **Role map** (provisional names until A lands; A's token contract wins):

| Element | State | Role (Untitled dark) | Today |
|---|---|---|---|
| Title bar fill, tab bar fill, toolbar fill, palette surface | default | `bg-secondary` `#171717` (A's role for today's `BG_PRIMARY`/`BG_TOOLBAR`) | `#18181B` literal, `BG_PRIMARY`, `BG_TOOLBAR`, `#2C2C2C` literal |
| Rail fill, ruler strips, window background | default | `bg-primary` `#0A0A0A` (A's role for today's `BG`) | `BG` `#121215` |
| Hairlines: title bar bottom, tab bar bottom, tab dividers, rail separators, toolbar divider, ruler edges | default | `border-secondary` `#262626` | `#27272A` literal, `BORDER` |
| Control outlines: active tab, contextual bar, Re-center pill, palette frame | default | `border-primary` `#404040` | `BORDER_STRONG`, `#383838` literals |
| Tool button, rail tile, tab, `+`, Home, palette tab | hover | C state `Hover` (provisional `bg-secondary_hover` `#262626`) | `BG_HOVER`, `BG_TOOLBAR_HOVER`, `#363636` |
| Tool button, rail tile, active tab, mode segment, palette tab, Actions button while open | selected | C state `Selected`, a neutral fill (P2); icon `fg-primary` `#FFFFFF` | `BG_SELECTED`, `#0D99FF` fill (Actions), `#383838` |
| Icons at rest / hover | default, hover | `fg-quaternary` `#737373` → `fg-quaternary_hover` `#A3A3A3`; primary icons `fg-secondary` `#D4D4D4` | `ICON`, `ICON_SECONDARY` |
| Labels: tab title, rail label, menu row | default, hover/selected | `text-tertiary` `#A3A3A3` → `text-primary` `#FAFAFA` | `TEXT_SECONDARY`, `TEXT` |
| Brand logo in the title bar | default | `bg-brand-solid` `#E36323` (R4) | `BG_BRAND` |
| Resize-handle highlight (dock, left panel) | hover, drag | `bg-brand-solid` `#E36323` (R4) | `BG_BRAND` |
| Tab close button | hover | C `Buttons/Button · Hierarchy=Tertiary` destructive hover: `bg-error-solid` `#DC2626` with `fg-white` `#FFFFFF` (4.83:1) | `#E11D48` + white |
| Unsaved-changes dot, rail help dot | default | `fg-brand-secondary` per R4 (provisional `#F97316`, A decides) | `#0D99FF` literal |
| File-kind badge (design, board, deck) | default | Untitled utility colours, 600 step: design `utility-blue-600` `#60A5FA`, board `utility-amber-600` `#FBBF24`, deck `utility-orange-600` `#FB923C`; tint = same colour at A's tint alpha | `#0D99FF`, `#F59E0B`, `#FF6A3D` literals |
| Dev-mode accent (tab Dev badge, Dev mode segment, Dev toggle) | default, hover | the Dev role decided in spec G (controller default `DEV_READY` `#14AE5C`); label/icon on it `on-brand-label` `#0A0A0A` (new name for the R4 dark label; `Neutral-950` in spec C's draft; 6.82:1) | `#1BC47D`, `#22D489`, white icon (2.27:1) |
| Focus ring (every focusable control in this spec) | focused | `focus-ring` `#F97316` (R4), drawn per C | none on most shell controls |
| Ruler ticks / labels / strip | default | ticks `fg-quaternary` `#737373`; labels `text-tertiary` `#A3A3A3` (7.85:1 on `#0A0A0A`) | `RULER_TICK` white 40 %, `RULER_TEXT` white 70 % |
| Ruler selection band and its labels, ruler guides and guide badge | default | Studio canvas exceptions (controller default): selection `#0D99FF`, guides `#F24822`; badge label `fg-white` | unchanged values, badge text literal white (`rulers.rs:422`) |

3. **Widgets (spec C).** Tab-bar Home, `+`, close and Update: C's icon button at the size section 3 rule 5 sets. Project tab: C's `Horizontal tabs` item (file-tab variant; C name TBD) with the kind badge as its leading icon. Rail main-menu, terminal and help buttons, every tool button, the Actions button and the Dev toggle: `Buttons/Button · Size=xs · Hierarchy=Tertiary` icon-only (32). Mode switcher: C's segmented control (`Button group` in the kit; C name TBD), 3 segments. Contextual bar and Re-center pill: `Buttons/Button · Size=xs · Hierarchy=Secondary`. Every hover tip: `Tooltip`. Main, file, tool-group, zoom and tab menus: `Dropdown menu` (C), unchanged API. Palette: C's floating surface + `Input field · Size=sm` search + segmented tabs + `Dropdown menu` rows. The floating toolbar: C's floating surface (radius from B; today `RADIUS_FRAME` 14).
4. **States.** Every interactive element in this spec shows C's Default, Hover, Pressed, Focused (keyboard), Disabled and Selected states where the control has them today. Disabled menu rows keep today's dimming but take C's Disabled colours. No new state is invented; no state is removed.
5. **Sizes (R5) and their layout effect.** Regions, widths and order do not change (R1). Heights:

| Element | Constant / code | Today | Target | Δ | Layout effect |
|---|---|---|---|---|---|
| Title bar | `shell.rs:71` | 28 | 28 | 0 | none |
| Wordmark type | `shell.rs:1328-1329` | 11 strong/regular | `text-sm` 14/20 (decision 1) | 0 | brand box ×14/11 wider; nothing else sits in the bar |
| Tab bar | `tabs.rs:19` | 32 | 32 (Open question 1) | 0 | none |
| Project tab | `tabs.rs:964` | 26 | 26 (Open question 1) | 0 | none |
| Home, `+` | `tabs.rs:925`, `:1064` | 24 | 24 (Open question 1) | 0 | "Home" at 12 px still fits the 72 px box (label starts at x+26) |
| Tab, rail, menu, palette labels | `font_body`/`font_caption`/`font_menu` | 10–11 | `text-xs` 12/18 | 0 | tab titles elide about 8 % earlier (fixed max width, `tabs.rs:1019`) |
| Rail width, cells, tile | `theme.rs:263`, `:268`, `:275` | 56 / 56 / 32 | unchanged, or the #1106 width (Open question 2) | 0 | none |
| Rail labels | `rail.rs:88-89` | 10 px | `text-xs` 12/18, or none after #1106 | 0 | "Variables" grows from about 44 px (measured on `D-01`) to about 53 px in a 55 px cell (INFERENCE, scaled) |
| Toolbar, tool buttons | `theme.rs:398`, `:401` | 48 / 32 | 48 / button `xs` 32 | 0 | none |
| Mode segments | `toolbar.rs:254-257` | 28 tall | C segment height (C draft: 28) | 0 | none |
| Contextual bar buttons | `toolbar.rs:129-157` | 28 (bar 34) | button `xs` 32 (bar 40) | +6 | the bar floats 6 px higher above the toolbar; width 128 → about 140 |
| Re-center pill | `toolbar.rs:333` | 28 | button `xs` 32 | +4 | the pill floats 4 px higher |
| Menu rows (main, file, tool, zoom, tab menus) | `theme.rs:419` via C | 24 | C's menu row (C draft: 32) | +8 per row | main menu 15 rows + 5 separators: 461 → 581 px (bottom at y 625 in a 1088 window); File submenu 12 rows: 338 → 434 px; at `MIN_WINDOW` 900×600 the menu clamps and scrolls (existing) |
| Palette rows | `menus.rs:879-883` | 8 × 24, search 38, tabs 34 | 8 × C menu row, search `Input field · Size=sm`, tabs C segment | +64 (rows) | palette 296 → about 360 px, still anchored above the toolbar |
| Rulers | `theme.rs:525-526` | 20 px, 9 px labels | unchanged (Open question 3) | 0 | none |

6. **Radii.** Literal radii in the owned files (`tabs.rs` `cr(6)`, `cr(3)`, `cr(4)`; `toolbar.rs` `cr(8)`, `cr(6)`; `menus.rs` `CornerRadius::same(6)`) take B's roles (controls `radius-md` 8 per R5, marks and badges B's small role). A nested shape stays concentric (P4).
7. **No behaviour change.** Hit areas, shortcuts, tooltips' text, menu contents, widget ids recorded through `widgets::record` and the order of controls stay as they are. Z08, Z13 and Z14 stay green.

## 4. Data model / schema changes

None. No document, preference or file-format change. Only paint calls, constants consumed from `theme.rs` and C's widget calls change.

## 5. Contracts

**File ownership across D–K.** Each file has exactly one owner. D owns:

| File | Owner | Shared surface and rule |
|---|---|---|
| `crates/designos-app/src/tabs.rs` | D | none |
| `crates/designos-app/src/shell.rs` | D | the toast (`:989-1016`) is J's surface: D applies J's toast rules in this file; the recording HUD (`:3403-3490`) is demo-only and is restyled to roles like the rest |
| `crates/designos-app/src/panels/rail.rs` | D | #1106 edits it first (section 12) |
| `crates/designos-app/src/panels/toolbar.rs` | D | the Dev-mode toolbar (`:314-444`) takes G's Dev role |
| `crates/designos-app/src/panels/menus.rs` | D | the command palette (`:879-1057`) is listed under J's scope ("command palette"); D restyles it here with C widgets and J lists this file as "not mine" |
| `crates/designos-app/src/canvas/rulers.rs` | D | none |

Not mine (listed so no file is owned twice): `theme.rs`, `design/*` (A, B); `widgets/menu.rs`, `widgets/mod.rs`, `widgets/field.rs`, `widgets/popover.rs`, `panels/mod.rs` (scrollbar), and at `origin/main` `widgets/field_tab.rs`, `widgets/popup_open.rs` (C); `panels/left.rs`, `panels/layers.rs`, `find/filter.rs` (E); `panels/right.rs` and the inspector files (F); `dev/*`, `specs.rs` (G); `home/*`, `updater.rs` (H); `terminal.rs`, `agent_*.rs` (I); `cmdk.rs`, `panels/context.rs`, `find/panel.rs`, `comments/*`, `history/*` (J); `panels/assets.rs`, `variables/*`, `libraries/*` (K). Unassigned in D–K (flag to the controller): `canvas/draw.rs` (11 literal lines), `canvas/grids.rs`, `canvas/tiles.rs`, `canvas/interact.rs`, `canvas/agent_*.rs`, `board/*`, `decks/*`, `present.rs`, `icons.rs` (R6 restyle), `lint.rs`, `image_jobs.rs`, `plugins/ui.rs`, `panels/check_designs.rs`.

**Colour-literal gate (new name).** This spec needs a gate that fails when a chrome site exists in an owned file. Proposal: spec A adds `scripts/check_chrome_colour_literals.py` (new name) with the regex and test-stripping of section 2, a per-file baseline that only falls (like `spacing_baseline.json`), and an allowlist of computed sites and canvas-exception lines. D's acceptance uses that gate; until it lands, the section 2 command is the check and must print `chrome_sites=0` for every owned file except the allowlisted lines in rule 3 of section 8.

## 6. File-format mapping

None.

## 7. UI

- **Implementer setup.** UI implementers load the `es-designer` skill first and follow the project's own design system: `design/principles.md`, `design/soul.md`, spec A's token contract, spec B's scale and spec C's widget kit win over any generic default. Purple Ban stays (R4); the kit's purple brand (`#7F56D9`, `#9E77ED`) is never used.
- **Regions.** Title bar 28, tab bar 32, rail 56 (or the #1106 width), floating toolbar centred 12 px above the canvas bottom, rulers 20. Same positions as `D-01` (section 10).
- **Copy.** No copy changes. UX-writing gate Z16 must stay green.
- **Evidence.** Before and after captures go into the PR body (not Figma, R3). Responsive: SKIPPED (owner default). The title bar has no CLI capture (section 2); the PR shows it with a macOS window capture of a manual run (`screencapture -l <window-id>`), labelled as manual.

## 8. Edge cases

1. Text grows from 11 to 12 px in fixed boxes: tab titles (`tabs.rs:1019-1022`) and menu labels elide earlier. Elision must stay on a character boundary with the ellipsis, never clip.
2. Rail labels at 12 px nearly fill the 55 px cell ("Variables", about 53 px, INFERENCE). If #1106 has not landed, labels must not overlap the cell edges; measure the widest label in the PR.
3. Allowlisted sites that stay literal: `Color32::TRANSPARENT` sentinels (`toolbar.rs:196`, `:198`, `:390`, `:392`, not counted); the ruler guide badge text stays white over `#F24822` (canvas exception) but moves to a theme constant, so it leaves the count.
4. Dev mode on: tab badge, mode segment and toolbar switch to G's Dev role together; the label on the Dev fill is dark (`#0A0A0A`), not white.
5. Actions open: the button shows C's Selected state, not blue. The palette and `cmdk` both set "open" (`toolbar.rs:232`); both must show the same state.
6. Menus taller than the window (main menu at `MIN_WINDOW` 900×600 after rows grow to 32): the existing clamp and scroll apply; no row is unreachable.
7. `clear_color` (`shell.rs:1180-1182`) shows for one frame before the first paint and behind transparent regions: it takes the window background role (`bg-primary`), converted to linear floats.
8. Board, deck and presentation modes reuse the tab bar and rail; their own toolbars (`board/toolbar.rs`, `decks/ui.rs`) are not in this spec (unassigned, section 5).

## 9. Non-goals

- Layout changes: no region moves, no width changes, no reordering (R1). The icon-only rail is #1106's change, not this spec's.
- New icons or icon art (R6: keep the set; stroke and colour follow the fg roles above). Icon licence is spec Q.
- Canvas overlay colours (selection, component cyan, guides, measurement) and document content (`DEFAULT_PAGE_BG`, default fills).
- The toast, context menu, `cmdk.rs`, Find and Comments (J); Home (H); the agent terminal (I).
- Making the title bar capturable from the CLI (a dogfood follow-up for spec L or O).

## 10. Test plan (red first)

Acceptance criteria:

1. The section 2 census (or A's `check_chrome_colour_literals.py` once it exists) prints `chrome_sites=0` for `tabs.rs`, `shell.rs`, `panels/rail.rs`, `panels/toolbar.rs`, `panels/menus.rs` and `canvas/rulers.rs`, with only the section 8 allowlist left.
2. `python3 scripts/check_spacing_relationships.py` passes and every owned file's radius and font-size counts are at or below `scripts/spacing_baseline.json`.
3. Spec L's layout gate reports the same regions, widths and control order as the baseline for every capture below; any height change matches the section 3 table.
4. Every role in section 3 traces to an A constant; `cargo test -p designos-app --bin designos theme::` passes (B's updated invariants).
5. `python3 scripts/zero_fake_ui_gate.py` passes (Z08, Z13, Z14 untouched) and `python3 scripts/design_os_self_evolve.py` G01 passes.
6. Contrast: every text/icon role on its fill in section 3 is at least 4.5:1 (text) or 3:1 (icons and focus ring), measured with A's a11y check (`ui ds a11y --dir .`) or the hex pairs listed (computed this session: `#A3A3A3` on `#0A0A0A` 7.85, on `#171717` 7.11; `#0A0A0A` on `#E36323` 5.73; `#0A0A0A` on `#14AE5C` 6.82; `#FFFFFF` on `#DC2626` 4.83; `#F97316` on `#0A0A0A` 7.06).
7. Before/after captures of every command below are in the PR, each with its exit code.

| Case | Expected | Fails before |
|---|---|---|
| census of the owned files | `chrome_sites=0` | yes (39 sites) |
| `theme::tests::*` (B's updated invariants) | pass | per B |
| `panels::rail::tests::cells_follow_the_measured_rail_layout` | pass unchanged (or #1106's updated test) | guard |
| `panels::toolbar::tests::*` (`toolbar_is_centered_and_above_bottom`, `toolbelt_is_512_wide_like_figma`, `contextual_floating_bar_buttons_mutate_selection_and_tool_state`) | pass; the width test value changes only if B changes a gap | guard |
| `widgets::menu::tests::a_menu_taller_than_the_screen_is_clamped_to_the_screen_height` | pass with C's row height | guard |
| new `tabs::tests::tab_bar_paints_only_theme_roles` (new name) | paints the bar into a test `Context` and asserts every fill and stroke colour equals one of A's role constants | yes |
| new `panels::toolbar::tests::dev_toggle_label_is_dark_on_the_dev_fill` (new name) | icon colour is `on-brand-label` `#0A0A0A` | yes (white) |

Run: `cargo test -p designos-app --bin designos theme::`, `cargo test -p designos-app --bin designos panels::toolbar`, `cargo test -p designos-app --bin designos panels::rail`, `cargo test -p designos-app --bin designos widgets::menu`, `cargo test -p designos-app --bin designos tabs::`, `python3 scripts/check_spacing_relationships.py`, `python3 scripts/zero_fake_ui_gate.py`, `python3 scripts/design_os_self_evolve.py`, `cargo fmt --all --check`, `cargo clippy -p designos-app --all-targets -- -D warnings`.

**Evidence commands.** The exit codes were recorded through the spec writer's wrapper (`run-shot.sh` in the writer's scratch folder, not in the repo), which ran `cargo run -q --release -p designos-app --bin designos -- <the same arguments>` from the repo root with an isolated `HOME` and absolute scratch paths. Each was run once on HEAD `ec47da26` by the spec writer with an isolated `HOME` and the exit code shown. `KIT` is a copy of `~/Downloads/❖ Untitled UI Figma – PRO VARIABLES (v8.0) gKXp7LqFY2NV (Copy).fig` in a scratch folder (never the original); `OUT` is a scratch folder; each kit run takes about 50 s plus build-lock waits. Run the same list after the change and put both sets in the PR.

```bash
cd "$(git rev-parse --show-toplevel)"   # repo root: cargo run -p needs it
export PATH="$HOME/.cargo/bin:$PATH"; S="$(mktemp -d)"; mkdir -p "$S/home" "$S/lib" "$S/shots"; OUT="$S/shots"
KIT="$S/untitled-ui-kit-copy.fig"; cp "$HOME/Downloads/❖ Untitled UI Figma – PRO VARIABLES (v8.0) gKXp7LqFY2NV (Copy).fig" "$KIT"   # a copy, never the original
RUN() { env HOME="$S/home" TMPDIR="$S/" CARGO_HOME="$HOME/.cargo" RUSTUP_HOME="$HOME/.rustup" cargo run -q --release -p designos-app --bin designos -- "$@"; echo "exit $?"; }
RUN "$KIT" --screenshot "$OUT/D-01-editor-default.png" --size 1600x1088 --no-toast                       # exit 0: tab bar, rail, toolbar, rulers
RUN "$KIT" --screenshot "$OUT/D-07-select-frame.png" --size 1600x1088 --no-toast --select "Welcome to Untitled UI"   # exit 0: selected state, ruler band
RUN "$KIT" --screenshot "$OUT/D-02-menu-main.png" --size 1600x1088 --no-toast --menu main             # exit 0: main menu
RUN "$KIT" --screenshot "$OUT/D-03-menu-main-file.png" --size 1600x1088 --no-toast --menu main/File   # exit 0: File submenu
RUN "$KIT" --screenshot "$OUT/D-04-menu-shape.png" --size 1600x1088 --no-toast --menu shape           # exit 0: tool-group menu
RUN "$KIT" --screenshot "$OUT/D-05-menu-palette.png" --size 1600x1088 --no-toast --menu palette       # exit 0: Actions palette
RUN "$KIT" --screenshot "$OUT/D-06-menu-zoom.png" --size 1600x1088 --no-toast --menu zoom             # exit 1 twice ("designos: screenshot was not captured"): zoom menu (accepted by preset_menu, menus.rs:599-602, not listed in --help)
RUN --home --library "$S/lib" --screenshot "$OUT/D-08-home-tab-bar.png" --size 1600x1088 --no-toast   # exit 0: tab bar with Home active
RUN "$KIT" --screenshot "$OUT/D-09-dev-css.png" --size 1600x1088 --no-toast --dev css --select "Welcome to Untitled UI"  # exit 1 twice ("designos: screenshot was not captured"): Dev toolbar, Dev badge
```

Not capturable from the CLI today: the title bar (hidden in screenshot mode), hover and focus states, the contextual floating bar (hidden while a toast is pending, `toolbar.rs:121-128`), tab close hover. The PR shows these with a manual window capture, labelled manual.

## 11. Performance budgets

- No new allocation in the per-frame paint path: roles are `const Color32`, and the badge tint is computed once per tab as today.
- No `request_repaint` added; idle CPU stays 0 % (CLAUDE.md section 4). No change touches `doc_hash`, `TileView::rev` or the GPU canvas.
- Text at 12 px instead of 11 px adds no measurable cost (same galley calls).

## 12. Dependencies and build order

- Build order: A (roles in `theme.rs`) and B (scale) land first; C's widgets next; L's baseline captures before the first surface PR. Then this spec: role swap per file (tabs, title bar, rail, toolbar, menus and palette, rulers), then R5 type and heights, then evidence.
- **#1106 (icon-only rail, P2, open, assigned by the PM to an Argon worker on 2026-10-25).** It removes the rail labels, adds tooltips and reduces the rail width. That conflicts with R1 ("same widths") unless L's baseline is taken after #1106 merges. Recommended order: #1106 first, L re-baselines the rail, then D restyles (Open question 2). D must not reintroduce labels.
- **#1108 (Pages/Layers separator, P1).** Worktree `agy-1108-separator` exists with no commits yet. It touches `panels/left.rs` (E) and `theme.rs`; no D file. Its brand-highlight handle should use the same `bg-brand-solid` role as the shell's resize handles (rule 2).
- **Spec L edits `shell.rs` first** (region rects `:1383-1398` recorded into the app state, screenshot write `:445-460` also writing `--layout-json`; `specs/L-layout-unchanged-gate-baseline-capture.md`). L lands before D, so D rebases on it; D owns `shell.rs` for the revamp only after L merges.
- **Open PRs touching D files** (`gh pr list --state open`, 2026-10-25): #1081 `fix/issue-1009-cli-export-writers` (`shell.rs`); #770, #768, #765 (stale `task/5xx` branches touching `shell.rs`, `tabs.rs`, `panels/toolbar.rs`, `panels/menus.rs`, `panels/rail.rs`, `theme.rs`, `widgets/menu.rs`). Rebase on whichever merges first; the stale three need a decision from the PM before this spec starts.

## 13. Owner rulings that apply

- R1: adopt Untitled UI dark 100 %, keep layout and arrangement. R2: take Untitled spacing and typography too. R4: Studio orange `#E36323`, dark label `#0A0A0A`, focus ring Orange-500 `#F97316`, Purple Ban stays. R5: small end of the scale in editor chrome (`text-xs` 12/18 rows and labels, `text-sm` 14/20 titles, button `xs` 32, row 32, `radius-md` 8). R6: keep the icon set, restyle colour to fg roles. R3: no Figma; evidence in the PR. R7: spec-driven.
- Controller defaults (2026-10-25): canvas overlays keep Figma-parity colours; chrome status colours move to Untitled.
- 2026-09-22: visual evidence in every PR. Standing: `main` is protected (PR, green macOS test, clippy, rustfmt, squash).
- 2026-10-10: responsive checks only on request ("responsive: SKIPPED (owner default)").

## Decisions recorded by the spec writer (technical; owner may reverse)

1. The title-bar wordmark takes `text-sm` 14/20, like a panel title (R5), not `text-xs`: it names the app and is the only text in its bar.
2. Chrome blue (`#0D99FF`) is chrome, not canvas: the Actions-open fill, the selected-mode icon, the unsaved dot and the rail help dot leave blue. The ruler selection band stays blue because it mirrors the canvas selection.
3. File-kind badges use Untitled utility colours (blue, amber, orange 600 steps); the misnamed `BADGE_BOARD_PURPLE` goes away with its literal.
4. The contextual bar and the Re-center pill grow to button `xs` 32 (R5 names buttons); they float, so no region changes.
5. The command palette stays in `panels/menus.rs` and is owned here, so one file has one owner.

## Open questions for the owner

1. **Tab bar height.** R5 makes buttons `xs` 32 in editor panels, but the tab strip is window chrome and is 32 tall. Recommended: keep the bar at 32 and its tabs at 26 and Home/`+` at 24, styled with C's anatomy (R1 keeps region heights; R5 names editor panels, not the window strip). Alternative: tab-bar buttons at `xs` 32, bar 40 (+8 px; the canvas loses 8 px).
2. **Rail width with #1106.** #1106 (owner request 2026-10-25 10:46, icon-only rail) reduces the rail width, R1 says widths stay. Recommended: #1106 merges first and its width is the new baseline for L and this spec; the revamp then styles an icon-only rail. Alternative: this spec keeps 56 px and #1106 rebases on it.
3. **Ruler label size.** Untitled's smallest type is `text-xs` 12; rulers use the declared 9 px canvas size (P5). Recommended: keep 9 px (canvas overlay exception; 12 px labels collide on dense ticks in a 20 px strip) and take only the Untitled colours. Alternative: 12 px labels in a 24 px strip (rulers grow 4 px on both axes).
