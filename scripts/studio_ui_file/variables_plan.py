"""Build the S1/S2 variable plan from theme.rs.

Primitives: value-derived steps, hidden, scopes [], mode Dark. Colour names follow colour_names.py (objective steps, never
hex); numbers are `family/<value>`. Semantic: one alias per drawable theme.rs constant, named `group/kebab` (array elements
0-based, like their code syntax), code syntax = the Rust constant, description = `token <key>` where G01 or
non_colour_token_parity maps one. The reader, the skip table, the token map and the naming pattern are the committed
`scripts/studio_ui_theme_*.py` modules, the same ones the gate uses.

CLI: python3 scripts/studio_ui_file plan --repo <worktree> [--out plan.json]   (prints counts)
"""
import argparse
import collections
import json
import pathlib
import re
import sys

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent.parent))  # scripts/: the committed reader and policy
import colour_names  # noqa: E402
import studio_ui_theme_policy as policy  # noqa: E402
import studio_ui_theme_reader as reader  # noqa: E402

MODE = "dark"  # mode id; its name is policy.MODE_NAME
COL_PRIM, COL_SEM = "col.primitives", "col.semantic"

# Number families keyed by semantic group.
PRIM_FAMILY = {"space": "space", "inset": "space", "layout": "space", "control": "size", "size": "size", "radius": "radius",
               "opacity": "opacity", "text-size": "font-size", "text-line": "line-height", "text-weight": "font-weight"}
SCOPES = {"color": ["ALL_FILLS", "STROKE_COLOR", "EFFECT_COLOR"], "space": ["GAP", "WIDTH_HEIGHT"], "inset": ["GAP", "WIDTH_HEIGHT"],
          "control": ["WIDTH_HEIGHT"], "size": ["WIDTH_HEIGHT", "STROKE_FLOAT"], "layout": ["WIDTH_HEIGHT", "GAP"],
          "radius": ["CORNER_RADIUS"], "opacity": ["OPACITY"], "text-size": ["FONT_SIZE"], "text-line": ["LINE_HEIGHT"],
          "text-weight": ["FONT_WEIGHT"], "text-family": ["FONT_FAMILY"]}
TEXT_NAMES = {"UI_FONT_FAMILY": ("text-family", "font-family-ui"), "UI_MONO_FAMILY": ("text-family", "font-family-mono"),
              "UI_FONT_WEIGHT": ("text-weight", "weight-regular"), "UI_FONT_WEIGHT_MEDIUM": ("text-weight", "weight-medium"),
              "UI_FONT_WEIGHT_STRONG": ("text-weight", "weight-strong"), "UI_MONO_WEIGHT": ("text-weight", "weight-mono"),
              "CANVAS_LABEL_FONT_WEIGHT": ("text-weight", "weight-canvas-label")}
LAYOUT_RE = re.compile(r"(_X|_Y|_DX|_DY|_PAD|_PAD_X|_MARGIN|_GAP|_OFFSET|_FROM_RIGHT|_INDENT|_STEP|_BOTTOM|_RIGHT|_PADDING|_LEAD)(\[\d+\])?$|_PAD_|_GAP_|_SHIFT_MULT")


def kebab(s):
    return s.lower().replace("_", "-")


def fmt(v):
    return str(int(v)) if float(v).is_integer() else repr(round(v, 4))


def classify(row):
    """(group key, short name) for a flattened theme row. Group `text-*` shows as `text` in the name."""
    n, kind = row["name"], row["kind"]
    base = re.sub(r"\[(\d+)\]$", lambda m: f"-{int(m.group(1))}", n)
    if kind == "colour":
        return "color", kebab(base)
    if n in TEXT_NAMES:
        return TEXT_NAMES[n]
    if kind == "text":
        raise SystemExit(f"no naming rule for text constant {n}")
    for pre, grp, strip in (("SPACE_", "space", 1), ("INSET_", "inset", 1), ("CONTROL_", "control", 1), ("RADIUS_", "radius", 1),
                            ("BADGE_", "control", 0), ("TARGET_", "control", 0)):
        if base.startswith(pre):
            return grp, kebab(base[len(pre):] if strip else base)
    if base.endswith("_RADIUS"):
        return "radius", kebab(base[:-7])
    if base.startswith("FONT_SIZE_"):
        return "text-size", kebab(base[10:])
    if base.endswith("_FONT_SIZE"):
        return "text-size", kebab(base[:-10])
    if base.startswith("LINE_"):
        return "text-line", "line-" + kebab(base[5:])
    if "ALPHA" in base:
        return "opacity", kebab(base)
    return ("layout" if LAYOUT_RE.search(base) else "size"), kebab(base)


def read_constants(repo):
    """theme.rs of `repo` resolved; a SKIPPED entry for a constant that no longer exists is fatal (stale policy)."""
    consts = reader.read_theme_constants((pathlib.Path(repo) / reader.THEME_REL).read_text(encoding="utf-8"))
    stale = sorted(set(policy.SKIPPED) - set(consts))
    if stale:
        raise SystemExit(f"stale SKIPPED entries (no such constant in theme.rs): {stale}")
    return consts


def build_plan(repo):
    rows = reader.drawable_rows(reader.flatten(read_constants(repo)))
    return plan_from_rows(rows, policy.token_keys(pathlib.Path(repo) / "scripts"))


def plan_from_rows(rows, keys):
    """The Primitive and Semantic variables for drawable theme rows; `keys` maps a constant to its token key."""
    skipped = [{"name": k, "reason": v} for k, v in sorted(policy.SKIPPED.items())]
    names = colour_names.build_names([(c["r"], c["g"], c["b"], c["a"]) for c in (r["value"] for r in rows if r["kind"] == "colour")])
    prims, sems, seen = {}, [], {}
    for row in rows:
        grp, short = classify(row)
        group = "text" if grp.startswith("text") else grp
        name = f"{group}/{short}"
        if name in seen:
            raise SystemExit(f"semantic name collision {name}: {seen[name]} vs {row['name']}")
        if not re.fullmatch(policy.SEMANTIC_NAME, name):
            raise SystemExit(f"semantic name {name} is not group/kebab (the gate rule)")
        seen[name] = row["name"]
        if row["kind"] == "colour":
            v = row["value"]
            pname, pkind, pval = names[(v["r"], v["g"], v["b"], v["a"])], "Color", v
        elif row["kind"] == "text":
            pname, pkind, pval = "font-family/" + ("mono" if "Mono" in row["value"] else "sans"), "String", row["value"]
        else:
            pname, pkind, pval = f"{PRIM_FAMILY[grp]}/{fmt(row['value'])}", "Number", float(row["value"])
        pid = "p." + pname.replace("/", ".")
        prims.setdefault(pid, {"id": pid, "name": pname, "kind": pkind, "value": pval})
        tok = keys.get(row["const"])
        sems.append({"id": "s." + name.replace("/", "."), "name": name, "group": group, "kind": pkind, "alias": pid,
                     "scopes": SCOPES[grp], "const": row["name"], "code_syntax": f"theme::{row['name']}",
                     "description": f"token {tok}" if tok else "", "value": pval})
    return {"primitives": sorted(prims.values(), key=lambda p: p["name"]), "semantic": sems, "skipped": skipped}


def summarise(plan):
    out = {"primitives": len(plan["primitives"]), "semantic": len(plan["semantic"]), "skipped": len(plan["skipped"])}
    for s in plan["semantic"]:
        out[f"semantic/{s['group']}"] = out.get(f"semantic/{s['group']}", 0) + 1
    return out


def main(argv=None):
    ap = argparse.ArgumentParser(prog="studio_ui_file plan", description="theme.rs -> variable plan; prints counts")
    ap.add_argument("--repo", default=str(pathlib.Path(__file__).resolve().parents[2]))
    ap.add_argument("--out", help="write the full plan as JSON")
    a = ap.parse_args(argv)
    try:
        consts = read_constants(a.repo)
    except reader.ThemeError as e:
        print(f"ERROR: {e}", file=sys.stderr)
        return 1
    print(f"theme.rs constants resolved: {len(consts)} (0 unresolved)")
    print("by rust type:", dict(collections.Counter(c.rust_type for c in consts.values())))
    plan = build_plan(a.repo)
    print(json.dumps(summarise(plan), indent=1))
    if a.out:
        pathlib.Path(a.out).write_text(json.dumps(plan, indent=1))
    return 0
