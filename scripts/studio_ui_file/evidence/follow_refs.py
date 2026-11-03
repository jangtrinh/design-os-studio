"""The Rust a set's look follows (`follows` in crops.json), resolved to `path:line fn`, plus the kit-states.json row when the set follows a kit painter.

Pure text work (no Pillow), shared by the compare and its tests.
"""
import pathlib
import re

REPO = pathlib.Path(__file__).resolve().parents[3]
SRC = REPO / "crates/designos-app/src"
KIT_PAINTERS = "widgets/kit/"
KIT_STATES_ROW = "widgets/kit/testdata/kit-states.json row "
PINNED = re.compile(r"^v0\.7\.5 (\S+):(\d+) (\w+) \(removed by #(\d+)\)$")


def _pinned(ref):
    """A painter the app removed, which the 0.7.5 captures still show: `v0.7.5 <path>:<line> <fn> (removed by #<pr>)` prints as written, and only while the function is gone
    from the tree (a lookup in git would fail in a scratch copy without the tag, so the 0.7.5 line is literal text)."""
    m = PINNED.match(ref)
    if not m:
        raise SystemExit(f"crops.json follows: a pinned ref reads `v0.7.5 <path>:<line> <fn> (removed by #<pr>)`, got {ref!r}")
    path, line, fn, pr = m.groups()
    file = SRC / path
    if file.exists() and re.search(r"^.*\bfn\s+" + fn + r"\b", file.read_text(encoding="utf-8"), re.M):
        raise SystemExit(f"crops.json follows: {fn} is still in {path}: use the plain ref `{path}::{fn}`")
    return f"{path}:{line} {fn} (v0.7.5, removed by #{pr})"


def rust_refs(refs):
    """`path::fn` -> `path:line fn`, the line of the function in crates/designos-app/src; a missing function is fatal. A pinned ref (`_pinned`) names a function the tree no longer has."""
    out = []
    for ref in refs:
        if ref.startswith("v0.7.5 "):
            out.append(_pinned(ref))
            continue
        path, fn = ref.split("::")
        text = (SRC / path).read_text(encoding="utf-8")
        m = re.search(r"^.*\bfn\s+" + fn + r"\b", text, re.M)
        if not m:
            raise SystemExit(f"crops.json follows: no fn {fn} in {path}")
        out.append(f"{path}:{text[:m.start()].count(chr(10)) + 1} {fn}")
    return out


def follow_refs(name, follows, kit_rows):
    """`rust_refs(follows)` plus the kit-states.json row of `name`, when the set follows a `widgets/kit/` painter and the file has a row for it."""
    kit = any(f.startswith(KIT_PAINTERS) for f in follows)
    return rust_refs(follows) + ([KIT_STATES_ROW + name] if kit and name in kit_rows else [])
