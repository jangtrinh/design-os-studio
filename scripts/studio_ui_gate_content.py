"""G4: no kit, client or library name anywhere in the saved file.

Every string value and every key of the whole JSON is matched case-insensitively against the deny list, so layer, page,
variable and style names, descriptions, text, component keys, source ids, every code-syntax platform and property
defaults are all covered. The list is static and the check never reads the machine (no library manifest): pass extra
local terms with `--deny`. Hex image payloads cannot contain any of the terms.

The first four are the names of the kit and of the clients; the rest are strings only the Untitled kit uses (its page, frame and style
names: `_Primitives`, `Color modes`, `Display 2xl`, `Dashboards 01`, `Swatch base`, the logo and mockup frames, the `❖` page marker), so the
screens built from its captures cannot carry them over unseen. They are not generic words: `Desktop` is Studio's frame preset and the
Tree/Row default name, `Base` is a ramp name, so neither is listed.

Checked against Studio's own source (`crates/designos-app`, `crates/designos-library`, 2026-11-01): none of the ten is a string the editor chrome draws. What does hold one: the singular
`Dashboard` is the frame name of Studio's demo documents (the list has the plural `dashboards`), `_Primitives` is a fixture collection name of the token-export tests, and the page marker
sits in the labels of the micro-interaction demo overlay (`shell.rs`), a marketing sequence no screen draws.
"""
from __future__ import annotations

from typing import Iterator, Tuple

from studio_ui_gate_doc import GateInputError

DENY = ("untitled", "vsf", "pcp", "olivia", "_primitives", "color modes", "display 2xl", "dashboards", "swatch base", "logomark", "press logo", "company logo",
        "screen mockups", "\u2756")


def strings(obj, where: str = "$") -> Iterator[Tuple[str, str]]:
    """(location, string) for every key and every string value."""
    if isinstance(obj, dict):
        for k, v in obj.items():
            yield f"{where}.{k}", str(k)
            yield from strings(v, f"{where}.{k}")
    elif isinstance(obj, list):
        for i, v in enumerate(obj):
            yield from strings(v, f"{where}[{i}]")
    elif isinstance(obj, str):
        yield where, obj


def check_content(doc: dict, deny: list) -> tuple[list, dict]:
    deny = [d.lower() for d in deny]
    hits, scanned = [], 0
    for where, s in strings(doc):
        scanned += 1
        low = s.lower()
        hits += [f"`{d}` in {where}: {s[:60]!r}" for d in deny if d in low]
    if scanned == 0:
        raise GateInputError("G4 found no strings to scan")
    return hits, {"strings": scanned, "deny_terms": len(deny)}
