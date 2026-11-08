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

/// Bound properties on a node that the auto-layout solver reads directly.
const LAYOUT_BOX_KEYS: [&str; 9] =
    ["width", "height", "visible", "itemSpacing", "counterAxisSpacing", "paddingTop", "paddingRight", "paddingBottom", "paddingLeft"];

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

/// The variables and collections a `set_variables` op writes or deletes, as they were before it ran.
pub(crate) struct VariablesBefore {
    variables: Vec<(String, Option<Variable>)>,
    collections: Vec<(String, Option<VariableCollection>)>,
}

impl VariablesBefore {
    pub(crate) fn capture(doc: &Document, written_variables: &[&str], written_collections: &[&str]) -> Self {
        Self {
            variables: written_variables.iter().map(|id| ((*id).to_string(), doc.variables.get(*id).cloned())).collect(),
            collections: written_collections.iter().map(|id| ((*id).to_string(), doc.variable_collections.get(*id).cloned())).collect(),
        }
    }

    /// Ids of the variables of one of `kinds` whose resolved value can have changed in this op: the
    /// written ones whose value changed, every variable of a collection whose default mode or mode
    /// list changed (they pick which value each variable resolves to), and every variable whose
    /// alias chain reaches one of those.
    fn changed_variables(&self, doc: &Document, kinds: &[VariableKind]) -> HashSet<String> {
        let mut changed: HashSet<String> = HashSet::new();
        for (id, before) in &self.variables {
            let after = doc.variables.get(id);
            if after == before.as_ref() {
                continue;
            }
            if after.or(before.as_ref()).is_some_and(|v| kinds.contains(&v.kind)) {
                changed.insert(id.clone());
            }
        }
        for (id, before) in &self.collections {
            let modes_of = |c: &VariableCollection| (c.default_mode.clone(), c.modes.iter().map(|m| m.id.clone()).collect::<Vec<_>>());
            let after = doc.variable_collections.get(id);
            if after.map(modes_of) == before.as_ref().map(modes_of) {
                continue;
            }
            for var_id in after.into_iter().chain(before.as_ref()).flat_map(|c| c.variables.iter()) {
                if doc.variables.get(var_id).is_some_and(|v| kinds.contains(&v.kind)) {
                    changed.insert(var_id.clone());
                }
            }
            changed.extend(doc.variables.values().filter(|v| v.collection == *id && kinds.contains(&v.kind)).map(|v| v.id.clone()));
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

    fn get_variable<'a>(&'a self, doc: &'a Document, id: &str) -> Option<&'a Variable> {
        match self.variables.iter().find(|(k, _)| k == id) {
            Some((_, before)) => before.as_ref(),
            None => doc.variables.get(id),
        }
    }

    fn get_collection<'a>(&'a self, doc: &'a Document, id: &str) -> Option<&'a VariableCollection> {
        match self.collections.iter().find(|(k, _)| k == id) {
            Some((_, before)) => before.as_ref(),
            None => doc.variable_collections.get(id),
        }
    }

    fn variable_mode_before(&self, doc: &Document, node: NodeId, collection: &str) -> Option<String> {
        let coll = self.get_collection(doc, collection)?;
        if coll.modes.len() <= 1 {
            return Some(coll.default_mode.clone());
        }
        let mut cur = Some(node);
        while let Some(id) = cur {
            let n = doc.get(id)?;
            if let Some(m) = n.variables.modes.get(collection) {
                return Some(m.clone());
            }
            cur = n.parent;
        }
        Some(coll.default_mode.clone())
    }

    /// Resolves `variable` at `node` against the variable and collection state before the op ran.
    pub(crate) fn resolve_before(&self, doc: &Document, node: NodeId, variable: &str) -> Option<VariableValue> {
        let mut id = variable.to_string();
        for _ in 0..16 {
            let v = self.get_variable(doc, &id)?;
            let mode = self.variable_mode_before(doc, node, &v.collection)?;
            let coll = self.get_collection(doc, &v.collection)?;
            let value = v.values.get(&mode).or_else(|| v.values.get(&coll.default_mode))?.clone();
            match value {
                VariableValue::Alias(next) => id = next,
                other => return Some(other),
            }
        }
        None
    }

    /// Variables that can change a text box (numbers and strings).
    fn changed_text_variables(&self, doc: &Document) -> HashSet<String> {
        self.changed_variables(doc, &[VariableKind::Number, VariableKind::String])
    }

    /// Nodes whose layout-affecting variable bindings reference a variable changed in this op
    /// (numbers and booleans) whose resolved value at the node actually changed.
    pub(crate) fn layout_bound_nodes(&self, doc: &Document) -> Vec<NodeId> {
        let changed = self.changed_variables(doc, &[VariableKind::Number, VariableKind::Boolean]);
        if changed.is_empty() {
            return Vec::new();
        }
        doc.nodes
            .values()
            .filter(|n| {
                n.variables.bindings.iter().any(|(k, v)| {
                    (LAYOUT_BOX_KEYS.contains(&k.as_str())
                        || (STROKE_WEIGHT_KEYS.contains(&k.as_str()) && crate::tools::relayout_scope::stroke_inset_read_by_solver(n).is_some()))
                        && changed.contains(v.as_str())
                        && self.resolve_before(doc, n.id, v) != doc.resolve_variable(n.id, v)
                })
            })
            .map(|n| n.id)
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
    pub(crate) fn remeasure_after(&mut self, doc: &Document, before: &VariablesBefore) -> Vec<Command> {
        let changed = before.changed_text_variables(doc);
        if changed.is_empty() {
            return Vec::new();
        }
        let mut cmds = Vec::new();
        for &id in self.ids(doc) {
            let Some(node) = doc.get(id) else { continue };
            let hits: Vec<(&str, VariableValue)> = text_box_bindings(doc, node)
                .into_iter()
                .filter(|(_, var)| changed.contains(*var))
                .filter_map(|(key, var)| {
                    let after = doc.resolve_variable(id, var)?;
                    (before.resolve_before(doc, id, var).as_ref() != Some(&after)).then_some((key, after))
                })
                .collect();
            if hits.is_empty() {
                continue;
            }
            let mut updated = node.clone();
            for (key, value) in &hits {
                set_bound_property(&mut updated, key, value);
            }
            resize_hug_text(doc, &mut updated);
            if &updated != node {
                cmds.push(Command::Update { node: Box::new(updated) });
            }
        }
        cmds
    }
}
