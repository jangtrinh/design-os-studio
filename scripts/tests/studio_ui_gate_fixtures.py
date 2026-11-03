"""Synthetic native `.designos` documents shared by the Studio UI gate tests (not a test module)."""
import sys
from pathlib import Path

SCRIPTS = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(SCRIPTS))
import studio_ui_theme_reader as theme  # noqa: E402

REPO = SCRIPTS.parent
THEME_SRC = "pub const BG_SECONDARY: Color32 = rgb(0x171717);\npub const SPACE_FACT: f32 = 4.0;\npub const SPACE_INLINE: f32 = 8.0;\n"
ROWS = theme.flatten(theme.read_theme_constants(THEME_SRC))
MODE = "dark"
GREY = {"r": 23 / 255, "g": 23 / 255, "b": 23 / 255, "a": 1.0}
CORNERS = ("topLeftRadius", "topRightRadius", "bottomRightRadius", "bottomLeftRadius")


REGISTRY = {"components": [{"name": "Divider/Hairline", "category": "element", "variants": ["State=Default"]},
                           {"name": "Divider/Plain", "category": "element"},
                           {"name": "Button/Icon", "category": "component", "variants": ["Size=Xs", "Size=Sm", "State=Default", "State=Hover"]}]}


def var(vid, name, col, value, code=None, kind="Color", hidden=False, scopes=()):
    v = {"id": vid, "name": name, "collection": col, "kind": kind, "values": {MODE: value}, "description": "", "scopes": list(scopes),
         "hidden_from_publishing": hidden, "key": None}
    if code:
        v["code_syntax"] = {"WEB": code}
    return v


def solid(c=GREY, visible=True):
    return {"kind": {"Solid": c}, "opacity": 1.0, "visible": visible, "blend": "Normal"}


def node(nid, name, kind="Rectangle", parent=None, children=(), **extra):
    return {"id": nid, "name": name, "kind": kind, "parent": parent, "children": list(children), **extra}


def make_doc():
    cols = {c: {"id": c, "name": n, "modes": [{"id": MODE, "name": "Dark"}], "default_mode": MODE, "variables": []}
            for c, n in (("cp", "Primitives"), ("cs", "Semantic"))}
    variables = {
        "p.grey": var("p.grey", "neutral/900", "cp", {"Color": GREY}, hidden=True),
        "p.4": var("p.4", "space/4", "cp", {"Number": 4.0}, kind="Number", hidden=True),
        "p.8": var("p.8", "space/8", "cp", {"Number": 8.0}, kind="Number", hidden=True),
        "s.bg": var("s.bg", "color/bg-secondary", "cs", {"Alias": "p.grey"}, "theme::BG_SECONDARY"),
        "s.fact": var("s.fact", "space/fact", "cs", {"Alias": "p.4"}, "theme::SPACE_FACT", "Number"),
        "s.inline": var("s.inline", "space/inline", "cs", {"Alias": "p.8"}, "theme::SPACE_INLINE", "Number"),
    }
    # The token keys the committed G01 and non-colour maps assign to these constants.
    for vid, key in (("s.bg", "colors.surface"), ("s.fact", "spacing.space-1"), ("s.inline", "spacing.space-2")):
        variables[vid]["description"] = f"token {key}"
    nodes = {
        "1": node(1, "00 Start Here", "Page"),
        "2": node(2, "01 Foundations", "Page", children=[4]),
        "3": node(3, "02 Core", "Page", children=[40, 42, 43]),
        "40": node(40, "Divider/Hairline", {"ComponentSet": {}}, parent=3, children=[41], component={"props": [
            {"name": "State", "kind": {"Variant": {"options": ["Default"]}}, "default": {"Variant": "Default"}}]}),
        "41": node(41, "State=Default", {"Component": {}}, parent=40),
        # one instance that mirrors a main (a plain component, not a set): the override gate refuses a document with none
        "42": node(42, "Divider/Plain", {"Component": {}}, parent=3),
        "43": node(43, "Divider", {"Instance": {"frame": {}, "component": 42, "component_key": "k"}}, parent=3, component={"source": 42}),
        "9": node(9, "03 Studio", "Page"),
        "4": node(4, "Swatch", parent=2, fills=[solid()], corner_radii=[4.0] * 4, variables={"modes": {}, "bindings": {
            "fills/0": "s.bg", **{k: "s.fact" for k in CORNERS}}}),
    }
    return {"name": "t", "kind": "Design", "nodes": nodes, "pages": [1, 2, 3, 9], "variables": variables, "variable_collections": cols}


def with_node(doc, **extra):
    """Replace the Swatch node's fields (extra) and return the node."""
    doc["nodes"]["4"].update(extra)
    return doc["nodes"]["4"]


def frame_kind(body_key="Frame", **auto_layout):
    al = {"item_spacing": 0.0, "counter_spacing": 0.0, "padding": [0.0] * 4, **auto_layout}
    if body_key == "Instance":
        return {"Instance": {"frame": {"auto_layout": al}, "component": 7, "component_key": "k"}}
    return {body_key: {"auto_layout": al}}
