//! A colour variable bound to a text layer reaches the style runs that still have the base colour
//! and leaves runs coloured differently (a highlighted word) alone. A colour style written to a
//! text layer recolours every run. The renderer paints the runs, so a run left behind would keep
//! showing the old colour.

use super::*;
use designos_doc::{resolved_node, TextRun};

const RED: Color = Color { r: 229.0 / 255.0, g: 72.0 / 255.0, b: 77.0 / 255.0, a: 1.0 };

/// The colours the renderer paints for `text`: the base fill, then each run, with bindings applied.
fn painted(app: &App, text: NodeId) -> Vec<Color> {
    let n = resolved_node(&app.doc, text).expect("text");
    let NodeKind::Text(t) = &n.kind else { panic!("not text") };
    let solid = |ps: &[Paint]| match ps.first().map(|p| &p.kind) {
        Some(PaintKind::Solid(c)) => *c,
        _ => panic!("no solid fill"),
    };
    std::iter::once(solid(&t.style.fills)).chain(t.runs.iter().map(|r| solid(&r.style.fills))).collect()
}

/// 28 px text in the brand blue on a black page, covered by runs of the given byte ranges and colours.
fn brand_text(runs: &[(usize, usize, Color)]) -> (Fx, NodeId, String) {
    let mut f = fx();
    f.doc.get_mut(f.page).unwrap().kind = NodeKind::Page { background: Color::BLACK };
    let var = f.color_var("Brand/Blue", BLUE, BLUE);
    let t = f.text(f.page, "Check the highlighted word today", 28.0, 400, BLUE);
    let NodeKind::Text(tp) = &mut f.doc.get_mut(t).unwrap().kind else { panic!("text") };
    tp.runs = runs
        .iter()
        .map(|(start, end, c)| TextRun { start: *start, end: *end, style: TextStyle { fills: vec![Paint::solid(*c)], ..tp.style.clone() } })
        .collect();
    (f, t, var)
}

/// Changes the colour variable to white, as editing it in the Variables panel would.
fn turn_variable_white(app: &mut App, variable: &str) {
    for value in app.doc.variables.get_mut(variable).expect("variable").values.values_mut() {
        *value = VariableValue::Color(Color::WHITE);
    }
    app.changed();
}

/// Binds the text's base fill to the variable the audit suggests, the way the panel's Fix does.
fn bind_fix(app: &mut App, var: &str) {
    let finding = fix_of(app, Rule::HardCodedValue);
    assert_eq!(finding.fix, Some(Fix::BindVariable { key: "fills/0".into(), variable: var.into() }));
    assert!(app.check_designs_fix(&finding), "the bind fix applies");
}

#[test]
fn a_run_that_matches_the_base_follows_the_variable() {
    let (f, text, var) = brand_text(&[(0, 32, BLUE)]);
    let mut app = f.app();
    bind_fix(&mut app, &var);
    assert_eq!(painted(&app, text), vec![BLUE, BLUE]);
    turn_variable_white(&mut app, &var);
    assert_eq!(painted(&app, text), vec![Color::WHITE; 2], "the glyphs are painted from the run, which has to follow");
}

#[test]
fn the_bind_fix_leaves_the_highlighted_run_alone() {
    let (f, text, var) = brand_text(&[(0, 10, BLUE), (10, 21, RED), (21, 32, BLUE)]);
    let mut app = f.app();
    bind_fix(&mut app, &var);
    assert_eq!(painted(&app, text), vec![BLUE, BLUE, RED, BLUE]);
}

#[test]
fn after_the_variable_changes_only_the_highlighted_run_keeps_its_colour() {
    let (f, text, var) = brand_text(&[(0, 10, BLUE), (10, 21, RED), (21, 32, BLUE)]);
    let mut app = f.app();
    bind_fix(&mut app, &var);
    turn_variable_white(&mut app, &var);
    assert_eq!(painted(&app, text), vec![Color::WHITE, Color::WHITE, RED, Color::WHITE]);
}

#[test]
fn applying_a_fill_style_recolours_every_run() {
    let (mut f, text, _) = brand_text(&[(0, 10, BLUE), (10, 21, RED)]);
    f.style("ink", "Ink", Style::Fill(vec![Paint::solid(gray(0x20))]), StyleProperty::Fill);
    let mut app = f.app();
    app.apply_style(&[text], "ink");
    assert_eq!(painted(&app, text), vec![gray(0x20), gray(0x20), gray(0x20)]);
    app.undo();
    assert_eq!(painted(&app, text), vec![BLUE, BLUE, RED], "one undo step puts all of it back");
}

#[test]
fn updating_a_linked_colour_style_still_recolours_every_run() {
    // Editing the style itself recolours every run of the layers linked to it, as applying it does.
    let (mut f, text, _) = brand_text(&[(0, 10, BLUE), (10, 21, RED)]);
    f.style("ink", "Ink", Style::Fill(vec![Paint::solid(BLUE)]), StyleProperty::Fill);
    f.doc.get_mut(text).unwrap().style_refs.insert("fill".into(), "ink".into());
    let mut app = f.app();
    let update = designos_doc::update_style(&app.doc, "ink", Style::Fill(vec![Paint::solid(gray(0x20))])).expect("the style can be edited");
    update.apply(&mut app.doc).expect("the update applies");
    assert_eq!(painted(&app, text), vec![gray(0x20), gray(0x20), gray(0x20)]);
}
