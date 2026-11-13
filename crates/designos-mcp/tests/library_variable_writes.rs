//! `designos_apply_batch` `set_variables` refuses every write to a subscribed library collection or variable, and the
//! same write on a local collection next to it goes through. A refusal is atomic: nothing earlier in the batch stays.

use designos_doc::{VariableKind, VariableStore, VariableValue};
use designos_mcp::engine::Engine;
use serde_json::{json, Value};

struct Fixture {
    engine: Engine,
    library: Names,
    local: Names,
}

struct Names {
    collection: String,
    mode: String,
    first: String,
    second: String,
    alias: String,
}

fn add_collection(store: &mut VariableStore, name: &str, library: bool) -> Names {
    let collection = store.add_collection(name);
    let mode = store.collections[&collection].default_mode.clone();
    let first = store.add_variable(&collection, "first", VariableKind::Number).unwrap();
    let second = store.add_variable(&collection, "second", VariableKind::Number).unwrap();
    let alias = store.add_variable(&collection, "alias", VariableKind::Number).unwrap();
    assert!(store.set_value(&first, &mode, VariableValue::Number(8.0)));
    assert!(store.set_value(&second, &mode, VariableValue::Number(4.0)));
    assert!(store.set_alias(&alias, &mode, &first));
    if library {
        store.collections.get_mut(&collection).unwrap().source_library_key = Some(format!("{name}-key"));
    }
    Names { collection, mode, first, second, alias }
}

fn fixture() -> Fixture {
    let mut store = VariableStore::default();
    let library = add_collection(&mut store, "Library", true);
    let local = add_collection(&mut store, "Local", false);
    let mut engine = Engine::default();
    engine.doc.variables = store.variables;
    engine.doc.variable_collections = store.collections;
    Fixture { engine, library, local }
}

fn set_variables(engine: &mut Engine, op: Value) -> Result<Value, Value> {
    let mut op = op;
    op["op"] = json!("set_variables");
    engine.call("designos_apply_batch", json!({"ops": [op]})).map_err(|e| e.to_json_value())
}

fn snapshot(engine: &Engine) -> String {
    engine.doc.to_json().expect("document json")
}

/// The four edits, built for one set of names.
fn edits(n: &Names) -> Vec<(&'static str, Value)> {
    vec![
        ("delete", json!({"delete_variables": [n.second]})),
        ("value edit", json!({"upsert_variables": [{"id": n.first, "values": {n.mode.clone(): 20}}]})),
        (
            "mode edit",
            json!({"upsert_collections": [{"id": n.collection, "modes": [{"id": n.mode, "name": "One"}, {"id": "extra", "name": "Extra"}], "default_mode": n.mode}]}),
        ),
        ("alias retarget", json!({"upsert_variables": [{"id": n.alias, "values": {n.mode.clone(): {"alias": n.second}}}]})),
    ]
}

#[test]
fn each_edit_is_refused_on_a_library_collection_and_leaves_the_document_untouched() {
    for i in 0..4 {
        let mut f = fixture();
        let (what, op) = edits(&f.library).remove(i);
        let before = snapshot(&f.engine);
        let depth = f.engine.history.undo_depth();

        let err = set_variables(&mut f.engine, op).expect_err(what);

        assert_eq!(err["cause"]["message"], "library variables are read-only", "{what}: {err}");
        assert_eq!(snapshot(&f.engine), before, "{what}: nothing changed");
        assert_eq!(f.engine.history.undo_depth(), depth, "{what}: no history step");
    }
}

#[test]
fn the_same_edits_go_through_on_a_local_collection_beside_a_library() {
    for i in 0..4 {
        let mut f = fixture();
        let (what, op) = edits(&f.local).remove(i);
        let library_before = f.engine.doc.variables[&f.library.first].clone();

        set_variables(&mut f.engine, op).unwrap_or_else(|e| panic!("{what} on a local collection: {e}"));

        assert_eq!(f.engine.doc.variables[&f.library.first], library_before, "{what}: the library variable is untouched");
    }

    let mut f = fixture();
    let (local, library) = (&f.local, &f.library);
    let (first, second, alias, mode, collection) =
        (local.first.clone(), local.second.clone(), local.alias.clone(), local.mode.clone(), local.collection.clone());
    let library_collection = library.collection.clone();
    set_variables(&mut f.engine, json!({"upsert_variables": [{"id": first, "values": {mode.clone(): 20}}]})).unwrap();
    assert_eq!(f.engine.doc.variables[&first].values[&mode], VariableValue::Number(20.0));
    set_variables(&mut f.engine, json!({"upsert_variables": [{"id": alias, "values": {mode.clone(): {"alias": second.clone()}}}]})).unwrap();
    assert_eq!(f.engine.doc.variables[&alias].values[&mode], VariableValue::Alias(second.clone()));
    set_variables(&mut f.engine, json!({"delete_variables": [second.clone()]})).unwrap();
    assert!(!f.engine.doc.variables.contains_key(&second));
    set_variables(
        &mut f.engine,
        json!({"upsert_collections": [{"id": collection, "modes": [{"id": mode, "name": "One"}, {"id": "extra", "name": "Extra"}], "default_mode": mode}]}),
    )
    .unwrap();
    assert!(f.engine.doc.variable_collections[&collection].modes.iter().any(|m| m.id == "extra"));
    assert_eq!(f.engine.doc.variable_collections[&library_collection].modes.len(), 1, "the library collection kept its modes");
}

#[test]
fn a_batch_with_a_library_write_after_a_local_write_changes_nothing() {
    let mut f = fixture();
    let before = snapshot(&f.engine);
    let depth = f.engine.history.undo_depth();
    let local_edit = json!({"op": "set_variables", "upsert_variables": [{"id": f.local.first, "values": {f.local.mode.clone(): 99}}]});
    let library_edit = json!({"op": "set_variables", "delete_variables": [f.library.second]});

    let err = f.engine.call("designos_apply_batch", json!({"ops": [local_edit, library_edit]})).expect_err("the second op is refused").to_json_value();

    assert_eq!(err["cause"]["message"], "library variables are read-only", "{err}");
    assert_eq!(err["cause"]["path"].as_str().map(|p| p.starts_with("/ops/1")), Some(true), "{err}");
    assert_eq!(snapshot(&f.engine), before, "the local edit before it was not kept");
    assert_eq!(f.engine.history.undo_depth(), depth);
}
