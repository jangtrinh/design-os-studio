"""IR -> Figma REST JSON + bind intents.

`Lowerer(tokens, styles, icons)` lowers one variant. Every node gets a deterministic id `c<n>` (the REST id survives import as
`source_id`, which is how the post-pass finds the node again). An intent is `{"sid", "key", "var"}`: `key` is a native binding
key (`fills/0`, `itemSpacing`, `paddingTop`, `topLeftRadius`, `strokeWeight`, ...) or `text_style`; `var` a Semantic name or a
text style name. The stored literal always comes from the same variable, so G2's "stored equals variable" holds by construction.

    low = Lowerer(plan, styles, icons); rest = low.component(set_slug, variant_name, ir, x, y); low.intents
"""
import rest_ir as ir
from rest_nested import NestedInstances
from rest_paints import CAP_REST, GradientPaints, JOIN_REST, StrokePaints

CORNERS = ("topLeftRadius", "topRightRadius", "bottomRightRadius", "bottomLeftRadius")
PADS = ("paddingTop", "paddingRight", "paddingBottom", "paddingLeft")
ALIGN = {"start": "MIN", "center": "CENTER", "end": "MAX", "between": "SPACE_BETWEEN"}
SIZE_MODE = {"hug": "HUG", "fixed": "FIXED", "fill": "FILL"}


def bb(x, y, w, h):
    return {"x": x, "y": y, "width": w, "height": h}


class Lowerer(NestedInstances, GradientPaints, StrokePaints):
    def __init__(self, plan, styles, icons):
        self.sem = {s["name"]: s for s in plan["semantic"]}
        self.styles = {s["name"]: s["text_style"] for s in styles}
        self.icons, self.intents, self.allow, self.n = icons, [], [], 0
        self.main_ir = {}
        self.main_ids = {}  # (set name, variant name) -> REST id of the main component, for nested instances
        self.main_sizes = {}  # (set name, variant name) -> estimated (w, h) of the main: a nested instance starts as big
        self.prop_keys = {}  # property name -> REST key `Name#id` of the set being lowered (set by the assembler)
        self.set_props = {}  # set name -> {property name: (REST key, type)} of every set lowered so far, for nested instances
        self.main_texts = {}  # (set name, variant name) -> [(REST id, content, property key)] of its texts, nested ones included (rest_nested)
        self.mirror_pairs = {}  # (set name, variant name) -> [(copied text REST id, main text REST id)] for copies that keep the main's content
        self.fill_texts, self.fill_pairs = {}, {}  # (set, variant) -> REST ids of the texts that follow the root's width; -> {copied text: (instance, main's root)} (rest_nested)
        self._nested_log, self._mirror_log, self._resize_log = [], [], []  # of the variant being lowered: nested instances (REST id, set, variant, props), copied-text pairs, own-sized copies
        self.resized = {}  # (set name, variant name) -> [(copy REST id, w or None, h or None)]: the sizes the module asked for (library_layout.check_resized)
        self._origin = (0.0, 0.0)  # the page position of the frame whose children are being lowered (a rotated text's corner is placed against it)

    def nid(self):
        self.n += 1
        return f"c{self.n}"

    def value(self, name):
        if isinstance(name, tuple):
            return name[1]
        if name not in self.sem:
            raise KeyError(f"unknown Semantic variable {name!r}")
        return self.sem[name]["value"]

    def paint(self, name, alpha=1.0):
        c = self.value(name)
        p = {"type": "SOLID", "color": {k: c[k] / 255 for k in "rgba"}}
        alpha = alpha[1] if isinstance(alpha, tuple) else alpha  # lit(opacity, reason): the entry is recorded by `_paint`
        if alpha != 1.0:
            p["opacity"] = alpha
        return p

    def bind(self, sid, key, var):
        """Record an intent, or, for a `lit(...)` literal, a gate allow-list entry with its reason and the literal it pins."""
        if isinstance(var, tuple):
            self.allow.append({"sid": sid, "property": key, "reason": var[2], "value": var[1]})
        else:
            self.intents.append({"sid": sid, "key": key, "var": var})

    def prop(self, name):
        if name not in self.prop_keys:
            raise KeyError(f"component property {name!r} is not defined on the set being lowered")
        return self.prop_keys[name]

    def lower(self, node, x, y, parent=None, ptype="FRAME", forced_id=None):
        """REST node for `node` at absolute (x, y); `parent` is the IR parent (its auto layout decides sizing fields)."""
        sid = forced_id or self.nid()
        w, h = ir.estimate(node, mains=self.main_sizes)
        out = {"id": sid, "name": node["name"], "absoluteBoundingBox": bb(x, y, w, h)}
        if node.get("visible_prop"):
            out["componentPropertyReferences"] = {"visible": self.prop(node["visible_prop"])}
        if node.get("slot"):  # the container of a SLOT property: its children are the default content (designos-fig rest_import `isSlot`, `slotContent`)
            out["isSlot"] = True
            out.setdefault("componentPropertyReferences", {})["slotContent"] = self.prop(node["slot"])
        if node.get("hidden"):
            out["visible"] = False
        if parent is not None and parent.get("dir"):
            sx, sy = self.child_sizing(node)
            out["layoutSizingHorizontal"], out["layoutSizingVertical"] = SIZE_MODE[sx], SIZE_MODE[sy]
        getattr(self, "_" + node["t"])(node, out, sid, x, y, w, h, ptype)
        return out

    def _T(self, n, out, sid, x, y, w, h, ptype):
        st = self.styles[n["style"]]
        line = st["line_height"]  # {"Pixels": n}, or "Auto": the font's own line (no `lineHeightPx`; the importer reads any other unit as Auto)
        out.update(type="TEXT", characters=n["text"], fills=[self.paint(n["color"], n.get("alpha", 1.0))],
                   style={"fontFamily": st["font_family"], "fontWeight": st["font_weight"], "fontSize": st["font_size"],
                          **({"lineHeightPx": line["Pixels"], "lineHeightUnit": "PIXELS"} if isinstance(line, dict) else {"lineHeightUnit": "INTRINSIC_%"}),
                          "textAutoResize": "HEIGHT" if n.get("wrap") else "WIDTH_AND_HEIGHT"})
        if n.get("align_h"):
            out["style"]["textAlignHorizontal"] = ir.ALIGN_H[n["align_h"]]
        if n.get("rot"):  # -90: local x runs up the page, local y runs right; the corner is the node's place in its parent
            if not n.get("abs"):
                raise ValueError(f"{n['name']}: a rotated text is an absolutely placed child (`abs`): its corner is its place in its parent")
            ox, oy = self._origin
            out["relativeTransform"] = [[0, 1, x - ox], [-1, 0, y - oy]]
        if n.get("prop"):
            out.setdefault("componentPropertyReferences", {})["characters"] = self.prop(n["prop"])
        self.bind(sid, "fills/0", n["color"])
        self.bind(sid, "text_style", n["style"])

    def _I(self, n, out, sid, x, y, w, h, ptype):
        k, size = self.icons.entry(n["icon"]), n["size"]
        gid = f"I{sid};{k['glyph']}"
        out.update(type="INSTANCE", componentId=k["id"], overrides=[{"id": gid, "overriddenFields": ["fills"]}],
                   children=[{"id": gid, "type": "VECTOR", "name": "Glyph", "absoluteBoundingBox": bb(x, y, size, size),
                              "fills": [self.paint(n["color"], n.get("alpha", 1.0))],
                              "fillGeometry": [{"path": self.icons.path(n["icon"], size), "windingRule": "NONZERO"}]}])
        if n.get("swap"):
            out.setdefault("componentPropertyReferences", {})["mainComponent"] = self.prop(n["swap"])
        self.bind(gid, "fills/0", n["color"])

    def _paint(self, n, out, sid):
        """fill, stroke (colour and weight) and corner radius of a frame-like node, each bound or recorded as a literal."""
        out["fills"] = [self.gradient(sid, i, g) for i, g in enumerate(n["fills"])] if n.get("fills") else [self.paint(n["fill"], n.get("alpha", 1.0))] if n.get("fill") else []
        if n.get("fill"):
            self.bind(sid, "fills/0", n["fill"])
            if isinstance(n.get("alpha"), tuple):
                self.bind(sid, "fills/0/opacity", n["alpha"])  # a paint opacity no variable holds: a literal with its reason (G2 opacity reads the entry)
        self.stroke_fields(n, out, sid)
        radius = n.get("radius")
        if isinstance(radius, list):  # per corner: the same binding path as a uniform radius, one corner at a time; a square corner (None) binds nothing
            out["rectangleCornerRadii"] = [self.value(c) if c else 0 for c in radius]
            for key, c in zip(CORNERS, radius):
                if c:
                    self.bind(sid, key, c)
        elif radius:
            out["cornerRadius"] = self.value(radius)
            for key in CORNERS:
                self.bind(sid, key, radius)

    def _E(self, n, out, sid, x, y, w, h, ptype):
        out["type"] = "ELLIPSE"
        self._paint(n, out, sid)

    def _V(self, n, out, sid, x, y, w, h, ptype):
        out.update(type="VECTOR", fillGeometry=[{"path": n["path"], "windingRule": "NONZERO"}])
        self._paint({"sa": "center", **n}, out, sid)  # an open path's stroke is centred on it
        if n.get("stroke"):
            out.update(strokeCap=CAP_REST[n.get("cap", "none")], strokeJoin=JOIN_REST[n.get("join", "miter")])

    def _frame_fields(self, n, out, sid, ptype):
        """Type, clipping, paints and auto layout of a frame-like node, each bound or recorded as a literal (no children)."""
        out.update(type=ptype, clipsContent=bool(n.get("clip")))
        self._paint(n, out, sid)
        d = n.get("dir")
        out["layoutMode"] = {"H": "HORIZONTAL", "V": "VERTICAL"}.get(d, "NONE")
        if d:
            pa, ca = n.get("align", ("center", "center"))
            out.update(primaryAxisAlignItems=ALIGN[pa], counterAxisAlignItems=ALIGN[ca],
                       primaryAxisSizingMode="AUTO" if n.get("sx" if d == "H" else "sy", "hug") == "hug" else "FIXED",
                       counterAxisSizingMode="AUTO" if n.get("sy" if d == "H" else "sx", "hug") == "hug" else "FIXED")
            if n.get("gap"):
                out["itemSpacing"] = self.value(n["gap"])
                self.bind(sid, "itemSpacing", n["gap"])
            for key, p in zip(PADS, ir.pads(n)):
                out[key] = self.value(p) if p else 0
                if p:
                    self.bind(sid, key, p)

    def _F(self, n, out, sid, x, y, w, h, ptype):
        self._frame_fields(n, out, sid, ptype)
        d = n.get("dir")
        children, cx, cy = [], x, y
        for k in n["kids"]:
            self._origin = (x, y)
            if k.get("abs"):
                ax, ay = k["abs"][:2]
                child = self.lower(k, x + ax, y + ay, n)
                child.update(layoutPositioning="ABSOLUTE", constraints={"horizontal": "LEFT_RIGHT", "vertical": "TOP_BOTTOM"})
            else:
                child = self.lower(k, cx, cy, n)
                if d and not k.get("hidden"):
                    cw, ch = ir.estimate(k, mains=self.main_sizes)
                    step = self.value(n["gap"]) if n.get("gap") else 0
                    cx, cy = (cx + cw + step, cy) if d == "H" else (cx, cy + ch + step)
            children.append(child)
        out["children"] = children

    def component(self, name, variant, node, x, y):
        """A COMPONENT for one variant of set `name`; remembers its id so a later set can nest an instance of it."""
        self._nested_log, self._mirror_log, self._resize_log = [], [], []
        rest = self.lower(node, x, y, ptype="COMPONENT")
        rest["name"] = variant
        self.main_ids[(name, variant)] = rest["id"]
        self.main_ir[(name, variant)] = node
        self.main_sizes[(name, variant)] = ir.estimate(node, mains=self.main_sizes)
        self.main_texts[(name, variant)] = self.texts_of(rest, (name, variant))
        self.mirror_pairs[(name, variant)], self.resized[(name, variant)] = list(self._mirror_log), list(self._resize_log)
        return rest
