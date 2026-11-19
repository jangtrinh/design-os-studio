//! A bound variable prints its per-platform code syntax verbatim in the generator for that
//! platform (iOS: SwiftUI and UIKit; Android: Compose and XML; WEB: CSS) and falls back to the
//! token name otherwise.

use designos_doc::*;
use designos_export::{android_xml_for, compose_for, css_for, swiftui_for, uikit_for};

/// A horizontal auto-layout frame (bound gap, bound fill) holding one text.
fn doc_with_syntax(platform: CodeSyntaxPlatform, brand: &str, gap: &str) -> (Document, NodeId) {
    let mut d = Document::new("t");
    let page = d.add_page("p");
    d.variable_collections.insert(
        "c".into(),
        VariableCollection {
            id: "c".into(),
            name: "Tokens".into(),
            modes: vec![VariableMode { id: "m".into(), name: "Mode".into() }],
            default_mode: "m".into(),
            variables: vec!["brand".into(), "gap".into()],
            key: None,
            source_library_key: None,
            extensions: Default::default(),
        },
    );
    let var = |id: &str, name: &str, kind, value, code: &str| Variable {
        id: id.into(),
        name: name.into(),
        collection: "c".into(),
        kind,
        values: [("m".to_string(), value)].into_iter().collect(),
        description: String::new(),
        scopes: vec![],
        hidden_from_publishing: false,
        key: None,
        code_syntax: [(platform, code.to_string())].into_iter().collect(),
        extensions: Default::default(),
        mode_extensions: Default::default(),
    };
    d.variables.insert("brand".into(), var("brand", "Color/Brand", VariableKind::Color, VariableValue::Color(Color::rgb(0x33, 0x66, 0x99)), brand));
    d.variables.insert("gap".into(), var("gap", "Spacing/Gap", VariableKind::Number, VariableValue::Number(12.0), gap));

    let fid = d.alloc_id();
    let mut frame = Node::frame(fid, Rect::new(0.0, 0.0, 200.0, 80.0));
    frame.name = "Bar".into();
    frame.kind = NodeKind::Frame(FrameProps {
        auto_layout: Some(AutoLayout { direction: LayoutDirection::Horizontal, item_spacing: 12.0, ..Default::default() }),
        ..Default::default()
    });
    frame.fills = vec![Paint::solid(Color::rgb(0x33, 0x66, 0x99))];
    frame.variables.bindings.insert("fills/0".into(), "brand".into());
    frame.variables.bindings.insert("itemSpacing".into(), "gap".into());
    let frame = d.insert(page, frame).unwrap();
    for name in ["One", "Two"] {
        let tid = d.alloc_id();
        let mut t = Node::text(tid, Rect::new(0.0, 0.0, 40.0, 20.0), name, TextStyle::default());
        t.name = name.into();
        d.insert(frame, t).unwrap();
    }
    (d, frame)
}

#[test]
fn ios_syntax_is_printed_by_swiftui_and_uikit() {
    let (d, id) = doc_with_syntax(CodeSyntaxPlatform::Ios, "Theme.brand", "Theme.gap");
    let swift = swiftui_for(&d, id);
    assert!(swift.contains(".background(Theme.brand)"), "{swift}");
    assert!(swift.contains("spacing: Theme.gap)"), "{swift}");
    let uikit = uikit_for(&d, id);
    assert!(uikit.contains("bar.backgroundColor = Theme.brand"), "{uikit}");
    assert!(uikit.contains("bar.spacing = Theme.gap"), "{uikit}");
    assert!(!uikit.contains("UIColor(named:"), "{uikit}");
    // another platform's syntax never leaks in
    let (d, id) = doc_with_syntax(CodeSyntaxPlatform::Android, "R.color.brand", "R.dimen.gap");
    assert!(uikit_for(&d, id).contains("UIColor(named: \"brand\")"));
    assert!(swiftui_for(&d, id).contains("Color(\"brand\")"));
}

#[test]
fn android_syntax_is_printed_by_compose_and_xml() {
    let (d, id) = doc_with_syntax(CodeSyntaxPlatform::Android, "AppTheme.brand", "AppTheme.gap");
    let compose = compose_for(&d, id);
    assert!(compose.contains(".background(AppTheme.brand)"), "{compose}");
    assert!(compose.contains("AppTheme.gap"), "{compose}");
    let xml = android_xml_for(&d, id);
    assert!(xml.contains("android:background=\"AppTheme.brand\""), "{xml}");
    assert!(xml.contains("android:layout_marginStart=\"AppTheme.gap\""), "{xml}");
    assert!(!xml.contains("@color/"), "{xml}");
    let (d, id) = doc_with_syntax(CodeSyntaxPlatform::Ios, "Theme.brand", "Theme.gap");
    assert!(android_xml_for(&d, id).contains("android:background=\"@color/color_brand\""));
}

#[test]
fn web_syntax_is_printed_by_css_only() {
    let (d, id) = doc_with_syntax(CodeSyntaxPlatform::Web, "var(--brand, #336699)", "tokens.gap");
    let css = css_for(&d, id);
    assert!(css.contains("background: var(--brand, #336699);"), "{css}");
    assert!(css.contains("gap: tokens.gap;"), "{css}");
    assert!(!swiftui_for(&d, id).contains("--brand"));
}
