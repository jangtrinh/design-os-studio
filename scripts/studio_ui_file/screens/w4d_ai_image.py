"""The AI image screens of batch W4D: the file panel open, a layer selected, and the menu or popover of the image tools (Remove background, Isolate subject, Expand, Erase).

The menu is `Menu/Panel` at `(1216, 668, 180, 161)` with its `Body` slot holding the four `Menu/Item` rows and one `Menu/Separator` of `image_jobs.rs` `paint_menu`; the two dialogs are
Popover/Panel `Kind=Erase` and `Kind=Expand` at the capture's rect (`gap_overlays_data_menus.MENU`, `gap_overlays_data.POPOVER`).
"""
from .gap_wiring import overlays
from .w4d_common import LAYERS_ALIASES, LAYERS_CANVAS, layers_shell, screen

CASES = ["ai-image-menu", "ai-image-erase", "ai-image-expand"]


def screens():
    return [screen(case, LAYERS_CANVAS, layers_shell(overlays(case)), aliases=LAYERS_ALIASES) for case in CASES]
