//! Applying a colour style from the style picker to layers whose fill is bound to a colour variable.

use crate::app::App;
use designos_doc::*;

const VARIABLE_COLOUR: Color = Color { r: 0.0, g: 0.0, b: 1.0, a: 1.0 };
const STYLE_COLOUR: Color = Color { r: 1.0 / 255.0, g: 2.0 / 255.0, b: 3.0 / 255.0, a: 1.0 };

struct Fixture {
    app: App,
    /// Rectangle whose fill, fill opacity and stroke colour are bound to variables.
    bound: NodeId,
    /// Rectangle with a plain fill.
    plain: NodeId,
    /// Text whose colour is bound to the same variable.
    text: NodeId,
    variable: String,
}

/// A document with a colour variable (blue), a fill style (`Brand/Primary`) and a stroke style
/// (`Outline/Ink`), both holding paints of their own.
fn fixture() -> Fixture {
    let mut d = Document::new("t");
    let page = d.add_page("p");
    let frame_id = d.alloc_id();
    let frame = d.insert(page, Node::frame(frame_id, Rect::new(0.0, 0.0, 200.0, 200.0))).unwrap();

    let mut store = VariableStore::from_doc(&d);
    let collection = store.add_collection("Theme");
    let variable = store.add_variable(&collection, "Brand/Primary", VariableKind::Color).unwrap();
    let opacity = store.add_variable(&collection, "Brand/Opacity", VariableKind::Number).unwrap();
    let ink = store.add_variable(&collection, "Brand/Ink", VariableKind::Color).unwrap();
    let mode = store.collections[&collection].default_mode.clone();
    assert!(store.set_value(&variable, &mode, VariableValue::Color(VARIABLE_COLOUR)));
    assert!(store.set_value(&opacity, &mode, VariableValue::Number(100.0)));
    assert!(store.set_value(&ink, &mode, VariableValue::Color(Color::rgb(9, 9, 9))));
    d.variables = store.variables;
    d.variable_collections = store.collections;

    let bound_id = d.alloc_id();
    let mut bound = Node::rectangle(bound_id, Rect::new(10.0, 10.0, 40.0, 40.0));
    bound.fills = vec![Paint::solid(VARIABLE_COLOUR)];
    bound.stroke.paints = vec![Paint::solid(Color::rgb(9, 9, 9))];
    for (key, var) in [("fills/0", &variable), ("fills/0/opacity", &opacity), ("strokes/0", &ink)] {
        bound.variables.bindings.insert(key.into(), var.clone());
    }
    let bound = d.insert(frame, bound).unwrap();

    let plain_id = d.alloc_id();
    let mut plain = Node::rectangle(plain_id, Rect::new(60.0, 10.0, 40.0, 40.0));
    plain.fills = vec![Paint::solid(Color::rgb(255, 0, 0))];
    let plain = d.insert(frame, plain).unwrap();

    let text_id = d.alloc_id();
    let mut text =
        Node::text(text_id, Rect::new(0.0, 100.0, 80.0, 20.0), "Hello", TextStyle { fills: vec![Paint::solid(VARIABLE_COLOUR)], ..Default::default() });
    text.variables.bindings.insert("fills/0".into(), variable.clone());
    let text = d.insert(frame, text).unwrap();

    d.styles.insert("Brand/Primary".into(), Style::Fill(vec![Paint::solid(STYLE_COLOUR)]));
    d.styles.insert("Outline/Ink".into(), Style::Fill(vec![Paint::solid(Color::rgb(200, 100, 50))]));
    d.style_meta.insert("Outline/Ink".into(), StyleMeta { name: "Outline/Ink".into(), property: StyleProperty::Stroke, ..Default::default() });

    Fixture { app: App::new(d), bound, plain, text, variable }
}

fn fill_bindings(doc: &Document, id: NodeId) -> Vec<String> {
    doc.get(id).unwrap().variables.bindings.keys().filter(|k| k.starts_with("fills/")).cloned().collect()
}

/// The paints the canvas draws for `id`, with its variable bindings applied.
fn drawn_fills(doc: &Document, id: NodeId) -> Vec<Paint> {
    resolved_node(doc, id).unwrap().fills.clone()
}

/// The CPU renderer's pixel at the middle of a 40x40 rectangle.
fn centre_pixel(doc: &Document, id: NodeId) -> [u8; 4] {
    let pixmap = designos_render::render_node(doc, id, 1.0).expect("the node renders");
    let p = pixmap.pixel(20, 20).expect("a pixel inside the node");
    [p.red(), p.green(), p.blue(), p.alpha()]
}

#[test]
fn applying_a_fill_style_drops_the_fill_binding_and_the_canvas_shows_the_style() {
    let Fixture { mut app, bound, plain, variable, .. } = fixture();
    assert_eq!(drawn_fills(&app.doc, bound), vec![Paint::solid(VARIABLE_COLOUR)], "before: the variable's colour is drawn");
    assert_eq!(centre_pixel(&app.doc, bound), [0, 0, 255, 255]);

    app.apply_style(&[bound, plain], "Brand/Primary");

    let node = app.doc.get(bound).unwrap();
    assert!(fill_bindings(&app.doc, bound).is_empty(), "the fill colour and opacity bindings are gone: {:?}", node.variables.bindings);
    assert_eq!(node.style_refs.get("fill").map(String::as_str), Some("Brand/Primary"));
    assert_eq!(node.fills, vec![Paint::solid(STYLE_COLOUR)]);
    assert_eq!(drawn_fills(&app.doc, bound), vec![Paint::solid(STYLE_COLOUR)], "the canvas draws the style's colour, not the variable's");
    assert_eq!(centre_pixel(&app.doc, bound), [1, 2, 3, 255]);
    assert_eq!(centre_pixel(&app.doc, plain), [1, 2, 3, 255], "a plain layer in the same selection takes the style too");
    assert!(node.variables.bindings.contains_key("strokes/0"), "the stroke colour binding is not a fill binding");
    assert!(app.doc.variables.contains_key(&variable), "the variable itself stays in the file");
}

#[test]
fn one_undo_restores_the_previous_fill_and_its_binding() {
    let Fixture { mut app, bound, plain, .. } = fixture();
    let before_bound = app.doc.get(bound).unwrap().clone();
    let before_plain = app.doc.get(plain).unwrap().clone();

    app.apply_style(&[bound, plain], "Brand/Primary");
    assert!(fill_bindings(&app.doc, bound).is_empty(), "the style edit dropped the binding in the same step");

    app.undo();
    assert_eq!(app.doc.get(bound).unwrap(), &before_bound, "fill, bindings and style links are back");
    assert_eq!(app.doc.get(plain).unwrap(), &before_plain);
    assert_eq!(fill_bindings(&app.doc, bound), ["fills/0", "fills/0/opacity"]);
    assert_eq!(drawn_fills(&app.doc, bound), vec![Paint::solid(VARIABLE_COLOUR)]);
    assert_eq!(centre_pixel(&app.doc, bound), [0, 0, 255, 255]);
    assert!(!app.hist.can_undo(), "the style and the binding came off as one step");

    app.redo();
    assert!(fill_bindings(&app.doc, bound).is_empty(), "redo applies the style and drops the binding again");
    assert_eq!(centre_pixel(&app.doc, bound), [1, 2, 3, 255]);
}

#[test]
fn applying_a_stroke_style_leaves_the_fill_bindings_alone() {
    let Fixture { mut app, bound, plain, .. } = fixture();
    let fill_before = app.doc.get(bound).unwrap().fills.clone();

    app.apply_style(&[bound, plain], "Outline/Ink");

    let node = app.doc.get(bound).unwrap();
    assert_eq!(node.style_refs.get("stroke").map(String::as_str), Some("Outline/Ink"));
    assert!(!node.style_refs.contains_key("fill"), "a stroke style does not link the fill");
    assert_eq!(node.stroke.paints, vec![Paint::solid(Color::rgb(200, 100, 50))]);
    assert_eq!(fill_bindings(&app.doc, bound), ["fills/0", "fills/0/opacity"], "the fill bindings stay");
    assert_eq!(node.fills, fill_before);
    assert_eq!(drawn_fills(&app.doc, bound), vec![Paint::solid(VARIABLE_COLOUR)], "the canvas keeps drawing the variable's colour");
    assert_eq!(centre_pixel(&app.doc, bound), [0, 0, 255, 255]);
    assert_eq!(centre_pixel(&app.doc, plain), [255, 0, 0, 255], "the plain layer's fill is not touched either");
}

#[test]
fn applying_a_fill_style_to_text_drops_the_colour_binding() {
    let Fixture { mut app, text, .. } = fixture();
    let drawn_text_colours = |doc: &Document| match &resolved_node(doc, text).unwrap().kind {
        NodeKind::Text(t) => (t.style.fills.clone(), t.runs.iter().map(|r| r.style.fills.clone()).collect::<Vec<_>>()),
        other => panic!("{other:?}"),
    };
    let before = app.doc.get(text).unwrap().clone();
    assert_eq!(drawn_text_colours(&app.doc).0, vec![Paint::solid(VARIABLE_COLOUR)]);

    app.apply_style(&[text], "Brand/Primary");

    assert!(fill_bindings(&app.doc, text).is_empty(), "{:?}", app.doc.get(text).unwrap().variables.bindings);
    let style_paints = vec![Paint::solid(STYLE_COLOUR)];
    let (base, runs) = drawn_text_colours(&app.doc);
    assert_eq!(base, style_paints, "the text is drawn in the style's colour");
    assert!(runs.iter().all(|r| r == &style_paints), "every run too: {runs:?}");

    app.undo();
    assert_eq!(app.doc.get(text).unwrap(), &before, "one undo restores the colour and its binding");
    assert_eq!(drawn_text_colours(&app.doc).0, vec![Paint::solid(VARIABLE_COLOUR)]);
}
