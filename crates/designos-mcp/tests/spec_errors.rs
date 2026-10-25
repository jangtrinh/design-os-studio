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

#[test]
fn round3_set_slot_bad_slot_reports_slot_field() {
    let mut e = Engine::default();
    let page = e.doc.pages[0].0;
    let card = e.call("designos_apply_spec", json!({"parent_id": page, "spec": {"kind": "frame", "name": "Card"}})).expect("create card")["root_node_id"]
        .as_u64()
        .unwrap();

    for slot_op in [
        json!({"op": "set_slot", "node": card, "slot": 5, "fill": [{"kind": "text", "text": "Body"}]}),
        json!({"op": "set_slot", "node": card, "slot": 5, "fill": false}),
        json!({"op": "set_slot", "node": card, "slot": 5, "value": [{"kind": "text", "text": "Body"}]}),
    ] {
        let err = e.call("designos_apply_batch", json!({"ops": [slot_op.clone()]})).expect_err("bad slot must fail");
        assert!(err.message.contains("slot must be a string"), "for {slot_op}, expected 'slot must be a string' in {:?}", err.message);
        assert!(!err.message.contains("fill must be a string"), "for {slot_op}, must not blame fill in {:?}", err.message);
    }
}

#[test]
fn round3_set_slot_bad_child_item_in_fill_reports_child_path() {
    let mut e = Engine::default();
    let page = e.doc.pages[0].0;
    let card = e.call("designos_apply_spec", json!({"parent_id": page, "spec": {"kind": "frame", "name": "Card"}})).expect("create card")["root_node_id"]
        .as_u64()
        .unwrap();

    let err_arr = e
        .call("designos_apply_batch", json!({"ops": [{"op": "set_slot", "node": card, "slot": "body", "fill": [[{"kind": "text", "text": "Body"}]]}]}))
        .expect_err("array of arrays in fill must fail");
    assert!(err_arr.message.contains("fill/0 must be a NodeSpec object"), "expected 'fill/0 must be a NodeSpec object' in {:?}", err_arr.message);
}

#[test]
fn round3_set_slot_bad_child_field_in_fill_reports_nested_path() {
    let mut e = Engine::default();
    let page = e.doc.pages[0].0;
    let card = e.call("designos_apply_spec", json!({"parent_id": page, "spec": {"kind": "frame", "name": "Card"}})).expect("create card")["root_node_id"]
        .as_u64()
        .unwrap();

    let err_child = e
        .call("designos_apply_batch", json!({"ops": [{"op": "set_slot", "node": card, "slot": "body", "fill": [{"kind": "text", "width": true}]}]}))
        .expect_err("bad child width in fill must fail");
    assert!(
        err_child.message.contains("fill/0/width must be a number or a \"var:...\" string"),
        "expected 'fill/0/width must be a number or a \"var:...\" string' in {:?}",
        err_child.message
    );
}

#[test]
fn round3_nested_spec_errors_carry_path_from_root() {
    let mut e = Engine::default();
    let page = e.doc.pages[0].0;

    let err_nested = e
        .call(
            "designos_apply_spec",
            json!({
                "parent_id": page,
                "spec": {
                    "kind": "frame",
                    "name": "Root",
                    "children": [{"kind": "frame", "name": "Child", "layout": {"padding": 16}}]
                }
            }),
        )
        .expect_err("nested bad padding must fail");
    assert!(
        err_nested.message.contains("children/0/layout/padding must be an array of 4 numbers or \"var:...\" strings"),
        "expected nested path in {:?}",
        err_nested.message
    );
}

#[test]
fn round3_f32_overflow_and_comments_parity_with_serde() {
    let mut e = Engine::default();
    let page = e.doc.pages[0].0;

    let err_overflow =
        e.call("designos_apply_spec", json!({"parent_id": page, "spec": {"kind": "frame", "name": "Big", "x": 1e40}})).expect_err("f32 overflow must fail");
    assert!(err_overflow.message.contains("x must be a finite number"), "expected 'x must be a finite number' in {:?}", err_overflow.message);

    let ok_comment = e.call(
        "designos_apply_spec",
        json!({
            "parent_id": page,
            "spec": {
                "kind": "frame",
                "name": "WithComment",
                "comments": [{
                    "id": "t-struct",
                    "resolved": false,
                    "page_id": page,
                    "page_name": "Page 1",
                    "world_x": 10.0,
                    "world_y": 20.0,
                    "target_node_id": null,
                    "target_node_name": null,
                    "target_node_kind": null,
                    "target_frame_id": null,
                    "target_frame_name": null,
                    "messages": [{
                        "id": "m1",
                        "author": {"id": "u1", "name": "Alice"},
                        "text": "Looks good",
                        "created": "2026-01-01T00:00:00Z",
                        "created_at": 12345
                    }]
                }]
            }
        }),
    );
    assert!(ok_comment.is_ok(), "structured comment author and extra created_at must succeed: {ok_comment:?}");

    for (bad_field, bad_val, expected_sub) in [
        ("author_color", json!("#fff"), "comments/0/messages/0/author_color must be"),
        ("author_id", json!(123), "comments/0/messages/0/author_id must be a string"),
        ("created", json!(false), "comments/0/messages/0/created must be a string"),
        ("edited", json!(99), "comments/0/messages/0/edited must be a string"),
    ] {
        let mut msg_obj = json!({
            "id": "m1",
            "author": "Alice",
            "text": "Hi",
            "created": "2026-01-01T00:00:00Z",
            "edited": null
        });
        msg_obj[bad_field] = bad_val;
        let err_c = e
            .call(
                "designos_apply_spec",
                json!({
                    "parent_id": page,
                    "spec": {
                        "kind": "frame",
                        "name": "BadComment",
                        "comments": [{
                            "id": "t-bad",
                            "resolved": false,
                            "page_id": page,
                            "page_name": "Page 1",
                            "world_x": 0.0,
                            "world_y": 0.0,
                            "messages": [msg_obj]
                        }]
                    }
                }),
            )
            .expect_err(&format!("bad comment {bad_field} must fail"));
        assert!(err_c.message.contains(expected_sub), "for {bad_field}, expected {expected_sub:?} in {:?}", err_c.message);
        for banned in ["struct Color", "f32", "sequence", "map,", "invalid type", "expected a "] {
            assert!(!err_c.message.contains(banned), "for {bad_field}, leaked {banned:?} in {:?}", err_c.message);
        }
    }

    let sample_values = [
        json!(null),
        json!(true),
        json!(false),
        json!(0),
        json!(-1),
        json!(1.5),
        json!(1e40),
        json!(""),
        json!("abc"),
        json!([]),
        json!([1]),
        json!([[]]),
        json!({}),
    ];
    let node_fields = [
        "id",
        "kind",
        "name",
        "component",
        "variant",
        "props",
        "overrides",
        "slots",
        "preset",
        "x",
        "y",
        "width",
        "height",
        "width_sizing",
        "height_sizing",
        "min_width",
        "max_width",
        "min_height",
        "max_height",
        "col_span",
        "row_span",
        "fill",
        "stroke",
        "stroke_weight",
        "stroke_width",
        "corner_radius",
        "radius",
        "corner_radii",
        "opacity",
        "clips_content",
        "visible",
        "text",
        "font_size",
        "line_height",
        "font_weight",
        "text_color",
        "color",
        "text_style",
        "effect_style",
        "grid_style",
        "mode",
        "layout",
        "dev_status",
        "annotation",
        "annotations",
        "in_progress",
        "comments",
        "children",
    ];
    let banned_tokens = [
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
        "struct Color",
        "tuple of size 4",
        "invalid type",
        "sequence",
        "map,",
        "UNKNOWN_FIELD:",
        "INVALID_SLOT_VALUE:",
    ];
    for field in node_fields {
        for v in &sample_values {
            let spec = json!({ field: v });
            let raw = serde_json::to_string(&spec).unwrap();
            let serde_ok = serde_json::from_str::<designos_doc::NodeSpec>(&raw).is_ok();
            let parsed = designos_mcp::tools::errors::parse_spec_json(&raw);
            match (serde_ok, parsed) {
                (true, Err(err)) => {
                    let is_positional_array_exception = matches!(field, "layout" | "in_progress" | "children" | "overrides" | "comments") && v.is_array();
                    let is_non_finite_f32_exception = v.as_f64().is_some_and(|n| !(n as f32).is_finite());
                    assert!(is_positional_array_exception || is_non_finite_f32_exception, "unexpected false reject on {field}={v}: {err}");
                }
                (_, Err(err)) => {
                    for b in banned_tokens {
                        assert!(!err.contains(b), "leaked {b:?} on {field}={v}: {err:?}");
                    }
                }
                _ => {}
            }
        }
    }
}

#[test]
fn round3_batch_apply_spec_runs_same_walk_as_direct_apply_spec() {
    let mut e = Engine::default();
    let page = e.doc.pages[0].0;

    let err_batch_layout = e
        .call("designos_apply_batch", json!({"ops": [{"op": "apply_spec", "parent": page, "spec": {"kind": "frame", "name": "Hero", "layout": []}}]}))
        .expect_err("batch apply_spec with layout: [] must fail");
    assert!(
        err_batch_layout.message.contains("layout must be a LayoutSpec object"),
        "expected 'layout must be a LayoutSpec object' in {:?}",
        err_batch_layout.message
    );
}
