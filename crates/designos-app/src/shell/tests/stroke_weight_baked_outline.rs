//! A layer imported from a `.fig` file carries a stroke outline baked at the weight it was imported with. Editing
//! the weight in the stroke panel must draw the new weight at once, and must keep drawing it after undo, redo and
//! a save and reload. The stored outline is only drawn while the layer still matches the stroke it was baked for.

use super::*;
use designos_doc::{Document, FillRule, Node, NodeKind, Path, PathCmd, Vec2 as DocVec2};

const SIZE: f32 = 100.0;
const BAKED_WEIGHT: f32 = 4.0;

fn ring(outer: f32, inset: f32) -> Path {
    let square = |lo: f32, hi: f32| {
        [
            PathCmd::MoveTo(DocVec2::new(lo, lo)),
            PathCmd::LineTo(DocVec2::new(hi, lo)),
            PathCmd::LineTo(DocVec2::new(hi, hi)),
            PathCmd::LineTo(DocVec2::new(lo, hi)),
            PathCmd::Close,
        ]
    };
    let mut cmds = square(0.0, outer).to_vec();
    cmds.extend(square(inset, outer - inset));
    Path { cmds, fill_rule: FillRule::EvenOdd }
}

/// A stroke-only layer of `kind`. With `baked` it also stores the outline of its 4px inside stroke, the way an
/// imported layer does (the stamp is written on insert).
fn add_stroked(s: &mut Sim, kind: NodeKind, name: &str, baked: bool) -> NodeId {
    let page = s.app().page;
    let id = s.shell.app.doc.alloc_id();
    let mut n = Node::new(id, kind);
    n.name = name.into();
    n.size = DocVec2::new(SIZE, SIZE);
    n.fills.clear();
    n.stroke.paints = vec![designos_doc::Paint::solid(designos_doc::Color::rgb(0, 0, 255))];
    n.stroke.weight = BAKED_WEIGHT;
    n.stroke.align = designos_doc::StrokeAlign::Inside;
    if baked {
        n.stroke_geometry = Some(vec![ring(SIZE, BAKED_WEIGHT)]);
    }
    s.shell.app.doc.insert(page, n).unwrap()
}

fn pixels_of(doc: &Document, id: NodeId) -> Vec<u8> {
    designos_render::render_node(doc, id, 1.0).expect("render").data().to_vec()
}

fn edit_field(s: &mut Sim, id: egui::Id, val: &str) {
    let r = s.ctx.data(|d| d.get_temp::<Rect>(id)).expect("field is shown");
    s.click(Pos2::new(r.left() + 40.0, r.center().y));
    s.type_text(val);
    s.key(Key::Enter);
}

fn edit_num_field(s: &mut Sim, key: &str, val: &str) {
    let id = egui::Id::new(("field-rect", format!("{key:?}")));
    edit_field(s, id, val);
}

fn live_outline(s: &Sim, id: NodeId) -> bool {
    s.app().doc.get(id).unwrap().live_stroke_outline().is_some()
}

#[test]
fn a_weight_edit_draws_the_new_stroke_not_the_baked_outline_and_undo_brings_the_outline_back() {
    let shapes = [("rectangle", NodeKind::Rectangle), ("frame", NodeKind::Frame(designos_doc::FrameProps::default()))];
    for (what, kind) in shapes {
        let mut s = Sim::new();
        let baked = add_stroked(&mut s, kind.clone(), "Baked", true);
        let control = add_stroked(&mut s, kind, "Control", false);
        s.shell.app.edit_nodes(&[control], |n| n.stroke.weight = 20.0);
        s.shell.app.selection = vec![baked];
        s.frame(vec![]);

        let before = pixels_of(&s.app().doc, baked);
        assert!(live_outline(&s, baked), "the imported layer draws its stored outline ({what})");
        assert!(before != pixels_of(&s.app().doc, control), "the 4px outline and the live 20px stroke are told apart ({what})");

        edit_num_field(&mut s, "stroke-w", "20");
        assert_eq!(s.app().doc.get(baked).unwrap().stroke.weight, 20.0, "the weight was set ({what})");
        assert!(!live_outline(&s, baked), "the stored outline no longer matches, so it is not drawn ({what})");
        assert!(pixels_of(&s.app().doc, baked) == pixels_of(&s.app().doc, control), "the panel edit draws the live 20px stroke ({what})");

        // A save and reload keeps rejecting the stale outline.
        let reloaded = Document::from_json(&s.app().doc.to_json().unwrap()).unwrap();
        assert!(reloaded.get(baked).unwrap().live_stroke_outline().is_none(), "reload does not revive the outline ({what})");
        assert!(pixels_of(&reloaded, baked) == pixels_of(&reloaded, control), "the reloaded layer draws the live stroke ({what})");

        s.cmd_key(Key::Z);
        assert_eq!(s.app().doc.get(baked).unwrap().stroke.weight, BAKED_WEIGHT, "undo restores the weight ({what})");
        assert!(pixels_of(&s.app().doc, baked) == before, "undo draws the stored outline again ({what})");
    }
}

#[test]
fn the_outline_baked_at_the_old_weight_is_drawn_again_only_when_the_weight_goes_back_to_it() {
    let mut s = Sim::new();
    let baked = add_stroked(&mut s, NodeKind::Rectangle, "Baked", true);
    s.shell.app.selection = vec![baked];
    s.frame(vec![]);
    let before = pixels_of(&s.app().doc, baked);

    edit_num_field(&mut s, "stroke-w", "9");
    assert!(!live_outline(&s, baked), "9px is not the baked weight");
    assert!(pixels_of(&s.app().doc, baked) != before);
    // The data stays on the node: the weight match is what decides whether it is drawn.
    assert!(s.app().doc.get(baked).unwrap().stroke_geometry.is_some(), "the stored outline is kept, only masked");

    edit_num_field(&mut s, "stroke-w", "4");
    assert!(live_outline(&s, baked), "back at the baked weight the outline matches the layer again");
    assert!(pixels_of(&s.app().doc, baked) == before, "and it draws what it drew at import, which is the 4px stroke");
}

#[test]
fn an_individual_side_weight_edit_draws_the_live_stroke_not_the_baked_outline() {
    let mut s = Sim::new();
    let baked = add_stroked(&mut s, NodeKind::Rectangle, "Baked", true);
    let control = add_stroked(&mut s, NodeKind::Rectangle, "Control", false);
    s.shell.app.edit_nodes(&[control], |n| n.stroke.side_weights = Some([4.0, 4.0, 4.0, 20.0]));
    s.shell.app.selection = vec![baked];
    s.frame(vec![]);

    // What the "Individual strokes" toggle does: one weight per side, all at the current weight.
    s.shell.app.edit_nodes(&[baked], |n| n.stroke.side_weights = Some([n.stroke.weight; 4]));
    s.frame(vec![]);
    assert!(!live_outline(&s, baked), "individual strokes are a different stroke from the one the outline was baked for");

    let left = egui::Id::new(("field-rect", format!("{:?}", ("stroke-side", 3usize))));
    edit_field(&mut s, left, "20");
    assert_eq!(s.app().doc.get(baked).unwrap().stroke.side_weights, Some([4.0, 4.0, 4.0, 20.0]));
    assert!(!live_outline(&s, baked));
    assert!(pixels_of(&s.app().doc, baked) == pixels_of(&s.app().doc, control), "the live per-side stroke is drawn");
}

#[test]
fn a_panel_weight_edit_survives_saving_a_fig_file_and_opening_it_again() {
    let mut s = Sim::new();
    let baked = add_stroked(&mut s, NodeKind::Rectangle, "Baked", true);
    let control = add_stroked(&mut s, NodeKind::Rectangle, "Control", false);
    s.shell.app.edit_nodes(&[control], |n| n.stroke.weight = 20.0);
    s.shell.app.selection = vec![baked];
    s.frame(vec![]);

    edit_num_field(&mut s, "stroke-w", "20");
    assert!(pixels_of(&s.app().doc, baked) == pixels_of(&s.app().doc, control), "the panel edit draws the live stroke");

    let reopened = designos_fig::import_fig(&designos_fig::export_fig(&s.app().doc).unwrap()).unwrap();
    let by_name = |name: &str| reopened.nodes.values().find(|n| n.name == name).unwrap().id;
    let same = pixels_of(&reopened, by_name("Baked")) == pixels_of(&reopened, by_name("Control"));
    assert!(same, "the saved and reopened layer still draws the live 20px stroke");
}
