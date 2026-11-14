//! Auto layout distribution (space around / evenly), wrap line spreading, layout version and text wrap
//! through the `.fig` binary, the REST-shaped JSON and the pen format.

use designos_doc::*;
use designos_fig::{decode_fig_to_json, export_fig, export_pen_json, export_rest_json, import_fig, import_fig_node_changes, import_pen_json, import_rest_json};
use serde_json::{json, Value};

fn doc_with(al: AutoLayout, text_wrap: TextWrap) -> Document {
    let mut d = Document::new("t");
    let page = d.add_page("p");
    let fid = d.alloc_id();
    let mut f = Node::frame(fid, Rect::new(0.0, 0.0, 300.0, 200.0));
    f.name = "Stack".into();
    f.kind = NodeKind::Frame(FrameProps { auto_layout: Some(al), ..Default::default() });
    let f = d.insert(page, f).unwrap();
    let tid = d.alloc_id();
    let mut t = Node::text(tid, Rect::new(0.0, 0.0, 120.0, 40.0), "Hello wrapped world", TextStyle::default());
    t.name = "Copy".into();
    if let NodeKind::Text(tp) = &mut t.kind {
        tp.text_wrap = text_wrap;
        tp.auto_resize = TextAutoResize::Height;
    }
    d.insert(f, t).unwrap();
    d
}

fn named<'a>(doc: &'a Document, name: &str) -> &'a Node {
    doc.nodes.values().find(|n| n.name == name).unwrap_or_else(|| panic!("no node named {name}"))
}

fn auto_layout(doc: &Document) -> AutoLayout {
    named(doc, "Stack").kind.frame_props().and_then(|f| f.auto_layout.clone()).expect("auto layout")
}

fn text_wrap(doc: &Document) -> TextWrap {
    match &named(doc, "Copy").kind {
        NodeKind::Text(t) => t.text_wrap,
        other => panic!("expected text, got {other:?}"),
    }
}

/// The raw kiwi fields of the exported frame, by field name.
fn wire_node(bytes: &[u8], name: &str) -> Value {
    let json = decode_fig_to_json(bytes).unwrap();
    json["message"]["nodeChanges"].as_array().unwrap().iter().find(|n| n["name"] == name).cloned().unwrap_or_else(|| panic!("no wire node {name}"))
}

fn stack(primary: LayoutAlign, version: u8) -> AutoLayout {
    AutoLayout {
        direction: LayoutDirection::Vertical,
        wrap: true,
        item_spacing: 8.0,
        counter_spacing: 12.0,
        primary_align: primary,
        counter_align_content: CounterAlignContent::SpaceBetween,
        layout_version: version,
        ..Default::default()
    }
}

#[test]
fn space_evenly_with_layout_version_one_is_written_as_the_css_enum_and_reads_back() {
    let bytes = export_fig(&doc_with(stack(LayoutAlign::SpaceEvenly, 1), TextWrap::Auto)).unwrap();
    assert_eq!(wire_node(&bytes, "Stack")["stackPrimaryAlignItems"], "SPACE_EVENLY_CSS");
    let al = auto_layout(&import_fig(&bytes).unwrap());
    assert_eq!((al.primary_align, al.layout_version), (LayoutAlign::SpaceEvenly, 1));
}

/// Recorded decision from real-file evidence (`FIELDS.md`: real files carry SPACE_EVENLY and
/// never SPACE_BETWEEN): the legacy `StackJustify` value 3 is Figma's space-between, so SPACE_EVENLY is
/// never written back as 3, whatever the layout version.
#[test]
fn space_evenly_is_always_written_as_the_css_enum_never_the_legacy_one() {
    for version in [0, 1] {
        let bytes = export_fig(&doc_with(stack(LayoutAlign::SpaceEvenly, version), TextWrap::Auto)).unwrap();
        assert_eq!(wire_node(&bytes, "Stack")["stackPrimaryAlignItems"], "SPACE_EVENLY_CSS", "layout version {version}");
        let al = auto_layout(&import_fig(&bytes).unwrap());
        assert_eq!((al.primary_align, al.layout_version), (LayoutAlign::SpaceEvenly, version));
    }
}

/// Same decision, import side: the wire values 3 (legacy SPACE_EVENLY) and 4 (SPACE_BETWEEN) are both
/// space-between; 5 is space-around; 6 (SPACE_EVENLY_CSS) is the only space-evenly.
#[test]
fn legacy_space_evenly_on_the_wire_imports_as_space_between() {
    let bytes = export_fig(&doc_with(stack(LayoutAlign::Start, 0), TextWrap::Auto)).unwrap();
    for (wire, want) in [
        ("SPACE_EVENLY", LayoutAlign::SpaceBetween),
        ("SPACE_BETWEEN", LayoutAlign::SpaceBetween),
        ("SPACE_AROUND", LayoutAlign::SpaceAround),
        ("SPACE_EVENLY_CSS", LayoutAlign::SpaceEvenly),
    ] {
        let mut json = decode_fig_to_json(&bytes).unwrap();
        let changes = json["message"]["nodeChanges"].as_array_mut().unwrap();
        let stack_node = changes.iter_mut().find(|n| n["name"] == "Stack").unwrap();
        stack_node["stackPrimaryAlignItems"] = json!(wire);
        let doc = import_fig_node_changes(&json["message"], "t").unwrap();
        assert_eq!(auto_layout(&doc).primary_align, want, "wire {wire}");
    }
}

#[test]
fn space_around_and_space_between_roundtrip_through_fig() {
    for align in [LayoutAlign::SpaceAround, LayoutAlign::SpaceBetween] {
        let bytes = export_fig(&doc_with(stack(align, 1), TextWrap::Auto)).unwrap();
        let want = if align == LayoutAlign::SpaceAround { "SPACE_AROUND" } else { "SPACE_BETWEEN" };
        assert_eq!(wire_node(&bytes, "Stack")["stackPrimaryAlignItems"], want);
        assert_eq!(auto_layout(&import_fig(&bytes).unwrap()).primary_align, align);
    }
}

#[test]
fn counter_align_content_and_layout_version_roundtrip_through_fig() {
    let bytes = export_fig(&doc_with(stack(LayoutAlign::Start, 1), TextWrap::Auto)).unwrap();
    let wire = wire_node(&bytes, "Stack");
    assert_eq!(wire["stackCounterAlignContent"], "SPACE_BETWEEN");
    assert_eq!(wire["stackLayoutVersion"], 1);
    let al = auto_layout(&import_fig(&bytes).unwrap());
    assert_eq!(al.counter_align_content, CounterAlignContent::SpaceBetween);
    assert_eq!(al.layout_version, 1);
    assert!(al.wrap);
    assert_eq!(al.direction, LayoutDirection::Vertical);
    assert_eq!(al.counter_spacing, 12.0);
}

#[test]
fn default_stack_writes_neither_new_field() {
    let plain = AutoLayout { direction: LayoutDirection::Horizontal, ..Default::default() };
    let bytes = export_fig(&doc_with(plain, TextWrap::Auto)).unwrap();
    let wire = wire_node(&bytes, "Stack");
    assert!(wire.get("stackCounterAlignContent").is_none() && wire.get("stackLayoutVersion").is_none(), "{wire}");
    assert!(wire_node(&bytes, "Copy").get("textWrapStyle").is_none());
    let al = auto_layout(&import_fig(&bytes).unwrap());
    assert_eq!((al.counter_align_content, al.layout_version), (CounterAlignContent::Auto, 0));
}

#[test]
fn text_wrap_style_roundtrips_through_fig() {
    for (wrap, name) in [(TextWrap::Balance, "BALANCE"), (TextWrap::Pretty, "PRETTY")] {
        let bytes = export_fig(&doc_with(stack(LayoutAlign::Start, 1), wrap)).unwrap();
        assert_eq!(wire_node(&bytes, "Copy")["textWrapStyle"], name);
        assert_eq!(text_wrap(&import_fig(&bytes).unwrap()), wrap);
    }
    let bytes = export_fig(&doc_with(stack(LayoutAlign::Start, 1), TextWrap::Auto)).unwrap();
    assert_eq!(text_wrap(&import_fig(&bytes).unwrap()), TextWrap::Auto);
}

#[test]
fn rest_json_roundtrips_distribution_wrap_content_and_text_wrap() {
    for align in [LayoutAlign::SpaceAround, LayoutAlign::SpaceEvenly, LayoutAlign::SpaceBetween] {
        let json = export_rest_json(&doc_with(stack(align, 1), TextWrap::Pretty));
        let rt = import_rest_json(&json.to_string()).unwrap();
        let al = auto_layout(&rt);
        assert_eq!(al.primary_align, align);
        assert_eq!(al.counter_align_content, CounterAlignContent::SpaceBetween);
        assert_eq!(al.layout_version, 1);
        assert_eq!(text_wrap(&rt), TextWrap::Pretty);
    }
}

#[test]
fn rest_json_reads_the_figma_names() {
    let node = |extra: Value| {
        let mut frame = json!({
            "id": "1:2", "name": "Stack", "type": "FRAME", "layoutMode": "VERTICAL", "layoutWrap": "WRAP",
            "absoluteBoundingBox": {"x": 0, "y": 0, "width": 100, "height": 100},
            "children": [{"id": "1:3", "name": "Copy", "type": "TEXT", "characters": "x", "textWrapStyle": "BALANCE",
                "absoluteBoundingBox": {"x": 0, "y": 0, "width": 10, "height": 10}}]
        });
        frame.as_object_mut().unwrap().extend(extra.as_object().unwrap().clone());
        let doc = json!({"name": "f", "document": {"id": "0:0", "type": "DOCUMENT", "children": [{"id": "0:1", "name": "p", "type": "CANVAS", "children": [frame]}]}});
        import_rest_json(&doc.to_string()).unwrap()
    };
    let rt = node(json!({"primaryAxisAlignItems": "SPACE_AROUND", "counterAxisAlignContent": "SPACE_BETWEEN"}));
    let al = auto_layout(&rt);
    assert_eq!((al.primary_align, al.counter_align_content), (LayoutAlign::SpaceAround, CounterAlignContent::SpaceBetween));
    assert_eq!(text_wrap(&rt), TextWrap::Balance);
    let rt = node(json!({"primaryAxisAlignItems": "SPACE_EVENLY", "counterAxisAlignContent": "AUTO"}));
    let al = auto_layout(&rt);
    assert_eq!((al.primary_align, al.counter_align_content), (LayoutAlign::SpaceEvenly, CounterAlignContent::Auto));
}

#[test]
fn pen_format_roundtrips_the_new_alignments() {
    for align in [LayoutAlign::SpaceAround, LayoutAlign::SpaceEvenly] {
        let pen = export_pen_json(&doc_with(stack(align, 1), TextWrap::Auto)).unwrap();
        let rt = import_pen_json(&pen).unwrap();
        assert_eq!(auto_layout(&rt).primary_align, align);
    }
}

#[test]
fn partial_override_without_layout_mode_patches_stack_layout_version_and_align() {
    let changes = json!([
        {"guid": {"sessionID": 0, "localID": 0}, "type": "DOCUMENT", "name": "d"},
        {"guid": {"sessionID": 0, "localID": 1}, "parentIndex": {"guid": {"sessionID": 0, "localID": 0}, "position": "!"}, "type": "CANVAS", "name": "p"},
        {
            "guid": {"sessionID": 1, "localID": 2},
            "parentIndex": {"guid": {"sessionID": 0, "localID": 1}, "position": "!"},
            "type": "SYMBOL",
            "name": "Master",
            "size": {"x": 100.0, "y": 100.0}
        },
        {
            "guid": {"sessionID": 1, "localID": 10},
            "parentIndex": {"guid": {"sessionID": 1, "localID": 2}, "position": "!"},
            "type": "FRAME",
            "name": "InnerStack",
            "size": {"x": 100.0, "y": 100.0},
            "stackMode": "HORIZONTAL",
            "stackWrap": "WRAP",
            "stackPrimaryAlignItems": "MIN",
            "stackCounterAlignContent": "AUTO",
            "bordersTakeSpace": true
        },
        {
            "guid": {"sessionID": 1, "localID": 3},
            "parentIndex": {"guid": {"sessionID": 0, "localID": 1}, "position": "\""},
            "type": "INSTANCE",
            "name": "Inst",
            "size": {"x": 100.0, "y": 100.0},
            "symbolData": {
                "symbolID": {"sessionID": 1, "localID": 2},
                "symbolOverrides": [{
                    "guidPath": {"guids": [{"sessionID": 1, "localID": 10}]},
                    "stackPrimaryAlignItems": "SPACE_EVENLY_CSS",
                    "stackCounterAlignContent": "SPACE_BETWEEN",
                    "stackLayoutVersion": 1
                }]
            }
        }
    ]);
    let doc = import_fig_node_changes(&changes, "d").unwrap();
    let inst = doc.nodes.values().find(|n| n.name == "Inst").unwrap();
    let inner = doc.get(inst.children[0]).unwrap();
    let al = inner.kind.frame_props().unwrap().auto_layout.as_ref().unwrap();
    assert_eq!(al.layout_version, 1);
    assert_eq!(al.primary_align, LayoutAlign::SpaceEvenly);
    assert_eq!(al.counter_align_content, CounterAlignContent::SpaceBetween);
}
