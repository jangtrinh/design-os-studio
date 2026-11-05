"""Screen/EditorDev/<case>: Dev mode (`g-dev-react`, `g-dev-annotate`, `g-dev-compare`, `micro-dev-boxmodel`).

The right panel is Panel/RightInspector `State=Dev` for the top row, the footer and the border (Dev mode's Inspect / Plugins tabs and green Share) with Panel/DevInspect over its content from
(1199, 113) to the scrollbar end (241 x 751, the clip the region records: `Panel/DevInspect`): `g-dev-react` draws `State=Inspect`, `g-dev-annotate` `Annotate` (the pin chip editor),
`micro-dev-boxmodel` `Component` (the playground and box model sections). The left panel is Panel/LeftLayers with the Dev list over its Layers part (the `Ready for dev` / `Layers` tabs and the
status rows), the toolbar the 266 px green Dev toolbar and the tab bar shows the Dev tile and the unsaved dot (`gap_left_dev_data.DEV`). `g-dev-compare` places Dialog/Modal `Kind=Compare`
(818 x 638 at (310, 130), `gap_dialogs_data.DIALOG`) over a scrim of black at alpha 140, the Scrim rectangle the framework allows a state to declare (SCRIMS in screens/w4b_gate.py).
"""
from screens.editor_design import at
from screens.gap_wiring import overlays
from screens.w4b_shell import screen, shell

FLOW = "EditorDev"
SCRIM = {"fill": "#0000008C", "above": "Panel/DevInspect"}  # dev/compare.rs:97 `Color32::from_black_alpha(140)`: black at 140 / 255


def _dev():
    return shell() + [at("Panel/DevInspect", "Panel/DevInspect", (1199, 113))]


def screens():
    return [screen(FLOW, "g-dev-react", _dev()), screen(FLOW, "g-dev-annotate", _dev()), screen(FLOW, "g-dev-compare", _dev() + overlays("g-dev-compare"), scrim=SCRIM),
            screen(FLOW, "micro-dev-boxmodel", _dev())]
