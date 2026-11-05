"""Screen/EditorDesign/<case> for the captures with a layer selected and no overlay (`fill-d9`, `micro-inspector-math`, `micro-typography-multiedit`, `micro-components-variables`) and
Screen/EditorPrototype/f-prototype-tab.

All five are the shell: Panel/RightInspector draws the state of the capture (`Frame`, `Layers`, `Prototype`, `gap_right_panel_data`) and Panel/LeftLayers its layer tree. `fill-d9` is
`d-editor-selected` with the dirty tab's dot (`gap_tab_bar_data`), and `micro-components-variables` adds the floating Variables window (642 x 380 at (0, 8), `Window/Variables` `Layout=Floating`).
The Interaction details popover of `f-prototype-tab` is Popover/Panel `Kind=Interaction`.
"""
from screens.gap_wiring import overlays
from screens.w4b_shell import screen, shell

FLOW = "EditorDesign"


def screens():
    return [screen(FLOW, "fill-d9", shell()), screen(FLOW, "micro-inspector-math", shell()), screen(FLOW, "micro-typography-multiedit", shell()),
            screen(FLOW, "micro-components-variables", shell()), screen("EditorPrototype", "f-prototype-tab", shell() + overlays("f-prototype-tab"))]
