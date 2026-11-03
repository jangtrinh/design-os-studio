"""Registry `element` records of the Actions row and its neighbours: the 1 px lines, the focus ring, the glyph box, the status
dot and the menu shortcut. Geometry comes from the kit code (widgets/kit/focus.rs, badge.rs, rows.rs, widgets/menu.rs); where
spec C differs the Rust wins and the build report lists the gap.
"""
from rest_ir import F, I, T, lit

RING_REASON = "kit/focus.rs paints a 2 px band (Stroke::new(2.0, ..)): a literal of the Rust and no stroke-weight constant in theme.rs; the Semantic variables that equal 2 are other roles"
RING_RADIUS = "kit/focus.rs: the ring's outer corner radius is RADIUS_CONTROL + SPACE_FACT (8 + 4 = 12), a sum with no variable of its own"
ICON_COLOUR = {"Default": "ICON_QUATERNARY", "Hover": "ICON_QUATERNARY_HOVER", "Selected": "ICON_SECONDARY_HOVER"}


def hairline(values, env):
    return F("Hairline", fill=env.c("BORDER"), w=160, h=env.num("BORDER_WIDTH"), sx="fixed", sy="fixed")


def focus_ring(values, env):
    colour = env.c("FOCUS_RING_ERROR" if values["Tone"] == "Error" else "FOCUS_RING")
    return F("Ring", stroke=colour, sw=lit(2, RING_REASON), radius=lit(12, RING_RADIUS), w=40, h=40, sx="fixed", sy="fixed")


def icon_glyph(values, env):
    size = int(env.num("LAYER_ICON_SIZE") + (env.num("SPACE_FACT") if values["Size"] == "Md" else 0))
    return F("Glyph box", [I("Icon", "Search", size, env.c(ICON_COLOUR[values["State"]]), swap="Icon")], dir="H", sx="hug", sy="hug")


def status_dot(values, env):
    colour = {"Info": "ICON_BRAND_PRIMARY", "Error": "DANGER", "Success": "SUCCESS"}[values["Tone"]]
    # The live dot is the toast's (shell.rs:1267-1268 `circle_filled(.., 4.0, DANGER | ICON_BRAND_PRIMARY)`, in `toast`, shell.rs:1201):
    # a radius of 4 = SPACE_FACT, so an 8 px disc. Success has no live painter (`kit::status_color`, 0 production callers).
    d = int(env.num("SPACE_FACT") * 2)
    return F("Dot", fill=env.c(colour), radius=env.c("SPACE_FACT"), w=d, h=d, sx="fixed", sy="fixed")


def menu_separator(values, env):
    """SEP_H = MENU_SEP_MARGIN * 2 + BORDER_WIDTH = 17 (spec C says 9): the line sits MENU_SEP_MARGIN below the row."""
    line = F("Line", fill=env.c("BORDER"), h=env.num("BORDER_WIDTH"), sx="fill", sy="fixed")
    m = env.c("MENU_SEP_MARGIN")
    return F("Separator", [line], dir="V", pad=[m, None, m, None], w=160, sx="fixed", sy="hug", align=("start", "start"))


def menu_shortcut(values, env):
    look = env.look("Menu/Shortcut", values["State"])
    return F("Shortcut", [T("Keys", "⌘K", "Body / Medium", look["text"], prop="Keys", alpha=look["alpha"])], dir="H", sx="hug", sy="hug")


def component_sets():
    return [
        {"name": "Divider/Hairline", "row": "Actions", "axes": {"State": ["Default"]}, "build": hairline},
        {"name": "Focus/Ring", "row": "Actions", "axes": {"Tone": ["Brand", "Error"]}, "build": focus_ring},
        {"name": "Icon/Glyph", "row": "Actions", "axes": {"Size": ["Sm", "Md"], "State": ["Default", "Hover", "Selected"]},
         "props": [{"name": "Icon", "type": "INSTANCE_SWAP", "default": "Search"}], "build": icon_glyph},
        {"name": "Status/Dot", "row": "Badges & Status", "axes": {"Tone": ["Info", "Error", "Success"]}, "build": status_dot},
        {"name": "Menu/Separator", "row": "Menus & Overlays", "axes": {"State": ["Default"]}, "build": menu_separator},
        {"name": "Menu/Shortcut", "row": "Menus & Overlays", "axes": {"State": ["Default", "Hover", "Active", "Disabled"]},
         "props": [{"name": "Keys", "type": "TEXT", "default": "⌘K"}], "build": menu_shortcut},
    ]
