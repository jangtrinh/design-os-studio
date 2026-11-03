"""Shared fixtures of the Dialog/CommandPalette tests: a synthetic env (names are the theme constants; looks from the real kit-states.json) and a layout oracle."""
import json
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent / "studio_ui_file"))
sys.path.insert(0, str(HERE.parent))
import components  # noqa: E402
import rest_ir as ir  # noqa: E402
import text_styles_plan  # noqa: E402
from components.env import KIT_STATE, KIT_STATES_REL  # noqa: E402

ROWS = json.loads((HERE.parents[1] / KIT_STATES_REL).read_text(encoding="utf-8"))["rows"]
NUM = {"CONTROL_MD": 40.0, "CONTROL_XS": 32.0, "BORDER_WIDTH": 1.0, "BADGE_SM": 22.0, "MENU_ITEM_HEIGHT": 32.0, "INSET_SEGMENT": 2.0, "MENU_PILL_INSET": 8.0,
       "MENU_PADDING": 8.0, "MENU_SHORTCUT_PAD": 16.0, "MENU_ICON_SIZE": 16.0}
LINE = 18.0  # Body, Caption and Text xs all have an 18 px line


class FakeEnv:
    def c(self, const):
        return const

    def num(self, const):
        return NUM[const]

    def look(self, row, state):
        cell = ROWS[row][KIT_STATE[state]]
        out = {k: cell.get(k) for k in ("fill", "border", "text", "icon", "ring")}
        out["alpha"] = cell["alpha"]
        return out


SETS = {s["name"]: s for s in components.discover()}
STYLES = {f"{role} / {label}" for role, label, *_ in text_styles_plan.ROLES}
ENV = FakeEnv()


def build(kind):
    return SETS["Dialog/CommandPalette"]["build"]({"Kind": kind, "State": "Default"}, ENV)


def named(node, name):
    return [k for k in ir.walk(node) if k["name"] == name]


def px(entry):
    return 0.0 if entry is None else entry[1] if isinstance(entry, tuple) else NUM[entry]


def flow_h(node):
    """Height of a built node the way auto layout stacks it: fixed `h` as given, a V frame the sum of its in-flow children plus gaps and pads, text one line."""
    if node.get("abs"):
        return 0.0
    if node["t"] == "T":
        return LINE
    if node["t"] == "I":
        return float(node["size"])
    if "h" in node:
        return node["h"]
    kids = [k for k in node["kids"] if not k.get("abs")]
    top, _, bottom, _ = ir.pads(node)
    if node["dir"] == "V":
        return sum(flow_h(k) for k in kids) + px(node.get("gap")) * max(len(kids) - 1, 0) + px(top) + px(bottom)
    return max(flow_h(k) for k in kids) + px(top) + px(bottom)


def lit_of(entry):
    return entry[:2] if isinstance(entry, tuple) else entry
