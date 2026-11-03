"""Duplicate-drawing detection and canvas row labels for page `04 Screens`.

When two screen specs have the same `(w, h, canvas, place, scrim)` drawing, `same_drawings(specs)` records the later screen against the first screen in user-flow order and
formats both the screen-gate `INFO same-drawing <screen> = <screen>` line and the canvas row label `#<pos> same drawing as #<n>: <reason>`. A reason is one of:

* `CAPTURE_REASON`, `same 0.7.5 capture`: the screen's 0.7.5 capture is pixel for pixel the first screen's (`CAPTURE_IDENTICAL`), so no gap is behind its drawing;
* the title of the consolidated gap-table row that lists the screen (`GAP_NAMES`): a drawing a component state cannot yet tell apart from another screen's.

A duplicate with neither fails `same_drawings`; a test fails an entry of either table that is not a duplicate of the screens.
"""
import json

import spec_kit as kit

LABEL = 28
SPEC_FIELDS = ("w", "h", "canvas", "place", "scrim")

CAPTURE_REASON = "same 0.7.5 capture"
CAPTURE_IDENTICAL = ("d-window-chrome", "e-no-selection", "i-terminal")

GAP_NAMES = {}


def spec_key(s):
    return json.dumps({k: s.get(k) for k in SPEC_FIELDS}, sort_keys=True)


def reason(state, gap):
    """Why a state may repeat another screen's drawing: the gap row naming it, else its capture being identical to the first screen's."""
    if gap is not None:
        return gap
    return CAPTURE_REASON if state in CAPTURE_IDENTICAL else None


def same_drawings(specs):
    """Map `state -> {state, name, pos, of_state, of_name, of_pos, gap, reason, label, info}` for every screen whose spec equals an earlier screen's (`gap` is None when a capture is the reason)."""
    seen, out = {}, {}
    for pos, s in enumerate(specs, 1):
        key, state = spec_key(s), s["state"]
        if key not in seen:
            seen[key] = (pos, s)
            continue
        of_pos, first = seen[key]
        gap = GAP_NAMES.get(state)
        why = reason(state, gap)
        if why is None:
            raise ValueError(f"{state!r} has the same drawing as {first['state']!r} (#{of_pos}), but is in neither GAP_NAMES nor CAPTURE_IDENTICAL")
        out[state] = {
            "state": state,
            "name": s["name"],
            "pos": pos,
            "of_state": first["state"],
            "of_name": first["name"],
            "of_pos": of_pos,
            "gap": gap,
            "reason": why,
            "label": f"#{pos} same drawing as #{of_pos}: {why}",
            "info": f"INFO same-drawing {state} = {first['state']}",
        }
    return out


def same_drawing_info_lines(specs):
    return [row["info"] for row in same_drawings(specs).values()]


def row_label_text(s, same, pos=None):
    if s.get("state") in same:
        return same[s["state"]]["label"]
    return f"#{pos} {s['name']}" if pos is not None else s["name"]


def row_label_spec(s, same, x=0, y=0, pos=None):
    return dict(kit.text(f"Label {s['name']}", row_label_text(s, same, pos=pos), style="Body / Medium", color="color/text-tertiary"), x=x, y=y)
