//! Component properties bound to variables.
//!
//! An instance can bind a boolean, text or variant property to a variable. The binding lives on the instance
//! root as `variables.bindings["componentProperties/<property name>"] = <variable id>` (the key Figma's REST
//! `boundVariables` uses for it). The value the variable resolves to in the instance's modes is written into the
//! instance's stored property value, so the layers (or the variant) the property drives follow the mode:
//!
//! * [`bind_prop_variable`] binds or unbinds in one command and applies the resolved value;
//! * [`prop_resync_scope`] / [`apply_prop_resync`] re-apply it after a mode switch, a variable edit or a binding
//!   edit made by any other path. `History` runs them as part of the same undo step.
//!
//! A value the property cannot take (a variant name that is no option, a variable of another type, a missing
//! variable) changes nothing: the last valid value stays and [`prop_binding_problem`] names the reason.

use crate::*;
use std::collections::{BTreeMap, BTreeSet};

/// Prefix of the binding key of a component property on an instance root.
pub const PROP_BINDING_PREFIX: &str = "componentProperties/";

/// Binding key of the property `name`.
pub fn prop_binding_key(name: &str) -> String {
    format!("{PROP_BINDING_PREFIX}{name}")
}

/// Property name of a binding key, or `None` when the key binds an ordinary node field.
pub fn prop_binding_name(key: &str) -> Option<&str> {
    key.strip_prefix(PROP_BINDING_PREFIX)
}

/// The variable bound to property `name` on `inst`.
pub fn bound_prop_variable<'a>(doc: &'a Document, inst: NodeId, name: &str) -> Option<&'a str> {
    doc.get(inst)?.variables.bindings.get(&prop_binding_key(name)).map(String::as_str)
}

/// Variable type a property accepts, or `None` for a property that takes no variable (instance swap, slot).
pub fn bindable_prop_kind(kind: &PropKind) -> Option<VariableKind> {
    match kind {
        PropKind::Boolean => Some(VariableKind::Boolean),
        PropKind::Text | PropKind::Variant { .. } => Some(VariableKind::String),
        PropKind::InstanceSwap { .. } | PropKind::Slot => None,
    }
}

/// Why a bind was refused.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PropBindError {
    /// The node is no instance.
    NotAnInstance(NodeId),
    /// The instance's component defines no property of this name.
    NoSuchProperty(String),
    /// Instance swap and slot properties take no variable.
    NotBindable(String),
    /// The variable does not exist.
    NoSuchVariable(String),
    /// The variable is of another type than the property.
    KindMismatch { property: String, expected: VariableKind, found: VariableKind },
}

impl std::fmt::Display for PropBindError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PropBindError::NotAnInstance(id) => write!(f, "node {} is not an instance", id.0),
            PropBindError::NoSuchProperty(p) => write!(f, "the component has no property \"{p}\""),
            PropBindError::NotBindable(p) => write!(f, "property \"{p}\" takes no variable: only boolean, text and variant properties do"),
            PropBindError::NoSuchVariable(v) => write!(f, "no variable \"{v}\""),
            PropBindError::KindMismatch { property, expected, found } => {
                write!(f, "property \"{property}\" needs a {expected:?} variable, not {found:?}")
            }
        }
    }
}

impl std::error::Error for PropBindError {}

/// Why a bound property keeps its last value instead of following its variable.
#[derive(Clone, Debug, PartialEq)]
pub enum PropBindingProblem {
    /// The instance's component no longer defines the property.
    MissingProperty,
    /// The variable no longer exists.
    MissingVariable(String),
    /// The variable has no value in the instance's modes.
    Unresolved(String),
    /// The variable is of another type than the property.
    KindMismatch { expected: VariableKind, found: VariableKind },
    /// A variant property whose variable gives a value that is no option.
    NotAnOption { value: String, options: Vec<String> },
}

impl std::fmt::Display for PropBindingProblem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PropBindingProblem::MissingProperty => write!(f, "the component no longer has this property"),
            PropBindingProblem::MissingVariable(v) => write!(f, "variable \"{v}\" is gone"),
            PropBindingProblem::Unresolved(v) => write!(f, "variable \"{v}\" has no value in this mode"),
            PropBindingProblem::KindMismatch { expected, found } => write!(f, "needs a {expected:?} variable, the variable is {found:?}"),
            PropBindingProblem::NotAnOption { value, options } => write!(f, "\"{value}\" is not a variant option ({})", options.join(", ")),
        }
    }
}

fn def_of(doc: &Document, inst: &Node, name: &str) -> Option<ComponentPropDef> {
    let comp = instance_component(inst)?;
    prop_defs(doc, comp).into_iter().find(|d| d.name == name)
}

/// The value the variable bound to property `def` gives instance `inst` in its modes now.
fn resolve_for_def(doc: &Document, inst: NodeId, def: &ComponentPropDef, var: &str) -> Result<PropValue, PropBindingProblem> {
    let v = doc.variables.get(var).ok_or_else(|| PropBindingProblem::MissingVariable(var.to_string()))?;
    let expected = bindable_prop_kind(&def.kind).ok_or(PropBindingProblem::MissingProperty)?;
    if v.kind != expected {
        return Err(PropBindingProblem::KindMismatch { expected, found: v.kind });
    }
    match (doc.resolve_variable(inst, var), &def.kind) {
        (Some(VariableValue::Boolean(b)), PropKind::Boolean) => Ok(PropValue::Bool(b)),
        (Some(VariableValue::String(s)), PropKind::Text) => Ok(PropValue::Text(s)),
        (Some(VariableValue::String(s)), PropKind::Variant { options }) => {
            if options.contains(&s) {
                Ok(PropValue::Variant(s))
            } else {
                Err(PropBindingProblem::NotAnOption { value: s, options: options.clone() })
            }
        }
        (Some(other), _) => {
            let found = match other {
                VariableValue::Color(_) => VariableKind::Color,
                VariableValue::Number(_) => VariableKind::Number,
                VariableValue::String(_) | VariableValue::Alias(_) => VariableKind::String,
                VariableValue::Boolean(_) => VariableKind::Boolean,
            };
            Err(PropBindingProblem::KindMismatch { expected, found })
        }
        (None, _) => Err(PropBindingProblem::Unresolved(var.to_string())),
    }
}

/// Why property `name` of `inst` does not follow its variable, or `None` when it has no binding or follows it.
pub fn prop_binding_problem(doc: &Document, inst: NodeId, name: &str) -> Option<PropBindingProblem> {
    let n = doc.get(inst)?;
    let var = n.variables.bindings.get(&prop_binding_key(name))?;
    let Some(def) = def_of(doc, n, name) else { return Some(PropBindingProblem::MissingProperty) };
    resolve_for_def(doc, inst, &def, var).err()
}

/// Whether `n` binds any component property to a variable.
pub fn has_prop_binding(n: &Node) -> bool {
    n.variables.bindings.keys().any(|k| prop_binding_name(k).is_some())
}

/// Every property binding on `inst` as `(property name, variable id)`.
pub fn prop_bindings_of(n: &Node) -> impl Iterator<Item = (&str, &str)> {
    n.variables.bindings.iter().filter_map(|(k, v)| Some((prop_binding_name(k)?, v.as_str())))
}

/// The variant each bound variant property of `n` (the instance node `id`, which sits where its ancestors say) gets from
/// its variable in the modes of that place. A binding that gives no valid option keeps the value `shown` (the main the
/// instance shows now) has for that property, so an unusable value changes nothing, as it does for a top instance.
pub(crate) fn bound_variant_choices(doc: &Document, id: NodeId, n: &Node, shown: NodeId) -> BTreeMap<String, PropValue> {
    let mut out = BTreeMap::new();
    for (name, var) in prop_bindings_of(n) {
        let Some(def) = def_of(doc, n, name).filter(|d| matches!(d.kind, PropKind::Variant { .. })) else { continue };
        match resolve_for_def(doc, id, &def, var) {
            Ok(v) => {
                out.insert(name.to_string(), v);
            }
            Err(_) => {
                let kept = doc.get(shown).and_then(|c| parse_variant_name(&c.name).into_iter().find_map(|(k, v)| (k == name).then_some(v)));
                if let Some(v) = kept {
                    out.insert(name.to_string(), PropValue::Variant(v));
                }
            }
        }
    }
    out
}

/// What a bind or unbind of a property means on the instance it is made on.
#[derive(Clone, Copy, PartialEq, Eq)]
enum CopyBinding {
    /// Not the variant property of a nested copy, or nothing inherited to unbind: the binding is set or removed as it is.
    NotACopyVariant,
    /// A variable bound to the variant of a nested copy: the copy's own binding.
    Own,
    /// An unbind of the binding a nested copy inherits from its main.
    FrozenUnbound,
}

/// Marks (or unmarks) the binding of variant property `name` on nested copy `n` as the copy's own.
fn set_own_variant_binding(n: &mut Node, name: &str, own: bool) {
    if own {
        n.component.own_variant_bindings.insert(name.to_string());
    } else {
        n.component.own_variant_bindings.remove(name);
    }
}

/// Gives the bindings of the variant properties of `n` back to its main, and every variant property a person picked by
/// hand back to its variable: the next sync restores the main's bindings and applies the variables. Returns whether `n`
/// held any of its own bindings or any pick.
pub(crate) fn release_own_variant_bindings(n: &mut Node) -> bool {
    let held = !n.component.own_variant_bindings.is_empty() || !n.component.variant_picks.is_empty();
    n.component.own_variant_bindings.clear();
    n.component.variant_picks.clear();
    held
}

/// Records that a person picked variant property `name` by hand on `n`, a top-level instance, when `name` is bound there.
/// A property that is not bound needs no record: the variant the instance shows already is its pick. A nested copy
/// records its picks through [`record_variant_swap`], as its variant is the main it shows.
pub(crate) fn record_variant_pick(n: &mut Node, name: &str) {
    if is_top_instance(n) && n.variables.bindings.contains_key(&prop_binding_key(name)) {
        n.component.variant_picks.insert(name.to_string());
    }
}

/// What the swap of instance node `n` from main `old` to main `new` (already made) means for the bound variant properties
/// of `n`. Each bound property that has a counterpart in `new` and whose value the swap changed becomes a pick; after a
/// swap to a main of another set, every bound property `new` also has does, as the swap chose the whole main. On a
/// top-level instance, the new value of each such bound variant property is stored too: the instance's variant comes from
/// its stored values, and the next resync would otherwise move it back to the old one. An instance that binds no variant
/// property (none at all, or only text or boolean ones) records nothing here, so its file stays as it was.
pub(crate) fn record_variant_swap(doc: &Document, n: &mut Node, old: Option<NodeId>, new: NodeId) {
    if !has_prop_binding(n) {
        return;
    }
    let pairs = |id: NodeId| doc.get(id).map(|c| parse_variant_name(&c.name)).unwrap_or_default();
    let set_of = |id: NodeId| doc.get(id).and_then(|c| c.parent).filter(|p| doc.get(*p).is_some_and(|p| matches!(p.kind, NodeKind::ComponentSet(_))));
    let same_set = old.is_some_and(|o| set_of(o).is_some() && set_of(o) == set_of(new));
    let old_pairs = old.map(pairs).unwrap_or_default();
    let top = is_top_instance(n);
    for (name, value) in pairs(new) {
        if !n.variables.bindings.contains_key(&prop_binding_key(&name)) {
            continue;
        }
        let changed = old_pairs.iter().find(|(k, _)| *k == name).is_none_or(|(_, v)| *v != value);
        if top && changed {
            n.component.values.insert(name.clone(), PropValue::Variant(value));
        }
        if changed || !same_set {
            n.component.variant_picks.insert(name);
        }
    }
}

/// `n`, a nested copy, with the variable bindings of the variant properties of its reference `r` (the main's nested
/// instance) in place of its own, or `None` when it has those already. A binding the copy holds as its own (see
/// [`set_own_variant_binding`]) stays and the main's for that property is not copied. The bindings of boolean and text
/// properties are left as they are: a person can bind those on the copy itself, and nothing records that as an override.
pub(crate) fn follow_reference_variant_bindings(doc: &Document, n: &Node, r: &Node) -> Option<Node> {
    let own_names = &n.component.own_variant_bindings;
    let is_variant = |name: &str| [n, r].into_iter().any(|x| def_of(doc, x, name).is_some_and(|d| matches!(d.kind, PropKind::Variant { .. })));
    let variant_bindings = |x: &Node| -> BTreeMap<String, String> {
        prop_bindings_of(x)
            .filter(|(name, _)| is_variant(name) && !own_names.contains(*name))
            .map(|(name, var)| (prop_binding_key(name), var.to_string()))
            .collect()
    };
    let (own, want) = (variant_bindings(n), variant_bindings(r));
    if own == want {
        return None;
    }
    let mut out = n.clone();
    for key in own.keys() {
        out.variables.bindings.remove(key);
    }
    out.variables.bindings.extend(want);
    Some(out)
}

/// Binds property `name` of instance `inst` to `var`, or with `None` unbinds it; see [`bind_prop_variables`].
pub fn bind_prop_variable(doc: &mut Document, inst: NodeId, name: &str, var: Option<&str>) -> Result<Command, PropBindError> {
    bind_prop_variables(doc, inst, &[(name, var)])
}

/// Binds each `(property name, variable)` of instance `inst` (`None` unbinds) and applies the value each variable
/// gives in the instance's modes: one command, one undo step. Unbinding keeps the last value the property had. A
/// variable whose value the property cannot take (a variant name that is no option) is still bound; the property keeps
/// its last valid value and [`prop_binding_problem`] says why. Nothing changes when any entry is refused.
pub fn bind_prop_variables(doc: &mut Document, inst: NodeId, edits: &[(&str, Option<&str>)]) -> Result<Command, PropBindError> {
    bind_prop_variables_reporting(doc, inst, edits).map(|(cmd, _)| cmd)
}

/// [`bind_prop_variables`], also why the guard refused the bind when it did: the sentence of
/// [`crate::cycle_refusal_message`] for the variant that the bound values select together. The command is then empty.
pub fn bind_prop_variables_explained(doc: &mut Document, inst: NodeId, edits: &[(&str, Option<&str>)]) -> Result<(Command, Option<String>), PropBindError> {
    let (cmd, refused) = bind_prop_variables_reporting(doc, inst, edits)?;
    let reason = refused.first().map(|swap| crate::refused_swap_message(doc, inst, *swap));
    Ok((cmd, reason))
}

fn bind_prop_variables_reporting(doc: &mut Document, inst: NodeId, edits: &[(&str, Option<&str>)]) -> Result<(Command, Vec<(NodeId, NodeId)>), PropBindError> {
    let n = doc.get(inst).filter(|n| matches!(n.kind, NodeKind::Instance { .. })).ok_or(PropBindError::NotAnInstance(inst))?;
    // (property name, variable to bind or None, value the variable gives now, what binding it on a nested copy means)
    let mut planned: Vec<(String, Option<String>, Option<PropValue>, CopyBinding)> = Vec::with_capacity(edits.len());
    let nested = !is_top_instance(n);
    for &(name, var) in edits {
        let def = def_of(doc, n, name).ok_or_else(|| PropBindError::NoSuchProperty(name.to_string()))?;
        let nested_variant = nested && matches!(def.kind, PropKind::Variant { .. });
        let kind = match (nested_variant, var) {
            (false, _) => CopyBinding::NotACopyVariant,
            (true, Some(_)) => CopyBinding::Own,
            // the main binds this property too: the copy keeps the variant it shows and stops following
            (true, None) if reference_binds_prop(doc, inst, name) => CopyBinding::FrozenUnbound,
            (true, None) => CopyBinding::NotACopyVariant,
        };
        let Some(var) = var else {
            planned.push((name.to_string(), None, None, kind));
            continue;
        };
        let expected = bindable_prop_kind(&def.kind).ok_or_else(|| PropBindError::NotBindable(name.to_string()))?;
        let v = doc.variables.get(var).ok_or_else(|| PropBindError::NoSuchVariable(var.to_string()))?;
        if v.kind != expected {
            return Err(PropBindError::KindMismatch { property: name.to_string(), expected, found: v.kind });
        }
        planned.push((name.to_string(), Some(var.to_string()), resolve_for_def(doc, inst, &def, var).ok(), kind));
    }
    let (cmd, _, refused) = set_instance_prop_edit_reporting(doc, inst, |n| {
        for (name, var, value, kind) in planned {
            let key = prop_binding_key(&name);
            // Binding means "follow from now on", and an unbound property has no pick to hold.
            n.component.variant_picks.remove(&name);
            match var {
                Some(var) => {
                    n.variables.bindings.insert(key, var);
                    // A nested copy shows the variant its own main names, which the sync switches; no value is stored for it.
                    match value {
                        Some(PropValue::Variant(_)) if kind == CopyBinding::Own => {}
                        Some(v) => {
                            n.component.values.insert(name.clone(), v);
                        }
                        None => {}
                    }
                    // A variable bound to the variant of a nested copy is that copy's own: it stays through a sync of the main.
                    set_own_variant_binding(n, &name, kind == CopyBinding::Own);
                }
                None => {
                    n.variables.bindings.remove(&key);
                    // Unbinding what the copy inherits would be put back by the same command's sync, so it is recorded
                    // instead: the copy's own binding is "none", and it is held on the variant it shows like a pick.
                    set_own_variant_binding(n, &name, kind == CopyBinding::FrozenUnbound);
                    if kind == CopyBinding::FrozenUnbound {
                        n.component.overrides.insert(OverrideField::MainComponent);
                    }
                }
            }
        }
    });
    Ok((cmd, refused))
}

/// The nodes a command can change a property binding's value for.
#[derive(Debug, Default)]
pub(crate) struct PropResyncScope {
    nodes: BTreeSet<NodeId>,
    subtrees: BTreeSet<NodeId>,
    switched: BTreeMap<NodeId, (Vec2, Affine, bool)>,
    all: bool,
}

impl PropResyncScope {
    pub(crate) fn is_empty(&self) -> bool {
        !self.all && self.nodes.is_empty() && self.subtrees.is_empty() && self.switched.is_empty()
    }
}

/// What `cmd` can change for a property binding, read against the document before `cmd` runs: a node whose
/// modes change moves everything below it, a node moved or inserted somewhere else moves what it holds, a variable
/// edit moves every bound instance, a binding edit moves that instance.
pub(crate) fn prop_resync_scope(doc: &Document, cmd: &Command) -> PropResyncScope {
    let mut out = PropResyncScope::default();
    collect_prop_resync_scope(doc, cmd, &mut out);
    out
}

fn collect_prop_resync_scope(doc: &Document, cmd: &Command, out: &mut PropResyncScope) {
    match cmd {
        Command::Update { node } => {
            let Some(cur) = doc.get(node.id) else { return };
            if cur.variables.modes != node.variables.modes {
                out.subtrees.insert(node.id);
            }
            if !prop_bindings_of(cur).eq(prop_bindings_of(node)) || cur.component.variant_picks != node.component.variant_picks {
                out.nodes.insert(node.id);
            }
            if matches!(cur.kind, NodeKind::Instance { .. })
                && (instance_component(cur) != instance_component(node) || cur.component.variant_picks != node.component.variant_picks)
                && (cur.size != node.size || cur.transform != node.transform || cur.visible != node.visible)
            {
                out.switched.entry(node.id).or_insert((cur.size, cur.transform, cur.visible));
            }
        }
        Command::SetVariables { .. } => out.all = true,
        // A node that arrives under another parent (a move, a paste) can sit in other modes than it did; a reorder
        // among its siblings (arrow-key nudge in a stack) keeps its place in the mode tree.
        Command::Reparent { id, parent, .. } => {
            if doc.get(*id).and_then(|n| n.parent) != Some(*parent) {
                out.subtrees.insert(*id);
            }
        }
        Command::Insert { nodes, .. } => out.nodes.extend(nodes.iter().filter(|n| has_prop_binding(n)).map(|n| n.id)),
        Command::Batch(cmds) => cmds.iter().for_each(|c| collect_prop_resync_scope(doc, c, out)),
        _ => {}
    }
}

/// Re-applies the bound component properties that `cmds` moved to `after`, a copy of `before` that `cmds` were applied to
/// one at a time with [`Command::apply`]. A history step does this after it applies a command; a trial copy that runs the
/// same commands without a history calls this once at the end, so it holds what the committed document will.
pub fn settle_bound_props_after(before: &Document, after: &mut Document, cmds: &[Command]) {
    let mut scope = PropResyncScope::default();
    cmds.iter().for_each(|c| collect_prop_resync_scope(before, c, &mut scope));
    if !scope.is_empty() {
        let _ = apply_prop_resync_and_relayout(after, &scope);
    }
}

/// [`apply_prop_resync`], then solves the auto layout the swapped instances feed. A variant switch changes an
/// instance's size after the layout pass of the command that moved the variable, so the hugging frames around it would
/// keep the old size for one more edit. The parent of each changed node is one seed; a seed is solved once however
/// many instances sit under it, and the climb goes up only while a frame's own size changes. Frames off that chain
/// keep their positions. A hug ancestor that several separate chains share is solved once per chain: that is how
/// `relayout_seeds_upward` climbs today. For a variant switch a history step and the trial copy of a batch run the
/// same resync and the same layout.
pub(crate) fn apply_prop_resync_and_relayout(doc: &mut Document, scope: &PropResyncScope) -> Result<Option<Command>, DocError> {
    let resync_inv = apply_prop_resync(doc, scope)?;
    // Variant resync or an explicit variant/component switch changes an instance size below plain Groups.
    // Refit only these ancestors on this explicit switch path; generic command replay preserves imported group bounds.
    let mut changed = Vec::new();
    let mut seeds = Vec::new();
    if let Some(inv) = &resync_inv {
        collect_update_ids(doc, inv, &mut changed);
        collect_relayout_seeds(doc, inv, &mut seeds);
    }
    for (&id, &(old_size, old_tf, old_vis)) in &scope.switched {
        if let Some(cur) = doc.get(id) {
            if cur.size != old_size || cur.transform != old_tf || cur.visible != old_vis {
                if !changed.contains(&id) {
                    changed.push(id);
                }
                if has_auto_layout(doc, id) {
                    seeds.push(id);
                }
                if let Some(p) = cur.parent {
                    seeds.push(p);
                }
            }
        }
    }
    if resync_inv.is_none() && changed.is_empty() {
        return Ok(None);
    }
    let old_nodes = climb_groups_and_relayout(doc, &changed, &seeds);
    let group_inv = (!old_nodes.is_empty()).then(|| Command::Batch(old_nodes.into_values().map(|n| Command::Update { node: Box::new(n) }).collect()));
    Ok(match (group_inv, resync_inv) {
        (None, None) => None,
        (Some(g), None) => Some(g),
        (None, Some(r)) => Some(r),
        (Some(g), Some(r)) => Some(Command::Batch(vec![g, r])),
    })
}

fn has_auto_layout(doc: &Document, id: NodeId) -> bool {
    doc.get(id).and_then(|n| n.kind.frame_props()).is_some_and(|f| f.auto_layout.is_some())
}

/// Walks the ancestors of `changed` from the inside out (deepest first), one level at a time:
/// each `NodeKind::Group` is fitted to its children and each auto-layout frame is solved, so
/// alternating groups and auto-layout frames each read the updated size of the level below.
/// Stops on any branch where a container's bounds did not change.
fn climb_groups_and_relayout(doc: &mut Document, changed: &[NodeId], initial_seeds: &[NodeId]) -> BTreeMap<NodeId, Node> {
    let mut groups = BTreeSet::new();
    for &id in changed {
        let mut cur = doc.get(id).and_then(|n| n.parent);
        while let Some(parent) = cur {
            if doc.get(parent).is_some_and(|n| matches!(n.kind, NodeKind::Group)) {
                groups.insert(parent);
            }
            cur = doc.get(parent).and_then(|n| n.parent);
        }
    }
    if groups.is_empty() {
        doc.relayout_seeds_upward(initial_seeds);
        return BTreeMap::new();
    }
    let mut before_group_fit = BTreeMap::new();
    for &gid in &groups {
        if let Some(g) = doc.get(gid).cloned() {
            for &cid in &g.children {
                if let Some(c) = doc.get(cid).cloned() {
                    before_group_fit.entry(cid).or_insert(c);
                }
            }
            before_group_fit.entry(gid).or_insert(g);
        }
    }
    let mut worklist: BTreeSet<(usize, NodeId)> = BTreeSet::new();
    for &s in initial_seeds {
        worklist.insert((group_depth(doc, s), s));
    }
    for &id in changed {
        if let Some(p) = doc.get(id).and_then(|n| n.parent) {
            worklist.insert((group_depth(doc, p), p));
        }
    }
    let mut old_nodes = BTreeMap::new();
    let mut final_al_seeds = Vec::new();
    while let Some((_depth, cur)) = worklist.pop_last() {
        if doc.get(cur).is_some_and(|n| matches!(n.kind, NodeKind::Group)) {
            let before_rect = doc.get(cur).map(|n| (n.size, n.transform));
            let _ = apply_resize_to_fit_one(doc, cur, |d, touched| {
                if let Some(n) = before_group_fit.get(&touched).cloned().or_else(|| d.get(touched).cloned()) {
                    old_nodes.entry(touched).or_insert(n);
                }
            });
            let moved = doc.get(cur).map(|n| (n.size, n.transform)) != before_rect;
            if moved {
                if let Some(p) = doc.get(cur).and_then(|n| n.parent) {
                    worklist.insert((group_depth(doc, p), p));
                }
            }
        } else if has_auto_layout(doc, cur) {
            let mut top_al = cur;
            while let Some(p) = doc.get(top_al).and_then(|n| n.parent) {
                if has_auto_layout(doc, p) {
                    top_al = p;
                } else {
                    break;
                }
            }
            let non_al_parent = doc.get(top_al).and_then(|n| n.parent);
            if let Some(enclosing_group) = non_al_parent.filter(|&p| doc.get(p).is_some_and(|n| matches!(n.kind, NodeKind::Group))) {
                let before_top = doc.get(top_al).map(|n| (n.size, n.transform));
                doc.relayout_seeds_upward(&[cur]);
                let moved = doc.get(top_al).map(|n| (n.size, n.transform)) != before_top;
                if moved {
                    worklist.insert((group_depth(doc, enclosing_group), enclosing_group));
                }
            } else {
                final_al_seeds.push(cur);
            }
        }
    }
    if !final_al_seeds.is_empty() {
        doc.relayout_seeds_upward(&final_al_seeds);
    }
    old_nodes.retain(|id, old| doc.get(*id) != Some(old));
    old_nodes
}

fn collect_update_ids(doc: &Document, cmd: &Command, out: &mut Vec<NodeId>) {
    match cmd {
        Command::Update { node } => {
            if doc.get(node.id).is_some_and(|cur| cur.size != node.size || cur.transform != node.transform || cur.visible != node.visible) {
                out.push(node.id);
            }
        }
        Command::Batch(cs) => cs.iter().for_each(|c| collect_update_ids(doc, c, out)),
        _ => {}
    }
}
fn group_depth(doc: &Document, id: NodeId) -> usize {
    let mut d = 0;
    let mut cur = doc.get(id).and_then(|n| n.parent);
    while let Some(p) = cur {
        d += 1;
        cur = doc.get(p).and_then(|n| n.parent);
    }
    d
}

/// Whether `def` is a variant property of instance `n`, top-level or nested, that a person picked by hand (see
/// [`ComponentLink::variant_picks`]), so its binding leaves it alone.
fn variant_is_picked(n: &Node, def: &ComponentPropDef) -> bool {
    matches!(def.kind, PropKind::Variant { .. }) && n.component.variant_picks.contains(&def.name)
}

fn prop_needs_resync(doc: &Document, id: NodeId, n: &Node, def: &ComponentPropDef, value: &PropValue) -> bool {
    if prop_value(doc, id, def) != *value {
        return true;
    }
    if let PropValue::Variant(want) = value {
        let shown = instance_component(n)
            .and_then(|c| doc.get(c))
            .and_then(|c| parse_variant_name(&c.name).into_iter().find_map(|(k, v)| (k == def.name).then_some(v)));
        if shown.as_deref() != Some(want.as_str()) {
            return true;
        }
    }
    false
}

/// Re-applies the bound values of every instance in `scope` whose stored value differs from what its variable gives
/// now, and returns the command that undoes it (`None` when nothing changed). A binding that gives no valid value
/// leaves its property alone.
pub(crate) fn apply_prop_resync(doc: &mut Document, scope: &PropResyncScope) -> Result<Option<Command>, DocError> {
    let mut candidates: BTreeSet<NodeId> = scope.nodes.clone();
    for &root in &scope.subtrees {
        doc.walk(root, |n, _| {
            if has_prop_binding(n) {
                candidates.insert(n.id);
            }
        });
    }
    if scope.all {
        candidates.extend(doc.nodes.values().filter(|n| has_prop_binding(n)).map(|n| n.id));
    }
    let mut stale: Vec<(NodeId, Vec<(String, PropValue)>)> = candidates
        .into_iter()
        .filter_map(|id| {
            let n = doc.get(id)?;
            if !matches!(n.kind, NodeKind::Instance { .. }) || n.variables.bindings.is_empty() {
                return None;
            }
            let changes: Vec<(String, PropValue)> = prop_bindings_of(n)
                .filter_map(|(name, var)| {
                    let def = def_of(doc, n, name)?;
                    if variant_is_picked(n, &def) {
                        return None;
                    }
                    let value = resolve_for_def(doc, id, &def, var).ok()?;
                    prop_needs_resync(doc, id, n, &def, &value).then(|| (name.to_string(), value))
                })
                .collect();
            (!changes.is_empty()).then_some((id, changes))
        })
        .collect();
    // A copy follows its main-side instance link by link, so the copies switch first: a main-side instance that
    // switched before them would leave their layers pointing at layers it replaced, and the overrides on those would
    // not carry over.
    stale.sort_by_key(|(id, _)| doc.get(*id).is_some_and(is_top_instance));
    let mut undo = Vec::new();
    for (id, changes) in stale {
        // A nested instance shows the variant its own main names, so its variant is switched by the sync below (the
        // same swap a person's pick makes) and no value is stored for it; its other values are stored as on a top one.
        let nested = doc.get(id).is_some_and(|n| !is_top_instance(n));
        let (cmd, _) = set_instance_prop_edit(doc, id, |n| {
            for (name, value) in changes {
                if nested && matches!(value, PropValue::Variant(_)) {
                    continue;
                }
                n.component.values.insert(name, value);
            }
        });
        if matches!(&cmd, Command::Batch(v) if v.is_empty()) {
            continue;
        }
        match cmd.apply_atomic(doc) {
            Ok(inv) => undo.push(inv),
            Err(e) => {
                for inv in undo.into_iter().rev() {
                    let _ = inv.apply(doc);
                }
                return Err(e);
            }
        }
    }
    undo.reverse();
    Ok(if undo.is_empty() { None } else { Some(Command::Batch(undo)) })
}

/// Applies the values of every bound property to its instance, with no undo step: for a document that was just read
/// (an imported file whose assignment holds only the variable's alias), so its layers show what the variables give.
///
/// A `.fig` file stores the variant a bound nested instance shows as a swap of that nested instance, which the import
/// records as a pick by hand. Where the variable gives the variant the file shows, the swap is the binding's doing and not
/// a pick: it is released here, so the reopened instance goes on following its variable. Where it is not, the bound
/// properties the file shows at another value than their variable gives are the picks. A file Studio wrote says its picks
/// itself (a carrier entry) and is left as it says.
pub fn settle_bound_props(doc: &mut Document) {
    release_bound_variants_the_file_stored(doc);
    let scope = PropResyncScope { all: true, ..Default::default() };
    let _ = apply_prop_resync(doc, &scope);
}

fn release_bound_variants_the_file_stored(doc: &mut Document) {
    let mut released: Vec<NodeId> = Vec::new();
    let mut picked: Vec<(NodeId, BTreeSet<String>)> = Vec::new();
    for n in doc.nodes.values().filter(|n| {
        matches!(n.kind, NodeKind::Instance { .. })
            && !is_top_instance(n)
            && n.component.overrides.contains(&OverrideField::MainComponent)
            && n.component.variant_picks.is_empty()
            && has_prop_binding(n)
    }) {
        let Some(shown) = instance_component(n) else { continue };
        let choices = bound_variant_choices(doc, n.id, n, shown);
        if choices.is_empty() {
            continue;
        }
        let target = bound_target_of_copy(doc, n.id, &choices);
        if target == Some(shown) {
            released.push(n.id);
            continue;
        }
        let set_of = |id: NodeId| doc.get(id).and_then(|c| c.parent);
        let swapped_to_another_set = target.is_some_and(|t| set_of(t) != set_of(shown));
        let shown_pairs = doc.get(shown).map(|c| parse_variant_name(&c.name)).unwrap_or_default();
        let held: BTreeSet<String> = choices
            .iter()
            .filter(|(name, want)| {
                let value = shown_pairs.iter().find(|(k, _)| k == *name).map(|(_, v)| v);
                match (value, want) {
                    (Some(v), PropValue::Variant(w)) => swapped_to_another_set || v != w,
                    _ => false,
                }
            })
            .map(|(name, _)| name.clone())
            .collect();
        if !held.is_empty() {
            picked.push((n.id, held));
        }
    }
    for id in released {
        if let Some(n) = doc.get_mut(id) {
            n.component.overrides.remove(&OverrideField::MainComponent);
        }
    }
    for (id, held) in picked {
        if let Some(n) = doc.get_mut(id) {
            n.component.variant_picks = held;
        }
    }
}

/// Property bindings whose value the property does not take: one `(instance, property, problem)` per binding.
pub fn prop_binding_problems(doc: &Document) -> Vec<(NodeId, String, PropBindingProblem)> {
    let mut out = Vec::new();
    for n in doc.nodes.values() {
        if !matches!(n.kind, NodeKind::Instance { .. }) {
            continue;
        }
        for (name, _) in prop_bindings_of(n) {
            if let Some(p) = prop_binding_problem(doc, n.id, name) {
                out.push((n.id, name.to_string(), p));
            }
        }
    }
    out
}
