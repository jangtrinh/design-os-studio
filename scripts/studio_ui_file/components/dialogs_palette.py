"""Registry `composite` Dialog/CommandPalette (page `03 Studio`, row Dialogs). Lines are the v0.7.5 tag.

Dialog/CommandPalette (Kind=CmdK, Actions x State=Default): the kit `command_bar_surface` (widgets/kit/surface.rs:71-74, `command_look` :41-44, the `Dialog/CommandPalette` row of
kit-states.json): `BG`, a `BORDER_MENU` 1 px edge, `RADIUS_XL`. Both painters are a search row, a tab row, then a list that differs per query, so the list is the Slot `Body` (default: the rows of the captures
`j-cmdk` and `j-palette`, the first selected, as both open). No part is a nested set: no Focus variant (CmdK paints no ring; Actions' `mark_focus` on tabs and rows,
menus.rs:1006, :1050, is a gap, Focus is no registry state here), no Divider/Hairline (its main is `BORDER`, the lines here are `BORDER_MENU`), no Input/Search (it has its own chrome), no
Menu/Item (the rows draw `command_row_look` and Regular text; W1c ruled they are not Menu/Item). The hairlines, placeholder, tabs, rows and badge are local frames.
* CmdK: `cmdk.rs:283-604` `show` (1 caller), 440 wide (:290), 74 of header (`HEADER_H` :271) + 28 per section (`SECTION_H`) + 36 per row (`ROW_PITCH`) + 10 (`LIST_END_PAD`, :276);
  the 4 above the first band (`LIST_GAP` :275) is inside the header's 74 + list. Search row 40 (:374) with its hairline on the last pixel (:375): `Icon/Search` 15 px centred at x 18
  (:377), the placeholder `Body / Regular` `TEXT_PLACEHOLDER` in a 365 px clipped field from x 34 (:394-404), the 26 px image button centred at right - 20 (:379-385; fill on hover only). Tab row
  34 (:419) with its hairline on row 33 (:420): tabs from x 14, each `text_width(Body Semibold) + 8` wide (:429-430), 12 apart (:452); the active tab is `Body / Semibold` `TEXT` with a 2 px
  `BORDER_SELECTED` underline inset 2 (:437-442), the rest `Body / Regular` `TEXT_QUATERNARY`. The tab widths are the capture's own boxes (j-cmdk layout.json): production measures every
  tab with the Semibold font whatever it draws, which a hug frame cannot do. Caption `Caption / Regular` `TEXT_TERTIARY` at x 16, 15 px into its band (:482); rows 32 high at x 8, 424 wide,
  `RADIUS_CONTROL`, `command_row_look` (rows.rs:57-65): selected `BG_SELECTED` + `TEXT` + `ICON`, rest no fill + `TEXT_SECONDARY` + `ICON_TERTIARY`; a 14 px icon at x 14 (:494), the title at x 32 (:505),
  the count badge ends 8 from the right (:497-502). The badge is local: Badge/StatusCount Sm pads `SPACE_FACT` (4 a side, census) and the palette pads `SPACE_GROUP` over the label (6 a side),
  and an instance cannot override padding. Gaps: the audit section (`render_audit_section`, :592-596), scrolling, hover fills (`BG_MENU_HOVER`), `Input/Search` as the field.
* Actions: `panels/menus.rs:918-1086` `palette` (1 caller), 520 wide (`PALETTE_MODAL_W` :911), 40 (`CONTROL_MD`) + 1 + 34 (`PALETTE_TAB_H`) + 1 + 22 (`PALETTE_SEC_H`) + 36 per row + `MENU_PADDING` (:952-954).
  Search row 40 (:969): `Icon/Search` 16 (`MENU_ICON_SIZE`) centred at x 20 (:970), `Icon/Dev` 14 centred at right - 20 (:971), the placeholder centred in a field from x 36 (:972-982); its hairline is the
  next pixel row, y 40 (:990), then the tab row at y 41 (:993): pills from x 12, `text_width(Body Regular) + 20` wide (the capture's boxes) and 24 high, centred, 6 apart (:994-997, :1013); the active
  pill is `TEXT` with a 2 px `BORDER_SELECTED` line on its bottom edge (:1001), the rest `TEXT_QUATERNARY`, every tab `Body / Regular`; a hairline at y 75 (:1015). Caption `Caption / Regular` at x 16
  in the 22 px band (:1018-1020). Rows are 36 high (`MENU_ITEM_HEIGHT` + 2 x `INSET_SEGMENT`, :952) with the pill inset (`MENU_PILL_INSET`, `INSET_SEGMENT`; 504 x 32, `RADIUS_CONTROL`, :1044-1046) behind the
  text: selected `BG_SELECTED` + `TEXT`, rest `TEXT_SECONDARY`; label `Body / Regular` at x 16 (:1058), shortcut `Body / Regular` `TEXT_QUATERNARY` `MENU_SHORTCUT_PAD` from the right (:1060-1061).
  Not Menu/Item: its label and hint are `Body / Medium` and the palette draws both Regular, and an instance cannot override a text style: local rows, a recorded deviation. Gaps: the disabled
  row (`TEXT_DISABLED`, :1051-1057), the "No matching actions" state (:1023-1031), the `first`/`shown` window, hover fills.
The default Slot holds the capture's rows (CmdK 2 sections and 5 rows, Actions 8 rows), so the default instance is the capture's 440 x 320 and 520 x 394, 1:1 (the tests lay it out with
`flow_h`, the production's own numbers).
"""
from rest_ir import F, I, T, lit

BAR_W, ACTIONS_W = 440.0, 520.0
CMDK_TABS = (("All", 23.0), ("Assets", 47.75), ("Plugins & widgets", 110.75), ("Design Gate", 78.5))  # j-cmdk.layout.json cmdk-tab boxes: text_width(Body Semibold) + 8
CMDK_ROWS = [("Recents", [("Ease Design Figma Agent", "Dev", "Development", True), ("Replace content", "Sparkle", "AI", False)]),
             ("Image editing", [("Insert example card", "Image", "AI", False), ("Boost resolution", "Sparkle", "AI", False), ("Edit image with prompt", "Sparkle", "AI", False)])]  # j-cmdk capture
ACTIONS_ROWS = [("Back to files", None, True), ("New design file", None, False), ("New board", None, False), ("New deck", None, False), ("Open...", "Ctrl+O", False),
                ("Import web capture...", None, False), ("Save", "Ctrl+S", False), ("Save as...", "Ctrl+Shift+S", False)]  # j-palette capture
ACTIONS_TABS = (("All", 34.0), ("Assets", 58.0), ("Plugins & widgets", 120.5), ("AI & Gate", 72.5))  # j-palette.layout.json palette-tab boxes: text_width(Body Regular) + 20
EDIT_LEFT = "cmdk.rs:394-395: the TextEdit rect starts at `input_row.left() + 34.0`, a raw literal"
EDIT_TOP = "cmdk.rs:394-395: the TextEdit rect starts 8 down (a raw `+ 8.0`) and its galley is 15 high (the capture's unnamed rect (34, 8, 365, 15)), so the glyph centre is 15.5; the Body line box is 18, so its top is 6.5"
TAB_LEFT = "cmdk.rs:421: `tx = tab_row.left() + 14.0`, a raw literal"
TAB_GAP = "cmdk.rs:452: `tx = tr.right() + 12.0`, a raw literal (SPACE_GROUP holds 12 for other roles)"
LIST_TOP = "cmdk.rs:275: LIST_GAP = 4.0, a local constant, the space above the first band"
LIST_END = "cmdk.rs:276: LIST_END_PAD = 10.0, a local constant, the space below the last row"
ROW_GAP = "cmdk.rs:273: ROW_PITCH = 36.0 less the row's CONTROL_XS 32: the 4 between two rows (a band takes the same 4 out of SECTION_H 28)"
CAPTION_X = "cmdk.rs:482: the caption is drawn at `rect.left() + 16.0`, a raw literal"
CAPTION_Y = "cmdk.rs:482: `row_y - SECTION_H + 15.0` centres the caption 15 px into its band, so its 18 px line box starts at 6"
ROW_TITLE_X = "cmdk.rs:505: the title is drawn at `row_rect.left() + 32.0`, a raw literal"
ROW_END = "cmdk.rs:499: the badge ends at `row_rect.right() - badge_w - 8.0`, a raw literal"
BADGE_PAD = "cmdk.rs:497: badge_w = text_width + SPACE_GROUP (12), so 6 a side; no variable holds half of SPACE_GROUP (Badge/StatusCount Sm pads SPACE_FACT, 4, so it is not nested)"
PILL = "kit/badge.rs:77 `radius_full(rect.height())` = 22 / 2: computed from the badge's height, no radius role holds it"
SEARCH_X = "menus.rs:972: the field starts at `srow.left() + 36.0`, a raw literal"
PILL_TAB_LEFT = "menus.rs:994: `tx = tab_row.left() + 12.0`, a raw literal"
PILL_TAB_GAP = "menus.rs:1013: `tx = pill_r.right() + 6.0`, a raw literal"
ACTIONS_CAPTION_X = "menus.rs:1020: the caption is drawn at `rect.left() + 16.0`, a raw literal"
LABEL_X = "menus.rs:1058: the label is drawn at `row.left() + 16.0`, a raw literal (MENU_LABEL_X_PLAIN holds 16 for the empty state, :1025)"


def line(env, name, w, y):
    """A local 1 px `BORDER_MENU` line at absolute (0, y)."""
    return F(name, fill=env.c("BORDER_MENU"), abs=(0.0, y, w, env.num("BORDER_WIDTH")))


def flow_line(env):
    return F("Line", fill=env.c("BORDER_MENU"), h=env.num("BORDER_WIDTH"), sx="fill", sy="fixed")


def underline(env, x, y, w):
    return F("Underline", fill=env.c("BORDER_SELECTED"), abs=(x, y, w, 2.0))


def tab(env, name, label, w, h, active, semibold):
    style = "Body / Semibold" if active and semibold else "Body / Regular"
    kids = [T("Label", label, style, env.c("TEXT" if active else "TEXT_QUATERNARY"))]
    if active:
        kids.append(underline(env, 2.0, h - 2.0, w - 4.0) if semibold else underline(env, 0.0, h - 1.0, w))
    return F(name, kids, dir="H", w=w, h=h, sx="fixed", sy="fixed")


def count_badge(env, label):
    return F("Badge", [T("Label", label, "Text xs / Medium", env.c("BADGE_NEUTRAL_TEXT"))], dir="H", fill=env.c("BADGE_NEUTRAL_BG"), stroke=env.c("BADGE_NEUTRAL_BORDER"),
             sw=env.c("BORDER_WIDTH"), radius=lit(11, PILL), pad=[None, lit(6, BADGE_PAD), None, lit(6, BADGE_PAD)], h=env.num("BADGE_SM"), sx="hug", sy="fixed")


def cmdk_row(env, name, title, icon, badge, selected):
    text, glyph = ("TEXT", "ICON") if selected else ("TEXT_SECONDARY", "ICON_TERTIARY")
    kids = [I("Icon", icon, 14, env.c(glyph), abs=(7.0, 9.0, 14.0, 14.0)), T("Title", title, "Body / Regular", env.c(text), sx="fill"), count_badge(env, badge)]
    return F(name, kids, dir="H", pad=[None, lit(8, ROW_END), None, lit(32, ROW_TITLE_X)], radius=env.c("RADIUS_CONTROL"), w=BAR_W - 16.0, h=env.num("CONTROL_XS"), sx="fixed", sy="fixed",
             align=("start", "center"), **({"fill": env.c("BG_SELECTED")} if selected else {}))


def cmdk(env, groups):
    """groups: [(caption, [(title, icon, badge, selected)])]. The bar hugs: 74 + 28 per section + 36 per row + 10."""
    field = F("Field", [T("Placeholder", "Search...", "Body / Regular", env.c("TEXT_PLACEHOLDER"), prop="Placeholder")], dir="H", w=365.0, sx="fixed", clip=True, align=("start", "center"))
    button = F("Image button", [I("Glyph", "Image", 14, env.c("ICON_SECONDARY"))], dir="H", abs=(BAR_W - 33.0, 7.0, 26.0, 26.0), sx="fixed", sy="fixed")
    search = F("Search row", [field, I("Search icon", "Search", 15, env.c("ICON_SECONDARY"), abs=(10.5, 12.5, 15.0, 15.0)), button, line(env, "Line", BAR_W, 39.0)], dir="H",
               pad=[lit(6.5, EDIT_TOP), None, None, lit(34, EDIT_LEFT)], h=40.0, sx="fill", sy="fixed", align=("start", "start"))
    tabs = [tab(env, f"Tab {i + 1}", label, w, 34.0, i == 0, True) for i, (label, w) in enumerate(CMDK_TABS)]
    strip = F("Tab row", [line(env, "Line", BAR_W, 33.0)] + tabs, dir="H", gap=lit(12, TAB_GAP), pad=[None, None, None, lit(14, TAB_LEFT)], h=34.0, sx="fill", sy="fixed", align=("start", "center"))
    kids, n = [], 0
    for i, (caption, rows) in enumerate(groups):
        kids.append(F("Section" if i == 0 else f"Section {i + 1}", [T("Caption", caption, "Caption / Regular", env.c("TEXT_TERTIARY"))], dir="H", pad=[lit(6, CAPTION_Y), None, None, lit(16, CAPTION_X)],
                      h=24.0, sx="fill", sy="fixed", align=("start", "start")))
        for title, icon, badge, selected in rows:
            n += 1
            kids.append(cmdk_row(env, f"Row {n}", title, icon, badge, selected))
    body = F("Body", kids, dir="V", slot="Body", gap=lit(4, ROW_GAP), pad=[lit(4, LIST_TOP), None, lit(10, LIST_END), None], sx="fill", sy="hug", align=("start", "center"))
    return surface(env, [search, strip, body], BAR_W)


def actions_row(env, name, label, shortcut, selected):
    seg, inset, row_h = env.num("INSET_SEGMENT"), env.num("MENU_PILL_INSET"), env.num("MENU_ITEM_HEIGHT") + 2 * env.num("INSET_SEGMENT")
    kids = []
    if selected:
        kids.append(F("Pill", fill=env.c("BG_SELECTED"), radius=env.c("RADIUS_CONTROL"), abs=(inset, seg, ACTIONS_W - 2 * inset, row_h - 2 * seg)))
    kids.append(T("Label", label, "Body / Regular", env.c("TEXT" if selected else "TEXT_SECONDARY"), sx="fill"))
    if shortcut:
        kids.append(T("Shortcut", shortcut, "Body / Regular", env.c("TEXT_QUATERNARY")))
    return F(name, kids, dir="H", pad=[None, env.c("MENU_SHORTCUT_PAD"), None, lit(16, LABEL_X)], h=row_h, sx="fill", sy="fixed", align=("start", "center"))


def actions(env, rows, caption="Recents"):
    """rows: [(label, shortcut or None, selected)]. The panel hugs: 40 + 1 + 34 + 1 + 22 + 36 per row + `MENU_PADDING`."""
    icon_px = int(env.num("MENU_ICON_SIZE"))
    search = F("Search row", [T("Placeholder", "Search...", "Body / Regular", env.c("TEXT_PLACEHOLDER"), prop="Placeholder"),
                              I("Search icon", "Search", icon_px, env.c("ICON_SECONDARY"), abs=(12.0, (40.0 - icon_px) / 2, float(icon_px), float(icon_px))),
                              I("Dev icon", "Dev", 14, env.c("ICON_SECONDARY"), abs=(ACTIONS_W - 27.0, 13.0, 14.0, 14.0))], dir="H", pad=[None, None, None, lit(36, SEARCH_X)],
                h=env.num("CONTROL_MD"), sx="fill", sy="fixed", align=("start", "center"))
    pills = [tab(env, f"Tab {i + 1}", label, w, 24.0, i == 0, False) for i, (label, w) in enumerate(ACTIONS_TABS)]
    strip = F("Tab row", pills, dir="H", gap=lit(6, PILL_TAB_GAP), pad=[None, None, None, lit(12, PILL_TAB_LEFT)], h=34.0, sx="fill", sy="fixed", align=("start", "center"))
    section = F("Section", [T("Caption", caption, "Caption / Regular", env.c("TEXT_TERTIARY"))], dir="H", pad=[None, None, None, lit(16, ACTIONS_CAPTION_X)], h=22.0, sx="fill", sy="fixed",
                align=("start", "center"))
    body = F("Body", [section] + [actions_row(env, f"Row {i + 1}", *row) for i, row in enumerate(rows)], dir="V", slot="Body", pad=[None, None, env.c("MENU_PADDING"), None], sx="fill", sy="hug")
    return surface(env, [search, flow_line(env), strip, flow_line(env), body], ACTIONS_W)


def surface(env, kids, w):
    look = env.look("Dialog/CommandPalette", "Default")
    return F("Palette", kids, dir="V", fill=look["fill"], stroke=look["border"], sw=env.c("BORDER_WIDTH"), radius=env.c("RADIUS_XL"), w=w, sx="fixed", sy="hug", align=("start", "start"))


def palette(values, env):
    return cmdk(env, CMDK_ROWS) if values["Kind"] == "CmdK" else actions(env, ACTIONS_ROWS)


def component_sets():
    return [{"name": "Dialog/CommandPalette", "row": "Dialogs", "axes": {"Kind": ["CmdK", "Actions"], "State": ["Default"]},
             "props": [{"name": "Placeholder", "type": "TEXT", "default": "Search..."}, {"name": "Body", "type": "SLOT"}], "build": palette}]
