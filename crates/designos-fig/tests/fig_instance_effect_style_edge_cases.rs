//! Edge cases of the rule "a linked effect style wins over a stale or empty cached copy": a detached
//! instance, nested override entries, a legacy-only link, a master that links a style with an empty
//! cache, variable-bound copies, and a nested relink through a `.fig` save.
use designos_doc::*;
use designos_fig::{export_fig, import_fig, import_fig_node_changes};
use serde_json::{json, Value};

const DETACHED: u32 = u32::MAX;

fn g(s: u32, l: u32) -> Value {
    json!({"sessionID": s, "localID": l})
}

fn node(guid: (u32, u32), parent: (u32, u32), pos: &str, ty: &str, name: &str, extra: Value) -> Value {
    let mut m = json!({
        "guid": g(guid.0, guid.1), "phase": "CREATED",
        "parentIndex": {"guid": g(parent.0, parent.1), "position": pos},
        "type": ty, "name": name, "visible": true, "opacity": 1.0,
        "transform": {"m00": 1.0, "m01": 0.0, "m02": 0.0, "m10": 0.0, "m11": 1.0, "m12": 0.0}
    });
    for (k, v) in extra.as_object().expect("extra is an object") {
        m[k] = v.clone();
    }
    m
}

fn shadow(radius: f64) -> Value {
    json!({"type": "DROP_SHADOW", "color": {"r": 0.0, "g": 0.0, "b": 0.0, "a": 0.25}, "offset": {"x": 0.0, "y": 2.0}, "radius": radius, "spread": 0.0, "visible": true})
}

fn bound(mut effect: Value, var: (u32, u32)) -> Value {
    effect["colorVar"] = json!({"dataType": "ALIAS", "value": {"alias": {"guid": g(var.0, var.1)}}});
    effect
}

fn effect_style(local: u32, name: &str, effects: Vec<Value>) -> Value {
    node(
        (9, local),
        (0, 2),
        &format!("s{local}"),
        "RECTANGLE",
        name,
        json!({"styleType": "EFFECT", "key": format!("key-{local}"), "size": {"x": 1.0, "y": 1.0}, "effects": effects}),
    )
}

fn link(local: u32) -> Value {
    json!({"guid": g(9, local)})
}

/// Master `Badge` and its child `Dot`. `root` and `dot` are the master's own effect fields.
fn doc_json(card: Vec<Value>, root: Value, dot: Value, extra: Vec<Value>) -> Value {
    let mut v = vec![
        node((0, 0), (0, 0), "!", "DOCUMENT", "Doc", json!({})),
        node((0, 1), (0, 0), "a", "CANVAS", "Page", json!({})),
        node((0, 2), (0, 0), "b", "CANVAS", "Internal Only Canvas", json!({"internalOnly": true})),
        effect_style(1, "Shadow/Card", card),
        effect_style(2, "Shadow/Raised", vec![shadow(12.0)]),
        node((1, 10), (0, 1), "a", "SYMBOL", "Badge", {
            let mut f = json!({"size": {"x": 44.0, "y": 22.0}});
            for (k, x) in root.as_object().expect("root fields") {
                f[k] = x.clone();
            }
            f
        }),
        node((1, 11), (1, 10), "a", "RECTANGLE", "Dot", {
            let mut f = json!({"size": {"x": 8.0, "y": 8.0}});
            for (k, x) in dot.as_object().expect("dot fields") {
                f[k] = x.clone();
            }
            f
        }),
    ];
    v.extend(extra);
    Value::Array(v)
}

fn standard(extra: Vec<Value>) -> Value {
    let linked = json!({"styleIdForEffect": link(1), "effects": [shadow(8.0)]});
    doc_json(vec![shadow(8.0)], linked.clone(), linked, extra)
}

fn inst(local: u32, name: &str, own: Value, entries: Vec<Value>) -> Value {
    let mut f = json!({"size": {"x": 44.0, "y": 22.0}, "symbolData": {"symbolID": g(1, 10), "symbolOverrides": entries}});
    for (k, v) in own.as_object().expect("own fields") {
        f[k] = v.clone();
    }
    node((3, local), (0, 1), &format!("b{local}"), "INSTANCE", name, f)
}

fn root_entry(fields: Value) -> Value {
    let mut e = json!({"guidPath": {"guids": [g(1, 10)]}});
    for (k, v) in fields.as_object().expect("entry fields") {
        e[k] = v.clone();
    }
    e
}

fn dot_entry(fields: Value) -> Value {
    let mut e = json!({"guidPath": {"guids": [g(1, 11)]}});
    for (k, v) in fields.as_object().expect("entry fields") {
        e[k] = v.clone();
    }
    e
}

fn named<'a>(doc: &'a Document, name: &str) -> &'a Node {
    doc.nodes.values().find(|n| n.name == name).unwrap_or_else(|| panic!("no node named {name}"))
}

fn dot_of<'a>(doc: &'a Document, instance: &str) -> &'a Node {
    doc.get(named(doc, instance).children[0]).expect("the instance's Dot")
}

fn radii(n: &Node) -> Vec<f32> {
    n.effects.iter().filter_map(|e| if let Effect::DropShadow { radius, .. } = e { Some(*radius) } else { None }).collect()
}

fn style_of(doc: &Document, n: &Node) -> Option<String> {
    n.style_refs.get("effect").map(|k| doc.style_name(k).to_string())
}

fn import(changes: &Value) -> Document {
    import_fig_node_changes(changes, "Doc").expect("import")
}

fn reopened(doc: &Document) -> Document {
    import_fig(&export_fig(doc).expect("export")).expect("reopen")
}

#[test]
fn a_detached_instance_keeps_the_effects_its_entry_holds() {
    let detached = json!({"guid": g(DETACHED, DETACHED)});
    let doc = import(&standard(vec![inst(1, "Detached", json!({"styleIdForEffect": detached}), vec![root_entry(json!({"effects": [shadow(3.0)]}))])]));
    let n = named(&doc, "Detached");
    assert_eq!(radii(n), vec![3.0]);
    assert_eq!(style_of(&doc, n), None);
}

#[test]
fn nested_stale_and_empty_copies_never_replace_the_linked_style() {
    let own = json!({"styleIdForEffect": link(1)});
    let changes = standard(vec![
        inst(1, "Stale nested", own.clone(), vec![dot_entry(json!({"effects": [shadow(2.0)]}))]),
        inst(2, "Empty nested", own, vec![dot_entry(json!({"effects": []}))]),
    ]);
    for stage in ["import", "reopen"] {
        let doc = if stage == "import" { import(&changes) } else { reopened(&import(&changes)) };
        for name in ["Stale nested", "Empty nested"] {
            let dot = dot_of(&doc, name);
            assert_eq!(radii(dot), vec![8.0], "{stage}: {name} keeps the style's shadow");
            assert_eq!(style_of(&doc, dot).as_deref(), Some("Shadow/Card"), "{stage}: {name} keeps the link");
        }
    }
}

#[test]
fn a_nested_child_the_main_leaves_unlinked_takes_its_entry_effects() {
    let changes = doc_json(vec![shadow(8.0)], json!({}), json!({}), vec![inst(1, "Plain", json!({}), vec![dot_entry(json!({"effects": [shadow(2.0)]}))])]);
    let doc = import(&changes);
    let dot = dot_of(&doc, "Plain");
    assert_eq!(radii(dot), vec![2.0]);
    assert_eq!(style_of(&doc, dot), None);
}

#[test]
fn a_nested_child_relinked_to_another_style_keeps_the_link_after_save_and_reopen() {
    let changes = standard(vec![inst(
        1,
        "Relinked",
        json!({"styleIdForEffect": link(1)}),
        vec![dot_entry(json!({"styleIdForEffect": link(2), "effects": [shadow(12.0)]}))],
    )]);
    let mut doc = import(&changes);
    for round in 0..3 {
        let dot = dot_of(&doc, "Relinked");
        assert_eq!(radii(dot), vec![12.0], "round {round}");
        assert_eq!(style_of(&doc, dot).as_deref(), Some("Shadow/Raised"), "round {round}: the relink survives");
        doc = reopened(&doc);
    }
}

#[test]
fn an_instance_linked_only_through_the_legacy_key_keeps_the_style() {
    let changes = standard(vec![inst(
        1,
        "Legacy",
        json!({"inheritEffectStyleID": g(9, 1), "effects": [shadow(2.0)]}),
        vec![root_entry(json!({"effects": [shadow(2.0)]}))],
    )]);
    let doc = import(&changes);
    let n = named(&doc, "Legacy");
    assert_eq!(radii(n), vec![8.0]);
    assert_eq!(style_of(&doc, n).as_deref(), Some("Shadow/Card"));
}

#[test]
fn a_main_that_links_a_style_with_an_empty_cache_shows_the_style() {
    let linked_empty = json!({"styleIdForEffect": link(1)});
    let changes = doc_json(
        vec![shadow(8.0)],
        linked_empty.clone(),
        linked_empty.clone(),
        vec![inst(1, "Placed", linked_empty, vec![root_entry(json!({"effects": []}))])],
    );
    let doc = import(&changes);
    let n = named(&doc, "Placed");
    assert_eq!(radii(n), vec![8.0]);
    assert_eq!(style_of(&doc, n).as_deref(), Some("Shadow/Card"));
}

#[test]
fn a_cached_copy_of_the_styles_own_variable_does_not_beat_the_style() {
    // The style holds two shadows bound to variable 7:1; the cached copy holds one bound to the same variable.
    let card = vec![bound(shadow(8.0), (7, 1)), bound(shadow(4.0), (7, 1))];
    let copy = json!({"styleIdForEffect": link(1), "effects": [bound(shadow(8.0), (7, 1))]});
    let changes = doc_json(card, json!({}), json!({}), vec![inst(1, "Copy", copy.clone(), vec![root_entry(json!({"effects": copy["effects"].clone()}))])]);
    let doc = import(&changes);
    let n = named(&doc, "Copy");
    assert_eq!(radii(n), vec![8.0, 4.0], "the style's list wins");
    assert_eq!(style_of(&doc, n).as_deref(), Some("Shadow/Card"));
}

#[test]
fn a_variable_the_style_does_not_carry_keeps_the_nodes_own_effects() {
    let card = vec![bound(shadow(8.0), (7, 1))];
    let own = json!({"styleIdForEffect": link(1), "effects": [bound(shadow(2.0), (7, 99))]});
    let doc = import(&doc_json(card, json!({}), json!({}), vec![inst(1, "Bound", own, vec![])]));
    assert_eq!(radii(named(&doc, "Bound")), vec![2.0]);
}

/// A variable node whose modes hold an alias to another variable (`Some`) or a literal colour (`None`).
fn variable(local: u32, modes: Vec<Option<(u32, u32)>>) -> Value {
    let entries: Vec<Value> = modes
        .into_iter()
        .enumerate()
        .map(|(i, target)| {
            let value = match target {
                Some((s, l)) => json!({"alias": {"guid": g(s, l)}}),
                None => json!({"colorValue": {"r": 0.0, "g": 0.0, "b": 0.0, "a": 1.0}}),
            };
            json!({"modeID": g(1, i as u32), "variableData": {"dataType": if target.is_some() { "ALIAS" } else { "COLOR" }, "value": value}})
        })
        .collect();
    node(
        (7, local),
        (0, 2),
        &format!("v{local}"),
        "VARIABLE",
        &format!("var{local}"),
        json!({"variableResolvedType": "COLOR", "variableDataValues": {"entries": entries}}),
    )
}

/// The style binds variable 7:1 on two shadows; one instance holds a one-shadow copy bound to `copy`.
fn copy_bound_to(copy: (u32, u32), variables: Vec<Value>) -> Document {
    let card = vec![bound(shadow(8.0), (7, 1)), bound(shadow(4.0), (7, 1))];
    let own = json!({"styleIdForEffect": link(1), "effects": [bound(shadow(2.0), copy)]});
    let mut extra = variables;
    extra.push(inst(1, "Aliased", own, vec![]));
    import(&doc_json(card, json!({}), json!({}), extra))
}

#[test]
fn a_variable_that_aliases_one_the_style_carries_is_a_copy_and_the_style_wins() {
    // 7:2 aliases 7:1 directly; 7:4 reaches it through 7:3; 7:6 aliases it in both of its modes.
    for (copy, variables) in [
        ((7, 2), vec![variable(1, vec![None]), variable(2, vec![Some((7, 1))])]),
        ((7, 4), vec![variable(1, vec![None]), variable(3, vec![Some((7, 1))]), variable(4, vec![Some((7, 3))])]),
        ((7, 6), vec![variable(1, vec![None]), variable(6, vec![Some((7, 1)), Some((7, 1))])]),
    ] {
        let doc = copy_bound_to(copy, variables);
        assert_eq!(radii(named(&doc, "Aliased")), vec![8.0, 4.0], "variable {copy:?} resolves to the style's variable");
    }
}

#[test]
fn a_variable_that_resolves_to_something_the_style_does_not_carry_keeps_the_nodes_own_effects() {
    let doc = copy_bound_to((7, 2), vec![variable(1, vec![None]), variable(2, vec![Some((7, 3))]), variable(3, vec![None])]);
    assert_eq!(radii(named(&doc, "Aliased")), vec![2.0]);
}

#[test]
fn an_alias_cycle_does_not_hang_and_counts_as_foreign() {
    let doc = copy_bound_to((7, 2), vec![variable(1, vec![None]), variable(2, vec![Some((7, 3))]), variable(3, vec![Some((7, 2))])]);
    assert_eq!(radii(named(&doc, "Aliased")), vec![2.0]);
}

#[test]
fn library_variables_that_differ_only_in_version_are_the_same_variable() {
    let by_key = |version: &str| json!({"colorVar": {"dataType": "ALIAS", "value": {"alias": {"assetRef": {"key": "lib-var", "version": version}}}}});
    let with_var = |mut effect: Value, version: &str| {
        effect["colorVar"] = by_key(version)["colorVar"].clone();
        effect
    };
    let card = vec![with_var(shadow(8.0), "1:1"), with_var(shadow(4.0), "1:1")];
    let own = json!({"styleIdForEffect": link(1), "effects": [with_var(shadow(2.0), "2:9")]});
    let doc = import(&doc_json(card, json!({}), json!({}), vec![inst(1, "Versions", own, vec![])]));
    assert_eq!(radii(named(&doc, "Versions")), vec![8.0, 4.0]);
}

#[test]
fn a_variable_that_aliases_the_styles_variable_in_only_one_mode_is_foreign() {
    for modes in [vec![None, Some((7, 1))], vec![Some((7, 1)), None]] {
        let doc = copy_bound_to((7, 6), vec![variable(1, vec![None]), variable(6, modes.clone())]);
        assert_eq!(radii(named(&doc, "Aliased")), vec![2.0], "modes {modes:?} differ, so the binding is the node's own");
    }
}
