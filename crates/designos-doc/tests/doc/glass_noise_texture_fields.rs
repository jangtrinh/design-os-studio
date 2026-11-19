//! Glass splay and bevel, and the second (Y) size of noise and texture: the keys are written only
//! when they carry a value, so files saved before they existed load unchanged and save byte for byte.

use designos_doc::*;

const OLD_NOISE: &str = r#"{"Noise":{"mode":{"Mono":{"color":{"r":0.0,"g":0.0,"b":0.0,"a":1.0}}},"size":3.0,"density":0.5,"opacity":0.25,"blend":"Normal","seed":7,"visible":true}}"#;
const OLD_TEXTURE: &str = r#"{"Texture":{"size":4.0,"radius":2.0,"clip_to_shape":true,"seed":9,"visible":true}}"#;
const OLD_GLASS: &str = r#"{"Glass":{"light_angle":315.0,"light_intensity":0.5,"refraction":0.5,"depth":10.0,"dispersion":0.25,"frost":4.0,"visible":true}}"#;

fn assert_saves_unchanged(json: &str) -> Effect {
    let e: Effect = serde_json::from_str(json).unwrap();
    assert_eq!(serde_json::to_string(&e).unwrap(), json);
    e
}

#[test]
fn old_designos_noise_with_scalar_size_loads_as_x_equals_y_and_saves_unchanged() {
    let e = assert_saves_unchanged(OLD_NOISE);
    assert!(matches!(e, Effect::Noise(n) if n.size == 3.0));
}

#[test]
fn old_designos_texture_and_glass_save_unchanged() {
    assert!(matches!(assert_saves_unchanged(OLD_TEXTURE), Effect::Texture(t) if t.size == 4.0));
    assert!(matches!(assert_saves_unchanged(OLD_GLASS), Effect::Glass(g) if g.depth == 10.0));
}

#[test]
fn a_second_noise_size_is_kept_and_written_next_to_the_first() {
    let with_y = OLD_NOISE.replace(r#""size":3.0,"#, r#""size":3.0,"size_y":7.0,"#);
    assert_saves_unchanged(&with_y);
}

#[test]
fn a_second_texture_size_is_kept_and_written_next_to_the_first() {
    let with_y = OLD_TEXTURE.replace(r#""size":4.0,"#, r#""size":4.0,"size_y":1.5,"#);
    assert_saves_unchanged(&with_y);
}

#[test]
fn glass_splay_and_bevel_are_kept_when_set() {
    let with_both = OLD_GLASS.replace(r#""frost":4.0,"#, r#""frost":4.0,"splay":0.25,"bevel":2.0,"#);
    assert_saves_unchanged(&with_both);
    let only_splay = OLD_GLASS.replace(r#""frost":4.0,"#, r#""frost":4.0,"splay":0.5,"#);
    assert_saves_unchanged(&only_splay);
}
