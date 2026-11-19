//! The inspector and the text session on a text in an instance made with the scale tool. Its size is bound to a
//! number variable (14 in light mode, 20 in dark mode) and it draws at the instance's 0.8, so a dark frame draws 16
//! while the stored size stays 11.2. The variable pill stays where a bound value is, so a typed number can never go
//! undrawn, and an edit or an undo inside a text session never loses the binding.

use super::*;
use designos_doc::{LineHeight, Node, NodeKind, Rect as DRect, TextStyle, VariableKind, VariableStore, VariableValue, Vec2 as DV};

const INNER: &str = "Rating in scaled badge";

/// What the fixture adds to the starter document, and the ids the tests need.
struct Ids {
    collection: String,
    dark: String,
    variable: String,
}

/// Adds a main with a text bound to the size variable and a 0.8 instance of it to the first page, far from the starter
/// content. The instance's text is named [`INNER`].
fn add_scaled_bound_text(d: &mut designos_doc::Document) -> Ids {
    let page = d.pages[0];
    let mut store = VariableStore::from_doc(d);
    let collection = store.add_collection("Type");
    let light = store.collections[&collection].modes[0].id.clone();
    let dark = store.add_mode(&collection, "Dark").unwrap();
    let variable = store.add_variable(&collection, "Font size/text-sm", VariableKind::Number).unwrap();
    store.set_value(&variable, &light, VariableValue::Number(14.0));
    store.set_value(&variable, &dark, VariableValue::Number(20.0));
    store.command().apply(d).unwrap();

    let style = TextStyle { font_size: 14.0, line_height: LineHeight::Pixels(20.0), ..Default::default() };
    let main_id = d.alloc_id();
    let main = d.insert(page, Node::frame(main_id, DRect::new(3000.0, 3000.0, 100.0, 28.0))).unwrap();
    let text_id = d.alloc_id();
    let mut text = Node::text(text_id, DRect::new(6.0, 4.0, 88.0, 20.0), "Rating", style);
    text.variables.bindings.insert("fontSize".into(), variable.clone());
    d.insert(main, text).unwrap();
    designos_doc::create_component(d, main).apply(d).unwrap();
    designos_doc::create_instance(d, main, page, DV::new(3000.0, 3200.0)).apply(d).unwrap();
    let inst = *d.get(page).unwrap().children.last().unwrap();
    let inner = d.get(inst).unwrap().children[0];
    let n = d.get_mut(inner).unwrap();
    n.name = INNER.into();
    n.variables.bindings.insert("fontSize".into(), variable.clone());
    if let NodeKind::Text(t) = &mut n.kind {
        t.apply_uniform_scale(0.8);
    }
    Ids { collection, dark, variable }
}

fn dark_page(d: &mut designos_doc::Document, ids: &Ids) {
    let page = d.pages[0];
    d.get_mut(page).unwrap().variables.modes.insert(ids.collection.clone(), ids.dark.clone());
}

/// The window with the scaled text selected, in a page whose frame is in dark mode when `dark`.
fn sim(dark: bool) -> (Sim, NodeId, std::rc::Rc<std::cell::RefCell<Option<Ids>>>) {
    let ids = std::rc::Rc::new(std::cell::RefCell::new(None));
    let slot = ids.clone();
    let s = Sim::with_starter_prep(
        Options { select: Some(format!("name:{INNER}")), ..Default::default() },
        move |d| {
            let made = add_scaled_bound_text(d);
            if dark {
                dark_page(d, &made);
            }
            *slot.borrow_mut() = Some(made);
        },
        |_| {},
    );
    let id = s.by_name(INNER);
    (s, id, ids)
}

fn recorded(s: &Sim, name: &str) -> bool {
    s.ctx.data(|d| d.get_temp::<Rect>(egui::Id::new(("test-rect", name.to_string()))).is_some())
}

fn has_size_field(s: &Sim) -> bool {
    s.ctx.data(|d| d.get_temp::<Rect>(egui::Id::new(("field-rect", format!("{:?}", "font-size")))).is_some())
}

fn text_props(s: &Sim, id: NodeId) -> designos_doc::TextProps {
    match &s.app().doc.get(id).unwrap().kind {
        NodeKind::Text(t) => t.clone(),
        _ => panic!("text layer"),
    }
}

fn drawn_size(s: &Sim, id: NodeId) -> f32 {
    let n = s.app().doc.get(id).unwrap();
    match &designos_doc::resolve_node(&s.app().doc, n).kind {
        NodeKind::Text(t) => t.style.font_size,
        _ => panic!("text layer"),
    }
}

#[test]
fn a_scaled_text_bound_to_a_variable_keeps_its_pill_outside_and_inside_a_text_session() {
    let (mut s, id, _) = sim(true);
    assert!(recorded(&s, "bound:fontSize"), "the pill stands where the bound size is");
    assert!(!has_size_field(&s), "so no number field takes a typed value that would not be drawn");
    crate::canvas::textsession::begin_text_edit(&mut s.shell.app, id);
    s.frame(vec![]);
    s.frame(vec![]);
    assert!(recorded(&s, "bound:fontSize"), "the pill stays in a text session");
    assert!(!has_size_field(&s));
}

#[test]
fn detaching_the_variable_of_a_scaled_text_in_a_dark_frame_keeps_the_drawn_size() {
    let (mut s, id, ids) = sim(true);
    let var = ids.borrow().as_ref().unwrap().variable.clone();
    assert!((drawn_size(&s, id) - 16.0).abs() < 1e-4, "dark is 20, drawn at 0.8 as 16");
    assert_eq!(s.app().doc.get(id).unwrap().variables.bindings.get("fontSize"), Some(&var));
    s.shell.app.detach_variable(&[id], &["fontSize"]);
    s.frame(vec![]);
    let n = s.app().doc.get(id).unwrap();
    assert!(!n.variables.bindings.contains_key("fontSize"), "unbound");
    let t = text_props(&s, id);
    assert!((t.style.font_size - 16.0).abs() < 1e-4, "the text holds the 16 it drew, got {}", t.style.font_size);
    assert!((t.uniform_scale - 0.8).abs() < 1e-6, "and still carries its factor");
    assert!((drawn_size(&s, id) - 16.0).abs() < 1e-4, "it still draws 16");
}

#[test]
fn a_text_session_edit_and_its_undo_leave_a_scaled_bound_text_as_it_was() {
    let (mut s, id, ids) = sim(true);
    let var = ids.borrow().as_ref().unwrap().variable.clone();
    let before = text_props(&s, id);
    crate::canvas::textsession::begin_text_edit(&mut s.shell.app, id);
    s.frame(vec![]);
    crate::canvas::textsession::style_selection_detaching_as(&mut s.shell.app, |st| st.font_size = 12.0, crate::textedit::EditKind::Style);
    s.frame(vec![]);
    assert_eq!(s.app().doc.get(id).unwrap().variables.bindings.get("fontSize"), Some(&var), "the edit leaves the binding");
    s.cmd_key(Key::Z);
    assert_eq!(s.app().doc.get(id).unwrap().variables.bindings.get("fontSize"), Some(&var), "and so does its undo");
    assert_eq!(text_props(&s, id), before, "the stored text is as before the edit");
    assert!((drawn_size(&s, id) - 16.0).abs() < 1e-4, "the canvas still draws 16 in dark mode");
}

#[test]
fn a_range_edit_in_a_scaled_bound_text_does_not_move_the_rest_of_the_text() {
    let (mut s, id, _) = sim(true);
    crate::canvas::textsession::begin_text_edit(&mut s.shell.app, id);
    {
        let sess = s.shell.app.canvas.text.as_mut().unwrap();
        sess.anchor = 0;
        sess.caret = 3;
    }
    crate::canvas::textsession::style_selection_detaching_as(&mut s.shell.app, |st| st.font_size = 12.0, crate::textedit::EditKind::Style);
    s.frame(vec![]);
    let n = s.app().doc.get(id).unwrap();
    let drawn = match &designos_doc::resolve_node(&s.app().doc, n).kind {
        NodeKind::Text(t) => t.clone(),
        _ => panic!("text layer"),
    };
    // the binding stays, so every range still draws the variable at the scale
    assert!(drawn.runs.iter().all(|r| (r.style.font_size - 16.0).abs() < 1e-4) && (drawn.style.font_size - 16.0).abs() < 1e-4, "{drawn:?}");
}
