//! Extended variable collections: a collection that inherits every variable of a parent collection
//! (in this file or in a library) and overrides single values per mode.
//!
//! The link lives in the collection's `extensions` under [`EXTENSION_KEY`], so undo, branching, publishing and the
//! document file format carry it with no new field. Readers on the resolution path look into that JSON without
//! building the typed [`CollectionExtension`].

use crate::*;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

/// Collection extension key holding a serialized [`CollectionExtension`].
pub const EXTENSION_KEY: &str = "org.designos.extendedCollection";
/// Collection extension key (`true`) on a collection other collections may extend (Figma's `isCollectionExtendable`).
pub const EXTENDABLE_KEY: &str = "org.designos.extendable";
/// Longest chain of extensions followed (an extension of an extension counts one more).
const MAX_EXTENSION_DEPTH: usize = 8;

/// The collection an extended collection inherits from.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ParentCollection {
    /// Id of the parent collection in this file. Empty when the parent is a library collection this file does not
    /// hold and only its key is known.
    pub collection: String,
    /// Figma key of the parent collection when it comes from a library, so the link survives a file that lacks it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub set_key: Option<String>,
}

/// What an extended collection adds on top of its parent: a mode mapping and the overridden values.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct CollectionExtension {
    pub parent: ParentCollection,
    /// Own mode id -> the parent mode it shows when the variable is not overridden.
    #[serde(default)]
    pub mode_map: BTreeMap<String, String>,
    /// Variable id -> own mode id -> overriding value.
    #[serde(default)]
    pub overrides: BTreeMap<String, BTreeMap<String, VariableValue>>,
}

impl CollectionExtension {
    /// True when `variable` has an override in `mode`.
    pub fn is_overridden(&self, variable: &str, mode: &str) -> bool {
        self.overrides.get(variable).is_some_and(|m| m.contains_key(mode))
    }
}

impl VariableCollection {
    /// True for a collection that extends another one.
    pub fn is_extension(&self) -> bool {
        self.extensions.contains_key(EXTENSION_KEY)
    }

    /// The typed extension, when this collection extends another and the stored value is well formed.
    pub fn extension(&self) -> Option<CollectionExtension> {
        serde_json::from_value(self.extensions.get(EXTENSION_KEY)?.clone()).ok()
    }

    /// Makes this collection an extension described by `ext`.
    pub fn set_extension(&mut self, ext: &CollectionExtension) {
        if let Ok(v) = serde_json::to_value(ext) {
            self.extensions.insert(EXTENSION_KEY.to_string(), v);
        }
    }

    /// True when other collections may extend this one.
    pub fn is_extendable(&self) -> bool {
        self.extensions.get(EXTENDABLE_KEY).and_then(Value::as_bool) == Some(true)
    }

    pub fn set_extendable(&mut self, extendable: bool) {
        if extendable {
            self.extensions.insert(EXTENDABLE_KEY.to_string(), Value::Bool(true));
        } else {
            self.extensions.remove(EXTENDABLE_KEY);
        }
    }

    /// Id of the parent collection, read straight from the stored JSON.
    pub fn extension_parent_id(&self) -> Option<&str> {
        self.extensions.get(EXTENSION_KEY)?.get("parent")?.get("collection")?.as_str().filter(|s| !s.is_empty())
    }

    /// Figma library key of the parent collection (`parent.set_key`), read straight from the stored JSON.
    pub fn extension_parent_key(&self) -> Option<&str> {
        self.extensions.get(EXTENSION_KEY)?.get("parent")?.get("set_key")?.as_str().filter(|s| !s.is_empty())
    }

    /// Id of the parent collection in `collections`: `parent.collection` when present in `collections`, else the
    /// collection whose library `key` matches `parent.set_key`.
    pub fn extension_parent_id_in<'a>(&'a self, collections: &'a BTreeMap<String, VariableCollection>) -> Option<&'a str> {
        if let Some(id) = self.extension_parent_id().filter(|id| collections.contains_key(*id)) {
            return Some(id);
        }
        if let Some(key) = self.extension_parent_key() {
            if let Some(c) = collections.values().find(|c| c.key.as_deref() == Some(key)) {
                return Some(c.id.as_str());
            }
        }
        self.extension_parent_id()
    }

    /// The parent mode shown for own `mode` when nothing overrides it.
    fn extension_parent_mode(&self, mode: &str) -> Option<&str> {
        self.extensions.get(EXTENSION_KEY)?.get("mode_map")?.get(mode)?.as_str()
    }

    /// The value this extension overrides `variable` with in `mode`.
    pub fn extension_override(&self, variable: &str, mode: &str) -> Option<VariableValue> {
        let v = self.extensions.get(EXTENSION_KEY)?.get("overrides")?.get(variable)?.get(mode)?;
        VariableValue::deserialize(v).ok()
    }
}

/// The value `variable` shows in `mode` of the extended collection `coll`: its override when set, else what the
/// parent collection shows in the mapped mode. `None` when `coll` does not extend anything or the parent chain breaks.
pub fn extension_value(
    collections: &BTreeMap<String, VariableCollection>,
    variables: &BTreeMap<String, Variable>,
    coll: &str,
    variable: &str,
    mode: &str,
) -> Option<VariableValue> {
    let mut coll = coll;
    let mut mode = mode.to_string();
    for _ in 0..MAX_EXTENSION_DEPTH {
        let c = collections.get(coll)?;
        if !c.is_extension() {
            let owner = collections.get(&variables.get(variable)?.collection)?;
            let v = variables.get(variable)?;
            return v.values.get(&mode).or_else(|| v.values.get(&owner.default_mode)).cloned();
        }
        if let Some(own) = c.extension_override(variable, &mode) {
            return Some(own);
        }
        let parent_id = c.extension_parent_id_in(collections)?;
        let parent = collections.get(parent_id)?;
        mode = match c.extension_parent_mode(&mode) {
            Some(m) if parent.modes.iter().any(|x| x.id == m) => m.to_string(),
            _ => parent.default_mode.clone(),
        };
        coll = parent_id;
    }
    None
}

/// True when `ancestor` is `coll` or one of the collections `coll` extends, directly or through a chain.
fn extends(collections: &BTreeMap<String, VariableCollection>, coll: &str, ancestor: &str) -> bool {
    let mut cur = coll;
    for _ in 0..MAX_EXTENSION_DEPTH {
        if cur == ancestor {
            return true;
        }
        match collections.get(cur).and_then(|c| c.extension_parent_id_in(collections)) {
            Some(p) => cur = p,
            None => return false,
        }
    }
    false
}

impl Document {
    /// Marks every collection that is extended as extendable. A file whose extension has a parent without the flag
    /// (an import, an older build) is normalised on open, so the canvas, which skips the extension lookup for a
    /// collection that is not extendable, resolves what the panel resolves.
    pub fn normalize_extensions(&mut self) {
        let parents: BTreeSet<String> =
            self.variable_collections.values().filter_map(|c| c.extension_parent_id_in(&self.variable_collections).map(String::from)).collect();
        for id in parents {
            if let Some(parent) = self.variable_collections.get_mut(&id) {
                if !parent.is_extendable() {
                    parent.set_extendable(true);
                }
            }
        }
    }

    /// Collections that extend `parent`, directly or through a chain, nearest extensions first.
    pub fn extensions_of(&self, parent: &str) -> Vec<&VariableCollection> {
        self.variable_collections.values().filter(|c| c.id != parent && c.is_extension() && extends(&self.variable_collections, &c.id, parent)).collect()
    }

    /// The value `variable` shows in `mode` of the extended collection `coll` (see [`extension_value`]).
    pub fn extended_value(&self, coll: &str, variable: &str, mode: &str) -> Option<VariableValue> {
        extension_value(&self.variable_collections, &self.variables, coll, variable, mode)
    }

    /// Variable ids an extended collection inherits: every live variable of the collection chain it extends.
    pub fn inherited_variable_ids(&self, coll: &str) -> Vec<String> {
        inherited_ids(&self.variable_collections, &self.variables, coll)
    }

    /// The value `v` takes at `node` when the nearest mode choice among its collection and the collections extending
    /// it is an extended collection's. `None` leaves the plain lookup in charge.
    pub(crate) fn node_extension_value(&self, node: NodeId, v: &Variable, mode_overrides: &[(&str, &str)]) -> Option<VariableValue> {
        // An extension needs a parent that allows it (`extend_collection` and the importers set the flag), so one lookup
        // of the variable's own collection, which every resolve makes anyway, rules out a document without extensions.
        if !self.variable_collections.get(&v.collection).is_some_and(VariableCollection::is_extendable) {
            return None;
        }
        #[cfg(test)]
        SCANS.with(|c| c.set(c.get() + 1));
        // extensions first, so a node that picks both the parent and an extension follows the extension
        let mut candidates: Vec<&str> = self
            .variable_collections
            .values()
            .filter(|c| c.id != v.collection && c.is_extension() && extends(&self.variable_collections, &c.id, &v.collection))
            .map(|c| c.id.as_str())
            .collect();
        if candidates.is_empty() {
            return None;
        }
        candidates.push(v.collection.as_str());

        let mut choice: Option<(&str, String)> = None;
        for c in &candidates {
            if let Some(&(_, m)) = mode_overrides.iter().find(|(oc, _)| oc == c) {
                choice = Some((c, m.to_string()));
                break;
            }
        }
        let mut cur = Some(node);
        while choice.is_none() {
            let Some(id) = cur else { break };
            let n = self.get(id)?;
            choice = candidates.iter().find_map(|c| n.variables.modes.get(*c).map(|m| (*c, m.clone())));
            cur = n.parent;
        }
        let (coll, mode) = choice?;
        if coll == v.collection {
            return None;
        }
        extension_value(&self.variable_collections, &self.variables, coll, &v.id, &mode)
    }
}

fn inherited_ids(collections: &BTreeMap<String, VariableCollection>, variables: &BTreeMap<String, Variable>, coll: &str) -> Vec<String> {
    let mut cur = coll;
    for _ in 0..MAX_EXTENSION_DEPTH {
        let Some(c) = collections.get(cur) else { return Vec::new() };
        let Some(parent) = c.extension_parent_id_in(collections) else {
            return c.variables.iter().filter(|id| variables.get(*id).is_some_and(|v| !v.is_soft_deleted())).cloned().collect();
        };
        cur = parent;
    }
    Vec::new()
}

#[cfg(test)]
thread_local! {
    static SCANS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}
#[cfg(test)]
pub(crate) fn reset_extension_scan_count() {
    SCANS.with(|c| c.set(0));
}
#[cfg(test)]
pub(crate) fn extension_scan_count() -> usize {
    SCANS.with(std::cell::Cell::get)
}

#[cfg(test)]
thread_local! {
    static ALIAS_CHECKS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}
#[cfg(test)]
pub(crate) fn reset_alias_check_count() {
    ALIAS_CHECKS.with(|c| c.set(0));
}
#[cfg(test)]
pub(crate) fn alias_check_count() -> usize {
    ALIAS_CHECKS.with(std::cell::Cell::get)
}

impl VariableStore {
    /// Applies a batch of value writes (variable, mode, value; `None` resets an override) and checks once, after the
    /// last one, that no alias left a variable without a value. The aliases of a batch may come before the values they
    /// point to; a cycle closed anywhere in the batch is refused. The batch commits only when every write and the final
    /// resolution check succeeds. Only variables and extension collections touched by the batch are snapshotted.
    /// Per-write checks are quadratic in the batch size.
    pub fn apply_value_writes(&mut self, writes: Vec<(String, String, Option<VariableValue>)>) -> Result<(), String> {
        let before = self.unresolved_pairs();
        let mut variables_before = BTreeMap::new();
        let mut collections_before = BTreeMap::new();
        for (variable, mode, value) in writes {
            variables_before.entry(variable.clone()).or_insert_with(|| self.variables.get(&variable).cloned());
            if let Some(ext) = self.extension_owning_mode(&mode, &variable) {
                collections_before.entry(ext.clone()).or_insert_with(|| self.collections.get(&ext).cloned());
            }
            let ok = match &value {
                Some(VariableValue::Alias(target)) => self.set_alias_with(&variable, &mode, target, false),
                Some(literal) => self.set_value(&variable, &mode, literal.clone()),
                None => match self.extension_owning_mode(&mode, &variable) {
                    Some(ext) => {
                        self.reset_override(&ext, &variable, &mode);
                        true
                    }
                    None => false,
                },
            };
            if !ok {
                let why = if value.is_none() {
                    "null resets an override and is only valid in a mode of an extended collection that extends the variable's collection"
                } else {
                    "refused: a library variable, a value of the wrong kind, an alias to a missing variable or a variable of another kind, or a mode of an extended collection that does not extend the variable's collection"
                };
                self.restore_value_write_batch(variables_before, collections_before);
                return Err(format!("value for variable '{variable}' in mode '{mode}' {why}"));
            }
        }
        if self.unresolved_pairs().is_subset(&before) {
            Ok(())
        } else {
            self.restore_value_write_batch(variables_before, collections_before);
            Err("an alias in this batch would leave a variable without a value (a cycle, or an alias to a variable with no value)".into())
        }
    }

    fn restore_value_write_batch(&mut self, variables: BTreeMap<String, Option<Variable>>, collections: BTreeMap<String, Option<VariableCollection>>) {
        for (id, variable) in variables {
            match variable {
                Some(variable) => {
                    self.variables.insert(id, variable);
                }
                None => {
                    self.variables.remove(&id);
                }
            }
        }
        for (id, collection) in collections {
            match collection {
                Some(collection) => {
                    self.collections.insert(id, collection);
                }
                None => {
                    self.collections.remove(&id);
                }
            }
        }
    }

    /// The model's rules for extended collections, as the set of rules a store breaks. An extension holds no
    /// variables of its own, no variable names an extension as its collection, and a value is never keyed by a mode of
    /// an extension the variable is not part of. An import keeps what Figma stored, so a store can already break one;
    /// an edit is judged by what it adds (see [`VariableStore::check_extension_rules`]), never by what was there.
    pub fn extension_violations(&self) -> BTreeSet<String> {
        let mut out = BTreeSet::new();
        for c in self.collections.values().filter(|c| c.is_extension()) {
            for variable in &c.variables {
                out.insert(format!("collection '{}' extends another collection and lists variable '{variable}' of its own", c.id));
            }
        }
        for v in self.variables.values() {
            if self.collections.get(&v.collection).is_some_and(VariableCollection::is_extension) {
                out.insert(format!("variable '{}' belongs to collection '{}', which extends another collection", v.id, v.collection));
            }
            let own = self.collections.get(&v.collection);
            for mode in v.values.keys() {
                if own.is_some_and(|c| c.modes.iter().any(|m| &m.id == mode)) {
                    continue;
                }
                if let Some(ext) = self.collections.values().find(|c| c.is_extension() && c.modes.iter().any(|m| &m.id == mode)) {
                    if !extends(&self.collections, &ext.id, &v.collection) {
                        out.insert(format!(
                            "variable '{}' has a value for mode '{mode}', which belongs to extended collection '{}' that does not extend '{}'",
                            v.id, ext.id, v.collection
                        ));
                    }
                }
            }
        }
        out
    }

    /// `Err` with the rules this store breaks that `before` did not. A store that already broke a rule on import is
    /// not locked: only an edit that adds a break is refused.
    pub fn check_extension_rules(&self) -> Result<(), String> {
        self.rules_added_since(&BTreeSet::new())
    }

    /// [`VariableStore::check_extension_rules`] relative to the violations recorded before an edit.
    pub fn rules_added_since(&self, before: &BTreeSet<String>) -> Result<(), String> {
        let added: Vec<String> = self.extension_violations().difference(before).cloned().collect();
        if added.is_empty() {
            Ok(())
        } else {
            Err(added.join("; "))
        }
    }

    /// True when `mode` is a mode of the collection that holds `variable`.
    pub(crate) fn is_own_mode(&self, variable: &str, mode: &str) -> bool {
        self.variables.get(variable).and_then(|v| self.collections.get(&v.collection)).is_some_and(|c| c.modes.iter().any(|m| m.id == mode))
    }

    /// True when `mode` belongs to some extended collection (whichever it extends).
    pub(crate) fn is_extension_mode(&self, mode: &str) -> bool {
        self.collections.values().any(|c| c.is_extension() && c.modes.iter().any(|m| m.id == mode))
    }

    /// Every mode `variable` shows a value in: its collection's modes and the modes of the collections extending it.
    fn modes_shown_for(&self, variable: &str) -> Vec<String> {
        let Some(v) = self.variables.get(variable) else { return Vec::new() };
        let mut modes: Vec<String> = self.collections.get(&v.collection).map(|c| c.modes.iter().map(|m| m.id.clone()).collect()).unwrap_or_default();
        for c in self.collections.values().filter(|c| c.id != v.collection && c.is_extension() && extends(&self.collections, &c.id, &v.collection)) {
            modes.extend(c.modes.iter().map(|m| m.id.clone()));
        }
        modes
    }

    /// The (variable, mode) pairs whose stored value is an alias that resolves to nothing: a cycle, or a target without
    /// a value. A variable that has no value at all in a mode is not an alias problem and is not listed.
    pub(crate) fn unresolved_pairs(&self) -> BTreeSet<(String, String)> {
        #[cfg(test)]
        ALIAS_CHECKS.with(|c| c.set(c.get() + 1));
        let mut out = BTreeSet::new();
        for (id, v) in &self.variables {
            for mode in self.modes_shown_for(id) {
                let own = if self.is_own_mode(id, &mode) { v.values.get(&mode).cloned() } else { self.extended_value_of(&mode, id) };
                if matches!(own, Some(VariableValue::Alias(_))) && self.resolve(id, &mode).is_none() {
                    out.insert((id.clone(), mode));
                }
            }
        }
        out
    }

    fn extended_value_of(&self, mode: &str, variable: &str) -> Option<VariableValue> {
        let ext = self.extension_owning_mode(mode, variable)?;
        self.extended_value(&ext, variable, mode)
    }

    /// Runs an alias edit and undoes it when it leaves any variable without a value in any mode it is shown in, in the
    /// collection itself or in an extension. The one check behind every alias: a parent alias, an override alias, from the
    /// panel, MCP or a script.
    pub(crate) fn guard_alias(&mut self, edit: impl FnOnce(&mut Self) -> bool) -> bool {
        let before = self.unresolved_pairs();
        let backup = (self.variables.clone(), self.collections.clone());
        if !edit(self) {
            return false;
        }
        if self.unresolved_pairs().is_subset(&before) {
            return true;
        }
        (self.variables, self.collections) = backup;
        false
    }

    /// Creates a collection that extends `parent`, with one mode for each of the parent's modes. A collection of the
    /// file's own becomes extendable; a library collection must already allow it. `None` when `parent` is missing,
    /// soft-deleted, or a library collection that is not extendable.
    pub fn extend_collection(&mut self, parent: &str, name: &str) -> Option<String> {
        let p = self.collections.get(parent)?;
        if p.is_soft_deleted() || (p.is_library() && !p.is_extendable()) {
            return None;
        }
        let parent_modes = p.modes.clone();
        let parent_default = p.default_mode.clone();
        let parent_key = p.key.clone().filter(|_| p.is_library());
        let id = self.fresh_id("VariableCollectionId");
        let mut modes = Vec::with_capacity(parent_modes.len());
        let mut mode_map = BTreeMap::new();
        let mut default_mode = String::new();
        for pm in &parent_modes {
            let mid = self.fresh_id_avoiding("VariableModeId", &modes.iter().map(|m: &VariableMode| m.id.clone()).collect());
            if pm.id == parent_default {
                default_mode = mid.clone();
            }
            mode_map.insert(mid.clone(), pm.id.clone());
            modes.push(VariableMode { id: mid, name: pm.name.clone() });
        }
        let mut ext = VariableCollection {
            id: id.clone(),
            name: name.to_string(),
            modes,
            default_mode,
            variables: Vec::new(),
            key: None,
            source_library_key: None,
            extensions: Default::default(),
        };
        ext.set_extension(&CollectionExtension {
            parent: ParentCollection { collection: parent.to_string(), set_key: parent_key },
            mode_map,
            overrides: BTreeMap::new(),
        });
        if let Some(p) = self.collections.get_mut(parent).filter(|p| !p.is_library()) {
            p.set_extendable(true);
        }
        self.collections.insert(id.clone(), ext);
        Some(id)
    }

    /// Overrides `variable` in `mode` of the extended collection `ext` with a literal or an alias. Refused when `ext`
    /// is a library copy or does not extend a collection that holds the variable, when `mode` is not one of its
    /// modes, or when the value does not fit the variable.
    pub fn set_override(&mut self, ext: &str, variable: &str, mode: &str, value: VariableValue) -> bool {
        self.set_override_with(ext, variable, mode, value, true)
    }

    /// [`VariableStore::set_override`], with the alias check optional: a batch runs it once after all its writes.
    pub(crate) fn set_override_with(&mut self, ext: &str, variable: &str, mode: &str, value: VariableValue, guard: bool) -> bool {
        if !self.can_override(ext, variable, mode) {
            return false;
        }
        let kind = self.variables[variable].kind;
        match &value {
            VariableValue::Alias(target) => {
                if !self.variables.get(target).is_some_and(|t| t.kind == kind && t.id != variable) {
                    return false;
                }
            }
            literal => {
                if value_kind(literal) != Some(kind) {
                    return false;
                }
            }
        }
        let write = |s: &mut Self| {
            let Some(c) = s.collections.get_mut(ext) else { return false };
            let Some(mut e) = c.extension() else { return false };
            e.overrides.entry(variable.to_string()).or_default().insert(mode.to_string(), value.clone());
            c.set_extension(&e);
            true
        };
        if guard && matches!(value, VariableValue::Alias(_)) {
            self.guard_alias(write)
        } else {
            write(self)
        }
    }

    /// Removes the override of `variable` in `mode`, so the extended collection shows the parent's value again.
    /// False when there was nothing to reset.
    pub fn reset_override(&mut self, ext: &str, variable: &str, mode: &str) -> bool {
        let Some(c) = self.collections.get_mut(ext).filter(|c| !c.is_library()) else { return false };
        let Some(mut e) = c.extension() else { return false };
        let Some(modes) = e.overrides.get_mut(variable) else { return false };
        if modes.remove(mode).is_none() {
            return false;
        }
        if modes.is_empty() {
            e.overrides.remove(variable);
        }
        c.set_extension(&e);
        true
    }

    /// True when `variable` is overridden in `mode` of `ext`.
    pub fn is_overridden(&self, ext: &str, variable: &str, mode: &str) -> bool {
        self.collections.get(ext).is_some_and(|c| c.extension_override(variable, mode).is_some())
    }

    /// The value `variable` shows in `mode` of the extended collection `ext`.
    pub fn extended_value(&self, ext: &str, variable: &str, mode: &str) -> Option<VariableValue> {
        extension_value(&self.collections, &self.variables, ext, variable, mode)
    }

    /// Variable ids the extended collection `ext` inherits.
    pub fn inherited_variable_ids(&self, ext: &str) -> Vec<String> {
        inherited_ids(&self.collections, &self.variables, ext)
    }

    /// The extended collection whose mode `mode` is, when it extends the collection that holds `variable`.
    pub fn extension_owning_mode(&self, mode: &str, variable: &str) -> Option<String> {
        let holder = &self.variables.get(variable)?.collection;
        self.collections
            .values()
            .find(|c| c.is_extension() && c.modes.iter().any(|m| m.id == mode) && extends(&self.collections, &c.id, holder))
            .map(|c| c.id.clone())
    }

    fn can_override(&self, ext: &str, variable: &str, mode: &str) -> bool {
        let Some(c) = self.collections.get(ext).filter(|c| c.is_extension() && !c.is_library()) else { return false };
        c.modes.iter().any(|m| m.id == mode) && self.inherited_variable_ids(ext).iter().any(|v| v == variable)
    }

    /// Drops the overrides of `variable`, and the overrides that alias it, from every extended collection.
    pub fn forget_overrides_of(&mut self, variable: &str) {
        self.edit_extensions(|e| {
            let before = e.overrides.clone();
            e.overrides.remove(variable);
            for modes in e.overrides.values_mut() {
                modes.retain(|_, v| !matches!(v, VariableValue::Alias(t) if t == variable));
            }
            e.overrides.retain(|_, m| !m.is_empty());
            e.overrides != before
        });
    }

    /// True for a collection whose modes, variables and groups are not edited here: a library copy, or an extended
    /// collection (it takes them from its parent).
    pub fn is_structure_locked(&self, coll: &str) -> bool {
        self.collections.get(coll).is_some_and(|c| c.is_library() || c.is_extension())
    }

    /// A parent mode was deleted: the extensions of `parent` stop mapping to it. Their modes keep any overrides and
    /// show what an unmapped mode shows, the parent's default mode.
    pub fn unmap_parent_mode(&mut self, parent: &str, mode: &str) {
        self.edit_direct_extensions_of(parent, |e| {
            let before = e.mode_map.len();
            e.mode_map.retain(|_, parent_mode| parent_mode != mode);
            e.mode_map.len() != before
        });
    }

    fn edit_direct_extensions_of(&mut self, parent: &str, mut f: impl FnMut(&mut CollectionExtension) -> bool) {
        let parent_key = self.collections.get(parent).and_then(|c| c.key.clone());
        for c in self.collections.values_mut().filter(|c| {
            c.extension_parent_id() == Some(parent)
                || (c.extension_parent_id().is_none() && parent_key.as_deref().is_some() && c.extension_parent_key() == parent_key.as_deref())
        }) {
            if let Some(mut e) = c.extension() {
                if f(&mut e) {
                    c.set_extension(&e);
                }
            }
        }
    }

    /// Removes every collection that extends `parent` (a deleted parent leaves nothing to inherit from).
    pub(crate) fn remove_extensions_of(&mut self, parent: &str) -> Vec<String> {
        let gone: Vec<String> = self
            .collections
            .values()
            .filter(|c| c.id != parent && c.is_extension() && extends(&self.collections, &c.id, parent))
            .map(|c| c.id.clone())
            .collect();
        for id in &gone {
            self.collections.remove(id);
        }
        gone
    }

    fn edit_extensions(&mut self, mut f: impl FnMut(&mut CollectionExtension) -> bool) {
        for c in self.collections.values_mut().filter(|c| c.is_extension()) {
            if let Some(mut e) = c.extension() {
                if f(&mut e) {
                    c.set_extension(&e);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// "Theme" with Light and Dark, one colour variable, one number variable, bound to a rectangle.
    fn base() -> (VariableStore, String, String, String, [String; 2]) {
        let mut s = VariableStore::default();
        let theme = s.add_collection("Theme");
        let light = s.collections[&theme].default_mode.clone();
        let dark = s.add_mode(&theme, "Dark").unwrap();
        let bg = s.add_variable(&theme, "bg", VariableKind::Color).unwrap();
        let gap = s.add_variable(&theme, "gap", VariableKind::Number).unwrap();
        assert!(s.set_value(&bg, &light, VariableValue::Color(Color::WHITE)));
        assert!(s.set_value(&bg, &dark, VariableValue::Color(Color::BLACK)));
        assert!(s.set_value(&gap, &light, VariableValue::Number(8.0)));
        assert!(s.set_value(&gap, &dark, VariableValue::Number(8.0)));
        (s, theme, bg, gap, [light, dark])
    }

    fn doc_with(store: &VariableStore) -> (Document, NodeId, NodeId) {
        let mut d = Document::new("t");
        let page = d.add_page("P");
        let fid = d.alloc_id();
        let frame = d.insert(page, Node::frame(fid, Rect::new(0.0, 0.0, 100.0, 100.0))).unwrap();
        let rid = d.alloc_id();
        let rect = d.insert(frame, Node::rectangle(rid, Rect::new(0.0, 0.0, 10.0, 10.0))).unwrap();
        d.variables = store.variables.clone();
        d.variable_collections = store.collections.clone();
        (d, frame, rect)
    }

    #[test]
    fn an_extension_stores_its_parent_and_maps_every_mode() {
        let (mut s, theme, _, _, [light, dark]) = base();
        let brand = s.extend_collection(&theme, "Brand").unwrap();
        let ext = s.collections[&brand].extension().unwrap();
        assert_eq!(ext.parent.collection, theme);
        assert_eq!(s.collections[&brand].modes.len(), 2);
        assert!(s.collections[&brand].variables.is_empty(), "variables are inherited, not copied");
        let mapped: Vec<&String> = s.collections[&brand].modes.iter().map(|m| &ext.mode_map[&m.id]).collect();
        assert_eq!(mapped, [&light, &dark]);
        assert_eq!(ext.mode_map[&s.collections[&brand].default_mode], light);
        assert!(s.collections[&theme].is_extendable());
        assert_eq!(s.inherited_variable_ids(&brand).len(), 2);
    }

    #[test]
    fn an_extension_shows_the_parent_value_until_it_overrides() {
        let (mut s, theme, bg, _, [_, dark]) = base();
        let brand = s.extend_collection(&theme, "Brand").unwrap();
        let brand_dark = s.collections[&brand].modes[1].id.clone();
        assert_eq!(s.extended_value(&brand, &bg, &brand_dark), Some(VariableValue::Color(Color::BLACK)));
        assert!(s.set_override(&brand, &bg, &brand_dark, VariableValue::Color(Color::rgb(255, 0, 0))));
        assert_eq!(s.extended_value(&brand, &bg, &brand_dark), Some(VariableValue::Color(Color::rgb(255, 0, 0))));
        // a parent change reaches every mode that is not overridden
        assert!(s.set_value(
            &bg,
            &s.collections[&brand].extension().unwrap().mode_map[&s.collections[&brand].modes[0].id].clone(),
            VariableValue::Color(Color::rgb(1, 2, 3))
        ));
        let brand_light = s.collections[&brand].modes[0].id.clone();
        assert_eq!(s.extended_value(&brand, &bg, &brand_light), Some(VariableValue::Color(Color::rgb(1, 2, 3))));
        assert_eq!(s.resolve(&bg, &dark), Some(VariableValue::Color(Color::BLACK)), "the parent keeps its own value");
    }

    #[test]
    fn a_node_that_picks_an_extension_mode_resolves_through_the_override() {
        let (mut s, theme, bg, _, _) = base();
        let brand = s.extend_collection(&theme, "Brand").unwrap();
        let (b_light, b_dark) = (s.collections[&brand].modes[0].id.clone(), s.collections[&brand].modes[1].id.clone());
        assert!(s.set_override(&brand, &bg, &b_dark, VariableValue::Color(Color::rgb(255, 0, 0))));
        let (mut d, frame, rect) = doc_with(&s);
        d.get_mut(rect).unwrap().variables.bindings.insert("fills/0".into(), bg.clone());
        assert_eq!(d.bound_value(rect, "fills/0"), Some(VariableValue::Color(Color::WHITE)), "no choice: the parent's default");
        d.get_mut(frame).unwrap().variables.modes.insert(brand.clone(), b_dark);
        assert_eq!(d.bound_value(rect, "fills/0"), Some(VariableValue::Color(Color::rgb(255, 0, 0))));
        d.get_mut(frame).unwrap().variables.modes.insert(brand.clone(), b_light);
        assert_eq!(d.bound_value(rect, "fills/0"), Some(VariableValue::Color(Color::WHITE)), "not overridden: the parent's mapped mode");
    }

    #[test]
    fn the_nearest_mode_choice_wins_between_parent_and_extension() {
        let (mut s, theme, bg, _, [_, dark]) = base();
        let brand = s.extend_collection(&theme, "Brand").unwrap();
        let b_light = s.collections[&brand].modes[0].id.clone();
        assert!(s.set_override(&brand, &bg, &b_light, VariableValue::Color(Color::rgb(0, 255, 0))));
        let (mut d, frame, rect) = doc_with(&s);
        d.get_mut(rect).unwrap().variables.bindings.insert("fills/0".into(), bg);
        d.get_mut(frame).unwrap().variables.modes.insert(brand, b_light);
        assert_eq!(d.bound_value(rect, "fills/0"), Some(VariableValue::Color(Color::rgb(0, 255, 0))));
        d.get_mut(rect).unwrap().variables.modes.insert(theme, dark);
        assert_eq!(d.bound_value(rect, "fills/0"), Some(VariableValue::Color(Color::BLACK)), "the node's own choice is nearer than the frame's");
    }

    #[test]
    fn resetting_an_override_restores_the_parent_value_in_one_undo_step() {
        let (mut s, theme, bg, _, _) = base();
        let brand = s.extend_collection(&theme, "Brand").unwrap();
        let b_dark = s.collections[&brand].modes[1].id.clone();
        assert!(s.set_override(&brand, &bg, &b_dark, VariableValue::Color(Color::rgb(255, 0, 0))));
        let (mut d, _, _) = doc_with(&s);
        let mut h = History::default();
        let mut edit = VariableStore::from_doc(&d);
        assert!(edit.reset_override(&brand, &bg, &b_dark));
        assert!(!edit.reset_override(&brand, &bg, &b_dark), "nothing left to reset");
        h.exec(&mut d, edit.command()).unwrap();
        assert!(!VariableStore::from_doc(&d).is_overridden(&brand, &bg, &b_dark));
        assert_eq!(d.extended_value(&brand, &bg, &b_dark), Some(VariableValue::Color(Color::BLACK)));
        h.undo(&mut d).unwrap();
        assert_eq!(d.extended_value(&brand, &bg, &b_dark), Some(VariableValue::Color(Color::rgb(255, 0, 0))));
    }

    #[test]
    fn overrides_are_checked_against_the_variable_and_the_collection() {
        let (mut s, theme, bg, _, [light, _]) = base();
        let brand = s.extend_collection(&theme, "Brand").unwrap();
        let b_light = s.collections[&brand].modes[0].id.clone();
        assert!(!s.set_override(&brand, &bg, &b_light, VariableValue::Number(1.0)), "wrong kind");
        assert!(!s.set_override(&brand, &bg, &light, VariableValue::Color(Color::WHITE)), "a parent mode is not a mode of the extension");
        assert!(!s.set_override(&theme, &bg, &light, VariableValue::Color(Color::WHITE)), "a plain collection takes values, not overrides");
        assert!(!s.set_override(&brand, "missing", &b_light, VariableValue::Color(Color::WHITE)));
        assert!(!s.set_override(&brand, &bg, &b_light, VariableValue::Alias(bg.clone())), "no self alias");
    }

    #[test]
    fn an_override_can_alias_another_variable_and_resolves_through_it() {
        let (mut s, theme, bg, _, _) = base();
        let accent = s.add_variable(&theme, "accent", VariableKind::Color).unwrap();
        let theme_light = s.collections[&theme].default_mode.clone();
        assert!(s.set_value(&accent, &theme_light, VariableValue::Color(Color::rgb(9, 9, 9))));
        let brand = s.extend_collection(&theme, "Brand").unwrap();
        let b_light = s.collections[&brand].modes[0].id.clone();
        assert!(s.set_override(&brand, &bg, &b_light, VariableValue::Alias(accent)));
        let (mut d, frame, rect) = doc_with(&s);
        d.get_mut(rect).unwrap().variables.bindings.insert("fills/0".into(), bg);
        d.get_mut(frame).unwrap().variables.modes.insert(brand, b_light);
        assert_eq!(d.bound_value(rect, "fills/0"), Some(VariableValue::Color(Color::rgb(9, 9, 9))));
    }

    #[test]
    fn a_library_parent_must_allow_extension_and_stays_read_only() {
        let (mut s, theme, _, _, _) = base();
        s.collections.get_mut(&theme).unwrap().source_library_key = Some("LIB".into());
        s.collections.get_mut(&theme).unwrap().key = Some("SET".into());
        assert!(s.extend_collection(&theme, "Brand").is_none(), "the publisher has not allowed extension");
        s.collections.get_mut(&theme).unwrap().set_extendable(true);
        let brand = s.extend_collection(&theme, "Brand").unwrap();
        assert_eq!(s.collections[&brand].extension().unwrap().parent.set_key.as_deref(), Some("SET"));
        assert!(!s.collections[&brand].is_library(), "the extension belongs to this file");
    }

    #[test]
    fn deleting_the_parent_deletes_its_extensions_and_a_variable_drops_its_overrides() {
        let (mut s, theme, bg, gap, _) = base();
        let brand = s.extend_collection(&theme, "Brand").unwrap();
        let b_light = s.collections[&brand].modes[0].id.clone();
        assert!(s.set_override(&brand, &gap, &b_light, VariableValue::Number(2.0)));
        assert!(s.set_override(&brand, &bg, &b_light, VariableValue::Color(Color::BLACK)));
        s.delete_variable(&gap);
        assert!(!s.collections[&brand].extension().unwrap().overrides.contains_key(&gap));
        assert!(s.collections[&brand].extension().unwrap().overrides.contains_key(&bg));
        s.delete_collection(&theme);
        assert!(!s.collections.contains_key(&brand));
    }

    #[test]
    fn editing_a_value_in_an_extension_mode_writes_an_override_not_the_parent() {
        let (mut s, theme, bg, _, [_, dark]) = base();
        let brand = s.extend_collection(&theme, "Brand").unwrap();
        let b_dark = s.collections[&brand].modes[1].id.clone();
        assert!(s.set_value(&bg, &b_dark, VariableValue::Color(Color::rgb(255, 0, 0))));
        assert!(s.is_overridden(&brand, &bg, &b_dark));
        assert_eq!(s.variables[&bg].values.len(), 2, "the parent variable gains no value for an extension mode");
        assert_eq!(s.resolve(&bg, &b_dark), Some(VariableValue::Color(Color::rgb(255, 0, 0))));
        assert_eq!(s.resolve(&bg, &dark), Some(VariableValue::Color(Color::BLACK)));
        assert!(!s.set_value(&bg, &b_dark, VariableValue::Number(1.0)), "wrong kind is refused");

        let accent = s.add_variable(&theme, "accent", VariableKind::Color).unwrap();
        assert!(s.set_value(&accent, &dark, VariableValue::Color(Color::rgb(0, 0, 255))));
        assert!(s.set_alias(&bg, &b_dark, &accent), "the alias target is read in the parent mode this mode maps to");
        assert_eq!(s.resolve(&bg, &b_dark), Some(VariableValue::Color(Color::rgb(0, 0, 255))));
        assert!(s.detach_alias(&bg, &b_dark));
        assert_eq!(s.collections[&brand].extension_override(&bg, &b_dark), Some(VariableValue::Color(Color::rgb(0, 0, 255))));
    }

    #[test]
    fn a_library_variable_can_be_overridden_in_a_local_extension_but_not_edited() {
        let (mut s, theme, bg, _, [light, _]) = base();
        s.collections.get_mut(&theme).unwrap().source_library_key = Some("LIB".into());
        s.collections.get_mut(&theme).unwrap().set_extendable(true);
        let brand = s.extend_collection(&theme, "Brand").unwrap();
        let b_light = s.collections[&brand].modes[0].id.clone();
        assert!(!s.set_value(&bg, &light, VariableValue::Color(Color::BLACK)), "the library copy stays read-only");
        assert!(s.set_value(&bg, &b_light, VariableValue::Color(Color::rgb(1, 1, 1))));
        assert!(s.reset_override(&brand, &bg, &b_light));
    }

    #[test]
    fn an_extension_of_an_extension_falls_back_through_the_chain() {
        let (mut s, theme, bg, _, _) = base();
        let brand = s.extend_collection(&theme, "Brand").unwrap();
        let b_light = s.collections[&brand].modes[0].id.clone();
        assert!(s.set_override(&brand, &bg, &b_light, VariableValue::Color(Color::rgb(7, 7, 7))));
        let sub = s.extend_collection(&brand, "Sub").unwrap();
        let (s_light, s_dark) = (s.collections[&sub].modes[0].id.clone(), s.collections[&sub].modes[1].id.clone());
        assert_eq!(s.extended_value(&sub, &bg, &s_light), Some(VariableValue::Color(Color::rgb(7, 7, 7))));
        assert_eq!(s.extended_value(&sub, &bg, &s_dark), Some(VariableValue::Color(Color::BLACK)));
        assert_eq!(s.inherited_variable_ids(&sub).len(), 2);
    }

    #[test]
    fn duplicating_an_extension_shows_the_same_values_in_every_mode() {
        let (mut s, theme, bg, _, _) = base();
        let brand = s.extend_collection(&theme, "Brand").unwrap();
        let (b_light, b_dark) = (s.collections[&brand].modes[0].id.clone(), s.collections[&brand].modes[1].id.clone());
        assert!(s.set_value(&bg, &b_light, VariableValue::Color(Color::rgb(255, 0, 0))));
        let copy = s.duplicate_collection(&brand).unwrap();
        let (c_light, c_dark) = (s.collections[&copy].modes[0].id.clone(), s.collections[&copy].modes[1].id.clone());
        assert_ne!(c_light, b_light, "the copy has its own mode ids");
        for (own, copied) in [(&b_light, &c_light), (&b_dark, &c_dark)] {
            assert_eq!(s.extended_value(&copy, &bg, copied), s.extended_value(&brand, &bg, own));
        }
        assert_eq!(s.extended_value(&copy, &bg, &c_dark), Some(VariableValue::Color(Color::BLACK)), "Dark keeps the parent's Dark");
        assert!(s.is_overridden(&copy, &bg, &c_light), "the override moved to the copy's mode");
        let ext = s.collections[&copy].extension().unwrap();
        assert!(ext.mode_map.keys().chain(ext.overrides.values().flat_map(|m| m.keys())).all(|m| s.collections[&copy].modes.iter().any(|x| &x.id == m)));
        let parent_copy = s.duplicate_collection(&theme).unwrap();
        assert!(!s.collections[&parent_copy].is_extendable(), "the copy of a parent is not extendable");
    }

    #[test]
    fn modes_variables_and_groups_of_an_extension_are_refused_in_the_model() {
        let (mut s, theme, bg, _, _) = base();
        let brand = s.extend_collection(&theme, "Brand").unwrap();
        let b_dark = s.collections[&brand].modes[1].id.clone();
        let (variables, modes) = (s.variables.len(), s.collections[&brand].modes.len());
        assert!(s.add_variable(&brand, "x", VariableKind::Color).is_none());
        assert!(s.add_mode(&brand, "Extra").is_none());
        assert!(s.duplicate_mode(&brand, &b_dark).is_none());
        assert!(!s.delete_mode(&brand, &b_dark));
        s.rename_mode(&brand, &b_dark, "Renamed");
        s.set_default_mode(&brand, &b_dark);
        s.rename_folder(&brand, "a", "b");
        assert_eq!((s.variables.len(), s.collections[&brand].modes.len()), (variables, modes));
        assert_eq!(s.collections[&brand].modes[1].name, "Dark");
        assert_ne!(s.collections[&brand].default_mode, b_dark);
        assert!(s.variables.contains_key(&bg));
    }

    #[test]
    fn overrides_that_alias_each_other_are_refused() {
        let (mut s, theme, bg, _, _) = base();
        let fg = s.add_variable(&theme, "fg", VariableKind::Color).unwrap();
        let brand = s.extend_collection(&theme, "Brand").unwrap();
        let b_light = s.collections[&brand].modes[0].id.clone();
        assert!(s.set_alias(&fg, &b_light, &bg));
        assert!(!s.set_alias(&bg, &b_light, &fg), "bg -> fg -> bg would resolve to nothing");
        assert!(s.resolve(&bg, &b_light).is_some() && s.resolve(&fg, &b_light).is_some());
        assert!(!s.is_overridden(&brand, &bg, &b_light), "the refused override left nothing behind");
    }

    #[test]
    fn deleting_a_parent_mode_unmaps_the_extension_and_it_shows_the_parent_default() {
        let (mut s, theme, bg, _, [light, dark]) = base();
        let brand = s.extend_collection(&theme, "Brand").unwrap();
        let b_dark = s.collections[&brand].modes[1].id.clone();
        assert!(s.delete_mode(&theme, &dark));
        let ext = s.collections[&brand].extension().unwrap();
        assert!(ext.mode_map.values().all(|m| m != &dark), "no mapping to the deleted mode is left");
        assert_eq!(s.collections[&brand].modes.len(), 2, "the extension keeps its own modes");
        assert_eq!(s.extended_value(&brand, &bg, &b_dark), s.variables[&bg].values.get(&light).cloned(), "an unmapped mode shows the parent default");
    }

    #[test]
    fn extension_collections_are_published() {
        let (mut s, theme, bg, _, _) = base();
        let brand = s.extend_collection(&theme, "Brand").unwrap();
        let b_dark = s.collections[&brand].modes[1].id.clone();
        assert!(s.set_value(&bg, &b_dark, VariableValue::Color(Color::rgb(255, 0, 0))));
        let (d, _, _) = doc_with(&s);
        let keys: Vec<String> = publishable_assets(&d).into_iter().map(|(k, _, _)| k).collect();
        assert!(keys.contains(&format!("col:{theme}")), "the parent publishes");
        assert!(keys.contains(&format!("col:{brand}")), "the extension publishes: {keys:?}");
        let snap = snapshot(&d, "L1", "Lib", 1);
        assert!(snap.variable_collections.contains_key(&theme));
        assert!(snap.variable_collections.contains_key(&brand));
    }

    #[test]
    fn importing_an_extension_from_a_library_prefixes_its_parent_link_and_modes_and_leaves_the_consumer_modes_editable() {
        let (mut lib_store, theme, bg, _, [light, _]) = base();
        let brand = lib_store.extend_collection(&theme, "Brand").unwrap();
        let b_light = lib_store.collections[&brand].modes[0].id.clone();
        let b_dark = lib_store.collections[&brand].modes[1].id.clone();
        assert!(lib_store.set_value(&bg, &b_dark, VariableValue::Color(Color::rgb(255, 0, 0))));
        let (mut lib, _, _) = doc_with(&lib_store);
        let lib_page = lib.pages[0];
        let cid = lib.alloc_id();
        let mut comp = Node::new(cid, NodeKind::Component(FrameProps::default()));
        comp.name = "Card".into();
        comp.variables.bindings.insert("fills/0".into(), bg.clone());
        comp.variables.modes.insert(brand.clone(), b_dark.clone());
        lib.insert(lib_page, comp).unwrap();
        let snap = snapshot(&lib, "L1", "Lib", 1);

        // Consumer file has its own collections and modes with the same Studio-allocated ids.
        let (mut own_store, own_theme, own_bg, _, [own_light, own_dark]) = base();
        let own_brand = own_store.extend_collection(&own_theme, "OwnBrand").unwrap();
        let own_b_dark = own_store.collections[&own_brand].modes[1].id.clone();
        let (mut d, _, _) = doc_with(&own_store);
        let page = d.pages[0];
        let cmd = import_asset(&mut d, &snap, &format!("col:{brand}"), None);
        cmd.apply(&mut d).unwrap();
        let comp_key = format!("c{}", cid.0);
        let cmd = import_asset(&mut d, &snap, &comp_key, Some(Placement { parent: page, at: Vec2::ZERO }));
        cmd.apply(&mut d).unwrap();

        let lib_brand = format!("L1/{brand}");
        let lib_theme = format!("L1/{theme}");
        let lib_bg = format!("L1/{bg}");
        let lib_b_light = format!("L1/{b_light}");
        let lib_b_dark = format!("L1/{b_dark}");
        let imported = &d.variable_collections[&lib_brand];
        assert_eq!(imported.extension_parent_id(), Some(lib_theme.as_str()));
        assert_eq!(imported.modes.iter().map(|m| m.id.as_str()).collect::<Vec<_>>(), vec![lib_b_light.as_str(), lib_b_dark.as_str()]);
        assert_eq!(imported.default_mode, lib_b_light);
        assert_eq!(d.extended_value(&lib_brand, &lib_bg, &lib_b_dark), Some(VariableValue::Color(Color::rgb(255, 0, 0))));
        assert_eq!(d.extended_value(&lib_brand, &lib_bg, &lib_b_light), lib.variables[&bg].values.get(&light).cloned());
        let inst = *d.get(page).unwrap().children.last().unwrap();
        assert_eq!(d.bound_value(inst, "fills/0"), Some(VariableValue::Color(Color::rgb(255, 0, 0))));
        assert_eq!(pending_updates(&d, &snap), vec![]);

        let mut store = VariableStore::from_doc(&d);
        assert!(store.set_value(&own_bg, &own_light, VariableValue::Color(Color::rgb(1, 2, 3))), "consumer base mode 1 remains editable");
        assert!(store.set_value(&own_bg, &own_dark, VariableValue::Color(Color::rgb(4, 5, 6))), "consumer base mode 2 remains editable");
        assert!(store.set_value(&own_bg, &own_b_dark, VariableValue::Color(Color::rgb(7, 8, 9))), "consumer extension mode remains editable");
    }

    #[test]
    fn an_extension_whose_parent_is_known_only_by_set_key_resolves_through_that_key() {
        let (mut s, theme, bg, gap, [light, _]) = base();
        s.collections.get_mut(&theme).unwrap().key = Some("SET_THEME".into());
        let brand = s.extend_collection(&theme, "Brand").unwrap();
        let b_light = s.collections[&brand].modes[0].id.clone();
        let b_dark = s.collections[&brand].modes[1].id.clone();
        assert!(s.set_value(&bg, &b_dark, VariableValue::Color(Color::rgb(255, 0, 0))));
        let mut ext = s.collections[&brand].extension().unwrap();
        ext.parent = ParentCollection { collection: String::new(), set_key: Some("SET_THEME".into()) };
        s.collections.get_mut(&brand).unwrap().set_extension(&ext);

        assert_eq!(s.inherited_variable_ids(&brand), vec![bg.clone(), gap]);
        assert_eq!(s.extended_value(&brand, &bg, &b_dark), Some(VariableValue::Color(Color::rgb(255, 0, 0))));
        assert_eq!(s.extended_value(&brand, &bg, &b_light), s.variables[&bg].values.get(&light).cloned());
        assert_eq!(s.resolve(&bg, &b_dark), Some(VariableValue::Color(Color::rgb(255, 0, 0))));
        assert_eq!(s.resolve(&bg, &b_light), s.variables[&bg].values.get(&light).cloned());

        let (mut d, frame, rect) = doc_with(&s);
        d.get_mut(frame).unwrap().variables.modes.insert(brand.clone(), b_dark);
        d.get_mut(rect).unwrap().variables.bindings.insert("fills/0".into(), bg.clone());
        assert_eq!(d.bound_value(rect, "fills/0"), Some(VariableValue::Color(Color::rgb(255, 0, 0))));
        d.get_mut(frame).unwrap().variables.modes.insert(brand, b_light);
        assert_eq!(d.bound_value(rect, "fills/0"), s.variables[&bg].values.get(&light).cloned());
    }

    #[test]
    fn a_parent_alias_cannot_close_a_cycle_through_an_extension_override_in_either_order() {
        for reversed in [false, true] {
            let (mut s, theme, bg, _, [light, _]) = base();
            let fg = s.add_variable(&theme, "fg", VariableKind::Color).unwrap();
            assert!(s.set_value(&fg, &light, VariableValue::Color(Color::BLACK)));
            let brand = s.extend_collection(&theme, "Brand").unwrap();
            let b_light = s.collections[&brand].modes[0].id.clone();
            let (first, second) = if reversed { ((&bg, &light, &fg), (&fg, &b_light, &bg)) } else { ((&fg, &b_light, &bg), (&bg, &light, &fg)) };
            assert!(s.set_alias(first.0, first.1, first.2), "the first alias is fine");
            assert!(!s.set_alias(second.0, second.1, second.2), "the second would leave a variable with no value (reversed: {reversed})");
            for v in [&bg, &fg] {
                assert!(s.resolve(v, &b_light).is_some() && s.resolve(v, &light).is_some(), "{v} still resolves in the parent and in Brand");
            }
        }
    }

    #[test]
    fn a_value_in_a_mode_of_an_unrelated_extension_is_refused() {
        let (mut s, theme, _, _, _) = base();
        let brand = s.extend_collection(&theme, "Brand").unwrap();
        let b_light = s.collections[&brand].modes[0].id.clone();
        let spacing = s.add_collection("Spacing");
        let sm = s.collections[&spacing].default_mode.clone();
        let gap = s.add_variable(&spacing, "gap", VariableKind::Number).unwrap();
        assert!(s.set_value(&gap, &sm, VariableValue::Number(8.0)));
        assert!(!s.set_value(&gap, &b_light, VariableValue::Number(4.0)), "gap does not belong to what Brand extends");
        assert!(!s.variables[&gap].values.contains_key(&b_light));
        assert!(s.set_value(&gap, &sm, VariableValue::Number(9.0)), "its own modes still edit");
    }

    #[test]
    fn the_extension_rules_refuse_variables_in_an_extension_and_stray_mode_keys() {
        let (mut s, theme, bg, _, _) = base();
        let brand = s.extend_collection(&theme, "Brand").unwrap();
        assert_eq!(s.check_extension_rules(), Ok(()));
        let mut a = s.clone();
        a.collections.get_mut(&brand).unwrap().variables.push(bg.clone());
        assert!(a.check_extension_rules().is_err(), "an extension with a variable list");
        let mut b = s.clone();
        b.variables.get_mut(&bg).unwrap().collection = brand.clone();
        assert!(b.check_extension_rules().is_err(), "a variable whose collection is an extension");
        let mut c = s.clone();
        let spacing = c.add_collection("Spacing");
        let gap = c.add_variable(&spacing, "gap", VariableKind::Number).unwrap();
        let b_light = c.collections[&brand].modes[0].id.clone();
        c.variables.get_mut(&gap).unwrap().values.insert(b_light, VariableValue::Number(4.0));
        assert!(c.check_extension_rules().is_err(), "a value keyed by a mode of an extension the variable is not part of");
    }

    #[test]
    fn a_lookup_without_extensions_scans_no_collection() {
        let mut s = VariableStore::default();
        let mut last = String::new();
        for i in 0..10 {
            let c = s.add_collection(&format!("C{i}"));
            last = s.add_variable(&c, "v", VariableKind::Number).unwrap();
        }
        let (d, _, rect) = doc_with(&s);
        reset_extension_scan_count();
        for _ in 0..100 {
            d.resolve_variable(rect, &last);
        }
        assert_eq!(extension_scan_count(), 0, "10 collections, no extension: no scan");
        // the same lookups in a document that does extend a collection do scan, so the counter is live
        let (mut s2, theme, ..) = base();
        s2.extend_collection(&theme, "Brand").unwrap();
        let (d2, _, rect2) = doc_with(&s2);
        let var = d2.variables.keys().next().cloned().unwrap();
        reset_extension_scan_count();
        d2.resolve_variable(rect2, &var);
        assert!(extension_scan_count() > 0);
    }

    #[test]
    fn the_snapshot_never_carries_a_variable_that_names_an_extension() {
        let (mut s, theme, _, _, _) = base();
        let brand = s.extend_collection(&theme, "Brand").unwrap();
        let stray = "stray".to_string();
        s.variables.insert(
            stray.clone(),
            Variable {
                id: stray.clone(),
                name: "stray".into(),
                collection: brand.clone(),
                kind: VariableKind::Color,
                values: Default::default(),
                description: String::new(),
                scopes: vec![],
                hidden_from_publishing: false,
                key: None,
                code_syntax: Default::default(),
                extensions: Default::default(),
                mode_extensions: Default::default(),
            },
        );
        let (d, _, _) = doc_with(&s);
        let keys: Vec<String> = publishable_assets(&d).into_iter().map(|(k, _, _)| k).collect();
        assert!(!keys.contains(&"var:stray".to_string()), "{keys:?}");
        let snap = snapshot(&d, "L1", "Lib", 1);
        assert!(!snap.variables.contains_key("stray"));
        assert!(snap.variable_collections.contains_key(&brand));
    }

    #[test]
    fn publishing_names_the_components_that_pick_an_extension_mode() {
        let (mut s, theme, bg, _, _) = base();
        let brand = s.extend_collection(&theme, "Brand").unwrap();
        let b_dark = s.collections[&brand].modes[1].id.clone();
        let (mut d, frame, _) = doc_with(&s);
        let page = d.pages[0];
        let cid = d.alloc_id();
        let mut comp = Node::new(cid, NodeKind::Component(FrameProps::default()));
        comp.name = "Card".into();
        comp.variables.bindings.insert("fills/0".into(), bg);
        comp.variables.modes.insert(brand, b_dark);
        d.insert(page, comp).unwrap();
        assert_eq!(publish_warnings(&d), vec!["Card".to_string()]);
        d.get_mut(frame).unwrap().variables.modes.insert("x".into(), "y".into());
        assert_eq!(publish_warnings(&d), vec!["Card".to_string()], "a plain mode pick is not reported");
    }

    #[test]
    fn replacing_modes_on_the_parent_through_the_shared_path_unmaps_extensions() {
        let (mut s, theme, _, _, [_, dark]) = base();
        let brand = s.extend_collection(&theme, "Brand").unwrap();
        s.unmap_parent_mode(&theme, &dark);
        assert!(s.collections[&brand].extension().unwrap().mode_map.values().all(|m| m != &dark));
    }

    #[test]
    fn a_document_whose_extension_parent_is_not_extendable_opens_normalised_and_resolves_the_same() {
        let (mut s, theme, bg, _, _) = base();
        let brand = s.extend_collection(&theme, "Brand").unwrap();
        let b_dark = s.collections[&brand].modes[1].id.clone();
        assert!(s.set_value(&bg, &b_dark, VariableValue::Color(Color::rgb(255, 0, 0))));
        s.collections.get_mut(&theme).unwrap().set_extendable(false);
        let (d, frame, rect) = doc_with(&s);
        let mut d = d;
        d.get_mut(frame).unwrap().variables.modes.insert(brand.clone(), b_dark.clone());
        d.get_mut(rect).unwrap().variables.bindings.insert("fills/0".into(), bg.clone());
        let opened = Document::from_json(&d.to_json().unwrap()).unwrap();
        assert!(opened.variable_collections[&theme].is_extendable(), "the flag is set on open");
        let canvas = opened.bound_value(rect, "fills/0");
        assert_eq!(canvas, VariableStore::from_doc(&opened).resolve(&bg, &b_dark), "canvas and panel agree");
        assert_eq!(canvas, Some(VariableValue::Color(Color::rgb(255, 0, 0))));
    }

    #[test]
    fn violations_are_a_set_so_an_edit_is_judged_by_what_it_adds() {
        let (mut s, theme, bg, _, _) = base();
        let brand = s.extend_collection(&theme, "Brand").unwrap();
        assert!(s.extension_violations().is_empty());
        // an extension-local variable, as an import keeps it
        s.variables.get_mut(&bg).unwrap().collection = brand.clone();
        let before = s.extension_violations();
        assert_eq!(before.len(), 1, "{before:?}");
        // an unrelated edit leaves the set as it was
        let spacing = s.add_collection("Spacing");
        let gap = s.add_variable(&spacing, "gap", VariableKind::Number).unwrap();
        let sm = s.collections[&spacing].default_mode.clone();
        assert!(s.set_value(&gap, &sm, VariableValue::Number(12.0)));
        assert!(s.extension_violations().is_subset(&before));
        // a new break is in the set and not in `before`
        let b_light = s.collections[&brand].modes[0].id.clone();
        s.variables.get_mut(&gap).unwrap().values.insert(b_light, VariableValue::Number(4.0));
        assert!(!s.extension_violations().is_subset(&before));
    }

    #[test]
    fn adding_another_extension_owned_variable_is_a_new_rule_violation() {
        let (mut s, theme, ..) = base();
        let brand = s.extend_collection(&theme, "Brand").unwrap();
        s.collections.get_mut(&brand).unwrap().variables.push("imported-own-variable".into());
        let before = s.extension_violations();
        s.collections.get_mut(&brand).unwrap().variables.push("new-own-variable".into());
        assert!(s.rules_added_since(&before).is_err(), "the second listed variable adds a separate violation");
    }

    #[test]
    fn a_failed_value_write_batch_leaves_the_store_unchanged() {
        let (mut s, theme, bg, _, [light, _]) = base();
        let brand = s.extend_collection(&theme, "Brand").unwrap();
        let brand_light = s.collections[&brand].modes[0].id.clone();
        let before = s.clone();
        let writes = vec![
            (bg.clone(), light.clone(), Some(VariableValue::Color(Color::rgb(1, 2, 3)))),
            (bg.clone(), brand_light, Some(VariableValue::Color(Color::rgb(4, 5, 6)))),
            ("missing-variable".into(), light, Some(VariableValue::Color(Color::BLACK))),
        ];
        assert!(s.apply_value_writes(writes).is_err(), "the final write targets no variable");
        assert_eq!(s, before, "a refused batch commits no earlier writes");
    }

    #[test]
    fn a_batch_of_aliases_runs_the_resolution_check_twice_not_once_per_alias() {
        let mut s = VariableStore::default();
        let c = s.add_collection("C");
        let mode = s.collections[&c].default_mode.clone();
        let ids: Vec<String> = (0..2500).map(|i| s.add_variable(&c, &format!("v{i}"), VariableKind::Number).unwrap()).collect();
        let writes: Vec<(String, String, Option<VariableValue>)> =
            (0..1000).map(|i| (ids[i].clone(), mode.clone(), Some(VariableValue::Alias(ids[1000 + i].clone())))).collect();
        reset_alias_check_count();
        let start = std::time::Instant::now();
        s.apply_value_writes(writes).unwrap();
        eprintln!("1000 aliases in a 2500-variable store: {:?}, {} resolution checks", start.elapsed(), alias_check_count());
        assert_eq!(alias_check_count(), 2, "before and after, whatever the number of aliases");
        assert!(s.variables[&ids[0]].values[&mode] == VariableValue::Alias(ids[1000].clone()));
    }

    #[test]
    fn a_batch_may_list_an_alias_before_the_value_it_points_to_but_not_close_a_cycle() {
        let (mut s, theme, bg, _, [light, _]) = base();
        let fg = s.add_variable(&theme, "fg", VariableKind::Color).unwrap();
        let prim = s.add_variable(&theme, "prim", VariableKind::Color).unwrap();
        s.variables.get_mut(&prim).unwrap().values.clear();
        let ok = vec![
            (fg.clone(), light.clone(), Some(VariableValue::Alias(prim.clone()))),
            (prim.clone(), light.clone(), Some(VariableValue::Color(Color::rgb(1, 2, 3)))),
        ];
        assert!(s.apply_value_writes(ok).is_ok(), "the alias comes first in the list, the value second");
        assert_eq!(s.resolve(&fg, &light), Some(VariableValue::Color(Color::rgb(1, 2, 3))));
        let cycle =
            vec![(bg.clone(), light.clone(), Some(VariableValue::Alias(fg.clone()))), (prim.clone(), light.clone(), Some(VariableValue::Alias(bg.clone())))];
        let before_cycle = s.clone();
        assert!(s.apply_value_writes(cycle).is_err(), "a cycle closed inside one batch is refused");
        assert_eq!(s, before_cycle, "a refused alias batch commits no writes");
    }
}
