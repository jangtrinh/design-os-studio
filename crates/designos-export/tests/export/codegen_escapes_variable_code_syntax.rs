//! Variable code-syntax strings are user input: they must not break out of the XML attribute or the
//! JS string literal they are printed into.

use designos_doc::*;
use designos_export::{android_xml_for, react_for};

/// A frame whose fill and item spacing are bound to variables carrying `brand` / `gap` as the
/// code syntax of `platform`.
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
fn android_xml_escapes_code_syntax_inside_attributes() {
    let (d, id) = doc_with_syntax(CodeSyntaxPlatform::Android, "@color/x\" android:id=\"@+id/pwn\" a=\"<&\nb", "8dp\"><Evil/><x a=\"");
    let xml = android_xml_for(&d, id);
    // the quote that would close the attribute and the tag that would open an element stay data
    assert!(xml.contains("android:background=\"@color/x&quot; android:id=&quot;@+id/pwn&quot; a=&quot;&lt;&amp;&#10;b\""), "{xml}");
    assert!(!xml.contains("<Evil/>") && !xml.contains("android:id=\"@+id/pwn\""), "{xml}");
    assert!(xml.contains("8dp&quot;&gt;&lt;Evil/&gt;&lt;x a=&quot;"), "{xml}");
    // every attribute line is a balanced `name="value"` pair: no raw newline or quote escaped out of one
    for line in xml.lines().filter(|l| l.contains("android:")) {
        assert_eq!(line.matches('"').count() % 2, 0, "unbalanced quotes: {line}");
    }
}

#[test]
fn android_xml_leaves_plain_code_syntax_untouched() {
    let (d, id) = doc_with_syntax(CodeSyntaxPlatform::Android, "@color/brand_primary", "@dimen/space_gap");
    let xml = android_xml_for(&d, id);
    assert!(xml.contains("android:background=\"@color/brand_primary\""), "{xml}");
    assert!(xml.contains("@dimen/space_gap"), "{xml}");
}

#[test]
fn react_keeps_a_newline_in_code_syntax_inside_its_string_literal() {
    let (d, id) = doc_with_syntax(CodeSyntaxPlatform::Web, "var(--brand)\nvar(--other)", "var(--gap)");
    let jsx = react_for(&d, id);
    // one escaped string, never a raw line break inside a quoted literal
    for line in jsx.lines() {
        assert_eq!(line.matches('\'').count() % 2, 0, "unterminated string literal: {line:?}\n{jsx}");
    }
    assert!(jsx.contains("var(--brand)\\nvar(--other)"), "{jsx}");
}

#[test]
fn react_escapes_quotes_and_backslashes_in_code_syntax() {
    let (d, id) = doc_with_syntax(CodeSyntaxPlatform::Web, "var(--b'rand\\x)", "var(--gap)");
    let jsx = react_for(&d, id);
    assert!(jsx.contains("var(--b\\'rand\\\\x)"), "{jsx}");
}
