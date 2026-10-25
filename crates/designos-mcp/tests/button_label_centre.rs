//! Regression tests for #1098 (AI-08): in an agent-built card, the button
//! label must be centred inside the button on both axes (within 1 px),
//! whether built via a button preset (`easeui-button-primary`) or via a
//! plain spec frame with `layout: { "align": "center", "justify": "center" }`.
#![allow(clippy::result_large_err)]

use designos_doc::{Document, Node, NodeId, NodeKind, Style, TextAlignH, TextAlignV, TextAutoResize, TextStyle, Vec2};
use designos_mcp::engine::Engine;
use designos_mcp::server::Session;
use serde_json::json;

fn page(e: &Engine) -> u64 {
    e.resolve_default_page().unwrap().0
}

fn find_by_name<'a>(doc: &'a Document, root: NodeId, name: &str) -> &'a Node {
    let mut found_id = None;
    doc.walk(root, |n, _| {
        if found_id.is_none() && n.name == name {
            found_id = Some(n.id);
        }
    });
    let id = found_id.unwrap_or_else(|| panic!("node {name:?} not found under {root:?}"));
    doc.get(id).expect("node exists")
}

struct CentringMetrics {
    btn_centre: (f32, f32),
    box_centre: (f32, f32),
    shaped_geom_centre: (f32, f32),
    pixel_bbox_centre: (f32, f32),
}

/// Measure the button centre, text box centre, shaped text layout centre, and
/// rendered glyph pixel bounding-box centre of `label_name` inside `button_name`.
fn measure_button_label_centring(doc: &Document, root: NodeId, button_name: &str, label_name: &str) -> CentringMetrics {
    let btn = find_by_name(doc, root, button_name);
    let lbl = find_by_name(doc, btn.id, label_name);
    let tp = match &lbl.kind {
        NodeKind::Text(t) => t,
        other => panic!("expected text node for {label_name}, got {other:?}"),
    };

    let btn_centre = (btn.size.x * 0.5, btn.size.y * 0.5);
    let box_centre = (lbl.transform.tx + lbl.size.x * 0.5, lbl.transform.ty + lbl.size.y * 0.5);

    let geom = designos_render::text_geometry(tp, lbl.size);
    let v_offset = match tp.style.align_v {
        TextAlignV::Top => 0.0,
        TextAlignV::Center => ((lbl.size.y - geom.height) * 0.5).max(0.0),
        TextAlignV::Bottom => (lbl.size.y - geom.height).max(0.0),
    };
    let h_offset = match tp.style.align_h {
        TextAlignH::Left | TextAlignH::Justified => 0.0,
        TextAlignH::Center => ((lbl.size.x - geom.width) * 0.5).max(0.0),
        TextAlignH::Right => (lbl.size.x - geom.width).max(0.0),
    };
    let shaped_geom_centre = (lbl.transform.tx + h_offset + geom.width * 0.5, lbl.transform.ty + v_offset + geom.height * 0.5);

    // Render the button cleanly and find the contrasting label glyph pixels.
    let mut doc_without_label = doc.clone();
    if let Some(n) = doc_without_label.get_mut(lbl.id) {
        n.visible = false;
    }

    let pm_with = designos_render::render_node(doc, btn.id, 1.0).expect("render button with label");
    let pm_without = designos_render::render_node(&doc_without_label, btn.id, 1.0).expect("render button without label");

    let mut min_x = u32::MAX;
    let mut min_y = u32::MAX;
    let mut max_x = 0u32;
    let mut max_y = 0u32;
    let w = pm_with.width();
    let h = pm_with.height();
    let d1 = pm_with.data();
    let d2 = pm_without.data();
    for y in 0..h {
        for x in 0..w {
            let idx = ((y * w + x) * 4) as usize;
            let dr = (d1[idx] as i32 - d2[idx] as i32).abs();
            let dg = (d1[idx + 1] as i32 - d2[idx + 1] as i32).abs();
            let db = (d1[idx + 2] as i32 - d2[idx + 2] as i32).abs();
            if dr.max(dg).max(db) > 32 {
                min_x = min_x.min(x);
                min_y = min_y.min(y);
                max_x = max_x.max(x);
                max_y = max_y.max(y);
            }
        }
    }
    assert!(min_x <= max_x && min_y <= max_y, "label {label_name:?} produced no visible pixels inside button {button_name:?}");
    let pixel_bbox_centre = ((min_x + max_x) as f32 * 0.5, (min_y + max_y) as f32 * 0.5);

    CentringMetrics { btn_centre, box_centre, shaped_geom_centre, pixel_bbox_centre }
}

#[test]
fn qa_card_button_label_is_centred_within_1px() {
    let mut engine = Engine::default();
    let page_id = page(&engine);

    // Build the QA Login Card using the easeui-button-primary preset and both
    // a 44px button with a child text node ("Sign in") and the preset's text
    // shorthand ("Continue"), then also verify after updating the label text
    // ("Log in") as in the AI-08 session.
    let spec = json!({
        "kind": "frame",
        "name": "Login Card",
        "preset": "easeui-card",
        "width": 360.0,
        "height": 260.0,
        "layout": {
            "direction": "column",
            "gap": 16.0,
            "padding": [24.0, 24.0, 24.0, 24.0],
            "align": "stretch"
        },
        "children": [
            {
                "kind": "text",
                "name": "Title",
                "text": "Welcome back",
                "font_size": 20.0,
                "font_weight": 600,
                "fill": "$easeui.text.primary"
            },
            {
                "kind": "frame",
                "name": "Email Input",
                "preset": "easeui-input",
                "width_sizing": "fill",
                "height": 40.0,
                "text": "[email]"
            },
            {
                "kind": "frame",
                "name": "Button",
                "preset": "easeui-button-primary",
                "width_sizing": "fill",
                "height": 44.0,
                "children": [
                    {
                        "kind": "text",
                        "name": "Button label",
                        "text": "Sign in",
                        "font_size": 14.0,
                        "font_weight": 600,
                        "fill": "#ffffff"
                    }
                ]
            },
            {
                "kind": "frame",
                "name": "Submit Button",
                "preset": "easeui-button-primary",
                "width_sizing": "fill",
                "text": "Continue"
            }
        ]
    });

    let applied = engine.call("designos_apply_spec", json!({ "parent_id": page_id, "spec": spec.clone() })).expect("apply_spec");
    let card_id = NodeId(applied["name_to_id"]["Login Card"].as_u64().expect("Login Card id"));

    // Optionally dump the rendered card PNG via designos_render_png when DESIGNOS_DUMP_CARD_PNG is set.
    if let Ok(path_str) = std::env::var("DESIGNOS_DUMP_CARD_PNG") {
        let out_path = std::path::PathBuf::from(&path_str);
        let dir = out_path.parent().expect("parent dir");
        let file_name = out_path.file_name().expect("file name").to_str().expect("utf8 file name");
        let _ = std::fs::remove_file(&out_path);
        let mut session = Session::new(dir).expect("session");
        let doc_val = session.call("designos_get_document", json!({})).expect("get_document");
        let s_page = doc_val["pages"][0]["node_id"].as_u64().expect("page id");
        let s_applied = session.call("designos_apply_spec", json!({ "parent_id": s_page, "spec": spec })).expect("session apply_spec");
        let s_card_id = s_applied["name_to_id"]["Login Card"].as_u64().expect("session Login Card id");
        session.call("designos_render_png", json!({ "path": file_name, "node_id": s_card_id, "scale": 2.0 })).expect("designos_render_png");
    }

    for (btn_name, lbl_name) in [("Button", "Button label"), ("Submit Button", "Continue")] {
        let CentringMetrics { btn_centre: btn_c, box_centre: box_c, shaped_geom_centre: geom_c, pixel_bbox_centre: px_c } =
            measure_button_label_centring(&engine.doc, card_id, btn_name, lbl_name);
        let dx_box = (box_c.0 - btn_c.0).abs();
        let dy_box = (box_c.1 - btn_c.1).abs();
        let dx_geom = (geom_c.0 - btn_c.0).abs();
        let dy_geom = (geom_c.1 - btn_c.1).abs();
        let dx_px = (px_c.0 - btn_c.0).abs();
        let dy_px = (px_c.1 - btn_c.1).abs();
        println!(
            "[{btn_name}/{lbl_name}] btn_c={btn_c:?} box_c={box_c:?} geom_c={geom_c:?} px_c={px_c:?} dx_geom={dx_geom:.2} dy_geom={dy_geom:.2} dx_px={dx_px:.2} dy_px={dy_px:.2}"
        );
        assert!(
            dx_box <= 1.0 && dy_box <= 1.0,
            "{btn_name}/{lbl_name} text box centre {box_c:?} is more than 1 px from button centre {btn_c:?} (dx={dx_box:.2}, dy={dy_box:.2})"
        );
        assert!(
            dx_geom <= 1.0 && dy_geom <= 1.0,
            "{btn_name}/{lbl_name} shaped glyph centre {geom_c:?} is more than 1 px from button centre {btn_c:?} (dx={dx_geom:.2}, dy={dy_geom:.2})"
        );
        assert!(
            dx_px <= 2.0 && dy_px <= 2.0,
            "{btn_name}/{lbl_name} rendered pixel centre {px_c:?} is more than 2 px from button centre {btn_c:?} (dx={dx_px:.2}, dy={dy_px:.2})"
        );
    }

    // Update the label via designos_update_node ("Sign in" -> "Log in") and re-verify.
    let lbl_id = find_by_name(&engine.doc, card_id, "Button label").id.0;
    engine
        .call(
            "designos_update_node",
            json!({
                "node_id": lbl_id,
                "text": "Log in"
            }),
        )
        .expect("update_node Log in");

    let CentringMetrics { btn_centre: btn_c, box_centre: box_c, shaped_geom_centre: geom_c, pixel_bbox_centre: px_c } =
        measure_button_label_centring(&engine.doc, card_id, "Button", "Button label");
    let dx_box = (box_c.0 - btn_c.0).abs();
    let dy_box = (box_c.1 - btn_c.1).abs();
    let dx_geom = (geom_c.0 - btn_c.0).abs();
    let dy_geom = (geom_c.1 - btn_c.1).abs();
    let dx_px = (px_c.0 - btn_c.0).abs();
    let dy_px = (px_c.1 - btn_c.1).abs();
    println!(
        "[Button/Log in] btn_c={btn_c:?} box_c={box_c:?} geom_c={geom_c:?} px_c={px_c:?} dx_geom={dx_geom:.2} dy_geom={dy_geom:.2} dx_px={dx_px:.2} dy_px={dy_px:.2}"
    );
    assert!(
        dx_box <= 1.0 && dy_box <= 1.0,
        "after update_node('Log in'), text box centre {box_c:?} is more than 1 px from button centre {btn_c:?} (dx={dx_box:.2}, dy={dy_box:.2})"
    );
    assert!(
        dx_geom <= 1.0 && dy_geom <= 1.0,
        "after update_node('Log in'), shaped glyph centre {geom_c:?} is more than 1 px from button centre {btn_c:?} (dx={dx_geom:.2}, dy={dy_geom:.2}); box_c={box_c:?}"
    );
    assert!(
        dx_px <= 2.0 && dy_px <= 2.0,
        "after update_node('Log in'), rendered pixel centre {px_c:?} is more than 2 px from button centre {btn_c:?} (dx={dx_px:.2}, dy={dy_px:.2})"
    );
}

#[test]
fn plain_spec_frame_with_align_and_justify_center_centres_label_within_1px() {
    let mut engine = Engine::default();
    let page_id = page(&engine);

    let spec = json!({
        "kind": "frame",
        "name": "Plain Row Button",
        "width": 312.0,
        "height": 44.0,
        "corner_radius": 8.0,
        "fill": "#10a37f",
        "layout": {
            "direction": "row",
            "align": "center",
            "justify": "center"
        },
        "children": [
            {
                "kind": "text",
                "name": "Plain Row Label",
                "text": "Sign in",
                "font_size": 14.0,
                "font_weight": 600,
                "fill": "#ffffff"
            }
        ]
    });

    let applied = engine.call("designos_apply_spec", json!({ "parent_id": page_id, "spec": spec })).expect("apply_spec plain row button");
    let row_id = NodeId(applied["name_to_id"]["Plain Row Button"].as_u64().expect("Plain Row Button id"));

    let CentringMetrics { btn_centre: btn_c, box_centre: box_c, shaped_geom_centre: geom_c, pixel_bbox_centre: px_c } =
        measure_button_label_centring(&engine.doc, row_id, "Plain Row Button", "Plain Row Label");
    let dx_box = (box_c.0 - btn_c.0).abs();
    let dy_box = (box_c.1 - btn_c.1).abs();
    let dx_geom = (geom_c.0 - btn_c.0).abs();
    let dy_geom = (geom_c.1 - btn_c.1).abs();
    let dx_px = (px_c.0 - btn_c.0).abs();
    let dy_px = (px_c.1 - btn_c.1).abs();
    println!(
        "[Plain Row Button] btn_c={btn_c:?} box_c={box_c:?} geom_c={geom_c:?} px_c={px_c:?} dx_geom={dx_geom:.2} dy_geom={dy_geom:.2} dx_px={dx_px:.2} dy_px={dy_px:.2}"
    );
    assert!(dx_box <= 1.0 && dy_box <= 1.0, "Plain Row Label text box centre {box_c:?} is more than 1 px from button centre {btn_c:?}");
    assert!(
        dx_geom <= 1.0 && dy_geom <= 1.0,
        "Plain Row Label shaped glyph centre {geom_c:?} is more than 1 px from button centre {btn_c:?} (dx={dx_geom:.2}, dy={dy_geom:.2})"
    );
    assert!(
        dx_px <= 2.0 && dy_px <= 2.0,
        "Plain Row Label rendered pixel centre {px_c:?} is more than 2 px from button centre {btn_c:?} (dx={dx_px:.2}, dy={dy_px:.2})"
    );

    // A column-direction frame with align/justify: center and a fill-width text child must NOT
    // overwrite the horizontal alignment of the stretched text child.
    let col_spec = json!({
        "kind": "frame",
        "name": "Plain Col Button",
        "width": 240.0,
        "height": 44.0,
        "corner_radius": 8.0,
        "fill": "#10a37f",
        "layout": {
            "direction": "column",
            "align": "center",
            "justify": "center"
        },
        "children": [
            {
                "kind": "text",
                "name": "Plain Col Label",
                "text": "Get started",
                "width_sizing": "fill",
                "font_size": 14.0,
                "font_weight": 600,
                "fill": "#ffffff"
            }
        ]
    });

    let applied_col = engine.call("designos_apply_spec", json!({ "parent_id": page_id, "spec": col_spec })).expect("apply_spec plain col button");
    let col_id = NodeId(applied_col["name_to_id"]["Plain Col Button"].as_u64().expect("Plain Col Button id"));
    let col_align_h = match &find_by_name(&engine.doc, col_id, "Plain Col Label").kind {
        NodeKind::Text(t) => t.style.align_h,
        other => panic!("expected text node, got {other:?}"),
    };

    // Grok's concrete spec: a fill-width text with `text_style: "Body"` inside a centred column
    // keeps its own Left/Top alignment.
    engine
        .doc
        .styles
        .insert("Body".into(), Style::Text(TextStyle { font_size: 14.0, align_h: TextAlignH::Left, align_v: TextAlignV::Top, ..Default::default() }));
    let grok_col_spec = json!({
        "kind": "frame",
        "name": "Welcome Column",
        "width": 320.0,
        "height": 120.0,
        "layout": { "direction": "column", "align": "center", "justify": "start" },
        "children": [{
            "kind": "text",
            "name": "Welcome Text",
            "text": "Welcome back to your account",
            "width_sizing": "fill",
            "text_style": "Body"
        }]
    });
    let applied_welcome = engine.call("designos_apply_spec", json!({ "parent_id": page_id, "spec": grok_col_spec })).expect("apply_spec welcome col");
    let welcome_col_id = NodeId(applied_welcome["name_to_id"]["Welcome Column"].as_u64().expect("Welcome Column id"));
    let welcome_lbl = find_by_name(&engine.doc, welcome_col_id, "Welcome Text");
    let welcome_tp = match &welcome_lbl.kind {
        NodeKind::Text(t) => t,
        other => panic!("expected text node, got {other:?}"),
    };

    println!("[Fill-Width Column] plain_col_align_h={:?} welcome_align=({:?}, {:?})", col_align_h, welcome_tp.style.align_h, welcome_tp.style.align_v);
    assert_eq!(col_align_h, TextAlignH::Left, "width_sizing: fill text in a centred column must keep Left horizontal alignment");
    assert_eq!(
        (welcome_tp.style.align_h, welcome_tp.style.align_v),
        (TextAlignH::Left, TextAlignV::Top),
        "fill-width styled text in a centred column must keep its own alignment"
    );
}

#[test]
fn create_node_in_centred_row_keeps_default_alignment() {
    let mut engine = Engine::default();
    let page_id = page(&engine);

    let applied = engine
        .call(
            "designos_apply_spec",
            json!({
                "parent_id": page_id,
                "spec": {
                    "kind": "frame",
                    "name": "Row Host",
                    "preset": "easeui-row",
                    "width": 320.0,
                    "height": 120.0
                }
            }),
        )
        .expect("apply_spec Row Host");
    let row_id = applied["name_to_id"]["Row Host"].as_u64().expect("Row Host id");

    let created = engine
        .call(
            "designos_create_node",
            json!({
                "parent_id": row_id,
                "kind": "text",
                "name": "Row Text",
                "text": "Hello"
            }),
        )
        .expect("create_node in easeui-row");
    let text_id = NodeId(created["node"]["id"].as_u64().expect("Row Text id"));
    let node = engine.doc.get(text_id).expect("Row Text exists");
    let tp = match &node.kind {
        NodeKind::Text(t) => t,
        other => panic!("expected text node, got {other:?}"),
    };
    println!("[create_node in easeui-row] align=({:?}, {:?}) auto_resize={:?}", tp.style.align_h, tp.style.align_v, tp.auto_resize);
    assert_eq!(
        (tp.style.align_h, tp.style.align_v),
        (TextAlignH::Left, TextAlignV::Top),
        "designos_create_node must not overwrite text alignment inside a centring parent"
    );
}

#[test]
fn sized_text_update_node_never_resizes_user_sized_text() {
    let mut engine = Engine::default();
    let page_id = page(&engine);

    // 1. Grok's sequence: create with width: 240, height: 32, then update_node { text }.
    // The size must stay 240x32.
    let created = engine
        .call(
            "designos_create_node",
            json!({
                "parent_id": page_id,
                "kind": "text",
                "name": "Sized Create",
                "width": 240.0,
                "height": 32.0,
                "text": "Hello"
            }),
        )
        .expect("create_node 240x32");
    let created_id = NodeId(created["node"]["id"].as_u64().expect("Sized Create id"));
    engine
        .call(
            "designos_update_node",
            json!({
                "node_id": created_id.0,
                "text": "Hello world"
            }),
        )
        .expect("update_node text on Sized Create");
    let (n1_size, ar1) = {
        let n1 = engine.doc.get(created_id).expect("Sized Create exists");
        let ar1 = match &n1.kind {
            NodeKind::Text(t) => t.auto_resize,
            other => panic!("expected text node, got {other:?}"),
        };
        (n1.size, ar1)
    };

    // 2. A width set by an earlier update_node on a hugging text also survives a later text-only edit.
    let applied = engine
        .call(
            "designos_apply_spec",
            json!({
                "parent_id": page_id,
                "spec": {
                    "kind": "text",
                    "name": "Hug Then Sized",
                    "text": "Short"
                }
            }),
        )
        .expect("apply_spec Hug Then Sized");
    let hug_id = NodeId(applied["name_to_id"]["Hug Then Sized"].as_u64().expect("Hug Then Sized id"));
    engine
        .call(
            "designos_update_node",
            json!({
                "node_id": hug_id.0,
                "width": 240.0
            }),
        )
        .expect("update_node width 240");
    engine
        .call(
            "designos_update_node",
            json!({
                "node_id": hug_id.0,
                "text": "Hello world"
            }),
        )
        .expect("update_node text after width");
    let (n2_size, ar2) = {
        let n2 = engine.doc.get(hug_id).expect("Hug Then Sized exists");
        let ar2 = match &n2.kind {
            NodeKind::Text(t) => t.auto_resize,
            other => panic!("expected text node, got {other:?}"),
        };
        (n2.size, ar2)
    };

    println!("[sized_text_update] n1.size={:?} ar1={:?} n2.size={:?} ar2={:?}", n1_size, ar1, n2_size, ar2);
    assert_eq!(n1_size, Vec2::new(240.0, 32.0), "text created with width: 240, height: 32 must keep 240x32 on a text-only update_node");
    assert_eq!(ar1, TextAutoResize::None);
    assert_eq!(n2_size.x, 240.0, "a width set by an earlier update_node must survive a later text-only edit");
    assert_eq!(ar2, TextAutoResize::Height);
}
