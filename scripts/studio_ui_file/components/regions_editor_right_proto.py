"""The Prototype state's content (`panels/prototype.rs` `selection_sections`, :224; lines are the 0.7.5 tag): the Interactions title with Plus, the `Show prototype settings` button (184 x 28,
the row advances 34, :54-65), one interaction row and the Scroll behavior title with its two dropdowns.

* The row (`reaction_row` :415-452): 224 x 44 at x 8, a 12 px Play glyph in PROTO_BLUE centred at +16, the trigger line (Body Semibold, TEXT) centred 14 down from x + 32 and the action
line (Body, TEXT_SECONDARY) 30 down,
  and the delete minus 28 from the right edge, centred; the row is the open one (the capture is taken with its editor shown), filled BG_SELECTED at `RADIUS_CONTROL` (:421-423). It advances
  46, the block ends with 4 and
  a hairline (:281-284).
* Scroll behavior (:299-346): the title, `Overflow` and the dropdown (the full 184, the group 8 below), `Position` and its dropdown; the second label adds 8 over the 8 the dropdown
advanced (`sublabel`, properties.rs:301-303),
  so the groups are `SPACE_BLOCK` apart.
The trigger text is neutral ("On click", "Scroll to Frame 2").
"""
from components.regions_editor_right_parts import button, group, icon_button, row, rule, section, select, title
from rest_ir import F, I, T

SEC_W = 239.0


def interactions_block(env):
    button_ = button(env, "Settings button", "Show prototype settings", env.num("SECTION_CONTENT_X"), 0.0, env.num("SECTION_CONTENT_W"), 28.0, style="Body / Semibold", tone="TEXT_WHITE")
    settings = F("Settings row", [button_], w=SEC_W, h=34.0, sx="fixed", sy="fixed")
    kids = [I("Trigger glyph", "Play", 12, env.c("PROTO_BLUE"), abs=(10.0, 16.0, 12, 12)), T("Trigger", "On click", "Body / Semibold", env.c("TEXT"), abs=(32.0, 5.0, 120.0, 18.0)),
            T("Action", "Scroll to Frame 2", "Body / Regular", env.c("TEXT_SECONDARY"), abs=(32.0, 21.0, 120.0, 18.0)), icon_button(env, "Delete", "Minus", 196.0, 10.0)]
    card = F("Interaction row", kids, fill=env.c("BG_SELECTED"), radius=env.c("RADIUS_CONTROL"), w=env.num("RIGHT_PANEL_WIDTH") - 16.0, h=env.num("PROTO_ROW_HEIGHT"), sx="fixed", sy="fixed",
             abs=(env.num("PAGE_ROW_INSET"), 0.0, env.num("RIGHT_PANEL_WIDTH") - 16.0, env.num("PROTO_ROW_HEIGHT")))
    block = F("Interaction row frame", [card], w=SEC_W, h=env.num("PROTO_ROW_HEIGHT") + 6.0, sx="fixed", sy="fixed")  # the row advances 46 and the block ends with 4
    return section(env, "Interactions", title(env, "Interactions", "Interactions", plus=True), [settings, block], bottom=None, gap=None)


def scroll_section(env):
    c = env.num("SECTION_CONTENT_W")
    overflow, position = select(env, "Overflow dropdown", 0.0, "No scrolling"), select(env, "Position dropdown", 0.0, "Scroll with parent")
    for s in (overflow, position):
        s.update(abs=(0.0, 0.0, c, env.num("FIELD_HEIGHT")), w=c)
    groups = [group(env, "Overflow", "Overflow", row(env, "Overflow row", [overflow])), group(env, "Position", "Position", row(env, "Position row", [position]))]
    return section(env, "Scroll behavior", title(env, "Scroll behavior", "Scroll behavior"), groups, gap="SPACE_BLOCK")


def prototype_content(env):
    return [interactions_block(env), rule(env, "Rule above Scroll behavior"), scroll_section(env)]
