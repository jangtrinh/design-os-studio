"""The Variables window of the 0.7.5 captures `k-variables` (full window) and `k-variables-floating` (the floating window over the editor): `Screen/EditorDesign/<case>`.

  k-variables           the tab bar, the rail and Window/Variables (49, 32, 1391, 868): no canvas, no panels (`regions.variables_full`)
  k-variables-floating  the editor shell and the floating window (0, 8, 642, 380): Window/Variables `Layout=Floating` (`gap_variables_present_data`)
"""
from screens.w4c_shell import at, shell, spec


def screens():
    return [
        spec("EditorDesign", "k-variables", shell(left=False, right=False, toolbar=False, rulers=False) + [at("Window/Variables", "Window/Variables", (49, 32))], canvas=None, aliases=None),
        spec("EditorDesign", "k-variables-floating", shell() + [at("Window/Variables", "Window/Variables", (0, 8))]),
    ]
