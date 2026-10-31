# AC7: keyboard focus ring on the instance boolean toggle

Proof (test): `panels::components::props_panel_tests::instance_boolean_toggle_shows_focus_ring_on_tab`
(passes, see `test-run.txt`). It paints a focusable widget, then the instance boolean toggle, in one egui context.
Real Tab key events (no `request_focus` on the toggle) move focus to the toggle: exactly one 2px `FOCUS_RING` stroke
at `track.expand(SPACE_FACT)`, none before Tab. Shift+Tab returns focus and the ring disappears.

Live capture attempt (Xvfb, fixture A): the toggle cannot be reached with the keyboard in the running app.
Tab at canvas level is the shell shortcut (select next sibling) and Tab inside the panel walks only text fields
(`widgets/field_tab.rs`), so no key path lands on any `kit::toggle`. This is app-wide behaviour for every kit toggle,
not specific to this panel, and is not changed in this PR (PM backlog: keyboard reachability of kit toggles).
Frames: `live-tab-after-toggle-click.png`, `live-variant-open-close-tab.png`.
