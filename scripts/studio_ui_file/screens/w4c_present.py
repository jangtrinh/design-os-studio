"""Presentation mode in the 0.7.5 captures `x-present` and `present-variables`: `Screen/Present/<case>`, the bar (0, 0, 1440, 48) over the page's colour
(no tab bar, rail or panels: `regions.present`).

  x-present          Bar/Presentation; the page the capture shows is the document's content (a kit's frames), so the screen draws the page colour only
  present-variables  the same, with the Variables toggle on and Window/Variables `Layout=Present` (1164, 56, 264, 836) over the right edge (`paint_vars_panel`)
"""
from screens.w4c_shell import at, spec

WINDOW = (0, 0, 1440, 900)


def screens():
    bar = [at("Bar/Presentation", "Bar/Presentation", (0, 0))]
    return [spec("Present", "x-present", bar, canvas=WINDOW, aliases=None), spec("Present", "present-variables", bar, canvas=WINDOW, aliases=None)]
