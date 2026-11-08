//! A grid frame's stroke width and padding bound to variables: the export keeps the variable references
//! and still folds the stroke the solver reserves into the padding, from the value the variable
//! resolves to (here 6 for the weight and 20 for the padding, against 2 and 8 stored).

use designos_doc::*;
use designos_export::{css_for, tailwind_for};

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
