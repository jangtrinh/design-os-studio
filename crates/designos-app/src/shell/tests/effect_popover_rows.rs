//! The settings popovers of glass, noise and texture list the rows Figma shows, in Figma's order. The noise
//! popover shows the opacity its grain is drawn with in every mode, and a second size row only while a second
//! size is stored.

use super::*;
use crate::panels::effects::{effect_rows, PopKind, PopState};
use crate::widgets::field::Prefix;
use designos_doc::{Effect, Style, StyleProperty};
use serde_json::json;

fn effect(json: serde_json::Value) -> Effect {
    serde_json::from_value(json).expect("an effect")
}

fn glass() -> Effect {
    effect(
        json!({"Glass": {"light_angle": 135.0, "light_intensity": 0.75, "refraction": 0.5, "depth": 20.0, "dispersion": 0.5, "frost": 4.0, "visible": true}}),
    )
}

fn noise(mode: serde_json::Value, size_y: Option<f32>) -> Effect {
    let mut n = json!({"mode": mode, "size": 3.0, "density": 0.5, "opacity": 0.5, "blend": "Normal", "seed": 1, "visible": true});
    if let Some(y) = size_y {
        n["size_y"] = json!(y);
    }
    effect(json!({"Noise": n}))
}

fn mono() -> serde_json::Value {
    json!({"Mono": {"color": {"r": 0.0, "g": 0.0, "b": 0.0, "a": 1.0}}})
}

fn duo() -> serde_json::Value {
    json!({"Duo": {"color": {"r": 0.0, "g": 0.0, "b": 0.0, "a": 1.0}, "secondary": {"r": 1.0, "g": 1.0, "b": 1.0, "a": 1.0}}})
}

fn texture(size_y: Option<f32>) -> Effect {
    let mut t = json!({"size": 3.0, "radius": 4.0, "clip_to_shape": true, "seed": 1, "visible": true});
    if let Some(y) = size_y {
        t["size_y"] = json!(y);
    }
    effect(json!({"Texture": t}))
}

/// "Welcome" with `fx` as its only effect and that effect's popover open.
fn open(fx: &Effect) -> Sim {
    let mut s = Sim::with(Options { select: Some("Welcome".into()), ..Default::default() });
    let welcome = s.by_name("Welcome");
    s.shell.app.edit_nodes(&[welcome], |n| n.effects = vec![fx.clone()]);
    let anchor = egui::Rect::from_min_size(Pos2::new(1200.0, 300.0), Vec2::new(24.0, 24.0));
    s.shell.app.props.pop = Some(PopState { kind: PopKind::Effect(0), anchor, ids: vec![welcome] });
    s.frame(vec![]);
    s.frame(vec![]);
    s
}

/// "Welcome" linked to an effect style that holds `fx`, with the style's "Edit style" popover open.
fn open_style(fx: &Effect) -> Sim {
    let mut s = Sim::with(Options { select: Some("Welcome".into()), ..Default::default() });
    let welcome = s.by_name("Welcome");
    s.shell.app.edit_nodes(&[welcome], |n| n.effects = vec![fx.clone()]);
    assert!(s.shell.app.create_style_from(welcome, StyleProperty::Effect).is_some());
    s.scroll_inspector_down();
    let button = s.rect_of("effect-style-edit");
    s.click(button.center());
    s.frame(vec![]);
    assert!(s.app().props.style_edit.is_some(), "the style popover is open");
    s
}

/// `(size, size_y)` of the noise or texture the style holds.
fn style_sizes(s: &Sim) -> (f32, Option<f32>) {
    let Some(Style::Effect(fx)) = s.app().doc.styles.get("Effect style 1") else { panic!("no effect style") };
    match &fx[0] {
        Effect::Noise(n) => (n.size, n.size_y),
        Effect::Texture(t) => (t.size, t.size_y),
        other => panic!("not a noise or a texture: {other:?}"),
    }
}

/// The labels painted down the popover's label column under its `title`, top to bottom. The popover
/// sits left of the inspector, so its title is the leftmost text with that string.
fn row_labels(s: &Sim, title: &str) -> Vec<String> {
    fn collect(shape: &egui::Shape, out: &mut Vec<(String, Pos2)>) {
        match shape {
            egui::Shape::Text(t) => out.push((t.galley.job.text.clone(), t.pos)),
            egui::Shape::Vec(shapes) => shapes.iter().for_each(|x| collect(x, out)),
            _ => {}
        }
    }
    let mut texts = Vec::new();
    s.shapes.iter().for_each(|c| collect(&c.shape, &mut texts));
    let head = texts.iter().filter(|(t, _)| t == title).map(|(_, p)| *p).min_by(|a, b| a.x.total_cmp(&b.x)).expect("the popover title is painted");
    let mut rows: Vec<&(String, Pos2)> = texts.iter().filter(|(_, p)| (p.x - head.x).abs() < 0.5 && p.y > head.y + 1.0 && p.y < head.y + 400.0).collect();
    rows.sort_by(|a, b| a.1.y.total_cmp(&b.1.y));
    rows.into_iter().map(|(t, _)| t.clone()).collect()
}

fn prefix(s: &Sim, key: &str) -> Prefix {
    s.ctx.data(|d| d.get_temp::<Prefix>(egui::Id::new(("field-prefix", key.to_string())))).unwrap_or_else(|| panic!("no field {key}"))
}

fn assert_distinct_prefixes(s: &Sim, keys: &[&str]) {
    let all: Vec<Prefix> = keys.iter().map(|k| prefix(s, k)).collect();
    for (i, a) in all.iter().enumerate() {
        assert!(matches!(a, Prefix::Icon(_)), "{} has an icon prefix", keys[i]);
        assert!(all[i + 1..].iter().all(|b| b != a), "{} repeats the prefix of a later row: {a:?}", keys[i]);
    }
}

/// Rect of a numeric field of the open popover, by the debug text of its key.
fn field_at(s: &Sim, key: &str) -> egui::Rect {
    s.ctx.data(|d| d.get_temp::<egui::Rect>(egui::Id::new(("field-rect", key.to_string())))).unwrap_or_else(|| panic!("no field {key}"))
}

/// The effect "Welcome" holds.
fn held_effect(s: &Sim) -> Effect {
    let welcome = s.by_name("Welcome");
    s.app().doc.get(welcome).unwrap().effects[0].clone()
}

/// `(size, size_y)` of the noise or texture "Welcome" holds.
fn sizes(s: &Sim) -> (f32, Option<f32>) {
    match held_effect(s) {
        Effect::Noise(n) => (n.size, n.size_y),
        Effect::Texture(t) => (t.size, t.size_y),
        other => panic!("not a noise or a texture: {other:?}"),
    }
}

fn opacity(s: &Sim) -> f32 {
    match held_effect(s) {
        Effect::Noise(n) => n.opacity,
        other => panic!("not a noise: {other:?}"),
    }
}

/// Clicks into field `key`, types `text` and commits it.
fn type_into(s: &mut Sim, key: &str, text: &str) {
    let r = field_at(s, key);
    s.click(Pos2::new(r.left() + 40.0, r.center().y));
    s.type_text(text);
    s.key(Key::Enter);
}

/// Presses the label of field `key`, moves the pointer `dx` px a frame for `frames` frames and releases it.
/// `watch` runs after each move with the number of moves made so far.
fn scrub(s: &mut Sim, key: &str, dx: f32, frames: usize, mut watch: impl FnMut(&Sim, usize)) {
    let r = field_at(s, key);
    let start = Pos2::new(r.left() + 8.0, r.center().y);
    s.mv(start);
    s.button(true);
    for k in 1..=frames {
        s.mv(start + Vec2::new(dx * k as f32, 0.0));
        watch(s, k);
    }
    s.button(false);
    s.frame(vec![]);
}

/// Scrubs the pressed side of `case` (X 3 -> 7 against Y = 7, Y 7 -> 3 against X = 3) until it equals the other side
/// and releases there. Returns the value the two sides share.
fn scrub_to_the_other_side(s: &mut Sim, case: &SizeCase) -> f32 {
    let (dx, shared) = if case.x { (1.0, 7.0) } else { (-1.0, 3.0) };
    scrub(s, case.key, dx, 4, |_, _| {});
    s.frame(vec![]);
    shared
}

/// The shared value as a user types it into a row that already shows it. A field commits only text that differs
/// from what it shows, so the value goes in with a decimal point: "7.0" for 7.
fn shared_as_typed(shared: f32) -> String {
    format!("{shared:.1}")
}

/// A size field to scrub: which effect, which side, and the keys and title that find it.
struct SizeCase {
    name: &'static str,
    fx: Effect,
    title: &'static str,
    /// Key of the field that is pressed.
    key: &'static str,
    /// Key of the other size row.
    other: &'static str,
    /// True when the pressed row is the X row.
    x: bool,
}

fn noise_x() -> SizeCase {
    SizeCase { name: "noise X", fx: noise(mono(), Some(7.0)), title: "Noise", key: "(\"fxp-nsize\", 0)", other: "(\"fxp-nsize-y\", 0)", x: true }
}

fn noise_y() -> SizeCase {
    SizeCase { name: "noise Y", fx: noise(mono(), Some(7.0)), title: "Noise", key: "(\"fxp-nsize-y\", 0)", other: "(\"fxp-nsize\", 0)", x: false }
}

fn texture_x() -> SizeCase {
    SizeCase { name: "texture X", fx: texture(Some(7.0)), title: "Texture", key: "(\"fxp-tsize\", 0)", other: "(\"fxp-tsize-y\", 0)", x: true }
}

fn texture_y() -> SizeCase {
    SizeCase { name: "texture Y", fx: texture(Some(7.0)), title: "Texture", key: "(\"fxp-tsize-y\", 0)", other: "(\"fxp-tsize\", 0)", x: false }
}

/// Noise and texture of size 3 with a second size 7, pressed on their X and on their Y row.
fn size_cases() -> Vec<SizeCase> {
    vec![noise_x(), noise_y(), texture_x(), texture_y()]
}

/// The two size rows, as the popover paints them.
fn two_size_rows(s: &Sim, title: &str) -> bool {
    let labels = row_labels(s, title);
    labels.iter().any(|l| l == "Size X") && labels.iter().any(|l| l == "Size Y")
}

#[test]
fn glass_rows_use_the_figma_labels_in_order() {
    let fx = glass();
    let s = open(&fx);
    assert_eq!(row_labels(&s, "Glass"), ["Light angle", "Light intensity", "Refraction", "Depth", "Dispersion", "Splay", "Bevel", "Frost"]);
    assert_eq!(effect_rows(&fx), 8);
    assert_distinct_prefixes(
        &s,
        &[
            "(\"fxp-angle\", 0)",
            "(\"fxp-glass\", 0, 0)",
            "(\"fxp-glass\", 0, 1)",
            "(\"fxp-glass\", 0, 2)",
            "(\"fxp-glass\", 0, 3)",
            "(\"fxp-glass\", 0, 4)",
            "(\"fxp-glass\", 0, 5)",
            "(\"fxp-glass\", 0, 6)",
        ],
    );
}

#[test]
fn noise_opacity_row_is_shown_for_every_mode_and_edits_the_opacity_the_grain_is_drawn_with() {
    let cases = [
        (mono(), vec!["Mode", "Color", "Size", "Density", "Opacity"]),
        (duo(), vec!["Mode", "Color 1", "Color 2", "Size", "Density", "Opacity"]),
        (json!("Multi"), vec!["Mode", "Size", "Density", "Opacity"]),
    ];
    for (mode, labels) in cases {
        let fx = noise(mode.clone(), None);
        let mut s = open(&fx);
        assert_eq!(row_labels(&s, "Noise"), labels, "mode {mode}");
        assert_eq!(effect_rows(&fx), labels.len(), "mode {mode}");
        // opening the popover leaves the stored value alone, and the row writes it
        assert_eq!(opacity(&s), 0.5, "mode {mode}");
        type_into(&mut s, "(\"fxp-nop\", 0)", "80");
        assert_eq!(opacity(&s), 0.8, "mode {mode}: the row edits the opacity");
    }
}

#[test]
fn the_opacity_of_a_mono_noise_changes_what_is_drawn() {
    let mut s = open(&noise(mono(), None));
    let welcome = s.by_name("Welcome");
    // a light card, so black grain shows at any opacity
    s.shell.app.edit_nodes(&[welcome], |n| n.fills = vec![Paint::solid(Color::WHITE)]);
    let render = |s: &Sim| designos_render::render_node(&s.app().doc, welcome, 0.5).expect("the card renders");
    let faint = render(&s);
    type_into(&mut s, "(\"fxp-nop\", 0)", "90");
    assert_eq!(opacity(&s), 0.9);
    let strong = render(&s);
    assert_ne!(faint.data(), strong.data(), "the grain is drawn with the opacity the row shows");
}

#[test]
fn unequal_noise_size_shows_x_and_y_rows() {
    let unequal = noise(mono(), Some(7.0));
    let s = open(&unequal);
    assert_eq!(row_labels(&s, "Noise"), ["Mode", "Color", "Size X", "Size Y", "Density", "Opacity"]);
    assert_eq!(effect_rows(&unequal), 6);
    assert_distinct_prefixes(&s, &["(\"fxp-nsize\", 0)", "(\"fxp-nsize-y\", 0)", "(\"fxp-dens\", 0)", "(\"fxp-nop\", 0)"]);

    let equal = noise(mono(), None);
    assert_eq!(row_labels(&open(&equal), "Noise"), ["Mode", "Color", "Size", "Density", "Opacity"]);
}

#[test]
fn unequal_texture_size_shows_x_and_y_rows() {
    let unequal = texture(Some(1.5));
    let s = open(&unequal);
    assert_eq!(row_labels(&s, "Texture"), ["Size X", "Size Y", "Radius", "Clip to shape"]);
    assert_eq!(effect_rows(&unequal), 4);
    assert_distinct_prefixes(&s, &["(\"fxp-tsize\", 0)", "(\"fxp-tsize-y\", 0)", "(\"fxp-trad\", 0)"]);

    let equal = texture(None);
    assert_eq!(row_labels(&open(&equal), "Texture"), ["Size", "Radius", "Clip to shape"]);
    assert_eq!(effect_rows(&equal), 3);
}

/// Scrubs the pressed side of `case` past the value of the other side (X goes 3 -> 10 past Y = 7, Y goes 7 -> 1
/// past X = 3; the two sides meet after the fourth move) and checks that both sides and both rows survive, and that
/// the whole scrub is one undo step.
fn scrub_past_the_other_side(case: SizeCase) {
    let mut s = open(&case.fx);
    let depth = s.app().hist.undo_depth();
    let (dx, frames, expected) = if case.x { (1.0, 7, (10.0, Some(7.0))) } else { (-1.0, 6, (3.0, Some(1.0))) };
    scrub(&mut s, case.key, dx, frames, |s, k| {
        if k >= 4 {
            let (x, y) = sizes(s);
            if k == 4 {
                assert_eq!(y.unwrap_or(x), x, "{}: the sides meet at move {k}", case.name);
            }
            assert!(two_size_rows(s, case.title), "{}: move {k} still shows the X and Y rows, got {:?}", case.name, row_labels(s, case.title));
        }
    });
    assert_eq!(sizes(&s), expected, "{}", case.name);
    assert!(two_size_rows(&s, case.title), "{}: the sides differ again, so both rows stay", case.name);
    assert_eq!(s.app().hist.undo_depth(), depth + 1, "{}: one undo step for the whole scrub", case.name);
    s.cmd_key(Key::Z);
    assert_eq!(sizes(&s), (3.0, Some(7.0)), "{}: one undo takes the whole scrub back", case.name);
}

#[test]
fn a_noise_size_x_scrub_past_the_y_value_keeps_y_and_is_one_undo_step() {
    scrub_past_the_other_side(noise_x());
}

#[test]
fn a_noise_size_y_scrub_past_the_x_value_keeps_x_and_is_one_undo_step() {
    scrub_past_the_other_side(noise_y());
}

#[test]
fn a_texture_size_x_scrub_past_the_y_value_keeps_y_and_is_one_undo_step() {
    scrub_past_the_other_side(texture_x());
}

#[test]
fn a_texture_size_y_scrub_past_the_x_value_keeps_x_and_is_one_undo_step() {
    scrub_past_the_other_side(texture_y());
}

#[test]
fn a_size_scrub_that_ends_where_the_two_sides_meet_keeps_both_rows_and_is_one_undo_step() {
    for case in size_cases() {
        let mut s = open(&case.fx);
        let depth = s.app().hist.undo_depth();
        let shared = scrub_to_the_other_side(&mut s, &case);
        assert_eq!(sizes(&s), (shared, Some(shared)), "{}: the scrub leaves the sides as they are", case.name);
        assert!(two_size_rows(&s, case.title), "{}: both rows stay, got {:?}", case.name, row_labels(&s, case.title));
        assert_eq!(s.app().hist.undo_depth(), depth + 1, "{}: one undo step for the whole scrub", case.name);
        s.cmd_key(Key::Z);
        assert_eq!(sizes(&s), (3.0, Some(7.0)), "{}: one undo takes the whole scrub back", case.name);
    }
}

#[test]
fn a_typed_size_after_a_scrub_that_ended_on_the_other_side_falls_back_to_one_size_in_one_step() {
    for case in size_cases() {
        for row in [case.key, case.other] {
            let mut s = open(&case.fx);
            let shared = scrub_to_the_other_side(&mut s, &case);
            let depth = s.app().hist.undo_depth();
            type_into(&mut s, row, &shared_as_typed(shared));
            assert_eq!(sizes(&s), (shared, None), "{}: typed on {row}", case.name);
            assert!(!two_size_rows(&s, case.title), "{}: typed on {row}, got {:?}", case.name, row_labels(&s, case.title));
            assert_eq!(s.app().hist.undo_depth(), depth + 1, "{}: typed on {row}: one undo step", case.name);
        }
    }
}

#[test]
fn a_typed_size_that_meets_the_other_side_falls_back_to_one_size_at_once() {
    for case in size_cases() {
        let mut s = open(&case.fx);
        let (typed, expected) = if case.x { ("7", (7.0, None)) } else { ("3", (3.0, None)) };
        type_into(&mut s, case.key, typed);
        assert_eq!(sizes(&s), expected, "{}", case.name);
        assert!(!two_size_rows(&s, case.title), "{}: {:?}", case.name, row_labels(&s, case.title));
    }
}

#[test]
fn a_size_scrub_in_an_effect_style_popover_keeps_both_sides_and_is_one_undo_step() {
    for case in [noise_x(), texture_y()] {
        let mut s = open_style(&case.fx);
        let depth = s.app().hist.undo_depth();
        let (dx, frames, expected) = if case.x { (1.0, 7, (10.0, Some(7.0))) } else { (-1.0, 6, (3.0, Some(1.0))) };
        scrub(&mut s, case.key, dx, frames, |s, k| {
            if k >= 4 {
                assert!(two_size_rows(s, "Edit style"), "{}: move {k} still shows the X and Y rows, got {:?}", case.name, row_labels(s, "Edit style"));
            }
        });
        assert_eq!(style_sizes(&s), expected, "{}", case.name);
        assert_eq!(s.app().hist.undo_depth(), depth + 1, "{}: one undo step for the whole scrub", case.name);
        s.cmd_key(Key::Z);
        assert_eq!(style_sizes(&s), (3.0, Some(7.0)), "{}: one undo takes the whole scrub back", case.name);
    }
}

#[test]
fn an_effect_style_scrub_that_ends_where_the_two_sides_meet_keeps_both_rows_and_is_one_undo_step() {
    for case in [noise_x(), texture_y()] {
        let mut s = open_style(&case.fx);
        let depth = s.app().hist.undo_depth();
        let shared = scrub_to_the_other_side(&mut s, &case);
        assert_eq!(style_sizes(&s), (shared, Some(shared)), "{}: the scrub leaves the sides as they are", case.name);
        assert!(two_size_rows(&s, "Edit style"), "{}: both rows stay, got {:?}", case.name, row_labels(&s, "Edit style"));
        assert_eq!(s.app().hist.undo_depth(), depth + 1, "{}: one undo step for the whole scrub", case.name);
        s.cmd_key(Key::Z);
        assert_eq!(style_sizes(&s), (3.0, Some(7.0)), "{}: one undo takes the whole scrub back", case.name);
    }
}

#[test]
fn a_typed_size_in_an_effect_style_after_a_scrub_that_ended_on_the_other_side_falls_back_to_one_size_in_one_step() {
    for case in [noise_x(), texture_y()] {
        for row in [case.key, case.other] {
            let mut s = open_style(&case.fx);
            let shared = scrub_to_the_other_side(&mut s, &case);
            let depth = s.app().hist.undo_depth();
            type_into(&mut s, row, &shared_as_typed(shared));
            assert_eq!(style_sizes(&s), (shared, None), "{}: typed on {row}", case.name);
            assert!(!two_size_rows(&s, "Edit style"), "{}: typed on {row}, got {:?}", case.name, row_labels(&s, "Edit style"));
            assert_eq!(s.app().hist.undo_depth(), depth + 1, "{}: typed on {row}: one undo step", case.name);
        }
    }
}

#[test]
fn opening_a_popover_that_stores_two_equal_sides_changes_nothing() {
    for fx in [noise(mono(), Some(3.0)), texture(Some(3.0))] {
        let s = open(&fx);
        let depth = s.app().hist.undo_depth();
        assert_eq!(sizes(&s), (3.0, Some(3.0)), "the stored sides are left as they are");
        assert_eq!(s.app().hist.undo_depth(), depth, "no undo step for opening it");
    }
}
