//! The auto layout distribution and text wrap fields keep old files loading unchanged and stay out of
//! the compact JSON while they hold their defaults.

use designos_doc::*;

fn doc(al: AutoLayout, wrap: TextWrap) -> Document {
    let mut d = Document::new("t");
    let page = d.add_page("p");
    let fid = d.alloc_id();
    let mut f = Node::frame(fid, Rect::new(0.0, 0.0, 100.0, 100.0));
    f.kind = NodeKind::Frame(FrameProps { auto_layout: Some(al), ..Default::default() });
    let f = d.insert(page, f).unwrap();
    let tid = d.alloc_id();
    let mut t = Node::text(tid, Rect::new(0.0, 0.0, 50.0, 20.0), "hi", TextStyle::default());
    if let NodeKind::Text(tp) = &mut t.kind {
        tp.text_wrap = wrap;
    }
    d.insert(f, t).unwrap();
    d
}

#[test]
fn defaults_are_not_serialized() {
    let json = doc(AutoLayout::default(), TextWrap::Auto).to_json().unwrap();
    for key in ["counter_align_content", "layout_version", "text_wrap"] {
        assert!(!json.contains(key), "{key} leaked into {json}");
    }
}

#[test]
fn set_values_roundtrip() {
    let al = AutoLayout {
        primary_align: LayoutAlign::SpaceEvenly,
        counter_align_content: CounterAlignContent::SpaceBetween,
        layout_version: 1,
        ..Default::default()
    };
    let back = Document::from_json(&doc(al.clone(), TextWrap::Balance).to_json().unwrap()).unwrap();
    let frame = back.nodes.values().find(|n| n.kind.frame_props().is_some_and(|f| f.auto_layout.is_some())).unwrap();
    assert_eq!(frame.kind.frame_props().unwrap().auto_layout.as_ref().unwrap(), &al);
    let text = back.nodes.values().find_map(|n| if let NodeKind::Text(t) = &n.kind { Some(t) } else { None }).unwrap();
    assert_eq!(text.text_wrap, TextWrap::Balance);
}

#[test]
fn files_written_before_the_fields_existed_load_with_defaults() {
    let json = doc(AutoLayout { primary_align: LayoutAlign::SpaceBetween, wrap: true, ..Default::default() }, TextWrap::Auto).to_json().unwrap();
    let back = Document::from_json(&json).unwrap();
    let al = back.nodes.values().find_map(|n| n.kind.frame_props().and_then(|f| f.auto_layout.as_ref())).unwrap();
    assert_eq!((al.primary_align, al.wrap), (LayoutAlign::SpaceBetween, true));
    assert_eq!((al.counter_align_content, al.layout_version), (CounterAlignContent::Auto, 0));
    let text = back.nodes.values().find_map(|n| if let NodeKind::Text(t) = &n.kind { Some(t) } else { None }).unwrap();
    assert_eq!(text.text_wrap, TextWrap::Auto);
}

#[test]
fn old_designos_file_loads_as_some_and_saves_byte_identical() {
    let mut legacy_doc = doc(
        AutoLayout { direction: LayoutDirection::Horizontal, wrap: true, item_spacing: 16.0, counter_spacing: Some(12.0), ..Default::default() },
        TextWrap::Auto,
    );
    let legacy_json = legacy_doc.to_json().unwrap();
    assert!(!legacy_json.contains("counter_spacing_auto"), "explicit Some must not write marker: {legacy_json}");
    let loaded = Document::from_json(&legacy_json).unwrap();
    let al = loaded.nodes.values().find_map(|n| n.kind.frame_props().and_then(|f| f.auto_layout.as_ref())).unwrap();
    assert_eq!((al.counter_spacing, al.line_gap()), (Some(12.0), 12.0));
    assert_eq!(loaded.to_json().unwrap(), legacy_json, "saving a document whose gaps are all Some must be byte-identical to main");
    let _ = &mut legacy_doc;
}

#[test]
fn none_counter_spacing_writes_resolved_number_plus_marker_and_round_trips() {
    let d =
        doc(AutoLayout { direction: LayoutDirection::Horizontal, wrap: true, item_spacing: 24.0, counter_spacing: None, ..Default::default() }, TextWrap::Auto);
    let json = d.to_json().unwrap();
    assert!(json.contains("\"counter_spacing\":24.0"), "older Studio builds need the resolved number in counter_spacing: {json}");
    assert!(json.contains("\"counter_spacing_auto\":true"), "marker field must be written for None: {json}");
    let back = Document::from_json(&json).unwrap();
    let al = back.nodes.values().find_map(|n| n.kind.frame_props().and_then(|f| f.auto_layout.as_ref())).unwrap();
    assert_eq!((al.counter_spacing, al.line_gap()), (None, 24.0));
    assert_eq!(back.to_json().unwrap(), json);
}
