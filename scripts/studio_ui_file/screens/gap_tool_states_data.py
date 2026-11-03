"""Per-screen data of the left rail's active tool and the bottom toolbar's active tool: plain dicts, no `screens()`: a screen module patches the instance it places with an entry.

TOOLS = {state: {"patch": {component: {"variant": {axis: value}}}}} for the screens whose capture shows another tile selected than File and Move:
* `Navigation/LeftRail` `Tool=Agents` on the 11 agents captures (`AGENTS`), `Tool=Assets` on `k-assets` and `Tool=Variables` on `k-variables` (`RAIL_13` less the 11 above is these two): the rail
  draws the tile of the open panel selected (panels/rail.rs `show`), measured from the tile fill (38 selected, 10 at rest) of every capture.
* `Navigation/BottomCanvasToolbar` `State=Comment` on `j-comments` (the Comment tool selected) and `State=AI` on `j-cmdk` and `j-palette` (the Move tool and the Actions button selected).
A nested `variant` override of a tile loses the swapped glyph and the fill (the step-0 spike), so the tool is a registry axis and the placed instance picks the value.
"""
from screens.gap_table_citations import AGENTS

RAIL = {**{state: "Agents" for state in AGENTS}, "k-assets": "Assets", "k-variables": "Variables"}
TOOLBAR = {"j-comments": "Comment", "j-cmdk": "AI", "j-palette": "AI"}
TOOLS = {state: {"patch": {"Navigation/LeftRail": {"variant": {"Tool": tool}}}} for state, tool in RAIL.items()}
for _state, _tool in TOOLBAR.items():
    TOOLS.setdefault(_state, {"patch": {}})["patch"]["Navigation/BottomCanvasToolbar"] = {"variant": {"State": _tool}}
