"""Screen/EditorDesign/<case> for the captures whose overlay is one of Studio's settings popovers (`widgets/popover.rs`, 240 wide; the type settings 312): the style editors
(`edit-style-*`, `f-edit-style-color`), the effect settings (`f-stroke-settings`, `effect-noise`, `effect-glass`, `effect-texture`) and the type settings (`f-type-details`).

The popover is Popover/Panel of the `Kind` the capture shows at the overlay's rect from the capture's layout.json (`overlays`): its title, label + control rows and controls are the kind's own
(`gap_overlays_data.POPOVER`). The type settings popover is 312 wide where the others are 240.
"""
from screens.gap_wiring import overlays
from screens.w4b_shell import screen, shell

FLOW = "EditorDesign"
CASES = ("f-edit-style-color", "edit-style-effect", "edit-style-grid", "edit-style-text", "f-stroke-settings", "effect-noise", "effect-glass", "effect-texture", "f-type-details")


def screens():
    return [screen(FLOW, case, shell() + overlays(case)) for case in CASES]
