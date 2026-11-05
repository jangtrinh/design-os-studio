"""Screen/EditorDesign in the state `d-editor-default` (shell.rs `draw_window`, the 0.7.5 capture `d-editor-default`, 1440 x 900): a design file open with nothing selected, the file
  panel open, the
Move tool active, rulers on, at 2 %. Every region sits at the rect the capture's layout.json gives it (`regions`): the tab bar (0, 0, 1440, 32), the rail (0, 32, 49, 868), the left
  panel (49, 32, 241, 868), the
canvas (290, 32, 909, 868) with the rulers over it and the toolbar (464, 840, 512, 48) over its bottom, and the right panel (1199, 32, 241, 868). No capture shows Navigation/TitleBar
  (`title_bar` is null in all
152 layout files: the capture harness has a shot, and the bar is hidden then, shell.rs:1737), so the screen does not place it.

The canvas background is the page's own colour, #F5F5F5 (the Page section of the right panel reads it); the capture's canvas content (a kit's frames) is not drawn: it is the document's,
  and it is a kit's.
"""

# The two header buttons of the Layers row are nested actions of a Panel/Header (nested instances cannot be renamed or exposed: #1471): the collapse button is its
# `Collapse action`, the filter its `Secondary action`:
# the geometry check finds them by that path (screen_geometry `PATH_SEP`).
ALIASES = {"layers-collapse-all": "Panel/LeftLayers > Layers header > Collapse action", "layers-search": "Panel/LeftLayers > Layers header > Secondary action"}
TAB_BAR, RAIL, LEFT, RIGHT = (0, 0), (0, 32), (49, 32), (1199, 32)
CANVAS = (290, 32, 909, 868)


def at(name, component, xy, variant=None):
    return {"name": name, "component": component, "variant": variant, "x": xy[0], "y": xy[1]}


def screens():
    return [{
        "name": "Screen/EditorDesign", "w": 1440, "h": 900, "state": "d-editor-default",
        "canvas": (*CANVAS, "#F5F5F5", "Canvas background"),
        "aliases": ALIASES,
        "place": [at("Navigation/TopTabBar", "Navigation/TopTabBar", TAB_BAR, {"Bar": "32px"}), at("Navigation/LeftRail", "Navigation/LeftRail", RAIL, {"Rail": "48px"}),
                  at("Panel/LeftLayers", "Panel/LeftLayers", LEFT), at("Navigation/Rulers", "Navigation/Rulers", CANVAS[:2]),
                  at("Navigation/BottomCanvasToolbar", "Navigation/BottomCanvasToolbar", (464, 840)), at("Panel/RightInspector", "Panel/RightInspector", RIGHT)]}]
