//! Escape in the colour picker's OKLCH and OKLAB fields does what it does in the hex field: egui drops the
//! field's focus, the picker closes and the shell runs its Escape action on the selection.

use super::*;
use crate::widgets::picker::Fmt;

/// What the window looks like after Escape.
#[derive(Debug, PartialEq)]
struct AfterEscape {
    picker_open: bool,
    selection: Vec<NodeId>,
    focused: Option<egui::Id>,
}

/// Opens the fill picker of the selected "Hero" layer in `fmt`, focuses the value cell `cell` for several frames
/// (the picker locks its arrow keys from the second focused frame on), presses Escape and reports the result
/// next to the selection the layer started with.
fn press_escape_in_cell(fmt: Fmt, cell: &str) -> (Vec<NodeId>, AfterEscape) {
    let mut s = Sim::with(Options { select: Some("Hero".into()), ..Default::default() });
    let initial = s.app().selection.clone();
    assert_eq!(initial.len(), 1, "one layer is selected");
    s.click(s.rect_of("sw:false:0").center());
    s.frame(vec![]);
    s.shell.app.props.picker.as_mut().expect("the swatch opens the picker").state.fmt = fmt;
    s.frame(vec![]);

    let id = egui::Id::new("color-picker").with(("txt", cell));
    s.ctx.memory_mut(|m| m.request_focus(id));
    for _ in 0..4 {
        s.frame(vec![]);
    }
    assert!(s.ctx.memory(|m| m.has_focus(id)), "the {cell} cell must hold the focus before Escape");
    assert!(s.app().props.picker.is_some(), "the picker is still open before Escape");

    s.key(Key::Escape);
    let after = AfterEscape { picker_open: s.app().props.picker.is_some(), selection: s.app().selection.clone(), focused: s.ctx.memory(|m| m.focused()) };
    (initial, after)
}

#[test]
fn escape_in_the_oklch_and_oklab_fields_does_what_it_does_in_the_hex_field() {
    let (initial, hex) = press_escape_in_cell(Fmt::Hex, "picker-hex");
    assert!(!hex.picker_open, "Escape closes the picker from the hex field: {hex:?}");
    assert_ne!(hex.selection, initial, "Escape from the hex field runs the shell action on the selection: {hex:?}");
    assert_eq!(hex.focused, None, "Escape from the hex field leaves no focused widget: {hex:?}");

    for (fmt, label) in [(Fmt::Oklch, "OKLCH"), (Fmt::Oklab, "OKLAB")] {
        let (_, ok) = press_escape_in_cell(fmt, "picker-ok");
        assert_eq!(ok, hex, "Escape from the {label} field must match the hex field");
    }
}
