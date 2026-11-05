"""Per-screen data of the Variables windows, the Present view and the Home tab bar: plain dicts, no `screens()`: a screen module patches the instance it places with an entry.

VARIABLES_PRESENT = {state: {"patch": {component: {key: value}}, "add": [place entries]}}: `patch` edits the instance the screen already places (a key set to None is removed), `add` places more.
* `k-variables-floating`: Window/Variables is the 642 x 380 floating window (`Layout=Floating`), drawn at its own size (view.rs:88-94 `floating_rect`). `micro-components-variables` has no
  Variables window in its shell (its capture shows the same floating window at (0, 8)), so its entry places one.
* `present-variables`: the `Variables` toggle is on (`Show Variables active`) and the live variables panel (`Layout=Present`, 264 x 836) sits at (1164, 56) (present.rs:804-869).
* `h-home`: the Home view has no project tab (`Show project tab` off, so the `+` follows the divider at x 94) and the Home tab is the selected one (`Show Home selected`, tabs.rs:963-971); the old
  `project-tab-0` visibility override goes (the prop replaces it).
* `k-variables` draws `Layout=Full` (the first value) with the capture's 18 default rows (four group headers, 14 colour variables): no data.
"""
FLOATING = {"patch": {"Window/Variables": {"variant": {"Layout": "Floating"}, "w": None, "h": None}}}
FLOATING_AT = {"name": "Window/Variables", "component": "Window/Variables", "variant": {"Layout": "Floating"}, "x": 0, "y": 8}
VARIABLES_PRESENT = {
    "k-variables-floating": FLOATING,
    "micro-components-variables": {"add": [FLOATING_AT]},
    "present-variables": {"patch": {"Bar/Presentation": {"props": {"Show Variables active": True}}},
                          "add": [{"name": "Window/Variables", "component": "Window/Variables", "variant": {"Layout": "Present"}, "x": 1164, "y": 56}]},
    "h-home": {"patch": {"Navigation/TopTabBar": {"props": {"Show project tab": False, "Show Home selected": True}, "overrides": None}}},
}
