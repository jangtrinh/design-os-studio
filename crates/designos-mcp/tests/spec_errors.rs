use designos_mcp::engine::Engine;
use designos_mcp::tools::errors::ErrorCode;
use serde_json::{json, Value};

#[test]
fn apply_spec_reports_field_and_expected_type_in_plain_words() {
    let mut e = Engine::default();
    let cases: &[(Value, &str)] = &[
        (
            json!({"parent_id": 1, "spec": {"name": "Card", "children": [[{"kind": "rectangle", "name": "Box"}]]}}),
            "children must be an array of NodeSpec objects",
        ),
        (json!({"parent_id": 1, "spec": {"name": "Card", "children": [[]]}}), "children must be an array of NodeSpec objects"),
        (json!({"parent_id": 1, "spec": {"name": "Card", "children": "nope"}}), "children must be an array of NodeSpec objects"),
        (json!({"parent_id": 1, "spec": [{"kind": "frame", "name": "Card"}]}), "spec must be a NodeSpec object"),
        (json!({"parent_id": 1, "spec": {"name": "Card", "layout": "row"}}), "layout must be a LayoutSpec object"),
        (json!({"parent_id": 1, "spec": {"name": "Card", "layout": []}}), "layout must be a LayoutSpec object"),
        (json!({"parent_id": 1, "spec": {"name": "Card", "overrides": [[{"path": "Title"}]]}}), "overrides must be an array of OverrideSpec objects"),
        (json!({"parent_id": 1, "spec": {"name": "Card", "overrides": [[]]}}), "overrides must be an array of OverrideSpec objects"),
        (json!({"parent_id": 1, "spec": {"name": "Card", "in_progress": "yes"}}), "in_progress must be a boolean or an in_progress object"),
        (json!({"parent_id": 1, "spec": {"name": "Card", "in_progress": []}}), "in_progress must be a boolean or an in_progress object"),
        (json!({"parent_id": 1, "spec": {"name": "Card", "width": true}}), "width must be a number or a \"var:...\" string"),
        (json!({"parent_id": 1, "spec": {"name": "Card", "opacity": []}}), "opacity must be a number between 0 and 1 or a \"var:...\" string"),
        (json!({"parent_id": 1, "spec": {"name": "Card", "corner_radii": [8, 8]}}), "corner_radii must be an array of 4 numbers or \"var:...\" strings"),
        (json!({"parent_id": 1, "spec": {"name": "Card", "layout": {"padding": 16}}}), "padding must be an array of 4 numbers or \"var:...\" strings"),
        (json!({"parent_id": 1, "spec": {"name": "Card", "visible": 123}}), "visible must be a boolean or a \"var:...\" string"),
        (json!({"parent_id": 1, "spec": {"name": "Card", "id": "abc"}}), "id must be a non-negative integer"),
        (json!({"parent_id": 1, "spec": {"name": "Card", "font_weight": "bold"}}), "font_weight must be a number between 100 and 900"),
        (json!({"parent_id": 1, "spec": {"name": "Card", "col_span": "two"}}), "col_span must be a positive integer"),
        (json!({"parent_id": 1, "spec": {"name": "Card", "layout": {"grid_columns": "three"}}}), "grid_columns must be a positive integer"),
        (json!({"parent_id": 1, "spec": {"name": "Card", "x": "left"}}), "x must be a number"),
        (json!({"parent_id": 1, "spec": {"name": "Card", "min_width": true}}), "min_width must be a number"),
        (json!({"parent_id": 1, "spec": {"name": "Card", "clips_content": "yes"}}), "clips_content must be a boolean"),
        (json!({"parent_id": 1, "spec": {"name": "Card", "variant": ["md"]}}), "variant must be an object"),
        (json!({"parent_id": 1, "spec": {"name": "Card", "mode": ["dark"]}}), "mode must be an object"),
        (json!({"parent_id": 1, "spec": {"name": "Card", "props": []}}), "props must be an object"),
        (json!({"parent_id": 1, "spec": {"name": "Card", "annotations": "one"}}), "annotations must be an array of strings"),
        (json!({"parent_id": 1, "spec": {"name": "Card", "comments": [[]]}}), "comments must be an array of comment thread objects"),
        (json!({"parent_id": "root", "spec": {"name": "Card"}}), "parent_id must be a non-negative integer"),
    ];

    let banned_rust_types = [
        "expected u64",
        "expected u32",
        "expected u16",
        "expected f32",
        "NumberOrVar",
        "BoolOrVar",
        "InProgressSpec",
        "struct LayoutSpec",
        "struct NodeSpec",
        "struct OverrideSpec",
        "tuple of size 4",
    ];

    for (args, expected_phrase) in cases {
        let err = e.call("designos_apply_spec", args.clone()).expect_err(&format!("args {args} must be refused"));
        assert_eq!(err.code, ErrorCode::InvalidArguments, "for {args}");
        assert_eq!(err.path, "", "path must remain unchanged in 0.6.2 for {args}");
        assert!(err.message.contains(expected_phrase), "for {args}, expected {expected_phrase:?} in {:?}", err.message);
        for banned in banned_rust_types {
            assert!(!err.message.contains(banned), "for {args}, error must not name Rust type {banned:?}: {:?}", err.message);
        }
    }
}
