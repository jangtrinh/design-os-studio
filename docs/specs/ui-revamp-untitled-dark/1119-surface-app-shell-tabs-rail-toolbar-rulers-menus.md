# D App shell: title bar, tab bar, left rail, bottom toolbar, rulers and the shell menus in Untitled UI dark

| Field | Value |
|---|---|
| Issue | #1119 (epic #1118, spec key D) |
| Size | M. About three days: 7 owned files, 39 chrome colour sites, C's widgets on four bars, the toast (J's rules, two actions), four hit areas raised to the 24 px floor (tab close, tool chevrons, the left-panel resize strip, the ruler strips), the R5 type and row sizes. No new behaviour. |
| Depends on | A (colour constants, `specs/token-contract.md`, contrast check C-1, colour-literal gate), B (type, spacing, radius and control-size scale), C (widget anatomy and states, `specs/C-widget-kit-untitled-anatomy.md`), L (layout-unchanged gate and baselines), G (the Dev accent role; its value `DEV_READY` `#14AE5C` is R10.4), J (toast rules 12–15, command bar rule 2). #1106 (icon-only rail) is merged and is the baseline. |
| Unblocks | M (docs and screenshot refresh). J §1 records that the palette inside `panels/menus.rs` is restyled here and that J does not edit that file. |
| Touches | `crates/designos-app/src/tabs.rs` (`:21-31`, `:50-56`, `:936-1265`); `crates/designos-app/src/shell.rs` (`:86`, `:1087-1160`, `:1327-1329`, `:1536-1562`, `:1599`, `:1667-1693`, `:3591-3675`); `crates/designos-app/src/panels/rail.rs` (`:36-193`); `crates/designos-app/src/panels/toolbar.rs` (`:131-470`); `crates/designos-app/src/panels/menus.rs` (`:430-870`, `:907-1085`); `crates/designos-app/src/canvas/rulers.rs` (`:99-106`, `:228-262`, `:373-498`); `crates/designos-app/src/shell/toast_layout.rs` (the toast's layout; unowned before this round: D paints the toast, so D owns the file that sizes it). Reads only (A and B own them): the shell constants in `crates/designos-app/src/theme.rs` (`:22-47`, `:260-274`, `:403-449`, `:532-549`). |

Code baseline: `origin/main` `8dfaee93` (v0.7.0 merged, 2026-10-28; read with `git show 8dfaee93:<path>`, no checkout). The spec was written against `205c72ef`; every `file.rs:line` cite in sections 2 to 12 was moved to `8dfaee93` from the `git diff -U0` hunks between the two commits (77 of the 232 cites in this file moved) and each moved cite was checked to point at the same source line at both refs. Cites in the changelog at the end are history and keep the ref they were written at. The census in section 2 was re-run at `8dfaee93` (section 10).

Drift to `8dfaee93` (measured 2026-10-28; 217 app files changed since `649da124`, `origin/main` is now v0.7.0). D's own files: `tabs.rs`, `panels/rail.rs`, `panels/toolbar.rs` and `canvas/rulers.rs` did not change. `shell.rs` grew 252 lines, 135 of them before the title bar (`Shell::build`: `--select` through `select_target`, `SELECT_MISS`, `SHOT_OPEN_ERROR`, fixtures behind `cfg(feature = "fixtures")`), so every cite after `:102` moved by +9 to +140; the title bar, rail call, show-sidebar button, resize handle, `clear_color` and the HUD are the same code. `panels/menus.rs` +6: the Help and account submenu gains "Open starter sample" (`Action::OpenStarterSample`, `:545`), so that submenu now has 6 rows and 3 separators; the main menu (15 rows, 5 separators) and the File menu (12 rows, 2) that rule 5 prices are unchanged, and the palette cites moved by +6. `theme.rs` +8 after `:290` (#1108, as before). `canvas/draw.rs` +163 (the two cites into it moved). **Rules the drift touches.** (1) The toast: `Shell::toast` now arms its lifetime on the first paint (`armed`, `duration`, `:1091-1114`) and a no-file-dialog toast carries a second action, "Enter a path…" (`toast-enter-path`, `:1121`, #1192), beside "Show in folder" (`toast-reveal`, `:1119`). Rules 2 and 3 and the toast tests therefore cover both actions; `shell/toast_layout.rs` did not change. (2) CLI: `--select` takes `id:`, `fig:`, `name:` or a layer name and exits 2 on a miss, and a `--screenshot` run whose file cannot open exits non-zero without a PNG, so the exit codes recorded in section 10 (0.6.x runs) are history: the implementer re-runs the evidence list on v0.7.0 (`tools/designos-latest`). (3) Census at `8dfaee93`: the same 39 sites; `shell.rs` `theme::` lines 54 → 55 and its site lines moved +135 to +140. No surface of D was added or removed, and no rule of this spec is broken by the drift. Re-anchor against the HEAD you branch from and report drift before writing code.

Naming follows the plan's contract: a colour is named by the token-contract Rust constant, its Untitled token name and the dark hex. A name the contract does not have is marked "new name" and is a request to A. Widgets are named by spec C's anatomy names.

## 1. Summary and user story

As a designer working in Studio, I see the window chrome (title bar, file tabs, left rail, floating toolbar, rulers, the shell menus, the Actions palette and toasts) in Untitled UI dark: the same neutral greys, hairlines, type scale, radii, states and focus rings as the rest of the revamped app, with Studio orange as the only brand colour. Nothing moves: every region, width and control order stays where it is today (R1). Heights change only where R5 raises a control or a type size; section 3 rule 5 lists each one.

## 2. Current state (FACT unless labelled)

Inventory of the surface (lines at `8dfaee93`):

- **Title bar** (`shell.rs:1536-1562`). Height `TOP_TITLE_BAR_HEIGHT` 28 (`shell.rs:86`). Fill `#18181B` and bottom hairline `#27272A` are literals (`:1541`, `:1542`). Logo in `theme::BG_BRAND` (`:1551`); wordmark "design" in `font_body_strong` `TEXT` and ":os studio" in `font_body` `TEXT_SECONDARY`, 11 px (`:1543-1554`). It is not drawn in `--screenshot` mode (`:1537`, `self.shot.is_none()`), so no CLI capture of it exists.
- **Tab bar** (`tabs.rs:936-1265`). Height `TOP_TAB_BAR_HEIGHT` 32 (`tabs.rs:21`); fill `TAB_BAR_BG` = `BG_PRIMARY` and bottom hairline (`:952-953`). File-local aliases (`:24-31`): active tab `BG_SECONDARY` + `BORDER_STRONG`, hover `BG_HOVER`, and three literal badge colours `#0D99FF` (design), `#F59E0B` (board, misnamed `BADGE_BOARD_PURPLE`), `#FF6A3D` (deck/spec) used by `badge_color` (`:50-56`). Home button 72×24 with literal radius `cr(6)` (`:966-980`); divider in `BORDER_STRONG` (`:988`); project tab 26 tall, 112–240 wide (`:1007`); close box 18×18 at `right − 13` (`:1014`), excluded from the tab's hit area (`:1018-1019`); active tab: top radii `RADIUS_CONTROL`, bottom `RADIUS_SEGMENT`, `BORDER_STRONG` outline (`:1028-1035`); hover fill (`:1037`); separator between inactive tabs (`:1040`); kind badge 14×14 (`:1044`): Dev badge `#1BC47D` with glyph `#0D1F16` (`:1046-1049`), other badges a 38-alpha tint of the badge colour (`:1051`, computed); pinned lock icon `TEXT_SECONDARY` (`:1057`); title strong when active (`:1063-1065`); unsaved dot `#0D99FF` (`:1072`); close hover fill `#E11D48` `cr(4)` with a white glyph, glyph 10 px (`:1075-1088`); `+` 24×24 (`:1108-1122`); Update button 60×24 with a `#1BC47D` dot (`:1126-1157`, dot `:1134`), drawn only when `UPDATER_UI_ENABLED` is true (it is false, Z14); the two tab menus use `widgets::menu::popup_menu` (`:1160-1223`).
- **Left rail** (`panels/rail.rs:36-193`, icon-only since #1106). Fill `BG`, right edge `BORDER` (`:38-39`). Width `RAIL_WIDTH` 48 (`theme.rs:263`); main-menu button 32 at inset 8 (`RAIL_MENU_BUTTON_SIZE`, `:267`) painted by `widgets::button_bg` with an 18 px icon `ICON_SECONDARY` → `ICON` (`rail.rs:43-55`); item cells 47×56 (`RAIL_ITEM_SIZE`, `theme.rs:268`) with a 32 px tile (`RAIL_TILE`, `:274`), selected `BG_SELECTED`, hover `BG_HOVER` (`rail.rs:77-81`), icons 20 px (`RAIL_ICON_SIZE`, `theme.rs:271`) all in `ICON` (`rail.rs:82-87`); tooltips on hover and on keyboard focus through egui's stock tooltip (`rail_tooltip`, `:166-176`); separators `BORDER` (`:191-193`); help button "Updates & Shortcuts" 32 (`:146-162`) with an unconditional `#0D99FF` dot (`:163`). No labels.
- **Bottom toolbar** (`panels/toolbar.rs:131-470`). Bar `TOOLBELT_HEIGHT` 48 (`theme.rs:403`), radius `RADIUS_FRAME` 14, fill `BG_TOOLBAR`, inner ring `INNER_HIGHLIGHT`, two drop shadows (`toolbar.rs:192-195`). Tool buttons 32 (`TOOL_BUTTON`, `theme.rs:406`): selected `BG_TOOLBAR_SELECTED`, hover `BG_TOOLBAR_HOVER` (`toolbar.rs:213-221`), icon `Color32::WHITE` when selected, else `ICON` (`:223`). Chevrons 16×32 (`TOOL_CHEVRON_W`, `theme.rs:407`; `toolbar.rs:230-253`), glyph `ICON_SECONDARY` 11 px (`:243`); every chevron is followed by `TOOL_GROUP_GAP` 8 (`theme.rs:413`): before the next group (`toolbar.rs:202-203`) or before Actions (`:258`). Actions button fills literal `#0D99FF` while open (`:136`, `:262-268`). Mode switcher: track `BG_TOOLBAR_MODE_SWITCHER`, 3 segments 28 tall (`:279-337`); Dev segment fills literal `#1BC47D` (`:302-306`) with a white icon (`:311`); the selected Draw/Design icon is `#0D99FF` (`:313`). Contextual floating bar 128×34 (`:151-152`), shadow (`:155`), fill `BG_TOOLBAR`, border `#383838` `cr(8)` (`:156-157`), two 28 px icon buttons (`:164`) and a 48×28 "More" button (`:179-189`). Dev-mode toolbar (`:341-470`): Re-center pill 118×28 (`:360`) with shadow (`:363`), fills `#2C2C2C`/`#363636`, border `#383838`, `cr(6)` (`:364-365`), label `font_caption_strong` (`:366-379`); tool icons white when active (`:422`); Dev toggle 36×32 in `#1BC47D`, hover `#22D489`, white icon (`:462-466`).
- **Shell menus** (`panels/menus.rs`). Main menu tree (`:430-573`), tool-group menus (`:584-593`), CLI menu presets (`:601-636`; the zoom arm is `:629-632`), file menu (`:638-678`), zoom menu (`:680-796`), dispatcher (`:803-870`). They are painted by `widgets::menu::popup_tree`/`popup_menu` (spec C's file) and carry no colours here. Rows `MENU_ITEM_HEIGHT` 24 (`theme.rs:424`), panel padding `MENU_PADDING` 8 (`:426`), pill inset `MENU_PILL_INSET` = `INSET_FRAME` 8 (`:428`), separator `SEP_H` = 2 × `MENU_SEP_MARGIN` 8 + 1 = 17 (`widgets/menu.rs:75`). A level taller than the screen is clamped and scrolls (`widgets/menu.rs:209`, test `:764`). The Actions palette is drawn here (`menus.rs:907-1085`): search row 38, tab row 34, section row 22, 8 rows of `MENU_ITEM_HEIGHT`, width 520 (`:907-911`), height formula `:953`, literals `#2C2C2C` surface, `#383838` border and active-tab fill (`:962-964`), white active-tab label (`:1010`).
- **Rulers** (`canvas/rulers.rs:373-498`). 20 px strips (`RULER_SIZE`, `theme.rs:532`) filled `BG` with a `BORDER` hairline (`rulers.rs:424-429`); ticks `RULER_TICK` and labels `RULER_TEXT` in the 9 px proportional `font_ruler()` (`theme.rs:535-536`, `:546-548`; `rulers.rs:436-462`); selection band `RULER_SELECTION_FILL` with `CANVAS_SELECTION` labels (`:474-493`); guides `RULER_GUIDE` `#F24822` (`:405-408`) and a value badge filled `RULER_GUIDE` with a literal white label in `font_caption_strong` (`:419-422`). The strips are also the guide-drag targets: `strips()` (`:99-106`) is both the paint rect and the press test of `press_with_alt` (`:228-262`), 20 px thick; a press on the 20×20 corner starts nothing (it only takes the right-click that clears guides, `:264-286`); an existing guide is grabbed within 4 px of its line outside the strips (`hit_guide`, `:287`).
- **Other shell paint in `shell.rs`.** Window background `theme::BG` (`:1599`). Collapsed-sidebar button: `PANEL_ACTION_BUTTON` 24 icon button on a `BG` tile (`:1667-1672`). Left-panel resize handle: a 6 px drag strip (`left_rect.right() ± 3`, the height of the panel, `:1686-1687`) registered after `left::show` so it is on top of the panel's widgets (`:1689`); the visible line is the panel's 1 px `BORDER` hairline, and a 2 px `BG_BRAND` line shows on hover and drag (`:1692`). Toast (`:1087-1160`, laid out by `shell/toast_layout.rs`): `BG_MENU` fill on `RADIUS_FRAME`, `elevation_shadow` (`:1134`), dot `DANGER` or `BG_BRAND` (`:1136`), one action in `TEXT_BRAND` with a `BG_HOVER` hover (`:1139-1145`): "Show in folder" (`toast-reveal`, `:1119`) or, since 0.7.0, "Enter a path…" (`toast-enter-path`, `:1121`); the action's pointer target is `ACTION_HEIGHT` 24 and the message is measured in `font_body` (`toast_layout.rs`, `:1125`). `clear_color` returns the float literal `[0.17, 0.17, 0.17, 1.0]` (`:1327-1329`), which the census regex does not see. The recording HUD for `--panel micro-*:seq` (`fn paint_micro_seq_hud`, `:3591-3675`) holds 10 literals: accent `#E36323`/`#0D99FF` (`:3594`), pointer arrow `#111216` with a white outline (`:3621`), metric pill fill `rgba(17,18,22,242)` (`:3639`) with white text (`:3641`), caption bar fill `rgba(17,18,22,238)` and stroke white α36 (`:3654-3655`), white keycap label (`:3661`), caption `#F4F4F6` (`:3667`); it also draws `elevation_shadow` twice (`:3638`, `:3653`). The micro-demo builders in this file use `designos_doc::Color::rgb` (143 lines match `Color::rgb(` at `8dfaee93`, 144 at `205c72ef`): that is document content, not chrome.

Measured colour census (non-test code: comments and strings blanked and `#[cfg(test)]` items dropped by `scripts/rust_literal_scan.clean`). A *chrome site* is a `Color32` constructor with literal arguments, a `from_gray`/`from_black_alpha`/`from_white_alpha` call, or a named `Color32` colour other than `TRANSPARENT`. A *computed site* builds a colour from variables.

| File | `theme::` lines | Chrome sites | Site lines | Computed sites |
|---|---|---|---|---|
| `tabs.rs` | 26 | 8 | 29, 30, 31, 1046, 1049, 1078, 1081, 1134 | 1051 (badge tint) |
| `shell.rs` | 55 | 12 | 1541, 1542, 3594 (×2), 3621 (×2), 3639, 3641, 3654, 3655, 3661, 3667 | none |
| `panels/rail.rs` | 22 | 1 | 163 | none |
| `panels/toolbar.rs` | 100 | 13 | 136, 157, 223, 268, 302, 311, 342, 364 (×2), 365, 422, 465, 466 | none |
| `panels/menus.rs` | 43 | 4 | 962, 963, 964, 1010 | none |
| `canvas/rulers.rs` | 24 | 1 | 422 | none |
| **Total** | 270 | **39** | | 1 |

Command (run from the repo root, or from the root of a `git archive` of the ref; it writes nothing, and `PYTHONDONTWRITEBYTECODE=1` keeps `scripts/` clean). Its output at `8dfaee93` is pasted in section 10 (exit 0).

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

The plan's own method (`grep -cE 'Color32::from_rgba?(_unmultiplied|_premultiplied)?\(|Color32::from_gray\(' <file>`, tests included) gives at `8dfaee93` (same as at `205c72ef`): `tabs.rs` 8, `shell.rs` 8, `panels/toolbar.rs` 7, `panels/menus.rs` 3, `panels/rail.rs` 1, `canvas/rulers.rs` 0. It misses named colours (`Color32::WHITE`) and counts computed constructors, so this spec uses the census above.

Gates today: no gate counts colour literals (spec A rule 13 adds `scripts/check_chrome_colour_literals.py`, new name, with a falling per-file baseline). `scripts/check_spacing_relationships.py` ratchets geometry and font-size literals against `scripts/spacing_baseline.json`; `scripts/design_os_self_evolve.py` G01 compares `theme.rs` colour constants with `design/design.tokens.json`; `scripts/zero_fake_ui_gate.py` checks behaviour (Z08 the contextual bar buttons, Z13 the zoom menu rows, Z14 the hidden Update button). Theme tests (`theme.rs:778-920`) pin the control family (`:850`), concentric radii (`:858`), `menu_pills_inset_by_the_frame_inset` (`:869`) and `selection_fills_are_neutral` (`:907`).

## 3. Behaviour rules

1. **Colours come from constants only.** After this change the 39 chrome sites in section 2 are gone from the owned files; each becomes a token-contract constant (or a new name A adds). The computed badge tint (`tabs.rs:1051`) goes too: the kind badge becomes an opaque utility tile (rule 2). The HUD's computed accent shades (`accent.gamma_multiply`) stay computed.
2. **Role map.** Surfaces and marks D paints directly (the widget states are in rule 3):

| Element | State | Constant (Untitled role, hex) | Today |
|---|---|---|---|
| Title bar fill (`shell.rs:1541`), tab bar fill (`tabs.rs:952`) | default | `BG_PRIMARY` (bg-primary `#0A0A0A`, contract row 2) | `#18181B` literal; `BG_PRIMARY` `#18181B` |
| Main toolbar, Dev toolbar, contextual bar fill | default | `BG_TOOLBAR` (bg-primary `#0A0A0A`, row 15) | `BG_TOOLBAR` `#18181B` |
| Toolbar inner ring, contextual bar border | default | `INNER_HIGHLIGHT` (border-secondary_alt `#262626`, row 53) | `INNER_HIGHLIGHT` white α22; `#383838` literal |
| Palette frame | default | `BG_MENU` (bg-primary `#0A0A0A`, row 7) + `BORDER_MENU` (border-secondary_alt `#262626`, row 22), radius `RADIUS_XL` 12 (J rule 2, plan.md C-R: the live kit `Command bar` is radius-xl), no shadow | `#2C2C2C` + `#383838` literals, `CornerRadius::same(6)` |
| Rail fill, ruler strips, window background, `clear_color` | default | `BG` (bg-primary `#0A0A0A`, row 1); `clear_color` returns `BG` as linear floats | `BG` `#121215`; `[0.17, 0.17, 0.17]` |
| Hairlines: title bar bottom, tab bar bottom, Home divider, inactive-tab separators, rail edge and separators, toolbar and contextual dividers, ruler edges | default | `BORDER` (border-secondary `#262626`, row 20), C `Divider/Hairline` | `#27272A` literal, `BORDER`, `BORDER_STRONG` (Home divider) |
| Drop shadows: toolbar (`toolbar.rs:192-193`, `:389-390`), contextual bar (`:155`), Re-center (`:363`), toast (`shell.rs:1134`), HUD (`:3638`, `:3653`) | default | none: every dark-mode shadow token is alpha 0 (contract rows 48–50), so the draw calls are removed | `SHADOW_COLOR*`, `elevation_shadow` |
| Brand logo in the title bar; left-panel resize highlight | default; hover, drag | `BG_BRAND` (bg-brand-solid `#E36323`, R4) | `BG_BRAND` |
| Unsaved-changes dot on a tab | default | `ICON_BRAND_PRIMARY` (fg-brand-primary `#F97316`, N14) | `#0D99FF` (`BADGE_DESIGN_BLUE`) |
| Update button dot (hidden by Z14) | default | `SUCCESS` (fg-success-primary `#22C55E`, row 37; chrome status, controller default) | `#1BC47D` literal |
| Rail help dot (`rail.rs:163`) | always on | removed (R10.5: no state sits behind it, P6 Zero Fake UI); the `circle_filled` call goes, which also retires the rail's one chrome site | `#0D99FF` literal |
| File-kind badge on a tab (design, board, deck, spec) | default | the kit `Featured icon` tile that R10.2 gives the same file kinds on Home: tile `UTILITY_<HUE>_50`, glyph `UTILITY_<HUE>_600` (contract N39–N46): design blue `#172554`/`#60A5FA`, board amber `#451A03`/`#FBBF24`, deck green `#052E16`/`#4ADE80`, spec orange `#431407`/`#FB923C` (C-1.31–C-1.34). Deck and spec split (today both orange) so a tab and its Home tile show the same colour (decision 3) | `#0D99FF`, `#F59E0B`, `#FF6A3D` literals at a 38-alpha tint |
| Dev badge on a tab, Dev mode segment, Dev toggle | default; hover (toggle) | `DEV_READY` (`#14AE5C`, R10.4: Figma parity; G's `dev-accent` role); toggle hover `DEV_READY_HOVER` (G rule 2, new name); glyph `TEXT_ON_BRAND` (`#0A0A0A`, N10, 6.82:1) | `#1BC47D`, `#22D489`, white glyph (2.27:1), badge glyph `#0D1F16` |
| Focus ring on every focusable control in this spec | focused (keyboard) | `FOCUS_RING` (focus-ring `#F97316`, N11), 2 px at gap 2, drawn by C's widgets | none on most shell controls |
| Tooltips (rail, toolbar, tab bar, contextual bar) | shown | C `Tooltip/Default` with the contract tooltip ruling: `BG_PRIMARY_SOLID` `#262626` (N19), text `TEXT` `#FAFAFA` (C-1.19). egui stock tooltips, styled globally by C (`theme.rs:627-664`); no tooltip paint in D's files | egui window style |
| Ruler strip, ticks, labels, selection band and its labels, guides | default | unchanged: `BG`, `RULER_TICK`, `RULER_TEXT`, `RULER_SELECTION_FILL`, `CANVAS_SELECTION`, `RULER_GUIDE` (contract rows 80–83 and (e): canvas overlays). Labels stay 9 px proportional (`FONT_SIZE_CANVAS`, row 80 "the 9px ruler exception"); see edge case 9 | same |
| Ruler guide value badge (`rulers.rs:419-422`) | dragging a guide | fill `RULER_GUIDE` `#F24822` unchanged; label `TEXT_ON_BRAND` `#0A0A0A` (5.40:1; white is 3.67:1 and fails AC6) | white literal |
| Recording HUD: pointer accent, drag line and ring, pill stroke, keycap fill, progress line | `--panel micro-*:seq` only | `BG_BRAND` `#E36323` or `INFO` `#0D99FF` (values unchanged: the pointer mirrors canvas state); shades stay computed | literals `:3594` |
| Recording HUD: pointer arrow body and outline | same | body `HUD_POINTER_FILL` (new name, value `#111216` unchanged: a cursor, not chrome); outline `ICON_WHITE` (N17) | `#111216`, white |
| Recording HUD: metric pill and caption bar | same | fill `BG_PRIMARY` (opaque); bar stroke `BORDER`; pill text and caption `TEXT`; keycap label `TEXT_ON_BRAND` (5.73:1 on orange, 6.62:1 on blue; white on orange is 3.46:1); radii: pill `RADIUS_CONTROL` 8, bar `RADIUS_XL` 12 (was 10), keycap `RADIUS_SEGMENT` 6 | `rgba(17,18,22,242/238)`, white α36, white, `#F4F4F6` |
| Toast (`shell.rs:1087-1160`, `shell/toast_layout.rs`) | shown | J rules 12–15 on C `Toast/Default` (J owns the anatomy, D applies it): fill `BG_PRIMARY_ALT` (bg-primary_alt `#171717`, N18), 1 px border `BORDER_MENU` `#262626`, no shadow, radius `RADIUS_XL` 12 (J rule 14; live kit `Notification` 1135:2645 is radius-xl; plan.md C-R, not 16), text `TEXT` text-sm 14 medium, height `TOAST_HEIGHT` 40; status dot 8: info `ICON_BRAND_PRIMARY` (N14), error `DANGER`, success `SUCCESS` (C adds it; no caller today); the action ("Show in folder" or "Enter a path…") and the web-capture "Details" link are C `Button/Tertiary` xs: label `TEXT_SECONDARY`, hover label `TEXT_SECONDARY_HOVER`, hover fill `BG_SECONDARY_HOVER` `#262626` (J's `BG_HOVER` `#171717` equals the toast fill and would be invisible; J rule 13 carries the same correction) | `BG_MENU`, `elevation_shadow`, dot `BG_BRAND`/`DANGER`, hover `BG_HOVER`, label `TEXT_BRAND` |

3. **Widgets and states (spec C).** Every control below uses C's anatomy and takes its states verbatim from C §3.4, mapped to contract constants here. Owner rulings that override C are applied (R8.2 selected `#262626`, R8.5 floating chrome 16 with items 8 at inset 8, R8.6 button labels text-sm 14 Semibold while tabs and rows stay 12, row and pill radius 8 per R5). Where D must differ from C, the row says so and the difference is sent to C (not to the owner) in section 12.

| Control (code at `8dfaee93`) | C widget | Size | Difference from C, sent to C |
|---|---|---|---|
| Project tab (`tabs.rs:1007-1065`) | `Tabs/Pill`, kind badge as the leading icon | 26 tall in the 32 bar, 112–240 wide, pill radius 8 (`RADIUS_CONTROL`) on all four corners, no outline; label text-xs 12 semibold (R8.6 keeps tabs at 12) | height 26 (C derives xs 32); decided by R10.1 (R1 over R5) |
| Home (`tabs.rs:966-980`) | `Tabs/Pill` with the house glyph | 72×24, radius 8 | height 24; R10.1 |
| `+` (`tabs.rs:1108-1122`) | `Button/Icon` | 24×24 (meets `TARGET_MIN`), glyph 13 | size 24 (C has xs 32 and sm 36); R10.1 |
| Tab close × (`tabs.rs:1014`, `:1075-1088`) | `Button/Close` | hit box 24×24 (`TARGET_MIN`) centred where the 18 px box is today; visible hover and pressed fill stays 18×18 at `RADIUS_SEGMENT` 6; glyph 10 | visual 18 (C xs 32); pressed equals hover on the active tab (state table below) |
| Update button, hidden (`tabs.rs:1126-1157`) | `Button/Tertiary` with the status dot | height 24; label text-sm 14 Semibold (R8.6); width grows to fit (INFERENCE about 72; right-aligned; not drawn while `UPDATER_UI_ENABLED` is false) | height 24 |
| Tab menus (`tabs.rs:1160-1223`) | `Menu/Panel`, `Menu/Item` (painted by C's `widgets/menu.rs`) | rows 32, see Menus | none |
| Rail main-menu button (`rail.rs:43-55`) | `Button/Icon` xs, Selected while the main menu is open | 32×32, icon 18 kept (R6) | none |
| Rail items (`rail.rs:58-144`) | `Button/Tool` (tile 32 centred in the 47×56 cell; the whole cell is the hit box) | unchanged | none |
| Rail help button (`rail.rs:146-162`) | `Button/Icon` xs | 32×32 | none; its dot is removed (R10.5) |
| Tool buttons (`toolbar.rs:209-228`), Actions (`:258-271`), Dev tools (`:398-455`) | `Button/Tool`; Actions shows Selected while the palette or `cmdk` is open (`:262`) | 32×32, icon 20 | none |
| Tool-group chevrons (`toolbar.rs:230-253`) | `Button/Tool` (chevron form) | visual 16×32 unchanged (toolbelt width unchanged, R1); hit box 24×32: it extends 8 px to the right into the `TOOL_GROUP_GAP` that follows every chevron; the menu anchor (`menu_anchor`, `:250-252`, and `chevron_rect` used by `menus.rs:626`) stays the visual 16 px rect | visual width 16 |
| Mode switch (`toolbar.rs:279-337`) | `Tabs/Segmented` (track 32, pad `MODE_SWITCH_PAD` 2, segments 28 at `RADIUS_SEGMENT` 6) | unchanged | C's Selected tile is `BG_SELECTED` `#262626` like D's (R8.2); the one difference left is the border: C draws a 1 px `BORDER_STRONG` `#404040` on the selected tile, D drops it (contract row 18: a `#262626` tile on the `#171717` track already separates). Sent to C |
| Contextual bar buttons (`toolbar.rs:160-189`) | `Button/Icon` xs ×2, `Button/Tertiary` xs "More" with a chevron (label 14 Semibold, R8.6) | 32 tall; bar 40 tall at inset 4, radius `RADIUS_XL` 12 (concentric: 8 + 4) | none |
| Re-center (`toolbar.rs:360-385`) | `Button/Secondary` xs; the "⇧2" hint is `Menu/Shortcut` text | 32 tall; label 14 Semibold (R8.6); radius 8 | none |
| Dev toggle (`toolbar.rs:462-469`) | filled icon button in `DEV_READY` (R10.4; no C widget) | 36×32 unchanged | not a C widget: G owns the role |
| Shell menus (`menus.rs` via `widgets/menu.rs`) | `Menu/Panel`, `Menu/Item`, `Menu/Shortcut`, `Menu/Separator` | rows `MENU_ITEM_HEIGHT` → `CONTROL_XS` 32 (contract (c)); panel radius 16 (`RADIUS_FRAME`, R8.5); item pill radius 8 (`RADIUS_CONTROL`) at inset 8 (`MENU_PILL_INSET` = `INSET_FRAME`, already 8 today) | R8.5 overrides C's `Menu/Panel` radius-md 8 and `Menu/Item` pill radius-sm 6 at inset 6 |
| Actions palette (`menus.rs:907-1085`) | `Dialog/CommandPalette`, composed exactly like J rule 2 so both palettes match | width 520 unchanged; input row 40 (J rule 2; +2, the 38 px row grows, R1 allows growth) holding C's borderless `Input/Search`; tab row stays 34 with underline tabs; section row 22; item pitch 36 (row 32 with a 2 px inset; J OQ1); frame radius `RADIUS_XL` 12 (J rule 2 and plan.md C-R; C's "palette 16" is wrong and C follows) | none beyond J's |
| Toast (`shell.rs:1087-1160`) | `Toast/Default` per J rules 12–15, both actions as `Button/Tertiary` xs | height 40 unchanged; the action's pointer target `ACTION_HEIGHT` 24 → `CONTROL_XS` 32 in `shell/toast_layout.rs` (it sits inside the 40 px toast with 4 px above and below) | none |
| Show-sidebar button (`shell.rs:1667-1672`) | `Button/Icon` xs, sized with `CONTROL_XS` 32 directly at `shell.rs:1668`; `PANEL_ACTION_BUTTON` stays 24 (B rule 6, contract appendix: the same constant sizes F's inspector buttons at x 209) and E's hide-sidebar button takes its own local 32 | 24 → 32, floats on the canvas | none |
| Left-panel resize strip (`shell.rs:1686-1693`) | not a widget: a drag edge (plan.md C-R) | visible line unchanged (1 px hairline, 2 px `BG_BRAND` on hover and drag); hit band `DRAG_DIVIDER_HIT` 24 (new name, = `TARGET_MIN`) centred on the panel edge, up from 6; priorities in the divider table below | hit area 6 → 24 |
| Ruler strips (`rulers.rs:99-106`, `:228-262`) | not a widget: a drag edge (plan.md C-R) | strips stay 20 px and paint as today; the press band is 24 px thick (`DRAG_DIVIDER_HIT`), 4 px more on the canvas side; priorities in the divider table below | hit area 20 → 24 |
| Web-capture report modal (`shell.rs:1162+`) | `Dialog/Modal` per J rule 3 | unchanged | none |

State colours for the widgets D paints itself (C §3.4 values; surface-relative hover rule S6):

| Widget | Default | Hover | Pressed | Selected |
|---|---|---|---|---|
| `Button/Tool` (tools, chevrons, Actions, rail items, Dev tools) | no fill; glyph `ICON_QUATERNARY_HOVER` `#A3A3A3` | `BG_TOOLBAR_HOVER` (rail: `BG_HOVER`) `#171717`; glyph `ICON_SECONDARY_HOVER` `#E5E5E5` | `BG_PRESSED` `#404040`; glyph `ICON_SECONDARY_HOVER` | `BG_TOOLBAR_SELECTED` (rail: `BG_SELECTED`) `#262626`; glyph `ICON` `#FFFFFF`. An open chevron menu shows the Hover fill, as today |
| `Button/Icon` (rail menu and help, `+`, contextual buttons, show-sidebar) | no fill; glyph `ICON_QUATERNARY` `#737373` | `BG_HOVER` `#171717`; glyph `ICON_QUATERNARY_HOVER` `#A3A3A3` | `BG_PRESSED`; glyph `ICON_QUATERNARY_HOVER` | `BG_SELECTED`; glyph `ICON_SECONDARY_HOVER` `#E5E5E5` |
| `Button/Close` on an inactive tab that is not hovered (tab `BG_PRIMARY`) | glyph `ICON_QUATERNARY` (4.18:1) | (hovering the × also hovers the tab: next row) | — | n/a |
| `Button/Close` on a hovered inactive tab (tab `BG_HOVER` `#171717`) | glyph `ICON_QUATERNARY` (3.78:1) | `BG_SECONDARY_HOVER` `#262626` (S6); glyph `ICON_QUATERNARY_HOVER` (6.00:1) | `BG_PRESSED` `#404040`; glyph `ICON_QUATERNARY_HOVER` (4.11:1) | n/a |
| `Button/Close` on the active tab (tab `BG_SELECTED` `#262626`) | glyph `ICON_QUATERNARY` (3.19:1) | `BG_PRESSED` `#404040` (S6); glyph `ICON_QUATERNARY_HOVER` (4.11:1) | same as Hover (sent to C) | n/a |
| `Tabs/Pill` (file tabs, Home) | no fill; label `TEXT_QUATERNARY` `#A3A3A3`; Home glyph and pinned lock `ICON_QUATERNARY` | `BG_HOVER`; label `TEXT_SECONDARY` `#D4D4D4`; glyph `ICON_QUATERNARY_HOVER` | n/a | `BG_SELECTED` `#262626`, no outline; label `TEXT_SECONDARY`; glyph `ICON_QUATERNARY_HOVER` |
| `Tabs/Segmented` (mode switch, track `BG_TOOLBAR_MODE_SWITCHER` `#171717`) | no fill; glyph `ICON_QUATERNARY` (3.78:1) | no fill; glyph `ICON_QUATERNARY_HOVER` | n/a | `BG_SELECTED`; glyph `ICON_SECONDARY` `#D4D4D4`. Dev segment selected: `DEV_READY` fill, glyph `TEXT_ON_BRAND` |
| `Button/Secondary` (Re-center, on canvas) | `BG_PRIMARY`, border `BORDER_STRONG` `#404040`; label `TEXT_SECONDARY`; hint `TEXT_QUATERNARY` | `BG_HOVER`; label `TEXT_SECONDARY_HOVER` `#E5E5E5` | `BG_PRESSED`; label `TEXT_SECONDARY_HOVER` | n/a |
| `Button/Tertiary` ("More", Update) | no fill; label `TEXT_TERTIARY` `#A3A3A3`; glyph `ICON_QUATERNARY` | `BG_HOVER`; label C's text-tertiary_hover `#D4D4D4`; glyph `ICON_QUATERNARY_HOVER` | `BG_PRESSED`; same label | n/a |
| Palette rows and tabs (J rule 2) | title `TEXT_SECONDARY`, icon `ICON_TERTIARY`, shortcut `TEXT_QUATERNARY`; other tabs `TEXT_QUATERNARY`; headings `TEXT_TERTIARY`; typed text `TEXT`, placeholder `TEXT_PLACEHOLDER` (R8.1) | row `BG_MENU_HOVER` `#171717` | n/a | row `BG_SELECTED`, title `TEXT`, icon `ICON`; current tab `TEXT` with a 2 px `BORDER_SELECTED` underline (replaces the `#383838` fill and white label) |
| `Button/Tertiary` on the toast (action, "Details") | no fill; label `TEXT_SECONDARY` `#D4D4D4` | `BG_SECONDARY_HOVER` `#262626`; label `TEXT_SECONDARY_HOVER` `#E5E5E5` | `BG_PRESSED` `#404040`; label `TEXT_SECONDARY` | n/a |

**Drag dividers (plan.md C-R; the same rule is in E for the Pages/Layers separator).** The visible line of a divider stays as it is. Its hit band becomes `DRAG_DIVIDER_HIT` 24 (new name, `TARGET_MIN`) centred on the line. Inside the band, in this order: (1) the 6 px core that is the hit area today (`PAGES_SEPARATOR_HIT`, `shell.rs:1686-1687`) keeps winning over everything it wins today, including the outer 2 px of the layers scrollbar track and the first 3 px of the left ruler; (2) outside the core, a scrollbar track keeps its own hit area (the layers scrollbar, an 8 px track, `panels/mod.rs` `scrollbar`), and so do controls and the ruler strips, because they exist today; (3) the divider band wins over everything else in it: canvas pointer handling (marquee, tool presses) and list rows.

| Divider | Visible line | Band today | Band after (panel-local x of `left_rect.right()` = R, ruler y from the canvas edge) | Free area after (nothing else claims it) |
|---|---|---|---|---|
| Left-panel resize strip | 1 px hairline at R − 1 (`left.rs:30`), 2 px `BG_BRAND` on hover | [R − 3, R + 3] = 6 | [R − 12, R + 12] = 24, full panel height | the layers scrollbar track takes [R − 9, R − 1] (8 px), minus the core's [R − 3, R − 1]; the panel's action buttons end at R − 9; so the panel side adds [R − 12, R − 9) = 3 px free. Canvas side with rulers off: [R + 3, R + 12) = 9 px free, so 6 + 3 + 9 = 18 px. With rulers on the left ruler (20 px, [R, R + 20)) keeps [R + 3, R + 20), so 6 + 3 = 9 px |
| Ruler strips | strip fill and 1 px `BORDER` hairline (`rulers.rs:431-432`) | strip 20 px | 24 px: the strip plus 4 px of canvas. An existing guide's own 4 px grab zone wins in those 4 px (today's behaviour there); otherwise the ruler wins over the canvas | the 4 px |

The 24 px figure is the band, not the free area: the scrollbar and the rulers are not asked to give up pixels, so the free width is 9 px with rulers on (the default in a real build, `shell.rs:283`) and 18 px with rulers off. Flagged to the controller in the report. **Mechanism.** `widgets::record` is a test-only hook (a no-op in release builds, `widgets/mod.rs:290`), so no production code may read recorded rects, and the `left-panel-resize-band` and `-hit` names in this spec are for tests and L only. The handle is registered after `left::show` (`shell.rs:1689`), so a 24 px rect registered there would take the scrollbar track and the panel buttons: the 6 px core stays registered last (on top, as today) and the extension is registered before the panel is drawn (under its widgets), clipped where the left ruler keeps its strip.

`ICON_QUATERNARY` never sits on `BG_PRESSED` (2.19:1, contract rule); every `#404040` state above uses `ICON_QUATERNARY_HOVER` or brighter. The menus D opens (main, file, tool-group, zoom, tab menus) are painted by C's `widgets/menu.rs` with C's `Menu/*` states; D checks them in its captures but does not paint them.

4. **States.** Every interactive element shows C's Default, Hover, Pressed, Focused (keyboard) and Selected states where C's widget has them; disabled menu rows take C's Disabled (50 %). D removes no state; the only state it adds is the Pressed state that C's widgets carry.
5. **Sizes (R5, R8.5, R8.6) and their layout effect.** Regions, widths and order do not change (R1). Heights:

| Element | Constant / code | Today | Target | Δ | Layout effect |
|---|---|---|---|---|---|
| Title bar | `shell.rs:86` | 28 | 28 | 0 | none |
| Wordmark | `shell.rs:1543-1554` | 11 strong/regular | text-sm 14/20 (decision 1) | 0 | brand box ×14/11 wider; nothing else sits in the bar |
| Tab bar | `tabs.rs:21` | 32 | 32 | 0 | none (R10.1) |
| Project tab / Home / `+` | `tabs.rs:1007`, `:966`, `:1108` | 26 / 24 / 24 | unchanged | 0 | none (R10.1: tab heights stay as today) |
| Tab close hit box | `tabs.rs:1014`, `:1018` | 18×18 | 24×24 (`TARGET_MIN`, P3 floor); visual 18 | 0 | none: the box spans tab top + 1 to + 25 and `right − 25` to `right − 1`; the tab body's hit area ends 2 px left of it (today's rule, `:1018`) |
| Tab title type | `font_body`/`font_body_strong` | 11 | text-xs 12/18 semibold (R8.6) | 0 | titles elide about 8 % earlier (`truncate_tab_title`, `:1065`) |
| Rail | `theme.rs:263`, `:268`, `:274` | 48 / 56 / 32 | unchanged | 0 | none |
| Toolbar, tool buttons | `theme.rs:403`, `:406` | 48 / 32 | unchanged | 0 | none |
| Chevron hit box | `toolbar.rs:232` | 16×32 | 24×32; visual 16 | 0 | none: `toolbelt_is_512_wide_like_figma` stays green |
| Mode segments | `toolbar.rs:284-287` | 28 | 28 | 0 | none |
| Contextual bar | `toolbar.rs:151-189` | buttons 28, bar 128×34 | buttons 32, bar 40; "More" label 14 Semibold | +6 height, +31 width | floats 6 px higher (the bottom stays 8 px above the toolbar); width grows to 159, measured on the bundled Inter: 4 inset + 2 × (32 + 2) icon buttons + 3 + 5 divider gaps + 75 "More" (10 pad + 35 label at 14 Semibold + 4 gap + 16 chevron + 10 pad, R11 whole pixels) + 4 inset = 159; stays centred (the PR measures; L's allow entry carries 2 px of slack) |
| Re-center | `toolbar.rs:360` | 118×28 | 32 tall; label 14 Semibold | +4 height, about +15 width | floats 4 px higher (the bottom stays 8 px above the bar); width grows to about 133 (INFERENCE: 10 pad + "↩ Re-center" about 85 at 14 Semibold + 8 gap + "⇧2" 20 + 10 pad; the PR measures); centred |
| Menu rows | `theme.rs:424` (B: `CONTROL_XS`) | 24 | 32 | +8 per row | main menu (15 rows, 5 separators): 461 → 541 px with C's 9 px `Menu/Separator`, or 581 if `MENU_SEP_MARGIN` stays 8 (B and C decide); File submenu (12 rows, 2 separators): 338 → 418 or 434; the Help and account submenu (6 rows, 3 separators since "Open starter sample"): 6 × 24 + 3 × 17 + 16 = 211 → 6 × 32 + 3 × 9 + 16 = 235 or 6 × 32 + 3 × 17 + 16 = 259; at `MIN_WINDOW` 900×600 the existing clamp and scroll apply |
| Palette | `menus.rs:907-911`, `:953` | 296 (38 + 1 + 34 + 1 + 22 + 8 × 24 + 8) | 394 (40 + 1 + 34 + 1 + 22 + 8 × 36 + 8) | +98 | +2 from the 40 px input row (J rule 2) and +96 from the rows at pitch 36; still anchored 68 px above the window bottom; its top moves up 98 px (clamped at y 20) |
| Toast | `theme.rs:466` | 40 | 40 | 0 | none |
| Show-sidebar button | `shell.rs:1668` | 24 | 32 | +8 | floats on the canvas at (+8, +8); no region change |
| Rulers | `theme.rs:532-533` | 20, 9 px labels | unchanged | 0 | none |
| Toast action target | `toast_layout.rs` `ACTION_HEIGHT` | 24 | 32 | +8 | inside the 40 px toast (4 px above and below); a wrapped toast is taller and centres it; no L case (every capture runs with `--no-toast`) |
| Resize strip, ruler strips | `shell.rs:1686-1687`, `rulers.rs:99-106` | hit 6 wide, strips 20 | hit 24 (new recorded name `left-panel-resize-band`; the strips are not recorded) | 0 painted | none painted; see the divider table in rule 3 |

**L allow entries D needs** (L's schema; a hit area that grows into a gap without moving a neighbour is recorded under a new `-hit` or `-band` name, so the baseline names keep their rects and L's derived cumulative shift does not push neighbours that did not move; `specs/L-…md` rule 9: `case`, `region`/`name_prefix`/`h_base`, `anchor_*`, `max_dw`, `max_dh`, `dx`, `dy`, `spec`, `reason`; every entry has `spec: "D"`). L's own examples for D (palette `max_dh` 64, contextual bar `dy: -6` with a bottom anchor and `max_dw` 12) disagree with these numbers; L takes these:

| Target (name or matcher) | Case | Entry | Arithmetic |
|---|---|---|---|
| Actions palette frame | `D-05-menu-palette` | `anchor_y: bottom`, `max_dh: 98` | 296 → 394: input row +2, 8 rows × (36 − 24) = +96 |
| Palette item rows | `D-05-menu-palette` | `h_base: 24`, `max_dh: 12` | slot 24 → 36 (the row widget may be the 32 px pill: +8, inside the same bound) |
| Palette input row | `D-05-menu-palette` | `h_base: 38`, `max_dh: 2` | 38 → 40 |
| `contextual-floating-bar` | none today (hidden while a toast is pending, not capturable) | `anchor_x: centre`, `anchor_y: bottom`, `max_dw: 33`, `max_dh: 6` | 159 − 128 = +31 plus 2 px slack; 40 − 34 = +6 with the bottom edge fixed, so the top moves up 6 (`dy` stays 0; L's example `anchor_y: bottom, dy: -6` would move the bottom edge) |
| `ctx-select-area`, `ctx-edit-prompt` | same | `name_prefix: "ctx-"`, `max_dw: 4`, `max_dh: 4` | 28 → 32 |
| `dev-recenter` | `D-09-dev-css` | `anchor_x: centre`, `anchor_y: bottom`, `max_dw: 20`, `max_dh: 4` | 118 → about 133, 28 → 32 with the bottom edge fixed |
| Main menu frame, File submenu frame | `D-02-menu-main`, `D-03-menu-main-file` | `anchor_y: top`, `max_dh: 120` | main 461 → 541 (C's 9 px separator) or 581 (separator margin stays 8): +80 or +120; File 338 → 418 or 434 is inside the same bound |
| Menu rows | `D-02-menu-main`, `D-03-menu-main-file`, `D-04-menu-shape` | `h_base: 24`, `max_dh: 8`; separators `h_base: 17`, `min_dh: -8` only if C adopts its 9 px separator (cites C `Menu/Separator`, R5) | 24 → 32 per row |
| `tab-close-<id>` | `D-01-editor-default` | none: it stays the 18 px visual rect (`tabs.rs:1015`); the 24 px hit is recorded as the new name `tab-close-hit-<id>`, which L reports and does not baseline (L's derived shift would otherwise push the next tab by the 6 px a hit growth does not move) | hit 18 → 24 around the same centre |
| `tool-chevron-<gi>`, `tool-chevron-hit-<gi>` | `D-01-editor-default` | none: neither name is recorded today; both are new (visual 16×32 and hit 24×32), reported, not baselined | hit 16 → 24, the extra 8 px reach into `TOOL_GROUP_GAP` |
| `show-sidebar` | a collapsed-sidebar case | `max_dw: 8`, `max_dh: 8` (top-left fixed) | 24 → 32 |
| `left-panel-resize-handle` | every case with the panel open | none: it stays the 6 px core (as L's own example records it); the band is the new name `left-panel-resize-band` (24 wide, centred on the panel edge), reported, not baselined | 6 → 24 for the band |

The ruler strips are not recorded widgets, so L has no entry for them; the unit test in section 10 covers the band.

6. **Radii.** Literal radii in the owned files (`tabs.rs` `cr(6)`, `cr(3)`, `cr(4)`; `toolbar.rs` `cr(8)`, `cr(6)`; HUD `8.0`, `10.0`, `6.0`; `menus.rs` `CornerRadius::same(6)`) take B's constants: controls, rows and pills `RADIUS_CONTROL` 8; floating chrome (menus, the toolbar frame) `RADIUS_FRAME` 16 (R8.5); the Actions palette frame, the toast, the contextual bar and the HUD caption bar `RADIUS_XL` 12 (live kit `Command bar` and `Notification`, plan.md C-R); segments and the close fill `RADIUS_SEGMENT` 6; the 14 px kind badge tile `RADIUS_MARK` (B's small mark role; today `cr(3)`). Nested shapes stay concentric (P4): toolbar 16 − inset 8 = 8, contextual bar 12 − 4 = 8, menu 16 − 8 = 8.
7. **No behaviour change.** Shortcuts, tooltip text, menu contents, widget ids recorded through `widgets::record` and the order of controls stay. Hit areas change only where the 24 px floor requires it (tab close, chevrons, the resize strip and the ruler strips, rule 5). Z08, Z13 and Z14 stay green.

## 4. Data model / schema changes

None. No document, preference or file-format change. Only paint calls, constants consumed from `theme.rs` and C's widget calls change.

## 5. Contracts

**File ownership across D–K.** Each file has exactly one owner. D owns:

| File | Owner | Shared surface and rule |
|---|---|---|
| `crates/designos-app/src/tabs.rs` | D | the Dev badge takes the Dev accent (`DEV_READY`, R10.4; G's role); the Update button's state comes from `updater.rs` (J); D paints the button, Z14 keeps it hidden |
| `crates/designos-app/src/shell.rs` | D | toast: J's rules 12–15 on C `Toast/Default`, painted by D (both actions, "Show in folder" and "Enter a path…"); web-capture report modal (`:1162+`): C `Dialog/Modal` per J rule 3; show-sidebar button (`:1667-1672`) pairs with E's hide-sidebar button (D sizes its own with `CONTROL_XS`, E uses a local name; neither touches `PANEL_ACTION_BUTTON`); left-panel resize handle (`:1685-1693`), its `BG_BRAND` highlight and its 24 px band are shared with E and #1108 (rule 3 divider table); the recording HUD is used by marketing films; spec L edits this file first (section 12); the micro-demo `designos_doc::Color::rgb` builders are document content and are not touched |
| `crates/designos-app/src/shell/toast_layout.rs` | D | the toast's size and action target; no colours; J writes the anatomy (rules 12–15), D applies it |
| `crates/designos-app/src/panels/rail.rs` | D | #1106 is merged; D restyles the icon-only rail and adds no labels |
| `crates/designos-app/src/panels/toolbar.rs` | D | the Dev-mode toolbar takes the Dev accent (`DEV_READY`, R10.4); the Actions "open" state is shared with `cmdk.rs` (J) |
| `crates/designos-app/src/panels/menus.rs` | D | the Actions palette (`:907-1085`) is D's; J owns the separate `cmdk.rs` command bar and states in its §1 that it does not edit this file; the zoom menu opens from F's zoom control |
| `crates/designos-app/src/canvas/rulers.rs` | D | the press band of the strips (24 px) shares the left edge with the resize strip (rule 3 divider table) |

Not mine (listed so no file is owned twice; refreshed 2026-10-28 from plan.md's ownership rulings and the H–K tables): `theme.rs`, `design/*` (A, B); `widgets/*` (including `widgets/menu.rs`, `widgets/field_tab.rs`, `widgets/popup_open.rs`) and `panels/mod.rs` (C; `widgets/picker.rs` is F's); `icons.rs` (icon tint and stroke restyle, R6: C); `panels/left.rs`, `panels/left_sep.rs` (#1108's separator, plan.md 22:55), `panels/layers.rs`, `find/filter.rs` (E); `panels/right.rs`, the inspector files and the style editors `panels/style_picker.rs`, `panels/style_edit.rs`, `panels/style_edit_grid_rows.rs` (F); `dev/*` (including the new `dev/code_view.rs`), `specs.rs` (G); `home/*` except `home/dialog.rs` (H); `terminal.rs`, `agent_*.rs`, `canvas/agent_*.rs`, `skills/*`, `image_jobs.rs` (status chip, inline loading) (I); `cmdk.rs`, `updater.rs` (the module `crate::home::updater`; there is no `home/updater.rs`), `home/dialog.rs`, `present.rs`, `panels/context.rs`, `find/panel.rs`, `find/mod.rs`, `find/select.rs`, `comments/*`, `history/*` (J); `panels/assets.rs`, `panels/check_designs.rs`, `variables/*`, `libraries/*`, `styles.rs`, `plugins/*` (K). Unassigned in D–K (flag to the controller): `canvas/draw.rs`, `canvas/grids.rs`, `canvas/tiles.rs`, `canvas/interact.rs`, `board/*`, `decks/*`, `lint.rs`.

Also D's, for colour-literal and constant re-points only (controller, plan.md H–K check rulings, 2026-10-29 01:05): `decks/ui.rs`, `board/*`, `canvas/draw.rs`, `canvas/grids.rs`, `canvas/tiles.rs`, `canvas/interact.rs`; no layout change (R1).

**Colour-literal gate.** Spec A rule 13 adds `scripts/check_chrome_colour_literals.py` (new name) with the regex and test stripping of section 2, a per-file baseline that only falls, and an allowlist. D's allowlist: the `Color32::TRANSPARENT` sentinels (not counted) and the computed sites in rule 1. Until the gate lands, the section 2 command is the check and must print `chrome_sites=0` for every owned file.

## 6. File-format mapping

None.

## 7. UI

- **Implementer setup.** Read the in-repo design sources first: `design/principles.md`, `design/soul.md`, `specs/token-contract.md` (A), spec B's scale and spec C's widget kit; they win over any generic default. The Purple Ban stays (R4): the kit's purple brand (`#7F56D9`, `#9E77ED`) is never used. Contributors who use the project's agent kit may also load its `es-designer` skill; it is not required.
- **Regions.** Title bar 28, tab bar 32, rail 48, floating toolbar centred 12 px above the window bottom, rulers 20. Same positions as the L baseline.
- **Copy.** No copy changes. UX-writing gate Z16 stays green.
- **Evidence.** Before and after captures go into the PR body (not Figma, R3). Responsive: SKIPPED (owner default). The title bar has no CLI capture (section 2); the PR shows it with a macOS window capture of a manual run (`screencapture -l <window-id>`), labelled manual.

## 8. Edge cases

1. Text grows from 11 to 12 px in fixed boxes: tab titles (`tabs.rs:1061-1065`) and menu labels elide earlier. Elision stays on a character boundary with the ellipsis, never clips. The menu pill inset is already 8 (`MENU_PILL_INSET`, `theme.rs:428`), so R8.5 costs no label width.
2. Close hit box at the minimum tab width (112): the tab body keeps at least 24 px of hit width (`.max(tab_rect.left() + 24.0)`, `tabs.rs:1018`); the title column does not change because `right_reserve` (`:1061`) already reserves 24 px for the close mark.
3. Chevron hit box: it reaches into the gap that follows it; the next tool's hit box starts after that gap, so no two hit boxes overlap. `primary_pressed_in(ui, cr)` (`toolbar.rs:247`) uses the 24 px box; the menu anchors to the 16 px visual rect, so menus open where they do today. `a_press_on_a_floating_panel_over_the_chevron_is_not_a_chevron_press` (`:721`) stays green.
4. Dev mode on: the tab badge, the mode segment and the toolbar switch to `DEV_READY` together; the glyph on the Dev fill is dark (`TEXT_ON_BRAND`), never white.
5. Actions open: the button shows `Button/Tool` Selected, not blue. The palette and `cmdk` both set "open" (`toolbar.rs:262`); both show the same state.
6. Menus taller than the window (main menu at `MIN_WINDOW` 900×600 after rows grow to 32): the existing clamp and scroll apply; no row is unreachable.
7. `clear_color` (`shell.rs:1327-1329`) shows for one frame before the first paint and behind transparent regions: it returns `BG` converted to linear floats.
8. Board, deck and presentation modes reuse the tab bar and rail; their own toolbars (`board/toolbar.rs`, `decks/ui.rs`) are not in this spec (unassigned, section 5).
9. Ruler numerals stay in the proportional 9 px `font_ruler()` (`theme.rs:546-548`) although the soul asks for mono numeric coordinates: the ruler is a canvas overlay (contract row 80, "the 9px ruler exception"). D records it as a named canvas exception and sends it to A to write into the soul.
10. The left-panel resize handle (`shell.rs:1686-1687`) is a 6 px drag strip the height of the panel, not a button, and the ruler strips are 20 px press bands. Plan.md C-R gives both a 24 px hit band centred on the line, with no new exception: rule 3's divider table says what wins where the band meets the layers scrollbar (the scrollbar keeps its track outside the 6 px core), the canvas (the band wins) and the left ruler (the ruler keeps [R + 3, R + 20)). Nothing is sent to A: the pointer floor holds as written. Keyboard path: the strip has none today (#1108's separator has one); a follow-up issue may add it, and D adds no behaviour (rule 7).
11. The recording HUD changes look (dark keycap label, opaque fills, no shadow). Marketing films that reuse it must be re-rendered from the released build (owner film rule); no new film is in this pack (21:35 sweep, M open question 3).
12. Toast with the second action: "Enter a path…" is longer than "Show in folder" by about 4 px at 14 Semibold; the toast wraps its message first (`toast_layout.rs` `MIN_TEXT_WIDTH`), never the action. Both actions keep a 32 px target inside the 40 px toast; a toast that wraps to two lines is taller and centres the action.

## 9. Non-goals

- Layout changes: no region moves, no width changes, no reordering (R1).
- New icons or icon art (R6: keep the set; stroke and colour follow the fg roles above). Icon licence is spec Q.
- Canvas overlay colours (selection, component cyan, guides, measurement, ruler ticks and labels) and document content (`DEFAULT_PAGE_BG`, default fills, the micro-demo builders). The one canvas change is the ruler guide badge label (AC6).
- `cmdk.rs`, `updater.rs`, the context menu, dialogs, Find and Comments (J); Home (H); the agent terminal (I). D paints the toast and the web-capture report modal with J's rules but does not write those rules.
- The paint of `widgets/menu.rs` (C).
- Making the title bar capturable from the CLI (a dogfood follow-up for spec L or O).

## 10. Test plan (red first)

Acceptance criteria:

1. The section 2 census (or A's `check_chrome_colour_literals.py` once it exists) prints `chrome_sites=0` for `tabs.rs`, `shell.rs`, `panels/rail.rs`, `panels/toolbar.rs`, `panels/menus.rs` and `canvas/rulers.rs`.
2. `python3 scripts/check_spacing_relationships.py` passes and every owned file's radius and font-size counts are at or below `scripts/spacing_baseline.json`.
3. Spec L's layout gate reports the same regions, widths and control order as the baseline for every capture below; every size or position change is covered by an entry of the "L allow entries D needs" table in rule 5 (palette +98, contextual bar +31 × +6, Re-center +4, menu rows +8, the four hit areas, show-sidebar +8), and by nothing else.
4. Every constant in section 3 is a token-contract constant or a new name A has added; `cargo test -p designos-app --bin designos theme::` passes (B's updated invariants).
5. `python3 scripts/zero_fake_ui_gate.py` passes (Z08, Z13, Z14 untouched) and `python3 scripts/design_os_self_evolve.py` G01 passes.
6. **Contrast.** `ui ds a11y` is not accepted: on this store it measured 0 pairs (false green, plan.md and contract (f)). The check is three parts:
   - (a) The contract's C-1 check (token-contract (f)) exits 0. Run 2026-10-28 from the specs folder against the contract as on disk on 2026-10-28 (23:19) and a scratch `theme.rs` built from `8dfaee93` with the contract's rows applied (C-1.1 to C-1.62 minus the retired C-1.25 and C-1.26): exit 0, "every C-1 row (62/62 on 2026-10-28; rerun against the final contract) pass, 4/4 exception floors held, 0 failure(s)". D relies on C-1.2 and C-1.54 (toast text), C-1.3 (palette selected title), C-1.14 (HUD keycap on orange), C-1.19 (tooltips), C-1.29 (focus ring on the rail), C-1.31–C-1.34 (kind badge glyphs on their tiles, R10.2) and C-1.35 (dark glyph on the Dev accent, R10.4).
   - (b) The D-P rows below, in the C-1 row shape, pass the same snippet with only the id prefix and the file changed. Run 2026-10-28 from the specs folder: exit 0, 46 rows ok (lowest non-text row D-P.20 3.19:1; lowest text row D-P.15 5.40:1; the placeholder exception D-P.16 4.18:1 against its 3.0 floor). D-P.40 to D-P.46 are the toast pairs, replacing the old reliance on C-1.11 (`TEXT_BRAND` on the toast): the action label is `TEXT_SECONDARY`.
   - (c) A adds the D-P rows to C-1 and to the `chrome_contrast_pairs` test (spec A section 10), as contract (f) asks of a spec that adds a pair. Until then D's PR runs (b).
7. Before/after captures of every command below are in the PR, each with its exit code.
8. The four raised hit areas (tab close, tool chevrons, the left-panel resize strip, the ruler strips) are at least `TARGET_MIN` (red tests below).

D-P rows (foreground on background, WCAG 2.x, the contract's luminance formula):

| Id | Foreground | Background | Min | Use |
|---|---|---|---|---|
| D-P.1 | `TEXT` #FAFAFA | `BG_PRIMARY` #0A0A0A | 4.5 | wordmark "design", HUD pill text and caption |
| D-P.2 | `TEXT_SECONDARY` #D4D4D4 | `BG_PRIMARY` #0A0A0A | 4.5 | wordmark ":os studio", Re-center label |
| D-P.3 | `TEXT_SECONDARY` #D4D4D4 | `BG_MENU` #0A0A0A | 4.5 | Menu/Item label, palette row title |
| D-P.4 | `TEXT_SECONDARY_HOVER` #E5E5E5 | `BG_MENU_HOVER` #171717 | 4.5 | Menu/Item hover label |
| D-P.5 | `TEXT_QUATERNARY` #A3A3A3 | `BG_PRIMARY` #0A0A0A | 4.5 | Tabs/Pill default label |
| D-P.6 | `TEXT_SECONDARY` #D4D4D4 | `BG_HOVER` #171717 | 4.5 | Tabs/Pill hover label |
| D-P.7 | `TEXT_SECONDARY` #D4D4D4 | `BG_SELECTED` #262626 | 4.5 | Tabs/Pill selected label |
| D-P.8 | `TEXT_QUATERNARY` #A3A3A3 | `BG_MENU` #0A0A0A | 4.5 | Menu/Shortcut, palette other tabs and shortcuts |
| D-P.9 | `TEXT_QUATERNARY` #A3A3A3 | `BG_MENU_HOVER` #171717 | 4.5 | Menu/Shortcut on a hovered row |
| D-P.10 | `TEXT_TERTIARY` #A3A3A3 | `BG_MENU` #0A0A0A | 4.5 | palette section headings |
| D-P.11 | `TEXT_SECONDARY_HOVER` #E5E5E5 | `BG_HOVER` #171717 | 4.5 | Re-center hover label |
| D-P.12 | `TEXT_TERTIARY` #A3A3A3 | `BG_TOOLBAR` #0A0A0A | 4.5 | "More" label (Button/Tertiary) |
| D-P.13 | `TEXT` #FAFAFA | `BG_MENU` #0A0A0A | 4.5 | palette typed text and current tab |
| D-P.14 | `TEXT_ON_BRAND` #0A0A0A | `INFO` #0D99FF | 4.5 | HUD keycap on the blue accent |
| D-P.15 | `TEXT_ON_BRAND` #0A0A0A | `RULER_GUIDE` #F24822 | 4.5 | ruler guide badge label (white is 3.67:1) |
| D-P.16 | `TEXT_PLACEHOLDER` #737373 | `BG_MENU` #0A0A0A | 3.0 | palette search placeholder (R8.1 named exception) |
| D-P.17 | `ICON_QUATERNARY` #737373 | `BG_PRIMARY` #0A0A0A | 3.0 | Button/Icon and Button/Close default glyph |
| D-P.18 | `ICON_QUATERNARY` #737373 | `BG_HOVER` #171717 | 3.0 | close glyph on a hovered tab |
| D-P.19 | `ICON_QUATERNARY` #737373 | `BG_TOOLBAR_MODE_SWITCHER` #171717 | 3.0 | Tabs/Segmented default glyph |
| D-P.20 | `ICON_QUATERNARY` #737373 | `BG_SELECTED` #262626 | 3.0 | close glyph on the active tab |
| D-P.21 | `ICON_QUATERNARY_HOVER` #A3A3A3 | `BG_TOOLBAR` #0A0A0A | 3.0 | Button/Tool default glyph, chevrons |
| D-P.22 | `ICON_QUATERNARY_HOVER` #A3A3A3 | `BG` #0A0A0A | 3.0 | rail item default glyph |
| D-P.23 | `ICON_QUATERNARY_HOVER` #A3A3A3 | `BG_HOVER` #171717 | 3.0 | Button/Icon hover glyph |
| D-P.24 | `ICON_QUATERNARY_HOVER` #A3A3A3 | `BG_SECONDARY_HOVER` #262626 | 3.0 | close hover on a hovered tab |
| D-P.25 | `ICON_QUATERNARY_HOVER` #A3A3A3 | `BG_PRESSED` #404040 | 3.0 | close hover and pressed on the active tab, Button/Icon pressed |
| D-P.26 | `ICON_SECONDARY_HOVER` #E5E5E5 | `BG_TOOLBAR_HOVER` #171717 | 3.0 | Button/Tool hover glyph |
| D-P.27 | `ICON_SECONDARY_HOVER` #E5E5E5 | `BG_HOVER` #171717 | 3.0 | rail item hover glyph |
| D-P.28 | `ICON_SECONDARY_HOVER` #E5E5E5 | `BG_PRESSED` #404040 | 3.0 | Button/Tool pressed glyph |
| D-P.29 | `ICON` #FFFFFF | `BG_SELECTED` #262626 | 3.0 | Button/Tool and rail item selected glyph |
| D-P.30 | `ICON_SECONDARY_HOVER` #E5E5E5 | `BG_SELECTED` #262626 | 3.0 | Button/Icon selected glyph (main menu open) |
| D-P.31 | `ICON_SECONDARY` #D4D4D4 | `BG_SELECTED` #262626 | 3.0 | Tabs/Segmented selected glyph |
| D-P.32 | `ICON_BRAND_PRIMARY` #F97316 | `BG_PRIMARY` #0A0A0A | 3.0 | unsaved dot on an inactive tab |
| D-P.33 | `ICON_BRAND_PRIMARY` #F97316 | `BG_SELECTED` #262626 | 3.0 | unsaved dot on the active tab |
| D-P.34 | `FOCUS_RING` #F97316 | `BG_PRIMARY` #0A0A0A | 3.0 | focus ring on the bars |
| D-P.35 | `FOCUS_RING` #F97316 | `BG_TOOLBAR_MODE_SWITCHER` #171717 | 3.0 | focus ring on the segmented track |
| D-P.36 | `DEV_READY` #14AE5C | `BG_TOOLBAR_MODE_SWITCHER` #171717 | 3.0 | Dev segment fill against its track |
| D-P.37 | `SUCCESS` #22C55E | `BG_PRIMARY` #0A0A0A | 3.0 | Update button dot |
| D-P.38 | `BG_BRAND` #E36323 | `BG_PRIMARY` #0A0A0A | 3.0 | title-bar logo |
| D-P.39 | `BG_BRAND` #E36323 | `BG` #0A0A0A | 3.0 | left-panel resize highlight |
| D-P.40 | `TEXT` #FAFAFA | `BG_PRIMARY_ALT` #171717 | 4.5 | toast text (= C-1.54, listed because D paints it) |
| D-P.41 | `TEXT_SECONDARY` #D4D4D4 | `BG_PRIMARY_ALT` #171717 | 4.5 | toast action label and "Details" (F-P.6's pair) |
| D-P.42 | `TEXT_SECONDARY_HOVER` #E5E5E5 | `BG_SECONDARY_HOVER` #262626 | 4.5 | toast action hover label |
| D-P.43 | `TEXT_SECONDARY` #D4D4D4 | `BG_PRESSED` #404040 | 4.5 | toast action pressed label (= C-1.6) |
| D-P.44 | `ICON_BRAND_PRIMARY` #F97316 | `BG_PRIMARY_ALT` #171717 | 3.0 | toast info dot |
| D-P.45 | `DANGER` #EF4444 | `BG_PRIMARY_ALT` #171717 | 3.0 | toast error dot |
| D-P.46 | `SUCCESS` #22C55E | `BG_PRIMARY_ALT` #171717 | 3.0 | toast success dot |

```bash
# from the specs folder; the contract's C-1 snippet with the id prefix and the file changed; exits 1 on a failure
python3 - D-surface-app-shell-tabs-rail-toolbar-rulers-menus.md <<'PY'
import re, sys
def lum(h):
    c = [int(h[i:i + 2], 16) / 255 for i in (0, 2, 4)]
    c = [x / 12.92 if x <= 0.04045 else ((x + 0.055) / 1.055) ** 2.4 for x in c]
    return 0.2126 * c[0] + 0.7152 * c[1] + 0.0722 * c[2]
bad = 0
for m in re.finditer(r"^\| (D-P\.\d+) \| `\w+` #(\w{6}) \| `\w+` #(\w{6}) \| ([\d.]+) \|", open(sys.argv[1]).read(), re.M):
    hi, lo = sorted((lum(m[2]), lum(m[3])), reverse=True)
    r = (hi + 0.05) / (lo + 0.05)
    ok = r >= float(m[4]); bad += not ok
    print(f"{m[1]:7} {r:5.2f}:1 (min {m[4]}) {'ok' if ok else 'FAIL'}")
sys.exit(1 if bad else 0)
PY
```

| Case | Expected | Fails before |
|---|---|---|
| census of the owned files | `chrome_sites=0` | yes (39 sites) |
| `theme::tests::*` (B's updated invariants, incl. `menu_pills_inset_by_the_frame_inset`) | pass | per B |
| `panels::rail::tests::*` from #1106 (`cells_follow_the_measured_rail_layout`, `rail_paints_icons_only_without_label_text`, `rail_shows_tooltip_on_hover_and_on_keyboard_focus`, `rail_buttons_preserve_accesskit_names`, `rail_width_and_active_tile_and_divider`, `rail.rs:241-389`) | pass unchanged | guard |
| `panels::toolbar::tests::*` (`toolbar_is_centered_and_above_bottom`, `toolbelt_is_512_wide_like_figma`, `contextual_floating_bar_buttons_mutate_selection_and_tool_state`, `a_press_on_a_floating_panel_over_the_chevron_is_not_a_chevron_press`) | pass; the toolbelt stays 512 wide | guard |
| `widgets::menu::tests::a_menu_taller_than_the_screen_is_clamped_to_the_screen_height` | pass with the 32 px row | guard |
| new `tabs::tests::tab_bar_paints_only_theme_roles` (new name) | paints the bar into a test `Context` and asserts every fill and stroke colour equals a constant listed in section 3 | yes |
| new `tabs::tests::tab_close_hit_box_meets_the_target_floor` (new name) | the recorded `tab-close-hit-<id>` rect is at least `TARGET_MIN` on both axes, shares its centre with `tab-close-<id>`, and the tab body's hit rect ends left of it | yes (18) |
| new `panels::toolbar::tests::chevron_hit_box_meets_the_target_floor` (new name) | each `tool-chevron-hit-<gi>` rect is at least `TARGET_MIN` wide and does not overlap the next tool's rect, and `tool-chevron-<gi>` stays 16 wide; the toolbelt width is unchanged | yes (16) |
| new `panels::toolbar::tests::dev_toggle_label_is_dark_on_the_dev_fill` (new name) | the toggle glyph colour is `TEXT_ON_BRAND` | yes (white) |
| new `canvas::rulers::tests::guide_badge_label_is_dark_on_the_guide_fill` (new name) | the badge label colour is `TEXT_ON_BRAND` | yes (white) |
| new `panels::rail::tests::help_button_paints_no_status_dot` (new name; the shape-walking harness at `rail.rs:210-239` plus a circle collector) | no filled circle is painted inside the `rail-help` rect (R10.5) | yes (the `#0D99FF` dot at `rail.rs:163`) |
| J's `toast_rules_for_d` (J section 10, lands with D) | toast fill `BG_PRIMARY_ALT`, border `BORDER_MENU`, radius `RADIUS_XL`, no shadow call in the toast; both actions (`toast-reveal`, `toast-enter-path`) paint `Button/Tertiary` xs (label `TEXT_SECONDARY`, hover fill `BG_SECONDARY_HOVER`) and keep a 32 px target | yes |
| new `shell::tests::left_panel_resize_hit_band_meets_the_target_floor` (new name) | the recorded `left-panel-resize-band` rect is `DRAG_DIVIDER_HIT` wide and centred on the panel edge (`left-panel-resize-handle` stays the 6 px core); a press 10 px inside the edge over the layers scrollbar track scrolls (the scrollbar keeps its track); a press 8 px right of the edge with rulers off resizes; the same press with rulers on starts a guide (the ruler keeps [R + 3, R + 20)) | yes (6) |
| new `canvas::rulers::tests::ruler_press_band_meets_the_target_floor` (new name) | a press 22 px from the canvas edge on the top and on the left ruler starts a guide; a press 25 px from the edge does not; an existing guide within 4 px of its line is grabbed instead in the 4 px band | yes (20) |
| new `panels::menus::tests::palette_is_394_tall_with_the_40_px_input_row` (new name) | the palette frame height equals 40 + 1 + 34 + 1 + 22 + 8 × 36 + 8 | yes (296) |

Run: `cargo test -p designos-app --bin designos theme::`, `cargo test -p designos-app --bin designos panels::toolbar`, `cargo test -p designos-app --bin designos panels::rail`, `cargo test -p designos-app --bin designos widgets::menu`, `cargo test -p designos-app --bin designos tabs::`, `cargo test -p designos-app --bin designos canvas::rulers`, `python3 scripts/check_spacing_relationships.py`, `python3 scripts/zero_fake_ui_gate.py`, `python3 scripts/design_os_self_evolve.py`, `cargo fmt --all --check`, `cargo clippy -p designos-app --all-targets -- -D warnings`.

**Census output at `8dfaee93`** (the section 2 command, run from the root of `git archive 8dfaee93 crates/designos-app/src scripts/rust_literal_scan.py`, 2026-10-28): see the end of this section; exit 0.

**Evidence commands.** `KIT` is a scratch copy of `~/Downloads/❖ Untitled UI Figma – PRO VARIABLES (v8.0) gKXp7LqFY2NV (Copy).fig` (never the original); `OUT` is a scratch folder; each kit run takes about 50 s plus build-lock waits. `FIX` is spec E's small fixture (E section 10 builds it with `--apply-spec`, exit 0). Run the same list after the change and put both sets in the PR.

```bash
cd "$(git rev-parse --show-toplevel)"   # repo root: cargo run -p needs it
export PATH="$HOME/.cargo/bin:$PATH"; S="$(mktemp -d)"; mkdir -p "$S/home" "$S/lib" "$S/shots"; OUT="$S/shots"
KIT="$S/untitled-ui-kit-copy.fig"; cp "$HOME/Downloads/❖ Untitled UI Figma – PRO VARIABLES (v8.0) gKXp7LqFY2NV (Copy).fig" "$KIT"   # a copy, never the original
RUN() { env HOME="$S/home" TMPDIR="$S/" CARGO_HOME="$HOME/.cargo" RUSTUP_HOME="$HOME/.rustup" cargo run -q --release -p designos-app --bin designos -- "$@"; echo "exit $?"; }
RUN "$KIT" --screenshot "$OUT/D-01-editor-default.png" --size 1600x1088 --no-toast                       # tab bar, rail, toolbar, rulers
RUN "$KIT" --screenshot "$OUT/D-07-select-frame.png" --size 1600x1088 --no-toast --select "Welcome to Untitled UI"   # selected state, ruler band
RUN "$KIT" --screenshot "$OUT/D-02-menu-main.png" --size 1600x1088 --no-toast --menu main             # main menu
RUN "$KIT" --screenshot "$OUT/D-03-menu-main-file.png" --size 1600x1088 --no-toast --menu main/File   # File submenu
RUN "$KIT" --screenshot "$OUT/D-04-menu-shape.png" --size 1600x1088 --no-toast --menu shape           # tool-group menu
RUN "$KIT" --screenshot "$OUT/D-05-menu-palette.png" --size 1600x1088 --no-toast --menu palette       # Actions palette
RUN "$KIT" --screenshot "$OUT/D-06-menu-zoom.png" --size 1600x1088 --no-toast --menu zoom             # zoom menu (preset arm menus.rs:629-632, not listed in --help)
RUN "$FIX" --screenshot "$OUT/D-06b-menu-zoom-fixture.png" --size 1600x1088 --no-toast --menu zoom    # fallback for D-06 on E's fixture
RUN --home --library "$S/lib" --screenshot "$OUT/D-08-home-tab-bar.png" --size 1600x1088 --no-toast   # tab bar with Home active
RUN "$KIT" --screenshot "$OUT/D-09-dev-css.png" --size 1600x1088 --no-toast --dev css --select "Welcome to Untitled UI"  # Dev toolbar, Dev badge
```

Recorded exit codes, with the ref and machine state of each run:

| Capture | Exit | Ref and conditions |
|---|---|---|
| D-01, D-02, D-03, D-04, D-05, D-07, D-08 | 0 | spec writer, local build of `ec47da26`, 2026-10-25, many concurrent cargo builds from other agents; D-01 predates #1106 (rail labels visible) |
| D-06 | 1 twice ("designos: screenshot was not captured") | spec writer, same build and load (0.6.x; re-run on v0.7.0) |
| D-09 | 1 twice (same message) | spec writer, same build and load |
| D-06b | 0, zoom menu captured | reviewer, local build of `ec47da26`, 2026-10-25; so the D-06 failure looks kit- or load-specific (INFERENCE) |
| D-08 | 0 | reviewer rerun, same build |

No capture was rerun at `8dfaee93`; the recorded runs are from 0.6.x builds. v0.7.0 changed `--select` (it takes `id:`, `fig:`, `name:` or a layer name and exits 2 on a miss) and the screenshot exit paths, so the implementer reruns the list on v0.7.0 (`tools/designos-latest`, an idle machine) and records the exit codes in the PR; D-06 and D-09 are the two that failed with exit 1 in the earlier runs, and D-06b is the fallback for the zoom menu.

Not capturable from the CLI today: the title bar (hidden in screenshot mode), hover, pressed and focus states, the contextual floating bar (hidden while a toast is pending, `toolbar.rs:143-150`), the tab close states and the HUD. The PR shows these with a manual window capture, labelled manual; the HUD with a `--panel micro-*:seq` run.

Census output pasted from the run at `8dfaee93` (exit 0):

```text
tabs.rs theme_lines=26 chrome_sites=8 [29, 30, 31, 1046, 1049, 1078, 1081, 1134] computed=[1051]
shell.rs theme_lines=55 chrome_sites=12 [1541, 1542, 3594, 3621, 3639, 3641, 3654, 3655, 3661, 3667] computed=[]
panels/rail.rs theme_lines=22 chrome_sites=1 [163] computed=[]
panels/toolbar.rs theme_lines=100 chrome_sites=13 [136, 157, 223, 268, 302, 311, 342, 364, 365, 422, 465, 466] computed=[]
panels/menus.rs theme_lines=43 chrome_sites=4 [962, 963, 964, 1010] computed=[]
canvas/rulers.rs theme_lines=24 chrome_sites=1 [422] computed=[]
exit 0
```

## 11. Performance budgets

- No new allocation in the per-frame paint path: constants are `const Color32`; the kind badge tile is a constant instead of a per-tab computed tint.
- No `request_repaint` added; idle CPU stays 0 % (CLAUDE.md section 4). No change touches `doc_hash`, `TileView::rev` or the GPU canvas.
- Removing the shadow draw calls (toolbar, contextual bar, Re-center, toast, HUD) lowers tessellation per frame.
- Text at 12 px instead of 11 px adds no measurable cost (same galley calls).

## 12. Dependencies and build order

- **Build order.** A (constants in `theme.rs`, C-1, the colour-literal gate) and B (scale) land first; C's widgets next; L's baseline captures before the first surface PR. Then this spec: role swap per file (tabs, title bar, rail, toolbar, menus and palette, rulers, toast, HUD), then R5 type and heights and the two hit boxes, then evidence.
- **#1106 (icon-only rail) is merged** (`b598a499`, inside `205c72ef`): rail 48 wide, no labels, tooltips on hover and focus. L's baseline is taken at or after it. D restyles the icon-only rail and must not reintroduce labels.
- **#1108 (Pages/Layers separator) is merged** (`07b961c8`, after `205c72ef`). It edits `panels/left.rs` and adds `panels/left_sep.rs` (E's, plan.md 22:55) and `theme.rs`; no D file. Its drag highlight uses the same `BG_BRAND` role as the shell's resize handle, and its hit band follows the same divider rule (rule 3, E rule 5).
- **Spec L edits `shell.rs` first** (region rects `:1600-1615` recorded into the app state, screenshot write `:545-560` also writing `--layout-json`). L lands before D, so D rebases on it.
- **J.** Toast rules 12–15 (J is corrected: hover fill `BG_SECONDARY_HOVER`, radius `RADIUS_XL`) and command bar rule 2 (D's palette follows it: input row 40, frame radius `RADIUS_XL` 12). J's `toast_rules_for_d` test lands with D and covers both toast actions.
- **G.** The Dev accent role and its hover (G rule 2); the value is `DEV_READY` `#14AE5C` (R10.4).
- **Sent to C** (exceptions, not owner questions): the 26 px file tab inside the 32 bar and the 24 px Home and `+` (kept by R10.1); the 18 px visual close mark with a 24 px hit box; pressed equals hover for `Button/Close` on a `#262626` surface; the `Tabs/Segmented` Selected tile without C's 1 px `#404040` border (the fill already matches, contract row 18 and R8.2); R8.5's overrides of `Menu/Panel` and `Menu/Item` radii; the chevron form of `Button/Tool`.
- **Sent to A (dead code):** nothing in D orphans a constant. **Sent to A (new names):** `HUD_POINTER_FILL` (value `#111216`, unchanged: a cursor body, not chrome); `DEV_READY_HOVER` (G's name; D uses it for the Dev toggle hover). **D declares `DRAG_DIVIDER_HIT`** (`24.0`, `= TARGET_MIN`, new name) in `theme.rs` for the hit band of the resize strip and the ruler strips, and E uses it for the separator; neither A nor B adds it (token-contract.md, drag-divider ruling); the D-P rows for C-1 and `chrome_contrast_pairs`; the ruler guide badge label now dark (`TEXT_ON_BRAND`, 5.40:1) while the canvas guide labels (`canvas/draw.rs:1177`, `GUIDE_TEXT` white on the variable `gcol`) and size badges (`draw.rs:294`, `CANVAS_SIZE_BADGE_TEXT` on `col`) stay white under contract rows 60 and 65 (D did not measure their fills; one request to decide whether they follow); the ruler-numeral canvas exception (edge case 9).
- **Sent to G:** G §2 counts `tabs.rs:1134` as a Dev accent; it is the Update button dot (inside `if UPDATER_UI_ENABLED`), which D maps to `SUCCESS`.
- **Open PRs touching D files** (`gh pr list --state open --json files`, 2026-10-28): `shell.rs` in #1318, #1312, #1206 (L's layout gate, `jangtrinh/agy-1126-layout-gate`), #1198 (also `tabs.rs`), #1145 and #1081; stale #770, #768, #765 (`panels/menus.rs` among others). Rebase on whichever merges first; L's #1206 lands before D (section 12), and the stale three need a PM decision before this spec starts.

## 13. Owner rulings that apply

- R1: adopt Untitled UI dark 100 %, keep layout and arrangement. R2: take Untitled spacing and typography too. R3: no Figma; evidence in the PR. R4: Studio orange `#E36323`, dark label `#0A0A0A` (5.73:1), focus ring Orange-500 `#F97316`, Purple Ban. R5: small end of the scale in editor chrome (text-xs 12/18 rows and labels, text-sm 14/20 titles, button xs 32, row 32, radius-md 8). R6: keep the icon set, restyle colour to fg roles. R7: spec-driven.
- R10.1 editor tab heights stay as today (file tabs 26 in the 32 bar, Home and `+` 24); R10.4 Dev accent `DEV_READY` `#14AE5C` with the dark label (6.82:1); R10.5 the rail help dot is removed.
- R8.2 selected fill `#262626`; R8.3 accent text `#F97316` (toast info dot and reveal action); R8.5 floating chrome 16, items 8 at inset 8 (menus); R8.6 button labels text-sm 14 Semibold, tab and row text 12 (Re-center, "More", Update); R8.1 placeholder `#737373` (palette search). R8.7 (destructive hover `#B91C1C`) does not apply: D has no destructive button, and the tab close × is neutral (`Button/Close`). R9: figma-agent read-only is allowed; not used in this spec.
- Controller defaults: canvas overlays keep Figma-parity colours; chrome status colours move to Untitled. 21:35 sweep: command bar item pitch 36 (palette); floating chrome 16 (toast); no new film in this pack.
- Plan.md C-R (2026-10-25, 22:55): command bar and toast radius 12 `RADIUS_XL`; J owns toast and command-bar anatomy and D applies it; the toast action hover is `BG_SECONDARY_HOVER`; `PANEL_ACTION_BUTTON` stays 24; drag dividers (resize strip, ruler strip) keep their visible line and get a 24 px hit band, no new exception.
- 2026-09-22: visual evidence in every PR. Standing: `main` is protected (PR, green macOS test, clippy, rustfmt, squash). 2026-10-10: responsive checks only on request.

## Decisions recorded by the spec writer (technical; owner may reverse)

1. The title-bar wordmark takes text-sm 14/20, like a panel title (R5), not text-xs: it names the app and is the only text in its bar.
2. Chrome blue (`#0D99FF`) leaves the chrome: the Actions-open fill, the selected-mode icon and the unsaved dot; the rail help dot is removed (R10.5). The ruler selection band stays blue because it mirrors the canvas selection.
3. File-kind badges on tabs use the same kit `Featured icon` tiles that R10.2 gives the file kinds on Home (design blue, board amber, deck green, spec orange; contract N39–N46), so a file shows one colour everywhere; deck and spec, both orange today, split. The misnamed `BADGE_BOARD_PURPLE` and the computed tint go away.
4. The contextual bar, the Re-center pill and the "More" and Update labels follow R5 and R8.6; they float, so no region changes.
5. The Actions palette stays in `panels/menus.rs` (J §1 agrees) and is composed exactly like J's command bar, so both palettes look the same.
6. The ruler guide badge label goes dark (`TEXT_ON_BRAND`): white fails AC6 (3.67:1) and the soul puts a dark label on an accent fill.
7. The tab close mark and the chevrons keep their visual size and get 24 px hit boxes (the P3 pointer floor is a law, not a question); neither change moves a region.
8. The HUD pointer and its accent keep their canvas values; the HUD's pill and caption bar take chrome constants.
9. The toast is J's anatomy and D's paint (C-R): radius 12, `BG_PRIMARY_ALT`, no shadow, both actions as `Button/Tertiary` xs. `shell/toast_layout.rs` joins D's files because D paints what it sizes.
10. Dividers share one priority rule with E: the 6 px core keeps winning, the band's extension yields to scrollbars, controls and rulers, and wins over canvas and list rows. This reads C-R's "24 px hit area centred on the line" without taking a pixel from a scrollbar or a ruler; the cost is a free width of 9 px (rulers on) or 18 px (rulers off), stated in the table.
11. The Actions palette's input row is 40 (J rule 2) and its frame radius 12; the old "stays 38" and "not settled" notes are gone.

## Open questions for the owner

None. The board 3 questions are decided (rail help dot removed, R10.5; editor tab heights stay, R10.1), and the 22:55 review questions are ruled in plan.md C-R (toast and palette radius, divider hit areas).

## Changelog (fix round 2026-10-25, review `reviews/DE-curator-review.md`)

| Review item | Resolution |
|---|---|
| D1 #1106 merged, rail content stale | Fixed. Re-anchored to `205c72ef`; rail 48, icon-only, tooltips as `Tooltip/Default`; rail-label rows, the old edge case 8.2 and the old open question 2 deleted; §12 says merged. |
| D2 role map vs token contract | Fixed. Bars `BG_PRIMARY`/`BG_TOOLBAR`/`BG_MENU` `#0A0A0A`; hover `BG_HOVER`/`BG_TOOLBAR_HOVER` `#171717`; selected `BG_SELECTED` `#262626`; rulers unchanged (rows 80–83). |
| D3 states and widget names | Fixed. C names throughout (`Tabs/Pill`, `Button/Close`, `Button/Tool`, `Button/Icon`, `Tabs/Segmented`, `Menu/*`, `Dialog/CommandPalette`, `Toast/Default`, `Tooltip/Default`); states from C §3.4; the tab close hover is neutral (no red); `TEXT_ON_BRAND` and the Dev accent (`DEV_READY`, R10.4); exceptions sent to C (§12). |
| D4 AC6 fails on the guide badge | Fixed. Badge label `TEXT_ON_BRAND` (5.40:1); AC6 rewritten (C-1 plus D-P rows, exit 0). |
| D5 11 unmapped sites | Fixed. HUD rows (pointer and accent keep canvas values; pill, bar and keycap take `BG_PRIMARY`, `BORDER`, `TEXT`, `TEXT_ON_BRAND`); Update dot `SUCCESS`; the micro-demo document builders are named content; G told about `tabs.rs:1134`. |
| D6 hit areas below 24 | Fixed. Close 24×24 and chevron 24×32 hit boxes with red tests. The review's owner question 1 (chevron) is answered as law per the coordinator: not an owner question, and no width changes because the hit box reaches into the existing gap. |
| D7 rail help dot | Decided by R10.5 (board 3, 21:55): the dot is removed; rule 2 says so and a red test guards it. |
| D8a zoom arm | Fixed: `menus.rs:623-626`. |
| D8b rail terminal and dock mentions | Removed. |
| D8c evidence prose | Fixed: a table gives the ref and machine state of every run. |
| D8d D-06 fallback | Added: D-06b on E's fixture. |
| D8e badge colours | Superseded by W1's contract rows N39–N46 (R10.2): the tab badges use those tiles and glyphs (C-1.31–C-1.34); no new name is left. |
| D8f proportional ruler numerals | Recorded as a canvas exception (edge case 9), sent to A. |
| D8g toast | D implements the toast with J's rules 12–15 on C `Toast/Default`; removed from the non-goals. |
| D8h `es-designer` | §7 points at the in-repo docs. |
| Both: AC6 accepted `ui ds a11y` | Fixed: C-1 (contract (f)) and the D-P rows; `ui ds a11y` is named as not accepted. |
| Both: R4 5.76 vs 5.73 | Fixed upstream by W1 (contract N10 says 5.73); D uses 5.73. |
| Both: palette ownership | J §1 now agrees: `cmdk.rs` is J's, the `menus.rs` palette is D's, and both follow J rule 2. |
| Both: shared surfaces of D's files | Listed in §5. |
| Coordinator: `updater.rs` belongs to J | Fixed in §5. |
| Coordinator: owner rulings | R8.2, R8.5, R8.6 applied; R8.7 recorded as not applicable; row and pill radius 8. |
| Coordinator: W1 additions | C-1 cited; N-names used (`ICON_WHITE` N17, `BG_PRIMARY_SOLID` N19, `RADIUS_XL` 12); tooltip text `#FAFAFA`; 5.73:1. |
| Coordinator: 21:35 sweep | Applied (palette pitch 36, toast radius 16). |
| Owner board 3 (R10.1, R10.4, R10.5) | Applied: tab heights stay 26/24 in the 32 bar (R10.1), the Dev accent is `DEV_READY` `#14AE5C` (R10.4), the rail help dot is removed (R10.5). D has no open owner question left. |
| figma-agent | Not run; no kit fact needed settling, and `design/` was not touched by this round. |

## Changelog (fix round 2026-10-28, review `reviews/DEFG-curator-review.md`, plan.md C-R)

| Review item | Resolution |
|---|---|
| D1 toast row contradicts J and C | Fixed. Radius `RADIUS_XL` 12, fill `BG_PRIMARY_ALT`, info dot `ICON_BRAND_PRIMARY`, actions as `Button/Tertiary` xs with label `TEXT_SECONDARY`; hover fill `BG_SECONDARY_HOVER` (C-R; J carries the same correction). AC6(a) no longer relies on C-1.11; D-P.40 to D-P.46 cover the toast. The invented success dot kept as J rule 13 has it (C adds it). |
| D2 false claim that B and E set the shared 24 px panel-button constant to 32 | Fixed. The show-sidebar button is sized with the contract's button xs step (32) directly; the shared constant (`PANEL_ACTION_BUTTON`) stays 24 (B rule 6). |
| D3 6 px resize strip unruled | Ruled by plan.md C-R: 24 px band, no exception. Rule 3's divider table gives the priorities, the arithmetic and the free widths (9 px rulers on, 18 px off); edge case 10 rewritten; two red tests; `DRAG_DIVIDER_HIT` requested. The ruler strips join (20 → 24). |
| D4 palette not like J rule 2 | Fixed. Input row 40, frame radius 12, height 296 → 394 (+98). |
| D5 ownership list stale | Fixed from plan.md and the H–K tables: no `home/updater.rs` (the file is `updater.rs`, J); `image_jobs.rs` I; `plugins/*`, `panels/check_designs.rs` K; `icons.rs` C; `left_sep.rs` E. |
| D6 `Tabs/Segmented` note from an old C | Fixed: only the border differs. |
| D7 L allow numbers | Fixed. The "L allow entries D needs" table gives each entry with its arithmetic; AC3 points at it. The contextual bar is +31 wide (159), not the +22 of the earlier estimate: the 14 px Semibold "More" label and its chevron take 75 px. |
| D8 Dev toggle hover unnamed | Fixed: `DEV_READY_HOVER` (G rule 2). |
| Re-anchor to `8dfaee93` | 77 of 232 cites moved and were verified line for line; drift note above; the toast gained a second action. |
