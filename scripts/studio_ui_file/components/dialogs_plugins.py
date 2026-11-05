"""The default Body of Dialog/Modal Kind=Plugins: the plugin library's empty state, drawn as `plugins/ui.rs` at v0.7.5 draws it (the capture `j-plugins`). Not a component module.

Painter `show` (plugins/ui.rs:17-227), `installed.is_empty()` arm (:111-116), inside the body ScrollArea (:57-62: 72 under the modal's top, `PAD` in, item spacing 12 :62): a 60 px row whose
centre holds `Icon::Tools` 28 px in `ICON_SECONDARY` (:112-113), then three `paragraph`s (:114-116, `font_body`, left-aligned, wrapping at the 512 px body): the title `Your plugin library
starts here` in TEXT and two TEXT_TERTIARY notes, 12 apart. The paragraphs are the install form's own helper (`dialogs_install.paragraph`), so the two bodies share one text builder.
This body follows the 0.7.5 painter, not the Home `EmptyState/Default` (a centred tile and 16 px title the capture does not draw).
Rects are in the Body slot's own coordinates (its top is the modal's y 72); a caller's content in the slot sits inside the modal's `INSET_MODAL` side padding.
Gaps: the installed-plugin cards (:117-166), the error line (:168-170).
"""
from components.dialogs_install import paragraph
from components.regions_sidepanels_parts import glyph

TITLE = "Your plugin library starts here"
KEEP_NOTE = "Install a plugin from jangtrinh/design-os-plugins to keep its package on this device."
UNDO_NOTE = "Browser plugins can insert SVG icons into the open document. Each insertion supports Undo."
# modal y of the first paragraph's 18 px line box: the painter's 15 px row starts at 72 + 60 + 12 = 144 and the capture's ink sits 1 px higher; each next one 27 lower
TITLE_Y, NOTE_PITCH = 143.0, 27.0


def body(env):
    """The Body slot's default children: the glyph box (centre x 280, 30 under the body's top) and the three paragraphs."""
    return [glyph("Icon", "Tools", 28, "ICON_SECONDARY", env, (266.0, 16.0, 28.0, 28.0)), paragraph(env, "Title", TITLE_Y, TITLE, "TEXT"),
            paragraph(env, "Keep note", TITLE_Y + NOTE_PITCH, KEEP_NOTE, "TEXT_TERTIARY"), paragraph(env, "Undo note", TITLE_Y + 2 * NOTE_PITCH, UNDO_NOTE, "TEXT_TERTIARY")]
