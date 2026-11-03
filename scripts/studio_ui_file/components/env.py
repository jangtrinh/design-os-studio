"""What a component module needs from the rest of the build: Semantic names by theme constant, kit looks, nested instances.

    env.c("BG_BRAND")            -> "color/bg-brand"      (Semantic variable name of a theme.rs constant; unknown constant fails)
    env.num("CONTROL_XS")        -> 32.0                  (its value, for fixed widths and heights that are not bound)
    env.look("Button/Primary", "Hover") -> {"fill", "border", "text", "icon", "ring"} Semantic names (or None) + "alpha"
    env.estimate(node)           -> (w, h) before layout; a nested X without `abs` is as big as its main's estimate (the mains lowered so far)
The looks are read from crates/designos-app/src/widgets/kit/testdata/kit-states.json, the table the kit's look functions are
tested against, so a colour is never retyped here. Registry state names map to the kit's: Active=Pressed, Focus=Focused.
"""
import json
import pathlib

import rest_ir as ir

KIT_STATES_REL = "crates/designos-app/src/widgets/kit/testdata/kit-states.json"
KIT_STATE = {"Default": "Default", "Hover": "Hover", "Active": "Pressed", "Focus": "Focused", "Disabled": "Disabled", "Selected": "Selected"}


class Env:
    def __init__(self, repo, plan, icons, mains=None):
        self.icons = icons
        self._mains = mains if mains is not None else {}  # (set, variant) -> (w, h) of the lowered mains: the Lowerer's own dict, kept by reference, never copied
        self.by_const = {s["const"]: s for s in plan["semantic"] if s["code_syntax"] == f"theme::{s['const']}"}
        self.rows = json.loads((pathlib.Path(repo) / KIT_STATES_REL).read_text(encoding="utf-8"))["rows"]

    def c(self, const):
        if const not in self.by_const:
            raise KeyError(f"theme constant {const} has no Semantic variable")
        return self.by_const[const]["name"]

    def num(self, const):
        self.c(const)
        return float(self.by_const[const]["value"])

    def estimate(self, node):
        return ir.estimate(node, mains=self._mains)

    def look(self, row, state):
        cell = self.rows[row][KIT_STATE[state]]
        if cell is None:
            raise KeyError(f"kit-states.json has no {KIT_STATE[state]} cell for {row}")
        out = {k: (self.c(cell[k]) if cell.get(k) else None) for k in ("fill", "border", "text", "icon", "ring")}
        out["alpha"] = cell["alpha"]
        return out
