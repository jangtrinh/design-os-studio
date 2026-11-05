"""Screen/EditorDesign/d-menu-main, d-menu-main-file, d-menu-shape, d-menu-zoom: the editor shell of `d-editor-default` with one open menu (or two) on top, each a Menu/Panel at the rect of the
capture's overlay (layout.json `overlays`: the main menu (12, 44, 210, 581), the File submenu (222, 142, 265, 434), the shape menu (619, 636, 246, 208), the zoom menu (1216, 44, 212, 453)).

The menus and their rows are `gap_overlays_data_menus.MENU` (production's panels/menus.rs: the main menu's top entries, the File submenu, the tool menu and the zoom menu with its input row);
where a menu has a check column every label sits at 32 or 60 (`Layout::has_check`), which Menu/Item draws with `Show check column`.
"""
from screens.gap_wiring import overlays
from screens.w4a_common import screen, shell

CASES = ("d-menu-main", "d-menu-main-file", "d-menu-shape", "d-menu-zoom")


def screens():
    return [screen(case, shell() + overlays(case)) for case in CASES]
