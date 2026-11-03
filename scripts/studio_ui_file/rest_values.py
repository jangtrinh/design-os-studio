"""Literal values of the IR (re-exported by `rest_ir`): numbers, colours and gradients that no Semantic variable owns, each with the reason.

    lit(value, reason)              a number for a bound property (radius, gap, padding, stroke weight); recorded in the gate allow-list with `reason`
    F(..., alpha=lit(0.22, reason))  a paint opacity of its own on the node's one bound fill (the Rust draws `CONST.gamma_multiply(0.22)`): the fill stays bound, the opacity is a literal
                                    recorded in the gate allow-list as `fills/0/opacity` (G2 opacity reserves 0.5 for Disabled and 1.0 for the rest unless an entry says otherwise)
    grad(stops, start, end)         a linear gradient whose stops are literal colours with a reason each
    segments_path(segments)         an SVG path of open straight subpaths, the `path` of a stroked `V`
"""


def lit(value, reason):
    return ("lit", float(value), reason)


def grad(stops, start=(0.0, 0.5), end=(1.0, 0.5)):
    """A linear gradient paint from `start` to `end` (points of the node's unit box: (0, .5) to (1, .5) is left to right, (.5, 0) to (.5, 1) top to bottom).
    `stops` is [(position, (r, g, b, a), reason)], positions rising from 0 to 1, channels 0..255: every colour is a literal with the reason no variable holds it."""
    if len(stops) < 2 or any(not (0 <= p <= 1) for p, _, _ in stops) or [p for p, _, _ in stops] != sorted(p for p, _, _ in stops):
        raise ValueError("a gradient needs two or more stops, positions from 0 to 1 in rising order")
    for _, c, reason in stops:
        if not isinstance(c, (tuple, list)) or len(c) != 4 or any(not isinstance(v, int) or isinstance(v, bool) or not 0 <= v <= 255 for v in c) or not reason:
            raise ValueError(f"a gradient stop is (position, (r, g, b, a) as 0..255 integers, reason): got {c!r}, {reason!r}")
    if tuple(start) == tuple(end):
        raise ValueError("a gradient needs two different points")
    return {"t": "grad", "start": tuple(start), "end": tuple(end), "stops": [(p, tuple(c), reason) for p, c, reason in stops]}


def check_alpha(name, kw):
    """The `alpha` of a frame: a plain number (1.0, or 0.5 for the Disabled half alpha), or `lit(opacity, reason)`, an opacity strictly between 0 and 1 with a reason that sits on the
    node's one `fill`: the allow-list entry names `fills/0/opacity`, so a gradient or a stroke (which the same opacity would also dim, unrecorded) is refused."""
    a = kw.get("alpha", 1.0)
    if not isinstance(a, tuple):
        return
    if a[:1] != ("lit",) or isinstance(a[1], bool) or not 0 < a[1] < 1 or not a[2]:
        raise ValueError(f"{name}: alpha as a literal is lit(opacity, reason) with an opacity strictly between 0 and 1 and a reason, got {a!r}")
    if not kw.get("fill") or kw.get("fills") or kw.get("stroke"):
        raise ValueError(f"{name}: a literal paint opacity belongs to the node's one `fill`: give `fill`, and no `fills` or `stroke`")


def segments_path(segments):
    """An SVG path of open straight subpaths for a stroked `V`: each ((x1, y1), (x2, y2)) is its own `M x1 y1 L x2 y2`, as egui's `line_segment`s are (two segments
    that meet overlap at the corner)."""
    return " ".join(f"M{x1:g} {y1:g} L{x2:g} {y2:g}" for (x1, y1), (x2, y2) in segments)
