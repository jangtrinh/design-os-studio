"""The fill exceptions of the override gate (studio_ui_gate_overrides): paint 0's colour, nothing else of the Fills field.

An icon glyph's swapped or recoloured fill and the content colour of a Swatch/Color layer are the only fills an instance may own. What they own is the first paint's solid
colour, and its opacity (a Disabled button dims the glyph to 0.5; G2 opacity owns that value), plus the binding `fills/0`. The paint count, paint 0's visibility and blend and
every other paint stay the main's, as does every other fill binding; a first paint that is not solid in both is compared whole.
"""
from __future__ import annotations

from typing import List, Tuple

FREE = ("kind", "opacity")  # the solid colour and the dimming of paint 0


def _solid(paint: dict) -> bool:
    kind = paint.get("kind")
    return isinstance(kind, dict) and "Solid" in kind


def _blank(paint: dict) -> dict:
    return {k: v for k, v in paint.items() if k not in FREE}


def without_first_colour(mine: List[dict], theirs: List[dict]) -> Tuple[List[dict], List[dict]]:
    """The two fill lists with paint 0's solid colour and opacity blanked, so comparing them leaves the paint count, paint 0's other fields and the other paints."""
    if mine and theirs and _solid(mine[0]) and _solid(theirs[0]):
        return [_blank(mine[0]), *mine[1:]], [_blank(theirs[0]), *theirs[1:]]
    return mine, theirs
