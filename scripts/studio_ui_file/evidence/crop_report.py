"""The verdict of a compare run over its scored rows, without Pillow: which rows fail, which findings are accepted, the exit code and the lines it prints.

A scored row is bad when its foreground box, surface colour, strongest-pixel colour or gradient fit (`crop_metrics.measure_pixels`) differs from production. A row may carry a
recorded `finding` (the why, in words) and a `finding_pin` (the measured numbers it describes, by score field: `{"fg_render_px": [230, 16], "fg_production_px": [234, 16]}`). A finding is
ACCEPTED when its pin equals what the row measured now and covers every check the row fails: a bad row with an accepted finding does not fail the run. Anything else does:

* a bad row with no finding, or a finding that pins nothing;
* a pin that no longer matches (the numbers moved, so the finding describes another drawing: re-measure it, or fix the component), on a failing row or a passing one;
* a pin that leaves a failing check out (the finding would hide a difference it never measured);
* an ambiguous background, which no finding accepts (the entry needs a tighter rect).

    exit_code(scores, geometry=())  0 when every row passes or carries an accepted finding, 1 otherwise (or when `geometry` holds a GEOMETRY line), 2 when nothing was compared
    problem_rows(scores)    [(row, [why])] the rows that fail the run, in order
    accepted_rows(scores)   the rows whose finding is accepted (their numbers are printed by `report_lines`)
    report_lines(scores)    the FAIL, FINDING and ACCEPTED lines of a run
"""
from crop_gradient import MEAN_TOL, P99_TOL

CHECKS = (("fg_ok", "box", ("fg_render_px", "fg_production_px")), ("surface_ok", "surface", ("surface_render", "surface_production")),
          ("peak_ok", "strongest pixel", ("peak_colour_render", "peak_colour_production")), ("fit_ok", "gradient fit", ("fit_mean", "fit_p99", "fit_size_ok")))


def _plain(v):
    return [_plain(x) for x in v] if isinstance(v, (list, tuple)) else v


def bad_rows(scores):
    """The scored entries whose verdict is bad."""
    return [s for s in scores if s["bad"]]


def failed_checks(s):
    return [(label, fields) for key, label, fields in CHECKS if s.get(key) is False]


def pin_problems(s):
    """Why the finding of row `s` is not accepted: [] when it carries no finding and no pin, or when the pin equals the measurement and covers every failing check."""
    finding, pin = s.get("finding"), s.get("finding_pin")
    if not finding and not pin:
        return []
    if not finding:
        return ["a pin without a finding says nothing: write the finding it belongs to"]
    if not pin:
        return ["the finding pins no numbers: add `finding_pin` with the measured score fields, so a later change re-fails"]
    out = [f"pinned {k} {_plain(v)}, measured {_plain(s.get(k))}" for k, v in pin.items() if _plain(s.get(k)) != _plain(v)]
    out += [f"fails the {label} check but the pin does not hold {', '.join(f for f in fields if f not in pin)}" for label, fields in failed_checks(s) if any(f not in pin for f in fields)]
    if s.get("background_ok") is False:
        out.append("an ambiguous background is never accepted: tighten the rect")
    return out


def problem_rows(scores):
    """[(row, problems)] for every row that fails the run: a bad row without an accepted finding, and a finding whose pin does not hold, bad or not."""
    out = []
    for s in scores:
        why = pin_problems(s)
        if why or (s["bad"] and not s.get("finding")):
            out.append((s, why or ["no recorded finding"]))
    return out


def accepted_rows(scores):
    return [s for s in scores if s.get("finding") and not pin_problems(s)]


def exit_code(scores, geometry=()):
    """0 when every row passes or carries an accepted finding, 1 when any row is a problem or any `render_rect` is not where its node puts it (`geometry`: the GEOMETRY lines of
    crop_geometry, which no finding accepts), 2 when nothing was compared (an empty run never passes)."""
    return 1 if geometry else 2 if not scores else 1 if problem_rows(scores) else 0


def pending_note(scores):
    """One line naming the rows exempt while an engine bug is open (None when there are none)."""
    pending = [s for s in scores if s.get("pending")]
    if not pending:
        return None
    issues = sorted({s["pending"] for s in pending})
    return (f"{len(pending)} of {len(scores)} rows are exempt from the box and colour checks while {', '.join(issues)} is open "
            f"(the swapped 20 px icon is reset to 16 px): " + "; ".join(f"{s['set']} {s['variant']} {s['capture']}" for s in pending))


def fail_line(s):
    """The line that names why the scored row `s` failed: the box, surface and strongest-pixel verdicts, and an ambiguous background; for a gradient row, its pixel fit."""
    ok = lambda flag: "ok" if flag else "FAIL"
    if s.get("gradient"):
        return (f"FAIL {s['set']} {s['variant']} {s['capture']}: gradient fit {ok(s['fit_ok'])} (mean difference {s['fit_mean']} of at most {MEAN_TOL}, 99th percentile {s['fit_p99']} of at most {P99_TOL}, "
                f"sizes {'agree' if s['fit_size_ok'] else 'differ by more than 1 px'})" + (f" -- finding: {s['finding']}" if s.get("finding") else ""))
    return (f"FAIL {s['set']} {s['variant']} {s['capture']}: box {ok(s['fg_ok'])} (render {s['fg_render_px']} vs production {s['fg_production_px']}), "
            f"surface {ok(s['surface_ok'])} ({s['surface_render']} vs {s['surface_production']}), strongest pixel "
            f"{ok(s['peak_ok'])} ({s['peak_colour_render']} vs {s['peak_colour_production']})"
            + ("" if s["background_ok"] else "; ambiguous background: the colour most pixels have is not clearly the surface, so the entry needs a tighter rect")
            + (f" -- finding: {s['finding']}" if s["finding"] else ""))


def report_lines(scores):
    """The lines a run prints after its summary: FAIL for a bad row without an accepted finding, FINDING for a pin that does not hold, ACCEPTED (with the pinned numbers) for each accepted finding."""
    out = []
    for s, why in problem_rows(scores):
        out.append(fail_line(s) if s["bad"] else f"FINDING {s['set']} {s['variant']} {s['capture']}: the row passes but its finding is not accepted")
        out += [f"  not accepted: {w}" for w in why]
    for s in accepted_rows(scores):
        numbers = ", ".join(f"{k} {_plain(v)}" for k, v in s["finding_pin"].items())
        out.append(f"ACCEPTED {s['set']} {s['variant']} {s['capture']}: {numbers}; the row {'fails ' + ' and '.join(label for label, _ in failed_checks(s)) if s['bad'] else 'passes'} -- finding: {s['finding']}")
    return out
