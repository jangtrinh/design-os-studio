//! A text or effect style is suggested on what the layer is painted as in its mode, so the fix
//! must accept exactly what the rule matched: a value that only equals the style after a variable
//! resolves in Dark mode is a finding whose button applies, not one that toasts "layer changed".

use super::*;

const DARK: &str = "dark";

fn dark_frame(f: &mut Fx) -> NodeId {
    let frame = f.frame(f.page, "Dark frame", Some(gray(0x1E)));
    f.doc.get_mut(frame).unwrap().variables.modes.insert("theme".into(), DARK.into());
    frame
}

fn soft_shadow(alpha: f32) -> Effect {
    Effect::DropShadow {
        color: Color { r: 0.0, g: 0.0, b: 0.0, a: alpha },
        offset: Vec2::new(0.0, 4.0),
        radius: 8.0,
        spread: 0.0,
        visible: true,
        show_behind: false,
    }
}

#[test]
fn a_text_style_matched_only_after_a_dark_mode_font_size_resolves_is_fixable() {
    let mut f = fx();
    f.doc.check_designs.disabled_rules.remove(Rule::TextStyles.figma_id());
    let size = f.number_var("Size/Title", 16.0, 32.0);
    let frame = dark_frame(&mut f);
    let title = f.text(frame, "Title", 16.0, 700, gray(0xEE));
    f.doc.get_mut(title).unwrap().variables.bindings.insert("fontSize".into(), size);
    f.style("Title", "Heading/Title", Style::Text(TextStyle { font_size: 32.0, font_weight: 700, ..TextStyle::default() }), StyleProperty::Text);
    let mut app = f.app();
    let finding = fix_of(&app, Rule::TextStyles);
    assert_eq!(finding.node, title);
    assert_eq!(finding.fix, Some(Fix::ApplyStyle { style: "Title".into() }));
    assert!(app.check_designs_fix(&finding), "the button must apply what the row offered");
    assert_eq!(app.doc.get(title).unwrap().style_refs.get("text").map(String::as_str), Some("Title"));
    assert!(audit(&app.doc, &Scope::Page(app.page)).findings.is_empty());
    app.undo();
    assert!(app.doc.get(title).unwrap().style_refs.is_empty());
}

#[test]
fn an_effect_style_matched_only_after_an_opacity_binding_resolves_is_fixable() {
    let mut f = fx();
    let fade = f.number_var("Shadow/Opacity", 25.0, 50.0);
    let frame = dark_frame(&mut f);
    let card = f.rect(frame, "Card", gray(0x30));
    {
        let n = f.doc.get_mut(card).unwrap();
        n.effects = vec![soft_shadow(0.1)];
        n.variables.bindings.insert("effects/0/opacity".into(), fade);
    }
    f.style("Shadow", "Shadow/Card", Style::Effect(vec![soft_shadow(0.5)]), StyleProperty::Effect);
    let mut app = f.app();
    let finding = fix_of(&app, Rule::HardCodedValue);
    assert_eq!(finding.node, card);
    assert_eq!(finding.fix, Some(Fix::ApplyStyle { style: "Shadow".into() }));
    assert!(app.check_designs_fix(&finding), "the button must apply what the row offered");
    assert_eq!(app.doc.get(card).unwrap().style_refs.get("effect").map(String::as_str), Some("Shadow"));
    assert!(audit(&app.doc, &Scope::Page(app.page)).findings.is_empty());
}

#[test]
fn a_style_fix_found_in_dark_is_refused_once_the_frame_is_back_in_light() {
    let mut f = fx();
    f.doc.check_designs.disabled_rules.remove(Rule::TextStyles.figma_id());
    let size = f.number_var("Size/Title", 16.0, 32.0);
    let frame = dark_frame(&mut f);
    let title = f.text(frame, "Title", 16.0, 700, gray(0xEE));
    f.doc.get_mut(title).unwrap().variables.bindings.insert("fontSize".into(), size);
    f.style("Title", "Heading/Title", Style::Text(TextStyle { font_size: 32.0, font_weight: 700, ..TextStyle::default() }), StyleProperty::Text);
    let mut app = f.app();
    let finding = fix_of(&app, Rule::TextStyles);
    app.doc.get_mut(frame).unwrap().variables.modes.clear();
    assert!(!app.check_designs_fix(&finding), "in Light the layer is 16px, so the 32px style no longer equals it");
    assert!(!app.hist.can_undo());
}
