use designos_doc::{ColorProfile, Document};
use designos_fig::{decode_fig_to_json, export_fig, import_fig, import_fig_node_changes};
use serde_json::{json, Value};

fn guid(s: u32, l: u32) -> Value {
    json!({"sessionID": s, "localID": l})
}

fn changes_with_profile(profile: Option<&str>) -> Value {
    let mut document = json!({"guid": guid(0, 0), "phase": "CREATED", "type": "DOCUMENT", "name": "Document"});
    if let Some(p) = profile {
        document["documentColorProfile"] = json!(p);
    }
    json!([
        document,
        {"guid": guid(0, 1), "phase": "CREATED", "parentIndex": {"guid": guid(0, 0), "position": "!"}, "type": "CANVAS", "name": "Page 1"},
    ])
}

#[test]
fn display_p3_document_profile_is_read_from_the_file() {
    let doc = import_fig_node_changes(&changes_with_profile(Some("DISPLAY_P3")), "Wide").unwrap();
    assert_eq!(doc.color_profile, ColorProfile::DisplayP3);
}

#[test]
fn srgb_legacy_and_missing_profiles_read_as_srgb() {
    for p in [Some("SRGB"), Some("LEGACY"), None] {
        let doc = import_fig_node_changes(&changes_with_profile(p), "Plain").unwrap();
        assert_eq!(doc.color_profile, ColorProfile::Srgb, "{p:?}");
    }
}

#[test]
fn display_p3_round_trips_through_a_saved_fig() {
    let mut doc = Document::new("Wide");
    doc.add_page("Page 1");
    doc.color_profile = ColorProfile::DisplayP3;
    let bytes = export_fig(&doc).unwrap();
    let json = decode_fig_to_json(&bytes).unwrap().to_string();
    assert!(json.contains("DISPLAY_P3"), "the DOCUMENT record must state documentColorProfile");
    assert_eq!(import_fig(&bytes).unwrap().color_profile, ColorProfile::DisplayP3);
}

#[test]
fn an_srgb_document_writes_no_profile_key() {
    let mut doc = Document::new("Plain");
    doc.add_page("Page 1");
    let bytes = export_fig(&doc).unwrap();
    let json = decode_fig_to_json(&bytes).unwrap().to_string();
    assert!(!json.contains("documentColorProfile"), "sRGB files must be written exactly as before");
    assert_eq!(import_fig(&bytes).unwrap().color_profile, ColorProfile::Srgb);
}
