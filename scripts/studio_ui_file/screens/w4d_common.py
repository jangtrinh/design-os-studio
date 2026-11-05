"""Shared pieces of the Agents & AI screens (batch W4D): the window shells, the overlay instances and the rects the 0.7.5 captures give.

Two shells, both `Screen/EditorDesign/<case>` (the Design tab is open and the Move tool is active in every capture of the batch):

    agents_shell(...)      the agents panel open: rail Agents active, `Panel/AgentTerminal` (49, 32, 401, 868), the canvas (450, 32, 749, 868) with the rulers and the toolbar over it,
                           the right inspector
    layers_shell(...)      the file panel open (`ai-image-*`): `Panel/LeftLayers` (49, 32, 241, 868), the canvas (290, 32, 909, 868)

Both are built from the instances the regions give, in paint order: the rulers sit under the right inspector, because `Navigation/Rulers` is drawn for the 909 px canvas of `d-editor-default`.
`popover` and `menu` place the closest existing overlay instance at the rect a capture gives; an instance keeps its main's inner layout, so only its size, its texts and what it hides change.
"""
from screens import gap_wiring

TAB_BAR, RAIL, LEFT, RIGHT, TOOLBAR = (0, 0), (0, 32), (49, 32), (1199, 32), (464, 840)
AGENTS_CANVAS = (450, 32, 749, 868)
LAYERS_CANVAS = (290, 32, 909, 868)
LAYERS_ALIASES = {"layers-collapse-all": "Panel/LeftLayers > Layers header > Collapse action", "layers-search": "Panel/LeftLayers > Layers header > Secondary action"}
_CLIS = ("claude", "codex", "grok", "antigravity", "gemini", "other-cli", "shell")
_MENTION = {"agent-mention-btn-left": "Panel/AgentTerminal > Mention"}
_CHIP = {"agent-selection-chip": "Panel/AgentTerminal > Selection chip", "agent-selection-chip-left": "Panel/AgentTerminal > Selection chip"}
_HDR = {"agents-header-skills": "Panel/AgentTerminal > Skills", "agents-runtime-add-btn": "Panel/AgentTerminal > Add session", "agents-sessions-btn": "Panel/AgentTerminal > Sessions"}
_COMP = {"agents-composer-attach": "Panel/AgentTerminal > Composer > Attach", "agents-composer-book": "Panel/AgentTerminal > Composer > Book",
         "agents-composer-box": "Panel/AgentTerminal > Composer", "agents-composer-context": "Panel/AgentTerminal > Composer > PRD",
         "agents-composer-plus": "Panel/AgentTerminal > Composer > Attach", "agents-composer-prd": "Panel/AgentTerminal > Composer > PRD",
         "agents-composer-send": "Panel/AgentTerminal > Composer > Send", "agents-composer-skills": "Panel/AgentTerminal > Composer > Skills"}
_PICKER = {"agents-new-session-picker": "Panel/AgentTerminal > Picker", **{f"agents-sample-prompt-{i}": f"Panel/AgentTerminal > Prompt {i + 1}" for i in range(5)},
           **{f"picker-row-agents-dropdown-{k}": f"Panel/AgentTerminal > Picker > CLI {i}" for i, k in enumerate(_CLIS, 1)}}
AGENTS_ALIASES = {**_MENTION, **_CHIP, **_HDR, **_COMP, **_PICKER}


def at(name, component, xy, variant=None, **extra):
    """One placed instance; `extra` carries what an instance takes beyond a position and a variant (`props`, `overrides`, `slots`, `w`, `h`)."""
    return {"name": name, "component": component, "variant": variant, "x": xy[0], "y": xy[1], **extra}


def hide(*paths):
    """Overrides that hide the named children of an instance (a visibility is data, not style)."""
    return [{"path": p, "visible": False} for p in paths]


def say(path, text):
    return {"path": path, "text": text}


def popover(rect, title, line=None, name="Popover/Panel", hide_paths=("Header/Close",)):
    """`Popover/Panel` at a capture's popover rect: its header title and its one body line carry the capture's words (Studio's own strings)."""
    x, y, w, h = rect
    ov = [*hide(*hide_paths), *([say("Body/Row/Label", line)] if line else [])]
    return at(name, "Popover/Panel", (x, y), w=w, h=h, props={"Title": title}, **({"overrides": ov} if ov else {}))


def agents_shell(extra=(), terminal=None, inspector=None):
    """The agents panel open. `extra` instances (overlays) are painted last."""
    return [at("Navigation/TopTabBar", "Navigation/TopTabBar", TAB_BAR, {"Bar": "32px"}), at("Navigation/LeftRail", "Navigation/LeftRail", RAIL, {"Rail": "48px"}),
            at("Panel/AgentTerminal", "Panel/AgentTerminal", LEFT, **(terminal or {})), at("Navigation/Rulers", "Navigation/Rulers", AGENTS_CANVAS[:2]),
            at("Navigation/BottomCanvasToolbar", "Navigation/BottomCanvasToolbar", TOOLBAR), at("Panel/RightInspector", "Panel/RightInspector", RIGHT, **(inspector or {})), *extra]


def layers_shell(extra=()):
    """The file panel open, no selection drawn in the panels (the regions' state)."""
    return [at("Navigation/TopTabBar", "Navigation/TopTabBar", TAB_BAR, {"Bar": "32px"}), at("Navigation/LeftRail", "Navigation/LeftRail", RAIL, {"Rail": "48px"}),
            at("Panel/LeftLayers", "Panel/LeftLayers", LEFT), at("Navigation/Rulers", "Navigation/Rulers", LAYERS_CANVAS[:2]),
            at("Navigation/BottomCanvasToolbar", "Navigation/BottomCanvasToolbar", TOOLBAR), at("Panel/RightInspector", "Panel/RightInspector", RIGHT), *extra]


def screen(case, canvas, place, aliases=None, **extra):
    """A screen spec in the shape `screens/__init__.py` documents; the frame is named after the registry screen it draws and the capture."""
    return gap_wiring.regions({"name": f"Screen/EditorDesign/{case}", "w": 1440, "h": 900, "state": case, "canvas": (*canvas, "#F5F5F5", "Canvas background"), "aliases": aliases or {},
                               "place": place, **extra})
