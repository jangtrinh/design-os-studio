"""Synthetic documents for the override gate tests (not a test module): a main on page `02 Core` (Card/Main: a text and a frame), a host
component that holds an instance of it with the copied children linked back through `component.source`, and one builder per exception
(an icon slot, a Focus/Ring, a Swatch/Color, a Tree/Row)."""
import copy
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
sys.path.insert(0, str(HERE.parent))
from studio_ui_gate_fixtures import CORNERS, GREY, frame_kind, make_doc, node, solid, var  # noqa: E402

BLUE = {"r": 0.0, "g": 0.0, "b": 1.0, "a": 1.0}
AL = dict(frame_kind(direction="Horizontal", item_spacing=4.0, padding=[0.0, 12.0, 0.0, 12.0])["Frame"]["auto_layout"])
TEXT_STYLE = {"font_family": "Inter", "font_weight": 600, "font_size": 14.0, "line_height": {"Pixels": 20.0}, "fills": [solid()]}
PLACE = {"absolute": False, "horizontal": "Hug", "vertical": "Hug", "grow": 0.0}
MAIN, TEXT, BOX, HOST, INST, ITEXT, IBOX = 50, 51, 52, 60, 61, 62, 63


def add(doc, n):
    doc["nodes"][str(n["id"])] = n
    return n


def mirror(doc, src_id, new_id, parent, kind=None):
    """A copy of main node `src_id` that links back to it (as an instance copy does)."""
    n = copy.deepcopy(doc["nodes"][str(src_id)])
    n.update(id=new_id, parent=parent, children=[], component={"source": src_id})
    if kind is not None:
        n["kind"] = kind
    return add(doc, n)


def base():
    d = make_doc()
    d["variables"]["p.blue"] = var("p.blue", "blue/500", "cp", {"Color": BLUE}, hidden=True)
    d["variables"]["s.accent"] = var("s.accent", "color/bg-brand", "cs", {"Alias": "p.blue"}, "theme::BG_BRAND")
    d["variables"]["p.14"] = var("p.14", "space/14", "cp", {"Number": 14.0}, kind="Number", hidden=True)
    d["variables"]["s.indent"] = var("s.indent", "inset/layer-indent", "cs", {"Alias": "p.14"}, "theme::LAYER_INDENT", "Number")
    bind = {"fills/0": "s.bg", **{k: "s.inline" for k in CORNERS}}
    add(d, node(MAIN, "Card/Main", {"Component": {"auto_layout": copy.deepcopy(AL)}}, parent=3, children=[TEXT, BOX], fills=[solid()], corner_radii=[8.0] * 4,
                layout_child=PLACE, variables={"modes": {}, "bindings": bind}))
    add(d, node(TEXT, "Label", {"Text": {"content": "Label", "style": TEXT_STYLE, "auto_resize": "WidthAndHeight"}}, parent=MAIN,
                style_refs={"text": "Text sm / Semibold"}, layout_child=PLACE, variables={"modes": {}, "bindings": {"fills/0": "s.bg"}}))
    add(d, node(BOX, "Box", frame_kind(direction="Vertical", item_spacing=4.0), parent=MAIN, fills=[solid()], corner_radii=[4.0] * 4,
                layout_child=PLACE, variables={"modes": {}, "bindings": {"fills/0": "s.bg", **{k: "s.fact" for k in CORNERS}}}))
    add(d, node(HOST, "Host", {"Component": {}}, parent=3, children=[INST]))
    inst = mirror(d, MAIN, INST, HOST, {"Instance": {"frame": {"auto_layout": copy.deepcopy(AL)}, "component": MAIN, "component_key": "k"}})
    inst["children"] = [ITEXT, IBOX]
    mirror(d, TEXT, ITEXT, INST)
    mirror(d, BOX, IBOX, INST)
    d["nodes"]["3"]["children"] += [MAIN, HOST]
    return d


def set_text(doc, nid, **style):
    doc["nodes"][str(nid)]["kind"]["Text"]["style"] = dict(TEXT_STYLE, **style)


def icon_doc():
    """A host whose instance holds an `Icon/Plus` instance (a nested icon slot) with its glyph, first child."""
    d = base()
    add(d, node(90, "Icon/Plus", {"Component": {}}, parent=3, children=[91], fills=[]))
    add(d, node(91, "Glyph", "Vector", parent=90, fills=[solid(GREY)], variables={"modes": {}, "bindings": {"fills/0": "s.bg"}}))
    d["nodes"]["3"]["children"].append(90)
    slot = mirror(d, 90, 92, HOST, {"Instance": {"frame": {}, "component": 90, "component_key": "icon-Plus"}})
    slot["children"] = [93]
    d["nodes"][str(HOST)]["children"].append(92)
    mirror(d, 91, 93, 92)
    return d


def ring_doc():
    d = base()
    add(d, node(70, "Focus/Ring", {"ComponentSet": {}}, parent=3, children=[71]))
    add(d, node(71, "Tone=Brand", {"Component": {}}, parent=70, corner_radii=[12.0] * 4, stroke={"paints": [solid()], "weight": 2.0},
                variables={"modes": {}, "bindings": {"strokes/0": "s.bg"}}))
    d["nodes"]["3"]["children"].append(70)
    mirror(d, 71, 73, HOST, {"Instance": {"frame": {}, "component": 71, "component_key": "ring"}})
    d["nodes"][str(HOST)]["children"].append(73)
    mirror(d, MAIN, 74, HOST, {"Instance": {"frame": {"auto_layout": copy.deepcopy(AL)}, "component": MAIN, "component_key": "k"}})
    d["nodes"][str(HOST)]["children"].append(74)
    return d


def swatch_doc():
    d = base()
    add(d, node(80, "Swatch/Color", {"ComponentSet": {}}, parent=3, children=[81]))
    add(d, node(81, "Default", {"Component": {}}, parent=80, children=[82]))
    add(d, node(82, "Color", {"Frame": {}}, parent=81, fills=[solid()], variables={"modes": {}, "bindings": {"fills/0": "s.bg"}}))
    d["nodes"]["3"]["children"].append(80)
    sw = mirror(d, 81, 92, HOST, {"Instance": {"frame": {}, "component": 81, "component_key": "sw"}})
    sw["children"] = [93]
    d["nodes"][str(HOST)]["children"].append(92)
    mirror(d, 82, 93, 92)
    return d


def tree_doc(pad_left):
    d = base()
    row_al = dict(AL, padding=[0.0, 8.0, 0.0, 0.0])
    add(d, node(80, "Tree/Row", {"ComponentSet": {}}, parent=3, children=[81]))
    add(d, node(81, "Depth=0", {"Component": {"auto_layout": row_al}}, parent=80))
    d["nodes"]["3"]["children"].append(80)
    row = mirror(d, 81, 82, HOST, {"Instance": {"frame": {"auto_layout": dict(row_al, padding=[0.0, 8.0, 0.0, pad_left])}, "component": 81,
                                                "component_key": "row"}})
    row["children"] = []
    d["nodes"][str(HOST)]["children"].append(82)
    return d


def on_page_03(doc):
    """Move the host component (and the instance it holds) from page `02 Core` to page `03 Studio`: the main stays on 02, the copy is on 03."""
    doc["nodes"]["3"]["children"].remove(HOST)
    doc["nodes"]["9"]["children"].append(HOST)
    doc["nodes"][str(HOST)]["parent"] = 9
    return doc
