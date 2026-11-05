"""How a layers row shares its text column between the name and the type label, as production does (panels/layers.rs at v0.7.5: `split_row_text` :235-241, the row text :646-668).

Where the column is too narrow for the label plus a readable name (`text_right - text_left < label_w + MIN_NAME_WIDTH_WITH_LABEL`, :223, :236 with `label_w` the label's width plus `TYPE_LABEL_PAD`,
:221, :647) the label is not drawn and the name keeps the whole column, up to `TEXT_RIGHT`; where it is drawn the name keeps the column up to the label and its padding. The name is elided with `…`
to exactly that room (:653 `widgets::elide(.., name_right - text_left)`, at least `MIN_NAME_ROOM`). Tree/Row draws the name from `NAME_LEFT` (the chevron box 24 + the glyph slot 22) plus the depth
indent, and the label right-aligned to `LABEL_RIGHT` (the 48 px of toggles and the label's 2 px padding left of the row's right edge).

`ADV` is the advance of each printable ASCII character and of `…` in Inter Regular at 12 px (the Body and Caption styles), measured once with Pillow on crates/designos-fonts/assets/inter; a text's
width is their sum. Pure data and arithmetic: no import.
"""
CHARS = "".join(chr(c) for c in range(32, 127)) + "…"
_ADV = (
    "3.38 3.45 5.59 7.59 7.7 11.78 7.73 3.59 4.38 4.38 6.02 7.94 3.45 5.52 3.45 4.33 7.58 4.88 7.31 7.41 7.75 7.12 7.44 6.8 7.42 7.44 3.45 3.62 7.94 7.94 7.94 6.14 11.59 8.28 7.86 8.77 "
    "8.66 7.22 7.08 8.95 8.92 3.22 6.84 8.06 6.78 10.84 9.05 9.17 7.67 9.17 7.72 7.7 7.75 8.94 8.28 11.83 8.19 8.14 7.55 4.38 4.33 4.38 5.66 5.47 3.88 6.73 7.34 6.86 7.34 7 4.44 7.36 "
    "7.09 2.91 2.91 6.59 2.91 10.52 7.09 7.2 7.34 7.34 4.52 6.33 3.92 7.09 6.75 9.83 6.55 6.75 6.62 5.11 3.98 5.11 7.94 10.38"
)
ADV = dict(zip(CHARS, (float(v) for v in _ADV.split())))
NAME_LEFT, TEXT_RIGHT, LABEL_RIGHT = 46.0, 176.0, 174.0  # from the row's left edge; the indent adds `LAYER_INDENT` (16) per depth
TYPE_LABEL_PAD, MIN_NAME_WIDTH_WITH_LABEL, MIN_NAME_ROOM, INDENT = 8.0, 16.0, 10.0, 16.0


def width(text):
    return sum(ADV[c] for c in text)


def name_left(depth):
    return NAME_LEFT + INDENT * depth


def shows_label(label, depth):
    """`split_row_text`: the label is drawn when the column holds it plus the minimum name."""
    return bool(label) and TEXT_RIGHT - name_left(depth) >= width(label) + TYPE_LABEL_PAD + MIN_NAME_WIDTH_WITH_LABEL


def name_right(label, depth):
    """`split_row_text`'s `name_right`: before the label (and its padding) when it is drawn, else the right edge of the whole text column."""
    return TEXT_RIGHT - width(label) - TYPE_LABEL_PAD if shows_label(label, depth) else TEXT_RIGHT


def elide(text, room):
    """`widgets::elide`: the text, or its longest prefix with `…` that fits `room`."""
    if width(text) <= room:
        return text
    cut = text
    while cut and width(cut.rstrip() + "…") > room:
        cut = cut[:-1]
    return cut.rstrip() + "…"


def fit(name, label, depth):
    """(name, label) as production draws the row: the label blanked where it does not fit, the name elided to the room left."""
    shown = label if shows_label(label, depth) else ""
    return elide(name, max(name_right(shown, depth) - name_left(depth), MIN_NAME_ROOM)), shown
