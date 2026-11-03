"""Registry `component` Tabs/Pill (row Navigation): the right panel's Design | Prototype tabs, painted inline in `panels::right::show`
(panels/right.rs:112-128). The kit `tab` (widgets/kit/tabs.rs:115-126) has 0 callers, so its look is a gap and the live painter is drawn.

A tab is `text_width + 2 * RIGHT_TAB_PAD_X` wide and `RIGHT_TABS_H` (24) high with `RADIUS_CONTROL`, its label centred. Selected: `BG_SECONDARY`
fill, `TEXT`, `font_body_strong`. Hover: `BG_HOVER` fill, `TEXT_SECONDARY`, `font_body`. Default: no fill, `TEXT_SECONDARY`, `font_body`. The painter
has no focus ring, so Focus is the Default look (Tag/Chip precedent), and no disabled path, so Disabled is the Default at half alpha on each paint
(`kit/mod.rs:134-162`, G2). The registry's `today=` and `paints=` (TEXT_QUATERNARY, a ring) describe the kit tab, not this painter.

Other tab painters, recorded and not drawn here: the picker's Custom | Libraries tabs (widgets/picker.rs:754-786: always `font_body_strong`, hover
`BG_TRANSPARENT_HOVER`), the Dev panel's Layers | Ready for dev strip (dev/list.rs:53: always `font_body_strong`, with a monospace count badge) and the top bar's file and home tabs (`kit::tab_look`, tabs.rs:995 and 1058), which belong to region Navigation/TopTabBar.
"""
from rest_ir import F, T

LABEL = {"Selected": ("Body / Semibold", "TEXT"), "Hover": ("Body / Regular", "TEXT_SECONDARY"), "Default": ("Body / Regular", "TEXT_SECONDARY")}
FILL = {"Selected": "BG_SECONDARY", "Hover": "BG_HOVER"}


def tab_pill(values, env):
    state = values["State"]
    look = "Default" if state in ("Focus", "Disabled") else state
    style, colour = LABEL[look]
    alpha = 0.5 if state == "Disabled" else 1.0
    pad = env.c("RIGHT_TAB_PAD_X")
    return F("Tab", [T("Label", "Design", style, env.c(colour), prop="Label", alpha=alpha)], dir="H", fill=env.c(FILL[look]) if look in FILL else None,
             radius=env.c("RADIUS_CONTROL"), pad=[None, pad, None, pad], h=env.num("RIGHT_TABS_H"), sx="hug", sy="fixed", alpha=alpha)


def component_sets():
    return [{"name": "Tabs/Pill", "row": "Navigation", "axes": {"State": ["Default", "Selected", "Hover", "Focus", "Disabled"]},
             "props": [{"name": "Label", "type": "TEXT", "default": "Design"}], "build": tab_pill}]
