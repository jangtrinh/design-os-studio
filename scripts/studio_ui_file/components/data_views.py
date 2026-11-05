"""Code/Block of page `03 Studio` (row `Data & Code`). Table/Data is in `data_table.py`. The record has no registry variants, so it is a plain COMPONENT (rule (j)).

Code/Block
* Painter: `code_block` (dev/code_view.rs:124-170). Callers: 1 in production, `dev/sections.rs:473` (plus a test, `shell/tests/dev_code_panel.rs:68`; the Inspect panel's code view for one language; `prompt_box`, :172-189, is a second painter, a framed prose
  box with a `BORDER_STRONG` stroke, not drawn). No capture shows the block's pixels: in every dev capture (`micro-dev-boxmodel`, `g-dev-*`) the block's top is at y 1067 or below on a 900 px
  image, so the whole set is UNVERIFIED-VISUAL. `micro-dev-boxmodel` gives the geometry from layout.json: the block (1215, 1329, 209, 272) = 14 rows * 18 + 2 * 8 + 4, and its thumb
  (1223, 1593, 80.75, 4) = 8 from the block's left edge and 8 above its bottom; the sample below follows the same rules at 8 rows.
* Nested sets: none (the painter draws no kit widget). The registry names no composition.
* Local parts: all of it. A rect filled `BG_SECONDARY` at `RADIUS_CONTROL` with no stroke; the rows are `CODE_LINE_HEIGHT` (18) high and start `INSET_CONTROL` in from the top and both sides; each
  row is one text per coloured span in `font_mono` (`Mono / Value`: Roboto Mono, `FONT_SIZE_BODY`), coloured by `highlight::color(tok)` (dev/highlight.rs:34-45): Plain, Property, Keyword, String, Number,
  Type, Function, Comment and Punct are the `SYNTAX_*` variables of the same names; the view clips the code at the insets. When a line is wider than the view the box keeps one more
  `SPACE_FACT` step, and a thumb (`SPACE_FACT` high, `RADIUS_MARK`, `TEXT_TERTIARY`, `TEXT_SECONDARY` while hovered) shows how much of the code is visible: `view_w ** 2 / content_w` wide, at least
  `TARGET_MIN`, `SPACE_FACT` under the last row. In flow that is the box's `SPACE_FACT` gap, the thumb, and `SPACE_FACT` of bottom padding, so no literal is needed.
* The code is content: the rows are the `Code` Slot (rule (m)), and the default is a neutral CSS sample (`SAMPLE`). A span's token was assigned by hand with the tokenizer's rules (`highlight::line`):
  an ident before `:` is a Property, before `(` a Function, a `.` makes the next ident Plain, `#hex` and digits with their unit are Numbers, `{}()[],;:.` are Punct; a leading space is the next span's, so no
  text node ends in a space. Two adjacent runs of one colour are one text.
* Gaps: a block whose lines all fit (no thumb, no strip: the thumb is always drawn here), the scroll offset (the thumb stays at the left edge), the hovered thumb colour, a code that has no
  rows (`No code`, sections.rs:476-479, a plain `Body` line), and a Slot filled with more rows than the sample (the thumb's width follows the sample).
"""
from rest_ir import F, T

CODE_W = 209  # `c.w - 2 * PAD` in the Inspect panel (layout.json `dev-code-block` width)
MONO_ADVANCE = 0.6  # the monospace face's advance in em: Roboto Mono's `0.6 * FONT_SIZE_BODY`, what `mono_char_w` (code_view.rs:24-26) measures
TOKEN = {"plain": "SYNTAX_PLAIN", "property": "SYNTAX_PROPERTY", "keyword": "SYNTAX_KEYWORD", "string": "SYNTAX_STRING", "number": "SYNTAX_NUMBER", "type": "SYNTAX_TYPE",
         "function": "SYNTAX_FUNCTION", "comment": "SYNTAX_COMMENT", "punct": "SYNTAX_PUNCT"}
SAMPLE = [  # (token, text): the tokenizer's spans of a neutral CSS rule, adjacent runs of one colour merged, a leading space kept with the next span
    [("punct", "."), ("plain", "card"), ("punct", " {")],
    [("property", "  display"), ("punct", ":"), ("plain", " flex"), ("punct", ";")],
    [("property", "  gap"), ("punct", ":"), ("number", " 8px"), ("punct", ";")],
    [("property", "  padding"), ("punct", ":"), ("number", " 16px"), ("number", " 24px"), ("punct", ";")],
    [("property", "  background"), ("punct", ":"), ("number", " #171717"), ("punct", ";")],
    [("property", "  border-radius"), ("punct", ":"), ("number", " 12px"), ("punct", ";")],
    [("property", "  box-shadow"), ("punct", ":"), ("number", " 0"), ("number", " 1px"), ("number", " 2px"), ("function", " rgba"), ("punct", "("), ("number", "0"), ("punct", ","),
     ("number", " 0"), ("punct", ","), ("number", " 0"), ("punct", ","), ("number", " 0.4"), ("punct", ");")],
    [("punct", "}")],
]


def code_block(values, env):
    inset, fact, line_h = env.c("INSET_CONTROL"), env.c("SPACE_FACT"), env.num("CODE_LINE_HEIGHT")
    lines = [F(f"Line {i}", [T("Span", text, "Mono / Value", env.c(TOKEN[tok])) for tok, text in row], dir="H", align=("start", "center"), h=line_h, sx="hug", sy="fixed")
             for i, row in enumerate(SAMPLE, 1)]
    view = F("Code", lines, dir="V", align=("start", "start"), clip=True, slot="Code", sx="fill", sy="hug")
    view_w = CODE_W - 2 * env.num("INSET_CONTROL")
    widest = max(sum(len(text) for _, text in row) for row in SAMPLE)
    thumb_w = min(max(view_w * view_w / (widest * MONO_ADVANCE * env.num("FONT_SIZE_BODY")), env.num("TARGET_MIN")), view_w)
    thumb = F("Scroll thumb", fill=env.c("TEXT_TERTIARY"), radius=env.c("RADIUS_MARK"), w=round(thumb_w, 2), h=env.num("SPACE_FACT"), sx="fixed", sy="fixed")
    return F("Code block", [view, thumb], dir="V", gap=fact, align=("start", "start"), pad=[inset, inset, fact, inset], fill=env.c("BG_SECONDARY"), radius=env.c("RADIUS_CONTROL"),
             w=CODE_W, sx="fixed", sy="hug")


def component_sets():
    return [{"name": "Code/Block", "row": "Data & Code", "axes": {}, "build": code_block, "props": [{"name": "Code", "type": "SLOT"}]}]
