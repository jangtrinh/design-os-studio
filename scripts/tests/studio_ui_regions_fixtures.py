"""Shared fixtures of the side-panel region tests: the real Env (Semantic plan, kit-states.json), the specs of every module, and tree helpers."""
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
REPO = HERE.parents[1]
sys.path.insert(0, str(HERE.parent / "studio_ui_file"))
sys.path.insert(0, str(HERE.parent))
import components  # noqa: E402
import icon_lib  # noqa: E402
import text_styles_plan  # noqa: E402
import variables_plan as plan_mod  # noqa: E402
from components.env import Env  # noqa: E402
from rest_ir import walk  # noqa: E402

PLAN = plan_mod.build_plan(str(REPO))
ENV = Env(str(REPO), PLAN, icon_lib.IconLib(str(REPO)))
SPECS = {s["name"]: s for s in components.discover()}
STYLES = {f"{role} / {label}" for role, label, *_ in text_styles_plan.ROLES}
KIT_WORDS = ("untitled", "vsf", "pcp", "olivia", "desktop", "mobile", "tabs and button", "dashboards", "swatch", "press logo", "logomark", "company logo", "cta")


def build(name):
    """The variant of an axis-less record, or the first variant (today's look) of one that gained an axis."""
    return SPECS[name]["build"]({a: v[0] for a, v in SPECS[name]["axes"].items()}, ENV)


def part(node, name):
    found = [k for k in walk(node) if k["name"] == name]
    assert len(found) == 1, (name, len(found))
    return found[0]


def texts(node):
    return [k["text"] for k in walk(node) if k["t"] == "T"]


def lit_reasons(node):
    """Every `lit(...)` of a property the Lowerer binds: padding, gap, radius, stroke weight."""
    out = []
    for k in walk(node):
        if k["t"] != "F":
            continue
        radius = k.get("radius")
        for e in ([k.get("gap"), k.get("sw")] + list(k.get("pad") or []) + (radius if isinstance(radius, list) else [radius])):
            if isinstance(e, tuple):
                out.append((k["name"], e))
    return out


def lit_of(entry):
    """`lit(value, reason)` without its reason; any other entry as it is."""
    return entry[:2] if isinstance(entry, tuple) else entry
