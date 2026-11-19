//! #161: the blend mode of drop and inner shadows serializes compactly and old files load as Normal.

use designos_doc::*;

fn drop(blend: BlendMode) -> Effect {
    Effect::DropShadow { color: Color::BLACK, offset: Vec2::new(0.0, 4.0), radius: 8.0, spread: 0.0, visible: true, show_behind: false, blend }
}

fn inner(blend: BlendMode) -> Effect {
    Effect::InnerShadow { color: Color::BLACK, offset: Vec2::new(0.0, 4.0), radius: 8.0, spread: 0.0, visible: true, blend }
}

#[test]
fn a_shadow_without_a_blend_field_loads_as_normal() {
    // the exact bytes written before the field existed: a Normal shadow still serializes this way
    let old_drop =
        r#"{"DropShadow":{"color":{"r":0.0,"g":0.0,"b":0.0,"a":1.0},"offset":{"x":0.0,"y":4.0},"radius":8.0,"spread":0.0,"visible":true,"show_behind":false}}"#;
    let old_inner = r#"{"InnerShadow":{"color":{"r":0.0,"g":0.0,"b":0.0,"a":1.0},"offset":{"x":0.0,"y":4.0},"radius":8.0,"spread":0.0,"visible":true}}"#;
    assert_eq!(serde_json::from_str::<Effect>(old_drop).unwrap(), drop(BlendMode::Normal));
    assert_eq!(serde_json::from_str::<Effect>(old_inner).unwrap(), inner(BlendMode::Normal));
    assert_eq!(serde_json::to_string(&drop(BlendMode::Normal)).unwrap(), old_drop);
    assert_eq!(serde_json::to_string(&inner(BlendMode::Normal)).unwrap(), old_inner);
}

#[test]
fn a_blend_mode_survives_a_json_round_trip() {
    for mode in [BlendMode::Multiply, BlendMode::Screen, BlendMode::LinearBurn, BlendMode::Luminosity] {
        for e in [drop(mode), inner(mode)] {
            let json = serde_json::to_string(&e).unwrap();
            assert!(json.contains("\"blend\""), "{json}");
            assert_eq!(serde_json::from_str::<Effect>(&json).unwrap(), e);
        }
    }
}
