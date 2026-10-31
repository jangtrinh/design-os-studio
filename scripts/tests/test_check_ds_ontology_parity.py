#!/usr/bin/env python3
"""Tests for scripts/check_ds_ontology_parity.py and the colour classes in G01.

Every fixture builds its own tree under a temp directory, so the tests need no `ui` CLI, no plan
folder and no cargo build. A check is shown red by a crafted fixture and green by the clean tree
next to it; the script's exit code is what each test asserts.
"""

from __future__ import annotations

import contextlib
import io
import json
import re
import shutil
import sys
import tempfile
import unittest
from pathlib import Path
from typing import Dict, Iterable, List, Optional, Sequence

SCRIPTS_DIR = Path(__file__).resolve().parent.parent
REPO_ROOT = SCRIPTS_DIR.parent
sys.path.insert(0, str(SCRIPTS_DIR))

import check_ds_ontology_parity as gate  # noqa: E402
import design_os_self_evolve as dse  # noqa: E402

APP_SRC_REL = Path("crates") / "designos-app" / "src"
TESTDATA_REL = APP_SRC_REL / "widgets" / "kit" / "testdata"

CREDIT_LINE = "Colour roles follow the Untitled UI dark system, credited here once."
CONTRACT_FIXTURE = """# Token contract fixture

| # | Constant (`theme.rs` line) | HEAD value | Action | New value | Untitled token | Reason |
|---|---|---|---|---|---|---|
| 1 | `ICON_QUATERNARY` | `rgb(0x737373)` | change | `rgb(0x737373)` | fg-quaternary | icon |
| 2 | `TEXT_PLACEHOLDER` | `rgb(0x737373)` | change | `rgb(0x737373)` | text-placeholder | placeholder |
| 3 | `BG_SECONDARY_HOVER` | `rgb(0x262626)` | change | `rgb(0x262626)` | bg-secondary_hover | hover |

| # | New constant (new name) | Value | Untitled token | Reason |
|---|---|---|---|---|
| N1 | `BORDER_TRANSLUCENT` | `rgba(0xFFFFFF, 0x1A)` | alpha-black-10 | scrim |
"""


def describe(
    tier: str,
    *,
    rust: Sequence[str],
    status: str = "exists",
    owner: str = "C",
    paints: Sequence[str] = (),
    pending: Sequence[str] = (),
    flow: Sequence[str] = (),
    composes: Sequence[str] = (),
    today: str = "",
) -> str:
    """The registry description of spec 3.2: one purpose sentence, then the tier= line."""
    fields = [f"tier={tier}", f"rust={','.join(rust)}", f"rust_status={status}"]
    if today:
        fields.append(f"today={today}")
    if paints:
        fields.append(f"paints={','.join(paints)}")
    if pending:
        fields.append(f"pending={','.join(pending)}")
    fields.append(f"spec={owner}")
    if flow:
        fields.append(f"flow={','.join(flow)}")
    if composes:
        fields.append(f"composes={','.join(composes)}")
    return f"{tier.capitalize()} of Studio's chrome, specified in spec {owner}.\n" + "; ".join(fields)


def record(
    name: str,
    tier: str,
    *,
    rust: Sequence[str],
    status: str = "exists",
    tokens: Sequence[str] = (),
    paints: Sequence[str] = (),
    pending: Sequence[str] = (),
    variants: Sequence[str] = (),
    flow: Sequence[str] = (),
    composes: Sequence[str] = (),
    category: Optional[str] = None,
    today: str = "",
) -> Dict[str, object]:
    children = "".join(f'<div data-ds="{c}"></div>' for c in composes)
    return {
        "name": name,
        "category": category or tier,
        "description": describe(
            tier, rust=rust, status=status, paints=paints, pending=pending, flow=flow, composes=composes, today=today
        ),
        "markup": f'<div data-ds="{name}">{children}</div>\n',
        "scope": "local",
        "tokensUsed": list(tokens),
        "variants": list(variants),
    }


ALL_STATES = ["State=Default", "State=Hover", "State=Active", "State=Focus", "State=Disabled"]


def good_records() -> List[Dict[str, object]]:
    return [
        record("Divider/Hairline", "element", rust=["widgets/mod.rs::hline"], tokens=["colors.border"], paints=["BORDER"]),
        record(
            "Button/Primary",
            "component",
            rust=["kit/button.rs::button"],
            status="new-in-C",
            tokens=["colors.primary"],
            paints=["BG_BRAND"],
            variants=["Size=Xs", *ALL_STATES],
        ),
        record(
            "Button/Close",
            "component",
            rust=["kit/button.rs::button"],
            status="new-in-C",
            tokens=["colors.primary"],
            paints=["BG_BRAND"],
            variants=ALL_STATES[:4],
        ),
        record(
            "Tooltip/Default",
            "component",
            rust=["theme.rs::apply"],
            paints=["BG_PRIMARY_SOLID"],
            pending=["BG_PRIMARY_SOLID"],
        ),
        record(
            "Panel/Header",
            "composite",
            rust=["kit/rows.rs::panel_header"],
            status="new-in-C",
            composes=["Button/Primary", "Divider/Hairline"],
        ),
        record(
            "Navigation/LeftRail",
            "region",
            rust=["panels/rail.rs::show"],
            composes=["Panel/Header", "Button/*"],
            variants=["Rail=48px"],
        ),
        record(
            "Screen/EditorDesign",
            "screen",
            rust=["shell.rs::draw_window"],
            flow=["editor-canvas"],
            composes=["Navigation/LeftRail", "Tooltip/Default"],
        ),
    ]


GOOD_FLOW = {
    "name": "fixture",
    "entryPoints": ["editor-canvas"],
    "screens": [
        {"id": "editor-canvas", "name": "Editor", "mode": "screen", "artifact": "crates/designos-app/src/canvas/mod.rs"}
    ],
    "transitions": [],
}

GOOD_TESTDATA = {
    "tokens": {},
    "rows": {
        "Button/Primary": {
            "Default": {},
            "Hover": {},
            "Pressed": {},
            "Focused": {},
            "Disabled": {},
            "Selected": None,
        },
        "Button/Close": {
            "Default": {},
            "Hover": {},
            "Pressed": {},
            "Focused": {},
            "Disabled": None,
            "Selected": None,
        },
    },
}

SOURCE_FILES = {
    "widgets/mod.rs": "pub fn hline() {}\n",
    "panels/rail.rs": "pub(crate) fn show() {}\n",
    "shell.rs": "pub fn draw_window() {}\n",
    "canvas/mod.rs": "pub fn show() {}\n",
    "home/mod.rs": "pub mod sidebar {\n    pub fn show() {}\n}\n",
    "widgets/kit/button.rs": "pub fn button() {}\n",
    "widgets/kit/rows.rs": "pub fn panel_header() {}\n",
}


def write_json(path: Path, data: object) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(data, indent=1), encoding="utf-8")


def build_tree(
    root: Path,
    records: Optional[Iterable[Dict[str, object]]] = None,
    flow: Optional[Dict[str, object]] = None,
    testdata: Optional[Dict[str, object]] = None,
    store_extra: Optional[Dict[str, str]] = None,
) -> Path:
    """A minimal tree the gate accepts: the real theme and token store, a small valid registry."""
    src = root / APP_SRC_REL
    src.mkdir(parents=True, exist_ok=True)
    shutil.copy2(REPO_ROOT / APP_SRC_REL / "theme.rs", src / "theme.rs")
    for rel, body in SOURCE_FILES.items():
        target = src / rel
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_text(body, encoding="utf-8")
    design = root / "design"
    design.mkdir(parents=True, exist_ok=True)
    store = json.loads((REPO_ROOT / "design" / "design.tokens.json").read_text(encoding="utf-8"))
    for path, value in (store_extra or {}).items():
        group, key = path.split(".", 1)
        store[group][key] = {"$type": "color", "$value": value}
    write_json(design / "design.tokens.json", store)
    write_json(
        design / "component-registry.json",
        {"version": "0.1.0", "components": list(records if records is not None else good_records())},
    )
    write_json(design / "flow.json", flow or GOOD_FLOW)
    (design / "soul.md").write_text(f"# Soul\n\n{CREDIT_LINE}\n", encoding="utf-8")
    (design / "principles.md").write_text(f"# Principles\n\n{CREDIT_LINE}\n", encoding="utf-8")
    write_json(root / TESTDATA_REL / "specimen.json", testdata if testdata is not None else GOOD_TESTDATA)
    return root


def run_gate(root: Path, *args: str) -> tuple[int, str]:
    out = io.StringIO()
    with contextlib.redirect_stdout(out), contextlib.redirect_stderr(out):
        rc = gate.main(["--root", str(root), *args])
    return rc, out.getvalue()


def copy_real_sources(root: Path) -> None:
    """The real chrome sources and the two ratchet scripts, for the checks that read painted code."""
    shutil.copytree(REPO_ROOT / APP_SRC_REL, root / APP_SRC_REL, dirs_exist_ok=True)
    scripts = root / "scripts"
    scripts.mkdir(parents=True, exist_ok=True)
    for name in (
        "check_chrome_colour_literals.py",
        "chrome_colour_baseline.json",
        "check_spacing_relationships.py",
        "spacing_baseline.json",
        "rust_literal_scan.py",
    ):
        shutil.copy2(REPO_ROOT / "scripts" / name, scripts / name)


class GateFixture(unittest.TestCase):
    def setUp(self) -> None:
        self._tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self._tmp.cleanup)
        self.root = Path(self._tmp.name)

    def tree(self, **kw: object) -> Path:
        return build_tree(self.root, **kw)  # type: ignore[arg-type]

    def assert_red(self, check: str, *args: str, needle: str = "") -> str:
        rc, out = run_gate(self.root, "--only", check, *args)
        self.assertEqual(rc, 1, f"{check} should be red:\n{out}")
        self.assertIn(f"{check} ", out)
        if needle:
            self.assertIn(needle, out)
        return out

    def assert_green(self, check: str, *args: str) -> str:
        rc, out = run_gate(self.root, "--only", check, *args)
        self.assertEqual(rc, 0, f"{check} should be green:\n{out}")
        return out


class CleanTreeTests(GateFixture):
    def test_clean_fixture_passes_p1_p3_p4_p5_p6_p9(self) -> None:
        self.tree()
        for check in ("P1", "P3", "P4", "P5", "P6", "P9"):
            with self.subTest(check=check):
                self.assert_green(check)

    def test_unknown_check_id_is_an_argument_error(self) -> None:
        self.tree()
        rc, _ = run_gate(self.root, "--only", "P8")
        self.assertEqual(rc, 2)


class P1RegistryRustTests(GateFixture):
    def mutate(self, name: str, **changes: object) -> None:
        records = good_records()
        for r in records:
            if r["name"] == name:
                r.update(changes)
        self.tree(records=records)

    def test_exists_symbol_that_does_not_resolve_is_red(self) -> None:
        records = good_records()
        records[0] = record(
            "Divider/Hairline", "element", rust=["widgets/kit/nope.rs::x"], tokens=["colors.border"], paints=["BORDER"]
        )
        self.tree(records=records)
        self.assert_red("P1", needle="widgets/kit/nope.rs::x")

    def test_function_missing_from_an_existing_file_is_red(self) -> None:
        records = good_records()
        records[0] = record(
            "Divider/Hairline", "element", rust=["widgets/mod.rs::vline"], tokens=["colors.border"], paints=["BORDER"]
        )
        self.tree(records=records)
        self.assert_red("P1", needle="widgets/mod.rs::vline")

    def test_symbol_claimed_by_two_families_is_red(self) -> None:
        records = good_records()
        records.append(
            record("Tabs/Pill", "component", rust=["kit/button.rs::button"], status="new-in-C", tokens=["colors.primary"], paints=["BG_BRAND"])
        )
        self.tree(records=records)
        self.assert_red("P1", needle="kit/button.rs::button")

    def test_record_without_a_description_line_is_red(self) -> None:
        records = good_records()
        records[0]["description"] = "Hairline divider, no binding line."
        self.tree(records=records)
        self.assert_red("P1", needle="Divider/Hairline")

    def test_record_without_a_rust_symbol_is_red(self) -> None:
        records = good_records()
        records[0]["description"] = "Element of Studio's chrome, specified in spec C.\ntier=element; rust_status=exists; spec=C"
        self.tree(records=records)
        self.assert_red("P1", needle="Divider/Hairline")

    def test_status_outside_exists_and_new_in_c_d_k_is_red(self) -> None:
        for status in ("new-in-Z", "new-in-T", "new-in-A", "new-in-c", "new-in-CD", "new-in-", "pending"):
            with self.subTest(status=status):
                records = good_records()
                records[0] = record(
                    "Divider/Hairline",
                    "element",
                    rust=["widgets/kit/nope.rs::x"],
                    status=status,
                    tokens=["colors.border"],
                    paints=["BORDER"],
                )
                self.tree(records=records)
                self.assert_red("P1", needle=f"rust_status='{status}'")

    def test_new_in_c_symbol_that_does_not_resolve_is_red(self) -> None:
        records = good_records()
        records[0] = record(
            "Divider/Hairline",
            "element",
            rust=["widgets/kit/nope.rs::x"],
            status="new-in-C",
            tokens=["colors.border"],
            paints=["BORDER"],
        )
        self.tree(records=records)
        self.assert_red("P1", needle="widgets/kit/nope.rs::x")

    def test_new_in_c_function_missing_from_an_existing_file_is_red(self) -> None:
        records = good_records()
        records[1] = record(
            "Button/Primary",
            "component",
            rust=["kit/button.rs::not_a_button"],
            status="new-in-C",
            tokens=["colors.primary"],
            paints=["BG_BRAND"],
        )
        self.tree(records=records)
        self.assert_red("P1", needle="kit/button.rs::not_a_button")

    def test_new_in_d_and_k_are_reported_not_blocking(self) -> None:
        for status in ("new-in-D", "new-in-K"):
            with self.subTest(status=status):
                records = good_records()
                records[0] = record(
                    "Divider/Hairline",
                    "element",
                    rust=["widgets/kit/nope.rs::x"],
                    status=status,
                    tokens=["colors.border"],
                    paints=["BORDER"],
                )
                self.tree(records=records)
                out = self.assert_green("P1")
                self.assertIn(status, out)

    def test_new_symbols_are_reported_not_blocking(self) -> None:
        self.tree()
        out = self.assert_green("P1")
        self.assertIn("new-in", out)

    def test_inline_module_symbol_resolves(self) -> None:
        records = good_records()
        records.append(
            record("Panel/HomeSidebar", "region", rust=["home/mod.rs::sidebar::show"])
        )
        self.tree(records=records)
        self.assert_green("P1")

    def test_today_is_never_checked(self) -> None:
        records = good_records()
        records[0] = record(
            "Divider/Hairline",
            "element",
            rust=["widgets/mod.rs::hline"],
            tokens=["colors.border"],
            paints=["BORDER"],
            today="nowhere/at-all.rs:1-2; other.rs::gone",
        )
        self.tree(records=records)
        self.assert_green("P1")


class P2PaintedCodeTests(GateFixture):
    def test_injected_colour_literal_in_a_panel_file_is_red(self) -> None:
        copy_real_sources(self.root)
        self.assert_green("P2")
        target = self.root / APP_SRC_REL / "tabs.rs"
        target.write_text(
            target.read_text(encoding="utf-8")
            + "\nfn __fixture_literal() { let _c = Color32::from_rgb(1, 2, 3); }\n",
            encoding="utf-8",
        )
        self.assert_red("P2", needle="check_chrome_colour_literals")


class P3NamesStoreRecordsTests(GateFixture):
    def theme_path(self) -> Path:
        return self.root / APP_SRC_REL / "theme.rs"

    def test_tooltip_binding_a_key_with_another_value_is_red(self) -> None:
        records = good_records()
        for r in records:
            if r["name"] == "Tooltip/Default":
                r["tokensUsed"] = ["colors.popover"]
        self.tree(records=records)
        self.assert_red("P3", needle="Tooltip/Default")

    def test_alpha_lost_in_theme_is_red(self) -> None:
        self.tree()
        theme = self.theme_path()
        src = theme.read_text(encoding="utf-8")
        self.assertIn("pub const BG_OVERLAY: Color32 = rgba(0x262626, 0xB3);", src)
        theme.write_text(
            src.replace(
                "pub const BG_OVERLAY: Color32 = rgba(0x262626, 0xB3);",
                "pub const BG_OVERLAY: Color32 = rgb(0x262626);",
            ),
            encoding="utf-8",
        )
        self.assert_red("P3", needle="BG_OVERLAY")

    def test_new_colour_constant_outside_every_class_is_red(self) -> None:
        self.tree()
        theme = self.theme_path()
        theme.write_text(theme.read_text(encoding="utf-8") + "\npub const FOO: Color32 = rgb(0x123456);\n", encoding="utf-8")
        self.assert_red("P3", needle="FOO")

    def test_new_colour_constant_with_a_trailing_comment_is_red(self) -> None:
        self.tree()
        theme = self.theme_path()
        theme.write_text(
            theme.read_text(encoding="utf-8") + "\npub const FOO: Color32 = rgb(0x123456); // new chrome colour\n",
            encoding="utf-8",
        )
        self.assert_red("P3", needle="FOO")

    def test_new_colour_constant_wrapped_over_two_lines_is_red(self) -> None:
        self.tree()
        theme = self.theme_path()
        theme.write_text(
            theme.read_text(encoding="utf-8") + "\npub const FOO: Color32 =\n    rgb(0x123456);\n",
            encoding="utf-8",
        )
        self.assert_red("P3", needle="FOO")

    def test_painted_colour_neither_bound_nor_pending_is_red(self) -> None:
        records = good_records()
        for r in records:
            if r["name"] == "Tooltip/Default":
                r["description"] = describe("component", rust=["theme.rs::apply"], paints=["BG_PRIMARY_SOLID"])
        self.tree(records=records)
        self.assert_red("P3", needle="BG_PRIMARY_SOLID")

    def test_pending_name_that_is_not_in_the_pending_class_is_red(self) -> None:
        records = good_records()
        for r in records:
            if r["name"] == "Divider/Hairline":
                r["description"] = describe(
                    "element", rust=["widgets/mod.rs::hline"], paints=["BORDER"], pending=["BORDER"]
                )
        self.tree(records=records)
        self.assert_red("P3", needle="BORDER")

    def test_paints_constant_missing_from_theme_is_red(self) -> None:
        records = good_records()
        records[0]["description"] = describe(
            "element", rust=["widgets/mod.rs::hline"], paints=["BORDER", "NO_SUCH_CONSTANT"]
        )
        self.tree(records=records)
        self.assert_red("P3", needle="NO_SUCH_CONSTANT")

    def test_pending_value_drift_is_red(self) -> None:
        self.tree()
        theme = self.theme_path()
        src = theme.read_text(encoding="utf-8")
        self.assertIn("pub const BG_PRIMARY_SOLID: Color32 = rgb(0x262626);", src)
        theme.write_text(src.replace("BG_PRIMARY_SOLID: Color32 = rgb(0x262626);", "BG_PRIMARY_SOLID: Color32 = rgb(0x262627);"), encoding="utf-8")
        self.assert_red("P3", needle="BG_PRIMARY_SOLID")

    def test_mapped_name_with_a_different_store_value_is_red(self) -> None:
        self.tree()
        store_path = self.root / "design" / "design.tokens.json"
        store = json.loads(store_path.read_text(encoding="utf-8"))
        store["colors"]["border"]["$value"] = "#010203"
        write_json(store_path, store)
        self.assert_red("P3", needle="BORDER")


class P4TiersTests(GateFixture):
    def test_category_outside_the_five_tiers_is_red(self) -> None:
        records = good_records()
        records[0]["category"] = "action"
        self.tree(records=records)
        self.assert_red("P4", needle="Divider/Hairline")

    def test_child_of_a_higher_tier_is_red(self) -> None:
        records = good_records()
        records.append(
            record("Toast/Default", "composite", rust=["kit/surface.rs::toast_surface"], status="new-in-C", composes=["Screen/EditorDesign"])
        )
        self.tree(records=records)
        self.assert_red("P4", needle="Toast/Default")

    def test_unknown_child_is_red(self) -> None:
        records = good_records()
        records[4] = record(
            "Panel/Header", "composite", rust=["kit/rows.rs::panel_header"], status="new-in-C", composes=["Ghost/Name"]
        )
        self.tree(records=records)
        self.assert_red("P4", needle="Ghost/Name")

    def test_wildcard_with_no_member_is_red(self) -> None:
        records = good_records()
        records[4] = record(
            "Panel/Header", "composite", rust=["kit/rows.rs::panel_header"], status="new-in-C", composes=["Nothing/*"]
        )
        self.tree(records=records)
        self.assert_red("P4", needle="Nothing/*")

    def test_cycle_inside_one_tier_is_red(self) -> None:
        records = good_records()
        records.append(record("Menu/Item", "component", rust=["kit/menu.rs::item"], status="new-in-C", composes=["Menu/Row"]))
        records.append(record("Menu/Row", "component", rust=["kit/menu.rs::row"], status="new-in-C", composes=["Menu/Item"]))
        self.tree(records=records)
        self.assert_red("P4", needle="cycle")

    def test_same_tier_nesting_without_a_cycle_is_green(self) -> None:
        records = good_records()
        records.append(record("Tabs/Pill", "component", rust=["kit/tabs.rs::tab"], status="new-in-C"))
        records.append(
            record("Tabs/Segmented", "composite", rust=["kit/tabs.rs::segmented"], status="new-in-C", composes=["Tabs/Pill"])
        )
        records.append(
            record("Dialog/Modal", "composite", rust=["kit/surface.rs::modal_surface"], status="new-in-C", composes=["Panel/Header", "Tabs/Segmented"])
        )
        self.tree(records=records)
        self.assert_green("P4")

    def test_primitive_ramp_token_is_red(self) -> None:
        records = good_records()
        records[0]["tokensUsed"] = ["colors.ramp-neutral-950"]
        self.tree(records=records, store_extra={"colors.ramp-neutral-950": "#0a0a0a"})
        self.assert_red("P4", needle="colors.ramp-neutral-950")

    def test_markup_children_that_disagree_with_composes_are_red(self) -> None:
        records = good_records()
        records[4]["markup"] = '<div data-ds="Panel/Header"></div>\n'
        self.tree(records=records)
        self.assert_red("P4", needle="Panel/Header")


class P5StatesTests(GateFixture):
    def test_close_button_with_a_disabled_state_is_red(self) -> None:
        records = good_records()
        records[2]["variants"] = ALL_STATES
        self.tree(records=records)
        self.assert_red("P5", needle="Button/Close")

    def test_primary_button_without_disabled_is_red(self) -> None:
        records = good_records()
        records[1]["variants"] = ["Size=Xs", *ALL_STATES[:4]]
        self.tree(records=records)
        self.assert_red("P5", needle="Button/Primary")

    def test_open_and_loading_states_are_ignored(self) -> None:
        records = good_records()
        records[1]["variants"] = ["Size=Xs", "State=Open", "State=Loading", *ALL_STATES]
        self.tree(records=records)
        self.assert_green("P5")

    def test_selected_travels_as_a_variant(self) -> None:
        data = json.loads(json.dumps(GOOD_TESTDATA))
        data["rows"]["Button/Primary"]["Selected"] = {}
        self.tree(testdata=data)
        self.assert_red("P5", needle="Button/Primary")

    def test_no_states_source_is_report_only(self) -> None:
        self.tree()
        shutil.rmtree(self.root / TESTDATA_REL)
        out = self.assert_green("P5")
        self.assertIn("report-only", out)

    def test_explicit_states_source_is_blocking(self) -> None:
        self.tree()
        shutil.rmtree(self.root / TESTDATA_REL)
        source = self.root / "states.json"
        write_json(
            source,
            [
                {"name": "Button/Primary", "states": ["default", "hover", "active", "focus", "disabled"], "selected": False},
                {"name": "Button/Close", "states": ["default", "hover", "active", "focus", "disabled"], "selected": False},
            ],
        )
        self.assert_red("P5", "--states-source", str(source), needle="Button/Close")


class P6FlowTests(GateFixture):
    def test_flow_screen_without_a_record_is_red(self) -> None:
        flow = json.loads(json.dumps(GOOD_FLOW))
        flow["screens"].append(
            {"id": "home-recents", "name": "Home", "mode": "screen", "artifact": "crates/designos-app/src/canvas/mod.rs"}
        )
        self.tree(flow=flow)
        self.assert_red("P6", needle="home-recents")

    def test_flow_id_listed_by_two_records_is_red(self) -> None:
        records = good_records()
        records.append(
            record("Screen/Present", "screen", rust=["shell.rs::draw_window"], flow=["editor-canvas"], composes=["Navigation/LeftRail"])
        )
        self.tree(records=records)
        self.assert_red("P6", needle="editor-canvas")

    def test_missing_artifact_is_red(self) -> None:
        flow = json.loads(json.dumps(GOOD_FLOW))
        flow["screens"][0]["artifact"] = "crates/designos-app/src/proto/present.rs"
        self.tree(flow=flow)
        self.assert_red("P6", needle="proto/present.rs")

    def test_record_naming_an_unknown_flow_id_is_red(self) -> None:
        records = good_records()
        records[-1] = record(
            "Screen/EditorDesign", "screen", rust=["shell.rs::draw_window"], flow=["editor-canvas", "no-such-screen"], composes=["Navigation/LeftRail"]
        )
        self.tree(records=records)
        self.assert_red("P6", needle="no-such-screen")


class P7PinnedSiteTests(GateFixture):
    def test_unmodified_tree_matches_the_pins(self) -> None:
        copy_real_sources(self.root)
        self.assert_green("P7")

    def edit(self, rel: str, old: str, new: str, count: int = 1) -> None:
        path = self.root / APP_SRC_REL / rel
        text = path.read_text(encoding="utf-8")
        self.assertIn(old, text)
        path.write_text(text.replace(old, new, count), encoding="utf-8")

    def test_new_upper_case_tag_literal_is_red(self) -> None:
        copy_real_sources(self.root)
        self.edit("skills/mod.rs", '"READY"', '"BETA"')
        self.assert_red("P7", needle="BETA")

    def test_second_monospace_call_in_a_pinned_function_is_red(self) -> None:
        copy_real_sources(self.root)
        path = self.root / APP_SRC_REL / "specs.rs"
        lines = path.read_text(encoding="utf-8").split("\n")
        in_fn, hit = False, None
        for i, line in enumerate(lines):
            if re.match(r"\s*(?:pub(?:\([a-z]+\))? )?fn show_right\b", line):
                in_fn = True
            if in_fn and "font_mono(" in line:
                hit = i
                break
        self.assertIsNotNone(hit)
        lines.insert(hit + 1, lines[hit])
        path.write_text("\n".join(lines), encoding="utf-8")
        self.assert_red("P7", needle="specs.rs")

    def test_mesh_constructor_in_chrome_is_red(self) -> None:
        copy_real_sources(self.root)
        path = self.root / APP_SRC_REL / "panels" / "toolbar.rs"
        path.write_text(
            path.read_text(encoding="utf-8") + "\nfn __fixture_mesh() { let _m = egui::Mesh::default(); }\n",
            encoding="utf-8",
        )
        self.assert_red("P7", needle="toolbar.rs")

    def test_brand_fill_as_selected_state_is_red(self) -> None:
        copy_real_sources(self.root)
        path = self.root / APP_SRC_REL / "panels" / "toolbar.rs"
        path.write_text(
            path.read_text(encoding="utf-8")
            + "\nfn __fixture_selected(selected: bool) -> Color32 { if selected { BG_BRAND } else { BG } }\n",
            encoding="utf-8",
        )
        self.assert_red("P7", needle="BG_BRAND")

    def test_a_removed_site_must_leave_the_pin(self) -> None:
        copy_real_sources(self.root)
        self.edit("skills/mod.rs", '"READY"', '"Ready"')
        out = self.assert_red("P7", needle="READY")
        self.assertIn("pin", out)

    def test_acronyms_are_allowed(self) -> None:
        sites = gate.site_pins.scan_source("panels/x.rs", 'fn f(ui: &mut Ui) { ui.label("MCP"); ui.label("PRD"); }\n')
        self.assertEqual(sites["ns1"], {})
        sites = gate.site_pins.scan_source("panels/x.rs", 'fn f(ui: &mut Ui) { ui.label("BETA"); }\n')
        self.assertEqual(sites["ns1"], {("panels/x.rs", "f", "BETA"): 1})


class P9NamesTests(GateFixture):
    def test_kit_role_as_a_store_key_is_red(self) -> None:
        self.tree(store_extra={"colors.bg-secondary_hover": "#262626"})
        contract = self.root / "contract.md"
        contract.write_text(CONTRACT_FIXTURE, encoding="utf-8")
        self.assert_red("P9", "--contract", str(contract), needle="bg-secondary_hover")

    def test_kit_role_as_a_testdata_key_is_red(self) -> None:
        data = json.loads(json.dumps(GOOD_TESTDATA))
        data["tokens"] = {"fg-quaternary": "#737373"}
        self.tree(testdata=data)
        contract = self.root / "contract.md"
        contract.write_text(CONTRACT_FIXTURE, encoding="utf-8")
        self.assert_red("P9", "--contract", str(contract), needle="fg-quaternary")

    def test_studio_name_that_equals_a_kit_role_string_is_green(self) -> None:
        # text-placeholder is a Studio key (kebab of TEXT_PLACEHOLDER); colors.border-translucent is the
        # kebab of BORDER_TRANSLUCENT. Neither may turn the gate red.
        self.tree(store_extra={"colors.border-translucent": "#ffffff1a", "colors.text-placeholder": "#737373"})
        contract = self.root / "contract.md"
        contract.write_text(CONTRACT_FIXTURE, encoding="utf-8")
        self.assert_green("P9", "--contract", str(contract))

    def test_substring_of_a_kit_role_is_not_a_hit(self) -> None:
        self.tree(store_extra={"colors.bg-secondary_hover-state": "#262626"})
        contract = self.root / "contract.md"
        contract.write_text(CONTRACT_FIXTURE, encoding="utf-8")
        self.assert_green("P9", "--contract", str(contract))

    def test_missing_contract_is_not_run_but_other_subchecks_stay_blocking(self) -> None:
        self.tree()
        out = self.assert_green("P9")
        self.assertIn("NOT RUN", out)
        records = good_records()
        records[0]["variants"] = ["Hierarchy=Primary"]
        build_tree(self.root, records=records)
        self.assert_red("P9", needle="Hierarchy")

    def test_unknown_variant_axis_is_red(self) -> None:
        records = good_records()
        records[1]["variants"] = ["Hierarchy=Primary", *ALL_STATES]
        self.tree(records=records)
        self.assert_red("P9", needle="Hierarchy=Primary")

    def test_allowed_variant_axes_are_green(self) -> None:
        records = good_records()
        records[1]["variants"] = ["Size=Xs", "Tone=Brand", "Kind=A", "Rail=48px", "Bar=32px", "Axis=Scrub", *ALL_STATES]
        self.tree(records=records)
        self.assert_green("P9")

    def test_untitled_ui_outside_the_two_credit_lines_is_red(self) -> None:
        self.tree()
        (self.root / "design" / "DESIGN.md").write_text("Built on Untitled UI tokens.\n", encoding="utf-8")
        self.assert_red("P9", needle="DESIGN.md")

    def test_second_credit_line_is_red(self) -> None:
        self.tree()
        soul = self.root / "design" / "soul.md"
        soul.write_text(soul.read_text(encoding="utf-8") + "\nAlso Untitled UI here.\n", encoding="utf-8")
        self.assert_red("P9", needle="soul.md")

    def test_missing_credit_line_is_red(self) -> None:
        self.tree()
        (self.root / "design" / "principles.md").write_text("# Principles\n", encoding="utf-8")
        self.assert_red("P9", needle="principles.md")

    def test_bare_untitled_word_stays_legal(self) -> None:
        records = good_records()
        records[0]["markup"] = '<div data-ds="Divider/Hairline"><span>Untitled</span></div>\n'
        self.tree(records=records)
        self.assert_green("P9")

    def test_utility_constant_in_crates_is_red(self) -> None:
        self.tree()
        extra = self.root / "crates" / "designos-app" / "src" / "x.rs"
        extra.write_text("const A: Color32 = UTILITY_GREEN_700;\n", encoding="utf-8")
        self.assert_red("P9", needle="UTILITY_GREEN_700")


class ColourClassTests(unittest.TestCase):
    """G01: every theme.rs colour name falls in exactly one declared class."""

    def setUp(self) -> None:
        self.theme = (REPO_ROOT / APP_SRC_REL / "theme.rs").read_text(encoding="utf-8")
        store = json.loads((REPO_ROOT / "design" / "design.tokens.json").read_text(encoding="utf-8"))
        self.store = dse.extract_design_token_colors(store)

    def test_the_four_classes_cover_every_theme_name(self) -> None:
        report = dse.check_colour_classes(self.theme, self.store)
        self.assertEqual(report["problems"], [])
        counts = report["counts"]
        self.assertEqual(sum(counts.values()), 188)
        self.assertEqual(
            counts, {"mapped": 32, "shared-key": 21, "pending": 68, "exempt": 67}
        )

    def test_classes_are_disjoint(self) -> None:
        classes = [
            set(dse.RUST_TO_DESIGN_TOKEN_MAP),
            set(dse.RUST_COLOUR_SHARED_KEY),
            set(dse.RUST_COLOUR_PENDING_STORE),
            set(dse.RUST_COLOUR_EXEMPT),
        ]
        for i, a in enumerate(classes):
            for b in classes[i + 1 :]:
                self.assertEqual(a & b, set())

    def test_pending_paths_follow_the_studio_naming_rule(self) -> None:
        for name, (path, value) in dse.RUST_COLOUR_PENDING_STORE.items():
            self.assertEqual(path, "colors." + name.lower().replace("_", "-"), name)
            self.assertRegex(value, r"^#[0-9A-F]{6}([0-9A-F]{2})?$", name)

    def test_unclassified_name_is_reported(self) -> None:
        report = dse.check_colour_classes(self.theme + "\npub const FOO: Color32 = rgb(0x123456);\n", self.store)
        self.assertTrue(any("FOO" in p for p in report["problems"]))

    def test_every_declared_colour_constant_is_read_on_the_real_tree(self) -> None:
        report = dse.check_colour_classes(self.theme, self.store)
        self.assertEqual(report["declared"], 188)
        self.assertEqual(report["declared"], report["total"])

    def test_constant_with_a_trailing_comment_is_unclassified(self) -> None:
        report = dse.check_colour_classes(
            self.theme + "\npub const FOO: Color32 = rgb(0x123456); // new chrome colour\n", self.store
        )
        self.assertEqual(report["declared"], 189)
        self.assertEqual(sum(1 for p in report["problems"] if "FOO" in p), 1, report["problems"])
        self.assertTrue(any("FOO is unclassified" in p for p in report["problems"]))

    def test_constant_wrapped_over_two_lines_is_unclassified(self) -> None:
        report = dse.check_colour_classes(self.theme + "\npub const FOO: Color32 =\n    rgb(0x123456);\n", self.store)
        self.assertEqual(sum(1 for p in report["problems"] if "FOO" in p), 1, report["problems"])
        self.assertTrue(any("FOO is unclassified" in p for p in report["problems"]))

    def test_constant_array_with_a_trailing_comment_is_unclassified(self) -> None:
        report = dse.check_colour_classes(
            self.theme + "\npub const FOO: [Color32; 2] = [BORDER, TEXT]; // pair\n", self.store
        )
        self.assertTrue(any("FOO is unclassified" in p for p in report["problems"]))

    def test_classified_name_that_gains_a_trailing_comment_is_reported_once(self) -> None:
        src = self.theme.replace(
            "pub const BG_OVERLAY: Color32 = rgba(0x262626, 0xB3);",
            "pub const BG_OVERLAY: Color32 = rgba(0x262626, 0xB3); // scrim",
        )
        self.assertNotEqual(src, self.theme)
        report = dse.check_colour_classes(src, self.store)
        mentions = [p for p in report["problems"] if "BG_OVERLAY" in p]
        self.assertEqual(len(mentions), 1, mentions)
        self.assertIn("is unclassified", mentions[0])

    def test_pending_pin_keeps_the_alpha(self) -> None:
        src = self.theme.replace(
            "pub const BG_OVERLAY: Color32 = rgba(0x262626, 0xB3);", "pub const BG_OVERLAY: Color32 = rgb(0x262626);"
        )
        report = dse.check_colour_classes(src, self.store)
        self.assertTrue(any("BG_OVERLAY" in p for p in report["problems"]))

    def test_shared_key_value_drift_is_reported(self) -> None:
        store = dict(self.store)
        store["surface-hover"] = "#010101"
        report = dse.check_colour_classes(self.theme, store)
        self.assertTrue(any("BG_SECONDARY_HOVER" in p for p in report["problems"]))

    def test_pending_name_that_gained_a_store_key_must_move(self) -> None:
        store = dict(self.store)
        store["bg-primary-solid"] = "#262626"
        report = dse.check_colour_classes(self.theme, store)
        self.assertTrue(any("BG_PRIMARY_SOLID" in p for p in report["problems"]))

    def test_theme_value_reader_resolves_aliases_and_alpha(self) -> None:
        values = dse.parse_theme_colour_values(
            "pub const A: Color32 = rgb(0x0a0b0c);\n"
            "pub const B: Color32 = rgba(0xFFFFFF, 0x1A);\n"
            "pub const E: Color32 = Color32::from_rgb(151, 71, 255);\n"
            "pub const F: Color32 = Color32::from_rgb(0x97, 71, 0xFF);\n"
            "pub const G: Color32 = Color32::from_rgb(001, 008, 003);\n"
            "pub const C: Color32 = A;\n"
            "pub const D: [Color32; 2] = [A, B];\n"
        )
        self.assertEqual(values["A"], "#0A0B0C")
        self.assertEqual(values["B"], "#FFFFFF1A")
        self.assertEqual(values["E"], "#9747FF")
        self.assertEqual(values["F"], "#9747FF")
        self.assertEqual(values["G"], "#010803")
        self.assertEqual(values["C"], "#0A0B0C")
        self.assertIsNone(values["D"])

    def test_the_renamed_badge_and_tile_constants_exist_with_their_values(self) -> None:
        values = dse.parse_theme_colour_values(self.theme)
        expected = {
            "BADGE_NEUTRAL_BG": "#171717",
            "BADGE_NEUTRAL_BORDER": "#404040",
            "BADGE_NEUTRAL_TEXT": "#D4D4D4",
            "BADGE_SUCCESS_BORDER": "#166534",
            "BADGE_WARNING_BORDER": "#854D0E",
            "BADGE_ERROR_BORDER": "#991B1B",
            "BADGE_SUCCESS_TEXT": "#86EFAC",
            "BADGE_WARNING_TEXT": "#FDE047",
            "BADGE_ERROR_TEXT": "#FCA5A5",
            "TILE_DESIGN_BG": "#172554",
            "TILE_DESIGN_GLYPH": "#60A5FA",
            "TILE_BOARD_BG": "#451A03",
            "TILE_BOARD_GLYPH": "#FBBF24",
            "TILE_DECK_BG": "#052E16",
            "TILE_DECK_GLYPH": "#4ADE80",
            "TILE_SPEC_BG": "#431407",
            "TILE_SPEC_GLYPH": "#FB923C",
        }
        self.assertEqual({k: values.get(k) for k in expected}, expected)
        self.assertEqual([n for n in values if n.startswith("UTILITY_")], [])


class DescriptionParserTests(unittest.TestCase):
    def test_today_with_semicolons_and_equals_does_not_split_fields(self) -> None:
        text = (
            "Region of Studio's chrome, specified in spec G.\n"
            "tier=region; rust=specs.rs::show_reader; rust_status=exists; today=hidden: specs_reader_open = false; spec=G; flow=panel-specs-reader"
        )
        fields = gate.parse_description(text)
        assert fields is not None
        self.assertEqual(fields["today"], "hidden: specs_reader_open = false")
        self.assertEqual(fields["spec"], "G")
        self.assertEqual(fields["flow"], "panel-specs-reader")
        text = "X of Studio's chrome, specified in spec C.\ntier=composite; rust=cmdk.rs::show; rust_status=new-in-C; today=cmdk.rs::show; panels/menus.rs:907-1085; spec=C/J/D"
        fields = gate.parse_description(text)
        assert fields is not None
        self.assertEqual(fields["today"], "cmdk.rs::show; panels/menus.rs:907-1085")
        self.assertEqual(fields["spec"], "C/J/D")

    def test_description_without_a_tier_line_has_no_fields(self) -> None:
        self.assertIsNone(gate.parse_description("Just a sentence."))


class RealTreeTests(unittest.TestCase):
    def test_the_repository_passes_the_gate(self) -> None:
        out = io.StringIO()
        with contextlib.redirect_stdout(out), contextlib.redirect_stderr(out):
            rc = gate.main(["--root", str(REPO_ROOT)])
        self.assertEqual(rc, 0, out.getvalue())


if __name__ == "__main__":
    unittest.main()
