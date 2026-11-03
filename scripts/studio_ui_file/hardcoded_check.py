"""The MCP `has-hardcoded` rows on pages `02 Core` and `03 Studio` equal the gate's allow-list, node for node.

    problems = check(mcp, native_file, allow_entries)    [] when every MCP row is allow-listed and every allow-listed node is a row

The plan's G2 once said these queries return 0. They return one row per node that holds a literal, and the literals are allow-listed
(`design/studio-ui.gate-allowlist.json`), so the rows must be exactly those nodes: a new literal, a removed one or a stale entry
shows here as well as in the file gate. MCP flags only nodes that own the literal: a component instance inherits it from its main, so
allow-list entries on instances are not expected as rows. The exception is a Focus/Ring instance whose radius differs from its main's:
it owns the corner literal, and MCP reports a row (probed on a `02 Core` copy). That instance is read from the file (its stored radius
against the main's), never from the allow-list: an entry names no host. The four corner keys are one query (`cornerRadius`).

MCP's `has-hardcoded` is the linter's `hardcoded-value` finding, and for a number it fires only when some Number variable holds exactly that
literal (designos-doc/src/lint/token_match.rs `find_number`: a literal is hard-coded when a variable could replace it). A literal no variable
holds, such as a toggle track's 0.5 px border (`W * 0.5`), is an allow-list entry that MCP never reports: `held_numbers` lists the values the
file's Number variables hold (any collection), and only an entry with such a value is expected as a row. The variable's scopes are not
modelled: an entry whose value is held but scoped away shows as "allow-listed but MCP reports no literal there", which is the loud direction.

`has-hardcoded('cornerRadius')` reports a node only when its four radii are equal (probed on designos-mcp 0.7.5: [11, 11, 11, 11] is a row, [11, 11, 0, 0], [11, 0, 0, 0], [0, 0, 11, 11] and
[11, 11, 11, 10] are none, with a Number variable holding 11): a per-corner radius with a literal corner is an allow-list entry the file gate guards and MCP never reports.

The linter never checks a node that is not itself visible, and nothing more (probed on designos-mcp 0.7.5: a visible child and a grandchild below a hidden grandparent are reported, the hidden node itself is not):
`has-hardcoded` goes `query/mod.rs:522-525` -> `SingleNodeLinter::lint`, which returns only on that node's own `!n.visible` (designos-doc/src/lint/traverse.rs:382); only
the full-tree walk skips a hidden subtree (traverse.rs:262), and the query does not use it. So a hidden node is no row, held literal or not, but a held literal on a
VISIBLE child (or grandchild) of a hidden parent IS a row. `paths_by_id` records each node's own `visible` and `expected` skips exactly the hidden ones: their entries are
allow-list-only, guarded by the file gate like an unheld literal.
"""
import json
import pathlib
import sys

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parents[1]))
from studio_ui_gate_doc import same, variable_value, walk  # noqa: E402
from studio_ui_gate_ring import main_radius, ring_mains  # noqa: E402

QUERIES = {"cornerRadius": ("topLeftRadius", "topRightRadius", "bottomRightRadius", "bottomLeftRadius"), "itemSpacing": ("itemSpacing",),
           "counterAxisSpacing": ("counterAxisSpacing",), "paddingTop": ("paddingTop",), "paddingRight": ("paddingRight",),
           "paddingBottom": ("paddingBottom",), "paddingLeft": ("paddingLeft",), "strokeWeight": ("strokeWeight",)}
PAGE = "02 Core"
PAGES = ("02 Core", "03 Studio")  # a page the file lacks is skipped; an allow-list entry under another page is never expected


def pins_own_radius(node, mains):
    """True for a Focus/Ring instance with a corner radius that differs from its main's: it owns that literal."""
    main = main_radius(node, mains)
    return main is not None and any(abs(float(r) - main) > 1e-3 for r in (node.get("corner_radii") or []))


def held_numbers(doc):
    """The numbers the file's Number variables hold (default mode, aliases followed): the literals MCP can call hard-coded."""
    out = set()
    for var in (doc.get("variables") or {}).values():
        if var.get("kind") == "Number":
            value = variable_value(doc, var)
            if isinstance(value, (int, float)) and not isinstance(value, bool):
                out.add(float(value))
    return out


def paths_by_id(doc, page_name=PAGE):
    """(page id, {node id: (path, is an instance, pins its own ring radius, is visible, its four corner radii are equal, mirrors a main node)}) of page `page_name`; a node without a
    `visible` key is visible. A node that mirrors a main node (`component.source`: a copy inside an instance) holds the main's literal without owning it."""
    page = next(doc["nodes"][str(p)] for p in doc["pages"] if doc["nodes"][str(p)]["name"] == page_name)
    mains, out = ring_mains(doc), {}
    for child in page.get("children", []):
        for node, path in walk(doc, child, (page_name,), {str(page["id"])}):
            out[node["id"]] = (path, isinstance(node["kind"], dict) and "Instance" in node["kind"], pins_own_radius(node, mains), node.get("visible", True) is not False,
                               len(set(node.get("corner_radii") or [])) <= 1,
                               (node.get("component") or {}).get("source") is not None)
    return page["id"], out


def expected(allow, by_id, held=None):
    """{query key: set of node paths} for the allow-list entries on nodes that own their literal: not instances, except the corner radius
    of a Focus/Ring instance that pins its own (read from the file), and not a copy inside an instance (`mirror`: it inherits the literal).
    With `held` (see held_numbers), an entry whose value no Number variable
    holds is skipped: MCP cannot flag it. `held=None` (a file with no variable table) expects every entry. A hidden node is never a row."""
    owners = {path for path, inst, _, visible, _, mirror in by_id.values() if not inst and visible and not mirror}
    pinned = {path for path, _, pin, visible, _, _ in by_id.values() if pin and visible}
    blind = {path for path, _, _, _, uniform, _ in by_id.values() if not uniform}  # `cornerRadius` sees only four equal radii
    out = {q: set() for q in QUERIES}
    for a in allow:
        if held is not None and not any(same(a["value"], h) for h in held):
            continue
        for q, props in QUERIES.items():
            if q == "cornerRadius" and a["path"] in blind:
                continue
            if a["property"] in props and (a["path"] in owners or (q == "cornerRadius" and a["path"] in pinned)):
                out[q].add(a["path"])
    return out


def compare(want, rows):
    """Mismatch lines between {key: expected paths} and {key: rows' paths}."""
    out = []
    for q in QUERIES:
        out += [f"has-hardcoded('{q}'): {p} is allow-listed but MCP reports no literal there" for p in sorted(want[q] - rows[q])]
        out += [f"has-hardcoded('{q}'): MCP reports a literal on {p} that the allow-list does not name" for p in sorted(rows[q] - want[q])]
    return out


def check(mcp, native_file, allow):
    doc = json.loads(pathlib.Path(native_file).read_text(encoding="utf-8"))
    held = held_numbers(doc) if doc.get("variables") else None
    problems = []
    for name in PAGES:
        if not any(doc["nodes"][str(p)]["name"] == name for p in doc["pages"]):
            continue
        page_id, by_id = paths_by_id(doc, name)
        rows = {}
        for q in QUERIES:
            r = mcp.tool_or_die("designos_query_xpath", {"selector": f"//*[has-hardcoded('{q}')]", "page_id": page_id, "limit": 500})
            if not r.get("done", True):
                return [f"has-hardcoded('{q}'): more than 500 rows, the check cannot compare them"]
            rows[q] = {by_id[row[r["columns"].index("id")] if isinstance(row, list) else row["id"]][0] for row in r.get("rows", [])}
        problems += compare(expected(allow, by_id, held), rows)
    return problems
