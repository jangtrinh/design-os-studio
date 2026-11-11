//! Edge cases of the rule "a linked effect style wins over a stale or empty cached copy": a detached
//! instance, nested override entries, a legacy-only link, a master that links a style with an empty
//! cache, variable-bound copies, and a nested relink through a `.fig` save.
use designos_doc::*;
use designos_fig::{decode_fig_to_json, export_fig, import_fig, import_fig_node_changes};
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

fn saved_record(doc: &Document, name: &str) -> Value {
    let saved = export_fig(doc).expect("export");
    let decoded = decode_fig_to_json(&saved).expect("decode");
    decoded["message"]["nodeChanges"]
        .as_array()
        .expect("nodeChanges")
        .iter()
        .find(|nc| nc["name"].as_str() == Some(name))
        .unwrap_or_else(|| panic!("no saved record named {name}"))
        .clone()
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

fn card_binding_7_1() -> Vec<Value> {
    vec![bound(shadow(8.0), (7, 1))]
}

#[test]
fn a_nested_entry_that_binds_a_foreign_variable_keeps_its_own_effects() {
    let linked = json!({"styleIdForEffect": link(1), "effects": [shadow(8.0)]});
    let own = json!({"styleIdForEffect": link(1)});
    let changes = doc_json(
        card_binding_7_1(),
        linked.clone(),
        linked,
        vec![inst(1, "Foreign nested", own, vec![dot_entry(json!({"effects": [bound(shadow(2.0), (7, 99))]}))])],
    );
    let doc = import(&changes);
    let dot = dot_of(&doc, "Foreign nested");
    assert_eq!(radii(dot), vec![2.0]);
    assert_eq!(style_of(&doc, dot).as_deref(), Some("Shadow/Card"), "the link survives, as it does on a node");
    let again = reopened(&doc);
    let dot = dot_of(&again, "Foreign nested");
    assert_eq!((radii(dot), style_of(&again, dot).as_deref()), (vec![2.0], Some("Shadow/Card")), "and it survives a save");
}

#[test]
fn a_root_entry_that_binds_a_foreign_variable_keeps_its_own_effects() {
    let own = json!({"styleIdForEffect": link(1)});
    let changes = doc_json(
        card_binding_7_1(),
        json!({}),
        json!({}),
        vec![inst(1, "Foreign root", own, vec![root_entry(json!({"effects": [bound(shadow(2.0), (7, 99))]}))])],
    );
    let doc = import(&changes);
    let n = named(&doc, "Foreign root");
    assert_eq!(radii(n), vec![2.0]);
    assert_eq!(style_of(&doc, n).as_deref(), Some("Shadow/Card"), "the link survives, as it does on a node");
}

#[test]
fn an_outer_edit_of_a_child_the_inner_instance_detached_keeps_the_edit() {
    let detach_and_set = json!({"guidPath": {"guids": [g(1, 11)]}, "styleIdForEffect": {"guid": g(DETACHED, DETACHED)}, "effects": [shadow(3.0)]});
    let card = node((1, 20), (0, 1), "b", "SYMBOL", "Card", json!({"size": {"x": 44.0, "y": 22.0}}));
    let badge_in = node(
        (1, 21),
        (1, 20),
        "a",
        "INSTANCE",
        "BadgeIn",
        json!({"size": {"x": 44.0, "y": 22.0}, "symbolData": {"symbolID": g(1, 10), "symbolOverrides": [detach_and_set]}}),
    );
    let outer = node(
        (3, 1),
        (0, 1),
        "c",
        "INSTANCE",
        "Outer",
        json!({"size": {"x": 44.0, "y": 22.0}, "symbolData": {"symbolID": g(1, 20), "symbolOverrides": [
            {"guidPath": {"guids": [g(1, 21), g(1, 11)]}, "effects": [shadow(5.0)]}
        ]}}),
    );
    let doc = import(&standard(vec![card, badge_in, outer]));
    let badge = doc.get(named(&doc, "Outer").children[0]).expect("the nested instance");
    let dot = doc.get(badge.children[0]).expect("its Dot");
    assert_eq!(radii(dot), vec![5.0]);
    assert_eq!(style_of(&doc, dot), None);
}

#[test]
fn a_deep_chain_of_multi_mode_aliases_finishes() {
    // Twenty variables, four modes each, every mode aliasing the next one; the last aliases the style's variable.
    // An unmemoised walk makes 4^20 steps; the answer is that the chain is a copy.
    let mut variables = vec![variable(1, vec![None])];
    for local in 101..=120 {
        let next = if local == 120 { (7, 1) } else { (7, local + 1) };
        variables.push(variable(local, vec![Some(next); 4]));
    }
    let doc = copy_bound_to((7, 101), variables);
    assert_eq!(radii(named(&doc, "Aliased")), vec![8.0, 4.0]);
}

#[test]
fn a_foreign_binding_on_a_node_keeps_the_link() {
    let card = card_binding_7_1();
    let own = json!({"styleIdForEffect": link(1), "effects": [bound(shadow(2.0), (7, 99))]});
    let doc = import(&doc_json(card, json!({}), json!({}), vec![inst(1, "Node level", own, vec![])]));
    let n = named(&doc, "Node level");
    assert_eq!((radii(n), style_of(&doc, n).as_deref()), (vec![2.0], Some("Shadow/Card")));
}

#[test]
fn a_very_long_alias_chain_imports_on_a_small_stack() {
    // 50,000 single-mode variables, each aliasing the next; the last one aliases the style's variable.
    let handle = std::thread::Builder::new()
        .stack_size(2 << 20)
        .spawn(|| {
            let mut variables = vec![variable(1, vec![None])];
            for local in 101..=50_100 {
                let next = if local == 50_100 { (7, 1) } else { (7, local + 1) };
                variables.push(variable(local, vec![Some(next)]));
            }
            let doc = copy_bound_to((7, 101), variables);
            radii(named(&doc, "Aliased"))
        })
        .expect("spawn");
    // Past the depth bound the chain counts as a different variable, so the node keeps its own list.
    assert_eq!(handle.join().expect("the import must not overflow the stack"), vec![2.0]);
}

/// `CardM` holds the instance `BadgeIn` of Badge. BadgeIn's own self entry is `self_entry`; the outer instance of
/// `CardM` restates `BadgeIn` with `outer_fields` and no style key.
fn nested_instance_doc(self_entry: Value, outer_fields: Value) -> Document {
    let card_m = node((1, 20), (0, 1), "b", "SYMBOL", "CardM", json!({"size": {"x": 44.0, "y": 22.0}}));
    let badge_in = node(
        (1, 21),
        (1, 20),
        "a",
        "INSTANCE",
        "BadgeIn",
        json!({"size": {"x": 44.0, "y": 22.0}, "styleIdForEffect": link(1), "symbolData": {"symbolID": g(1, 10), "symbolOverrides": [self_entry]}}),
    );
    let mut entry = json!({"guidPath": {"guids": [g(1, 21)]}});
    for (k, v) in outer_fields.as_object().expect("entry fields") {
        entry[k] = v.clone();
    }
    let outer = node(
        (3, 1),
        (0, 1),
        "c",
        "INSTANCE",
        "Outer",
        json!({"size": {"x": 44.0, "y": 22.0}, "symbolData": {"symbolID": g(1, 20), "symbolOverrides": [entry]}}),
    );
    import(&standard(vec![card_m, badge_in, outer]))
}

#[test]
fn an_outer_foreign_binding_on_an_instance_its_own_entry_relinked_keeps_the_relinked_style() {
    let relink = root_entry(json!({"styleIdForEffect": link(2), "effects": [shadow(12.0)]}));
    let doc = nested_instance_doc(relink, json!({"effects": [bound(shadow(2.0), (7, 99))]}));
    let badge = doc.get(named(&doc, "Outer").children[0]).expect("BadgeIn");
    assert_eq!((radii(badge), style_of(&doc, badge).as_deref()), (vec![2.0], Some("Shadow/Raised")));
    let again = reopened(&doc);
    let badge = again.get(named(&again, "Outer").children[0]).expect("BadgeIn after save");
    assert_eq!((radii(badge), style_of(&again, badge).as_deref()), (vec![2.0], Some("Shadow/Raised")), "and after a save");
}

#[test]
fn an_outer_edit_of_an_instance_its_own_entry_detached_keeps_the_edit() {
    let detach = root_entry(json!({"styleIdForEffect": {"guid": g(DETACHED, DETACHED)}, "effects": [shadow(3.0)]}));
    let doc = nested_instance_doc(detach, json!({"effects": [shadow(5.0)]}));
    let badge = doc.get(named(&doc, "Outer").children[0]).expect("BadgeIn");
    assert_eq!((radii(badge), style_of(&doc, badge)), (vec![5.0], None));
}

#[test]
fn a_root_instance_linked_only_through_the_legacy_key_keeps_its_link_with_a_foreign_binding() {
    let own = json!({"inheritEffectStyleID": g(9, 1)});
    let entry = root_entry(json!({"effects": [bound(shadow(2.0), (7, 99))]}));
    let doc = import(&doc_json(card_binding_7_1(), json!({}), json!({}), vec![inst(1, "Legacy foreign", own, vec![entry])]));
    let n = named(&doc, "Legacy foreign");
    assert_eq!((radii(n), style_of(&doc, n).as_deref()), (vec![2.0], Some("Shadow/Card")));
    let again = reopened(&doc);
    let n = named(&again, "Legacy foreign");
    assert_eq!((radii(n), style_of(&again, n).as_deref()), (vec![2.0], Some("Shadow/Card")), "and after a save");
}

#[test]
fn a_root_instance_that_inherits_its_link_keeps_it_with_a_foreign_binding() {
    let linked = json!({"styleIdForEffect": link(1), "effects": [shadow(8.0)]});
    let entry = root_entry(json!({"effects": [bound(shadow(2.0), (7, 99))]}));
    let doc = import(&doc_json(card_binding_7_1(), linked.clone(), linked, vec![inst(1, "Inherits", json!({}), vec![entry])]));
    let n = named(&doc, "Inherits");
    assert_eq!((radii(n), style_of(&doc, n).as_deref()), (vec![2.0], Some("Shadow/Card")));
    assert_eq!(saved_record(&doc, "Inherits")["styleIdForEffect"], link(1));
    let again = reopened(&doc);
    let n = named(&again, "Inherits");
    assert_eq!((radii(n), style_of(&again, n).as_deref()), (vec![2.0], Some("Shadow/Card")), "the save writes no detach marker");
}

#[test]
fn two_self_entries_without_record_link_keep_a_detach_in_either_order() {
    let linked = json!({"styleIdForEffect": link(1), "effects": [bound(shadow(8.0), (7, 1))]});
    let detach = root_entry(json!({"styleIdForEffect": {"guid": g(DETACHED, DETACHED)}}));
    let gone = root_entry(json!({"styleIdForEffect": link(77)}));
    for (kind, clear) in [("detached", detach), ("gone style", gone)] {
        for (row, fx) in [("stale", shadow(3.0)), ("copy of style variable", bound(shadow(3.0), (7, 1))), ("foreign variable", bound(shadow(3.0), (7, 99)))] {
            let restate = root_entry(json!({"effects": [fx]}));
            for (order, entries) in [("detach first", vec![clear.clone(), restate.clone()]), ("restate first", vec![restate, clear.clone()])] {
                let doc = import(&doc_json(card_binding_7_1(), linked.clone(), linked.clone(), vec![inst(1, "Detached two", json!({}), entries)]));
                for (stage, doc) in [("import", doc.clone()), ("reopen", reopened(&doc))] {
                    let n = named(&doc, "Detached two");
                    assert_eq!((radii(n), style_of(&doc, n)), (vec![3.0], None), "{kind}, {row}, {order}, {stage}");
                }
            }
        }
    }
}

#[test]
fn two_self_entries_keep_the_shadow_and_the_library_style_the_first_one_names() {
    // The record links a style that is gone; the first self entry names a library style the file lacks; the
    // second restates the effects without a key.
    let own = json!({"styleIdForEffect": link(77), "effects": [shadow(8.0)]});
    let library = root_entry(json!({"styleIdForEffect": {"assetRef": {"key": "absent-lib"}}}));
    let stale = root_entry(json!({"effects": [shadow(8.0)]}));
    let doc = import(&standard(vec![inst(1, "Two entries", own, vec![library, stale])]));
    for (stage, doc) in [("import", doc.clone()), ("reopen", reopened(&doc))] {
        let n = named(&doc, "Two entries");
        assert_eq!(radii(n), vec![8.0], "{stage}: the shadow is kept");
        assert_eq!(n.style_refs.get("effect").map(String::as_str), Some("key:absent-lib"), "{stage}: the library style the entry names");
    }
}

/// An instance with no link keys of its own; its record caches the main's effects, as Figma writes it.
fn inherited(master: Value, entry_effects: Vec<Value>, name: &str) -> Document {
    let cache = json!({"effects": master["effects"].clone()});
    inherited_cached(master, cache, entry_effects, name)
}

/// Like [`inherited`], with the record's own fields (`cache`) given: Figma may cache a scaled copy, or none.
fn inherited_cached(master: Value, cache: Value, entry_effects: Vec<Value>, name: &str) -> Document {
    let entry = root_entry(json!({"effects": entry_effects}));
    import(&doc_json(card_binding_7_1(), master.clone(), master, vec![inst(1, name, cache, vec![entry])]))
}

#[test]
fn a_stale_copy_under_an_inherited_link_keeps_the_mains_link() {
    let master = json!({"styleIdForEffect": link(1), "effects": [bound(shadow(8.0), (7, 1))]});
    let doc = inherited(master.clone(), vec![shadow(8.0)], "Inherited stale");
    for (stage, doc) in [("import", doc.clone()), ("reopen", reopened(&doc))] {
        let n = named(&doc, "Inherited stale");
        assert_eq!((radii(n), style_of(&doc, n).as_deref()), (vec![8.0], Some("Shadow/Card")), "{stage}");
    }
    // The record's own cache differs from the main's (a resized instance): the cache is what Figma shows, and it stays.
    let scaled = inherited_cached(master, json!({"effects": [bound(shadow(4.0), (7, 1))]}), vec![shadow(8.0)], "Inherited scaled");
    let n = named(&scaled, "Inherited scaled");
    assert_eq!((radii(n), style_of(&scaled, n).as_deref()), (vec![4.0], Some("Shadow/Card")), "scaled cache, import");
    let reopened = reopened(&scaled);
    let n = named(&reopened, "Inherited scaled");
    assert_eq!((radii(n), style_of(&reopened, n).as_deref()), (vec![4.0], Some("Shadow/Card")), "scaled cache, reopen");
}

/// Figma caches a scaled copy of the linked style on an instance that was resized. A save keeps that cache.
#[test]
fn a_scaled_record_cache_under_an_inherited_link_survives_saves() {
    let master = json!({"styleIdForEffect": link(1), "effects": [bound(shadow(8.0), (7, 1))]});
    for (tag, entry) in [("stale entry", vec![shadow(8.0)]), ("entry copy of the style's variable", vec![bound(shadow(8.0), (7, 1))])] {
        let cache = json!({"effects": [bound(shadow(4.0), (7, 1))]});
        let mut doc = inherited_cached(master.clone(), cache, entry, "Scaled");
        let n = named(&doc, "Scaled");
        assert_eq!((radii(n), style_of(&doc, n).as_deref()), (vec![4.0], Some("Shadow/Card")), "{tag}, import");
        assert!(saved_record(&doc, "Scaled").get("styleIdForEffect").is_none(), "{tag}: record keeps no styleIdForEffect pin");
        for round in 1..=3 {
            doc = reopened(&doc);
            let n = named(&doc, "Scaled");
            assert_eq!((radii(n), style_of(&doc, n).as_deref()), (vec![4.0], Some("Shadow/Card")), "{tag}, save {round}");
        }
    }
    let foreign_cache = inherited_cached(master, json!({"effects": [bound(shadow(4.0), (7, 99))]}), vec![shadow(8.0)], "Foreign cache");
    assert_eq!(saved_record(&foreign_cache, "Foreign cache")["styleIdForEffect"], link(1), "a foreign-bound record cache keeps its pin");
}

/// The same in a file big enough to defer its pages, which flags an instance's overrides from the keys of its own
/// entry instead of from what the loader made of them.
#[test]
fn a_scaled_record_cache_survives_saves_in_a_file_with_deferred_pages() {
    let master = json!({"styleIdForEffect": link(1), "effects": [bound(shadow(8.0), (7, 1))]});
    let entry = root_entry(json!({"effects": [shadow(8.0)]}));
    let cache = json!({"effects": [bound(shadow(4.0), (7, 1))]});
    let mut changes = doc_json(card_binding_7_1(), master.clone(), master, vec![inst(1, "Scaled", cache, vec![entry])]);
    let list = changes.as_array_mut().expect("node list");
    for i in 0..5100u32 {
        list.push(node((8, i), (0, 1), &format!("p{i:05}"), "RECTANGLE", "pad", json!({"size": {"x": 1.0, "y": 1.0}})));
    }
    let mut doc = import(&changes);
    assert_eq!(radii(named(&doc, "Scaled")), vec![4.0], "import");
    for round in 1..=2 {
        doc = reopened(&doc);
        assert_eq!(radii(named(&doc, "Scaled")), vec![4.0], "save {round}");
    }
}

/// A record with no `effects` key of its own, under an inherited link, shows the same list before and after a save.
#[test]
fn a_record_without_cached_effects_under_an_inherited_link_survives_a_save() {
    let master = json!({"styleIdForEffect": link(1), "effects": [bound(shadow(8.0), (7, 1))]});
    let doc = inherited_cached(master, json!({}), vec![shadow(2.0)], "Uncached");
    let n = named(&doc, "Uncached");
    let before = (radii(n), style_of(&doc, n));
    let after = reopened(&doc);
    let n = named(&after, "Uncached");
    assert_eq!((radii(n), style_of(&after, n)), before, "a save keeps the render and the link");
}

#[test]
fn a_copy_of_the_styles_own_variable_under_an_inherited_link_keeps_the_mains_link() {
    let master = json!({"styleIdForEffect": link(1), "effects": [bound(shadow(8.0), (7, 1))]});
    let doc = inherited(master, vec![bound(shadow(8.0), (7, 1))], "Inherited copy");
    for (stage, doc) in [("import", doc.clone()), ("reopen", reopened(&doc))] {
        let n = named(&doc, "Inherited copy");
        assert_eq!((radii(n), style_of(&doc, n).as_deref()), (vec![8.0], Some("Shadow/Card")), "{stage}");
    }
}

#[test]
fn a_main_linked_to_a_library_style_keeps_that_link_under_a_foreign_binding() {
    let lib = json!({"assetRef": {"key": "absent-lib"}});
    let master = json!({"styleIdForEffect": lib.clone(), "effects": [shadow(8.0)]});
    let entry = root_entry(json!({"effects": [bound(shadow(2.0), (7, 99))]}));
    let own_doc = import(&doc_json(
        card_binding_7_1(),
        master.clone(),
        master.clone(),
        vec![inst(1, "Own library", json!({"styleIdForEffect": lib.clone(), "effects": [shadow(8.0)]}), vec![entry])],
    ));
    let inh_doc = inherited(master, vec![bound(shadow(2.0), (7, 99))], "Inherited library");
    let want = vec![Effect::DropShadow {
        color: Color { r: 0.0, g: 0.0, b: 0.0, a: 0.25 },
        offset: Vec2::new(0.0, 2.0),
        radius: 2.0,
        spread: 0.0,
        visible: true,
        show_behind: false,
        blend: BlendMode::Normal,
    }];
    for (name, doc) in [("Inherited library", inh_doc), ("Own library", own_doc)] {
        assert_eq!(saved_record(&doc, name)["styleIdForEffect"], lib);
        for (stage, doc) in [("import", doc.clone()), ("reopen", reopened(&doc))] {
            let n = named(&doc, name);
            assert_eq!(n.style_refs.get("effect").map(String::as_str), Some("key:absent-lib"), "{name}, {stage}: link");
            assert_eq!(radii(n), vec![2.0], "{name}, {stage}: radii");
            assert_eq!(n.effects, want, "{name}, {stage}: effects");
            assert_eq!(n.variables.bindings.get("effects/0/color").map(String::as_str), Some("VariableID:7:99"), "{name}, {stage}: binding");
        }
    }
}

/// `BadgeIn` in `CardM` links Card (`record_link` holds the link keys) and binds a foreign variable in its own self
/// entry; the outer instance restates `BadgeIn` with a copy of the style's own variable.
fn dropped_copy_doc(record_link: Value) -> Document {
    let card_m = node((1, 20), (0, 1), "b", "SYMBOL", "CardM", json!({"size": {"x": 44.0, "y": 22.0}}));
    let mut badge_fields = json!({"size": {"x": 44.0, "y": 22.0}, "symbolData": {"symbolID": g(1, 10), "symbolOverrides": [
        root_entry(json!({"effects": [bound(shadow(2.0), (7, 99))]}))
    ]}});
    for (k, v) in record_link.as_object().expect("link keys") {
        badge_fields[k] = v.clone();
    }
    let badge_in = node((1, 21), (1, 20), "a", "INSTANCE", "BadgeIn", badge_fields);
    let entry = json!({"guidPath": {"guids": [g(1, 21)]}, "effects": [bound(shadow(8.0), (7, 1))]});
    let outer = node(
        (3, 1),
        (0, 1),
        "c",
        "INSTANCE",
        "Outer",
        json!({"size": {"x": 44.0, "y": 22.0}, "symbolData": {"symbolID": g(1, 20), "symbolOverrides": [entry]}}),
    );
    let linked = json!({"styleIdForEffect": link(1), "effects": [bound(shadow(8.0), (7, 1))]});
    import(&doc_json(card_binding_7_1(), linked.clone(), linked, vec![card_m, badge_in, outer]))
}

#[test]
fn a_dropped_copy_leaves_no_binding_behind_so_a_save_keeps_the_render() {
    for (label, link_keys) in [("modern link", json!({"styleIdForEffect": link(1)})), ("legacy link", json!({"inheritEffectStyleID": g(9, 1)}))] {
        let doc = dropped_copy_doc(link_keys);
        let before = radii(doc.get(named(&doc, "Outer").children[0]).expect("BadgeIn"));
        let again = reopened(&doc);
        let after = radii(again.get(named(&again, "Outer").children[0]).expect("BadgeIn after save"));
        assert_eq!(before, after, "{label}: a save does not change the render");
    }
}
