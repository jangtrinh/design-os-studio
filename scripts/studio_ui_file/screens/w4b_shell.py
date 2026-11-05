"""Shared parts of the W4B screens (Inspector, styles and dev): the editor shell and the screen spec.

Every capture of the batch is the 0.7.5 editor window at 1440 x 900 with the regions of `Screen/EditorDesign` at the same rects (the layouts' `regions` are equal), so a screen is the shell
plus what the capture adds: a popover (Popover/Panel of the capture's `Kind`), a menu (Menu/Panel with Menu/Item rows) or the colour picker, which `gap_wiring.overlays` gives. Nothing here
draws: an instance carries the capture's texts as component data (a prop, an override, a slot of instances), never as a local text.

    shell(right="Panel/RightInspector")                 the six shell instances, painted bottom to top
    screen(flow, case, place, scrim=None)               the spec: `Screen/<flow>/<case>`, the capture `case` is the state; its regions draw the state the capture shows
"""
from screens import gap_wiring
from screens.editor_design import ALIASES, CANVAS, LEFT, RAIL, RIGHT, TAB_BAR, at


def shell(right="Panel/RightInspector"):
    return [at("Navigation/TopTabBar", "Navigation/TopTabBar", TAB_BAR, {"Bar": "32px"}), at("Navigation/LeftRail", "Navigation/LeftRail", RAIL, {"Rail": "48px"}),
            at("Panel/LeftLayers", "Panel/LeftLayers", LEFT), at("Navigation/Rulers", "Navigation/Rulers", CANVAS[:2]),
            at("Navigation/BottomCanvasToolbar", "Navigation/BottomCanvasToolbar", (464, 840)), at(right, right, RIGHT)]


def screen(flow, case, place, scrim=None):
    return gap_wiring.regions({"name": f"Screen/{flow}/{case}", "w": 1440, "h": 900, "state": case, "canvas": (*CANVAS, "#F5F5F5", "Canvas background"), "aliases": ALIASES, "place": place,
                               "scrim": scrim})
