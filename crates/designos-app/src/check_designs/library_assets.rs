//! Library assets rule (`REQUIRE_ASSETS_FROM_SELECTED_LIBRARIES`): flags any visible layer using
//! an external component, style, or variable whose library is not enabled in this file.

use super::{Finding, LibrarySources, Rule};
use designos_doc::{Document, LibraryAssetKind, Node, NodeKind, REMOTE_LIBRARY_PREFIX};
use std::collections::BTreeSet;

fn is_library_enabled(doc: &Document, lib_id: &str) -> bool {
    !lib_id.starts_with(REMOTE_LIBRARY_PREFIX) && doc.library.enabled.iter().any(|r| r.id == lib_id)
}

fn resolve_library_name<'a>(doc: &'a Document, libs: &'a LibrarySources, lib_id: &'a str) -> &'a str {
    if let Some(name) = libs.known_names.get(lib_id) {
        return name.as_str();
    }
    if let Some(r) = doc.library.enabled.iter().find(|r| r.id == lib_id) {
        return r.name.as_str();
    }
    lib_id.strip_prefix(REMOTE_LIBRARY_PREFIX).unwrap_or(lib_id)
}

/// `src` is the main component's layer `n` mirrors, when it is inside an instance: what `n` has
/// in common with it is reported on the main component, not again here.
pub(super) fn check(doc: &Document, libs: &LibrarySources, n: &Node, src: Option<&Node>, out: &mut Vec<Finding>) {
    // 1. External component instance.
    if let NodeKind::Instance { component: Some(comp_id), .. } = &n.kind {
        let inherited_nested_instance = src.is_some_and(|s| matches!(&s.kind, NodeKind::Instance { component, .. } if *component == Some(*comp_id)));
        let comp_parent = doc.get(*comp_id).and_then(|c| c.parent);
        if !inherited_nested_instance {
            if let Some(ext) = doc.library.external.values().find(|e| {
                matches!(e.kind, LibraryAssetKind::Component | LibraryAssetKind::ComponentSet)
                    && (e.node == Some(*comp_id) || (comp_parent.is_some() && e.node == comp_parent))
            }) {
                if !is_library_enabled(doc, &ext.library_id) {
                    let asset = super::detached::instance_name(doc, *comp_id);
                    let lib_name = resolve_library_name(doc, libs, &ext.library_id);
                    out.push(Finding {
                        node: n.id,
                        node_name: n.name.clone(),
                        rule: Rule::LibraryAssets,
                        property: "component".into(),
                        message: format!("Uses \"{asset}\" from library \"{lib_name}\", which isn't enabled in this file"),
                        fix: None,
                        fix_label: String::new(),
                    });
                }
            }
        }
    }

    // 2. External shared styles.
    let mut style_slots: Vec<(&String, &String)> = n.style_refs.iter().collect();
    style_slots.sort_by_key(|(slot, _)| *slot);
    for (slot, style_key) in style_slots {
        if src.is_some_and(|s| s.style_refs.get(slot) == Some(style_key)) {
            continue;
        }
        if let Some(ext) = doc.library.external.values().find(|e| e.kind == LibraryAssetKind::Style && e.local_key.as_deref() == Some(style_key.as_str())) {
            if !is_library_enabled(doc, &ext.library_id) {
                let asset = doc.style_name(style_key);
                let lib_name = resolve_library_name(doc, libs, &ext.library_id);
                out.push(Finding {
                    node: n.id,
                    node_name: n.name.clone(),
                    rule: Rule::LibraryAssets,
                    property: slot.clone(),
                    message: format!("Uses \"{asset}\" from library \"{lib_name}\", which isn't enabled in this file"),
                    fix: None,
                    fix_label: String::new(),
                });
            }
        }
    }

    // 3. External bound variables (deduplicated per variable on the layer).
    let mut bindings: Vec<(&String, &String)> = n.variables.bindings.iter().collect();
    bindings.sort_by_key(|(prop, _)| *prop);
    let mut seen_vars: BTreeSet<&str> = BTreeSet::new();
    for (prop, var_id) in bindings {
        if src.is_some_and(|s| s.variables.bindings.get(prop) == Some(var_id)) {
            continue;
        }
        if !seen_vars.insert(var_id.as_str()) {
            continue;
        }
        let Some(v) = doc.variables.get(var_id) else { continue };
        let Some(lib_id) = super::hardcoded::variable_library_id(doc, v) else { continue };
        if !is_library_enabled(doc, lib_id) {
            let lib_name = resolve_library_name(doc, libs, lib_id);
            out.push(Finding {
                node: n.id,
                node_name: n.name.clone(),
                rule: Rule::LibraryAssets,
                property: prop.clone(),
                message: format!("Uses \"{}\" from library \"{lib_name}\", which isn't enabled in this file", v.name),
                fix: None,
                fix_label: String::new(),
            });
        }
    }
}
