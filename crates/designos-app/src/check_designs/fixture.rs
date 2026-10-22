//! Screenshot fixture for `--fixture check-designs`: a small design system (one colour variable,
//! three shared styles, one main component) and three frames that drift from it, one per Check
//! designs rule family, plus one finding that has no safe fix. The audit and the fixes run on this
//! document exactly as on a user file.

use designos_doc::{
    Color, Document, Effect, FrameProps, Node, NodeKind, Paint, Rect, Style, StyleMeta, StyleProperty, TextStyle, Variable, VariableCollection, VariableKind,
    VariableMode, VariableValue, Vec2,
};

/// Value of `--fixture` that loads this document.
pub const FIXTURE: &str = "check-designs";

const PRIMARY: Color = Color { r: 13.0 / 255.0, g: 153.0 / 255.0, b: 1.0, a: 1.0 };

fn card_shadow() -> Effect {
    Effect::DropShadow { color: Color::rgba(0, 0, 0, 56), offset: Vec2::new(0.0, 6.0), radius: 16.0, spread: 0.0, visible: true, show_behind: false }
}

fn heading_style() -> TextStyle {
    TextStyle { font_size: 24.0, font_weight: 700, ..TextStyle::default() }
}

fn text(name: &str, content: &str, at: Rect, style: TextStyle, color: Color) -> Node {
    let mut n = Node::text(designos_doc::NodeId(0), at, content, TextStyle { fills: vec![Paint::solid(color)], ..style });
    n.name = name.into();
    n
}

pub fn document() -> Document {
    let mut d = Document::new("Check designs");
    let page = d.add_page("Page 1");
    d.variable_collections.insert(
        "brand".into(),
        VariableCollection {
            id: "brand".into(),
            name: "Brand".into(),
            modes: vec![VariableMode { id: "default".into(), name: "Default".into() }],
            default_mode: "default".into(),
            variables: vec!["primary".into()],
            key: None,
            source_library_key: None,
        },
    );
    d.variables.insert(
        "primary".into(),
        Variable {
            id: "primary".into(),
            name: "Color/Primary".into(),
            collection: "brand".into(),
            kind: VariableKind::Color,
            values: [("default".to_string(), VariableValue::Color(PRIMARY))].into_iter().collect(),
            description: String::new(),
            scopes: vec![],
            hidden_from_publishing: false,
            key: None,
            code_syntax: Default::default(),
        },
    );
    let mut style = |key: &str, name: &str, s: Style, property: StyleProperty| {
        d.styles.insert(key.into(), s);
        d.style_meta.insert(key.into(), StyleMeta { name: name.into(), property, ..StyleMeta::default() });
    };
    style("heading", "Heading/H2", Style::Text(heading_style()), StyleProperty::Text);
    style("body", "Body/Regular", Style::Text(TextStyle::default()), StyleProperty::Text);
    style("shadow", "Shadow/Card", Style::Effect(vec![card_shadow()]), StyleProperty::Effect);
    style("surface", "Surface/Card", Style::Fill(vec![Paint::solid(Color::rgb(0xF4, 0xF4, 0xF5))]), StyleProperty::Fill);

    let add = |d: &mut Document, parent, mut n: Node| {
        n.id = d.alloc_id();
        d.insert(parent, n).expect("fixture parent exists")
    };

    // Hard-coded values: a shadowed card, a heading and a button that all repeat design-system values.
    let mut card = Node::frame(designos_doc::NodeId(0), Rect::new(80.0, 80.0, 320.0, 240.0));
    card.name = "Pricing card".into();
    card.fills = vec![Paint::solid(Color::rgb(0xF4, 0xF4, 0xF5))];
    card.effects = vec![card_shadow()];
    card.corner_radii = [16.0; 4];
    let card = add(&mut d, page, card);
    add(&mut d, card, text("Plan name", "Pro plan", Rect::new(24.0, 24.0, 200.0, 32.0), heading_style(), Color::rgb(0x22, 0x22, 0x22)));
    let mut button = Node::rectangle(designos_doc::NodeId(0), Rect::new(24.0, 168.0, 272.0, 48.0));
    button.name = "Upgrade button".into();
    button.fills = vec![Paint::solid(PRIMARY)];
    button.corner_radii = [8.0; 4];
    add(&mut d, card, button);
    // Contrast: grey caption on the light card area (3.14:1) and a dark footer with dim text (1.88:1).
    let mut billing_note =
        text("Billing note", "Billed monthly, cancel any time", Rect::new(24.0, 72.0, 272.0, 20.0), TextStyle::default(), Color::rgb(0x8A, 0x8A, 0x8A));
    billing_note.style_refs.insert("text".into(), "body".into());
    add(&mut d, card, billing_note);
    let mut footer = Node::frame(designos_doc::NodeId(0), Rect::new(80.0, 360.0, 320.0, 96.0));
    footer.name = "Footer".into();
    footer.fills = vec![Paint::solid(Color::rgb(0x1E, 0x1E, 0x1E))];
    footer.corner_radii = [16.0; 4];
    let footer = add(&mut d, page, footer);
    let mut footer_note =
        text("Footer note", "Terms and conditions apply", Rect::new(24.0, 36.0, 272.0, 20.0), TextStyle::default(), Color::rgb(0x4A, 0x4A, 0x4A));
    footer_note.style_refs.insert("text".into(), "body".into());
    add(&mut d, footer, footer_note);

    // Detached instance: the Save button was detached from the main component, which is still here.
    let mut main = Node::frame(designos_doc::NodeId(0), Rect::new(480.0, 80.0, 160.0, 48.0));
    main.name = "Button".into();
    main.kind = NodeKind::Component(FrameProps::default());
    main.source_id = Some("12:34".into());
    main.fills = vec![Paint::solid(Color::rgb(0x27, 0x27, 0x2A))];
    main.corner_radii = [8.0; 4];
    add(&mut d, page, main);
    let mut save = Node::frame(designos_doc::NodeId(0), Rect::new(480.0, 168.0, 160.0, 48.0));
    save.name = "Save".into();
    save.fills = vec![Paint::solid(Color::rgb(0x27, 0x27, 0x2A))];
    save.corner_radii = [8.0; 4];
    save.component.detached_symbol_id = Some("guid:12:34".into());
    add(&mut d, page, save);
    // Detached and then resized: an instance would lose the new size, so it is reported with no fix.
    let mut cancel = Node::frame(designos_doc::NodeId(0), Rect::new(480.0, 248.0, 208.0, 48.0));
    cancel.name = "Cancel".into();
    cancel.fills = vec![Paint::solid(Color::rgb(0x27, 0x27, 0x2A))];
    cancel.corner_radii = [8.0; 4];
    cancel.component.detached_symbol_id = Some("guid:12:34".into());
    add(&mut d, page, cancel);
    d
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::check_designs::{audit, Rule, Scope};

    #[test]
    fn the_fixture_exercises_every_rule_family() {
        let d = document();
        let report = audit(&d, &Scope::Page(d.pages[0]));
        for rule in [Rule::HardCodedValue, Rule::TextStyles, Rule::Contrast, Rule::DetachedInstance] {
            assert!(report.of(rule).next().is_some(), "{rule:?} has no finding in the fixture: {:#?}", report.findings);
        }
        let unfixable: Vec<_> = report.findings.iter().filter(|f| f.fix.is_none()).map(|f| f.node_name.as_str()).collect();
        assert_eq!(unfixable, ["Cancel"], "only the edited detached frame has no fix");
        assert_eq!(report.findings.len(), 8, "{:#?}", report.findings);
        let note = report.findings.iter().find(|f| f.node_name == "Billing note").expect("the grey caption is flagged");
        assert_eq!(note.message, "Contrast 3.14:1 is below 4.5:1 (WCAG AA)", "the ratio quoted in the fixture comment");
    }
}
