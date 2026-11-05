"""Per-screen data of the settings popovers, dialogs and the vector bar. Plain dicts, no `screens()`, no `_gate.py` suffix; `gap_wiring.overlays` places them. The menus are in
gap_overlays_data_menus.py.

    POPOVER   {state: place entry}: the Popover/Panel of that capture's overlay, its `Kind` the one production popover the capture shows, at the overlay's origin (layout.json `overlays`
              or the named popover rect). The size is the variant's own, so the entry has no `w`/`h` and none of today's `popover_places` extras (a 312 px kind needs no close or hairline).
              A kind draws neutral content where the capture shows a kit's own name (the style `Name`, `Description`): the rects of those texts, relative to the popover, are TEXTS (below).
    VECTOR    {state: place entry}: the vector sub-tool bar, Tabs/Segmented with its third cell shown, at the rect of the capture's `vtool:*` widgets (618.5, 800, 252, 28).
    TEXTS     {state: [(rect in the screen, what, the neutral text the screen draws)]}: the kit strings and the machine's own strings the overlays replace, for the screen gate's TEXTS masks.
    ALIASES   {state: {layout key: path of names below the screen}}: the named rects of the capture that a kind's parts draw, joined by ` > ` as `screens/__init__.py` documents.
"""
from screens.editor_design import at


def kind(name, kind_name, x, y):
    """A Popover/Panel of one Kind at (x, y), named after the capture's widget where the capture names the popover."""
    return at(name, "Popover/Panel", (x, y), {"State": "Default", "Kind": kind_name})


POPOVER = {
    "f-edit-style-color": kind("Popover/Panel", "EditStyleColor", 960, 412),
    "edit-style-effect": kind("Popover/Panel", "EditStyleEffect", 960, 412),
    "edit-style-grid": kind("Popover/Panel", "EditStyleGrid", 960, 412),
    "edit-style-text": kind("Popover/Panel", "EditStyleText", 960, 412),
    "f-stroke-settings": kind("Popover/Panel", "Stroke", 960, 668),
    "effect-noise": kind("Popover/Panel", "Noise", 960, 668),
    "effect-glass": kind("Popover/Panel", "Glass", 960, 636),
    "effect-texture": kind("Popover/Panel", "Texture", 960, 732),
    "f-type-details": kind("Popover/Panel", "TypeSettings", 888, 156),
    "f-prototype-tab": kind("proto-popover", "Interaction", 952, 105),
    "ai-image-erase": kind("ai-erase-popover", "Erase", 944, 640),
    "ai-image-expand": kind("ai-expand-popover", "Expand", 956, 640),
    "j-comments": kind("Thread popover", "CommentThread", 801, 381),
    "agents-skills": kind("agents-skills-picker-popover", "Skills", 57, 608),
    "i-agent-permissions": kind("agent-permissions-modal", "Permissions", 540, 326),
}

VECTOR = {"micro-vector-edit": {"name": "Vector tools", "component": "Tabs/Segmented", "variant": {"State": "Selected"}, "x": 618, "y": 800, "w": 252, "h": 28,
                                "props": {"First label": "Move", "Second label": "Bend", "Third label": "Paint bucket", "Show cell 3": True}}}

# The style `Name` of the four style popovers is a kit's style name in the capture; the screen draws "Style name" (the variant's default) in the name field's value text. The rect is the popover's
# name row control column: x 960 + 112, y 412 + 40 (header) + 8, 112 x 24.
TEXTS = {state: [((1072, 460, 112, 24), "style name in the name field", "Style name")] for state in ("f-edit-style-color", "edit-style-effect", "edit-style-grid", "edit-style-text")}
TEXTS["edit-style-effect"].append(((1072, 492, 112, 24), "style description in the description field", ""))
# The thread's message is a document string (the capture's differs from the neutral one); the skill names, descriptions and the user-skills path are the machine's own skills; the layer picker's
# eight names are a kit's. Rects are in the screen: popover origin plus each part's offset (message (44, 64); skill row i at (8, 32 + 44 i), title (24, 3), description (24, 19), path (12, 174)).
TEXTS["j-comments"] = [((845, 445, 244, 18), "thread message", "Align icon spacing to 8px grid")]
TEXTS["agents-skills"] = ([((89, 643 + 44 * i, 150, 18), f"skill {i + 1} name", f"{n} @1.0") for i, n in enumerate(("first-skill", "second-skill", "third-skill"))]
                          + [((89, 659 + 44 * i, 270, 18), f"skill {i + 1} description", d) for i, d in enumerate(("Describes what the first skill adds to an agent run.",
                             "Describes what the second skill adds to an agent run.", "Describes what the third skill adds to an agent run."))]
                          + [((69, 782, 220, 18), "user skills path", "User skills: /library/skills/<name>/SKILL.md")])
TEXTS["micro-layer-picker"] = [((714, 449 + 32 * i, 180, 18), f"layer name {i + 1}", n) for i, n in enumerate(("Text 1", "Frame 2", "Frame 3", "Frame 4", "Frame 5", "Frame 6", "Frame 7", "Frame 8"))]

ALIASES = {
    "j-comments": {"comment-resolve": "Thread popover > Resolve", "comment-dots-0": "Thread popover > More", "comment-reply-box": "Thread popover > Reply box",
                   "comment-reply-post": "Thread popover > Reply"},
    "ai-image-erase": {"ai-erase-instruction": "ai-erase-popover > Instruction", "ai-erase-cancel": "ai-erase-popover > Cancel", "ai-erase-run": "ai-erase-popover > Run"},
    "ai-image-expand": {"ai-expand-preset-25": "ai-expand-popover > Preset", "ai-expand-cancel": "ai-expand-popover > Cancel", "ai-expand-run": "ai-expand-popover > Run"},
    "i-agent-permissions": {"agent-perm-skip": "agent-permissions-modal > Skip permissions", "agent-perm-ask": "agent-permissions-modal > Ask before acting"},
    "agents-skills": {"agents-skills-picker-close": "agents-skills-picker-popover > Close"},
}
