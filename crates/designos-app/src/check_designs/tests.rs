//! Literal fixtures for every Check designs rule, plus the fix paths through the real undo stack.

use super::contrast::{contrast_ratio, required_ratio};
use super::*;
use crate::app::App;
use designos_doc::{
    Effect, FrameProps, GradientStop, Node, NodeKind, Paint, PaintKind, Rect, Style, StyleMeta, StyleProperty, TextStyle, Variable, VariableCollection,
    VariableKind, VariableMode, VariableValue, Vec2,
};

const BLUE: Color = Color { r: 26.0 / 255.0, g: 115.0 / 255.0, b: 232.0 / 255.0, a: 1.0 };

fn gray(v: u8) -> Color {
    Color::rgb(v, v, v)
}

struct Fx {
    doc: Document,
    page: NodeId,
}

fn fx() -> Fx {
    let mut doc = Document::new("check");
    doc.check_designs.disabled_rules.insert(Rule::TextStyles.figma_id().into());
    let page = doc.add_page("Page 1");
    Fx { doc, page }
}

impl Fx {
    /// How many layers `parent` already holds. Frames and texts are laid out side by side and one
    /// under the other, because a layer that overlaps an earlier sibling is not contrast-checked unless that sibling is a
    /// plain opaque cover.
    fn slot(&self, parent: NodeId) -> f32 {
        self.doc.get(parent).map_or(0, |p| p.children.len()) as f32
    }

    fn frame(&mut self, parent: NodeId, name: &str, fill: Option<Color>) -> NodeId {
        let id = self.doc.alloc_id();
        let mut n = Node::frame(id, Rect::new(300.0 * self.slot(parent), 0.0, 200.0, 120.0));
        n.name = name.into();
        n.fills = fill.map(Paint::solid).into_iter().collect();
        self.doc.insert(parent, n).unwrap()
    }

    fn rect(&mut self, parent: NodeId, name: &str, fill: Color) -> NodeId {
        let id = self.doc.alloc_id();
        let mut n = Node::rectangle(id, Rect::new(10.0, 10.0, 40.0, 40.0));
        n.name = name.into();
        n.fills = vec![Paint::solid(fill)];
        self.doc.insert(parent, n).unwrap()
    }

    fn text(&mut self, parent: NodeId, content: &str, size: f32, weight: u16, color: Color) -> NodeId {
        let id = self.doc.alloc_id();
        let style = TextStyle { font_size: size, font_weight: weight, fills: vec![Paint::solid(color)], ..TextStyle::default() };
        let mut n = Node::text(id, Rect::new(8.0, 8.0 + 24.0 * self.slot(parent), 100.0, 20.0), content, style);
        n.name = content.into();
        self.doc.insert(parent, n).unwrap()
    }

    /// A colour variable with one value per mode of the shared "Theme" collection (light default, dark).
    fn color_var(&mut self, name: &str, light: Color, dark: Color) -> String {
        self.theme_var(name, VariableKind::Color, VariableValue::Color(light), VariableValue::Color(dark))
    }

    /// A number variable with one value per mode of the shared "Theme" collection.
    fn number_var(&mut self, name: &str, light: f32, dark: f32) -> String {
        self.theme_var(name, VariableKind::Number, VariableValue::Number(light), VariableValue::Number(dark))
    }

    fn theme_var(&mut self, name: &str, kind: VariableKind, light: VariableValue, dark: VariableValue) -> String {
        let coll = self.doc.variable_collections.entry("theme".into()).or_insert_with(|| VariableCollection {
            id: "theme".into(),
            name: "Theme".into(),
            modes: vec![VariableMode { id: "light".into(), name: "Light".into() }, VariableMode { id: "dark".into(), name: "Dark".into() }],
            default_mode: "light".into(),
            variables: vec![],
            key: None,
            source_library_key: None,
        });
        let id = format!("var:{name}");
        coll.variables.push(id.clone());
        let values = [("light".to_string(), light), ("dark".to_string(), dark)].into_iter().collect();
        self.doc.variables.insert(
            id.clone(),
            Variable {
                id: id.clone(),
                name: name.into(),
                collection: "theme".into(),
                kind,
                values,
                description: String::new(),
                scopes: vec![],
                hidden_from_publishing: false,
                key: None,
                code_syntax: Default::default(),
            },
        );
        id
    }

    fn style(&mut self, key: &str, name: &str, style: Style, property: StyleProperty) {
        self.doc.styles.insert(key.into(), style);
        self.doc.style_meta.insert(key.into(), StyleMeta { name: name.into(), property, ..StyleMeta::default() });
    }

    fn report(&self) -> Report {
        audit(&self.doc, &Scope::Page(self.page))
    }

    fn app(self) -> App {
        let mut app = App::new(self.doc);
        app.page = self.page;
        app
    }
}

fn only(report: &Report, rule: Rule) -> Finding {
    let all: Vec<&Finding> = report.of(rule).collect();
    assert_eq!(all.len(), 1, "expected exactly one {rule:?} finding, got {:#?}", report.findings);
    all[0].clone()
}

fn shadow(radius: f32) -> Effect {
    Effect::DropShadow { color: Color::rgba(0, 0, 0, 64), offset: Vec2::new(0.0, 4.0), radius, spread: 0.0, visible: true, show_behind: false }
}

// ---------------------------------------------------------------- hard-coded values

#[test]
fn an_unbound_fill_equal_to_a_variable_suggests_binding_it() {
    let mut f = fx();
    let r = f.rect(f.page, "Card", BLUE);
    let var = f.color_var("Color/Primary", BLUE, BLUE);
    let found = only(&f.report(), Rule::HardCodedValue);
    assert_eq!(found.node, r);
    assert_eq!(found.fix, Some(Fix::BindVariable { key: "fills/0".into(), variable: var }));
    assert_eq!(found.message, "Fill #1A73E8 matches variable \"Color/Primary\"");
    assert_eq!(found.fix_label, "Bind to variable Color/Primary");
}

#[test]
fn a_fill_one_channel_off_a_variable_is_not_flagged() {
    let mut f = fx();
    f.rect(f.page, "Card", Color::rgb(26, 115, 233));
    f.color_var("Color/Primary", BLUE, BLUE);
    assert!(f.report().findings.is_empty());
}

#[test]
fn a_fill_already_bound_or_linked_to_a_style_is_not_flagged() {
    let mut f = fx();
    let bound = f.rect(f.page, "Bound", BLUE);
    let linked = f.rect(f.page, "Linked", BLUE);
    let var = f.color_var("Color/Primary", BLUE, BLUE);
    f.style("Brand", "Brand/Primary", Style::Fill(vec![Paint::solid(BLUE)]), StyleProperty::Fill);
    f.doc.get_mut(bound).unwrap().variables.bindings.insert("fills/0".into(), var);
    f.doc.get_mut(linked).unwrap().style_refs.insert("fill".into(), "Brand".into());
    assert!(f.report().findings.is_empty());
}

#[test]
fn a_variable_is_matched_in_the_mode_in_effect_at_the_layer() {
    let mut f = fx();
    let frame = f.frame(f.page, "Dark frame", None);
    f.rect(frame, "Card", gray(0x20));
    f.color_var("Surface", Color::WHITE, gray(0x20));
    assert!(f.report().findings.is_empty(), "light mode: #202020 is not the variable's value");
    f.doc.get_mut(frame).unwrap().variables.modes.insert("theme".into(), "dark".into());
    let found = only(&f.report(), Rule::HardCodedValue);
    assert_eq!(found.message, "Fill #202020 matches variable \"Surface\"");
}

#[test]
fn a_variable_whose_scope_excludes_the_property_is_not_suggested() {
    let mut f = fx();
    f.rect(f.page, "Card", BLUE);
    let var = f.color_var("Color/Border", BLUE, BLUE);
    f.doc.variables.get_mut(&var).unwrap().scopes = vec!["STROKE_COLOR".into()];
    assert!(f.report().findings.is_empty());
}

#[test]
fn a_fill_list_equal_to_a_fill_style_suggests_the_style_when_no_variable_matches() {
    let mut f = fx();
    let r = f.rect(f.page, "Card", BLUE);
    f.style("Brand", "Brand/Primary", Style::Fill(vec![Paint::solid(BLUE)]), StyleProperty::Fill);
    let found = only(&f.report(), Rule::HardCodedValue);
    assert_eq!(found.node, r);
    assert_eq!(found.fix, Some(Fix::ApplyStyle { style: "Brand".into() }));
    assert_eq!(found.message, "Fill matches style \"Brand/Primary\"");
}

#[test]
fn a_stroke_is_matched_against_stroke_variables_and_stroke_styles() {
    let mut f = fx();
    let a = f.rect(f.page, "A", gray(0xF0));
    let b = f.rect(f.page, "B", gray(0xF1));
    for id in [a, b] {
        f.doc.get_mut(id).unwrap().stroke.weight = 1.0;
    }
    f.doc.get_mut(a).unwrap().stroke.paints = vec![Paint::solid(BLUE)];
    f.doc.get_mut(b).unwrap().stroke.paints = vec![Paint::solid(gray(0x33))];
    let var = f.color_var("Color/Primary", BLUE, BLUE);
    f.style("Line", "Line/Strong", Style::Fill(vec![Paint::solid(gray(0x33))]), StyleProperty::Stroke);
    let report = f.report();
    let fa = report.findings.iter().find(|x| x.node == a).unwrap();
    assert_eq!(fa.fix, Some(Fix::BindVariable { key: "strokes/0".into(), variable: var }));
    assert_eq!(fa.message, "Stroke #1A73E8 matches variable \"Color/Primary\"");
    let fb = report.findings.iter().find(|x| x.node == b).unwrap();
    assert_eq!(fb.fix, Some(Fix::ApplyStyle { style: "Line".into() }));
    assert_eq!(report.findings.len(), 2, "the stroke style must not match the fills");
}

#[test]
fn text_equal_to_a_text_style_apart_from_its_colour_suggests_the_style() {
    let mut f = fx();
    f.doc.check_designs.disabled_rules.remove(Rule::TextStyles.figma_id());
    let white = f.frame(f.page, "Surface", Some(Color::WHITE));
    let t = f.text(white, "Heading", 32.0, 700, gray(0x11));
    let ts = TextStyle { font_size: 32.0, font_weight: 700, ..TextStyle::default() };
    f.style("H1", "Heading/H1", Style::Text(ts), StyleProperty::Text);
    let report = f.report();
    let found = only(&report, Rule::TextStyles);
    assert_eq!(found.node, t);
    assert_eq!(found.fix, Some(Fix::ApplyStyle { style: "H1".into() }));
    assert_eq!(found.message, "Text matches style \"Heading/H1\"");
    let mut f = fx();
    f.doc.check_designs.disabled_rules.remove(Rule::TextStyles.figma_id());
    let white = f.frame(f.page, "Surface", Some(Color::WHITE));
    f.text(white, "Heading", 31.0, 700, gray(0x11));
    f.style("H1", "Heading/H1", Style::Text(TextStyle { font_size: 32.0, font_weight: 700, ..TextStyle::default() }), StyleProperty::Text);
    let unstyled = only(&f.report(), Rule::TextStyles);
    assert_eq!(unstyled.fix, None);
    assert_eq!(unstyled.message, "No text style");
}

#[test]
fn effects_equal_to_an_effect_style_suggest_the_style() {
    let mut f = fx();
    let r = f.rect(f.page, "Card", gray(0xEE));
    f.doc.get_mut(r).unwrap().effects = vec![shadow(8.0)];
    f.style("Shadow", "Shadow/Card", Style::Effect(vec![shadow(8.0)]), StyleProperty::Effect);
    let found = only(&f.report(), Rule::HardCodedValue);
    assert_eq!(found.fix, Some(Fix::ApplyStyle { style: "Shadow".into() }));
    assert_eq!(found.message, "Effects match style \"Shadow/Card\"");
    f.doc.get_mut(r).unwrap().effects = vec![shadow(9.0)];
    assert!(f.report().findings.is_empty());
}

// ---------------------------------------------------------------- contrast

#[test]
fn contrast_ratio_matches_the_wcag_reference_values() {
    assert!((contrast_ratio(Color::BLACK, Color::WHITE) - 21.0).abs() < 1e-6);
    assert!((contrast_ratio(Color::WHITE, Color::WHITE) - 1.0).abs() < 1e-9);
    let grey_777 = contrast_ratio(gray(0x77), Color::WHITE);
    assert!((4.47..4.49).contains(&grey_777), "#777 on white is 4.478:1, got {grey_777}");
    let grey_767676 = contrast_ratio(gray(0x76), Color::WHITE);
    assert!((4.53..4.55).contains(&grey_767676), "#767676 on white is 4.542:1, got {grey_767676}");
    assert_eq!(contrast_ratio(gray(0x77), Color::WHITE), contrast_ratio(Color::WHITE, gray(0x77)), "order does not matter");
}

#[test]
fn large_text_needs_three_to_one_and_everything_else_four_and_a_half() {
    use designos_doc::ContrastLevel;
    assert_eq!(required_ratio(16.0, 400, ContrastLevel::AA), 4.5);
    assert_eq!(required_ratio(23.9, 400, ContrastLevel::AA), 4.5);
    assert_eq!(required_ratio(24.0, 400, ContrastLevel::AA), 3.0);
    assert_eq!(required_ratio(18.66, 700, ContrastLevel::AA), 3.0);
    assert_eq!(required_ratio(18.0, 700, ContrastLevel::AA), 4.5);
}

#[test]
fn text_just_under_aa_is_flagged_and_just_over_is_not() {
    let mut f = fx();
    let frame = f.frame(f.page, "Card", Some(Color::WHITE));
    let low = f.text(frame, "Low", 14.0, 400, gray(0x77));
    f.text(frame, "Ok", 14.0, 400, gray(0x76));
    let found = only(&f.report(), Rule::Contrast);
    assert_eq!(found.node, low);
    assert_eq!(found.message, "Contrast 4.47:1 is below 4.5:1 (WCAG AA)");
    assert!(matches!(found.fix, Some(Fix::SetTextColor { .. })));
}

#[test]
fn large_text_uses_the_three_to_one_threshold() {
    let mut f = fx();
    let frame = f.frame(f.page, "Card", Some(Color::WHITE));
    f.text(frame, "Passes", 24.0, 400, gray(0x94));
    let fails = f.text(frame, "Fails", 24.0, 400, gray(0x99));
    let found = only(&f.report(), Rule::Contrast);
    assert_eq!(found.node, fails);
    assert!(found.message.ends_with("is below 3:1 (WCAG AA)"), "{}", found.message);
}

#[test]
fn the_backdrop_is_composited_from_the_page_through_every_ancestor_fill() {
    let mut f = fx();
    // No ancestor fill: the page colour (#5A5A5A) is the backdrop.
    let t = f.text(f.page, "On page", 14.0, 400, gray(0x70));
    assert_eq!(only(&f.report(), Rule::Contrast).node, t);
    // A half transparent white frame lightens the page to about #ADADAD, where #303030 passes AA.
    let frame = f.frame(f.page, "Veil", Some(Color::rgba(255, 255, 255, 128)));
    let inside = f.text(frame, "Inside", 14.0, 400, gray(0x30));
    let report = f.report();
    assert!(report.findings.iter().all(|x| x.node != inside), "#303030 on #ADADAD is about 5.9:1");
    // The same frame at 30% opacity barely lightens the page, so #707070 still fails inside it.
    f.doc.get_mut(frame).unwrap().fills = vec![Paint::solid(Color::rgba(255, 255, 255, 77))];
    let weak = f.text(frame, "Weak", 14.0, 400, gray(0x70));
    assert!(f.report().findings.iter().any(|x| x.node == weak));
}

#[test]
fn a_gradient_backdrop_is_unknown_so_nothing_is_flagged() {
    let mut f = fx();
    let frame = f.frame(f.page, "Hero", None);
    f.doc.get_mut(frame).unwrap().fills = vec![Paint {
        kind: PaintKind::LinearGradient {
            handles: [Vec2::new(0.0, 0.5), Vec2::new(1.0, 0.5), Vec2::new(0.0, 1.0)],
            stops: vec![GradientStop { pos: 0.0, color: Color::WHITE }, GradientStop { pos: 1.0, color: gray(0xEE) }],
        },
        opacity: 1.0,
        visible: true,
        blend: designos_doc::BlendMode::Normal,
    }];
    f.text(frame, "Low", 14.0, 400, gray(0xCC));
    assert!(f.report().findings.is_empty());
}

#[test]
fn hidden_empty_and_mixed_colour_text_is_not_checked() {
    let mut f = fx();
    let frame = f.frame(f.page, "Card", Some(Color::WHITE));
    let hidden = f.text(frame, "Hidden", 14.0, 400, gray(0xCC));
    f.doc.get_mut(hidden).unwrap().visible = false;
    f.text(frame, "   ", 14.0, 400, gray(0xCC));
    let mixed = f.text(frame, "Mixed", 14.0, 400, gray(0xCC));
    if let NodeKind::Text(t) = &mut f.doc.get_mut(mixed).unwrap().kind {
        let mut run_style = t.style.clone();
        run_style.fills = vec![Paint::solid(Color::BLACK)];
        t.runs = vec![designos_doc::TextRun { start: 0, end: 2, style: run_style }];
    }
    assert!(f.report().findings.is_empty());
}

#[test]
fn the_suggested_text_colour_meets_aa_and_keeps_its_direction() {
    let mut f = fx();
    let dark = f.frame(f.page, "Dark", Some(gray(0x1E)));
    f.text(dark, "Low", 14.0, 400, gray(0x40));
    let light = f.frame(f.page, "Light", Some(Color::WHITE));
    f.text(light, "Low", 14.0, 400, gray(0xAA));
    let report = f.report();
    let mut fixes = Vec::new();
    for found in report.of(Rule::Contrast) {
        let Some(Fix::SetTextColor { color }) = found.fix else { panic!("no colour fix: {found:?}") };
        fixes.push((found.node, color));
    }
    assert_eq!(fixes.len(), 2);
    assert!(contrast_ratio(fixes[0].1, gray(0x1E)) >= 4.5 && fixes[0].1.r > gray(0x40).r, "on dark the text gets lighter");
    assert!(contrast_ratio(fixes[1].1, Color::WHITE) >= 4.5 && fixes[1].1.r < gray(0xAA).r, "on white the text gets darker");
}

#[test]
fn a_bound_failing_text_colour_has_no_fix_because_the_variable_owns_it() {
    let mut f = fx();
    let frame = f.frame(f.page, "Card", Some(Color::WHITE));
    let t = f.text(frame, "Low", 14.0, 400, gray(0x77));
    let var = f.color_var("Gray/500", gray(0x77), gray(0x77));
    f.doc.get_mut(t).unwrap().variables.bindings.insert("fills/0".into(), var);
    let found = only(&f.report(), Rule::Contrast);
    assert_eq!(found.fix, None);
    assert!(found.message.contains("Gray/500"), "{}", found.message);
}

// ---------------------------------------------------------------- detached instances

fn component(f: &mut Fx, source_id: &str) -> NodeId {
    let id = f.doc.alloc_id();
    let mut n = Node::frame(id, Rect::new(300.0, 0.0, 80.0, 40.0));
    n.name = "Button".into();
    n.kind = NodeKind::Component(FrameProps::default());
    n.source_id = Some(source_id.into());
    f.doc.insert(f.page, n).unwrap()
}

/// A detached copy of `comp` on the page: the same tree with its component bookkeeping replaced by
/// the `detachedSymbolId` provenance a `.fig` import leaves, named `name`, at (40, 60).
fn detached_copy(f: &mut Fx, comp: NodeId, name: &str, symbol: &str) -> NodeId {
    detached_copy_in(f, comp, f.page, name, symbol)
}

/// [`detached_copy`] placed under `parent`, which may itself be a main component.
fn detached_copy_in(f: &mut Fx, comp: NodeId, parent: NodeId, name: &str, symbol: &str) -> NodeId {
    fn copy(f: &mut Fx, src: NodeId, parent: NodeId) -> NodeId {
        let mut n = f.doc.get(src).unwrap().clone();
        let kids = std::mem::take(&mut n.children);
        n.id = f.doc.alloc_id();
        n.source_id = None;
        if let NodeKind::Component(p) = n.kind.clone() {
            n.kind = NodeKind::Frame(p);
        }
        let id = f.doc.insert(parent, n).unwrap();
        for k in kids {
            copy(f, k, id);
        }
        id
    }
    let id = copy(f, comp, parent);
    let n = f.doc.get_mut(id).unwrap();
    n.name = name.into();
    n.transform.tx = 40.0;
    n.transform.ty = 60.0;
    n.component.detached_symbol_id = Some(symbol.into());
    id
}

#[test]
fn a_frame_detached_from_a_component_in_the_file_offers_to_reconnect() {
    let mut f = fx();
    let comp = component(&mut f, "12:34");
    let frame = detached_copy(&mut f, comp, "Save", "guid:12:34");
    let found = only(&f.report(), Rule::DetachedInstance);
    assert_eq!(found.node, frame);
    assert_eq!(found.message, "Detached from component \"Button\"");
    assert_eq!(found.fix, Some(Fix::Reconnect { component: comp }));
    assert_eq!(found.fix_label, "Reconnect to Button");
}

#[test]
fn detached_from_a_published_library_component_is_reported_without_a_fix() {
    let mut f = fx();
    let frame = f.frame(f.page, "Save", Some(Color::WHITE));
    f.doc.get_mut(frame).unwrap().detached_symbol_id = Some("assetRef:abc123:7".into());
    let found = only(&f.report(), Rule::DetachedInstance);
    assert_eq!(found.fix, None);
    assert_eq!(found.message, "Detached from a component that is not in this file");
}

#[test]
fn an_ordinary_frame_and_an_instance_are_not_detached() {
    let mut f = fx();
    component(&mut f, "12:34");
    f.frame(f.page, "Plain", Some(Color::WHITE));
    assert!(f.report().findings.is_empty());
}

// ---------------------------------------------------------------- traversal

#[test]
fn hidden_layers_and_instance_contents_are_skipped_and_scope_is_honoured() {
    let mut f = fx();
    f.color_var("Color/Primary", BLUE, BLUE);
    let hidden = f.rect(f.page, "Hidden", BLUE);
    f.doc.get_mut(hidden).unwrap().visible = false;
    let inner = f.rect(f.page, "Inside instance", BLUE);
    f.doc.get_mut(inner).unwrap().component.source = Some(hidden);
    let live = f.rect(f.page, "Live", BLUE);
    let report = f.report();
    assert_eq!(report.findings.iter().map(|x| x.node).collect::<Vec<_>>(), vec![live]);
    assert_eq!(report.scanned, 1);
    let scoped = audit(&f.doc, &Scope::Layers(vec![hidden, live]));
    assert_eq!(scoped.findings.len(), 1, "a hidden scope root is skipped too");
    assert!(audit(&f.doc, &Scope::Layers(vec![])).findings.is_empty());
}

// ---------------------------------------------------------------- fixes

fn snapshot(app: &App, ids: &[NodeId]) -> Vec<Option<Node>> {
    ids.iter().map(|id| app.doc.get(*id).cloned()).collect()
}

fn fix_of(app: &App, rule: Rule) -> Finding {
    only(&audit(&app.doc, &Scope::Page(app.page)), rule)
}

#[test]
fn binding_a_variable_is_one_undo_step() {
    let mut f = fx();
    let r = f.rect(f.page, "Card", BLUE);
    let var = f.color_var("Color/Primary", BLUE, BLUE);
    let mut app = f.app();
    let before = snapshot(&app, &[r]);
    let finding = fix_of(&app, Rule::HardCodedValue);
    assert!(app.check_designs_fix(&finding));
    assert_eq!(app.doc.get(r).unwrap().variables.bindings.get("fills/0"), Some(&var));
    assert!(audit(&app.doc, &Scope::Page(app.page)).findings.is_empty(), "the finding is gone after the fix");
    app.undo();
    assert_eq!(snapshot(&app, &[r]), before);
    assert!(!app.hist.can_undo(), "exactly one undo step");
}

#[test]
fn applying_fill_text_and_effect_styles_links_them_in_one_step_each() {
    let mut f = fx();
    f.doc.check_designs.disabled_rules.remove(Rule::TextStyles.figma_id());
    let r = f.rect(f.page, "Card", gray(0xEE));
    f.doc.get_mut(r).unwrap().effects = vec![shadow(8.0)];
    let white = f.frame(f.page, "Surface", Some(Color::WHITE));
    let t = f.text(white, "Heading", 32.0, 700, gray(0x22));
    f.style("Brand", "Brand/Primary", Style::Fill(vec![Paint::solid(gray(0xEE))]), StyleProperty::Fill);
    f.style("Shadow", "Shadow/Card", Style::Effect(vec![shadow(8.0)]), StyleProperty::Effect);
    f.style("H1", "Heading/H1", Style::Text(TextStyle { font_size: 32.0, font_weight: 700, ..TextStyle::default() }), StyleProperty::Text);
    let mut app = f.app();
    let report = audit(&app.doc, &Scope::Page(app.page));
    assert_eq!(report.findings.len(), 3, "{:#?}", report.findings);
    let before = snapshot(&app, &[r, t]);
    for found in &report.findings {
        assert!(app.check_designs_fix(found), "{found:?}");
    }
    let n = app.doc.get(r).unwrap();
    assert_eq!(n.style_refs.get("fill").map(String::as_str), Some("Brand"));
    assert_eq!(n.style_refs.get("effect").map(String::as_str), Some("Shadow"));
    assert_eq!(app.doc.get(t).unwrap().style_refs.get("text").map(String::as_str), Some("H1"));
    assert!(audit(&app.doc, &Scope::Page(app.page)).findings.is_empty());
    for _ in 0..3 {
        app.undo();
    }
    assert_eq!(snapshot(&app, &[r, t]), before);
    assert!(!app.hist.can_undo(), "three fixes, three undo steps");
}

#[test]
fn the_contrast_fix_recolours_the_text_and_undoes_in_one_step() {
    let mut f = fx();
    let frame = f.frame(f.page, "Card", Some(Color::WHITE));
    let t = f.text(frame, "Low", 14.0, 400, gray(0x77));
    let mut app = f.app();
    let before = snapshot(&app, &[t]);
    let finding = fix_of(&app, Rule::Contrast);
    assert!(app.check_designs_fix(&finding));
    assert!(audit(&app.doc, &Scope::Page(app.page)).findings.is_empty());
    assert_ne!(snapshot(&app, &[t]), before);
    app.undo();
    assert_eq!(snapshot(&app, &[t]), before);
    assert!(!app.hist.can_undo());
}

#[test]
fn reconnecting_swaps_the_frame_for_an_instance_in_place_and_undoes_in_one_step() {
    let mut f = fx();
    let comp = component(&mut f, "12:34");
    let before_sibling = f.frame(f.page, "Before", None);
    let frame = detached_copy(&mut f, comp, "Save", "guid:12:34");
    let after_sibling = f.frame(f.page, "After", None);
    let mut app = f.app();
    let page = app.page;
    let order_before = app.doc.get(page).unwrap().children.clone();
    let finding = fix_of(&app, Rule::DetachedInstance);
    assert!(app.check_designs_fix(&finding));
    assert!(app.doc.get(frame).is_none(), "the detached frame is gone");
    let kids = app.doc.get(page).unwrap().children.clone();
    let pos = kids.iter().position(|k| *k == before_sibling).unwrap() + 1;
    assert_eq!(kids[pos + 1], after_sibling, "same slot in the stacking order");
    let inst = app.doc.get(kids[pos]).unwrap();
    assert!(matches!(inst.kind, NodeKind::Instance { component, .. } if component == Some(comp)));
    assert_eq!((inst.transform.tx, inst.transform.ty), (40.0, 60.0));
    assert_eq!(inst.name, "Save", "the layer keeps its own name");
    assert!(audit(&app.doc, &Scope::Page(page)).findings.is_empty());
    app.undo();
    assert_eq!(app.doc.get(page).unwrap().children, order_before);
    assert!(app.doc.get(frame).is_some());
    assert!(!app.hist.can_undo());
}

#[test]
fn fix_all_is_one_undo_step_and_a_contrast_fix_outranks_a_bind_it_made_stale() {
    let mut f = fx();
    let frame = f.frame(f.page, "Card", Some(Color::WHITE));
    let low = f.text(frame, "Low", 14.0, 400, gray(0x77));
    let swatch = f.rect(f.page, "Swatch", BLUE);
    f.color_var("Gray/500", gray(0x77), gray(0x77));
    let blue = f.color_var("Color/Primary", BLUE, BLUE);
    let mut app = f.app();
    let before = snapshot(&app, &[low, swatch]);
    let report = audit(&app.doc, &Scope::Page(app.page));
    assert_eq!(report.findings.len(), 3, "{:#?}", report.findings);
    let applied = app.check_designs_fix_all(&report.findings);
    assert_eq!(applied, 2, "the Gray/500 bind would put the failing colour back, so it is skipped");
    assert!(app.doc.get(low).unwrap().variables.bindings.is_empty());
    assert_eq!(app.doc.get(swatch).unwrap().variables.bindings.get("fills/0"), Some(&blue));
    assert!(audit(&app.doc, &Scope::Page(app.page)).findings.is_empty());
    app.undo();
    assert_eq!(snapshot(&app, &[low, swatch]), before);
    assert!(!app.hist.can_undo(), "Fix all is a single undo step");
}

#[test]
fn a_fix_for_a_layer_that_changed_since_the_audit_is_refused_and_leaves_no_undo_step() {
    let mut f = fx();
    let r = f.rect(f.page, "Card", BLUE);
    f.color_var("Color/Primary", BLUE, BLUE);
    let mut app = f.app();
    let finding = fix_of(&app, Rule::HardCodedValue);
    app.doc.get_mut(r).unwrap().fills = vec![Paint::solid(gray(0x10))];
    assert!(!app.check_designs_fix(&finding), "the fill no longer equals the variable");
    assert!(app.doc.get(r).unwrap().variables.bindings.is_empty());
    assert!(!app.hist.can_undo());
    assert_eq!(app.check_designs_fix_all(&[]), 0);
}

fn white_page(f: &mut Fx) {
    f.doc.get_mut(f.page).unwrap().kind = NodeKind::Page { background: Color::WHITE };
}

fn place(f: &mut Fx, id: NodeId, x: f32, y: f32) {
    let n = f.doc.get_mut(id).unwrap();
    n.transform.tx = x;
    n.transform.ty = y;
}

fn resize(f: &mut Fx, id: NodeId, w: f32, h: f32) {
    f.doc.get_mut(id).unwrap().size = Vec2::new(w, h);
}

mod backdrop;
mod covering_sibling;
mod fix_all;
mod instances;
mod libraries;
mod not_painted_as_computed;
mod numbers;
mod outside_the_box;
mod painted_behind;
mod resolved_styles;
mod rules;
mod safety;
mod scale;
mod subtree_bounds;
mod text_run_fills;
mod under_an_opaque_fill;
