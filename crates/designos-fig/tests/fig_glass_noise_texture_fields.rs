//! `.fig` effects that carry a glass splay or bevel, or lack them, and the variable bindings of effect
//! fields Studio does not carry.
use designos_doc::*;
use designos_fig::container::{self, Compression};
use designos_fig::import_fig_with_report;
use designos_fig::kiwi::KiwiValue as V;
use designos_fig::testkit::{color, en, figma_like_schema, fl, guid, msg, node, vec2};

fn fig_with_effects(effects: Vec<V>) -> Vec<u8> {
    let nodes = vec![
        node((0, 0), (0, 0), "!", "DOCUMENT", "Document", vec![]),
        node((0, 1), (0, 0), "a", "CANVAS", "Page", vec![("backgroundColor", color(1.0, 1.0, 1.0, 1.0))]),
        node((0, 2), (0, 0), "b", "CANVAS", "Internal Only Canvas", vec![("internalOnly", V::Bool(true))]),
        node((2, 10), (0, 1), "a", "FRAME", "Holder", vec![("size", vec2(100.0, 100.0)), ("effects", V::Array(effects))]),
    ];
    let message = msg(vec![("nodeChanges", V::Array(nodes))]);
    let schema = figma_like_schema();
    let root = schema.find("Message").unwrap();
    let body = schema.encode_message(root, &message).unwrap();
    container::write_container(&schema.encode(), &body, Compression::Zstd, 38)
}

fn alias(local: u32) -> V {
    msg(vec![("dataType", en("ALIAS")), ("value", msg(vec![("alias", msg(vec![("guid", guid(6, local))]))]))])
}

fn glass(extra: Vec<(&'static str, V)>) -> V {
    let mut f = vec![
        ("type", en("GLASS")),
        ("specularAngle", fl(135.0)),
        ("specularIntensity", fl(0.75)),
        ("refractionIntensity", fl(0.5)),
        ("refractionRadius", fl(20.0)),
        ("chromaticAberration", fl(0.5)),
        ("radius", fl(4.0)),
        ("visible", V::Bool(true)),
    ];
    f.extend(extra);
    msg(f)
}

fn noise(extra: Vec<(&'static str, V)>) -> V {
    let mut f = vec![
        ("type", en("NOISE")),
        ("noiseType", en("MONOTONE")),
        ("color", color(0.0, 0.0, 0.0, 1.0)),
        ("noiseSize", vec2(3.0, 7.0)),
        ("density", fl(0.5)),
        ("opacity", fl(0.5)),
        ("seed", V::Uint(1)),
        ("visible", V::Bool(true)),
    ];
    f.extend(extra);
    msg(f)
}

fn holder(doc: &Document) -> &Node {
    doc.nodes.values().find(|n| n.name == "Holder").expect("the holder frame")
}

#[test]
fn missing_splay_and_bevel_load_as_zero() {
    let (doc, _) = import_fig_with_report(&fig_with_effects(vec![glass(vec![])])).unwrap();
    let Effect::Glass(g) = &holder(&doc).effects[0] else { panic!("not a glass") };
    assert_eq!((g.splay, g.bevel), (0.0, 0.0));
}

#[test]
fn a_noise_and_a_texture_without_a_second_size_read_as_one_size() {
    let scalar = |ty: &str| msg(vec![("type", en(ty)), ("noiseSize", vec2(5.0, 5.0)), ("seed", V::Uint(1)), ("visible", V::Bool(true))]);
    let (doc, _) = import_fig_with_report(&fig_with_effects(vec![scalar("NOISE"), scalar("GRAIN")])).unwrap();
    let fx = &holder(&doc).effects;
    assert!(matches!(&fx[0], Effect::Noise(n) if n.size == 5.0 && n.size_y.is_none()));
    assert!(matches!(&fx[1], Effect::Texture(t) if t.size == 5.0 && t.size_y.is_none()));
}

#[test]
fn dropped_variable_bindings_of_glass_and_noise_are_counted() {
    let effects = vec![glass(vec![("splayVar", alias(1))]), noise(vec![("noiseSizeYVar", alias(2)), ("secondaryColorVar", alias(3))])];
    let (_, report) = import_fig_with_report(&fig_with_effects(effects)).unwrap();
    assert_eq!(report.dropped_variable_bindings, 3);
}

#[test]
fn every_glass_and_noise_binding_field_studio_does_not_carry_is_counted_once() {
    let glass_vars = ["splayVar"];
    let noise_vars = ["noiseSizeXVar", "noiseSizeYVar", "densityVar", "secondaryColorVar"];
    let effects = vec![glass(glass_vars.iter().map(|k| (*k, alias(1))).collect()), noise(noise_vars.iter().map(|k| (*k, alias(2))).collect())];
    let (_, report) = import_fig_with_report(&fig_with_effects(effects)).unwrap();
    assert_eq!(report.dropped_variable_bindings, glass_vars.len() + noise_vars.len());
}

#[test]
fn bindings_studio_carries_are_not_counted_as_dropped() {
    let effects = vec![
        glass(vec![
            ("radiusVar", alias(1)),
            ("specularAngleVar", alias(4)),
            ("specularIntensityVar", alias(5)),
            ("refractionIntensityVar", alias(6)),
            ("refractionRadiusVar", alias(7)),
            ("chromaticAberrationVar", alias(8)),
        ]),
        noise(vec![("colorVar", alias(2)), ("effectOpacityVar", alias(3))]),
    ];
    let (_, report) = import_fig_with_report(&fig_with_effects(effects)).unwrap();
    assert_eq!(report.dropped_variable_bindings, 0);
}

#[test]
fn a_file_without_effect_bindings_reports_none() {
    let (_, report) = import_fig_with_report(&fig_with_effects(vec![glass(vec![]), noise(vec![])])).unwrap();
    assert_eq!(report.dropped_variable_bindings, 0);
    assert_eq!(report.instances_left_unexpanded, 0);
}
