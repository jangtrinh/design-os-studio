//! Editor side: check variable / style references on replayed node edits so missing tokens do
//! not leave dangling links.

use super::LivePatchReport;
use crate::{Document, Node};

/// Drops any newly added/changed variable binding or shared-style reference on `merged` whose
/// target variable or style does not exist in `doc`, restoring `cur`'s entry and recording the
/// reason in `report.skipped`. Returns the number of skipped links.
pub(super) fn filter_missing_token_links_on_update(doc: &Document, cur: &Node, merged: &mut Node, report: &mut LivePatchReport) -> usize {
    let mut skipped = 0;
    let new_bindings: Vec<(String, String)> =
        merged.variables.bindings.iter().filter(|(k, v)| cur.variables.bindings.get(*k) != Some(*v)).map(|(k, v)| (k.clone(), v.clone())).collect();
    for (prop, var_id) in new_bindings {
        if !doc.variables.contains_key(&var_id) {
            report.skipped.push(format!("update of node {}: variable '{var_id}' not found", cur.id.0));
            match cur.variables.bindings.get(&prop) {
                Some(prev) => {
                    merged.variables.bindings.insert(prop, prev.clone());
                }
                None => {
                    merged.variables.bindings.remove(&prop);
                }
            }
            skipped += 1;
        }
    }
    let new_modes: Vec<(String, String)> =
        merged.variables.modes.iter().filter(|(k, v)| cur.variables.modes.get(*k) != Some(*v)).map(|(k, v)| (k.clone(), v.clone())).collect();
    for (coll_id, _) in new_modes {
        if !doc.variable_collections.contains_key(&coll_id) {
            report.skipped.push(format!("update of node {}: variable collection '{coll_id}' not found", cur.id.0));
            match cur.variables.modes.get(&coll_id) {
                Some(prev) => {
                    merged.variables.modes.insert(coll_id, prev.clone());
                }
                None => {
                    merged.variables.modes.remove(&coll_id);
                }
            }
            skipped += 1;
        }
    }
    let new_styles: Vec<(String, String)> =
        merged.style_refs.iter().filter(|(k, v)| cur.style_refs.get(*k) != Some(*v)).map(|(k, v)| (k.clone(), v.clone())).collect();
    for (prop, style_id) in new_styles {
        if !doc.styles.contains_key(&style_id) {
            report.skipped.push(format!("update of node {}: style '{style_id}' not found", cur.id.0));
            match cur.style_refs.get(&prop) {
                Some(prev) => {
                    merged.style_refs.insert(prop, prev.clone());
                }
                None => {
                    merged.style_refs.remove(&prop);
                }
            }
            skipped += 1;
        }
    }
    skipped
}

/// Strips any variable binding or shared-style reference on inserted `nodes` whose target does
/// not exist in `doc`, recording the reason in `report.skipped`.
pub(super) fn filter_missing_token_links_on_insert(doc: &Document, nodes: &mut [Node], report: &mut LivePatchReport) {
    for n in nodes {
        let missing_vars: Vec<(String, String)> =
            n.variables.bindings.iter().filter(|(_, v)| !doc.variables.contains_key(*v)).map(|(k, v)| (k.clone(), v.clone())).collect();
        for (prop, var_id) in missing_vars {
            report.skipped.push(format!("insert of node {}: variable '{var_id}' not found", n.id.0));
            n.variables.bindings.remove(&prop);
        }
        let missing_colls: Vec<String> = n.variables.modes.keys().filter(|c| !doc.variable_collections.contains_key(*c)).cloned().collect();
        for coll_id in missing_colls {
            report.skipped.push(format!("insert of node {}: variable collection '{coll_id}' not found", n.id.0));
            n.variables.modes.remove(&coll_id);
        }
        let missing_styles: Vec<(String, String)> =
            n.style_refs.iter().filter(|(_, s)| !doc.styles.contains_key(*s)).map(|(k, s)| (k.clone(), s.clone())).collect();
        for (prop, style_id) in missing_styles {
            report.skipped.push(format!("insert of node {}: style '{style_id}' not found", n.id.0));
            n.style_refs.remove(&prop);
        }
    }
}
