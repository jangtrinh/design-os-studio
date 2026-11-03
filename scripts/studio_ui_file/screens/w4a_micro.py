"""Screen/EditorDesign/picker-page and the eight canvas micro states (micro-layer-picker, micro-smart-selection, micro-selection-corner, micro-scale-tool, micro-vector-edit,
micro-shape-controls, micro-autolayout, micro-css-grid): the editor shell with the capture's canvas as the document's content (not drawn: it is the open file's, and a kit's) and, where the
capture shows one, its overlay.

* `picker-page`: the colour picker open over the canvas from the Page section's fill, Popover/ColorPicker `Kind=Page` at the overlay rect (960, 123, 240, 498).
* `micro-layer-picker`: the "Select layer" list (`show_layer_picker`, context.rs:169) at the overlay rect (680, 410, 220, 294): a Menu/Panel (`gap_overlays_data_menus.MENU`).
* `micro-vector-edit`: the vector tools (`vtool:Move`, `vtool:Bend`, `vtool:Paint bucket`, (618.5, 800, 252, 28)) are a Tabs/Segmented with its third cell shown (`gap_overlays_data.VECTOR`).
* the others draw the shell only, its regions drawing the capture's selection inspector and layer rows (`gap_wiring.regions`); the zoom levels of the rulers stay gaps the gate data pins.
"""
from screens.gap_wiring import overlays
from screens.w4a_common import screen, shell

CASES = ("picker-page", "micro-layer-picker", "micro-smart-selection", "micro-selection-corner", "micro-scale-tool", "micro-vector-edit", "micro-shape-controls", "micro-autolayout",
         "micro-css-grid")


def screens():
    return [screen(case, shell() + overlays(case)) for case in CASES]
