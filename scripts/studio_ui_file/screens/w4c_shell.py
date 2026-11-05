"""The editor shell the Panels, dialogs and variables screens of batch W4C start from (the regions of `Screen/EditorDesign`, the same rects), and the one function that turns a capture into a
screen spec.

    at(name, component, xy, variant=None, **extra)            one placed instance
    shell(left=True, right=True, toolbar=True, rulers=True)   the instances in paint order: tab bar, rail, left panel, rulers, toolbar, right panel (a part left out is covered by an overlay region)
    spec(flow, case, place, ...)                              {"name": `Screen/<flow>/<case>`, ...} for the capture `case`; `flow` is the registry `screen` record the capture shows

The rects are the ones `Screen/EditorDesign` places (`d-editor-default.layout.json` `regions`; every capture of this batch reports the same tab bar, rail, left panel, canvas and right panel).
A part a capture draws in another state than the one the region has (a selected layer, a scrolled list, an active rail tool) keeps the region's only state: the difference is measured by the
screen gate and listed as a gap in the report, never drawn locally.
"""
from screens import gap_wiring
from screens.editor_design import ALIASES, CANVAS, LEFT, RAIL, RIGHT, TAB_BAR

TOOLBAR = (464, 840)
BACKGROUND = "#F5F5F5"  # the page's own colour behind the canvas content, as on `Screen/EditorDesign`


def at(name, component, xy, variant=None, **extra):
    """One placed instance: its registry record, the variant, the position in the screen and any further key the screen builder reads (`w`, `h`: a size)."""
    return {"name": name, "component": component, "variant": variant, "x": xy[0], "y": xy[1], **extra}


def shell(left=True, right=True, toolbar=True, rulers=True):
    out = [at("Navigation/TopTabBar", "Navigation/TopTabBar", TAB_BAR, {"Bar": "32px"}), at("Navigation/LeftRail", "Navigation/LeftRail", RAIL, {"Rail": "48px"})]
    if left:
        out.append(at("Panel/LeftLayers", "Panel/LeftLayers", LEFT))
    if rulers:
        out.append(at("Navigation/Rulers", "Navigation/Rulers", CANVAS[:2]))
    if toolbar:
        out.append(at("Navigation/BottomCanvasToolbar", "Navigation/BottomCanvasToolbar", TOOLBAR))
    if right:
        out.append(at("Panel/RightInspector", "Panel/RightInspector", RIGHT))
    return out


def spec(flow, case, place, canvas=CANVAS, aliases=ALIASES, **extra):
    """One screen: 1440 x 900, drawn from the capture `case`; `canvas` None for a window with no canvas rectangle behind it."""
    s = {"name": f"Screen/{flow}/{case}", "w": 1440, "h": 900, "state": case, "place": place, "aliases": aliases,
         "canvas": (*canvas, BACKGROUND, "Canvas background") if canvas else None}
    return gap_wiring.regions(dict(s, **extra))
