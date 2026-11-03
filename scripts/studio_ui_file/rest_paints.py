"""Gradient paints and strokes for the lowerer: the REST `GRADIENT_LINEAR` paint of a frame-like node (`rest_ir.grad`) with the allow-list entry of each stop, and
the stroke of a frame-like node, uniform or per side (`rest_ir.F(sw=[top, right, bottom, left])`).

A stop colour is a literal no Semantic variable holds, so it binds nothing: the gate reads a gradient per stop (`fills/<i>/stops/<j>`, studio_ui_gate_bound._paint_props)
and the build records each stop as an allow-list entry with its reason and the rgba it pins (0..255, the gate's `_rgba`). The three handles follow Figma's convention for a
linear gradient: the start, the end, and the start moved half the unit box along the perpendicular (a third handle sets the width of a gradient that only a transform can skew).
`Lowerer` supplies `allow`, `value`, `paint` and `bind`.
"""
import rest_ir as ir

SIDES = ("top", "right", "bottom", "left")
SIDE_KEYS = {"top": "strokeTopWeight", "right": "strokeRightWeight", "bottom": "strokeBottomWeight", "left": "strokeLeftWeight"}
CAP_REST, JOIN_REST = {"none": "NONE", "round": "ROUND", "square": "SQUARE"}, {"miter": "MITER", "bevel": "BEVEL", "round": "ROUND"}


def _point(p):
    return {"x": p[0], "y": p[1]}


class GradientPaints:
    def gradient(self, sid, i, g):
        """The REST paint of gradient `g` (paint index `i` of node `sid`); records one allow-list entry per stop."""
        (x0, y0), (x1, y1) = g["start"], g["end"]
        side = (x0 - 0.5 * (y1 - y0), y0 + 0.5 * (x1 - x0))
        for j, (_, colour, reason) in enumerate(g["stops"]):
            self.allow.append({"sid": sid, "property": f"fills/{i}/stops/{j}", "reason": reason, "value": dict(zip("rgba", colour))})
        return {"type": "GRADIENT_LINEAR", "gradientHandlePositions": [_point((x0, y0)), _point((x1, y1)), _point(side)],
                "gradientStops": [{"position": pos, "color": {k: v / 255 for k, v in zip("rgba", colour)}} for pos, colour, _ in g["stops"]]}


class StrokePaints:
    def stroke_fields(self, n, out, sid):
        """The stroke of a frame-like node: its colour, weight and alignment, each bound or recorded as a literal. `sw` as a list is per side:
        `individualStrokeWeights` with 0 on the sides without one, and one binding (or entry) per side that has one, on the key the gate reads
        (`strokeBottomWeight`); `strokeWeight` carries the first weight, as the importer needs one."""
        if not n.get("stroke"):
            return
        sw = n["sw"]
        sides = isinstance(sw, list)
        out.update(strokes=[self.paint(n["stroke"], n.get("alpha", 1.0))], strokeWeight=self.value(next(w for w in sw if w) if sides else sw),
                   strokeAlign=ir.STROKE_ALIGN_REST[n.get("sa", "inside")])
        self.bind(sid, "strokes/0", n["stroke"])
        if not sides:
            self.bind(sid, "strokeWeight", sw)
            return
        out["individualStrokeWeights"] = {side: self.value(w) if w else 0 for side, w in zip(SIDES, sw)}
        for side, w in zip(SIDES, sw):
            if w:
                self.bind(sid, SIDE_KEYS[side], w)
