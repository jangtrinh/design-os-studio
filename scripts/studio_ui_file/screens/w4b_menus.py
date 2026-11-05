"""Screen/EditorDesign/<case> for the captures whose overlay is a menu, a list or the colour picker: `f-effects-menu` (the add-effect menu), `grid-styles` (the grid style menu),
`f-effect-styles` (the effect style list) and `f-picker`, `f-picker-gradient` (the colour picker).

The menus are Menu/Panel at the overlay's rect (layout.json `overlays`) with the capture's rows as a Body slot of Menu/Item instances at the panel's inner width and Menu/Separator
(`gap_overlays_data_menus.MENU`). Studio's own labels are kept (the effect kinds, the grid presets, the style actions); the effect style list's kit style names and group captions are replaced by
neutral ones of the same kind (`Group n` captions as disabled rows, `Style n` rows, the first one selected). The grid presets and style rows show their check column (labels at 32).
The picker is Popover/ColorPicker (`Kind=Fill` at (960, 346), its own 240 x 538; `Kind=Gradient` at (960, 332), 552 tall with the stops row: `gap_dialogs_data.PICKER`).
"""
from screens.gap_wiring import overlays
from screens.w4b_shell import screen, shell

FLOW = "EditorDesign"
CASES = ("f-effects-menu", "grid-styles", "f-effect-styles", "f-picker", "f-picker-gradient")


def screens():
    return [screen(FLOW, case, shell() + overlays(case)) for case in CASES]
