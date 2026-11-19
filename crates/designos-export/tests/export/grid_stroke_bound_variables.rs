//! A grid frame's stroke width and padding bound to variables: the export keeps the variable references
//! and still folds the stroke the solver reserves into the padding, from the value the variable
//! resolves to (here 6 for the weight and 20 for the padding, against 2 and 8 stored).

use designos_doc::*;
use designos_export::{css_for, react_for, tailwind_for};

fn number_variable(id: &str, name: &str, px: f32) -> Variable {
    Variable {
        id: id.into(),
        name: name.into(),
        collection: "c".into(),
        kind: VariableKind::Number,
        values: [("m".to_string(), VariableValue::Number(px))].into_iter().collect(),
        description: String::new(),
        scopes: vec![],
        hidden_from_publishing: false,
        key: None,
        code_syntax: Default::default(),
        extensions: Default::default(),
        mode_extensions: Default::default(),
    }
}

/// A 200 x 100 grid frame with a bound stroke weight (resolves to 6) and, when `bind_padding`, a bound
/// left padding (resolves to 20). Returns the document and the grid.
fn grid(version: u8, included: bool, align: StrokeAlign, bind_padding: bool) -> (Document, NodeId) {
    let mut doc = Document::new("bound-grid-stroke");
    let page = doc.add_page("Page 1");
    doc.variable_collections.insert(
        "c".into(),
        VariableCollection {
            id: "c".into(),
            name: "Tokens".into(),
            modes: vec![VariableMode { id: "m".into(), name: "Mode".into() }],
            default_mode: "m".into(),
            variables: vec!["weight".into(), "pad".into()],
            key: None,
            source_library_key: None,
            extensions: Default::default(),
        },
    );
    doc.variables.insert("weight".into(), number_variable("weight", "Stroke/Width", 6.0));
    doc.variables.insert("pad".into(), number_variable("pad", "Space/Pad", 20.0));
    let id = doc.alloc_id();
    let mut frame = Node::frame(id, Rect::new(0.0, 0.0, 200.0, 100.0));
    frame.fills.clear();
    frame.stroke.paints = vec![Paint::solid(Color::rgb(0x33, 0x33, 0x33))];
    frame.stroke.weight = 2.0;
    frame.stroke.align = align;
    frame.variables.bindings.insert("strokeWeight".into(), "weight".into());
    if bind_padding {
        frame.variables.bindings.insert("paddingLeft".into(), "pad".into());
    }
    let al = AutoLayout {
        direction: LayoutDirection::Grid,
        grid_columns: 2,
        padding: [8.0; 4],
        layout_version: version,
        stroke_included: included,
        primary_sizing: Sizing::Fixed,
        counter_sizing: Sizing::Fixed,
        ..Default::default()
    };
    frame.kind = NodeKind::Frame(FrameProps { clips_content: false, auto_layout: Some(al), ..Default::default() });
    let grid = doc.insert(page, frame).unwrap();
    (doc, grid)
}

fn decl(css: &str, prop: &str) -> String {
    let pre = format!("{prop}: ");
    css.lines().find_map(|l| l.strip_prefix(&pre)).map(|v| v.trim_end_matches(';').to_string()).unwrap_or_else(|| format!("<no {prop} in {css}>"))
}

fn classes(html: &str) -> Vec<String> {
    let line = html.lines().next().unwrap_or_default();
    let start = line.find("class=\"").map(|i| i + 7).unwrap_or(0);
    let end = line[start..].find('"').map(|i| start + i).unwrap_or(start);
    line[start..end].split_whitespace().map(String::from).collect()
}

#[test]
fn a_bound_center_stroke_pads_by_half_the_resolved_weight() {
    let (doc, g) = grid(0, true, StrokeAlign::Center, false);
    let css = css_for(&doc, g);
    // 8 stored padding + half of the resolved weight 6 = 11
    assert_eq!(decl(&css, "padding"), "11px", "{css}");
    assert_eq!(decl(&css, "outline"), "var(--Stroke-Width, 6px) solid #333", "{css}");
    assert_eq!(decl(&css, "outline-offset"), "calc(-0.5 * var(--Stroke-Width, 6px))", "{css}");
    let tw = classes(&tailwind_for(&doc, g));
    for want in ["p-[11px]", "outline", "outline-[length:var(--Stroke-Width)]", "outline-offset-[calc(-0.5*var(--Stroke-Width))]"] {
        assert!(tw.iter().any(|c| c == want), "{want} missing in {tw:?}");
    }
}

#[test]
fn a_bound_inside_stroke_the_solver_does_not_reserve_is_an_outline_offset_by_its_weight() {
    let (doc, g) = grid(1, false, StrokeAlign::Inside, false);
    let css = css_for(&doc, g);
    assert_eq!(decl(&css, "padding"), "8px", "{css}");
    assert_eq!(decl(&css, "outline-offset"), "calc(-1 * var(--Stroke-Width, 6px))", "{css}");
    assert!(!css.lines().any(|l| l.starts_with("border")), "{css}");
    let tw = classes(&tailwind_for(&doc, g));
    assert!(tw.iter().any(|c| c == "outline-offset-[calc(-1*var(--Stroke-Width))]"), "{tw:?}");
    assert!(!tw.iter().any(|c| c.starts_with("border")), "{tw:?}");
}

#[test]
fn bound_padding_carries_the_stroke_as_a_calc() {
    let (doc, g) = grid(0, true, StrokeAlign::Center, true);
    let css = css_for(&doc, g);
    assert_eq!(decl(&css, "padding"), "11px 11px 11px calc(var(--Space-Pad, 20px) + 3px)", "{css}");
    let tw = classes(&tailwind_for(&doc, g));
    for want in ["py-[11px]", "pr-[11px]"] {
        assert!(tw.iter().any(|c| c == want), "{want} missing in {tw:?}");
    }
    assert!(tw.iter().any(|c| c == "pl-[calc(var(--Space-Pad)+3px)]"), "{tw:?}");
}

#[test]
fn an_included_inside_stroke_stays_a_border_with_the_stored_padding() {
    let (doc, g) = grid(0, true, StrokeAlign::Inside, false);
    let css = css_for(&doc, g);
    assert_eq!(decl(&css, "padding"), "8px", "{css}");
    assert_eq!(decl(&css, "border"), "var(--Stroke-Width, 6px) solid #333", "{css}");
    assert!(!css.contains("outline"), "{css}");
}

fn color_variable() -> Variable {
    Variable {
        id: "stroke-color".into(),
        name: "Color/Stroke".into(),
        collection: "c".into(),
        kind: VariableKind::Color,
        values: [("m".to_string(), VariableValue::Color(Color::rgb(0x33, 0x33, 0x33)))].into_iter().collect(),
        description: String::new(),
        scopes: vec![],
        hidden_from_publishing: false,
        key: None,
        code_syntax: Default::default(),
        extensions: Default::default(),
        mode_extensions: Default::default(),
    }
}

/// A version 1 grid with an Inside stroke of sides 4/1/2/3 (the solver reserves nothing), stored padding
/// `padding` on every side, and the stroke colour bound to a variable.
fn sided_grid(padding: f32) -> (Document, NodeId) {
    let (mut doc, g) = grid(1, false, StrokeAlign::Inside, false);
    doc.variables.insert("stroke-color".into(), color_variable());
    doc.variable_collections.get_mut("c").unwrap().variables.push("stroke-color".into());
    let n = doc.get_mut(g).unwrap();
    n.variables.bindings.clear();
    n.stroke.side_weights = Some([4.0, 1.0, 2.0, 3.0]);
    n.variables.bindings.insert("strokes/0".into(), "stroke-color".into());
    if let NodeKind::Frame(f) = &mut n.kind {
        f.auto_layout.as_mut().unwrap().padding = [padding; 4];
    }
    (doc, g)
}

#[test]
fn an_inset_shadow_stroke_keeps_the_bound_colour_variable() {
    let (doc, g) = sided_grid(8.0);
    let css = css_for(&doc, g);
    assert!(decl(&css, "box-shadow").contains("var(--Color-Stroke"), "{css}");
    assert!(!css.lines().any(|l| l.starts_with("border")), "{css}");
    let tw = tailwind_for(&doc, g);
    assert!(tw.contains("shadow-[inset_0px_4px_0px_0px_var(--Color-Stroke)"), "{tw}");
}

#[test]
fn a_per_side_stroke_wider_than_the_padding_is_still_an_inset_shadow_with_its_colour_variable() {
    let (doc, g) = sided_grid(2.0);
    let css = css_for(&doc, g);
    assert!(decl(&css, "box-shadow").contains("var(--Color-Stroke"), "{css}");
    assert_eq!(decl(&css, "padding"), "2px", "the solver reserves nothing: the stored padding stays: {css}");
    assert!(!css.lines().any(|l| l.starts_with("border")), "{css}");
    let tw = tailwind_for(&doc, g);
    assert!(tw.contains("shadow-[inset_0px_4px_0px_0px_var(--Color-Stroke)") && !tw.contains("border-t-"), "{tw}");
}

#[test]
fn the_inset_shadow_does_not_depend_on_the_padding() {
    for padding in [0.0, 3.0, 4.0, 8.0] {
        let (doc, g) = sided_grid(padding);
        let css = css_for(&doc, g);
        assert!(css.contains("box-shadow: inset") && !css.contains("border-width"), "padding {padding}: {css}");
    }
}

/// Padding `[4, -5, 4, 4]` under sides `4/0/4/4`: a negative side makes the whole CSS `padding` shorthand
/// invalid, so the shadow model clamps every side, in CSS and in Tailwind alike.
#[test]
fn negative_padding_on_an_unstroked_side_of_a_shadow_stroke_stays_valid_in_css_and_tailwind() {
    let (mut doc, g) = sided_grid(4.0);
    let n = doc.get_mut(g).unwrap();
    n.stroke.side_weights = Some([4.0, 0.0, 4.0, 4.0]);
    if let NodeKind::Frame(f) = &mut n.kind {
        f.auto_layout.as_mut().unwrap().padding = [4.0, -5.0, 4.0, 4.0];
    }
    let css = css_for(&doc, g);
    assert_eq!(decl(&css, "padding"), "4px 0px 4px 4px", "{css}");
    let tw = classes(&tailwind_for(&doc, g));
    assert!(!tw.iter().any(|c| c.contains("[-")), "{tw:?}");
    assert!(tw.iter().any(|c| c == "pl-1") && tw.iter().any(|c| c == "py-1"), "{tw:?}");
    assert!(!tw.iter().any(|c| c.starts_with("pr-")), "the clamped right side is 0 and dropped: {tw:?}");
}

#[test]
fn per_side_inside_stroke_with_bound_side_widths_uses_inset_shadow_variables() {
    let (mut doc, g) = sided_grid(0.0);
    doc.variables.insert("w-top".into(), number_variable("w-top", "Border/Top", 4.0));
    doc.variables.insert("w-right".into(), number_variable("w-right", "Border/Right", 1.0));
    let vars = &mut doc.variable_collections.get_mut("c").unwrap().variables;
    vars.push("w-top".into());
    vars.push("w-right".into());
    let n = doc.get_mut(g).unwrap();
    n.variables.bindings.clear();
    n.variables.bindings.insert("strokeTopWeight".into(), "w-top".into());
    n.variables.bindings.insert("strokeRightWeight".into(), "w-right".into());
    let css = css_for(&doc, g);
    assert_eq!(
        decl(&css, "box-shadow"),
        "inset 0px var(--Border-Top, 4px) 0 0 #333, inset calc(-1 * var(--Border-Right, 1px)) 0px 0 0 #333, inset 0px -2px 0 0 #333, inset 3px 0px 0 0 #333",
        "{css}"
    );
    assert!(!css.lines().any(|l| l.starts_with("border") || l.starts_with("box-sizing")), "{css}");
    let tw = tailwind_for(&doc, g);
    assert!(
        tw.contains(
            "shadow-[inset_0px_var(--Border-Top)_0px_0px_#333333,inset_calc(-1*var(--Border-Right))_0px_0px_0px_#333333,inset_0px_-2px_0px_0px_#333333,inset_3px_0px_0px_0px_#333333]"
        ),
        "{tw}"
    );
    let tw_classes = classes(&tw);
    assert!(!tw_classes.iter().any(|c| c.starts_with("border") || c == "box-border"), "{tw_classes:?}");
}

#[test]
fn dashed_per_side_inside_stroke_wider_than_padding_keeps_content_in_place_and_notes_css_limit() {
    let (mut doc, g) = sided_grid(0.0);
    let n = doc.get_mut(g).unwrap();
    n.variables.bindings.clear();
    n.stroke.dashes = vec![4.0, 2.0];
    let css = css_for(&doc, g);
    assert!(
        css.contains("/* CSS cannot draw a per-side dashed stroke without a border that would shift grid content; drawn as a solid inset box-shadow */"),
        "{css}"
    );
    assert_eq!(decl(&css, "box-shadow"), "inset 0px 4px 0 0 #333, inset -1px 0px 0 0 #333, inset 0px -2px 0 0 #333, inset 3px 0px 0 0 #333", "{css}");
    assert!(!css.lines().any(|l| l.starts_with("border") || l.starts_with("box-sizing")), "{css}");
    let tw = tailwind_for(&doc, g);
    assert!(
        tw.contains("<!-- CSS cannot draw a per-side dashed stroke without a border that would shift grid content; drawn as a solid inset box-shadow -->"),
        "{tw}"
    );
    assert!(
        tw.contains("shadow-[inset_0px_4px_0px_0px_#333333,inset_-1px_0px_0px_0px_#333333,inset_0px_-2px_0px_0px_#333333,inset_3px_0px_0px_0px_#333333]"),
        "{tw}"
    );
    let tw_classes = classes(&tw);
    assert!(!tw_classes.iter().any(|c| c.starts_with("border") || c == "box-border"), "{tw_classes:?}");
}

#[test]
fn gradient_per_side_inside_stroke_wider_than_padding_keeps_content_in_place_and_notes_css_limit() {
    let (mut doc, g) = sided_grid(0.0);
    let n = doc.get_mut(g).unwrap();
    n.variables.bindings.clear();
    n.stroke.paints = vec![Paint {
        kind: PaintKind::LinearGradient {
            stops: vec![GradientStop { pos: 0.0, color: Color::rgb(0x33, 0x33, 0x33) }, GradientStop { pos: 1.0, color: Color::rgb(0x99, 0x99, 0x99) }],
            handles: [Vec2::new(0.0, 0.0), Vec2::new(1.0, 0.0), Vec2::new(0.0, 1.0)],
        },
        opacity: 1.0,
        visible: true,
        blend: BlendMode::Normal,
    }];
    let css = css_for(&doc, g);
    assert!(
        css.contains("/* CSS cannot draw a per-side gradient stroke without a border that would shift grid content; drawn as a solid inset box-shadow */"),
        "{css}"
    );
    assert_eq!(decl(&css, "box-shadow"), "inset 0px 4px 0 0 #333, inset -1px 0px 0 0 #333, inset 0px -2px 0 0 #333, inset 3px 0px 0 0 #333", "{css}");
    assert!(!css.lines().any(|l| l.starts_with("border") || l.starts_with("box-sizing")), "{css}");
    let tw = tailwind_for(&doc, g);
    assert!(
        tw.contains("<!-- CSS cannot draw a per-side gradient stroke without a border that would shift grid content; drawn as a solid inset box-shadow -->"),
        "{tw}"
    );
    assert!(
        tw.contains("shadow-[inset_0px_4px_0px_0px_#333333,inset_-1px_0px_0px_0px_#333333,inset_0px_-2px_0px_0px_#333333,inset_3px_0px_0px_0px_#333333]"),
        "{tw}"
    );
    let tw_classes = classes(&tw);
    assert!(!tw_classes.iter().any(|c| c.starts_with("border") || c == "box-border"), "{tw_classes:?}");
}

#[test]
fn a_stroke_only_a_shader_paints_still_pads_by_the_width_the_solver_reserves() {
    let (mut doc, g) = grid(0, true, StrokeAlign::Inside, false);
    let n = doc.get_mut(g).unwrap();
    n.variables.bindings.clear();
    n.stroke.weight = 4.0;
    n.stroke.paints = vec![Paint { kind: PaintKind::Shader(ShaderRef::default()), opacity: 1.0, visible: true, blend: BlendMode::Normal }];
    let css = css_for(&doc, g);
    assert_eq!(decl(&css, "padding"), "12px", "{css}");
}

#[test]
fn per_side_widths_win_over_a_uniform_stroke_weight_variable() {
    let (mut doc, g) = sided_grid(0.0);
    let n = doc.get_mut(g).unwrap();
    n.variables.bindings.clear();
    n.variables.bindings.insert("strokeWeight".into(), "weight".into());
    let css = css_for(&doc, g);
    assert_eq!(decl(&css, "box-shadow"), "inset 0px 4px 0 0 #333, inset -1px 0px 0 0 #333, inset 0px -2px 0 0 #333, inset 3px 0px 0 0 #333", "{css}");
}

#[test]
fn a_uniform_gradient_inside_stroke_exports_as_before() {
    let (mut doc, g) = grid(1, false, StrokeAlign::Inside, false);
    let n = doc.get_mut(g).unwrap();
    n.variables.bindings.clear();
    n.stroke.weight = 4.0;
    if let NodeKind::Frame(f) = &mut n.kind {
        f.auto_layout.as_mut().unwrap().padding = [0.0; 4];
    }
    n.stroke.paints = vec![Paint {
        kind: PaintKind::LinearGradient {
            stops: vec![GradientStop { pos: 0.0, color: Color::rgb(0x33, 0x33, 0x33) }, GradientStop { pos: 1.0, color: Color::rgb(0x99, 0x99, 0x99) }],
            handles: [Vec2::new(0.0, 0.0), Vec2::new(1.0, 0.0), Vec2::new(0.0, 1.0)],
        },
        opacity: 1.0,
        visible: true,
        blend: BlendMode::Normal,
    }];
    let css = css_for(&doc, g);
    assert!(!css.contains("/*") && !css.contains("box-shadow"), "{css}");
    assert!(css.contains("border-image"), "{css}");
}

#[test]
fn the_limit_note_does_not_leak_into_react_style_entries() {
    let (mut doc, g) = sided_grid(0.0);
    let n = doc.get_mut(g).unwrap();
    n.variables.bindings.clear();
    n.stroke.dashes = vec![4.0, 2.0];
    let react = react_for(&doc, g);
    assert!(!react.contains("/*") && !react.contains("CSS cannot"), "{react}");
    assert!(react.contains("boxShadow: 'inset 0px 4px 0 0 #333, inset -1px 0px 0 0 #333, inset 0px -2px 0 0 #333, inset 3px 0px 0 0 #333'"), "{react}");
}

#[test]
fn negative_padding_on_an_outline_stroke_stays_valid_in_css_and_tailwind() {
    let (mut doc, g) = grid(1, false, StrokeAlign::Inside, false);
    let n = doc.get_mut(g).unwrap();
    n.variables.bindings.clear();
    if let NodeKind::Frame(f) = &mut n.kind {
        f.auto_layout.as_mut().unwrap().padding = [4.0, -5.0, 4.0, 4.0];
    }
    let css = css_for(&doc, g);
    assert_eq!(decl(&css, "padding"), "4px 0px 4px 4px", "{css}");
    let tw = classes(&tailwind_for(&doc, g));
    assert!(!tw.iter().any(|c| c.starts_with('p') && c.contains("[-")), "{tw:?}");
    assert!(tw.iter().any(|c| c == "pl-1") && tw.iter().any(|c| c == "py-1"), "{tw:?}");
    assert!(!tw.iter().any(|c| c.starts_with("pr-")), "the clamped right side is 0 and dropped: {tw:?}");
}
