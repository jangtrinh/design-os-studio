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
use std::collections::BTreeSet;

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

/// Binds property `name` of instance `inst` to `var`, or with `None` unbinds it; see [`bind_prop_variables`].
pub fn bind_prop_variable(doc: &mut Document, inst: NodeId, name: &str, var: Option<&str>) -> Result<Command, PropBindError> {
    bind_prop_variables(doc, inst, &[(name, var)])
}

/// Binds each `(property name, variable)` of instance `inst` (`None` unbinds) and applies the value each variable
/// gives in the instance's modes: one command, one undo step. Unbinding keeps the last value the property had. A
/// variable whose value the property cannot take (a variant name that is no option) is still bound; the property keeps
/// its last valid value and [`prop_binding_problem`] says why. Nothing changes when any entry is refused.
pub fn bind_prop_variables(doc: &mut Document, inst: NodeId, edits: &[(&str, Option<&str>)]) -> Result<Command, PropBindError> {
    let n = doc.get(inst).filter(|n| matches!(n.kind, NodeKind::Instance { .. })).ok_or(PropBindError::NotAnInstance(inst))?;
    // (property name, variable to bind or None, value the variable gives now)
    let mut planned: Vec<(String, Option<String>, Option<PropValue>)> = Vec::with_capacity(edits.len());
    for &(name, var) in edits {
        let def = def_of(doc, n, name).ok_or_else(|| PropBindError::NoSuchProperty(name.to_string()))?;
        let Some(var) = var else {
            planned.push((name.to_string(), None, None));
            continue;
        };
        let expected = bindable_prop_kind(&def.kind).ok_or_else(|| PropBindError::NotBindable(name.to_string()))?;
        let v = doc.variables.get(var).ok_or_else(|| PropBindError::NoSuchVariable(var.to_string()))?;
        if v.kind != expected {
            return Err(PropBindError::KindMismatch { property: name.to_string(), expected, found: v.kind });
        }
        planned.push((name.to_string(), Some(var.to_string()), resolve_for_def(doc, inst, &def, var).ok()));
    }
    Ok(set_instance_prop_edit(doc, inst, |n| {
        for (name, var, value) in planned {
            let key = prop_binding_key(&name);
            match var {
                Some(var) => {
                    n.variables.bindings.insert(key, var);
                    if let Some(v) = value {
                        n.component.values.insert(name, v);
                    }
                }
                None => {
                    n.variables.bindings.remove(&key);
                }
            }
        }
    })
    .0)
}

/// The nodes a command can change a property binding's value for.
#[derive(Debug, Default)]
pub(crate) struct PropResyncScope {
    nodes: BTreeSet<NodeId>,
    subtrees: BTreeSet<NodeId>,
    all: bool,
}

impl PropResyncScope {
    pub(crate) fn is_empty(&self) -> bool {
        !self.all && self.nodes.is_empty() && self.subtrees.is_empty()
    }
}

/// What `cmd` can change for a property binding, read against the document before `cmd` runs: a node whose
/// modes change moves everything below it, a variable edit moves every bound instance, a binding edit moves
/// that instance.
pub(crate) fn prop_resync_scope(doc: &Document, cmd: &Command) -> PropResyncScope {
    fn walk(doc: &Document, cmd: &Command, out: &mut PropResyncScope) {
        match cmd {
            Command::Update { node } => {
                let Some(cur) = doc.get(node.id) else { return };
                if cur.variables.modes != node.variables.modes {
                    out.subtrees.insert(node.id);
                }
                if !prop_bindings_of(cur).eq(prop_bindings_of(node)) {
                    out.nodes.insert(node.id);
                }
            }
            Command::SetVariables { .. } => out.all = true,
            Command::Batch(cmds) => cmds.iter().for_each(|c| walk(doc, c, out)),
            _ => {}
        }
    }
    let mut out = PropResyncScope::default();
    walk(doc, cmd, &mut out);
    out
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
    let stale: Vec<(NodeId, Vec<(String, PropValue)>)> = candidates
        .into_iter()
        .filter_map(|id| {
            let n = doc.get(id)?;
            if !matches!(n.kind, NodeKind::Instance { .. }) || n.variables.bindings.is_empty() {
                return None;
            }
            let changes: Vec<(String, PropValue)> = prop_bindings_of(n)
                .filter_map(|(name, var)| {
                    let def = def_of(doc, n, name)?;
                    let value = resolve_for_def(doc, id, &def, var).ok()?;
                    prop_needs_resync(doc, id, n, &def, &value).then(|| (name.to_string(), value))
                })
                .collect();
            (!changes.is_empty()).then_some((id, changes))
        })
        .collect();
    let mut undo = Vec::new();
    for (id, changes) in stale {
        let (cmd, _) = set_instance_prop_edit(doc, id, |n| {
            for (name, value) in changes {
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
pub fn settle_bound_props(doc: &mut Document) {
    let scope = PropResyncScope { all: true, ..Default::default() };
    let _ = apply_prop_resync(doc, &scope);
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
