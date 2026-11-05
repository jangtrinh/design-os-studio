"""Shared by the editor-shell region tests (not a test module): the real Semantic plan, the discovered specs and small readers of the IR a builder returns."""
import re
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
SIZE = {"Navigation/TitleBar": (1440, 28), "Navigation/TopTabBar": (1440, 32), "Navigation/LeftRail": (49, 868), "Panel/LeftLayers": (241, 868), "Panel/RightInspector": (241, 868),
        "Navigation/BottomCanvasToolbar": (512, 48), "Navigation/Rulers": (909, 868)}
DENY = re.compile(r"untitled|vsf|pcp|olivia|figma", re.I)


def build(name, **values):
    """One variant of a region; an axis the caller leaves out takes its first value, which is today's look (a new axis never moves the existing placements)."""
    return SPECS[name]["build"]({**{a: v[0] for a, v in SPECS[name]["axes"].items()}, **values}, ENV)


def default_build(name):
    return build(name, **{a: v[0] for a, v in SPECS[name]["axes"].items()})


def by_name(root):
    return {n["name"]: n for n in walk(root)}


def rect(node):
    return tuple(node["abs"])
