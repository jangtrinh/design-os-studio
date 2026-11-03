"""Registry `region` Navigation/TitleBar, Navigation/TopTabBar and Navigation/LeftRail (page `03 Studio`, row Regions). Lines are the 0.7.5 tag of crates/designos-app/src.

Every region is sized to its rect in the capture's layout.json (`regions`: tab_bar 1440 x 32, rail 49 x 868), its children are placed where that code puts them, and the named widgets
carry the layout.json key as their node name, so the editor screen is instances only and a widget rect is found by its name. Texts are neutral (no file or project name of a kit).

Navigation/TitleBar (shell.rs:1500-1519 `title_bar`, 28 px: `TOP_TITLE_BAR_HEIGHT`, shell.rs:88): BG_PRIMARY, a BORDER hairline on its bottom row, the brand at x + 76 (the macOS window
buttons sit in the 76 px before it). The brand is the logo, then "design" (Text sm Semibold, TEXT) and ":os studio" (Regular, TEXT_SECONDARY) with no gap between the two words.
UNVERIFIED-VISUAL: the bar is drawn only when `tabs.always_visible && shot.is_none()` (shell.rs:1737) and the capture harness always has a shot, so no capture (`regions.title_bar` is null in
all 152 layout files) shows it. Gap: `Icon::Logo` is a raster brand mark (icons.rs:579-608, `icon_256.png`), no Phosphor glyph, so the logo is a 14 px disc in BG_BRAND.

Navigation/TopTabBar (Bar=32px; tabs.rs:961-1190 `show_top_tab_bar_ui`, `TOP_TAB_BAR_HEIGHT` 32): BG_PRIMARY, a BORDER hairline on row 31. The Home tab is (8, 4, 72, 24) with the home glyph
centred at +14 (14 px) and "Home" at +26 in `font_body_strong`, both in `tab_look` (widgets/kit/tabs.rs:53-61): Default is no fill, TEXT_QUATERNARY / ICON_QUATERNARY. A 1 px BORDER line
  at x 86 from
y 8 to 24 divides it from the project tabs. The project tab (the editor is open, so it is Selected) is (92, 3, 240, 26): BG_SELECTED at RADIUS_CONTROL, a 14 px file tile (RADIUS_MARK,
TILE_DESIGN_BG, an 11 px glyph in TILE_DESIGN_GLYPH) centred at +13, the title (Text xs Semibold, TEXT_SECONDARY) 6 px after it, and the 18 px close plate (RADIUS_SEGMENT, no fill at rest,
a 10 px Close in ICON_QUATERNARY) centred 13 from the right edge. The `+` button is (337, 4, 24, 24) in the Icon look with a 13 px Plus.
The strip is ONE auto layout row, as `show_top_tab_bar_ui` walks it with a running x (tabs.rs:960-1121): 8 px of inset, the Home tab (72), `x = home.right + 6` for the 1 px divider, `x += 6`
for the first tab, `x = tab.right + 3` after each tab and the `+` at `x + 2`. So the strip's gap is 3, the divider sits in a 6 px slot (the line 3 in)
  and the `+` in a 26 px slot (2 and 24): a hidden
project tab takes neither its width nor its gap, so hiding it puts the `+` at x 94 (`Show project tab` off, the Home view, capture h-home). Show
  parts, as the painter's own branches draw them: `Show Home selected` (tabs.rs:964-971
`tab_look(Selected)`: BG_SELECTED, TEXT_SECONDARY, ICON_QUATERNARY_HOVER, over the Default Home tab; its glyph is not drawn twice), `Show dev tile`
  (tabs.rs:1043-1056: the ready mark, a DEV_READY tile with a 10 px Dev glyph
in TEXT_ON_BRAND, over the file tile) and `Show unsaved dot` (tabs.rs:1081-1085: a 6 px ICON_BRAND_PRIMARY disc at tab.right - 28). Gaps: the tabs are not Tabs/Pill (the painter is
`tab_look`, which the registry's `composes=` does not name), and Button/Icon is 32 px with a 16 px glyph, so the `+`, the close plate and the glyphs here are local frames and icon instances.
The tab title is neutral and shorter than the capture's, and a title that overflows is truncated by the painter (`truncate_tab_title`): that text is not compared.

Navigation/LeftRail (Rail=48px; panels/rail.rs `show`): BG, a BORDER line on its right edge (x 48), the menu button (8, 8, 32, 32) = Button/Icon Xs with a ListView glyph, a hairline at
`RAIL_SEPARATOR_Y` (48) from x 8 to 39, six 47 x 56 cells from `RAIL_FIRST_Y` (56) whose 32 px tile is Button/Tool (File is Selected: the file panel is open), a second hairline
`RAIL_GROUP_GAP` / 2 above the cell of Variables, and the help button (8, bottom - 40, 32, 32) = Button/Icon Xs with a Rotation glyph. Gap: the menu glyph is 18 px in the painter and Button/Icon
Xs draws 16 (the glyph of an instance cannot be resized).
"""
from rest_ir import E, F, I, T, X, lit

HOME_X = "tabs.rs:991: the Home tab is `bar.left() + 8` wide of the 8 px frame inset: a literal in the Rust"
TILE_PAD = "tabs.rs:1073: the file tile is centred at tab.left + 13 and is 14 px wide: 6 px from the tab's edge, a literal"
CLOSE_PAD = "tabs.rs:1097: the close plate is centred 13 px from the tab's right edge and is 18 px wide: 4 px from it, a literal"
TILE_GAP = "tabs.rs:1096: the title starts 6 px after the tile (`badge_rect.right() + 6.0`), a literal"
TAB_GAP = "tabs.rs:1116 `x = tab_rect.right() + 3.0`: the gap after every tab is a raw 3 in the Rust, no constant named"
HOME_GLYPH_PAD = "tabs.rs:1000-1001: the home glyph is centred at +14 and is 14 px wide (7 px from the edge); the label starts at +26 (5 px after it): literals"
BRAND_PAD = "shell.rs:1508-1512: the logo is centred 8 px right of x and is 14 px wide (1 px from x); the word mark starts at x + 19 (4 px after it): literals"
BAR_W, BAR_H, TITLE_H = 1440.0, 32.0, 28.0  # layout.json tab_bar; shell.rs:88
RAIL_W, RAIL_H = 49.0, 868.0  # layout.json rail
LINE = "Divider/Hairline"


def hair(name, x, y, w, h):
    """A 1 px BORDER line: the Divider/Hairline main resized to its run (a vertical line is the same line turned, 1 px wide)."""
    return X(name, LINE, "State=Default", abs=(x, y, w, h))


def title_bar(values, env):
    glyph = E("Logo", 14, 14, fill=env.c("BG_BRAND"))
    word = F("Wordmark", [T("design", "design", "Text sm / Semibold", env.c("TEXT")), T("os studio", ":os studio", "Text sm / Regular", env.c("TEXT_SECONDARY"))], dir="H")
    brand = F("window-title-brand", [glyph, word], dir="H", gap=env.c("SPACE_FACT"), pad=[None, None, None, lit(1, BRAND_PAD)], h=24.0, sx="hug", sy="fixed",
              abs=(76.0, 2.0, 120.0, 24.0), align=("start", "center"))
    return F("Title bar", [brand, hair("Bottom line", 0, TITLE_H - 1, BAR_W, 1)], fill=env.c("BG_PRIMARY"), w=BAR_W, h=TITLE_H, sx="fixed", sy="fixed")


def home_tab(env):
    look = {"icon": env.c("ICON_QUATERNARY"), "text": env.c("TEXT_QUATERNARY")}  # kit::tab_look(Default)
    flow = dict(dir="H", gap=lit(5, HOME_GLYPH_PAD), pad=[None, None, None, lit(7, HOME_GLYPH_PAD)], align=("start", "center"))
    # kit::tab_look(Selected): BG_SELECTED behind the Default glyph and a brighter label over the Default label; the glyph (ICON_QUATERNARY_HOVER) is one step off, so it is not drawn twice
    fill = F("Home selected", fill=env.c("BG_SELECTED"), radius=env.c("RADIUS_CONTROL"), w=72.0, h=24.0, sx="fixed", sy="fixed", abs=(0.0, 0.0, 72.0, 24.0),
             visible_prop="Show Home selected", hidden=True)
    bright = T("Home selected label", "Home", "Body / Semibold", env.c("TEXT_SECONDARY"), abs=(26.0, 3.0, 36.0, 18.0), visible_prop="Show Home selected", hidden=True)
    kids = [fill, I("Home glyph", "Home", 14, look["icon"]), T("Home label", "Home", "Body / Semibold", look["text"]), bright]
    return F("tab-bar-home", kids, radius=env.c("RADIUS_CONTROL"), w=72.0, h=24.0, sx="fixed", sy="fixed", **flow)


def project_tab(env):
    look = {"fill": env.c("BG_SELECTED"), "text": env.c("TEXT_SECONDARY")}  # kit::tab_look(Selected)
    tile = F("File tile", [I("Design glyph", "Design", 11, env.c("TILE_DESIGN_GLYPH"))], dir="H", fill=env.c("TILE_DESIGN_BG"), radius=env.c("RADIUS_MARK"), w=14.0, h=14.0, sx="fixed", sy="fixed")
    title = T("Title", "Design file", "Text xs / Semibold", look["text"], prop=None)
    plate = F("tab-close-1", [I("Close glyph", "Close", 10, env.c("ICON_QUATERNARY"))], dir="H", radius=env.c("RADIUS_SEGMENT"), w=18.0, h=18.0, sx="fixed", sy="fixed",
              abs=(218.0, 4.0, 18.0, 18.0))
    dev = F("Dev tile", [I("Dev glyph", "Dev", 10, env.c("TEXT_ON_BRAND"))], dir="H", fill=env.c("DEV_READY"), radius=env.c("RADIUS_MARK"), w=14.0, h=14.0, sx="fixed", sy="fixed",
            abs=(6.0, 6.0, 14.0, 14.0), visible_prop="Show dev tile", hidden=True)
    dot = E("Unsaved dot", 6, 6, fill=env.c("ICON_BRAND_PRIMARY"), abs=(209.0, 10.0, 6, 6), visible_prop="Show unsaved dot", hidden=True)
    return F("project-tab-0", [tile, title, plate, dev, dot], dir="H", gap=lit(6, TILE_GAP), pad=[None, None, None, lit(6, TILE_PAD)], fill=look["fill"], radius=env.c("RADIUS_CONTROL"),
             w=240.0, h=26.0, sx="fixed", sy="fixed", align=("start", "center"), visible_prop="Show project tab")


def new_tab(env):
    return F("tab-bar-new", [I("Plus glyph", "Plus", 13, env.c("ICON_QUATERNARY"))], dir="H", radius=env.c("RADIUS_CONTROL"), w=24.0, h=24.0, sx="fixed", sy="fixed", abs=(2.0, 0.0, 24.0, 24.0))


def slot(name, kid, w, h):
    return F(name, [kid], w=w, h=h, sx="fixed", sy="fixed")


def top_tab_bar(values, env):
    divider = slot("Home divider slot", hair("Home divider", 3, 0, 1, 16), 6.0, 16.0)  # `x = home.right + 6`: 3 of the strip's gap and 3 before the line; the 2 after it, with the gap, make `x += 6`
    plus = slot("New tab slot", new_tab(env), 26.0, 24.0)  # the `+` is at `x + 2` (tabs.rs:1121)
    kids = [home_tab(env), divider, project_tab(env), plus, hair("Bottom line", 0, BAR_H - 1, BAR_W, 1)]
    return F("Bar", kids, dir="H", gap=lit(3, TAB_GAP), pad=[None, None, None, lit(8, HOME_X)], fill=env.c("BG_PRIMARY"), w=BAR_W, h=BAR_H, sx="fixed", sy="fixed", align=("start", "center"))


RAIL_ITEMS = (("rail-File", "File"), ("rail-Agents", "Sparkle"), ("rail-Assets", "Assets"), ("rail-Tools", "Tools"), ("rail-Variables", "Variables"), ("rail-Plugins", "Component"))


def rail_cell(env, name, icon, selected, top):
    """A 47 x 56 cell holding the 32 px Button/Tool tile centred on it (rail.rs: `Rect::from_center_size(cell centre, RAIL_TILE)`)."""
    tile = env.num("RAIL_TILE")
    inner = (env.num("RAIL_WIDTH") - env.num("BORDER_WIDTH") - tile) / 2
    state = "State=Selected" if selected else "State=Default"
    t = X("Tile", "Button/Tool", state, props={"Icon": icon}, abs=(inner, (env.num("RAIL_ITEM_SIZE") - tile) / 2, tile, tile))
    return F(name, [t], w=env.num("RAIL_WIDTH") - env.num("BORDER_WIDTH"), h=env.num("RAIL_ITEM_SIZE"), sx="fixed", sy="fixed", abs=(0.0, top, 47.0, 56.0))


def left_rail(values, env):
    inset, side = env.num("INSET_FRAME"), env.num("RAIL_MENU_BUTTON_SIZE")
    run = env.num("RAIL_WIDTH") - env.num("BORDER_WIDTH") - 2 * inset  # rail.rs: separators run from INSET_FRAME to the inner width - INSET_FRAME
    top, kids = env.num("RAIL_FIRST_Y"), []
    menu = X("rail-menu", "Button/Icon", "Size=Xs, State=Default", props={"Icon": "ListView"}, abs=(inset, env.num("RAIL_MENU_BUTTON_Y"), side, side))
    kids += [menu, hair("Menu separator", inset, env.num("RAIL_SEPARATOR_Y"), run, 1)]
    for name, icon in RAIL_ITEMS:
        if name == "rail-Variables":
            top += env.num("RAIL_GROUP_GAP")
            kids.append(hair("Group separator", inset, top - env.num("RAIL_GROUP_GAP") / 2, run, 1))
        kids.append(rail_cell(env, name, icon, name == f"rail-{values['Tool']}", top))
        top += env.num("RAIL_ITEM_SIZE")
    kids += [X("rail-help", "Button/Icon", "Size=Xs, State=Default", props={"Icon": "Rotation"}, abs=(inset, RAIL_H - inset - side, side, side)), hair("Right border", RAIL_W - 1, 0, 1, RAIL_H)]
    return F("Rail", kids, fill=env.c("BG"), w=RAIL_W, h=RAIL_H, sx="fixed", sy="fixed")


def component_sets():
    return [
        {"name": "Navigation/TitleBar", "row": "Regions", "axes": {}, "uses": [LINE], "build": title_bar},
        {"name": "Navigation/TopTabBar", "row": "Regions", "axes": {"Bar": ["32px"]}, "uses": [LINE], "build": top_tab_bar,
         "props": [{"name": "Show project tab", "type": "BOOLEAN", "default": True}, {"name": "Show Home selected", "type": "BOOLEAN", "default": False},
                   {"name": "Show dev tile", "type": "BOOLEAN", "default": False}, {"name": "Show unsaved dot", "type": "BOOLEAN", "default": False}]},
        {"name": "Navigation/LeftRail", "row": "Regions", "axes": {"Rail": ["48px"], "Tool": ["File", "Agents", "Assets", "Variables"]}, "uses": [LINE, "Button/Icon", "Button/Tool"],
         "build": left_rail},
    ]
