"""Registry `composite` Panel/Header, Section/Header and Tabs/Segmented (page `03 Studio`, row Headers & Tabs). Lines are the v0.7.5 tag.

Panel/Header (State=Default): the kit `panel_header` (widgets/kit/rows.rs:114-119) paints the title only: Text sm Semibold in TEXT at `rect.left + INSET_PANEL`,
centred vertically. Its two callers are the left panel's `section_title` (panels/left.rs:245-247): "Pages" (left.rs:114, `PANEL_TITLE_ROW_HEIGHT`) and "Layers" (left.rs:212,
`LAYER_HEADER_HEIGHT`, the same 40), each followed by `action_cluster` (left.rs:31-37): two `widgets::icon_button` rects of `LEFT_ACTION_BUTTON` (`CONTROL_XS`) at
`LEFT_ACTION_PITCH` (`CONTROL_XS` + `SPACE_FACT`), the first `PANEL_ACTION_RIGHT_PAD` from the row's right edge. `icon_button` paints Button/Icon Xs (widgets/mod.rs:144-150,
Hierarchy::Icon, Size::Xs), so the row nests Button/Icon Xs copies: Search (the set's own glyph) and Plus (an engine swap) as Pages; Layers draws ChevronDown (`collapse_btn`, left.rs:228)
and Search (`fb`, :220). A nested instance's glyph cannot be exposed as a property of the host (#1471), so the choice is parts: a hidden `Collapse action` shown by
`Show Collapse action` (default off) and the Plus hidden by `Show Add action` (default on); the Layers header sets the first on and the second off, and the default row is the Pages one.
Local: the row frame and the title (the caller's layout; the title takes the free width, so no spacer frame). Gaps: the dev-mode tabs replace the Layers title
(left.rs:208-209, not this set); the registry's `composes=` Badge/StatusCount has no live badge, so none is drawn.

Section/Header (State=Default, Disabled): the kit's `section_header` (rows.rs:121-127) has 0 callers, so the live painter is drawn: `panels/properties.rs:207-226` `title`
(25 call lines: properties 11, prototype 7, components 4, effects, layoutguide, vertex), a `RIGHT_PANEL_WIDTH - 1` x `SECTION_TITLE_HEIGHT` row with the title in
Body Semibold (`font_body_strong`) in TEXT at `SECTION_CONTENT_X`, and the optional action `icon_button` at (`SECTION_TRAILING_X`, 8), `PANEL_ACTION_BUTTON` square: a Button/Icon Xs
resized to 24 (placement), `Has Action` shows it, a swap makes it Plus or Minus; the Page title's action is `Icon::Opacity` (properties.rs:405), a hidden `Opacity action`
shown by `Show Opacity action` (default off, `Has Action` off with it). Disabled is the kit arm `section_header_look(Disabled)` (rows.rs:151-157): half alpha on the
title, and the action is Button/Icon State=Disabled (a Disabled composite nests only sets that have a Disabled variant). The painter has no disabled path
of its own: the arm is the kit's value, a gap in production (UNVERIFIED-VISUAL). Gaps: `dim` (TEXT_SECONDARY, an empty section: properties.rs:964, effects.rs:143), the kit's
bottom hairline (rows.rs:125, not live), `paint_title` (properties.rs:230-) draws two action buttons for Fill and Stroke, and `section_sep` (properties.rs:199-204) is the caller's.

Tabs/Segmented (State=Selected, Default, Hover, Focus, Disabled): the kit `segmented` (widgets/kit/tabs.rs:76-113): a shell in BG_SECONDARY at `RADIUS_CONTROL`, cells inset
`INSET_SEGMENT` at `RADIUS_SEGMENT` (cell h = `RIGHT_TABS_H` - 2 * `INSET_SEGMENT`, :20), the label Text xs Semibold (`label_font`, :64-66), look `segmented_look` (:42-50),
ring `segmented_ring` (:32-35). The one caller is `comments/ui.rs:134-136`, with two sites (comments/panel.rs:46, 53), each 216 x `COMMENT_FILTER_H` with two labels.
A variant is the two-cell strip whose first cell is in that state and the second Default (Selected: the first selected); `Show cell 3` (default off, text `Third label`) shows a third cell,
Default like the second, which the vector sub-tool bar needs (canvas/vector.rs:1110-1142: three cells). Local: the cells (the segment tile is not Tabs/Pill:
`composes=` is spec intent, the painter draws its own cells), and the label at `Text xs / Semibold` (`label_font`). Focus draws the ring on the first cell itself (a Focus/Ring instance, no nested Focus variant): the cell is its host, so
the corner is `round(RADIUS_SEGMENT + SPACE_FACT)` = 10, a literal (kit/focus.rs:38).
Deviation, Disabled (controller ruling W2b-2): the live app never draws a disabled segmented (`state_of(.., true)` in the call, widgets/kit/tabs.rs:101), so State=Disabled is the whole strip
disabled: both cells take `segmented_look(Disabled)` (tabs.rs:46) and the shell is dimmed to 0.5 like every Disabled control in the file. UNVERIFIED-VISUAL.
Gaps: the Selected + hover blend (tabs.rs:102-103, `selected_hover`) has no registry state. Other segmented painters, not drawn: the icon strips (panels/properties.rs:314-,
Align and Layout rows), the text strips of the typography panel (panels/typography.rs:123, 243, 506), the check-designs tile (panels/check_designs.rs:385) and the toolbar's
mode switcher (panels/toolbar.rs:338-344).
"""
from rest_ir import F, T, X, lit

DISABLED_ARM = "State=Disabled is the kit arm `section_header_look(Disabled)` (widgets/kit/rows.rs:151-157); the live title painter has no disabled path (UNVERIFIED-VISUAL)"
DISABLED_STRIP = "State=Disabled is the whole strip disabled: the app never draws a disabled segmented (`state_of(.., true)`, widgets/kit/tabs.rs:101)"
RING_CELL = "kit/focus.rs:38 rounds RADIUS_SEGMENT (6) + SPACE_FACT (4) = 10 on the cell the ring wraps: a sum with no variable"
STRIP_W = 216.0  # comments/panel.rs:24 `rect.width() - SIDE * 2 - 1`: 216 in the 240 px comments panel (capture j-comments, comments-filter-pages)
CELL_STATES = {"Selected": ("Selected", "Default"), "Default": ("Default", "Default"), "Hover": ("Hover", "Default"), "Focus": ("Focus", "Default"),
               "Disabled": ("Disabled", "Disabled")}


def panel_header(values, env):
    action = lambda name, icon, **kw: X(name, "Button/Icon", "Size=Xs, State=Default", props={"Icon": icon} if icon else None, **kw)
    title = T("Title", "Pages", "Text sm / Semibold", env.c("TEXT"), prop="Title", sx="fill")
    kids = [title, action("Collapse action", "ChevronDown", visible_prop="Show Collapse action", hidden=True), action("Secondary action", None),
            action("Primary action", "Plus", visible_prop="Show Add action")]
    return F("Header", kids, dir="H", gap=env.c("SPACE_FACT"),
             pad=[None, env.c("PANEL_ACTION_RIGHT_PAD"), None, env.c("INSET_PANEL")], w=env.num("LEFT_PANEL_WIDTH"), h=env.num("PANEL_TITLE_ROW_HEIGHT"),
             sx="fixed", sy="fixed", align=("start", "center"))


def section_header(values, env):
    disabled = values["State"] == "Disabled"
    h, side = env.num("SECTION_TITLE_HEIGHT"), env.num("PANEL_ACTION_BUTTON")
    title = T("Title", "Section", "Body / Semibold", env.c("TEXT"), prop="Title", **({"alpha": 0.5} if disabled else {}))
    action = X("Action", "Button/Icon", f"Size=Xs, State={values['State']}", props={"Icon": "Plus"}, visible_prop="Has Action",
               abs=(env.num("SECTION_TRAILING_X"), (h - side) / 2, side, side))
    opacity = X("Opacity action", "Button/Icon", f"Size=Xs, State={values['State']}", props={"Icon": "Opacity"}, visible_prop="Show Opacity action", hidden=True, abs=action["abs"])
    return F("Header", [title, action, opacity], dir="H", pad=[None, None, None, env.c("SECTION_CONTENT_X")], w=env.num("RIGHT_PANEL_WIDTH") - 1, h=h,
             sx="fixed", sy="fixed", align=("start", "center"))


def segment_cell(name, label, prop, state, env):
    """One cell of the strip in a kit state: tile fill and edge only where the look has them, the label centred."""
    look = env.look("Tabs/Segmented", state)
    cell_w, cell_h = (STRIP_W - 2 * env.num("INSET_SEGMENT")) / 2, env.num("COMMENT_FILTER_H") - 2 * env.num("INSET_SEGMENT")
    kids = [T("Label", label, "Text xs / Semibold", look["text"], prop=prop, alpha=look["alpha"])]
    if look["ring"]:
        kids.append(X("Focus ring", "Focus/Ring", "Tone=Brand", abs=(-4, -4, cell_w + 8, cell_h + 8), radius=lit(10, RING_CELL)))
    paints = {"fill": look["fill"]} if look["fill"] else {}
    if look["border"]:
        paints.update(stroke=look["border"], sw=env.c("BORDER_WIDTH"))
    return F(name, kids, dir="H", radius=env.c("RADIUS_SEGMENT"), sx="fill", sy="fill", alpha=look["alpha"], **paints)


def segmented(values, env):
    first, second = CELL_STATES[values["State"]]
    cells = [segment_cell("Cell 1", "This page", "First label", first, env), segment_cell("Cell 2", "All pages", "Second label", second, env)]
    # Show cell 3: the vector sub-tool bar (canvas/vector.rs:1110-1142) is a three-cell strip; the cells are `fill` wide, so a third one takes its third of the strip when shown.
    third = dict(segment_cell("Cell 3", "Third", "Third label", second, env), visible_prop="Show cell 3", hidden=True)
    cells.append(third)
    return F("Segmented", cells, dir="H", fill=env.c("BG_SECONDARY"), radius=env.c("RADIUS_CONTROL"), pad=env.c("INSET_SEGMENT"), w=STRIP_W,
             h=env.num("COMMENT_FILTER_H"), sx="fixed", sy="fixed", alpha=0.5 if values["State"] == "Disabled" else 1.0)


def component_sets():
    return [
        {"name": "Panel/Header", "row": "Headers & Tabs", "axes": {"State": ["Default"]}, "uses": ["Button/Icon"],
         "props": [{"name": "Title", "type": "TEXT", "default": "Pages"}, {"name": "Show Collapse action", "type": "BOOLEAN", "default": False},
                   {"name": "Show Add action", "type": "BOOLEAN", "default": True}], "build": panel_header},
        {"name": "Section/Header", "description": DISABLED_ARM, "row": "Headers & Tabs", "axes": {"State": ["Default", "Disabled"]}, "uses": ["Button/Icon"],
         "props": [{"name": "Title", "type": "TEXT", "default": "Section"}, {"name": "Has Action", "type": "BOOLEAN", "default": True},
                   {"name": "Show Opacity action", "type": "BOOLEAN", "default": False}], "build": section_header},
        {"name": "Tabs/Segmented", "description": DISABLED_STRIP, "row": "Headers & Tabs", "axes": {"State": ["Selected", "Default", "Hover", "Focus", "Disabled"]}, "uses": ["Focus/Ring"],
         "props": [{"name": "First label", "type": "TEXT", "default": "This page"}, {"name": "Second label", "type": "TEXT", "default": "All pages"},
                   {"name": "Third label", "type": "TEXT", "default": "Third"}, {"name": "Show cell 3", "type": "BOOLEAN", "default": False}], "build": segmented},
    ]
