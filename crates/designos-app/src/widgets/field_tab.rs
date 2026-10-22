//! Keyboard focus between the text fields of the panels.
//!
//! Tab and Shift+Tab walk the fields in the order they are drawn. egui's own Tab walks every
//! focusable widget, and every icon button and swatch is one, so from a field it would stop on the
//! next button. Each field registers here while it draws ([`register`]); while one has focus,
//! [`take_tab_press`] keeps the Tab press away from egui's walk and [`start_frame`] moves the focus
//! to the neighbouring field instead.
//!
//! The same registry tells the shortcut handler that this frame's click landed on a field
//! ([`press_lands_on_field`]), before the panel has drawn and given that field the focus.

use egui::{Context, Event, Id, Key, LayerId, Modifiers, PointerButton, RawInput, Rect, Ui};

/// A text field that drew in some frame, and where: the layer and the clip rect of the `Ui` it drew in.
/// Tab never leaves that scope. A dialog's fields do not lead to the panel behind it, and a panel
/// draws its fields in the region it clips to (`panels::clipped`), so the fields of the Design panel
/// are not reached from a rename in the page list.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Stop {
    id: Id,
    layer: LayerId,
    clip: [i32; 4],
}

#[derive(Clone, Default)]
struct Stops {
    /// The fields drawn so far in the current frame, in drawing order.
    drawn: Vec<Stop>,
    /// The fields of the previous frame: what the user saw when they pressed the key.
    last: Vec<Stop>,
}

/// A Tab press waiting for [`start_frame`]: `true` for Shift+Tab.
#[derive(Clone, Copy, Default)]
struct TabPress {
    backward: bool,
}

fn stops_key() -> Id {
    Id::new("field-tab-stops")
}

fn press_key() -> Id {
    Id::new("field-tab-press")
}

/// Notes that the text field `id` drew `rect` this frame. A field that is disabled or scrolled out
/// of view is not a stop.
pub(super) fn register(ui: &Ui, rect: Rect, id: Id) {
    if !ui.is_enabled() || !ui.is_rect_visible(rect) {
        return;
    }
    let clip = ui.clip_rect();
    let stop = Stop { id, layer: ui.layer_id(), clip: [clip.min.x, clip.min.y, clip.max.x, clip.max.y].map(|v| v.round() as i32) };
    ui.ctx().data_mut(|d| {
        let stops = d.get_temp_mut_or_default::<Stops>(stops_key());
        if !stops.drawn.iter().any(|s| s.id == id) {
            stops.drawn.push(stop);
        }
    });
}

/// True when this frame's primary press (or click) landed on the field `id`, as far as egui's hit
/// test of the previous frame's widgets can tell. The field has not drawn yet, so it has no focus.
pub(super) fn is_pressed(ctx: &Context, id: Id) -> bool {
    ctx.input(|i| i.pointer.primary_pressed()) && ctx.read_response(id).is_some_and(|r| r.clicked_by(PointerButton::Primary) || r.is_pointer_button_down_on())
}

/// True when this frame's click landed on a text field drawn in the previous frame. egui gives that
/// field the focus only while the panel draws, after the shortcuts have run, so a key that arrives
/// in the same frame as the click is the field's, not a shortcut's.
pub fn press_lands_on_field(ctx: &Context) -> bool {
    if !ctx.input(|i| i.pointer.primary_pressed()) {
        return false;
    }
    let last = ctx.data(|d| d.get_temp::<Stops>(stops_key())).map(|s| s.last).unwrap_or_default();
    last.iter().any(|s| is_pressed(ctx, s.id))
}

/// The field after (or, with `backward`, before) `focused`, wrapping around, among the fields drawn
/// in the same scope. `None` when `focused` is not a field or has no neighbour.
fn neighbour(stops: &[Stop], focused: Id, backward: bool) -> Option<Id> {
    let home = stops.iter().find(|s| s.id == focused)?;
    let ring: Vec<Id> = stops.iter().filter(|s| s.layer == home.layer && s.clip == home.clip).map(|s| s.id).collect();
    if ring.len() < 2 {
        return None;
    }
    let at = ring.iter().position(|id| *id == focused)?;
    Some(if backward { ring[(at + ring.len() - 1) % ring.len()] } else { ring[(at + 1) % ring.len()] })
}

fn is_plain_tab(key: Key, m: &Modifiers) -> bool {
    key == Key::Tab && !m.alt && !m.ctrl && !m.command && !m.mac_cmd
}

/// Called with the frame's raw input, before egui reads it. While a text field that has a
/// neighbour holds the focus, the Tab and Shift+Tab presses are taken out of the input, so egui
/// does not walk its own focus order, and kept for [`start_frame`].
pub fn take_tab_press(ctx: &Context, raw: &mut RawInput) {
    ctx.data_mut(|d| d.remove_temp::<TabPress>(press_key()));
    if !raw.events.iter().any(|e| matches!(e, Event::Key { key, modifiers, .. } if is_plain_tab(*key, modifiers))) {
        return;
    }
    let Some(focused) = ctx.memory(|m| m.focused()) else { return };
    // The registry still holds the fields of the frame that just ended.
    let drawn = ctx.data(|d| d.get_temp::<Stops>(stops_key())).map(|s| s.drawn).unwrap_or_default();
    if neighbour(&drawn, focused, false).is_none() {
        return;
    }
    let mut press = None;
    raw.events.retain(|e| match e {
        Event::Key { key, pressed, modifiers, .. } if is_plain_tab(*key, modifiers) => {
            if *pressed {
                press = Some(TabPress { backward: modifiers.shift });
            }
            false
        }
        _ => true,
    });
    if let Some(press) = press {
        ctx.data_mut(|d| d.insert_temp(press_key(), press));
    }
}

/// Called first in every frame, once egui has begun it and before anything draws. Starts the
/// registry of this frame's fields and, if [`take_tab_press`] kept a Tab press, gives the focus to
/// the neighbouring field. Doing it here, rather than while the fields draw, lets the field that
/// gets the focus select its whole value and the one that loses it commit what was typed.
pub fn start_frame(ctx: &Context) {
    let press = ctx.data_mut(|d| {
        let stops = d.get_temp_mut_or_default::<Stops>(stops_key());
        stops.last = std::mem::take(&mut stops.drawn);
        d.remove_temp::<TabPress>(press_key())
    });
    let Some(press) = press else { return };
    let Some(focused) = ctx.memory(|m| m.focused()) else { return };
    let last = ctx.data(|d| d.get_temp::<Stops>(stops_key())).map(|s| s.last).unwrap_or_default();
    if let Some(next) = neighbour(&last, focused, press.backward) {
        ctx.memory_mut(|m| m.request_focus(next));
    }
}

/// True when the text field that has the keyboard focus is one of the registered fields.
#[cfg(test)]
pub fn a_field_has_focus(ctx: &Context) -> bool {
    let Some(focused) = ctx.memory(|m| m.focused()) else { return false };
    let stops = ctx.data(|d| d.get_temp::<Stops>(stops_key())).unwrap_or_default();
    stops.drawn.iter().chain(&stops.last).any(|s| s.id == focused)
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui::Order;

    const WHOLE_WINDOW: [i32; 4] = [0, 0, 1600, 1088];

    fn stop(name: &str, layer: LayerId) -> Stop {
        Stop { id: Id::new(name), layer, clip: WHOLE_WINDOW }
    }

    fn stop_in(name: &str, layer: LayerId, clip: [i32; 4]) -> Stop {
        Stop { id: Id::new(name), layer, clip }
    }

    #[test]
    fn neighbours_wrap_around_in_drawing_order() {
        let layer = LayerId::new(Order::Middle, Id::new("panel"));
        let stops = [stop("x", layer), stop("y", layer), stop("w", layer)];
        assert_eq!(neighbour(&stops, Id::new("x"), false), Some(Id::new("y")));
        assert_eq!(neighbour(&stops, Id::new("w"), false), Some(Id::new("x")), "past the last field comes the first");
        assert_eq!(neighbour(&stops, Id::new("x"), true), Some(Id::new("w")), "before the first field comes the last");
        assert_eq!(neighbour(&stops, Id::new("w"), true), Some(Id::new("y")));
    }

    #[test]
    fn a_neighbour_is_on_the_same_layer() {
        let panel = LayerId::new(Order::Middle, Id::new("panel"));
        let dialog = LayerId::new(Order::Foreground, Id::new("dialog"));
        let stops = [stop("x", panel), stop("name", dialog), stop("y", panel), stop("size", dialog)];
        assert_eq!(neighbour(&stops, Id::new("name"), false), Some(Id::new("size")));
        assert_eq!(neighbour(&stops, Id::new("size"), false), Some(Id::new("name")));
        assert_eq!(neighbour(&stops, Id::new("x"), false), Some(Id::new("y")));
    }

    #[test]
    fn a_neighbour_is_in_the_same_panel() {
        let layer = LayerId::new(Order::Middle, Id::new("panel"));
        let inspector = [1376, 80, 1600, 1088];
        let stops = [stop("page-rename", layer), stop_in("x", layer, inspector), stop("file-rename", layer), stop_in("y", layer, inspector)];
        assert_eq!(neighbour(&stops, Id::new("x"), false), Some(Id::new("y")));
        assert_eq!(neighbour(&stops, Id::new("y"), false), Some(Id::new("x")));
        assert_eq!(neighbour(&stops, Id::new("page-rename"), false), Some(Id::new("file-rename")), "same clip, same ring");
        let stops = [stop("page-rename", layer), stop_in("x", layer, inspector), stop_in("y", layer, inspector)];
        assert_eq!(neighbour(&stops, Id::new("page-rename"), false), None, "a rename in the page list never leads into the inspector");
        assert_eq!(neighbour(&stops, Id::new("x"), true), Some(Id::new("y")));
    }

    #[test]
    fn a_lone_field_and_a_stranger_have_no_neighbour() {
        let panel = LayerId::new(Order::Middle, Id::new("panel"));
        let dialog = LayerId::new(Order::Foreground, Id::new("dialog"));
        let stops = [stop("x", panel), stop("name", dialog)];
        assert_eq!(neighbour(&stops, Id::new("x"), false), None, "a field alone on its layer keeps egui's own Tab");
        assert_eq!(neighbour(&stops, Id::new("button"), false), None, "a button is not a field");
    }

    #[test]
    fn only_plain_tab_and_shift_tab_are_taken() {
        assert!(is_plain_tab(Key::Tab, &Modifiers::NONE));
        assert!(is_plain_tab(Key::Tab, &Modifiers::SHIFT));
        assert!(!is_plain_tab(Key::Tab, &Modifiers::COMMAND), "Cmd+Tab and Ctrl+Tab switch document tabs");
        assert!(!is_plain_tab(Key::Tab, &Modifiers::CTRL));
        assert!(!is_plain_tab(Key::Tab, &Modifiers::ALT));
        assert!(!is_plain_tab(Key::A, &Modifiers::NONE));
    }
}
