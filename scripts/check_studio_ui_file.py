#!/usr/bin/env python3
"""Gate for design/studio-ui.designos, read straight from the native JSON (no MCP, no network, runs anywhere).

G1 parity   Semantic variables alias Primitives whose values equal theme.rs; S1/S2 structure; token half (INFO only).
G2 bound    pages 01-03: every fill, stroke, radius, gap, padding and effect colour is bound to a Semantic variable of the
            right kind and equal value. The allow-list (design/studio-ui.gate-allowlist.json) pins each waived literal (`value`),
            must match exactly the unbound properties found (a stale or duplicate entry fails: allowed == entries). A radius binds
            through the four corner keys only (0.7.5 ignores a uniform cornerRadius). A Focus/Ring instance rings its parent: it is
            the parent grown by SPACE_FACT on every side, each corner round(parent radius + SPACE_FACT) (kit/focus.rs:36-38), a literal
            (no radius binding) whose entry carries the value and no `host`.
            G2 stray: no binding of those families sits on a property the node does not set (a side with weight 0): the engine draws it at the variable's value.
            G2 also reads opacity: `State=Disabled` variants carry 0.5 on every visible paint, every other variant on 02 Core and 03 Studio 1.0,
            and no node inside a variant carries an opacity. A paint opacity other than those is waived only by an exact allow-list entry `fills/<i>/opacity`
            (value strictly between 0 and 1, 0.5 times it where Disabled is sticky; a stale, duplicate or valueless entry fails).
G2 overrides  on pages 02-03 an instance, and every node inside one, differs from its main only in non-style fields, placement and the
            listed exceptions (studio_ui_gate_overrides): Opacity, Blend, Fills, Stroke, Effects, CornerRadii, TextStyle, Layout and a style binding fail.
G3 registry every component set on 02 Core and 03 Studio is a design/component-registry.json record, on the page its category belongs to, with the registry's variant
            values; a record with no variants is one plain component.
G3 screens  every frame on page 04 Screens is a registry `screen` record holding instances only, plus at most one local rectangle, the canvas background (studio_ui_gate_screens).
G4 content  no kit, client or library name in any string or key of the file (static list, plus --deny).
Pair        the allow-list records the sha256 of this file (`designos_sha256`), so a half-published pair fails (studio_ui_gate_pair).
G5 size     a report, never a failure: the bytes each screen of 04 Screens adds to the file and the projection for the planned 74 screens (studio_ui_gate_size).

Exit 0 = all pass, 1 = a check failed, 2 = unusable input (empty, malformed or foreign file, broken node graph, nothing to check).
Usage: python3 scripts/check_studio_ui_file.py design/studio-ui.designos [--allowlist F] [--tokens F] [--theme F]
           [--token-map F] [--registry F] [--deny TERM]...
The unit tests (scripts/tests) use synthetic documents and a stub --token-map; this command on the committed file, with the
committed G01 and non-colour maps, is an advisory workflow step. Only the loading errors listed in `main` exit 2: any other
exception is a bug in the gate and propagates.
"""
from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path

SCRIPTS = Path(__file__).resolve().parent
sys.path.insert(0, str(SCRIPTS))
import studio_ui_theme_policy as policy  # noqa: E402
import studio_ui_theme_reader as theme  # noqa: E402
from studio_ui_gate_bound import CORNERS, GATED_PAGES, check_bound, node_props  # noqa: E402,F401
from studio_ui_gate_content import DENY, check_content  # noqa: E402
from studio_ui_gate_doc import GateInputError, load_doc  # noqa: E402
from studio_ui_gate_opacity import check_opacity  # noqa: E402
from studio_ui_gate_overrides import check_overrides  # noqa: E402
from studio_ui_gate_pair import check_pair  # noqa: E402
from studio_ui_gate_registry import check_registry  # noqa: E402
from studio_ui_gate_screens import check_screens  # noqa: E402
from studio_ui_gate_size import check_size, lines as size_lines  # noqa: E402
from studio_ui_gate_stray import check_stray  # noqa: E402
from studio_ui_gate_parity import check_parity, check_primitive_names, check_structure, check_tokens  # noqa: E402

REPO = SCRIPTS.parent


def load_token_map(path):
    """`{CONST: token key}` from a JSON file, or the committed G01 and non-colour maps when no file is given."""
    if not path:
        return policy.token_keys(SCRIPTS)
    keys = json.loads(Path(path).read_text(encoding="utf-8"))
    if not isinstance(keys, dict) or not all(isinstance(k, str) and isinstance(v, str) for k, v in keys.items()):
        raise GateInputError(f"{path} must be a JSON object mapping constant names to token keys")
    return keys


def main(argv=None) -> int:
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    ap.add_argument("file")
    ap.add_argument("--allowlist", default=str(REPO / policy.ALLOWLIST_REL))
    ap.add_argument("--tokens", default=str(REPO / "design/design.tokens.json"))
    ap.add_argument("--theme", default=str(REPO / theme.THEME_REL))
    ap.add_argument("--registry", default=str(REPO / "design/component-registry.json"))
    ap.add_argument("--token-map", help="JSON {CONST: token key}; default: the committed G01 and non-colour maps")
    ap.add_argument("--deny", action="append", default=[], help="extra deny term (repeatable)")
    a = ap.parse_args(argv)
    try:
        doc = load_doc(Path(a.file))
        rows = theme.flatten(theme.read_theme_constants(Path(a.theme).read_text(encoding="utf-8")))
        allow_doc = json.loads(Path(a.allowlist).read_text(encoding="utf-8"))
        allow = allow_doc.get("allow", [])
        tokens = json.loads(Path(a.tokens).read_text(encoding="utf-8"))
        token_map = load_token_map(a.token_map)
        f1, c1 = check_parity(doc, rows)
        f1 = check_structure(doc) + check_primitive_names(doc) + f1
        f_tok, infos = check_tokens(doc, rows, tokens, token_map)
        f2, c2 = check_bound(doc, allow)
        f2s, c2s = check_stray(doc)
        f2o, c2o = check_opacity(doc, allow)
        f2v, c2v = check_overrides(doc)
        registry = json.loads(Path(a.registry).read_text(encoding="utf-8"))
        f3, n3 = check_registry(doc, registry)
        f3s, c3s = check_screens(doc, registry)
        f4, c4 = check_content(doc, list(DENY) + a.deny)
        f5, sha = check_pair(Path(a.file), allow_doc)
        size = check_size(doc, Path(a.file).stat().st_size)
    except (GateInputError, theme.ThemeError, OSError, json.JSONDecodeError, ImportError) as e:
        print(f"ERROR: {e}", file=sys.stderr)
        return 2
    print(f"G1 parity : {c1['semantic']} Semantic variables vs {c1['drawable_constants']} drawable theme.rs constants, "
          f"{len(f1) + len(f_tok)} failures ({len(f_tok)} in the token half)")
    for page, c in c2.items():
        print(f"G2 bound  : {page}: {c['nodes']} nodes, {c['properties']} properties, {c['bound']} bound, "
              f"{c['allowed']} allowed ({c['entries']} allow-list entries), {c['unbound']} unbound")
    print(f"G2 stray  : {c2s} bindings on pages 01-03, {len(f2s)} on a property the node does not set")
    print(f"G2 opacity: {c2o['variants']} variants ({c2o['disabled']} Disabled), {c2o['paints']} paints ({c2o['listed']} with an allow-listed opacity), {len(f2o)} failures")
    print(f"G2 overrides: {c2v['instances']} instances, {c2v['nodes']} mirrored nodes vs their mains, {len(f2v)} failures")
    print(f"G3 registry: {n3} component sets and plain components on 02 Core and 03 Studio, {len(f3)} failures")
    print(f"G3 screens: {c3s['screens']} screens on 04 Screens, {c3s['instances']} instances, {c3s['copies']} copies bound like their mains, "
          f"{c3s['backgrounds']} background rectangles, {len(f3s)} failures")
    print(f"G4 content: {c4['strings']} strings and keys vs {c4['deny_terms']} deny terms, {len(f4)} hits")
    print(f"Pair      : the file (sha256 {sha[:12]}...) and its allow-list, {len(f5)} failures")
    print("\n".join(size_lines(size)))
    for line in infos:
        print(line)
    fails = f1 + f_tok + f2 + f2s + f2o + f2v + f3 + f3s + f4 + f5
    for line in fails[:40]:
        print("FAIL", line)
    if len(fails) > 40:
        print(f"... and {len(fails) - 40} more")
    print("PASS" if not fails else f"FAIL ({len(fails)})")
    return 1 if fails else 0


if __name__ == "__main__":
    sys.exit(main())
