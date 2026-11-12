//! `designos_get_variables` tells a variable of a subscribed library collection from a local one through the
//! variable's collection: the tool lists each variable's native record, which has no flag of its own, and each
//! collection with its `source_library_key`.
#![allow(clippy::result_large_err)]

use designos_mcp::engine::Engine;
use serde_json::{json, Value};

fn engine_with_a_subscribed_collection() -> Engine {
    let payload = json!({"meta": {
        "variableCollections": {
            "C:local": {"id": "C:local", "name": "Theme", "defaultModeId": "L", "modes": [{"modeId": "L", "name": "Light"}], "variableIds": ["V:surface"]},
            "C:lib": {"id": "C:lib", "name": "Brand", "remote": true, "key": "K_LIB", "defaultModeId": "B", "modes": [{"modeId": "B", "name": "Default"}], "variableIds": ["V:primary"]}
        },
        "variables": {
            "V:surface": {"id": "V:surface", "name": "Surface", "resolvedType": "FLOAT", "variableCollectionId": "C:local", "valuesByMode": {"L": 1}},
            "V:primary": {"id": "V:primary", "name": "Primary", "resolvedType": "FLOAT", "variableCollectionId": "C:lib", "valuesByMode": {"B": 2}}
        }
    }});
    let mut e = Engine::default();
    designos_fig::import_variables_value(&mut e.doc, &payload);
    e
}

#[test]
fn get_variables_names_the_library_of_the_collection_a_variable_belongs_to() {
    let mut e = engine_with_a_subscribed_collection();
    let listed: Value = e.call("designos_get_variables", json!({})).unwrap();

    let library_of = |variable: &str| {
        let collection = listed["variables"][variable]["collection"].as_str().unwrap_or_else(|| panic!("{variable} lists its collection: {listed}"));
        listed["collections"][collection]["source_library_key"].as_str().map(String::from)
    };
    assert_eq!(library_of("V:primary").as_deref(), Some("K_LIB"), "{listed}");
    assert_eq!(library_of("V:surface"), None, "{listed}");
}
