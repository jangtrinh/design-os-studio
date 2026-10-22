//! A `var:` colour that names a variable of an enabled library cannot be bound by `update_node`,
//! `create_node` or a batch (importing is a document change of its own). The error says so and what to do instead.
//! It is checked through `Session::call`, the layer every MCP call goes through, because that layer rewrites
//! `corrected_example` and `safe_to_retry` of any error: what an agent receives is not what the engine returns.
#![allow(clippy::result_large_err)]

mod support;

use designos_doc::LibraryRef;
use designos_mcp::server::Session;
use serde_json::{json, Value};
use support::*;

const MESSAGE: &str = "Library variable. Import it with designos_apply_spec first.";
const NEW_LAYER_STEP: &str = "Build this layer with designos_apply_spec using the same var:.";
const EXISTING_LAYER_STEP: &str =
    "Library variables bind only through designos_apply_spec in this version. Rebuild the layer that way, or use a local variable.";

/// The error as an agent receives it, and the layer count the refused call must leave alone.
fn refused(s: &mut Session, tool: &str, args: Value) -> Value {
    let layers = s.engine.doc.nodes.len();
    let err = s.call(tool, args).expect_err("the write is refused").to_json_value();
    assert_eq!(s.engine.doc.nodes.len(), layers, "{tool}: a refused write creates no layer");
    err
}

fn assert_library_error(err: &Value, cause: bool, next_step: &str) {
    let e = if cause { &err["cause"] } else { err };
    assert_eq!(e["code"], "UNKNOWN_VARIABLE", "{err}");
    assert_eq!(e["message"], MESSAGE, "{err}");
    assert_eq!(e["next_step"], next_step, "{err}");
    assert_eq!(e["path"].as_str().map(|p| p.ends_with("/fill")), Some(true), "{err}");
    assert!(e.get("corrected_example").is_none(), "an example would be rewritten into the failing call's arguments: {err}");
    assert!(e.get("valid_options").is_none(), "{err}");
    assert!(!e["next_step"].as_str().unwrap().contains("designos_get_variables"), "that tool cannot list library variables: {err}");
}

#[test]
fn a_library_variable_is_refused_with_the_limit_stated_and_no_example_through_the_session() {
    // One test per process: the library root is read from the environment.
    let base = std::env::temp_dir().join(format!("designos-mcp-colour-lib-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&base);
    let (library, workspace) = (base.join("library"), base.join("workspace"));
    std::fs::create_dir_all(&workspace).unwrap();
    std::env::set_var("DESIGNOS_LIBRARY", &library);

    let mut source = palette();
    source
        .call(
            "designos_apply_batch",
            json!({"ops": [{"op": "set_variables", "upsert_variables": [
                {"id": "color/brand", "name": "Brand", "collection": "color", "kind": "color", "values": {"light": "#7C3AED"}}
            ]}]}),
        )
        .unwrap();
    designos_library::Library::open(&library).unwrap().publish_library("lib-brand", &source.doc, "v1").unwrap();

    let mut s = Session::new(&workspace).unwrap();
    s.engine.doc.library.enabled.push(LibraryRef { id: "lib-brand".into(), name: "Brand".into(), version: 1 });
    let page = s.engine.resolve_default_page().unwrap().0;
    let rect = s.call("designos_create_node", json!({"parent_id": page, "kind": "rectangle", "name": "Box", "fill": "#CCCCCC"})).unwrap()["node"]["id"]
        .as_u64()
        .unwrap();
    let before = s.engine.doc.get(designos_doc::NodeId(rect)).unwrap().clone();

    // A layer that does not exist yet is built with designos_apply_spec instead.
    let created = refused(&mut s, "designos_create_node", json!({"parent_id": page, "kind": "rectangle", "fill": "var:Color/Brand"}));
    assert_library_error(&created, false, NEW_LAYER_STEP);
    let inserted = refused(&mut s, "designos_apply_batch", json!({"ops": [{"op": "insert", "parent": page, "kind": "rectangle", "fill": "var:Color/Brand"}]}));
    assert_library_error(&inserted, true, NEW_LAYER_STEP);

    // A layer that exists cannot take it in this version.
    let updated = refused(&mut s, "designos_update_node", json!({"node_id": rect, "fill": "var:Color/Brand"}));
    assert_library_error(&updated, false, EXISTING_LAYER_STEP);
    let batch_updated = refused(&mut s, "designos_apply_batch", json!({"ops": [{"op": "update", "node": rect, "fill": "var:Color/Brand"}]}));
    assert_library_error(&batch_updated, true, EXISTING_LAYER_STEP);
    assert_eq!(s.engine.doc.get(designos_doc::NodeId(rect)).unwrap(), &before, "a refused write leaves the node alone");

    // A name no library has is still an ordinary unknown variable, repaired with its valid_options.
    let other = refused(&mut s, "designos_update_node", json!({"node_id": rect, "fill": "var:Color/Nope"}));
    assert_eq!(other["code"], "UNKNOWN_VARIABLE", "{other}");
    assert!(other["message"].as_str().unwrap().starts_with("Unknown variable"), "{other}");
    assert_eq!(other["next_step"], "Call designos_get_variables or pick one of valid_options.", "{other}");

    // Following the new-layer step works, and the variable can then be bound to an existing layer.
    s.call("designos_apply_spec", json!({"parent_id": page, "spec": {"kind": "frame", "name": "Imported", "fill": "var:Color/Brand"}})).unwrap();
    s.call("designos_update_node", json!({"node_id": rect, "fill": "var:Color/Brand"})).expect("bound after the import");
    assert!(s.engine.doc.get(designos_doc::NodeId(rect)).unwrap().variables.bindings.contains_key("fills/0"));

    std::env::remove_var("DESIGNOS_LIBRARY");
    let _ = std::fs::remove_dir_all(&base);
}
