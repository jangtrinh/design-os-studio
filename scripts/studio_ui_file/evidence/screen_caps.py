"""The screen gate (G3 pixels): the mean difference of each region of a screen against its capture stays under a cap, and so does the worst 40 px tile.

The caps are what the first screen measured (`Screen/EditorDesign` against `d-editor-default`: screen 4.240, tab bar 0.293, rail 2.385, left panel 2.498, right panel 6.333,
ruler strips 6.399 and 6.574, toolbar 3.498, worst tile 31.2) plus about 8 %, rounded up. They are per region because a region moved 2 px barely moves the whole screen's mean:
the screen mean alone catches a panel moved 2 px (4.96) and misses the rail and the toolbar (4.34, 4.42), while each region's own mean goes over its cap (right panel 8.10,
left panel 5.17, rail 2.93, tab bar 2.24, toolbar 6.82). The 99th percentile is not capped: it is dominated by text-edge anti-aliasing (86 to 181 on this screen, the same
before and after a 2 px move) and is reported only.

A cap is a ceiling, never a target; it is not loosened to make a render pass. Every other state gets its caps from its batch's gate data (`screens/<batch>_gate.py`,
studio_ui_gate_screen_data), and no state calibrates itself: a region's cap is the editor-default cap of the region of that name (`DEFAULT_CAPS`), or a pinned measured value
with its cause in `CAUSES[state][region]`; the worst-tile limit of a state is 34 unless `WORST_TILE[state]` overrides it with a cause under `"worst tile"`. A named widget rect
known to be off by more than 1 px is pinned in `GEOMETRY[state]` with the measured layout and node rects and a cause, the way the crop compare accepts a finding: the pinned
numbers pass, anything else fails.

    judge(scores) -> [failure line]     `scores` is one entry of screens-scores.json; empty when every cap holds. Failures: a state with no caps, a region (or the screen) with no counted pixel,
                                        a region with no cap or a cap with no region in the scores, a mean over its cap, a cap or a worst-tile limit that is not the default and has no cause, a
                                        worst tile over its limit or absent, a widget rect off by more than 1 px that is not pinned, whose numbers moved, that has no cause, or a pin
                                        the screen no longer needs
    exit_code(scores) -> 0 | 1 | 2     2 for no screen, 1 for any failure above
"""
import pathlib
import sys

_SCRIPTS = str(pathlib.Path(__file__).resolve().parents[2])
if _SCRIPTS not in sys.path:
    sys.path.insert(0, _SCRIPTS)
import studio_ui_gate_screen_data as gate_data  # noqa: E402

BASE_CAPS = {"d-editor-default": {"screen": 4.6, "tab bar": 0.4, "rail": 2.6, "left panel": 2.7, "right panel": 6.9, "ruler top strip": 7.1, "ruler left strip": 7.1, "toolbar": 3.8}}
DEFAULT_CAPS = BASE_CAPS["d-editor-default"]
WORST_TILE = 34.0
TILE_KEY = "worst tile"
SAME = 0.001  # pinned rects are rounded to 3 decimals

_DATA = gate_data.load()
CAPS = gate_data.merge(BASE_CAPS, _DATA, "CAPS")
CAUSES, WORST_TILES, GEOMETRY_PINS = _DATA["CAUSES"], _DATA["WORST_TILE"], _DATA["GEOMETRY"]


def _same(a, b):
    return a is not None and b is not None and len(a) == len(b) and all(abs(p - q) <= SAME for p, q in zip(a, b))


def _caps_without_cause(who, table, causes):
    return [f"{who}: the cap of region {n!r} ({cap}) is not the editor-default cap ({DEFAULT_CAPS.get(n)}) and has no cause: pin it with its measured value and cause in CAUSES"
            for n, cap in table.items() if cap != DEFAULT_CAPS.get(n) and not causes.get(n)]


def _geometry(who, scores, pins):
    """Failures of the named widget rects: each rect off by more than 1 px is pinned at its measured numbers with a cause, and every pin is still needed."""
    pinned, seen, out = {p["key"]: p for p in pins}, set(), []
    for m in (scores.get("geometry") or {}).get("mismatched", []):
        key, pin = m["key"], pinned.get(m["key"])
        seen.add(key)
        numbers = f"layout {m.get('layout')}, node {m.get('node')}"
        if pin is None:
            out.append(f"{who}: widget rect {key!r} ({numbers}) is off by more than 1 px and not pinned: fix the screen, or pin the numbers with their cause in GEOMETRY")
        elif not pin.get("cause"):
            out.append(f"{who}: the pin of widget rect {key!r} has no cause")
        elif not (_same(pin.get("layout"), m.get("layout")) and _same(pin.get("node"), m.get("node"))):
            out.append(f"{who}: widget rect {key!r} moved: pinned layout {pin.get('layout')}, node {pin.get('node')}; measured {numbers}")
    return out + [f"{who}: the pin of widget rect {key!r} is stale: the screen has no such mismatch any more, remove it" for key in pinned if key not in seen]


def judge(scores):
    state, table = scores["state"], CAPS.get(scores["state"])
    who = f"{scores['screen']} ({state})"
    if table is None:
        return [f"{who}: no caps for this screen, so nothing says how far its render may be from the capture: declare CAPS in its batch's gate data (screens/<batch>_gate.py)"]
    measured = {"screen": scores["pixels"], **scores["regions"]}
    out = [f"{who}: the cap of region {n!r} is not in the scores" for n in table if n not in measured]
    for name, stat in measured.items():
        if name not in table:
            out.append(f"{who}: region {name!r} has no cap")
        elif not stat["pixels"] or stat["mean"] is None:
            out.append(f"{who}: region {name!r} has no counted pixel: an empty selection never passes")
        elif stat["mean"] > table[name]:
            out.append(f"{who}: region {name!r} mean {stat['mean']} is over its cap {table[name]}")
    causes, limit = CAUSES.get(state, {}), WORST_TILES.get(state, WORST_TILE)
    out += _caps_without_cause(who, table, causes)
    if state in WORST_TILES and not causes.get(TILE_KEY):
        out.append(f"{who}: the worst-tile limit {limit} is not the default {WORST_TILE} and has no cause: pin it with its measured value and cause in CAUSES['{TILE_KEY}']")
    tile = scores["worst_tile"]
    if tile["rect"] is None:
        out.append(f"{who}: no worst tile was measured")
    elif tile["mean"] > limit:
        out.append(f"{who}: the worst tile {tile['rect']} mean {tile['mean']} is over {limit}")
    return out + _geometry(who, scores, GEOMETRY_PINS.get(state, ()))


def exit_code(scores):
    """The exit code of `evidence screens`: 2 when no screen was compared (an empty run never passes), 1 when a cap or a pin does not hold, else 0."""
    if not scores:
        return 2
    return 1 if any(judge(s) for s in scores) else 0
