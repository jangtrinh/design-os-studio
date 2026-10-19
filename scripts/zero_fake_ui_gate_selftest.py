#!/usr/bin/env python3
"""
Self-test for `scripts/zero_fake_ui_gate.py` (pytest-free).

Re-introduces each known fake-UI regression into an in-memory copy of the app source and
asserts the gate FAILS on exactly the invariant that guards it. The files on disk are never
touched. Also asserts the gate still PASSES the unmodified tree and ignores comment-only /
test-only mentions of the retired names.

Run: python3 scripts/zero_fake_ui_gate_selftest.py   (exit 0 = every regression is caught)
"""

from __future__ import annotations

import importlib.util
import sys
from pathlib import Path
from typing import Dict, List, Optional, Tuple

GATE_PATH = Path(__file__).with_name("zero_fake_ui_gate.py")
_spec = importlib.util.spec_from_file_location("zero_fake_ui_gate", GATE_PATH)
assert _spec and _spec.loader
gate = importlib.util.module_from_spec(_spec)
sys.modules["zero_fake_ui_gate"] = gate
_spec.loader.exec_module(gate)

ZOOM_IN_ROW = 'ZoomRowSpec { label: "Zoom in", shortcut: "⌘+", action: Action::ZoomIn, checked: false, sep_after: false },'
SIDEBAR_GUARD = (
    "pub fn show_sidebar_card_in_rect(ui: &mut Ui, rect: Rect, state: &mut UpdaterState) {\n"
    "    if !UPDATER_UI_ENABLED {\n        return;\n    }\n"
)
MODAL_GUARD = (
    "pub fn show_update_modal(ctx: &Context, state: &mut UpdaterState) {\n"
    "    if !UPDATER_UI_ENABLED {\n        return;\n    }\n"
)

# (case label, invariant that must fail, file, anchor in the real source, replacement)
REGRESSIONS: List[Tuple[str, str, str, str, str]] = [
    (
        "1. toast-only ctx-crop (Icon::Rectangle) back in the toolbar",
        "Z08",
        "panels/toolbar.rs",
        '(Icon::Rectangle, "ctx-select-area", "Select area"),',
        '(Icon::Rectangle, "ctx-select-area", "Select area"), (Icon::Rectangle, "ctx-crop", "Crop"),',
    ),
    (
        '2. "Outlines" zoom row mapped to ZoomIn with a hard-coded check',
        "Z13",
        "panels/menus.rs",
        ZOOM_IN_ROW,
        ZOOM_IN_ROW + '\n        ZoomRowSpec { label: "Outlines", shortcut: "⌘⇧O", action: Action::ZoomIn, checked: true, sep_after: false },',
    ),
    (
        "3. `if UPDATER_UI_ENABLED` guard removed from tabs.rs (comment still names the constant)",
        "Z14",
        "tabs.rs",
        "if crate::home::updater::UPDATER_UI_ENABLED {",
        "{",
    ),
    (
        "4. updater modal guard removed",
        "Z14",
        "updater.rs",
        MODAL_GUARD,
        MODAL_GUARD.split("\n", 1)[0] + "\n",
    ),
    # Extra coverage of the same hardening rules.
    ("5. updater sidebar card guard removed", "Z14", "updater.rs", SIDEBAR_GUARD, SIDEBAR_GUARD.split("\n", 1)[0] + "\n"),
    ("6. cmdk 'Remove background' entry back", "Z08", "cmdk.rs", 'title: "Boost resolution",', 'title: "Remove background",'),
    ("7. ctx-remove-bg match arm back", "Z08", "panels/toolbar.rs", '"ctx-select-area" => {', '"ctx-remove-bg" => {}\n        "ctx-select-area" => {'),
    (
        "8. allow-listed zoom row with a hard-coded checked: true",
        "Z13",
        "panels/menus.rs",
        ZOOM_IN_ROW,
        ZOOM_IN_ROW.replace("checked: false", "checked: true"),
    ),
    (
        "9. unlisted zoom row even with a state-backed check",
        "Z13",
        "panels/menus.rs",
        ZOOM_IN_ROW,
        ZOOM_IN_ROW + '\n        ZoomRowSpec { label: "Outlines", shortcut: "", action: Action::ToggleRulers, checked: app.ui.rulers, sep_after: false },',
    ),
    (
        "10. allow-listed label rewired to a different action",
        "Z13",
        "panels/menus.rs",
        ZOOM_IN_ROW,
        ZOOM_IN_ROW.replace("Action::ZoomIn", "Action::ZoomOut"),
    ),
    (
        '11. concat!("Remove ", "background") evasion in cmdk.rs',
        "Z08",
        "cmdk.rs",
        'title: "Boost resolution",',
        'title: concat!("Remove ", "background"),',
    ),
    (
        r'12. "Remove \u{62}ackground" unicode-escape evasion in cmdk.rs',
        "Z08",
        "cmdk.rs",
        'title: "Boost resolution",',
        r'title: "Remove \u{62}ackground",',
    ),
    (
        "13. retired label const defined in another module (panels/mod.rs)",
        "Z08",
        "panels/mod.rs",
        "pub mod toolbar;",
        'pub mod toolbar;\npub const RETIRED_LABEL: &str = "Remove background";',
    ),
    (
        '14. concat!("ctx-", "crop") evasion in panels/toolbar.rs',
        "Z08",
        "panels/toolbar.rs",
        '(Icon::Rectangle, "ctx-select-area", "Select area"),',
        '(Icon::Rectangle, concat!("ctx-", "crop"), "Select area"),',
    ),
    (
        "15. updater guard rewritten with `&& false`",
        "Z14",
        "updater.rs",
        "if !UPDATER_UI_ENABLED {",
        "if !UPDATER_UI_ENABLED && false {",
    ),
    (
        "16. aliased module path (`use crate::home::updater as upd`) bypassing tabs.rs guard",
        "Z14",
        "tabs.rs",
        "if crate::home::updater::UPDATER_UI_ENABLED {",
        "use crate::home::updater as upd;\n    let _ = upd::UpdaterState::new();\n    if crate::home::updater::UPDATER_UI_ENABLED {",
    ),
    (
        "17. adjacent literals joined by a macro in panels/mod.rs",
        "Z08",
        "panels/mod.rs",
        "pub mod toolbar;",
        "pub mod toolbar;\n"
        "macro_rules! join_label { ($a:literal $b:literal) => { concat!($a, $b) }; }\n"
        'pub const RETIRED_LABEL: &str = join_label!("Remove " "background");',
    ),
    (
        '18. concat!(r"Remove ", r"background") with raw-string pieces in panels/mod.rs',
        "Z08",
        "panels/mod.rs",
        "pub mod toolbar;",
        'pub mod toolbar;\npub const RETIRED_LABEL: &str = concat!(r"Remove ", r"background");',
    ),
    (
        "19. concat! with a char piece spelling ctx-crop in panels/mod.rs",
        "Z08",
        "panels/mod.rs",
        "pub mod toolbar;",
        "pub mod toolbar;\npub const RETIRED_ACTION: &str = concat!(\"ctx-cro\", 'p');",
    ),
    (
        "20. retired label split by a backslash line continuation in panels/mod.rs",
        "Z08",
        "panels/mod.rs",
        "pub mod toolbar;",
        'pub mod toolbar;\npub const RETIRED_LABEL: &str = "Remove\\\n    background";',
    ),
    (
        r'21. retired label with a \t escape between the words in panels/mod.rs',
        "Z08",
        "panels/mod.rs",
        "pub mod toolbar;",
        'pub mod toolbar;\npub const RETIRED_LABEL: &str = "Remove\\tbackground";',
    ),
    (
        "22. updater modal guard reads a different UPDATER_UI_ENABLED (mod allow, = true)",
        "Z14",
        "updater.rs",
        MODAL_GUARD,
        "mod allow {\n    pub const UPDATER_UI_ENABLED: bool = true;\n}\n\n"
        "pub fn show_update_modal(ctx: &Context, state: &mut UpdaterState) {\n"
        "    if !allow::UPDATER_UI_ENABLED {\n        return;\n    }\n",
    ),
    (
        "23. tabs.rs button guard names a path other than crate::home::updater",
        "Z14",
        "tabs.rs",
        "if crate::home::updater::UPDATER_UI_ENABLED {",
        "if crate::elsewhere::UPDATER_UI_ENABLED {",
    ),
    (
        "24. updater modal guard shadowed by a local const UPDATER_UI_ENABLED = true",
        "Z14",
        "updater.rs",
        MODAL_GUARD,
        MODAL_GUARD + "    const UPDATER_UI_ENABLED: bool = true;\n",
    ),
    (
        "25. tabs.rs button guard reads a local `let UPDATER_UI_ENABLED = true` instead of the real constant",
        "Z14",
        "tabs.rs",
        "if crate::home::updater::UPDATER_UI_ENABLED {",
        "#[allow(non_snake_case)]\n    let UPDATER_UI_ENABLED = true;\n    if UPDATER_UI_ENABLED {",
    ),
    (
        '26. concat!["Remove ", "background"] with square brackets in panels/mod.rs',
        "Z08",
        "panels/mod.rs",
        "pub mod toolbar;",
        'pub mod toolbar;\npub const RETIRED_LABEL: &str = concat!["Remove ", "background"];',
    ),
    (
        '27. concat!{"ctx-", "crop"} with braces as a new toolbar entry',
        "Z08",
        "panels/toolbar.rs",
        '(Icon::Rectangle, "ctx-select-area", "Select area"),',
        '(Icon::Rectangle, "ctx-select-area", "Select area"),\n        (Icon::Rectangle, concat!{"ctx-", "crop"}, "Crop"),',
    ),
    (
        "28. bracketed concat! with a piece that cannot be folded in cmdk.rs",
        "Z08",
        "cmdk.rs",
        'title: "Boost resolution",',
        'title: concat!["Boost ", stringify!(resolution)],',
    ),
    (
        "29. braced concat! with a piece that cannot be folded in panels/toolbar.rs",
        "Z08",
        "panels/toolbar.rs",
        '(Icon::Rectangle, "ctx-select-area", "Select area"),',
        '(Icon::Rectangle, "ctx-select-area", "Select area"),\n        (Icon::Rectangle, concat!{"x-", stringify!(y)}, "Extra"),',
    ),
    (
        "30. zoom row label written as concat!{...} of allow-listed text",
        "Z13",
        "panels/menus.rs",
        ZOOM_IN_ROW,
        ZOOM_IN_ROW.replace('label: "Zoom in"', 'label: concat!{"Zoom ", "in"}'),
    ),
    (
        "31. concat !(...) with whitespace before the bang in cmdk.rs",
        "Z08",
        "cmdk.rs",
        'title: "Boost resolution",',
        'title: concat !("Boost ", stringify!(resolution)),',
    ),
    (
        "32. synthesize_spec_from_prompt called from terminal.rs",
        "Z15",
        "terminal.rs",
        "pub const MAKE_DESIGN_FIRST_DRAFT_PROMPT: &str =",
        "fn _fake_synth(p: &str) -> String { crate::cmdk::synthesize_spec_from_prompt(p) }\npub const MAKE_DESIGN_FIRST_DRAFT_PROMPT: &str =",
    ),
    (
        "33. canned 'Executed via' transcript literal back in terminal.rs",
        "Z15",
        "terminal.rs",
        "pub const MAKE_DESIGN_FIRST_DRAFT_PROMPT: &str =",
        'const _FAKE_EXEC: &str = "Status: Executed via claude CLI shell & streamed to Canvas";\npub const MAKE_DESIGN_FIRST_DRAFT_PROMPT: &str =',
    ),
    (
        "34. synthesize_spec_from_prompt exported as pub fn in cmdk.rs",
        "Z15",
        "cmdk.rs",
        "fn synthesize_spec_from_prompt(prompt: &str) -> NodeSpec {",
        "pub fn synthesize_spec_from_prompt(prompt: &str) -> NodeSpec {",
    ),
    (
        "35. fake agents-composer-mic control re-introduced in terminal.rs",
        "Z04",
        "terminal.rs",
        "pub const MAKE_DESIGN_FIRST_DRAFT_PROMPT: &str =",
        'const _FAKE_MIC: &str = "agents-composer-mic";\npub const MAKE_DESIGN_FIRST_DRAFT_PROMPT: &str =',
    ),
]


def verdicts(sources: "gate.SourceSet") -> Dict[str, Dict[str, object]]:
    return {r["id"]: r for r in gate.run_all_checks(sources)}


def main() -> int:
    base = gate.SourceSet.from_disk()
    failures: List[str] = []

    baseline = verdicts(base)
    bad = [cid for cid, r in baseline.items() if not r["passed"]]
    print(f"baseline (unmodified tree): {len(baseline) - len(bad)}/{len(baseline)} invariants pass")
    if bad:
        failures.append(f"baseline already failing: {bad}")

    print("-" * 100)
    for label, check_id, rel, anchor, replacement in REGRESSIONS:
        mutated = base.with_replacement(rel, anchor, replacement)
        result = verdicts(mutated)
        caught = not result[check_id]["passed"]
        note = ""
        if label.startswith("3."):
            # Old weak check: any mention of the constant in tabs.rs (here, the comment) counted as gated.
            naive_would_pass = "UPDATER_UI_ENABLED" in mutated.raw("tabs.rs")
            note = f" (substring check would still pass: {naive_would_pass})"
            if not naive_would_pass:
                failures.append(f"{label}: self-test premise broken, the comment no longer names the constant")
        print(f"{'CAUGHT' if caught else 'MISSED':<7}| gate {check_id} FAIL expected | {label}{note}")
        if caught:
            print(f"        evidence: {str(result[check_id]['detail'])[:170]}")
        else:
            failures.append(f"{label}: gate {check_id} still passes")

    print("-" * 100)
    controls: List[Tuple[str, str, str, str, str]] = [
        (
            "comment-only mention of every retired name is ignored",
            "Z08",
            "cmdk.rs",
            'title: "Boost resolution",',
            '// ctx-crop ctx-remove-bg Remove background\n        title: "Boost resolution",',
        ),
        (
            "test-only mention of a retired name is ignored",
            "Z08",
            "cmdk.rs",
            "#[cfg(test)]\nmod tests {",
            '#[cfg(test)]\nmod tests {\n    const RETIRED: &str = "ctx-crop";',
        ),
        (
            "raw string holding escape-looking text is not decoded into a retired name",
            "Z08",
            "panels/mod.rs",
            "pub mod toolbar;",
            'pub mod toolbar;\npub const NOTE: &str = r"ctx-\\u{63}rop is not decoded in a raw string";',
        ),
        (
            "escaped-backslash and raw u{_} text is left alone, not a crash",
            "Z08",
            "panels/mod.rs",
            "pub mod toolbar;",
            'pub mod toolbar;\npub const NOTE: &str = "\\\\u{_}";\npub const RAW_NOTE: &str = r"\\u{_}";',
        ),
        (
            "two separate literals in an array do not form a retired label",
            "Z08",
            "panels/mod.rs",
            "pub mod toolbar;",
            'pub mod toolbar;\npub const WORDS: [&str; 2] = ["Remove ", "background"];',
        ),
        (
            "a quote char literal does not desynchronise literal scanning",
            "Z08",
            "panels/mod.rs",
            "pub mod toolbar;",
            "pub mod toolbar;\npub const QUOTE: char = '\"';\npub const NEXT: &str = \"Boost resolution\";",
        ),
    ]
    for label, check_id, rel, anchor, replacement in controls:
        result = verdicts(base.with_replacement(rel, anchor, replacement))
        ok = bool(result[check_id]["passed"])
        print(f"{'OK' if ok else 'WRONG':<7}| gate {check_id} PASS expected | {label}")
        if not ok:
            failures.append(f"control failed: {label}: {result[check_id]['detail']}")

    print("-" * 100)
    if failures:
        print("SELF-TEST FAIL")
        for f in failures:
            print(f"  - {f}")
        return 1
    print(f"SELF-TEST PASS: {len(REGRESSIONS)}/{len(REGRESSIONS)} regressions caught, {len(controls)}/{len(controls)} controls clean")
    return 0


if __name__ == "__main__":
    sys.exit(main())
