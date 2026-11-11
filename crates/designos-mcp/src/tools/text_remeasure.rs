//! Re-fits hug text when something other than its own `text` changes: a text style it is
//! linked to (`set_style`) or a variable it is bound to (`set_variables`). The box follows
//! what the renderer draws: bindings and linked-style bindings applied, runs included.

use std::collections::{BTreeMap, HashMap, HashSet};

use designos_doc::{
    resolve_node, set_bound_property, Command, Document, Node, NodeId, NodeKind, TextAutoResize, Variable, VariableCollection, VariableKind, VariableValue,
    Vec2,
};

/// Bound properties that change the size of a text box.
const TEXT_BOX_KEYS: [&str; 11] = [
    "fontSize",
    "fontWeight",
    "lineHeight",
    "letterSpacing",
    "paragraphSpacing",
    "paragraphIndent",
    "listSpacing",
    "characters",
    "fontFamily",
    "fontStyle",
    "maxWidth",
];

/// Bound properties on a node that the auto-layout solver reads directly.
const LAYOUT_BOX_KEYS: [&str; 13] = [
    "width",
    "height",
    "minWidth",
    "maxWidth",
    "minHeight",
    "maxHeight",
    "visible",
    "itemSpacing",
    "counterAxisSpacing",
    "paddingTop",
    "paddingRight",
    "paddingBottom",
    "paddingLeft",
];

/// Bound stroke widths. The solver reads them only on a frame that
/// [`crate::tools::relayout_scope::stroke_inset_read_by_solver`] names.
const STROKE_WEIGHT_KEYS: [&str; 5] = ["strokeWeight", "strokeTopWeight", "strokeRightWeight", "strokeBottomWeight", "strokeLeftWeight"];

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
            TextAutoResize::WidthAndHeight => designos_doc::text_measure::measure_hug_text_with(drawn_tp, drawn.layout_child.max_w, crate::tools::measure_text),
            _ => Vec2::new(node.size.x, crate::tools::measure_text(drawn_tp, Some(node.size.x.max(20.0))).y),
        }
    };
    node.size = size;
}

/// Snapshot of the document's variable and collection maps before a `set_variables` op runs,
/// including collections that `unmap_parent_mode` or override edits rewrite.
pub(crate) struct VariablesBefore {
    variables: BTreeMap<String, Variable>,
    collections: BTreeMap<String, VariableCollection>,
}

impl VariablesBefore {
    pub(crate) fn capture(doc: &Document) -> Self {
        Self { variables: doc.variables.clone(), collections: doc.variable_collections.clone() }
    }

    /// Resolves `variable` at `node` with [`Document::resolve_variable`] against the pre-op
    /// `variables` and `variable_collections` maps.
    fn resolve_in_pre_op(&mut self, doc: &mut Document, node: NodeId, variable: &str) -> Option<VariableValue> {
        std::mem::swap(&mut doc.variables, &mut self.variables);
        std::mem::swap(&mut doc.variable_collections, &mut self.collections);
        let value = doc.resolve_variable(node, variable);
        std::mem::swap(&mut doc.variables, &mut self.variables);
        std::mem::swap(&mut doc.variable_collections, &mut self.collections);
        value
    }

    /// Ids of the variables of one of `kinds` whose resolved value can have changed in this op: the
    /// written ones whose value changed, every variable of a collection whose default mode, mode
    /// list, or extension state changed (they pick which value each variable resolves to), and every
    /// variable whose alias chain reaches one of those.
    fn changed_variables(&self, doc: &Document, kinds: &[VariableKind]) -> HashSet<String> {
        let mut changed: HashSet<String> = HashSet::new();
        for (id, after) in &doc.variables {
            if self.variables.get(id) != Some(after) && kinds.contains(&after.kind) {
                changed.insert(id.clone());
            }
        }
        for (id, before) in &self.variables {
            if !doc.variables.contains_key(id) && kinds.contains(&before.kind) {
                changed.insert(id.clone());
            }
        }
        let all_collections: HashSet<&String> = self.collections.keys().chain(doc.variable_collections.keys()).collect();
        for id in all_collections {
            fn modes_of(c: &VariableCollection) -> (&str, Vec<&str>, &designos_doc::FigmaExtensions) {
                (&c.default_mode, c.modes.iter().map(|m| m.id.as_str()).collect(), &c.extensions)
            }
            let before = self.collections.get(id);
            let after = doc.variable_collections.get(id);
            if after.map(modes_of) == before.map(modes_of) {
                continue;
            }
            let is_ext = before.is_some_and(VariableCollection::is_extension) || after.is_some_and(VariableCollection::is_extension);
            for var_id in after.into_iter().chain(before).flat_map(|c| c.variables.iter().cloned()).chain(doc.inherited_variable_ids(id)) {
                if doc.variables.get(&var_id).or_else(|| self.variables.get(&var_id)).is_some_and(|v| kinds.contains(&v.kind)) {
                    changed.insert(var_id);
                }
            }
            changed.extend(
                doc.variables
                    .values()
                    .filter(|v| (v.collection == *id || is_ext || v.values.keys().any(|k| k.contains('+'))) && kinds.contains(&v.kind))
                    .map(|v| v.id.clone()),
            );
        }
        if changed.is_empty() {
            return changed;
        }
        let mut aliased_by: HashMap<String, Vec<String>> = HashMap::new();
        for (id, v) in doc.variables.iter().chain(self.variables.iter()) {
            for value in v.values.values() {
                if let VariableValue::Alias(target) = value {
                    aliased_by.entry(target.clone()).or_default().push(id.clone());
                }
            }
        }
        for c in doc.variable_collections.values().chain(self.collections.values()) {
            if let Some(ext) = c.extension() {
                for (var_id, by_mode) in ext.overrides {
                    for value in by_mode.into_values() {
                        if let VariableValue::Alias(target) = value {
                            aliased_by.entry(target).or_default().push(var_id.clone());
                        }
                    }
                }
            }
        }
        let mut stack: Vec<String> = changed.iter().cloned().collect();
        while let Some(id) = stack.pop() {
            for dependent in aliased_by.get(&id).into_iter().flatten() {
                if changed.insert(dependent.clone()) {
                    stack.push(dependent.clone());
                }
            }
        }
        changed
    }

    /// Variables that can change a text box (numbers and strings).
    fn changed_text_variables(&self, doc: &Document) -> HashSet<String> {
        self.changed_variables(doc, &[VariableKind::Number, VariableKind::String])
    }

    /// Nodes whose layout-affecting variable bindings reference a variable changed in this op
    /// (numbers and booleans) whose resolved value at the node actually changed.
    pub(crate) fn layout_bound_nodes(&mut self, doc: &mut Document) -> Vec<NodeId> {
        let changed = self.changed_variables(doc, &[VariableKind::Number, VariableKind::Boolean]);
        if changed.is_empty() {
            return Vec::new();
        }
        let candidates: Vec<(NodeId, Vec<String>)> = doc
            .nodes
            .values()
            .filter_map(|n| {
                let vars: Vec<String> = n
                    .variables
                    .bindings
                    .iter()
                    .filter(|(k, v)| {
                        (LAYOUT_BOX_KEYS.contains(&k.as_str())
                            || (STROKE_WEIGHT_KEYS.contains(&k.as_str()) && crate::tools::relayout_scope::stroke_inset_read_by_solver(n).is_some()))
                            && changed.contains(v.as_str())
                    })
                    .map(|(_, v)| v.clone())
                    .collect();
                (!vars.is_empty()).then_some((n.id, vars))
            })
            .collect();
        candidates
            .into_iter()
            .filter(|(id, vars)| vars.iter().any(|v| self.resolve_in_pre_op(doc, *id, v) != doc.resolve_variable(*id, v)))
            .map(|(id, _)| id)
            .collect()
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
    /// its resolved value at the node in the `set_variables` op that `before` was captured for.
    /// `doc` is the document after that op. Colour and boolean variables, and no-op upserts, cost nothing.
    pub(crate) fn remeasure_after(&mut self, doc: &mut Document, before: &mut VariablesBefore) -> Vec<Command> {
        let changed = before.changed_text_variables(doc);
        if changed.is_empty() {
            return Vec::new();
        }
        let ids = self.ids(doc).to_vec();
        let mut cmds = Vec::new();
        for id in ids {
            let Some(node) = doc.get(id) else { continue };
            let bindings: Vec<(String, String)> =
                text_box_bindings(doc, node).into_iter().filter(|(_, var)| changed.contains(*var)).map(|(k, v)| (k.to_string(), v.to_string())).collect();
            if bindings.is_empty() {
                continue;
            }
            let hits: Vec<(String, VariableValue)> = bindings
                .into_iter()
                .filter_map(|(key, var)| {
                    let after = doc.resolve_variable(id, &var)?;
                    let before_val = before.resolve_in_pre_op(doc, id, &var);
                    (before_val.as_ref() != Some(&after)).then_some((key, after))
                })
                .collect();
            if hits.is_empty() {
                continue;
            }
            let Some(node) = doc.get(id) else { continue };
            let mut updated = node.clone();
            let old_style = updated.style_refs.get("text").and_then(|k| doc.styles.get(k));
            for (key, value) in &hits {
                if updated.variables.bindings.contains_key(key.as_str()) {
                    set_bound_property(&mut updated, key, value);
                } else {
                    designos_doc::with_kept_text_style_overrides(&mut updated, old_style, |n| {
                        set_bound_property(n, key, value);
                    });
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
