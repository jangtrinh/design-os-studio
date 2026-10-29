# J — Surface: command bar, dialogs, toasts, context menu, Find, comments, history and presentation (Untitled UI dark)

| Field | Value |
|---|---|
| Issue | #1146 (epic #1118, spec key J) |
| Size | M–L (about 2 days: 6 files with 1–11 chrome colour sites, 14 files that re-point sizes only, toast rules for D) |
| Depends on | A, B, C and L |
| Unblocks | D (toast rules in section 3, painted in `shell.rs`), M (docs and screenshot refresh) |
| Touches | `crates/designos-app/src/cmdk.rs` (palette constants `:268-272`, `show` `:275-566`, `render_audit_section` `:567-640`); `updater.rs` (constants `:38-45`, `show_sidebar_card_in_rect` `:569`, `show_update_modal` `:662`); `present.rs` (`show` `:406`, bars `:500`, `:507`, link dialog `:902-928`, reason pill `:785-794`); `panels/context.rs` (`show` `:145`, `show_layer_picker` `:169`); `home/dialog.rs` (`show` `:243`); `history/modal.rs`, `history/panel.rs`, `history/banner.rs`; `find/panel.rs`, `find/mod.rs`, `find/select.rs`; `comments/panel.rs`, `comments/thread.rs`, `comments/ui.rs`, `comments/pins.rs`; tests: new `crates/designos-app/src/shell/tests/overlays_chrome.rs` (new name) |

**Code baseline.** Every `file:line` is at `origin/main@8dfaee93` (v0.7.0 merged), read with `git show 8dfaee93:<path>` (never a checkout). The earlier drafts cited `ec47da26` and then `205c72ef`; fix round 3 (2026-10-28) moved every citation by a line diff of `205c72ef..8dfaee93` (a line that did not change keeps its text and gets its new number; the few whose code changed were placed by code text, marked "placed by hand"). The cross-check: the census at `8dfaee93` (section 2) prints exactly the mapped lines. Re-anchor against the commit you branch from before editing (search for the quoted code, not the number). Colour and size names are the token contract's and are Studio's names (R14); Untitled role names are given beside them as references. Kit facts marked "live kit" were read read-only in Figma under R9 (`figma-agent context <node> --dir <scratch>`, file key `VEffelx01mDl5BJJanwxja`, nothing mutated); the node ids are the receipts for the radii (command bar `3307:407031`, Modal `4057:415360`, Notification `1135:2645`; spec D cites the same Notification id, while C's anatomy notes `1135:507` as a frame of another variant that binds the same radius).

**Drift note (`205c72ef` to `8dfaee93`).** J's painting files did not move except `home/dialog.rs` (507 to 735 lines, +230: the typed-path import prompt) and the toast in `shell.rs`/`shell/toast_layout.rs` (D's files); `cmdk.rs` (−6 lines after `:640`), `updater.rs`, `present.rs`, `panels/context.rs`, `history/*`, `find/*` and `comments/*` kept their line numbers (`cmdk.rs` after `:640` only). Callers moved: `shell.rs` +140 lines net (+196 / −56; `:1339` to `:1465`, `:1604` to `:1739`, `:1605` to `:1740`, `:1588` to `:1723`), `panels/left.rs` +18 (Find at `:162-165`), `theme.rs` +8 after `:290`, `app.rs` `show_toast` `:364`. Census: the same 35 sites. What changed that J must style: (1) **Toast**: the message wraps and the toast grows upward (`shell/toast_layout.rs`: left 28, right 16, vertical 12, action gap 12, up to 720 wide, action target `ACTION_HEIGHT` 24), the action is "Show in folder" (`toast-reveal`, `shell.rs:1119`) or, when no system file dialog opened, "Enter a path…" (`toast-enter-path`, `shell.rs:1121`, #1192), and the action label is `TEXT_BRAND` (baked into the galley at `toast_layout.rs:50`) with a `BG_HOVER` fill on its target (`shell.rs:1141`); the "Details" word is part of the message and the whole toast is its click target (`toast-details`, `shell.rs:3680`, a hand cursor and no fill). (2) **`home/dialog.rs`** has a new prompt dialog, `Intent::ImportPath` ("Import file", the typed-path prompt behind "Enter a path…"), with a field hint `IMPORT_PATH_HINT` drawn in `TEXT_TERTIARY` (`:309`). (3) `updater.rs` is still hidden (Z14). Uncovered surfaces: see section 12.

## 1. Summary and user story

As a designer, I want every floating surface — the Actions command bar (Cmd+K), confirm and picker dialogs, the context menu, toasts, Find and Replace, comment threads, version history and the presentation player's own controls — to look like Untitled UI's dark kit, so pop-ups stop mixing Figma UI3 greys (`#2C2C2C`, `#383838`), Figma blue and white-on-orange labels with the rest of the app.

Positions, widths and order stay (R1). Colours come from the token contract; anatomy from spec C (`Dialog/Modal`, `Dialog/CommandPalette`, `Toast/Default`, `Menu/Context`, `Menu/Item`, `Popover/Panel`, `Input/Search`, `Tabs/Segmented`, `Button/*`, `Tooltip/Default`); sizes from R5 (editor surfaces: text-xs rows, button xs 32, row 32; dialogs: text-sm 14/20, buttons sm 36 / md 40).

**Agreement with spec D.** D owns `panels/menus.rs`, including the menu command palette (`panels/menus.rs:907-1085`, captured with `--menu palette`). J agrees and does not edit that file. J owns the separate Actions command bar in `cmdk.rs` (opened with Cmd+K, docked above the toolbar, `J-01`). Both use C's `Dialog/CommandPalette`, so they look the same. D also owns `shell.rs`; J writes the toast rules (section 3, rules 12–15) and D paints them at `shell.rs:1087-1160`.

## 2. Current state

FACT unless labelled. Screenshots of today in section 10 (`J-01`..`J-05`).

| Surface | Code | Today |
|---|---|---|
| Actions command bar | `cmdk.rs:275` `show`; private palette `:268-272` (`CMDK_BG_SURFACE #2C2C2C`, `CMDK_BG_ROW_HOVER #383838`, `CMDK_BORDER #383838`, `CMDK_BADGE_BG #383838`, `CMDK_BADGE_TEXT #B3B3B3`); width `min(440, screen − 32)` `:282`; docked above the toolbar with no scrim (`Area` at `:347`); frame `elevation_400` `:353` + `CMDK_BG_SURFACE` + 1 px `CMDK_BORDER` `:354-355`; height `40 + 34 + sections × 28 + rows × 32 + audit + 10` `:343`; input row 40 `:359` (input `:357-385`); tab row 34 with a 2 px `Color32::WHITE` underline `:395-426`; rows 30 high at a 32 pitch, radius 6 `:448-474`; badge 18 `:469`; selected icon and title `Color32::WHITE` `:464`, `:474`; tab label `WHITE` `:416`; Design Gate audit section `:567-640` reads I's terminal palette (`cmdk.rs:14-16`, used `:579-625`) | Figma UI3 greys |
| Updater card and modal | `updater.rs` UI3 palette `:38-45` (`UI3_BG_PANEL #1E1E1E`, `UI3_BG_CARD #2C2C2C`, `UI3_BG_HOVER #343434`, `UI3_BORDER #383838`, `UI3_BLUE #0D99FF`, `UI3_EMERALD #1BC47D`, `UI3_AMBER #F59E0B`, `UI3_RED #F24822`); card `:569` returns at once while `UPDATER_UI_ENABLED` is false (`:570-572`, gate Z14 keeps it hidden); modal `:662`, scrim black α150 `:677`, channel pill label `WHITE` `:730`, "Stage Atomic Update" 154×26 on `UI3_BLUE` with `WHITE` label `:827-831` | hidden; not capturable |
| Presentation chrome | `present.rs:406` `show`; stage `PRESENT_BG #09090B` (`theme.rs:603`), bar `PRESENT_BAR_HEIGHT` 48; hairlines white α18 `present.rs:500`, `:507`; device bezel `Color32::BLACK` `:702`; hotspot reason pill 26 high, black α190 + `WHITE` text `:790-794`; variables panel `paint_vars_panel` `:805` (shadow `elevation_shadow` `:826`, `BG_MENU` + `BORDER_MENU` `:827-828`); "Open this link?" prompt `confirm_bar` `:903-924`: a 520×56 card 88 px above the stage bottom, no scrim (shadow `:908`, fill `BG_MENU` `:909`, Open 64×24 pill on `BG_BRAND` with `WHITE` label `:913`, `:918-919`, Cancel 60×24 `:914`); dead `_icons_used` `:928`; texture tints `Color32::WHITE` `:270`, `:898`; computed document colours `:665`, `:670`, `:766` | — |
| Context menu | `panels/context.rs:145` `show` (rows through C's `widgets::menu`), entries `:65`, layer picker `:169` (shadow `elevation_shadow` `:190`, `BG_MENU` + `BORDER_MENU` `:191-192`; hover text `Color32::WHITE` `:220`, `:227`) | rows 24 (`MENU_ITEM_HEIGHT` = `CONTROL_DENSE`) |
| Confirm, picker and about dialogs | `home/dialog.rs:243` `show` (W 440, PAD 20, BTN_H 32, ROW_H 28 `:76-79`); measuring colour `WHITE` `:254`; scrim black α140 `:273`; panel `menu_panel` `:277`; OK label `WHITE` `:383`; the typed-path prompt `Intent::ImportPath` (`:37`, "Import file", field hint `IMPORT_PATH_HINT` `:49` drawn in `TEXT_TERTIARY` `:309`, opened by `open_import_path_dialog` `:235-239`); called from `shell.rs:1465`, `:1739` and Home | dialog |
| Version-history modal | `history/modal.rs:16` (W 440, PAD 20, FIELD_H 32, DESC_H 88, BTN_H 32, LABEL_H 18 `:9-14`); scrim black α140 `:32`; panel `menu_panel` `:36`; called from `shell.rs:1740` | dialog |
| Version-history panel | `history/panel.rs:22` (replaces the right panel, `panels/right.rs:14`; rows `HISTORY_ROW_H` 44, day headers `HISTORY_DAY_H` 28; `history/panel.rs:199`, `:208`) | — |
| Version-preview banner (`Banner/Inline`) | `history/banner.rs:9` (`HISTORY_BANNER_H` 40; called `shell.rs:1723`) | — |
| Find and Replace | `find/panel.rs:179` `show` (in the left panel, `panels/left.rs:162-165`); field `find/panel.rs:158-177`; constants `FIND_FIELD_H` = `CONTROL_STANDARD` 28, `FIND_TOGGLE_SIZE` 22, `FIND_BUTTON_H` 24, `FIND_STATUS_H` 32, `FIND_GROUP_ROW_H` 28, `FIND_HIGHLIGHT`, `FIND_HIGHLIGHT_SELECTED` (`theme.rs:311-321`); tests `find/panel.rs:432` | — |
| Comments panel | `comments/panel.rs:19` (replaces the right panel, `panels/right.rs:18`; SIDE 12, AVATAR 20, SEARCH_H 28, FOOTER_H 40 `comments/panel.rs:13-17`; `COMMENT_PANEL_HEADER_H` 48, `COMMENT_CARD_PAD` 12; two segmented filters at `COMMENT_FILTER_H` 24: "This page / All pages" `:44-49` (`comments-filter-pages`) and "Open / Resolved" `:51-56` (`comments-filter-status`); then the search field `SEARCH_H` 28 `:60`) | — |
| Comment thread popover | `comments/thread.rs:38` `show` (`COMMENT_POP_W` 300, `COMMENT_HEADER_H` 36, `COMMENT_BTN_H` 24, `COMMENT_ICON_BTN` 20, `COMMENT_AVATAR` 24, box 56–160; composer W 280 `:13`); frame `pop_frame` → `menu_panel` `:47`; `:52`, `:120`, `:284` | — |
| Comment widgets | `comments/ui.rs:57`, buttons `:92-108`, dots button `:117-131`, segmented `:133-152` (C lists all three as duplicates D–K retire) | — |
| Comment pins and avatars | `comments/pins.rs`; `COMMENT_PIN` 32, `COMMENT_PIN_RESOLVED #757575`, `AVATAR_COLORS` (`theme.rs:473-479`) | canvas overlay + identity colours |
| Toast | `shell.rs:1087-1160` (D's file): `TOAST_HEIGHT` 40, `TOAST_BOTTOM` 72, `TOAST_SECONDS` 4 (`theme.rs:466-468`), elevation shadow `shell.rs:1134` (placed by hand), status dot `:1136` (`DANGER` or `BG_BRAND`), fill `:1135`, action label `TEXT_BRAND` (galley built in `shell/toast_layout.rs:50`, painted `shell.rs:1143`) with a `BG_HOVER` fill on the action target on hover `:1141`; the "Details" word is message text and the whole toast is its click target (`toast_click_target` `:3680`, no fill); actions "Show in folder" (`:1119`) and "Enter a path…" (`:1121`); the layout lives in `shell/toast_layout.rs` (D); raised by `app.rs:364` `show_toast` | — |

**Chrome colour census** (spec D's command, unchanged across D–K):

```bash
cd /Users/jangtrinh/Products/design-os-studio
# REV=8dfaee93 reads that commit with `git show` (no checkout); drop it to read your working tree
REV=8dfaee93 PYTHONDONTWRITEBYTECODE=1 python3 - cmdk.rs updater.rs present.rs panels/context.rs home/dialog.rs \
  history/modal.rs history/panel.rs history/banner.rs history/mod.rs history/session.rs history/preset.rs \
  find/mod.rs find/panel.rs find/select.rs comments/mod.rs comments/panel.rs comments/thread.rs comments/pins.rs \
  comments/ui.rs comments/settings.rs comments/model.rs comments/import.rs comments/time.rs <<'PY'
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

Result at `8dfaee93` (`REV=8dfaee93`, run 2026-10-28, exit 0):

| File | `theme::` lines | Chrome sites | Lines | Computed |
|---|---|---|---|---|
| `cmdk.rs` | 24 | 9 | `:268-272`, `:416`, `:425`, `:464`, `:474` | — |
| `updater.rs` | 40 | 11 | `:38-45`, `:677`, `:730`, `:831` | — |
| `present.rs` | 53 | 9 | `:270`, `:500`, `:507`, `:702`, `:792`, `:794`, `:898`, `:919`, `:928` | `:665`, `:670`, `:766` (document colours) |
| `panels/context.rs` | 13 | 2 | `:220`, `:227` | — |
| `home/dialog.rs` | 25 | 3 | `:254`, `:273`, `:383` | — |
| `history/modal.rs` | 8 | 1 | `:32` | — |
| `history/panel.rs`, `history/banner.rs` | 32, 9 | 0 | — | — |
| `find/panel.rs` | 76 | 0 | — | — |
| `comments/panel.rs`, `thread.rs`, `ui.rs`, `pins.rs` | 40, 39, 32, 1 | 0 | — | — |
| the other 10 files | 0–1 | 0 | — | — |

Total today: **35** chrome sites in J's files. (plan.md's `grep -c 'Color32::from_rgb('` gives cmdk 5, updater 8, present 3; the census above is the contract.)

**Gates today.** No gate counts colour literals; `.github/workflows/design-gates.yml` is advisory (`continue-on-error`, `:4-5`, `:21`). `scripts/zero_fake_ui_gate.py` checks Z08 (`panels/toolbar.rs` + `cmdk.rs`: contextual actions mutate state; Remove background stays removed), Z14 (updater UI stays hidden while `UPDATER_UI_ENABLED = false`) and Z15 (`terminal.rs` + `cmdk.rs`: no fake template synthesis; `synthesize_spec_from_prompt` stays private to `cmdk.rs`). `scripts/check_spacing_relationships.py` passed on the first draft's `ec47da26` (exit 0; not re-run at `8dfaee93`, which needs a checkout).

## 3. Behaviour rules

1. **Layout unchanged (R1).** Every surface keeps its anchor, width and order: command bar 440 wide docked above the toolbar; dialogs 440 wide centred; thread popover 300 wide beside its pin; Find in the left panel; comments and history in the right panel's place; presentation bar 48. Heights change only where R5 raises a control or row (table 3).
2. **Command bar** (`cmdk.rs`; J owns this anatomy, and D composes its own palette in `panels/menus.rs` from it) is C's `Dialog/CommandPalette`: frame `BG_MENU` + 1 px `BORDER_MENU`, radius `RADIUS_XL` 12, no shadow (delete `widgets::elevation_400` at `cmdk.rs:353`; C T10), no scrim (today it has none; R1). **Kit receipt** (read-only, R9: `figma-agent context <node> --dir <scratch>`, fileKey `VEffelx01mDl5BJJanwxja`, Command menus page `3307:403757`, 2026-10-25): component set `3307:407570`, variant `Type=Command only, Text=Default` `3307:407212`; its `Command bar` frame `3307:407031` binds radius-xl 12, fill bg-primary, no stroke, effect shadow-xl (alpha 0 in dark, contract T10), max width 640, and sits on a `Background overlay` instance `4896:411546`. Deviation, recorded: the kit separates the borderless bar from the page with that overlay; Studio's bar is docked above the toolbar without a scrim, so it keeps the 1 px `BORDER_MENU` edge that every Studio menu and popover has (R8.5), instead of adding a scrim (a new element, R1). Radius and variant follow the kit (plan.md C-K: "command bar = kit Command menu"); input row stays 40 (region; controller ruling: C aligns with J) with C's borderless `Input/Search` inside; tabs are Untitled underline tabs (current label `TEXT`, others `TEXT_QUATERNARY`, underline `BORDER_SELECTED` 2 px instead of white); section headings `TEXT_TERTIARY` caption; rows are `_Command dropdown menu item` at pitch 36 (row 32 with 2 px inset; decided, controller default "Command bar item pitch 36 … — J OQ1"), hover `BG_MENU_HOVER`, selected `BG_SELECTED`, icon `ICON_TERTIARY` (selected `ICON`), title `TEXT_SECONDARY` (selected `TEXT`); the right-side tag ("AI", "Development") is C's `Badge/StatusCount` gray (`BADGE_NEUTRAL_BG` / `_BORDER` / `_TEXT`). The Design Gate audit section reads I's terminal palette aliases (spec I rule 3) and needs no edit beyond its own literals (none).
3. **Dialogs** (`home/dialog.rs`, `history/modal.rs`, updater modal; not the presentation link prompt, rule 9) are C's `Dialog/Modal`: fill `BG`, **no border**, width unchanged, padding 24 (`INSET_MODAL`), title text-md semibold `TEXT`, body text-sm `TEXT_TERTIARY`, footer buttons `CONTROL_MD` 40 (decided, controller default "Dialog footer buttons md 40 — J OQ2"; Primary + Secondary, or Destructive for danger dialogs), fields `CONTROL_SM` 36 (`Input/Field` sm; the `ImportPath` prompt's hint is a placeholder, `TEXT_PLACEHOLDER`, E-1, not today's `TEXT_TERTIARY` `home/dialog.rs:309`), picker rows C's `List/Row` sm 36. Radius `RADIUS_FRAME` 16 (plan.md C-K: "dialogs/modals = kit Modals"). **Kit receipt** (read-only, R9, Modals page `172:4293`, 2026-10-25): variant `Type=Stacked left aligned, Breakpoint=Desktop` `4057:415359`; its `Modal` frame `4057:415360` (also `4057:422034`) binds radius-2xl 16 (variable `6298:46481`), fill bg-primary, no stroke, max width 400; `Background overlay` `4896:411057` is bg-overlay at layer opacity 0.7 with a background blur. Scrim `BG_OVERLAY` (contract N33: bg-overlay `#262626` at 70 %, the kit's `Background overlay` layer opacity 0.7), drawn by C's `kit::surface::modal_surface`, which replaces the scrim literals (`home/dialog.rs:273`, `history/modal.rs:32`, `updater.rs:677`) and the `menu_panel` calls (`home/dialog.rs:277`, `history/modal.rs:36`); J's files keep no scrim literal and no shadow call. **Blur: not drawn** (J's decision, contract N33): the kit adds a background blur, Figma `BACKGROUND_BLUR` radius 16, which Figma's CSS export writes as `backdrop-filter: blur(8px)` (CSS takes the Gaussian standard deviation, so radius 16 = sigma 8; contract N33 records both), but egui has no backdrop filter, so Studio would need an extra offscreen pass over the whole window every frame a dialog is open; P8's frame budget does not allow it for a decorative effect, and the 70 % scrim alone keeps the dialog legible.
4. **Brand and destructive labels.** A label on `BG_BRAND` / `BG_BRAND_HOVER` / `BG_BRAND_PRESSED` is `TEXT_ON_BRAND` (R4 dark label); a label on `BG_ERROR_SOLID` (hover `BG_ERROR_SOLID_HOVER` `#B91C1C`, R8.7) is `TEXT_WHITE`. Every xs button label is text-sm 14 semibold 600 (R8.6). No `Color32::WHITE` label remains (`home/dialog.rs:383`, `present.rs:919`, `updater.rs:730`, `:831`, `cmdk.rs:416`, `:464`, `:474`).
5. **Context menu** (`panels/context.rs`) uses C's `Menu/Context` and `Menu/Item`: rows `MENU_ITEM_HEIGHT` 32 (contract (c)), hover `BG_MENU_HOVER` with label `TEXT_SECONDARY_HOVER`, shortcuts `TEXT_QUATERNARY` (C's `Menu/Shortcut`), separators `BORDER_MENU`. The layer picker (`:169`) paints its rows with the same `Menu/Item` look instead of `Color32::WHITE` hover text, and its frame with C's `menu_surface` (16, `BG_MENU` + `BORDER_MENU`, no shadow: delete `elevation_shadow` at `panels/context.rs:190`).
6. **Find and Replace** (`find/panel.rs`): fields are C's `Input/Search` / `Input/Field` at `FIND_FIELD_H` (contract: `CONTROL_XS` 32); option toggles keep `FIND_TOGGLE_SIZE` 22 inside the 32 field (C's `Button/Icon` look, selected = `BG_SELECTED`); Replace and Replace all are `Button/Secondary` xs 32 (J re-points its call sites from `FIND_BUTTON_H` 24 to `CONTROL_XS`); result group rows `FIND_GROUP_ROW_H` (contract: 32); match highlight keeps `FIND_HIGHLIGHT` / `FIND_HIGHLIGHT_SELECTED` (contract (e): Studio-only info family).
7. **Comments.** Panel header 48 and footer 40 stay (regions); the two filters are C's `Tabs/Segmented` at today's height `COMMENT_FILTER_H` 24 (plan.md C-K: "Segmented controls outside R10.1's three strips keep today's height (R1)"; C row 130: "height is the surface's current height"; `COMMENT_FILTER_H` keeps its contract value 24, so J keeps reading it); search is `Input/Search` xs 32 (local `SEARCH_H` 28 → `CONTROL_XS`; fill `BG`, border `BORDER_STRONG`, plan.md C-F); cards `BG_SECONDARY` + `BORDER`, hover `BG_SECONDARY_HOVER`. Thread popover: C's `Popover/Panel` (`popover_surface` 16, `BG_MENU` + `BORDER_MENU`, no shadow: `pop_frame`'s `menu_panel` call at `comments/thread.rs:47` goes; header 36 stays), action buttons `Button/*` xs 32 (call sites use `CONTROL_XS` instead of `COMMENT_BTN_H` 24), icon buttons `Button/Icon` xs (hit 32 inside the 36 header, glyph unchanged), composer `Input/Field` (textarea) with `BORDER_STRONG`, focus `FOCUS_RING`. Avatars keep `AVATAR_COLORS` and sizes (identity, contract (e)); pins keep `COMMENT_PIN_RESOLVED` (canvas overlay).
8. **Version history.** Panel rows stay 44 (two-line rows; R5's 32 would shrink them, R1 forbids) with hover `BG_HOVER`, selected `BG_SELECTED`, day headers 28 caption `TEXT_TERTIARY`; banner (`Banner/Inline`) 40 stays (region) with `Button/*` xs 32 inside; modal per rule 3.
9. **Presentation chrome.** Stage `PRESENT_BG` (contract: `#0A0A0A`); bar and footer hairlines `BORDER`; the hotspot reason pill uses the tooltip look (contract tooltip ruling: fill `BG_PRIMARY_SOLID` `#262626`, text `TEXT`; 26 high stays); the variables panel (`paint_vars_panel`, `:805`) is C's `Popover/Panel` (16, `BG_MENU` + `BORDER_MENU`; delete `elevation_shadow` at `:826`). The "Open this link?" prompt (`confirm_bar`, `:904-923`) is not a modal: it is a `Popover/Panel`-style card (R1: no scrim, no new element): `BG_MENU` + 1 px `BORDER_MENU`, radius `RADIUS_FRAME` 16, no shadow (delete `elevation_shadow` at `:908`), title text-xs 12 semibold `TEXT`, URL `TEXT_SECONDARY`, Open = `Button/Primary` sm 36 and Cancel = `Button/Secondary` sm 36 (radius 8 instead of today's pill); the card grows from 56 to 60 (12 + 36 + 12; the two 18 px text lines also fit), keeping its top 88 px above the stage bottom, so its bottom gap shrinks from 32 to 28. The device bezel (`:702`) and the texture tints (`:270`, `:898`) are content and stay (allowlist). The dead `_icons_used` (`:927`) is deleted.
10. **Updater** (hidden): restyle its code so it is ready if Z14 is ever lifted, without unhiding it: UI3 palette → contract constants (table 1); never change `UPDATER_UI_ENABLED`.
11. **Focus.** Every focusable control shows C's focus ring (`FOCUS_RING`) on keyboard focus; the command bar input and dialog fields show it when focused.

**Toast rules** (J owns the toast anatomy and D paints it in `shell.rs:1087-1160`; this is the one complete rule D's toast row applies, coordinator amendment). C's `Toast/Default` provides the look (`kit::surface::toast_surface`, radius 12).

12. **Anatomy.** C's `Toast/Default` (kit reference: `Notification · Type=No icon`, single line): height `TOAST_HEIGHT` 40 at least (a long message wraps and the toast grows upward, `shell/toast_layout.rs`, up to 720 wide), text text-sm 14 medium `TEXT` (C-1.54, 17.18:1 on `BG_PRIMARY_ALT`). The layout stays today's (R1: left 28 holding the 8 dot, right 16, vertical 12, action gap 12; "padding 0 / 16, gap 8" in C's table is the kit's, not a Studio change); only the action target grows from `ACTION_HEIGHT` 24 to `CONTROL_XS` 32 (D, inside the 40 toast). The toast carries one action: "Show in folder" (`toast-reveal`) or "Enter a path…" (`toast-enter-path`, #1192), and the message may end in "Details" (`toast-details`).
13. **Colours.** Fill `BG_PRIMARY_ALT` `#171717` (contract N18, toast fill); 1 px border `BORDER_MENU` `#262626` (border-secondary_alt); **no shadow**: delete the `elevation_shadow` call at `shell.rs:1134` (C T10). Status dot 8 (`:1136`): info `ICON_BRAND_PRIMARY` `#F97316` (N14), error `DANGER` `#EF4444`, success `SUCCESS` `#22C55E` (C `Toast/Default` adds success); non-text ratios on `#171717`: 6.40, 4.76 and 7.87:1 (over 3.0). The "Details" link and the reveal action ("Show in folder" or "Enter a path…") are C's `Button/Tertiary` xs: rest no fill, label `TEXT_SECONDARY` `#D4D4D4` (12.09:1 on `#171717`; today `TEXT_BRAND`, baked into the galley at `shell/toast_layout.rs:50` and painted at `shell.rs:1143`, so D passes the colour in); hover fill `BG_SECONDARY_HOVER` `#262626` (the `#262626` fill of plan.md C-R) (C rule S6: on a `bg-secondary` surface the hover moves one step up, and `BG_HOVER` `#171717` would equal the toast fill and be invisible), label `TEXT_SECONDARY_HOVER` `#E5E5E5` (12.01:1 on `#262626`). The hover fill sits 1.18:1 from the toast fill: the kit's own one-step hover, a state cue that the label carries, not a 3:1 control boundary. Today's hover `BG_HOVER` at `:1141` is replaced. **Code note for D** (`8dfaee93`): the action's hover fill is painted on its own target rect (`shell.rs:1141`), but the "Details" word is part of the message and its click target is the whole toast (`toast-details`, `shell.rs:3680`: a hand cursor, no fill today). The rule above stands for both, hover fill `#262626` and never `BG_HOVER`; D either narrows the Details target to the word or paints its hover over the toast (listed for D).
14. **Radius** `RADIUS_XL` 12 (plan.md C-K: "toasts = kit Notification"; R8.5's 16 covers menus, dropdowns and popovers, not toasts). Today `shell.rs:1135` uses `RADIUS_FRAME`. **Kit receipt** (read-only, R9, Alerts & notifications page `176:4256`, 2026-10-25): component set `1135:618`, variant `Type=No icon, Breakpoint=Desktop` `1135:2644`; its `Notification` frame `1135:2645` binds radius-xl 12, 1 px stroke border-secondary_alt, fill bg-primary_alt, effect shadow-lg (alpha 0 in dark, T10).
15. **Timing and position** unchanged: `TOAST_BOTTOM` 72, `TOAST_SECONDS` 4, `TOAST_REVEAL_SECONDS` 10 (`theme.rs:466-470`).

## 4. Data model / schema changes

None.

## 5. Contracts

- **Colour constants consumed** (token contract (a)): `BG`, `BG_PRIMARY`, `BG_SECONDARY`, `BG_SECONDARY_HOVER`, `BG_TERTIARY`, `BG_HOVER`, `BG_PRESSED`, `BG_MENU`, `BG_MENU_HOVER`, `BG_SELECTED`, `BG_BRAND`, `BG_BRAND_HOVER`, `BG_BRAND_PRESSED`, `BG_ERROR_SOLID`, `BG_ERROR_SOLID_HOVER`, `BG_PRIMARY_ALT`, `BG_PRIMARY_SOLID`, `BG_OVERLAY`, `BADGE_NEUTRAL_BG`, `BADGE_NEUTRAL_BORDER`, `BADGE_NEUTRAL_TEXT`, `BORDER`, `BORDER_STRONG`, `BORDER_MENU`, `BORDER_SELECTED`, `TEXT`, `TEXT_SECONDARY`, `TEXT_SECONDARY_HOVER`, `TEXT_TERTIARY`, `TEXT_QUATERNARY`, `TEXT_PLACEHOLDER`, `TEXT_DISABLED`, `TEXT_ON_BRAND`, `TEXT_WHITE`, `TEXT_ERROR`, `TEXT_BRAND`, `ICON_BRAND_PRIMARY`, `ICON`, `ICON_SECONDARY`, `ICON_TERTIARY`, `ICON_QUATERNARY`, `DANGER`, `SUCCESS`, `WARNING`, `FOCUS_RING`, `PRESENT_BG`, `FIND_HIGHLIGHT`, `FIND_HIGHLIGHT_SELECTED`, `COMMENT_PIN_RESOLVED`, `AVATAR_COLORS`, `MENU_SHORTCUT_HIGHLIGHT`.
- **Former gaps, now in the contract (W1, N24–N37 and G-4):** `BG_OVERLAY` replaces the three scrim alphas (140 / 140 / 150); `RADIUS_XL` 12 serves the command bar and the toast. J requests no new colour; C must give the command palette a radius-12 surface with a border and no scrim (listed for W2), because `modal_surface` draws a scrim and no border.
- **Size constants consumed** (contract (c)): `CONTROL_XS` 32, `CONTROL_SM` 36, `CONTROL_MD` 40, `BADGE_SM` 22, `MENU_ITEM_HEIGHT` 32, `FIND_FIELD_H` 32, `FIND_GROUP_ROW_H` 32, `RADIUS_CONTROL` 8, `RADIUS_XL` 12 (command bar, toast), `RADIUS_FRAME` 16 (dialogs, popovers and menus per R8.5), `INSET_MODAL` 24, `INSET_FRAME` 8 (menu item inset, R8.5), `FONT_SIZE_*` and `LINE_*` per B. Theme constants J stops reading (their values stay per the contract appendix; A/B may delete them when no reader is left): `FIND_BUTTON_H` 24, `COMMENT_BTN_H` 24, `COMMENT_ICON_BTN` 20. J keeps reading `COMMENT_FILTER_H` 24 (the comments filters keep today's height, plan.md C-K). `CONTROL_DENSE` / `CONTROL_STANDARD` sites in J's files: re-pointed to 0.
- **Widgets consumed** (spec C, `widgets/kit/`, new names there): `Dialog/CommandPalette`, `Dialog/Modal` (`kit::surface::modal_surface`), `Popover/Panel` (`popover_surface`), `Menu/Context` (`menu_surface`), `Menu/Item`, `Menu/Shortcut`, `Menu/Separator`, `Toast/Default` (for D), `Tooltip/Default`, `Input/Search`, `Input/Field`, `Tabs/Segmented`, `List/Row`, `Button/Primary|Secondary|Tertiary|Destructive|Icon|Close`, `Badge/StatusCount`. C names these J duplicates to retire: `comments/ui.rs:92`, `:117`, `:133`; `home/dialog.rs:277-280`, `:339-341`, `:367-410`; `cmdk.rs:357-385`, `:450-474`, `:468-471`; `find/panel.rs:158-177` (C section 5.1).
- **Rust surface.** `cmdk::show`, `home::dialog::show`, `history::*::show`, `find::panel::show`, `comments::*::show`, `present::show` keep their signatures. The private `CMDK_*` and `UI3_*` constants in `cmdk.rs` and `updater.rs` are deleted (0 uses outside their files).

## 6. File-format mapping

None.

## 7. UI

UI implementers load the `es-designer` skill first and complete its checklist; the project's design system and the token contract win over the skill's defaults. Evidence goes in the PR (not Figma, R3). Responsive: SKIPPED (owner default); capture at 1600×1088.

**Table 1: colours** (the Studio constant is the name that goes into the store and the code, R14; the Untitled role in brackets is a reference only; hex is the contract's dark value):

| Element | Today | Studio constant (contract; Untitled reference in brackets) | Hex |
|---|---|---|---|
| Command bar, popover, menu surface | `#2C2C2C` / `BG_MENU` | `BG_MENU` (bg-primary) | `#0A0A0A` |
| Command bar, popover, menu outline | `#383838` / `BORDER_MENU` | `BORDER_MENU` (border-secondary_alt), 1 px | `#262626` |
| Dialog surface (`Dialog/Modal`) | `BG_MENU` + shadow and ring (`menu_panel`) | `BG` (kit Modal bg-primary), **no border** (live kit, rule 3) | `#0A0A0A` |
| Command bar row hover / selected | `#383838` | `BG_MENU_HOVER` / `BG_SELECTED` | `#171717` / `#262626` |
| Command bar tab underline | `Color32::WHITE` | `BORDER_SELECTED` (border-brand) | `#FB923C` |
| Command bar tab label current / other | `WHITE` / `TEXT_SECONDARY` | `TEXT` / `TEXT_QUATERNARY` | `#FAFAFA` / `#A3A3A3` |
| Command row title / selected | `TEXT_SECONDARY` / `WHITE` | `TEXT_SECONDARY` / `TEXT` | `#D4D4D4` / `#FAFAFA` |
| Command row icon / selected | `ICON_SECONDARY` / `WHITE` | `ICON_TERTIARY` / `ICON` | `#A3A3A3` / `#FFFFFF` |
| Command tag badge | `#383838` + `#B3B3B3` | gray `Badge/StatusCount`: `BADGE_NEUTRAL_BG` + `BADGE_NEUTRAL_BORDER` + `BADGE_NEUTRAL_TEXT` | `#171717` + `#404040` + `#D4D4D4` |
| Dialog title / body | `TEXT` / `TEXT_SECONDARY` | `TEXT` / `TEXT_TERTIARY` | `#FAFAFA` / `#A3A3A3` |
| Primary button | `BG_BRAND` + `WHITE`; updater `UI3_BLUE` + `WHITE` | `BG_BRAND` / `_HOVER` / `_PRESSED` + `TEXT_ON_BRAND` | `#E36323` / `#F97316` / `#EA580C` + `#0A0A0A` |
| Destructive button | `BG_BRAND`/`DANGER` variants (`home/dialog.rs:371-372`) | `BG_ERROR_SOLID`, hover `BG_ERROR_SOLID_HOVER` (R8.7) + `TEXT_WHITE` | `#DC2626` / `#B91C1C` + white |
| Secondary button | `BG_TERTIARY` + `BORDER` | `BG` + 1 px `BORDER_STRONG` (E-2, plan.md C-F), label `TEXT_SECONDARY`; hover `BG_HOVER` + `TEXT_SECONDARY_HOVER` | `#0A0A0A` + `#404040` |
| Modal scrim | black α140 / α140 / α150 | `BG_OVERLAY` (bg-overlay at 70 %), no blur (rule 3) | `#262626` @ 70 % |
| Context-menu hover label | `Color32::WHITE` | `TEXT_SECONDARY_HOVER` | `#E5E5E5` |
| Menu shortcut | `TEXT_TERTIARY` / `MENU_SHORTCUT_HIGHLIGHT` on the lit row | `TEXT_QUATERNARY` / `MENU_SHORTCUT_HIGHLIGHT` | `#A3A3A3` / `#E5E5E5` |
| Find field, command-bar input, comments search / Find toggle selected | theme field look | fill `BG` (kit Input bg-primary, plan.md C-F) + 1 px `BORDER_STRONG` (E-2), placeholder `TEXT_PLACEHOLDER` (E-1), focus `FOCUS_RING`; toggle on `BG_SELECTED` | `#0A0A0A` + `#404040`, `#737373`; `#262626` |
| Comments filters (`Tabs/Segmented`) | `comments/ui.rs:133-152` look | C `Tabs/Segmented`: rest no fill + `TEXT_QUATERNARY`, hover `TEXT_SECONDARY`, selected `BG_SELECTED` + 1 px `BORDER_STRONG` + `TEXT_SECONDARY` | `#A3A3A3`; `#D4D4D4`; `#262626` + `#404040` |
| Find match highlight | `FIND_HIGHLIGHT`, `_SELECTED` | unchanged (contract (e)) | `#0D99FF` α 0x59, `#7CC4F8` α 0x66 |
| Comment card / hover | theme | `BG_SECONDARY` + `BORDER` / `BG_SECONDARY_HOVER` | `#171717` + `#262626` / `#262626` |
| Comment avatars, resolved pin | `AVATAR_COLORS`, `COMMENT_PIN_RESOLVED` | unchanged (identity, canvas overlay) | — |
| History row hover / selected | theme | `BG_HOVER` / `BG_SELECTED` | `#171717` / `#262626` |
| Presentation stage | `PRESENT_BG #09090B` | `PRESENT_BG` (contract: bg-primary) | `#0A0A0A` |
| Presentation hairlines | white α18 | `BORDER` | `#262626` |
| Hotspot reason pill | black α190 + `WHITE` | tooltip: `BG_PRIMARY_SOLID` + `TEXT` | `#262626` + `#FAFAFA` |
| Device bezel, texture tints | `BLACK`, `WHITE` tint | unchanged (content, allowlist) | — |
| Updater surfaces | `UI3_BG_PANEL/CARD/HOVER`, `UI3_BORDER` | `BG_MENU` / `BG_SECONDARY` / `BG_SECONDARY_HOVER`, `BORDER_MENU` | `#0A0A0A` / `#171717` / `#262626`, `#262626` |
| Updater status | `UI3_EMERALD`, `UI3_AMBER`, `UI3_RED`, `UI3_BLUE` | `SUCCESS`, `WARNING`, `DANGER`, `BG_BRAND` (actions) / `INFO` (info dot) | `#22C55E`, `#EAB308`, `#EF4444`, `#E36323` |
| Toast (D paints, rules 12–15) | `BG_MENU` + elevation shadow, dot, action label `TEXT_BRAND`, action hover `BG_HOVER` | `BG_PRIMARY_ALT` + `BORDER_MENU`, no shadow, radius 12; dot `ICON_BRAND_PRIMARY` / `DANGER` / `SUCCESS`; action label `TEXT_SECONDARY` → `TEXT_SECONDARY_HOVER`, hover fill `BG_SECONDARY_HOVER` | `#171717` + `#262626`; `#F97316` / `#EF4444` / `#22C55E`; `#D4D4D4` → `#E5E5E5` on `#262626` |

**Table 2: type** (B's constants): editor surfaces (command bar rows per C use text-sm medium; Find, comments, history, context menu) rows `FONT_SIZE_BODY` 12 / `LINE_BODY` 18, xs button labels text-sm 14 semibold 600 (`FONT_SIZE_TEXT_SM`, R8.6), captions `FONT_SIZE_CAPTION` 12 / 18, panel and popover titles `FONT_SIZE_LARGE` 14; dialogs: title 16 semibold (`FONT_SIZE_TEXT_MD`), body 14 (`FONT_SIZE_TEXT_SM` / `LINE_TEXT_SM` 20); toast text-sm 14 medium; weights 400 / 500 / 600.

**Table 3: sizes** ("Δ" at 1600×1088):

| Element | Today | After | Δ |
|---|---|---|---|
| Command bar width / anchor | 440, above the toolbar | same | 0 |
| Command bar input row | 40 | 40 (region; input sm 36 inside) | 0 |
| Command bar tab row | 34 | 34 | 0 |
| Command bar section heading | 28 | 28 | 0 |
| Command bar item pitch | 32 (row 30) | 36 (row 32, C's R5 pitch) | +4 per row |
| Command bar frame radius | 6 | `RADIUS_XL` 12 (live kit `Command bar`) | — |
| Dialog padding | 20 | `INSET_MODAL` 24 | +8 height, content 8 narrower |
| Dialog footer buttons | 32 | `CONTROL_MD` 40 | +8 |
| Dialog fields (history modal) | 32 | `CONTROL_SM` 36 | +4 per field |
| Dialog picker rows (`home/dialog.rs` ROW_H) | 28 | 36 (`List/Row` sm) | +8 per row |
| Presentation link prompt (card, rule 9) | 56 high, buttons 24 | 60 high, buttons `CONTROL_SM` 36 | +4 card (top fixed, bottom gap 32 → 28), +12 buttons |
| Context-menu rows | 24 | `MENU_ITEM_HEIGHT` 32 | +8 per row |
| Find field | 28 | `FIND_FIELD_H` 32 | +4 |
| Find Replace buttons | 24 | `CONTROL_XS` 32 | +8 |
| Find result group rows | 28 | `FIND_GROUP_ROW_H` 32 | +4 per row |
| Comments filters / search | 24 / 28 | `COMMENT_FILTER_H` 24 (today's height, plan.md C-K) / `CONTROL_XS` 32 | 0 / +4 |
| Thread popover buttons / icon buttons | 24 / 20 | 32 / 32 hit | +8 / +12 hit (header 36 unchanged) |
| History rows / day header / banner | 44 / 28 / 40 | same | 0 |
| History banner buttons | theme sizes | `CONTROL_XS` 32 | ≤ +8 |
| Toast | 40 high, bottom 72 | same | 0 |
| Radii | 6 / 14 | controls `RADIUS_CONTROL` 8; dialogs, menus and popovers `RADIUS_FRAME` 16 with items 8 at inset 8 (R8.5, kit Modal); command bar and toast `RADIUS_XL` 12 | — |

**States**: default, hover, pressed (`BG_PRESSED`), focused (`FOCUS_RING`), disabled (50 % opacity, C rule S5; dialog OK disabled keeps `TEXT_ON_BRAND` at 50 %, not `TEXT_DISABLED` on orange), selected (`BG_SELECTED`). Empty states: command bar "no results" and comments "no comments" use C's `EmptyState/Default` text roles (`TEXT`, `TEXT_TERTIARY`), no new illustration.

## 8. Edge cases

1. **Short window** (900×600 minimum): the command bar grows 4 px per row; its existing max-height and scroll logic (`cmdk.rs:343` sums rows) must still keep it above the toolbar; check with 12 rows at 900×600.
2. **Long dialog text** wraps inside the 440 width minus 2 × 24 padding (8 px narrower than today); the measurement at `home/dialog.rs:254` uses `TEXT` instead of `WHITE` (same metrics, colour does not affect layout).
3. **Danger dialogs** (`d.danger`, `home/dialog.rs:371-372`): Destructive hierarchy, `TEXT_WHITE` label at 4.83:1 on `BG_ERROR_SOLID` (contract).
4. **Toast while a dialog is open:** the toast sits above the scrim as today; no z-order change.
5. **Find with 0 results:** status row (`FIND_STATUS_H` 32) shows `TEXT_TERTIARY`; Replace buttons disabled at 50 %.
6. **Thread popover near the window edge:** it flips as today; the 8 px taller buttons can push its bottom past the edge only if the composer is at max 160; the existing clamp must hold (check in the PR).
7. **Updater hidden:** Z14 must stay OK; its restyle is verified by the census and a unit test, not a screenshot.
8. **Comment avatars** on `BG_SECONDARY` cards: identity colours keep their contrast checks; initials stay `TEXT_ON_BRAND`-style dark only where today's code uses a dark label (no change).
9. **Presentation of a light prototype:** stage `#0A0A0A` instead of `#09090B` (1-step change); bezel and the computed document colours (`present.rs:665`, `:670`, `:766`) unchanged.
10. **Design Gate tab in the command bar** shows I's status colours through the terminal aliases; when I lands first, the audit rows change colour without a J edit.

## 9. Non-goals

- `panels/menus.rs` (menu bar and its command palette, D) and `shell.rs` (D paints the toast).
- Canvas comment pins, avatar identity colours, Find canvas match colours (`FIND_CANVAS_MATCH*`), presentation content.
- Unhiding the updater (Z14), any change to command behaviour, search ranking, Find logic, comment model, history data.
- Light mode, icon set (R6), Figma (R3).

## 10. Test plan (red first)

New file `crates/designos-app/src/shell/tests/overlays_chrome.rs` (new name), registered with one `mod overlays_chrome;` line in `crates/designos-app/src/shell/tests.rs` (`:1124-1201`); it uses the shared `chrome_colour_sites` helper from `shell/tests/chrome_census.rs` (new name, spec H section 10: an exact port of `scripts/rust_literal_scan.clean` that blanks each `#[cfg(test)]` item and keeps the shipped code after it, which matters here because `cmdk.rs:86` and `present.rs:123` hold early test items; it returns raw sites, and J's test removes its own allowlist by code text) and the recorded-rect hook (`widgets::record`, `widgets/mod.rs:254`) through `Sim::rect_of` (`shell/tests.rs:202`).

| Test | Asserts | Red at `8dfaee93` |
|---|---|---|
| `overlay_sources_paint_no_chrome_colour_literals` | `chrome_colour_sites` over J's 23 files, minus the `present.rs` allowlist (two texture tints via `texture_tint_line`, the device bezel by code text), returns nothing | yes (35, the same lines as section 2) |
| `overlay_surfaces_draw_no_shadow` | cleaned source of J's files has no `elevation_shadow(`, `elevation_400(` or `menu_panel(` call (today `cmdk.rs:353`, `panels/context.rs:190`, `present.rs:826`, `:908`, `home/dialog.rs:277`, `history/modal.rs:36`, `comments/thread.rs:47`); dialogs call `modal_surface`, popovers and menus `popover_surface` / `menu_surface` | yes (7 calls) |
| `cmdk_palette_uses_contract_roles` | `cmdk.rs` has no `CMDK_` constant; the recorded command rows (`cmdk-row-*`, add `widgets::record` where missing) are 32 high at a 36 pitch | yes |
| `dialog_footer_buttons_are_control_md` | open a confirm dialog in the `Sim` harness; OK and Cancel rects are `CONTROL_MD` high; dialog padding 24 | yes (32, 20) |
| `present_link_prompt_is_a_card` | `Sim` in presentation with a pending link: `present-url-open` and `present-url-cancel` are `CONTROL_SM` 36 high; the card is 60 high with its top 88 px above the stage bottom; no scrim is painted | yes (24, 56) |
| `brand_labels_are_dark` | `kit::button_look(Hierarchy::Primary, State::Default).text == theme::TEXT_ON_BRAND` and no `Color32::WHITE` label in J files (census) | yes |
| `context_menu_rows_are_32` | right-click a layer; the recorded context rows are `MENU_ITEM_HEIGHT` (32) | yes (24) |
| `find_controls_follow_r5` | `find/panel.rs` field 32, Replace buttons 32, group rows 32 (recorded rects) | yes (28 / 24 / 28) |
| `comment_controls_follow_r5` | comments search 32; thread popover buttons 32; the two filters `comments-filter-pages` and `comments-filter-status` stay `COMMENT_FILTER_H` 24 (guard, plan.md C-K) | yes (search 28, buttons 24) |
| `updater_stays_hidden` | `UPDATER_UI_ENABLED == false` and `updater.rs` census 0 | yes (census 11) |
| `toast_rules_for_d` | (lands with D) C's `Toast/Default` look as D paints it, for both actions (`toast-reveal`, `toast-enter-path`): radius `RADIUS_XL`, fill `BG_PRIMARY_ALT`, border `BORDER_MENU`; dots `ICON_BRAND_PRIMARY` / `DANGER` / `SUCCESS`; action label `TEXT_SECONDARY`, hovered `TEXT_SECONDARY_HOVER` on `BG_SECONDARY_HOVER` (never `BG_HOVER`); action target `CONTROL_XS`; no shadow call in `shell.rs:1087-1160` | yes (`RADIUS_FRAME`, shadow, `BG_HOVER`, `TEXT_BRAND`) |
| `import_path_prompt_follows_dialog_rules` | `Sim` with `App::open_import_path_dialog()`: the `ImportPath` prompt is a `Dialog/Modal` (`BG`, no border, padding 24, footer buttons `CONTROL_MD`), its field hint is `TEXT_PLACEHOLDER` | yes (`TEXT_TERTIARY`, 32 / 20) |

Keep green: `find/panel.rs` tests (`:432`), `cmdk.rs` tests, `shell/tests/find.rs`, `shell/tests/comments.rs`, `shell/tests/history.rs`, `shell/tests/menus.rs`.

Commands (repo root): `cargo test -p designos-app --bin designos overlays_chrome`; `cargo test -p designos-app --bin designos find comments history cmdk`; `cargo test --workspace --lib`; `python3 scripts/zero_fake_ui_gate.py` (Z08, Z14, Z15 OK); `python3 scripts/check_spacing_relationships.py` (exit 0); `python3 scripts/design_os_self_evolve.py`.

**Screenshot evidence.** Captures run only in L's hermetic mode, through L's capture script (`scripts/layout_baseline_capture.py`, `--only-spec J`): per case a fresh run dir, `env -i`, scratch `HOME` and `TMPDIR`, a library seeded with `designos --seed-library <run>/lib` and passed as `DESIGNOS_LIBRARY`, `DESIGNOS_AGENT_CLI_DIR=<stubs>`. Before = L's baseline blessed on the branch base; after = the same cases on the PR build; both sets, the colour and wireframe sheets and `invocations.log` go in the PR. The file argument is the seeded library's "Design system" file, which L's case `args` name through `{LIB}`. Never run a capture against the owner's library: the repo is public. Planning-round CLI runs use `tools/designos-latest` (v0.7.0, the controller's build for this round; the 0.6.2 pin of plan.md 22:40 is superseded); no J capture was run in fix rounds 2 and 3.

| Id | `<args>` (after the seeded file) | L case |
|---|---|---|
| J-01 | `--panel cmdk` | L's case for these args (id per `cases.json`) |
| J-02 | `--find Button` | same |
| J-03 | `--find Button --replace-with Action` | same |
| J-04 | `--present` | same |
| J-05 | `--panel version-history` | same |

**Not isolated, withdrawn.** The spec writer's 2026-10-25 runs of these five commands (a build of `ec47da26`) used the setup `--seed-library --library "$LIB"`, which seeds `./--library` because `--seed-library` takes the next argument as its directory (`cli.rs:364`), with `$LIB` never assigned, and ran on the host `PATH` and `HOME` without `DESIGNOS_LIBRARY`. Their exit codes and PNGs are not PR evidence.

Not reachable from the CLI: confirm and picker dialogs, the history modal and banner, the context menu, comment panel and thread, the presentation link prompt and variables panel, and the toast (L does not gate toasts; every capture uses `--no-toast`). Each is rendered from injected state in L's `Sim` tier with the same scratch library, or listed in the PR as "not captured — covered by test <name>" (section 10 tests). A manual capture, if any, uses the same scratch library and `DESIGNOS_LIBRARY`, and is labelled manual. The updater is hidden (Z14) and is verified by tests only.

**Acceptance criteria**

1. The census prints `chrome_sites=0` for every J file except `present.rs`, which prints only the 3 allowlisted content sites (2 texture tints, device bezel). On the branch base the shared helper prints the same 35 lines as section 2 (spec H acceptance 9, helper/Python parity), and `overlay_sources_paint_no_chrome_colour_literals` fails with them.
2. The tests above pass (the toast test lands with D) and the listed existing tests stay green.
3. `grep -rnwE 'CONTROL_(DENSE|STANDARD)|FIND_BUTTON_H|COMMENT_BTN_H|COMMENT_ICON_BTN|elevation_shadow|elevation_400|menu_panel' <J files>` prints nothing (`COMMENT_FILTER_H` stays, rule 7).
4. `J-01`..`J-05` after-shots (hermetic, L's script) show contract colours, no white label on orange, no white tab underline, the command bar at the same anchor and width with radius 12 and a 1 px `BORDER_MENU` edge; L's layout gate passes for these captures.
5. Z08, Z14, Z15 stay OK; `check_spacing_relationships.py` exits 0.
6. The PR lists re-anchored lines, the before/after census and which surfaces were captured by hand.
7. Contrast: every text/background pair J paints is a row of token-contract.md section (f) "Contrast check C-1" and its snippet exits 0 (`ui ds a11y` is not a gate: recorded false green). J's rows: C-1.1, 1.2, 1.3, 1.5, 1.7, 1.8, 1.14–1.20 (brand, destructive and tooltip labels), 1.24 (gray tag), 1.29 (focus ring), and J's own rows C-1.52 (`TEXT_SECONDARY_HOVER` on `BG_MENU_HOVER`, hovered menu row), C-1.53 (`TEXT_QUATERNARY` on `BG_MENU`, shortcuts and other tabs), C-1.54 (`TEXT` on `BG_PRIMARY_ALT`, toast), C-1.55 (`TEXT_TERTIARY` on `BG_MENU`, dialog body and captions). Named exceptions, which are not C-1 passes: E-1 (placeholder `#737373` in the command-bar, Find, comments-search and dialog fields, R8.1) and E-2 (1 px `BORDER_STRONG` `#404040` on those fields and on secondary buttons, R10.7 and plan.md C-F; field fill `BG`, the kit Input's bg-primary). Computed here for the toast (rule 13; listed for W1): action label `TEXT_SECONDARY` on `BG_PRIMARY_ALT` 12.09:1, hovered `TEXT_SECONDARY_HOVER` on `BG_SECONDARY_HOVER` 12.01:1, dots 6.40 / 4.76 / 7.87:1 (non-text).

## 11. Performance budgets

Command bar: rows are painted per frame while open; replacing private constants and adding a `Look` lookup per row adds no allocation (pure fns over constants). Removing the shadow draw calls (command bar, dialogs, layer picker, presentation panel and link prompt, thread popover, toast) lowers tessellation. Idle CPU unchanged (0.00 %); no new repaint requests. Frame time with the command bar open at 20 rows must not rise by more than 0.5 ms.

## 12. Dependencies, ownership and build order

**Build order.** L captures baselines (`J-01`..`J-05`) → A (contract constants, including N14–N37 and `RADIUS_XL`) → B (sizes, `MENU_ITEM_HEIGHT`, `FIND_*`) → C (`widgets/kit/`, including `Dialog/CommandPalette` with a radius-12 bordered surface, `Dialog/Modal`, `Toast/Default`, `modal_surface`, `popover_surface`, `menu_surface`, `toast_surface`) → this spec; D applies J's toast rules (12–15) and builds its `panels/menus.rs` palette from rule 2 in the same wave (J owns both anatomies, D applies them). I may land before or after J: the command bar's Design Gate section reads I's aliases either way.

**Technical decisions recorded here** (the owner may reverse): (1) the command bar input row stays 40 and holds C's sm 36 input (R1 forbids shrinking the region to C's 36 row); (2) history rows stay 44 (two-line rows; R5's 32 would shrink them); (3) device bezel and texture tints are content; (4) J re-points its call sites from three theme size constants instead of asking B to change their values, so the contract stays as written; (5) the dialog background blur is not drawn (rule 3); (6) the command bar keeps a 1 px `BORDER_MENU` edge although the kit's `Command bar` has no stroke, because Studio has no backdrop overlay behind it (rule 2); (7) the presentation link prompt is a card, not a modal (rule 9).

**File ownership across H–K** (identical table in I, J and K; H gives the same assignments with a note per file):

| File | Owner |
|---|---|
| `home/mod.rs`, `home/thumb.rs`, `home/time.rs`, `home/session.rs`, `home/leave.rs` and `home/unsaved.rs` (new on `origin/main`, no painting), `home/sidebar.rs` and `home/main_view.rs` (deleted), `shell/tests/home.rs` | H |
| `terminal.rs`, `agent_*.rs` (30 top-level files at `8dfaee93`: 26 at `205c72ef` plus `agent_approval_channel.rs`, `agent_hook_outputs.rs`, `agent_project_config.rs` and one test file, none of which paints; plus any `agent_*` module added later), `gemini_dialog.rs`, `gemini_screen.rs`, `gemini_status_hooks.rs` (no paint), `canvas/agent_banner.rs`, `canvas/agent_changes_chip.rs`, `canvas/agent_overlay.rs`, `skills/*`, `image_jobs.rs`, `shell/tests/agent_chrome.rs` (new) | I |
| `cmdk.rs`, `updater.rs` (controller ruling), `present.rs`, `panels/context.rs`, `home/dialog.rs`, `history/*`, `find/mod.rs`, `find/panel.rs`, `find/select.rs`, `comments/*`, `shell/tests/overlays_chrome.rs` (new) | J |
| `panels/assets.rs`, `variables/*`, `libraries/*`, `styles.rs`, `plugins/*`, `panels/check_designs.rs` (controller ruling), `shell/tests/assets_variables_libraries_chrome.rs` (new) | K |
| `shell/tests/chrome_census.rs` (new; the `chrome_colour_sites` helper of spec H section 10; whichever of H–K lands first adds it exactly as H defines it) | H |

Not mine: `panels/menus.rs` (D, including its command palette `:907-1085`: agreed with D), `shell.rs` (D; J writes the toast rules, D paints them), `tabs.rs`, `panels/rail.rs`, `panels/toolbar.rs`, `canvas/rulers.rs` (D); `panels/left.rs`, `panels/layers.rs`, `find/filter.rs` (E: the Layers filter field); `panels/right.rs`, `panels/properties.rs`, `panels/inspect.rs`, `widgets/picker.rs`, `panels/style_picker.rs`, `panels/style_edit.rs`, `panels/style_edit_grid_rows.rs` (F); `dev/*`, `specs.rs` (G); `theme.rs`, `design/*`, `scripts/*` (A, B); `widgets/*`, `widgets/kit/*` (C). Spec D's not-mine list now says `updater.rs` is J's (it is the module `crate::home::updater`; there is no `home/updater.rs`); `shell/toast_layout.rs` is D's (D table), with J writing the toast anatomy; uncovered by any spec: `canvas/proto.rs` (4 chrome sites, the prototype-flow overlay) [F or L], `lint.rs` (`:297`, non-chrome) [K]. `shell/tests.rs` is L's (`Sim`, `:69-154`; `Sim::rect_of` `:202`): each H–K PR only appends one `mod` line to its module list (`:1124-1201`). `scripts/spacing_baseline.json` is a shared ratchet: each surface PR may only lower its own files' counts with `--write-baseline`.

Cross-spec conflicts, now ruled by the controller: (a) Find highlight: `FIND_HIGHLIGHT` stays as the contract has it (E follows). (b) Radius, plan.md C-K (22:35): "dialogs/modals = kit Modals; command bar = kit Command menu; toasts = kit Notification; menus/dropdowns/popovers 16 (R8.5)", so dialogs 16, command bar 12, toast 12, menus and popovers 16, each with its receipt (rules 2, 3, 14). C rows 75 and 144 still give the command palette 16; the coordinator has W2 change them to 12, and D's toast row 96 is re-synced to rules 12–15 (W3); both lists are in this round's report. (c) The command bar input row stays 40; C aligns with J. (d) `updater.rs` is J's (D's not-mine list said H). (e) Comments filters keep their height 24, plan.md C-K: "Segmented controls outside R10.1's three strips keep today's height (R1)" (K's AA/AAA follows the same line).

**Open PRs touching J files** (`gh pr list --state open`, 2026-10-28, 57 open; file lists from the same call): only the stale #765 (`CONFLICTING`), #768 and #770 (opened 2026-10-19; `cmdk.rs`, `home/dialog.rs`, `panels/context.rs`, `comments/*`, `history/*`) touch J files; no live feature PR does. #1206 and #1207 (L's and A's own draft PRs) touch other surfaces. Rebase on whatever lands first; re-run the census after each rebase.

## 13. Owner rulings that apply

- R1: "sử dụng 100% style kiểu của Untitled UI nhưng vẫn giữ cấu trúc layout và bố cục hiện tại" (all of Untitled's style; keep layout and arrangement).
- R2: take Untitled spacing and typography as they are, adapted to Studio's layout.
- R3: Studio improves itself; no Figma. Evidence goes in the PR.
- R14 (owner, 2026-10-28 23:05): Studio's design store (tokens, registry, soul) uses Studio's own names and values only. J follows it: the toast and command-bar anatomy it writes for D and C is named by Studio constants and C's widget names (`Toast/Default`, `Dialog/CommandPalette`); the kit's frame and component names (`Notification`, `Command bar`, `Modal`) appear only as receipts and references.
- plan.md C-R (controller, 22:55): "command bar and toast radius 12 `RADIUS_XL` (live kit `Command bar` 3307:407031 and Notification are radius-xl; C's \"palette 16\" is wrong); J owns toast and command-bar anatomy, D applies it; toast action hover = the `#262626` fill (J's `BG_HOVER` is invisible on the `#171717` toast)" (rules 2, 12–14; receipts in rules 2, 3 and 14).
- R4: brand = Untitled anatomy with Studio orange `#E36323`, dark label `#0A0A0A`; focus ring `#F97316`; Purple Ban stays.
- R5: editor surfaces text-xs 12/18, button xs 32, row 32, radius md 8; dialogs text-sm 14/20, buttons sm 36 / md 40.
- R6: keep the icon set; restyle colour to Untitled fg roles.
- R7: spec-driven; the PM coordinates delivery.
- Controller defaults: canvas overlays (comment pins, Find canvas match) keep their colours; chrome status colours move to Untitled.
- plan.md C-F (controller, 22:30): "Field fill = kit Input `bg-primary`; E-2 covers every 1 px kit `border-primary` `#404040` control boundary: unchecked checkbox and radio (R10.7), text field and select, secondary button".
- plan.md C-K (controller, 22:35): "Radii and variants follow the live kit (R2), each with a recorded read-only receipt: dialogs/modals = kit Modals; command bar = kit Command menu; toasts = kit Notification; menus/dropdowns/popovers 16 (R8.5). Segmented controls outside R10.1's three strips keep today's height (R1). Controls inside a tab bar match its tabs (contract (d); agent header "Sessions", `+`, Skills, Stop stay 24). Home "Empty trash" uses the kit's secondary-destructive button (C adds it)".
- plan.md (owner, 22:40, superseded 2026-10-28): the planning round first pinned the 0.6.2 build; fix round 3 uses `tools/designos-latest` (v0.7.0), and code citations are at `origin/main@8dfaee93`.
- Coordinator (fix round 2): J owns the toast and command-bar anatomy and D applies it; radius 12 for both.
- Standing: `main` is protected (PR, green macOS test, Clippy, Rustfmt, squash); visual evidence in every PR (2026-09-22). Z14: the updater stays hidden.

## Open questions (all decided)

None remain for J. Decided since the first draft:

- **OQ1 · Command bar item height** — decided: pitch 36 (row 32). Source: plan.md, controller defaults from the 21:35 sweep: "Command bar item pitch 36 (row 32; kit command item 44 at the small end), the bar may grow, never shrink (R1) — J OQ1".
- **OQ2 · Dialog footer buttons** — decided: md 40. Source: controller default "Dialog footer buttons md 40 (C `Dialog/Modal`, R5 allows sm/md) — J OQ2".
- **OQ3 · Frame radius** — decided: dialogs 16 (`RADIUS_FRAME`), command bar and toast 12 (`RADIUS_XL`). Source: plan.md C-K "dialogs/modals = kit Modals; command bar = kit Command menu; toasts = kit Notification", with the receipts in rules 2, 3 and 14 (Modal `4057:415360` radius-2xl 16; Command bar `3307:407031` and Notification `1135:2645` radius-xl 12).
- **Review OQ3 · Comments filter height** — decided: stays 24. Source: plan.md C-K "Segmented controls outside R10.1's three strips keep today's height (R1)".
- **OQ4 · Find match highlight** — decided: `FIND_HIGHLIGHT` info-blue. Source: controller default "Find highlight stays `FIND_HIGHLIGHT` info-blue per the contract — J OQ4, E decision 2".

## Changelog (fix round 2, curator review `reviews/HIJK-curator-review.md`, and the D–G review amendment)

- J-1 and coordinator amendment 2a: toast action hover `BG_SECONDARY_HOVER` `#262626` (C S6) instead of `BG_HOVER` `#171717`, which equals the toast fill; ratios shown: rest label 12.09:1, hovered label 12.01:1, hover fill 1.18:1 from the toast (rule 13).
- J-2 and amendment 2b: rules 12–15 are one complete toast rule (radius `RADIUS_XL` 12, `BG_PRIMARY_ALT`, `BORDER_MENU`, no shadow, dots `ICON_BRAND_PRIMARY` / `DANGER` / `SUCCESS`, `Button/Tertiary` xs `TEXT_SECONDARY` → `TEXT_SECONDARY_HOVER` on `BG_SECONDARY_HOVER`); J owns it, D applies it; `toast_rules_for_d` asserts the same names (no more `BG_SECONDARY`); D row 96 changes listed for W3.
- J-3 (plan.md C-K): command bar radius 12 with its live-kit receipt (set `3307:407570`, variant `3307:407212`, frame `3307:407031`); receipts also for the Modal (`4057:415360`) and the Notification (`1135:2645`), recorded in rules 2, 3 and 14 (node id, property, value); section 12 (b) cites C-K; C rows 75 and 144 listed for W2.
- J-4 (plan.md C-K): comments filters stay `COMMENT_FILTER_H` 24 (two filters, `comments/panel.rs:44`, `:51`); table 3, test and acceptance 3 updated.
- J-5: helper wording refers to H's exact port of `rust_literal_scan.clean`; acceptance 1 adds helper/Python parity (35 lines).
- J-6: captures only through L's hermetic script with a seeded scratch `DESIGNOS_LIBRARY`; the misparsed `--seed-library --library "$LIB"` setup and its exit-code table withdrawn as not isolated; uncapturable states from `Sim` or "not captured — covered by test <name>".
- J-7: dialogs `BG`, no border (table 1 row split); command bar, popovers and menus keep `BORDER_MENU`. The command bar keeps its border although the kit bar has none, because Studio draws no overlay behind it (recorded deviation, decision 6).
- J-8: the presentation link prompt leaves rule 3; rule 9 makes it a `Popover/Panel`-style card (`BG_MENU` + `BORDER_MENU`, 16, sm 36 buttons, no scrim), 56 → 60 high; test `present_link_prompt_is_a_card`.
- J-9: every shadow and old-panel call listed for deletion: `cmdk.rs:353`, `panels/context.rs:190`, `present.rs:826`, `:908`, `home/dialog.rs:262`, `history/modal.rs:36`, `comments/thread.rs:47`, plus the toast's `shell.rs:1013` (D); test `overlay_surfaces_draw_no_shadow`, acceptance 3 grep.
- J-10: "the other 9 files" → 10; acceptance 7 cites C-1.52–C-1.55 and drops the W1 hand-off for them; blur stated once from contract N33 (Figma radius 16 = CSS `blur(8px)`).
- Review "Same edit for I, J, K": acceptance 7 cites E-1 and E-2 with plan.md C-F; fields are `BG` + `BORDER_STRONG`.
- Coordinator amendment 1: 0.6.2 build for planning-round CLI runs; code citations stay at `origin/main` `205c72ef`. C-F, C-K and the toast/command-bar ownership quoted in section 13.


## Changelog (fix round 3, 2026-10-28: re-anchor to `origin/main@8dfaee93`, plan.md C-K / C-R, R14)

Review re-check against `reviews/HIJK-curator-review.md` (each id read on the spec text):

- J-1 and C-R (toast hover): rule 13 now says the action's hover fill is `BG_SECONDARY_HOVER` `#262626` (the `#262626` fill), never `BG_HOVER` `#171717` (= the toast fill); ratios shown: label 12.09:1 at rest, 12.01:1 hovered, hover fill 1.18:1 from the toast (a state cue, not a 3:1 boundary). Kept as the brief and C-R say (Details link and reveal action both `#262626`). Code note for D in rule 13: the action has its own 24-high target with a hover fill, while the "Details" target is the whole toast with no fill today, so D narrows it or paints its hover.
- J-2: present: rules 12–15 are one complete toast rule (radius `RADIUS_XL` 12, `BG_PRIMARY_ALT`, `BORDER_MENU`, no shadow, dots, `Button/Tertiary` xs); rule 12 now also fixes the layout (R1: today's `toast_layout.rs` constants stay) and both actions; `toast_rules_for_d` covers both actions; D's row 96 already carries it.
- J-3 (plan.md C-K / C-R): present: command bar and toast radius 12 `RADIUS_XL` with the live-kit receipts in rules 2, 3 and 14 and in OQ3 (nodes `3307:407031`, `1135:2645`, `4057:415360`); no separate evidence file was created (this round writes specs only). C notes `1135:507` as a frame of another variant that binds the same radius, so the two ids do not conflict.
- J-4 (plan.md C-K): present: comments filters stay `COMMENT_FILTER_H` 24.
- J-5: present: acceptance 1 says the helper prints the same 35 lines as section 2.
- J-6: present: captures only through L's hermetic script with a seeded scratch `DESIGNOS_LIBRARY`.
- J-7: present: dialogs `BG`, no border.
- J-8: present: the presentation link prompt is a `Popover/Panel`-style card (rule 9), 56 to 60 high.
- J-9: present: all shadow and old-panel calls listed for deletion (`cmdk.rs:353`, `panels/context.rs:190`, `present.rs:826`, `:908`, `home/dialog.rs:277`, `history/modal.rs:36`, `comments/thread.rs:47`, toast `shell.rs:1134`).
- J-10: present: "the other 10 files", C-1.52–C-1.55 cited (rows exist in the recorded C-1 run, `evidence/AB-fix-round-261028/c1-contrast-check-at-8dfaee93.txt` section 1, 70/70 pass, exit 0), blur stated once from N33.
- X-1 to X-5 (cross-spec findings of the same review): answered one line each in H's fix round 3 changelog; the ownership table here matches plan.md 22:20 and D's refreshed list, and the unassigned list names `lint.rs` (K) and `canvas/proto.rs`.
- Re-anchor: every citation moved from `205c72ef` to `8dfaee93` by a line diff (`home/dialog.rs` +15, `shell.rs` callers +94 to +135, toast `:1087-1160`, `panels/left.rs` `:162-165`, `theme.rs` +8); census re-run (exit 0): 35 sites, lines moved only in `home/dialog.rs` (`:254`, `:273`, `:383`); drift note added.
- Drift that changed the surface: the toast (wrapping layout, "Enter a path…" action, action label `TEXT_BRAND` baked in `toast_layout.rs`: rules 12–13, section 2 row, test row), the `ImportPath` prompt in `home/dialog.rs` (rule 3, section 2 row, test `import_path_prompt_follows_dialog_rules`).
- R14: table 1 header and ruling say Studio constants first, kit names as references and receipts.
- plan.md C-R: J owns toast and command-bar anatomy and D applies it (agreement paragraph, rules 2 and 12–15, section 13).

## Changelog (T amendments, 2026-10-29)

From `specs/T-design-system-ontology-registry.md` section 12, block D–K:

- The `UTILITY` family names replaced by their V2 names (5.4 V2): rule 2, the colour constants consumed in section 5 and the command tag badge row. No value changed.
- Name alignment: the version-preview banner names `Banner/Inline` (inventory row and rule 8).
- Already in place: the toast cite is `shell.rs:1087-1160` (the toast function at `8dfaee93`); the stale `shell.rs:993-1034` range does not occur in J.
