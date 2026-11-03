"""Registry `composite` Menu/Panel, Menu/Context and Popover/Panel (page `03 Studio`, row Menus & Popovers). Lines are the v0.7.5 tag.

Menu/Panel (State=Default): `popup_tree_with_row_h` (widgets/menu.rs:321-573). The panel is `menu_panel` (menu.rs:445-451, widgets/mod.rs:82-84) = kit `menu_surface` (widgets/kit/surface.rs:56-58,
`menu_panel_look` :23-25): `BG`, a `BORDER_MENU` 1 px edge, `RADIUS_FRAME`. Rows start `MENU_PADDING` from the top (`Layout::new`, menu.rs:175-182, `MENU_PADDING` at :179), each a pill inset `MENU_PILL_INSET` on both
sides (the Menu/Item main is that 184 px), a separator after a row is a `BORDER` line across the whole panel (menu.rs:517, scrolled branch :463, `hline(rect.left, rect.right)`, `SEP_H` = 17). Callers of `popup_tree`: 4,
`panels/menus.rs:818` (tool menus, `TOOL_MENU_WIDTH`), `:832` (main menu, `MAIN_MENU_WIDTH` 210 at `MAIN_MENU_X/Y`), `panels/context.rs:157`, `widgets/menu.rs:699`. Nested (live): Menu/Item and
Menu/Separator. Local: the panel and the Slot container `Body` (counter-axis centred, so a 184 px pill keeps its 8 px and a `sx=fill` separator spans the panel: no side padding, no spacer frame).
The width is the 200 `MENU_WIDTH` minimum (a real menu is as wide as its widest row; the main menu is 210). Default content, neutral: a row with no submenu, a separator, a row with its icon and
shortcut, a separator, a row with its submenu (the three rows an open main menu shows at its top). Gaps: no menu in this set has a check column (`has_check` keys on `checkable`, menu.rs:177;
`main_nodes` in panels/menus.rs has none at its top level), so Menu/Item's check-menu gap (w2a "Carry to W3", INFO 1) does not arise here; direct `menu_panel` callers with their own rows are second
painters: board/bar.rs:451, board/toolbar.rs:214 and 273, dev/compare.rs:101, panels/menus.rs:726 (the zoom menu: a check at 18, 12 px, an input row), panels/typography.rs:401.

Menu/Context (State=Default): `panels/context.rs:145-167` `show` -> `popup_tree(.., "selection-menu", at, 230.0, ..)` (:157), the Menu/Panel painter at a 230 minimum with `entries(app)` (no
`.on(`, so no check column either). It is Menu/Panel at 230, so the pills are 214 and their Menu/Item copies fill the body, which has `MENU_PILL_INSET` of side padding. The separators are 230 wide copies (`X(.., w=230)`, own
sizing Fixed): the body centres its children, so a 230 line overflows the 214 content box by 8 px on each side and spans the panel, as `hline(rect.left, rect.right)` does. No capture shows the selection menu:
the shared parts are verified through Menu/Panel, the set is UNVERIFIED-VISUAL. The layer picker (`show_layer_picker`, context.rs:169-, its own caption "Select layer" and a `BORDER_MENU` line,
overlay `micro-layer-picker`) is a second painter, recorded for W4.

Popover/Panel (State=Default): `widgets/popover.rs:17-50` `show`: kit `popover_surface` (surface.rs:60-62, `popover_look` :32-34: `BG`, `BORDER_MENU`, `RADIUS_FRAME`), the title in Body Semibold (`font_body_strong`) in
TEXT at `LABEL_X` (a local 16, a `lit`: popover.rs:9; `INSET_POPOVER` only holds the same value), a `BORDER` hairline at `HEADER_H - 1` (a local 40, popover.rs:7), and a 24 x 24 close at (right - 32, top + 8) (:33-34), a
`widgets::icon_button` = Button/Close Xs resized to 24 (placement; the W1a Close glyph stays 16). Callers: 4, `panels/effects.rs:331` and `panels/stroke.rs:120` and `panels/style_edit.rs:123` (240 wide) and
`panels/typography.rs:621` (`DETAILS_W` 312). Nested (live): Button/Close, and Divider/Hairline, whose main is the same 1 px `BORDER` line (resized to the panel width, absolute at y 39).
Local: the header row, the Slot container `Body` and its neutral default (one label row in `font_body`, TEXT_SECONDARY, at `LABEL_X`, 24 high as `popover::row` paints it, popover.rs:53-60) between
8 px of padding (a `lit`: the body's raw `+ 8.0`, popover.rs:54; `INSET_FRAME` only holds the same value). The width is the three callers' 240. Second painters, recorded: the 15 other `popover_surface` call lines draw their own content without
`popover::show` (libraries/review.rs:33, panels/check_designs.rs:160 and 333, variables/popover.rs:115, comments/thread.rs:49 ...). `composes=` Panel/Header, Menu/Item, List/Row is spec intent, not live.

Popover/Panel `Kind`: the second axis, `Default` first (today's popover: its paths `Header/Title`, `Header/Close`, `Header/Hairline` and `Body/Row/Label` stay, and a placement with only `State=Default`
resolves to it). Every other Kind is one production popover drawn at its capture's own size from its painter, parts placed with `abs`, the controls nested registry instances
(popover_kinds.py, popover_kinds_settings.py, popover_kinds_type.py, popover_kinds_dialogs.py, popover_kinds_skills.py), so a screen sets a Kind and nothing else.
Menu/Item `Show check column` (menus.py) and Tabs/Segmented `Show cell 3` (headers.py) are the parts the menus and the vector bar need.
"""
from components import popover_kinds
from rest_ir import F, T, X, lit

ITEM = "Size=Xs, State=Default"
POPOVER_W = 240.0  # panels/effects.rs:331, stroke.rs:120, style_edit.rs:123 (typography's DETAILS_W 312 is its own caller)
CLOSE_BOX, CLOSE_INSET = 24.0, 8.0  # popover.rs:33 `rect.right() - 8.0 - 24.0`, `rect.top() + 8.0`
HEADER_H = 40.0  # popover.rs:7 `pub const HEADER_H: f32 = 40.0`: a local constant
LABEL_X = popover_kinds.LABEL_X_REASON
BODY_PAD = "widgets/popover.rs:54 `body.top() + 8.0`: a raw literal, no theme.rs name (INSET_CONTROL and INSET_FRAME hold 8 for other roles)"
ROW_H = 24.0  # popover.rs:55 `Vec2::new(body.width(), 24.0)`


def menu_rows(fill_items, sep_width=None):
    """The neutral default content of a menu: a row, a separator, a row with its icon and shortcut, a separator, a row with its submenu. The separators fill the body, or, where the body has side
    padding (the pills' inset), are `sep_width` wide: the panel's own width, which the body's centring lets them overflow its content box to span."""
    size = {"sx": "fill"} if fill_items else {}
    item = lambda name, props: X(name, "Menu/Item", ITEM, props=props, **size)
    sep = lambda name: X(name, "Menu/Separator", "State=Default", **({"w": sep_width} if sep_width else {"sx": "fill"}))
    return [item("Item 1", {"Label": "Item", "Has Submenu": False}), sep("Separator 1"),
            item("Item 2", {"Label": "Item", "Has Icon": True, "Has Shortcut": True, "Has Submenu": False}), sep("Separator 2"), item("Item 3", {"Label": "Item"})]


def menu(width, fill_items):
    def build(values, env):
        side = [None, env.c("MENU_PILL_INSET"), None, env.c("MENU_PILL_INSET")] if fill_items else None
        body = F("Body", menu_rows(fill_items, width if fill_items else None), dir="V", slot="Body", sx="fill", sy="hug", align=("start", "center"), **({"pad": side} if side else {}))
        return F("Panel", [body], dir="V", fill=env.c("BG"), stroke=env.c("BORDER_MENU"), sw=env.c("BORDER_WIDTH"), radius=env.c("RADIUS_FRAME"),
                 pad=[env.c("MENU_PADDING"), None, env.c("MENU_PADDING"), None], w=width or env.num("MENU_WIDTH"), sx="fixed", sy="hug", align=("start", "start"))
    return build


def popover(values, env):
    if values.get("Kind", "Default") in popover_kinds.KINDS:
        return popover_kinds.build(values["Kind"], env)
    header_h = HEADER_H
    close = X("Close", "Button/Close", "Size=Xs, State=Default", abs=(POPOVER_W - CLOSE_INSET - CLOSE_BOX, CLOSE_INSET, CLOSE_BOX, CLOSE_BOX))
    line = X("Hairline", "Divider/Hairline", "State=Default", abs=(0.0, header_h - 1, POPOVER_W, env.num("BORDER_WIDTH")))
    title = T("Title", "Settings", "Body / Semibold", env.c("TEXT"), prop="Title")
    header = F("Header", [title, close, line], dir="H", pad=[None, None, None, lit(16, LABEL_X)], h=header_h, sx="fill", sy="fixed", align=("start", "center"))
    row = F("Row", [T("Label", "Setting", "Body / Regular", env.c("TEXT_SECONDARY"))], dir="H", pad=[None, None, None, lit(16, LABEL_X)], h=ROW_H, sx="fill", sy="fixed", align=("start", "center"))
    body = F("Body", [row], dir="V", slot="Body", sx="fill", sy="hug", pad=[lit(8, BODY_PAD), None, lit(8, BODY_PAD), None])
    return F("Popover", [header, body], dir="V", fill=env.c("BG"), stroke=env.c("BORDER_MENU"), sw=env.c("BORDER_WIDTH"), radius=env.c("RADIUS_FRAME"), w=POPOVER_W,
             sx="fixed", sy="hug", align=("start", "start"))


def component_sets():
    slot = [{"name": "Body", "type": "SLOT"}]
    return [
        {"name": "Menu/Panel", "row": "Menus & Popovers", "axes": {"State": ["Default"]}, "uses": ["Menu/Item", "Menu/Separator"], "props": slot, "build": menu(None, False)},
        {"name": "Menu/Context", "row": "Menus & Popovers", "axes": {"State": ["Default"]}, "uses": ["Menu/Item", "Menu/Separator"], "props": slot,
         "build": menu(230.0, True)},  # panels/context.rs:157 `popup_tree(.., at, 230.0, ..)`
        {"name": "Popover/Panel", "row": "Menus & Popovers", "axes": {"State": ["Default"], "Kind": ["Default", *popover_kinds.ORDER]}, "uses": ["Button/Close", "Divider/Hairline", "Button/Icon",
         "Button/Primary", "Checkbox/Box", "Input/Field", "Input/PropertyField", "Radio/Card", "Select/Trigger"],
         "props": [{"name": "Title", "type": "TEXT", "default": "Settings"}, *slot], "build": popover},
    ]
