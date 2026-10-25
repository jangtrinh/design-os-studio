# J — Surface: command bar, dialogs, toasts, context menu, Find, comments, history and presentation (Untitled UI dark)

| Field | Value |
|---|---|
| Issue | #1146 (epic #1118, spec key J) |
| Size | M–L (about 2 days: 6 files with 1–11 chrome colour sites, 14 files that re-point sizes only, toast rules for D) |
| Depends on | A, B, C and L |
| Unblocks | D (toast rules in section 3, painted in `shell.rs`), M (docs and screenshot refresh) |
| Touches | `crates/designos-app/src/cmdk.rs` (palette constants `:258-262`, `show` `:265-558`, `render_audit_section` `:559-631`); `updater.rs` (constants `:38-45`, `show_sidebar_card_in_rect` `:569`, `show_update_modal` `:662`); `present.rs` (`show` `:399`, bars `:493`, `:500`, link dialog `:887-913`, reason pill `:770-779`); `panels/context.rs` (`show` `:145`, `show_layer_picker` `:169`); `home/dialog.rs` (`show` `:196`); `history/modal.rs`, `history/panel.rs`, `history/banner.rs`; `find/panel.rs`, `find/mod.rs`, `find/select.rs`; `comments/panel.rs`, `comments/thread.rs`, `comments/ui.rs`, `comments/pins.rs`; tests: new `crates/designos-app/src/shell/tests/overlays_chrome.rs` (new name) |

**Code baseline.** Every `file:line` is at `ec47da26` (local `main`, 2026-10-25). `origin/main` (`205c72ef`) has moved some of these files (`git diff --stat ec47da26 origin/main -- <files>`): `cmdk.rs` 333 lines changed, `home/dialog.rs` 88, `comments/settings.rs` 33, `present.rs` 19, `history/session.rs` 2; `theme.rs` gains `TOAST_REVEAL_SECONDS = 10.0` after `:463` (token contract header). Re-anchor every line on the branch HEAD before editing (search for the quoted code) and re-run the census; report drift in the PR. Colour and size names are the token contract's (`specs/token-contract.md`); Untitled role names are given beside them for review.

## 1. Summary and user story

As a designer, I want every floating surface — the Actions command bar (Cmd+K), confirm and picker dialogs, the context menu, toasts, Find and Replace, comment threads, version history and the presentation player's own controls — to look like Untitled UI's dark kit, so pop-ups stop mixing Figma UI3 greys (`#2C2C2C`, `#383838`), Figma blue and white-on-orange labels with the rest of the app.

Positions, widths and order stay (R1). Colours come from the token contract; anatomy from spec C (`Dialog/Modal`, `Dialog/CommandPalette`, `Toast/Default`, `Menu/Context`, `Menu/Item`, `Popover/Panel`, `Input/Search`, `Tabs/Segmented`, `Button/*`, `Tooltip/Default`); sizes from R5 (editor surfaces: text-xs rows, button xs 32, row 32; dialogs: text-sm 14/20, buttons sm 36 / md 40).

**Agreement with spec D.** D owns `panels/menus.rs`, including the menu command palette (`panels/menus.rs:879-1057`, captured with `--menu palette`). J agrees and does not edit that file. J owns the separate Actions command bar in `cmdk.rs` (opened with Cmd+K, docked above the toolbar, `J-01`). Both use C's `Dialog/CommandPalette`, so they look the same. D also owns `shell.rs`; J writes the toast rules (section 3, rules 12–15) and D paints them at `shell.rs:989-1016`.

## 2. Current state

FACT unless labelled. Screenshots of today in section 10 (`J-01`..`J-05`).

| Surface | Code | Today |
|---|---|---|
| Actions command bar | `cmdk.rs:265` `show`; private palette `:258-262` (`CMDK_BG_SURFACE #2C2C2C`, `CMDK_BG_ROW_HOVER #383838`, `CMDK_BORDER #383838`, `CMDK_BADGE_BG #383838`, `CMDK_BADGE_TEXT #B3B3B3`); width `min(440, screen − 32)` `:272`; height `40 + 34 + sections × 28 + rows × 32 + audit + 10` `:333`; input row 40 `:349` (input `:347-375`); tab row 34 with a 2 px `Color32::WHITE` underline `:385-416`; rows 30 high at a 32 pitch, radius 6 `:438-464`; badge 18 `:459`; selected icon and title `Color32::WHITE` `:454`, `:464`; tab label `WHITE` `:406`; Design Gate audit section `:559-631` reads I's terminal palette (`cmdk.rs:12-14`, used `:571-614`) | Figma UI3 greys |
| Updater card and modal | `updater.rs` UI3 palette `:38-45` (`UI3_BG_PANEL #1E1E1E`, `UI3_BG_CARD #2C2C2C`, `UI3_BG_HOVER #343434`, `UI3_BORDER #383838`, `UI3_BLUE #0D99FF`, `UI3_EMERALD #1BC47D`, `UI3_AMBER #F59E0B`, `UI3_RED #F24822`); card `:569` returns at once while `UPDATER_UI_ENABLED` is false (`:570-572`, gate Z14 keeps it hidden); modal `:662`, scrim black α150 `:677`, channel pill label `WHITE` `:730`, "Stage Atomic Update" 154×26 on `UI3_BLUE` with `WHITE` label `:827-831` | hidden; not capturable |
| Presentation chrome | `present.rs:399` `show`; stage `PRESENT_BG #09090B` (`theme.rs:596`), bar `PRESENT_BAR_HEIGHT` 48; hairlines white α18 `:493`, `:500`; device bezel `Color32::BLACK` `:687`; hotspot reason pill 26 high, black α190 + `WHITE` text `:775-779`; "Open this link?" dialog `:887-913` (shadow `:893`, fill `BG_MENU`, Open 64×24 on `BG_BRAND` with `WHITE` label `:898`, `:903-904`, Cancel 60×24 `:899`); dead `_icons_used` `:913`; texture tints `Color32::WHITE` `:263`, `:883`; computed document colours `:650`, `:655`, `:751` | — |
| Context menu | `panels/context.rs:145` `show` (rows through C's `widgets::menu`), entries `:65`, layer picker `:169` (hover text `Color32::WHITE` `:220`, `:227`) | rows 24 (`MENU_ITEM_HEIGHT` = `CONTROL_DENSE`) |
| Confirm, picker and about dialogs | `home/dialog.rs:196` `show` (W 440, PAD 20, BTN_H 32, ROW_H 28 `:60-63`); measuring colour `WHITE` `:207`; scrim black α140 `:225`; OK label `WHITE` `:332`; called from `shell.rs:1274`, `:1544` and Home | dialog |
| Version-history modal | `history/modal.rs:16` (W 440, PAD 20, FIELD_H 32, DESC_H 88, BTN_H 32, LABEL_H 18 `:9-14`); scrim black α140 `:32`; called from `shell.rs:1545` | dialog |
| Version-history panel | `history/panel.rs:22` (replaces the right panel, `panels/right.rs:14`; rows `HISTORY_ROW_H` 44, day headers `HISTORY_DAY_H` 28; `:199`, `:208`) | — |
| Version-preview banner | `history/banner.rs:9` (`HISTORY_BANNER_H` 40; called `shell.rs:1528`) | — |
| Find and Replace | `find/panel.rs:179` `show` (in the left panel, `panels/left.rs:145-148`); field `:158-177`; constants `FIND_FIELD_H` = `CONTROL_STANDARD` 28, `FIND_TOGGLE_SIZE` 22, `FIND_BUTTON_H` 24, `FIND_STATUS_H` 32, `FIND_GROUP_ROW_H` 28, `FIND_HIGHLIGHT`, `FIND_HIGHLIGHT_SELECTED` (`theme.rs:306-316`); tests `:432` | — |
| Comments panel | `comments/panel.rs:19` (replaces the right panel, `panels/right.rs:18`; SIDE 12, AVATAR 20, SEARCH_H 28, FOOTER_H 40 `:13-17`; `COMMENT_PANEL_HEADER_H` 48, `COMMENT_FILTER_H` 24, `COMMENT_CARD_PAD` 12) | — |
| Comment thread popover | `comments/thread.rs:38` `show` (`COMMENT_POP_W` 300, `COMMENT_HEADER_H` 36, `COMMENT_BTN_H` 24, `COMMENT_ICON_BTN` 20, `COMMENT_AVATAR` 24, box 56–160; composer W 280 `:13`); `:52`, `:120`, `:284` | — |
| Comment widgets | `comments/ui.rs:57`, buttons `:92-108`, dots button `:117-131`, segmented `:133-152` (C lists all three as duplicates D–K retire) | — |
| Comment pins and avatars | `comments/pins.rs`; `COMMENT_PIN` 32, `COMMENT_PIN_RESOLVED #757575`, `AVATAR_COLORS` (`theme.rs:466-472`) | canvas overlay + identity colours |
| Toast | `shell.rs:989-1016` (D's file): `TOAST_HEIGHT` 40, `TOAST_BOTTOM` 72, `TOAST_SECONDS` 4 (`theme.rs:461-463`), elevation shadow `:1005`, status dot `:1007`; raised by `app.rs:313` `show_toast` | — |

**Chrome colour census** (spec D's command, unchanged across D–K):

```bash
cd /Users/jangtrinh/Products/design-os-studio
PYTHONDONTWRITEBYTECODE=1 python3 - cmdk.rs updater.rs present.rs panels/context.rs home/dialog.rs \
  history/modal.rs history/panel.rs history/banner.rs history/mod.rs history/session.rs history/preset.rs \
  find/mod.rs find/panel.rs find/select.rs comments/mod.rs comments/panel.rs comments/thread.rs comments/pins.rs \
  comments/ui.rs comments/settings.rs comments/model.rs comments/import.rs comments/time.rs <<'PY'
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

Result at `ec47da26` (run 2026-10-25, exit 0):

| File | `theme::` lines | Chrome sites | Lines | Computed |
|---|---|---|---|---|
| `cmdk.rs` | 24 | 9 | `:258-262`, `:406`, `:415`, `:454`, `:464` | — |
| `updater.rs` | 40 | 11 | `:38-45`, `:677`, `:730`, `:831` | — |
| `present.rs` | 53 | 9 | `:263`, `:493`, `:500`, `:687`, `:777`, `:779`, `:883`, `:904`, `:913` | `:650`, `:655`, `:751` (document colours) |
| `panels/context.rs` | 13 | 2 | `:220`, `:227` | — |
| `home/dialog.rs` | 22 | 3 | `:207`, `:225`, `:332` | — |
| `history/modal.rs` | 8 | 1 | `:32` | — |
| `history/panel.rs`, `history/banner.rs` | 32, 9 | 0 | — | — |
| `find/panel.rs` | 76 | 0 | — | — |
| `comments/panel.rs`, `thread.rs`, `ui.rs`, `pins.rs` | 40, 39, 32, 1 | 0 | — | — |
| the other 9 files | 0–1 | 0 | — | — |

Total today: **35** chrome sites in J's files. (plan.md's `grep -c 'Color32::from_rgb('` gives cmdk 5, updater 8, present 3; the census above is the contract.)

**Gates today.** No gate counts colour literals; `.github/workflows/design-gates.yml` is advisory (`continue-on-error`, `:4-5`, `:21`). `scripts/zero_fake_ui_gate.py` checks Z08 (`panels/toolbar.rs` + `cmdk.rs`: contextual actions mutate state; Remove background stays removed), Z14 (updater UI stays hidden while `UPDATER_UI_ENABLED = false`) and Z15 (`terminal.rs` + `cmdk.rs`: no fake template synthesis; `synthesize_spec_from_prompt` stays private to `cmdk.rs`). `scripts/check_spacing_relationships.py` passes at `ec47da26` (exit 0).

## 3. Behaviour rules

1. **Layout unchanged (R1).** Every surface keeps its anchor, width and order: command bar 440 wide docked above the toolbar; dialogs 440 wide centred; thread popover 300 wide beside its pin; Find in the left panel; comments and history in the right panel's place; presentation bar 48. Heights change only where R5 raises a control or row (table 3).
2. **Command bar** (`cmdk.rs`) is C's `Dialog/CommandPalette`: frame `BG_MENU` + `BORDER_MENU`, no shadow; input row stays 40 (region) with C's borderless `Input/Search` inside; tabs are Untitled underline tabs (current label `TEXT`, others `TEXT_QUATERNARY`, underline `BORDER_SELECTED` 2 px instead of white); section headings `TEXT_TERTIARY` caption; rows are `_Command dropdown menu item` at C's R5 pitch 36 (row 32 with 2 px inset), hover `BG_MENU_HOVER`, selected `BG_SELECTED`, icon `ICON_TERTIARY` (selected `ICON`), title `TEXT_SECONDARY` (selected `TEXT`); the right-side tag ("AI", "Development") is C's `Badge/StatusCount` gray. The Design Gate audit section reads I's terminal palette aliases (spec I rule 3) and needs no edit beyond its own literals (none).
3. **Dialogs** (`home/dialog.rs`, `history/modal.rs`, updater modal, presentation link dialog) are C's `Dialog/Modal`: width unchanged, padding 24 (`INSET_MODAL`), title text-md semibold `TEXT`, body text-sm `TEXT_TERTIARY`, footer buttons `CONTROL_MD` 40 (Primary + Secondary, or Destructive for danger dialogs), fields `CONTROL_SM` 36 (`Input/Field` sm), picker rows C's `List/Row` sm 36. The scrim is drawn by C's `kit::surface::modal_surface` (contract gap G-3); J's files keep no scrim literal.
4. **Brand and destructive labels.** A label on `BG_BRAND` / `BG_BRAND_HOVER` / `BG_BRAND_PRESSED` is `TEXT_ON_BRAND` (R4 dark label); a label on `BG_ERROR_SOLID` is `TEXT_WHITE`. No `Color32::WHITE` label remains (`home/dialog.rs:332`, `present.rs:904`, `updater.rs:730`, `:831`, `cmdk.rs:406`, `:454`, `:464`).
5. **Context menu** (`panels/context.rs`) uses C's `Menu/Context` and `Menu/Item`: rows `MENU_ITEM_HEIGHT` 32 (contract (c)), hover `BG_MENU_HOVER` with label `TEXT_SECONDARY_HOVER`, shortcuts `TEXT_QUATERNARY` (C's `Menu/Shortcut`), separators `BORDER_MENU`. The layer picker (`:169`) paints its rows with the same `Menu/Item` look instead of `Color32::WHITE` hover text.
6. **Find and Replace** (`find/panel.rs`): fields are C's `Input/Search` / `Input/Field` at `FIND_FIELD_H` (contract: `CONTROL_XS` 32); option toggles keep `FIND_TOGGLE_SIZE` 22 inside the 32 field (C's `Button/Icon` look, selected = `BG_SELECTED`); Replace and Replace all are `Button/Secondary` xs 32 (J re-points its call sites from `FIND_BUTTON_H` 24 to `CONTROL_XS`); result group rows `FIND_GROUP_ROW_H` (contract: 32); match highlight keeps `FIND_HIGHLIGHT` / `FIND_HIGHLIGHT_SELECTED` (contract (e): Studio-only info family).
7. **Comments.** Panel header 48 and footer 40 stay (regions); filter is C's `Tabs/Segmented` xs 32 (J's call sites use `CONTROL_XS` instead of `COMMENT_FILTER_H` 24); search is `Input/Search` xs 32 (local `SEARCH_H` 28 → `CONTROL_XS`); cards `BG_SECONDARY` + `BORDER`, hover `BG_SECONDARY_HOVER`. Thread popover: C's `Popover/Panel` (header 36 stays), action buttons `Button/*` xs 32 (call sites use `CONTROL_XS` instead of `COMMENT_BTN_H` 24), icon buttons `Button/Icon` xs (hit 32 inside the 36 header, glyph unchanged), composer `Input/Field` (textarea) with `BORDER_STRONG`, focus `FOCUS_RING`. Avatars keep `AVATAR_COLORS` and sizes (identity, contract (e)); pins keep `COMMENT_PIN_RESOLVED` (canvas overlay).
8. **Version history.** Panel rows stay 44 (two-line rows; R5's 32 would shrink them, R1 forbids) with hover `BG_HOVER`, selected `BG_SELECTED`, day headers 28 caption `TEXT_TERTIARY`; banner 40 stays (region) with `Button/*` xs 32 inside; modal per rule 3.
9. **Presentation chrome.** Stage `PRESENT_BG` (contract: `#0A0A0A`); bar and footer hairlines `BORDER`; the hotspot reason pill uses C's `Tooltip/Default` look (26 high stays); the link dialog is a dialog (rule 3) with buttons `CONTROL_SM` 36 and no shadow (contract: shadows resolve to alpha 0, so the `elevation_shadow` call at `:893` is removed). The device bezel (`:687`) and the texture tints (`:263`, `:883`) are content and stay (allowlist). The dead `_icons_used` (`:913`) is deleted.
10. **Updater** (hidden): restyle its code so it is ready if Z14 is ever lifted, without unhiding it: UI3 palette → contract constants (table 1); never change `UPDATER_UI_ENABLED`.
11. **Focus.** Every focusable control shows C's focus ring (`FOCUS_RING`) on keyboard focus; the command bar input and dialog fields show it when focused.

**Toast rules** (D paints them in `shell.rs:989-1016`; J owns the rules):

12. Anatomy: C's `Toast/Default` (Untitled `Notification · Type=No icon`, single line): height `TOAST_HEIGHT` 40, padding 0 / 16, gap 8, text text-sm medium `TEXT`.
13. Colours: fill `BG_SECONDARY` (kit bg-primary_alt `#171717`, same value), border `BORDER_MENU`, no shadow (remove the `elevation_shadow` call at `:1005`); status dot 8: info `TEXT_BRAND`, error `DANGER`, success `SUCCESS`.
14. Radius: `RADIUS_FRAME` (contract 16). C's anatomy says radius-xl 12; the contract has no 12 constant (conflict, Open question 3).
15. Timing and position unchanged: `TOAST_BOTTOM` 72, `TOAST_SECONDS` 4, and `TOAST_REVEAL_SECONDS` 10 where `origin/main` has it.

## 4. Data model / schema changes

None.

## 5. Contracts

- **Colour constants consumed** (token contract (a)): `BG`, `BG_PRIMARY`, `BG_SECONDARY`, `BG_SECONDARY_HOVER`, `BG_TERTIARY`, `BG_HOVER`, `BG_PRESSED`, `BG_MENU`, `BG_MENU_HOVER`, `BG_SELECTED`, `BG_BRAND`, `BG_BRAND_HOVER`, `BG_BRAND_PRESSED`, `BG_ERROR_SOLID`, `BORDER`, `BORDER_STRONG`, `BORDER_MENU`, `BORDER_SELECTED`, `TEXT`, `TEXT_SECONDARY`, `TEXT_SECONDARY_HOVER`, `TEXT_TERTIARY`, `TEXT_QUATERNARY`, `TEXT_PLACEHOLDER`, `TEXT_DISABLED`, `TEXT_ON_BRAND`, `TEXT_WHITE`, `TEXT_ERROR`, `TEXT_BRAND`, `ICON`, `ICON_SECONDARY`, `ICON_TERTIARY`, `ICON_QUATERNARY`, `DANGER`, `SUCCESS`, `WARNING`, `FOCUS_RING`, `PRESENT_BG`, `FIND_HIGHLIGHT`, `FIND_HIGHLIGHT_SELECTED`, `COMMENT_PIN_RESOLVED`, `AVATAR_COLORS`, `MENU_SHORTCUT_HIGHLIGHT`.
- **Contract gaps** (requested from spec A; shared with I and K): G-3 modal scrim colour (C's `modal_surface(+scrim)` draws it; the contract has no constant; today three different alphas 140/140/150 are used). G-4 radius-xl 12 for the command bar frame and toasts (C's anatomy; contract has `RADIUS_FRAME` 16 only): J follows the contract and lists it (Open question 3).
- **Size constants consumed** (contract (c)): `CONTROL_XS` 32, `CONTROL_SM` 36, `CONTROL_MD` 40, `BADGE_SM` 22, `MENU_ITEM_HEIGHT` 32, `FIND_FIELD_H` 32, `FIND_GROUP_ROW_H` 32, `RADIUS_CONTROL` 8, `RADIUS_FRAME` 16, `INSET_MODAL` 24, `FONT_SIZE_*` and `LINE_*` per B. Theme constants J stops reading (their values stay per the contract appendix; A/B may delete them when no reader is left): `FIND_BUTTON_H` 24, `COMMENT_BTN_H` 24, `COMMENT_FILTER_H` 24, `COMMENT_ICON_BTN` 20. `CONTROL_DENSE` / `CONTROL_STANDARD` sites in J's files: re-pointed to 0.
- **Widgets consumed** (spec C, `widgets/kit/`, new names there): `Dialog/CommandPalette`, `Dialog/Modal`, `Popover/Panel`, `Menu/Context`, `Menu/Item`, `Menu/Shortcut`, `Menu/Separator`, `Toast/Default` (for D), `Tooltip/Default`, `Input/Search`, `Input/Field`, `Tabs/Segmented`, `List/Row`, `Button/Primary|Secondary|Tertiary|Destructive|Icon|Close`, `Badge/StatusCount`. C names these J duplicates to retire: `comments/ui.rs:92`, `:117`, `:133`; `home/dialog.rs:229-232`, `:288-290`, `:316-345`; `cmdk.rs:347-375`, `:440-464`, `:458-461`; `find/panel.rs:158-177` (C section 5.1).
- **Rust surface.** `cmdk::show`, `home::dialog::show`, `history::*::show`, `find::panel::show`, `comments::*::show`, `present::show` keep their signatures. The private `CMDK_*` and `UI3_*` constants in `cmdk.rs` and `updater.rs` are deleted (0 uses outside their files).

## 6. File-format mapping

None.

## 7. UI

UI implementers load the `es-designer` skill first and complete its checklist; the project's design system and the token contract win over the skill's defaults. Evidence goes in the PR (not Figma, R3). Responsive: SKIPPED (owner default); capture at 1600×1088.

**Table 1: colours** (contract constant first; hex is the contract's dark value):

| Element | Today | Contract constant (Untitled role) | Hex |
|---|---|---|---|
| Command bar, dialog, popover, menu surface | `#2C2C2C` / `BG_MENU` | `BG_MENU` (bg-primary) | `#0A0A0A` |
| Their outline | `#383838` / `BORDER_MENU` | `BORDER_MENU` (border-secondary_alt) | `#262626` |
| Command bar row hover / selected | `#383838` | `BG_MENU_HOVER` / `BG_SELECTED` | `#171717` / `#262626` |
| Command bar tab underline | `Color32::WHITE` | `BORDER_SELECTED` (border-brand) | `#FB923C` |
| Command bar tab label current / other | `WHITE` / `TEXT_SECONDARY` | `TEXT` / `TEXT_QUATERNARY` | `#FAFAFA` / `#A3A3A3` |
| Command row title / selected | `TEXT_SECONDARY` / `WHITE` | `TEXT_SECONDARY` / `TEXT` | `#D4D4D4` / `#FAFAFA` |
| Command row icon / selected | `ICON_SECONDARY` / `WHITE` | `ICON_TERTIARY` / `ICON` | `#A3A3A3` / `#FFFFFF` |
| Command tag badge | `#383838` + `#B3B3B3` | gray `Badge/StatusCount`: `BG_SECONDARY` + `BORDER_STRONG` + `TEXT_SECONDARY` | `#171717` + `#404040` + `#D4D4D4` |
| Dialog title / body | `TEXT` / `TEXT_SECONDARY` | `TEXT` / `TEXT_TERTIARY` | `#FAFAFA` / `#A3A3A3` |
| Primary button | `BG_BRAND` + `WHITE`; updater `UI3_BLUE` + `WHITE` | `BG_BRAND` / `_HOVER` / `_PRESSED` + `TEXT_ON_BRAND` | `#E36323` / `#F97316` / `#EA580C` + `#0A0A0A` |
| Destructive button | `BG_BRAND`/`DANGER` variants (`home/dialog.rs:320-321`) | `BG_ERROR_SOLID` + `TEXT_WHITE` | `#DC2626` + white |
| Secondary button | `BG_TERTIARY` + `BORDER` | `BG` + `BORDER_STRONG`, label `TEXT_SECONDARY`; hover `BG_HOVER` + `TEXT_SECONDARY_HOVER` | `#0A0A0A` + `#404040` |
| Modal scrim | black α140 / α140 / α150 | C's `modal_surface` (G-3) | per C / A |
| Context-menu hover label | `Color32::WHITE` | `TEXT_SECONDARY_HOVER` | `#E5E5E5` |
| Menu shortcut | `TEXT_TERTIARY` / `MENU_SHORTCUT_HIGHLIGHT` on the lit row | `TEXT_QUATERNARY` / `MENU_SHORTCUT_HIGHLIGHT` | `#A3A3A3` / `#E5E5E5` |
| Find field / toggle selected | theme field look | `BG` + `BORDER_STRONG`, focus `FOCUS_RING`; toggle on `BG_SELECTED` | `#0A0A0A` + `#404040`; `#262626` |
| Find match highlight | `FIND_HIGHLIGHT`, `_SELECTED` | unchanged (contract (e)) | `#0D99FF` α 0x59, `#7CC4F8` α 0x66 |
| Comment card / hover | theme | `BG_SECONDARY` + `BORDER` / `BG_SECONDARY_HOVER` | `#171717` + `#262626` / `#262626` |
| Comment avatars, resolved pin | `AVATAR_COLORS`, `COMMENT_PIN_RESOLVED` | unchanged (identity, canvas overlay) | — |
| History row hover / selected | theme | `BG_HOVER` / `BG_SELECTED` | `#171717` / `#262626` |
| Presentation stage | `PRESENT_BG #09090B` | `PRESENT_BG` (contract: bg-primary) | `#0A0A0A` |
| Presentation hairlines | white α18 | `BORDER` | `#262626` |
| Hotspot reason pill | black α190 + `WHITE` | C's `Tooltip/Default` look | per C |
| Device bezel, texture tints | `BLACK`, `WHITE` tint | unchanged (content, allowlist) | — |
| Updater surfaces | `UI3_BG_PANEL/CARD/HOVER`, `UI3_BORDER` | `BG_MENU` / `BG_SECONDARY` / `BG_SECONDARY_HOVER`, `BORDER_MENU` | `#0A0A0A` / `#171717` / `#262626`, `#262626` |
| Updater status | `UI3_EMERALD`, `UI3_AMBER`, `UI3_RED`, `UI3_BLUE` | `SUCCESS`, `WARNING`, `DANGER`, `BG_BRAND` (actions) / `INFO` (info dot) | `#22C55E`, `#EAB308`, `#EF4444`, `#E36323` |
| Toast (D paints) | `BG_MENU` + elevation shadow, dot | `BG_SECONDARY` + `BORDER_MENU`, no shadow; dot `TEXT_BRAND` / `DANGER` / `SUCCESS` | `#171717` + `#262626`; `#F97316` / `#EF4444` / `#22C55E` |

**Table 2: type** (B's constants): editor surfaces (command bar rows per C use text-sm medium; Find, comments, history, context menu) rows and buttons `FONT_SIZE_BODY` 12 / `LINE_BODY` 18, captions `FONT_SIZE_CAPTION` 12 / 18, panel and popover titles `FONT_SIZE_LARGE` 14; dialogs: title 16 semibold (`FONT_SIZE_TEXT_MD`), body 14 (`FONT_SIZE_TEXT_SM` / `LINE_TEXT_SM` 20); toast text-sm 14 medium; weights 400 / 500 / 600.

**Table 3: sizes** ("Δ" at 1600×1088):

| Element | Today | After | Δ |
|---|---|---|---|
| Command bar width / anchor | 440, above the toolbar | same | 0 |
| Command bar input row | 40 | 40 (region; input sm 36 inside) | 0 |
| Command bar tab row | 34 | 34 | 0 |
| Command bar section heading | 28 | 28 | 0 |
| Command bar item pitch | 32 (row 30) | 36 (row 32, C's R5 pitch) | +4 per row |
| Command bar frame radius | 6 | `RADIUS_FRAME` 16 (OQ 3) | — |
| Dialog padding | 20 | `INSET_MODAL` 24 | +8 height, content 8 narrower |
| Dialog footer buttons | 32 | `CONTROL_MD` 40 | +8 |
| Dialog fields (history modal) | 32 | `CONTROL_SM` 36 | +4 per field |
| Dialog picker rows (`home/dialog.rs` ROW_H) | 28 | 36 (`List/Row` sm) | +8 per row |
| Presentation link dialog buttons | 24 | `CONTROL_SM` 36 | +12 (card grows to fit) |
| Context-menu rows | 24 | `MENU_ITEM_HEIGHT` 32 | +8 per row |
| Find field | 28 | `FIND_FIELD_H` 32 | +4 |
| Find Replace buttons | 24 | `CONTROL_XS` 32 | +8 |
| Find result group rows | 28 | `FIND_GROUP_ROW_H` 32 | +4 per row |
| Comments filter / search | 24 / 28 | `CONTROL_XS` 32 / 32 | +8 / +4 |
| Thread popover buttons / icon buttons | 24 / 20 | 32 / 32 hit | +8 / +12 hit (header 36 unchanged) |
| History rows / day header / banner | 44 / 28 / 40 | same | 0 |
| History banner buttons | theme sizes | `CONTROL_XS` 32 | ≤ +8 |
| Toast | 40 high, bottom 72 | same | 0 |
| Radii | 6 / 14 | `RADIUS_CONTROL` 8 / `RADIUS_FRAME` 16 | — |

**States**: default, hover, pressed (`BG_PRESSED`), focused (`FOCUS_RING`), disabled (50 % opacity, C rule S5; dialog OK disabled keeps `TEXT_ON_BRAND` at 50 %, not `TEXT_DISABLED` on orange), selected (`BG_SELECTED`). Empty states: command bar "no results" and comments "no comments" use C's `EmptyState/Default` text roles (`TEXT`, `TEXT_TERTIARY`), no new illustration.

## 8. Edge cases

1. **Short window** (900×600 minimum): the command bar grows 4 px per row; its existing max-height and scroll logic (`:333` sums rows) must still keep it above the toolbar; check with 12 rows at 900×600.
2. **Long dialog text** wraps inside the 440 width minus 2 × 24 padding (8 px narrower than today); the measurement at `:207` uses `TEXT` instead of `WHITE` (same metrics, colour does not affect layout).
3. **Danger dialogs** (`d.danger`, `home/dialog.rs:320-321`): Destructive hierarchy, `TEXT_WHITE` label at 4.83:1 on `BG_ERROR_SOLID` (contract).
4. **Toast while a dialog is open:** the toast sits above the scrim as today; no z-order change.
5. **Find with 0 results:** status row (`FIND_STATUS_H` 32) shows `TEXT_TERTIARY`; Replace buttons disabled at 50 %.
6. **Thread popover near the window edge:** it flips as today; the 8 px taller buttons can push its bottom past the edge only if the composer is at max 160; the existing clamp must hold (check in the PR).
7. **Updater hidden:** Z14 must stay OK; its restyle is verified by the census and a unit test, not a screenshot.
8. **Comment avatars** on `BG_SECONDARY` cards: identity colours keep their contrast checks; initials stay `TEXT_ON_BRAND`-style dark only where today's code uses a dark label (no change).
9. **Presentation of a light prototype:** stage `#0A0A0A` instead of `#09090B` (1-step change); bezel and the computed document colours (`:650`, `:655`, `:751`) unchanged.
10. **Design Gate tab in the command bar** shows I's status colours through the terminal aliases; when I lands first, the audit rows change colour without a J edit.

## 9. Non-goals

- `panels/menus.rs` (menu bar and its command palette, D) and `shell.rs` (D paints the toast).
- Canvas comment pins, avatar identity colours, Find canvas match colours (`FIND_CANVAS_MATCH*`), presentation content.
- Unhiding the updater (Z14), any change to command behaviour, search ranking, Find logic, comment model, history data.
- Light mode, icon set (R6), Figma (R3).

## 10. Test plan (red first)

New file `crates/designos-app/src/shell/tests/overlays_chrome.rs` (new name), registered with one `mod overlays_chrome;` line in `crates/designos-app/src/shell/tests.rs` (`:1017-1048`); it uses the shared `chrome_colour_sites` helper (spec H section 10 defines its rule) and the recorded-rect hook (`widgets::record`, `widgets/mod.rs:245`).

| Test | Asserts | Red at `ec47da26` |
|---|---|---|
| `overlay_sources_paint_no_chrome_colour_literals` | census over J's 23 files returns only the `present.rs` allowlist (texture tints and the device bezel, matched by code text) | yes (35) |
| `cmdk_palette_uses_contract_roles` | `cmdk.rs` has no `CMDK_` constant; the recorded command rows (`cmdk-row-*`, add `widgets::record` where missing) are 32 high at a 36 pitch | yes |
| `dialog_footer_buttons_are_control_md` | open a confirm dialog in the `Sim` harness; OK and Cancel rects are `CONTROL_MD` high; dialog padding 24 | yes (32, 20) |
| `brand_labels_are_dark` | `kit::button_look(Hierarchy::Primary, State::Default).text == theme::TEXT_ON_BRAND` and no `Color32::WHITE` label in J files (census) | yes |
| `context_menu_rows_are_32` | right-click a layer; the recorded context rows are `MENU_ITEM_HEIGHT` (32) | yes (24) |
| `find_controls_follow_r5` | `find/panel.rs` field 32, Replace buttons 32, group rows 32 (recorded rects) | yes (28 / 24 / 28) |
| `comment_controls_follow_r5` | comments filter and search 32; thread popover buttons 32 | yes (24 / 28 / 24) |
| `updater_stays_hidden` | `UPDATER_UI_ENABLED == false` and `updater.rs` census 0 | yes (census 11) |
| `toast_rules_for_d` | (lands with D) toast fill `BG_SECONDARY`, border `BORDER_MENU`, no shadow call in `shell.rs:989-1016` | yes |

Keep green: `find/panel.rs` tests (`:432`), `cmdk.rs` tests, `shell/tests/find.rs`, `shell/tests/comments.rs`, `shell/tests/history.rs`, `shell/tests/menus.rs`.

Commands (repo root): `cargo test -p designos-app --bin designos overlays_chrome`; `cargo test -p designos-app --bin designos find comments history cmdk`; `cargo test --workspace --lib`; `python3 scripts/zero_fake_ui_gate.py` (Z08, Z14, Z15 OK); `python3 scripts/check_spacing_relationships.py` (exit 0); `python3 scripts/design_os_self_evolve.py`.

**Screenshot evidence** (before = at `ec47da26`, run 2026-10-25; after = same commands on the PR branch; both in the PR). Setup as spec I: `--seed-library --library "$LIB"` (exit 0), `FILE` = third seeded file ("Design system"); each run `cargo run -q --release -p designos-app --bin designos -- "$FILE" <args> --screenshot <out>.png --size 1600x1088 --no-toast`.

| Id | `<args>` | Exit code (this session) |
|---|---|---|
| J-01 | `--panel cmdk` | 1 ("screenshot was not captured"), retry 0 |
| J-02 | `--find Button` | 0 |
| J-03 | `--find Button --replace-with Action` | 0 |
| J-04 | `--present` | 0 |
| J-05 | `--panel version-history` | 1 ("screenshot was not captured"), retry 0 |
| (D) | `--menu palette` (D's palette, recorded for D's evidence only) | 0 |

The exit-1 runs happened while other agents were building in the same checkout; both retries passed. Not captured from the CLI: confirm and picker dialogs, the history modal and banner, the context menu, comment panel and thread, the presentation link dialog, and the toast (a toast needs a run without `--no-toast` and an action that raises one; not run in this session). The implementer captures them from a live session and says which were captured by hand. The updater is hidden (Z14) and is verified by tests only.

**Acceptance criteria**

1. The census prints `chrome_sites=0` for every J file except `present.rs`, which prints only the 3 allowlisted content sites (2 texture tints, device bezel).
2. The tests above pass (the toast test lands with D) and the listed existing tests stay green.
3. `grep -rnwE 'CONTROL_(DENSE|STANDARD)|FIND_BUTTON_H|COMMENT_BTN_H|COMMENT_FILTER_H|COMMENT_ICON_BTN' <J files>` prints nothing.
4. `J-01`..`J-05` after-shots show contract colours, no white label on orange, no white tab underline, the command bar at the same anchor and width; L's layout gate passes for these captures.
5. Z08, Z14, Z15 stay OK; `check_spacing_relationships.py` exits 0.
6. The PR lists re-anchored lines, the before/after census and which surfaces were captured by hand.

## 11. Performance budgets

Command bar: rows are painted per frame while open; replacing private constants and adding a `Look` lookup per row adds no allocation (pure fns over constants). Removing shadow draw calls (dialogs, link dialog, toast) lowers tessellation. Idle CPU unchanged (0.00 %); no new repaint requests. Frame time with the command bar open at 20 rows must not rise by more than 0.5 ms.

## 12. Dependencies, ownership and build order

**Build order.** L captures baselines (`J-01`..`J-05`) → A (contract constants; G-3, G-4 if accepted) → B (sizes, `MENU_ITEM_HEIGHT`, `FIND_*`) → C (`widgets/kit/`, including `Dialog/CommandPalette`, `Dialog/Modal`, `Toast/Default`, `modal_surface`) → this spec; D applies the toast rules in the same wave. I may land before or after J: the command bar's Design Gate section reads I's aliases either way.

**Technical decisions recorded here** (the owner may reverse): (1) the command bar input row stays 40 and holds C's sm 36 input (R1 forbids shrinking the region to C's 36 row); (2) history rows stay 44 (two-line rows; R5's 32 would shrink them); (3) device bezel and texture tints are content; (4) J re-points its call sites from four theme size constants instead of asking B to change their values, so the contract stays as written; (5) the toast fill is `BG_SECONDARY` because the kit's bg-primary_alt has no contract constant and has the same value.

**File ownership across H–K** (same table in H, I, J and K):

| File | Owner |
|---|---|
| `home/mod.rs`, `home/thumb.rs`, `home/time.rs`, `home/session.rs`, `home/sidebar.rs` and `home/main_view.rs` (deleted), `shell/tests/home.rs` | H |
| `terminal.rs`, `agent_*.rs`, `canvas/agent_banner.rs`, `canvas/agent_changes_chip.rs`, `canvas/agent_overlay.rs`, `skills/*`, `image_jobs.rs`, `shell/tests/agent_chrome.rs` (new) | I |
| `cmdk.rs`, `updater.rs`, `present.rs`, `panels/context.rs`, `home/dialog.rs`, `history/*`, `find/mod.rs`, `find/panel.rs`, `find/select.rs`, `comments/*`, `shell/tests/overlays_chrome.rs` (new) | J |
| `panels/assets.rs`, `variables/*`, `libraries/*`, `styles.rs`, `plugins/*`, `shell/tests/assets_variables_libraries_chrome.rs` (new) | K |
| `shell/tests.rs` | shared, append-only: one `mod` line per spec and the `chrome_colour_sites` helper (first to land adds it) |

Not mine: `panels/menus.rs` (D, including its command palette `:879-1057`: agreed with D), `shell.rs` (D; J writes the toast rules, D paints them), `tabs.rs`, `panels/rail.rs`, `panels/toolbar.rs`, `canvas/rulers.rs` (D); `panels/left.rs`, `panels/layers.rs`, `find/filter.rs` (E: the Layers filter field); `panels/right.rs`, `panels/properties.rs`, `panels/inspect.rs`, `widgets/picker.rs`, `panels/style_picker.rs`, `panels/style_edit.rs`, `panels/style_edit_grid_rows.rs` (F); `dev/*`, `specs.rs` (G); `theme.rs`, `design/*`, `scripts/*` (A, B); `widgets/*`, `widgets/kit/*` (C). Spec D's not-mine list names `updater.rs` as H's; it is J's (this table and H agree).

Conflicts with other specs, recorded for the controller: (a) spec E maps the Find match highlight to a brand tint; the token contract keeps `FIND_HIGHLIGHT` in the info family, and J follows the contract. (b) Spec C gives the command bar frame and toasts radius-xl 12; the contract has `RADIUS_FRAME` 16 only (Open question 3). (c) Spec C sets the command bar input row to 36; J keeps the 40 region (R1).

**Open PRs touching J files** (`gh pr list --state open`, 2026-10-25; file lists from `gh api repos/{owner}/{repo}/pulls/<n>/files`): stale #765, #768 and #770 (opened 2026-10-19, `mergeable: CONFLICTING`) all touch `cmdk.rs`, `home/dialog.rs` and `panels/context.rs`; #765 also `comments/mod.rs`, `comments/settings.rs`, `history/panel.rs`, `history/session.rs`, `present.rs`; #768 `comments/settings.rs`; #770 `history/panel.rs`. No other open PR (#1051, #1070, #1075, #1081, #1084, #1085, #1112, #1113) touches J files. Rebase on whatever lands first; re-run the census after each rebase.

## 13. Owner rulings that apply

- R1: "sử dụng 100% style kiểu của Untitled UI nhưng vẫn giữ cấu trúc layout và bố cục hiện tại" (all of Untitled's style; keep layout and arrangement).
- R2: take Untitled spacing and typography as they are, adapted to Studio's layout.
- R3: Studio improves itself; no Figma. Evidence goes in the PR.
- R4: brand = Untitled anatomy with Studio orange `#E36323`, dark label `#0A0A0A`; focus ring `#F97316`; Purple Ban stays.
- R5: editor surfaces text-xs 12/18, button xs 32, row 32, radius md 8; dialogs text-sm 14/20, buttons sm 36 / md 40.
- R6: keep the icon set; restyle colour to Untitled fg roles.
- R7: spec-driven; the PM coordinates delivery.
- Controller defaults: canvas overlays (comment pins, Find canvas match) keep their colours; chrome status colours move to Untitled.
- Standing: `main` is protected (PR, green macOS test, Clippy, Rustfmt, squash); visual evidence in every PR (2026-09-22). Z14: the updater stays hidden.

## Open questions for the owner

1. **Command bar item height.** Recommended: 36 pitch (row 32), C's R5 reading of the kit's 44 command item; the bar grows 4 px per row. Alternative: keep the 32 pitch so the bar's height is unchanged.
2. **Dialog footer buttons.** Recommended: md 40 (C's `Dialog/Modal` footer, R5 allows sm 36 / md 40). Alternative: sm 36, 4 px shorter, matching Home's header buttons.
3. **Frame radius for the command bar, dialogs and toasts.** Recommended: follow the token contract's `RADIUS_FRAME` 16 (one frame radius across Studio; contract OQ-5 already asks about it). Alternative: add radius-xl 12 as a new contract constant for these three surfaces, as spec C's anatomy reads the kit.
4. **Find match highlight.** Recommended: keep the contract's info-blue highlight (`FIND_HIGHLIGHT`), distinct from selection orange on the canvas. Alternative (spec E's proposal): brand-tinted highlight; then A changes the contract and both E and J follow.
