//! The settings popovers of glass, noise and texture list the rows Figma shows, in Figma's order, and
//! show a second size row only when the two sizes differ.

use super::*;
use crate::panels::effects::{effect_rows, PopKind, PopState};
use crate::widgets::field::Prefix;
use designos_doc::Effect;
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
fn noise_opacity_row_only_for_multitone() {
    let cases = [
        (mono(), vec!["Mode", "Color", "Size", "Density"]),
        (duo(), vec!["Mode", "Color 1", "Color 2", "Size", "Density"]),
        (json!("Multi"), vec!["Mode", "Size", "Density", "Opacity"]),
    ];
    for (mode, labels) in cases {
        let fx = noise(mode.clone(), None);
        let s = open(&fx);
        assert_eq!(row_labels(&s, "Noise"), labels, "mode {mode}");
        assert_eq!(effect_rows(&fx), labels.len(), "mode {mode}");
        // a row that is not shown never rewrites the value it hides
        let welcome = s.by_name("Welcome");
        let Effect::Noise(n) = &s.app().doc.get(welcome).unwrap().effects[0] else { panic!("not a noise") };
        assert_eq!(n.opacity, 0.5, "mode {mode}");
    }
}

#[test]
fn unequal_noise_size_shows_x_and_y_rows() {
    let unequal = noise(mono(), Some(7.0));
    let s = open(&unequal);
    assert_eq!(row_labels(&s, "Noise"), ["Mode", "Color", "Size X", "Size Y", "Density"]);
    assert_eq!(effect_rows(&unequal), 5);
    assert_distinct_prefixes(&s, &["(\"fxp-nsize\", 0)", "(\"fxp-nsize-y\", 0)", "(\"fxp-dens\", 0)"]);

    let equal = noise(mono(), None);
    assert_eq!(row_labels(&open(&equal), "Noise"), ["Mode", "Color", "Size", "Density"]);
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
