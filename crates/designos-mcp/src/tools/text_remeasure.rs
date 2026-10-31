//! Re-fits hug text when something other than its own `text` changes: a text style it is
//! linked to (`set_style`) or a variable it is bound to (`set_variables`). The box follows
//! what the renderer draws: bindings and linked-style bindings applied, runs included.

use std::collections::{HashMap, HashSet};

use designos_doc::{
    resolve_node, set_bound_property, Command, Document, Node, NodeId, NodeKind, TextAutoResize, Variable, VariableCollection, VariableKind, VariableValue,
    Vec2,
};

/// Bound properties that change the size of a text box.
const TEXT_BOX_KEYS: [&str; 10] =
    ["fontSize", "fontWeight", "lineHeight", "letterSpacing", "paragraphSpacing", "paragraphIndent", "listSpacing", "characters", "fontFamily", "fontStyle"];

/// Sets the size of a hug text (`WidthAndHeight` or `Height`) to the renderer's measure of
/// what `node` draws in `doc`. Other nodes and fixed-size text are left alone.
pub(crate) fn resize_hug_text(doc: &Document, node: &mut Node) {
    let NodeKind::Text(tp) = &node.kind else { return };
    let mode = tp.auto_resize;
    if !matches!(mode, TextAutoResize::WidthAndHeight | TextAutoResize::Height) {
        return;
    }
    let size = {
        let drawn = resolve_node(doc, node);
        let NodeKind::Text(drawn_tp) = &drawn.kind else { return };
        match mode {
            TextAutoResize::WidthAndHeight => crate::tools::measure_text(drawn_tp, None),
            _ => Vec2::new(node.size.x, crate::tools::measure_text(drawn_tp, Some(node.size.x.max(20.0))).y),
        }
    };
    node.size = size;
}

/// The variables a `set_variables` op writes or deletes, as they were before it ran.
pub(crate) struct VariablesBefore {
    variables: Vec<(String, Option<Variable>)>,
    collections: Vec<(String, Option<VariableCollection>)>,
}

impl VariablesBefore {
    pub(crate) fn capture(doc: &Document, written: &[&str], collection_ids: &[&str]) -> Self {
        Self {
            variables: written.iter().map(|id| ((*id).to_string(), doc.variables.get(*id).cloned())).collect(),
            collections: collection_ids.iter().map(|id| ((*id).to_string(), doc.variable_collections.get(*id).cloned())).collect(),
        }
    }

    /// Ids of the written variables whose value changed and can change a text box (numbers
    /// and strings), plus every variable whose alias chain reaches one of them.
    fn changed_text_variables(&self, doc: &Document) -> HashSet<String> {
        let mut changed: HashSet<String> = HashSet::new();
        for (id, before) in &self.variables {
            let after = doc.variables.get(id);
            if after == before.as_ref() {
                continue;
            }
            let kind = after.or(before.as_ref()).map(|v| v.kind);
            if matches!(kind, Some(VariableKind::Number | VariableKind::String)) {
                changed.insert(id.clone());
            }
        }
        for (id, before) in &self.collections {
            let Some(after) = doc.variable_collections.get(id) else { continue };
            if Some(after) == before.as_ref() {
                continue;
            }
            for variable_id in &after.variables {
                if matches!(doc.variables.get(variable_id).map(|v| v.kind), Some(VariableKind::Number | VariableKind::String)) {
                    changed.insert(variable_id.clone());
                }
            }
        }
        if changed.is_empty() {
            return changed;
        }
        let mut aliased_by: HashMap<&str, Vec<&str>> = HashMap::new();
        for (id, v) in &doc.variables {
            for value in v.values.values() {
                if let VariableValue::Alias(target) = value {
                    aliased_by.entry(target.as_str()).or_default().push(id.as_str());
                }
            }
        }
        let mut stack: Vec<String> = changed.iter().cloned().collect();
        while let Some(id) = stack.pop() {
            for dependent in aliased_by.get(id.as_str()).into_iter().flatten() {
                if changed.insert((*dependent).to_string()) {
                    stack.push((*dependent).to_string());
                }
            }
        }
        changed
    }
}

/// `(property, variable)` pairs that size `node`'s text: its own bindings, then those of its
/// linked text style that the node does not override.
fn text_box_bindings<'a>(doc: &'a Document, node: &'a Node) -> Vec<(&'a str, &'a str)> {
    let mut out: Vec<(&str, &str)> =
        node.variables.bindings.iter().filter(|(k, _)| TEXT_BOX_KEYS.contains(&k.as_str())).map(|(k, v)| (k.as_str(), v.as_str())).collect();
    if let Some(meta) = node.style_refs.get("text").and_then(|key| doc.style_meta.get(key)) {
        for (k, v) in &meta.bindings {
            if TEXT_BOX_KEYS.contains(&k.as_str()) && !node.variables.bindings.contains_key(k) {
                out.push((k.as_str(), v.as_str()));
            }
        }
    }
    out
}

/// Hug text with a bound text property, listed once for a run of `set_variables` ops in
/// one batch. Any other op can add, rebind or delete text, so it calls [`Self::invalidate`].
#[derive(Default)]
pub(crate) struct BoundTextIndex {
    ids: Option<Vec<NodeId>>,
}

impl BoundTextIndex {
    pub(crate) fn invalidate(&mut self) {
        self.ids = None;
    }

    fn ids(&mut self, doc: &Document) -> &[NodeId] {
        self.ids.get_or_insert_with(|| {
            doc.nodes
                .values()
                .filter(|n| matches!(&n.kind, NodeKind::Text(tp) if matches!(tp.auto_resize, TextAutoResize::WidthAndHeight | TextAutoResize::Height)))
                .filter(|n| !n.variables.bindings.is_empty() || n.style_refs.contains_key("text"))
                .filter(|n| !text_box_bindings(doc, n).is_empty())
                .map(|n| n.id)
                .collect()
        })
    }

    /// One `Update` per hug text whose box changes because a variable it is bound to changed
    /// in the `set_variables` op that `before` was captured for. `doc` is the document after
    /// that op. Colour and boolean variables, and no-op upserts, cost nothing.
    pub(crate) fn remeasure_after(&mut self, doc: &Document, before: &VariablesBefore) -> Vec<Command> {
        let changed = before.changed_text_variables(doc);
        if changed.is_empty() {
            return Vec::new();
        }
        let mut cmds = Vec::new();
        for &id in self.ids(doc) {
            let Some(node) = doc.get(id) else { continue };
            let hits: Vec<(&str, &str)> = text_box_bindings(doc, node).into_iter().filter(|(_, var)| changed.contains(*var)).collect();
            if hits.is_empty() {
                continue;
            }
            let mut updated = node.clone();
            for (key, var) in hits {
                if let Some(value) = doc.resolve_variable(id, var) {
                    set_bound_property(&mut updated, key, &value);
                }
            }
            resize_hug_text(doc, &mut updated);
            if &updated != node {
                cmds.push(Command::Update { node: Box::new(updated) });
            }
        }
        cmds
    }
}
