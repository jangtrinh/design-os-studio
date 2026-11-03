"""Registry `region` Panel/LeftLayers (page `03 Studio`, row Regions): the left panel of the editor with the file header, the Pages list and the Layers tree (panels/left.rs `show`, `file_header`,
`pages_list`; panels/layers.rs; panels/left_sep.rs `paint_and_interact`). Lines are the 0.7.5 tag of crates/designos-app/src. A 241 x 868 region (layout.json `regions.left_panel`: 240 +
the 1 px right border), BG, its children placed where that code puts them and the named widgets carrying the layout.json key as their name.

* Header (0.. `LEFT_HEADER_HEIGHT` 65, a BORDER hairline on its last row): the file name in `font_large_strong` (Large Semibold, TEXT) from `PANEL_PADDING_X` (16) with a 12 px
  ChevronDown 4 px after it (`file-header`, the hit rect (8, 8, 184, 24)), the subtitle `Drafts · Team project` in `font_body` TEXT_TERTIARY at the same x (`left-header-subtitle`, 2 px
  left of the text, 18 high), and the sidebar button (`sidebar`, Button/Icon Xs, Sidebar glyph) `LEFT_SIDEBAR_BTN_RIGHT_PAD` 12 from the inner edge. The file name is neutral content (no
  file or project name of a kit).
The body below the header is ONE auto layout column (panels/left.rs:111-152 walks it with a running `y`): `pages-toggle`, the Pages list, the `SPACE_INLINE` gap with the separator, the Layers header
and the Layers list. The two lists are native SLOTs (`Pages`, `Layers`), filled with List/Row and Tree/Row instances by the screen; the Pages list
  hugs its rows (`left_sep::prepare_height`), so a file with two
pages moves the Layers header up and the Layers list grows (`Show pages scrollbar` is off when the list fits).
* Pages (Panel/Header at y 65, 240 x 40, `pages-toggle`; Title Pages): a 160 px list (`pages-list`, 5 rows of `PAGE_ROW_HEIGHT`) of List/Row Xs, 224 wide, inset `PAGE_ROW_INSET` 8. The
  page names are neutral (`Page 1`, `Page 2`, a rule of em dashes, `SECTION`, `↳ Page 3`: the production list holds a kit's page names in the same five slots, one of them a divider drawn
  from dashes, one a section label, one nested). A 4 px wide thumb of `scrollbar` (panels/mod.rs:49-73: TEXT_TERTIARY, RADIUS_MARK, at `rect.right - 6`, the 24 px minimum height) sits in
  an 8 px track: the list has more pages than rows.
* A BORDER hairline (the separator, `left_sep`) at y 272, under the list and `SPACE_INLINE` 8 below it.
* Layers (Panel/Header at y 273, 240 x 40, Title Layers) and the tree (`layers-list`, y 313 to the panel's bottom, clipped): Tree/Row Xs, 224 wide at x 8, one per `LAYER_ROW_HEIGHT` 32,
  18 rows (the last is cut by the clip), with a 123 px thumb at the top of its track. The names are neutral (`Frame 1` ... with the type `Auto layout`, two `Component` rows with the type
  `Instance`, as the capture's first two layers are).
The Layers header shows ChevronDown and Search, as production draws them: its `Show Collapse action` is on and its `Show Add action` off, and `layers-collapse-all` / `layers-search` are
the header's nested `Collapse action` / `Secondary action` (the screen check finds them by that path).
Gaps: Tree/Row has no component-row look (TEXT_COMPONENT name and glyph: the registry has no axis for it, see the merge-3 report), so the two component rows draw like the others. The
page row of the current page (Selected), a dev-mode `Ready for development` section, the find panel that replaces the layers, and the agent terminal that replaces the whole panel are
other states.
"""
from rest_ir import F, I, T, X

TOP, SUB_TOP = 8.0, 39.0  # file-header y (8: LEFT_ACTION_Y - SPACE_FACT), subtitle y (LEFT_SUBLABEL_Y 40 + 8 - 9)
PANEL_W, PANEL_H = 241.0, 868.0  # layout.json regions.left_panel
PAGES = ("Page 1", "Page 2", "——————", "SECTION", "↳ Page 3")
# (name, type label, icon, has children): the kinds the capture's first rows show (two instances, then frames laid out in columns or in rows), with neutral names
LAYERS = (("Component 1", "Instance", "Instance", False), ("Component 2", "Instance", "Instance", False)) + tuple(
    (f"Frame {i}", "Auto layout", "AutoLayoutV" if i in (3, 4, 7, 8) else "AutoLayoutH", True) for i in range(1, 17))
ROW_W = 224.0


def scroll_track(env, x, y, track_h, thumb_h):
    bar = F("Thumb", fill=env.c("TEXT_TERTIARY"), radius=env.c("RADIUS_MARK"), w=4.0, h=thumb_h, sx="fixed", sy="fixed", abs=(2.0, 0.0, 4.0, thumb_h))
    return F("scrollbar-track", [bar], w=8.0, h=track_h, sx="fixed", sy="fixed", abs=(x, y, 8.0, track_h))


def header_block(env):
    inner = env.num("LEFT_PANEL_WIDTH")
    name = T("File name", "Design file", "Large / Semibold", env.c("TEXT"))
    head = F("file-header", [name, I("Chevron", "ChevronDown", 12, env.c("ICON_QUATERNARY"))], dir="H", gap=env.c("SPACE_FACT"), pad=[None, None, None, env.c("INSET_FRAME")],
             w=184.0, h=24.0, sx="fixed", sy="fixed", abs=(8.0, TOP, 184.0, 24.0), align=("start", "center"))
    sub = F("left-header-subtitle", [T("Subtitle", "Drafts · Team project", "Body / Regular", env.c("TEXT_TERTIARY"))], dir="H", pad=[None, env.c("SPACE_FACT"), None, env.c("INSET_SEGMENT")],
            h=18.0, sy="fixed", abs=(14.0, SUB_TOP, 130.0, 18.0), align=("start", "center"))  # left.rs:491-495: the hit rect is the text + 2 px before and 4 px after (width + 6)
    side = env.num("CONTROL_XS")  # LEFT_ACTION_BUTTON = CONTROL_XS (panels/left.rs:15)
    sidebar = X("sidebar", "Button/Icon", "Size=Xs, State=Default", props={"Icon": "Sidebar"},
                abs=(inner - env.num("LEFT_SIDEBAR_BTN_RIGHT_PAD") - side, TOP, side, side))
    rule = X("Header rule", "Divider/Hairline", "State=Default", abs=(0.0, env.num("LEFT_HEADER_HEIGHT") - 1, inner, 1))
    return [head, sub, sidebar, rule]


def left_layers(values, env):
    w, inset = env.num("LEFT_PANEL_WIDTH"), env.c("PAGE_ROW_INSET")
    header = F("Header", header_block(env), w=PANEL_W, h=env.num("LEFT_HEADER_HEIGHT"), sx="fixed", sy="fixed")
    toggle = X("pages-toggle", "Panel/Header", "State=Default", props={"Title": "Pages"})
    pages = [X(f"page-{i}", "List/Row", "Size=Xs, State=Default", props={"Label": label}) for i, label in enumerate(PAGES)]
    pages_list = F("pages-list", pages, dir="V", pad=[None, None, None, inset], w=w, sx="fixed", sy="hug", align=("start", "start"), slot="Pages")
    rule = X("Separator rule", "Divider/Hairline", "State=Default", abs=(0.0, env.num("SPACE_INLINE") - 1, w, 1))
    gap = F("Pages and layers separator", [rule], w=w, h=env.num("SPACE_INLINE"), sx="fixed", sy="fixed")  # the hairline is the last row of the `SPACE_INLINE` gap under the list (left.rs:152)
    layers = {"Title": "Layers", "Show Collapse action": True, "Show Add action": False}
    layers_header = X("Layers header", "Panel/Header", "State=Default", props=layers)
    tree = [X(f"layer-{i}", "Tree/Row", "Size=Xs, State=Default, Depth=0", props={"Name": n, "Type label": t, "Has Children": kids_, "Icon": icon})
            for i, (n, t, icon, kids_) in enumerate(LAYERS)]
    layers_list = F("Layers rows", tree, dir="V", pad=[None, None, None, inset], w=w, sx="fixed", sy="hug", align=("start", "start"), slot="Layers")
    area = F("layers-list", [layers_list, scroll_track(env, w - 8.0, 0.0, 555.0, 123.0)], w=w, sx="fixed", sy="fill", clip=True)
    pages_scroll = scroll_track(env, w - 8.0, env.num("LEFT_HEADER_HEIGHT") + env.num("PANEL_TITLE_ROW_HEIGHT"), 5 * env.num("PAGE_ROW_HEIGHT"), 24.0)
    pages_scroll["visible_prop"] = "Show pages scrollbar"  # both tracks keep the name `scrollbar-track` and their order, pages first: the screens' pins of it resolve as before
    border = X("Right border", "Divider/Hairline", "State=Default", abs=(PANEL_W - 1, 0.0, 1, PANEL_H))  # the region is the 240 px panel + its 1 px border (layout.json regions.left_panel)
    return F("Panel", [header, toggle, pages_list, pages_scroll, gap, layers_header, area, border], dir="V", fill=env.c("BG"), w=PANEL_W, h=PANEL_H, sx="fixed", sy="fixed",
             align=("start", "start"))


def component_sets():
    return [{"name": "Panel/LeftLayers", "row": "Regions", "axes": {}, "uses": ["Button/Icon", "Divider/Hairline", "List/Row", "Panel/Header", "Tree/Row"], "build": left_layers,
             "props": [{"name": "Pages", "type": "SLOT"}, {"name": "Layers", "type": "SLOT"}, {"name": "Show pages scrollbar", "type": "BOOLEAN", "default": True}]}]
