"""The editor's sync, for a generated file: every node that mirrors a main node takes the main's variable bindings and style refs.

The REST import expands a nested instance from its main before any binding exists (bindings are the post-pass's job), so the copies hold the main's literal values
and none of its bindings or text style. A sync in the editor copies every field an instance does not override (`apply_reference`, designos-doc/src/components.rs:1069-1131,
`copy_variable_bindings_for_field`); this does the same for what the generator wrote, so a nested instance never carries a binding its main does not and never lacks one it does:

    counts, allow = mirror(doc, allow_entries)

* a node with `component.source` S gets S's `variables.bindings` keys and `style_refs` keys that it does not carry itself (what a node binds itself, a recoloured icon glyph for one, wins).
  A source that itself mirrors another node is settled first, so a main nested in a main passes its bindings on, however deep.
* an allow-list entry on S (an unbound literal the gate waives) is repeated on the mirror's path: the mirror holds the same literal. The entry list returned is the given one plus those.
  Paths are the gate's (page name and node names joined by `/`), pages 02 and 03 only, which is where nested instances live.
"""
import pathlib
import sys

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parents[1]))
from studio_ui_gate_doc import walk  # noqa: E402

PAGES = ("02", "03")


def _source(n):
    return (n.get("component") or {}).get("source")


def mirror(doc, allow=()):
    nodes, paths = doc["nodes"], {}
    for pid in doc["pages"]:
        page = nodes[str(pid)]
        if page["name"].startswith(PAGES):
            for child in page.get("children", []):
                for node, path in walk(doc, child, (page["name"],), {str(pid)}):
                    paths[node["id"]] = path
    done, counts = set(), {"bindings": 0, "style_refs": 0, "allow": 0}
    entries = {(a["path"], a["property"]) for a in allow}
    extra = []
    by_path = {}
    for a in allow:
        by_path.setdefault(a["path"], []).append(a)

    def settle(nid, open_=()):
        if nid in done:
            return
        n = nodes[str(nid)]
        src = _source(n)
        if src is not None and str(src) in nodes:
            if src in open_:
                raise ValueError(f"node {nid} ({n['name']}) mirrors itself through {src}")
            settle(src, open_ + (nid,))
            _copy(nodes[str(src)], n, counts)
            for a in list(by_path.get(paths.get(src), [])):
                key = (paths.get(nid), a["property"])
                if key[0] is not None and key not in entries:
                    entries.add(key)
                    entry = dict(a, path=key[0])
                    extra.append(entry)
                    by_path.setdefault(key[0], []).append(entry)
                    counts["allow"] += 1
        done.add(nid)

    for nid in sorted(int(i) for i, n in nodes.items() if _source(n) is not None):
        settle(nid)
    return counts, list(allow) + extra


def _copy(src, node, counts):
    have = node.setdefault("variables", {}).setdefault("bindings", {})
    node["variables"].setdefault("modes", {})
    for key, var in ((src.get("variables") or {}).get("bindings") or {}).items():
        if key not in have:
            have[key] = var
            counts["bindings"] += 1
    refs = src.get("style_refs") or {}
    if refs:
        mine = node.setdefault("style_refs", {})
        for key, ref in refs.items():
            if key not in mine:
                mine[key] = ref
                counts["style_refs"] += 1
    if not node["variables"]["bindings"] and not node["variables"]["modes"]:
        del node["variables"]
