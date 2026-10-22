//! A `var:` colour that names a variable of an enabled library cannot be bound by `update_node`,
//! `create_node` or a batch (importing is a document change of its own), and says how to import it.
#![allow(clippy::result_large_err)]

mod support;

use designos_doc::LibraryRef;
use designos_mcp::engine::Engine;
use serde_json::{json, Value};
use support::*;

#[test]
fn a_library_variable_is_named_as_one_and_the_import_path_is_given() {
    // One test per process: the library root is read from the environment.
    let dir = std::env::temp_dir().join(format!("designos-mcp-colour-lib-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::env::set_var("DESIGNOS_LIBRARY", &dir);

    let mut source = palette();
    source
        .call(
            "designos_apply_batch",
            json!({"ops": [{"op": "set_variables", "upsert_variables": [
                {"id": "color/brand", "name": "Brand", "collection": "color", "kind": "color", "values": {"light": "#7C3AED"}}
            ]}]}),
        )
        .unwrap();
    designos_library::Library::open(&dir).unwrap().publish_library("lib-brand", &source.doc, "v1").unwrap();

    let mut e = Engine::default();
    e.doc.library.enabled.push(LibraryRef { id: "lib-brand".into(), name: "Brand".into(), version: 1 });
    let rect = apply(&mut e, json!({"kind": "rectangle", "name": "Box", "width": 20, "height": 20, "fill": "#CCCCCC"}));
    let before = e.doc.get(rect).unwrap().clone();

    let err: Value = e.call("designos_update_node", json!({"node_id": rect.0, "fill": "var:Color/Brand"})).unwrap_err().to_json_value();
    assert_eq!(err["code"], "UNKNOWN_VARIABLE", "{err}");
    assert_eq!(err["path"], "/fill", "{err}");
    let message = err["message"].as_str().unwrap();
    let next_step = err["next_step"].as_str().unwrap();
    assert_eq!(message, "Library variable. Import it with designos_apply_spec first.", "{err}");
    assert!(next_step.contains("designos_apply_spec"), "{next_step}");
    assert!(!next_step.contains("designos_get_variables"), "that tool cannot list library variables: {next_step}");
    assert!(message.split_whitespace().count() <= 10, "plain and short: {message}");
    assert_eq!(err["safe_to_retry"], false, "resending the same call cannot work before the import: {err}");
    assert_eq!(e.doc.get(rect).unwrap(), &before, "a refused write leaves the node alone");

    // The same through create_node and a batch op.
    let created = e.call("designos_create_node", json!({"parent_id": page(&e), "kind": "rectangle", "fill": "var:Color/Brand"})).unwrap_err();
    assert_eq!(created.message, message);
    let batch = e.call("designos_apply_batch", json!({"ops": [{"op": "update", "node": rect.0, "fill": "var:Color/Brand"}]})).unwrap_err().to_json_value();
    assert_eq!(batch["cause"]["message"], message, "{batch}");
    assert_eq!(batch["cause"]["safe_to_retry"], false, "{batch}");

    // A name no library has is still an ordinary unknown variable.
    let other = e.call("designos_update_node", json!({"node_id": rect.0, "fill": "var:Color/Nope"})).unwrap_err().to_json_value();
    assert_eq!(other["code"], "UNKNOWN_VARIABLE");
    assert!(other["message"].as_str().unwrap().starts_with("Unknown variable"), "{other}");
    assert_eq!(other["safe_to_retry"], true, "an ordinary unknown variable is repaired and resent: {other}");

    // Importing through apply_spec, as the error says, makes the same write work.
    e.call("designos_apply_spec", json!({"parent_id": page(&e), "spec": {"kind": "frame", "name": "Import", "fill": "var:Color/Brand"}})).unwrap();
    e.call("designos_update_node", json!({"node_id": rect.0, "fill": "var:Color/Brand"})).expect("bound after the import");
    assert!(e.doc.get(rect).unwrap().variables.bindings.contains_key("fills/0"));

    std::env::remove_var("DESIGNOS_LIBRARY");
    let _ = std::fs::remove_dir_all(&dir);
}
