//! A glass saved before splay and bevel existed loads with both at zero.

use designos_doc::*;

#[test]
fn missing_splay_and_bevel_load_as_zero() {
    let old = r#"{"Glass":{"light_angle":315.0,"light_intensity":0.5,"refraction":0.5,"depth":10.0,"dispersion":0.25,"frost":4.0,"visible":true}}"#;
    let Effect::Glass(g) = serde_json::from_str::<Effect>(old).unwrap() else { panic!("not a glass") };
    assert_eq!((g.splay, g.bevel), (0.0, 0.0));
}

#[test]
fn missing_second_size_loads_as_none_for_noise_and_texture() {
    let noise = r#"{"Noise":{"mode":"Multi","size":3.0,"density":0.5,"opacity":0.25,"blend":"Normal","seed":7,"visible":true}}"#;
    let Effect::Noise(n) = serde_json::from_str::<Effect>(noise).unwrap() else { panic!("not a noise") };
    assert_eq!(n.size_y, None);
    let texture = r#"{"Texture":{"size":4.0,"radius":2.0,"clip_to_shape":true,"seed":9,"visible":true}}"#;
    let Effect::Texture(t) = serde_json::from_str::<Effect>(texture).unwrap() else { panic!("not a texture") };
    assert_eq!(t.size_y, None);
}
