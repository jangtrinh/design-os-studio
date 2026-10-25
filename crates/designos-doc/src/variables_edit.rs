//! Editing model for variables: a clonable working copy of the variables and
//! collections with the operations the Variables panel needs (add, rename,
//! duplicate, delete, reorder, set values and aliases). Commit it with
//! [`VariableStore::command`] so the whole edit is one undo step.

use crate::*;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug, Default, PartialEq)]
pub struct VariableStore {
    pub variables: BTreeMap<String, Variable>,
    pub collections: BTreeMap<String, VariableCollection>,
}

/// Default value for a new variable of `kind`.
pub fn default_value(kind: VariableKind) -> VariableValue {
    match kind {
        VariableKind::Color => VariableValue::Color(Color::rgb(0xD9, 0xD9, 0xD9)),
        VariableKind::Number => VariableValue::Number(0.0),
        VariableKind::String => VariableValue::String(String::new()),
        VariableKind::Boolean => VariableValue::Boolean(false),
    }
}

/// Kind a literal value belongs to (`None` for aliases).
pub fn value_kind(v: &VariableValue) -> Option<VariableKind> {
    Some(match v {
        VariableValue::Color(_) => VariableKind::Color,
        VariableValue::Number(_) => VariableKind::Number,
        VariableValue::String(_) => VariableKind::String,
        VariableValue::Boolean(_) => VariableKind::Boolean,
        VariableValue::Alias(_) => return None,
    })
}

/// Splits "Color/Brand/Primary" into ("Color/Brand", "Primary").
pub fn split_name(name: &str) -> (&str, &str) {
    match name.rfind('/') {
        Some(i) => (&name[..i], &name[i + 1..]),
        None => ("", name),
    }
}

impl VariableStore {
    pub fn from_doc(doc: &Document) -> Self {
        Self { variables: doc.variables.clone(), collections: doc.variable_collections.clone() }
    }

    /// The undoable command that installs this state.
    pub fn command(self) -> Command {
        Command::SetVariables { variables: self.variables, collections: self.collections }
    }

    pub fn matches_doc(&self, doc: &Document) -> bool {
        self.variables == doc.variables && self.collections == doc.variable_collections
    }

    fn fresh_id(&self, prefix: &str) -> String {
        self.fresh_id_avoiding(prefix, &BTreeSet::new())
    }

    fn fresh_id_avoiding(&self, prefix: &str, avoid: &BTreeSet<String>) -> String {
        let mut n = 1u64;
        loop {
            let id = format!("{prefix}:d{n}");
            let taken = avoid.contains(&id)
                || self.variables.contains_key(&id)
                || self.collections.contains_key(&id)
                || self.collections.values().any(|c| c.modes.iter().any(|m| m.id == id));
            if !taken {
                return id;
            }
            n += 1;
        }
    }

    /// Name not used by another variable of the collection (appends " 2", " 3"...).
    fn unique_variable_name(&self, coll: &str, wanted: &str, except: Option<&str>) -> String {
        let taken = |n: &str| self.variables.values().any(|v| v.collection == coll && Some(v.id.as_str()) != except && v.name == n);
        if !taken(wanted) {
            return wanted.to_string();
        }
        let mut i = 2;
        loop {
            let cand = format!("{wanted} {i}");
            if !taken(&cand) {
                return cand;
            }
            i += 1;
        }
    }

    /// True when `coll` is a subscribed copy of a library's collection: it is edited in its library
    /// file, so every edit that would reach it, or its variables, is refused.
    pub fn is_library_collection(&self, coll: &str) -> bool {
        self.collections.get(coll).is_some_and(VariableCollection::is_library)
    }

    /// True when `var` lives in a library collection.
    pub fn is_library_variable(&self, var: &str) -> bool {
        self.variables.get(var).is_some_and(|v| self.is_library_collection(&v.collection))
    }

    /// Puts every library collection and its variables back to how `original` has them. The panel
    /// calls this before it commits, so a path that edits the maps directly cannot change a library.
    pub fn enforce_library_read_only(&mut self, original: &VariableStore) {
        let is_library = |id: &str| original.is_library_collection(id);
        // a variable of the file's own that an edit pointed at a library collection
        let moved_in: Vec<String> = self.variables.iter().filter(|(_, v)| is_library(&v.collection)).map(|(id, _)| id.clone()).collect();
        self.collections.retain(|id, _| !is_library(id));
        self.variables.retain(|_, v| !is_library(&v.collection));
        for (id, c) in original.collections.iter().filter(|(id, _)| is_library(id)) {
            self.collections.insert(id.clone(), c.clone());
        }
        for (id, v) in original.variables.iter().filter(|(_, v)| is_library(&v.collection)) {
            self.variables.insert(id.clone(), v.clone());
        }
        for id in moved_in {
            if let Some(v) = original.variables.get(&id).filter(|v| !is_library(&v.collection)) {
                self.variables.insert(id, v.clone());
            }
        }
        // only its own collection lists a library variable
        for c in self.collections.values_mut().filter(|c| !c.is_library()) {
            c.variables.retain(|v| !original.is_library_variable(v));
        }
    }

    // ------------------------------------------------------------ collections

    pub fn add_collection(&mut self, name: &str) -> String {
        let id = self.fresh_id("VariableCollectionId");
        let mode = self.fresh_id("VariableModeId");
        self.collections.insert(
            id.clone(),
            VariableCollection {
                id: id.clone(),
                name: name.to_string(),
                modes: vec![VariableMode { id: mode.clone(), name: "Mode 1".into() }],
                default_mode: mode,
                variables: Vec::new(),
                key: None,
                source_library_key: None,
                extensions: BTreeMap::new(),
            },
        );
        id
    }

    pub fn rename_collection(&mut self, id: &str, name: &str) {
        if self.is_library_collection(id) {
            return;
        }
        if let Some(c) = self.collections.get_mut(id) {
            c.name = name.to_string();
        }
    }

    pub fn duplicate_collection(&mut self, id: &str) -> Option<String> {
        let src = self.collections.get(id)?.clone();
        let new_id = self.fresh_id("VariableCollectionId");
        // fresh mode ids, remembered so values can be remapped
        let mut mode_map = BTreeMap::new();
        let mut modes = Vec::new();
        let mut reserved = BTreeSet::new();
        for m in &src.modes {
            let nid = self.fresh_id_avoiding("VariableModeId", &reserved);
            reserved.insert(nid.clone());
            mode_map.insert(m.id.clone(), nid.clone());
            modes.push(VariableMode { id: nid, name: m.name.clone() });
        }
        let default_mode = mode_map.get(&src.default_mode).cloned().unwrap_or_default();
        let mut var_ids = Vec::new();
        let mut id_map = BTreeMap::new();
        for vid in &src.variables {
            let Some(v) = self.variables.get(vid).cloned() else { continue };
            let nv = self.fresh_id("VariableID");
            let mut copy = v.clone();
            copy.id = nv.clone();
            copy.key = None;
            copy.collection = new_id.clone();
            copy.values = v.values.iter().filter_map(|(m, x)| Some((mode_map.get(m)?.clone(), x.clone()))).collect();
            copy.mode_extensions = v.mode_extensions.iter().filter_map(|(m, x)| Some((mode_map.get(m)?.clone(), x.clone()))).collect();
            self.variables.insert(nv.clone(), copy);
            id_map.insert(vid.clone(), nv.clone());
            var_ids.push(nv);
        }
        // aliases inside the copy keep pointing at the copy's siblings
        for nv in &var_ids {
            if let Some(v) = self.variables.get_mut(nv) {
                for x in v.values.values_mut() {
                    if let VariableValue::Alias(t) = x {
                        if let Some(n) = id_map.get(t) {
                            *t = n.clone();
                        }
                    }
                }
            }
        }
        self.collections.insert(
            new_id.clone(),
            VariableCollection {
                id: new_id.clone(),
                name: format!("{} copy", src.name),
                modes,
                default_mode,
                variables: var_ids,
                key: None,
                source_library_key: None,
                extensions: src.extensions.clone(),
            },
        );
        Some(new_id)
    }

    /// Removes a collection with its variables. Returns the removed variable ids.
    pub fn delete_collection(&mut self, id: &str) -> Vec<String> {
        if self.is_library_collection(id) {
            return Vec::new();
        }
        let Some(c) = self.collections.remove(id) else { return Vec::new() };
        let mut gone: Vec<String> = c.variables.clone();
        gone.extend(self.variables.values().filter(|v| v.collection == id).map(|v| v.id.clone()));
        gone.sort();
        gone.dedup();
        // a list that names a variable of a library collection does not make it this collection's to delete
        gone.retain(|g| !self.is_library_variable(g));
        for g in &gone {
            self.remove_variable_inner(g);
        }
        gone
    }

    // ------------------------------------------------------------ modes

    pub fn add_mode(&mut self, coll: &str, name: &str) -> Option<String> {
        if self.is_library_collection(coll) {
            return None;
        }
        let id = self.fresh_id("VariableModeId");
        let c = self.collections.get(coll)?;
        let base = c.default_mode.clone();
        let ids = c.variables.clone();
        for vid in ids {
            if let Some(v) = self.variables.get_mut(&vid) {
                let val = v.values.get(&base).cloned().unwrap_or_else(|| default_value(v.kind));
                v.values.insert(id.clone(), val);
            }
        }
        self.collections.get_mut(coll)?.modes.push(VariableMode { id: id.clone(), name: name.to_string() });
        Some(id)
    }

    pub fn rename_mode(&mut self, coll: &str, mode: &str, name: &str) {
        if self.is_library_collection(coll) {
            return;
        }
        if let Some(m) = self.collections.get_mut(coll).and_then(|c| c.modes.iter_mut().find(|m| m.id == mode)) {
            m.name = name.to_string();
        }
    }

    pub fn duplicate_mode(&mut self, coll: &str, mode: &str) -> Option<String> {
        if self.is_library_collection(coll) {
            return None;
        }
        let name = self.collections.get(coll)?.modes.iter().find(|m| m.id == mode)?.name.clone();
        let id = self.fresh_id("VariableModeId");
        let ids = self.collections.get(coll)?.variables.clone();
        for vid in ids {
            if let Some(v) = self.variables.get_mut(&vid) {
                if let Some(x) = v.values.get(mode).cloned() {
                    v.values.insert(id.clone(), x);
                }
                if let Some(ext) = v.mode_extensions.get(mode).cloned() {
                    v.mode_extensions.insert(id.clone(), ext);
                }
            }
        }
        let c = self.collections.get_mut(coll)?;
        let pos = c.modes.iter().position(|m| m.id == mode)? + 1;
        c.modes.insert(pos, VariableMode { id: id.clone(), name: format!("{name} copy") });
        Some(id)
    }

    /// Deletes a mode (the last one cannot be deleted). Returns whether it was removed.
    pub fn delete_mode(&mut self, coll: &str, mode: &str) -> bool {
        if self.is_library_collection(coll) {
            return false;
        }
        let Some(c) = self.collections.get_mut(coll) else { return false };
        if c.modes.len() <= 1 || !c.modes.iter().any(|m| m.id == mode) {
            return false;
        }
        c.modes.retain(|m| m.id != mode);
        if c.default_mode == mode {
            c.default_mode = c.modes[0].id.clone();
        }
        let ids = c.variables.clone();
        for v in self.variables.values_mut() {
            if v.collection == coll || ids.contains(&v.id) {
                v.values.remove(mode);
                v.mode_extensions.remove(mode);
            }
        }
        true
    }

    pub fn set_default_mode(&mut self, coll: &str, mode: &str) {
        if self.is_library_collection(coll) {
            return;
        }
        if let Some(c) = self.collections.get_mut(coll) {
            if c.modes.iter().any(|m| m.id == mode) {
                c.default_mode = mode.to_string();
            }
        }
    }

    // ------------------------------------------------------------ variables

    pub fn add_variable(&mut self, coll: &str, name: &str, kind: VariableKind) -> Option<String> {
        if self.is_library_collection(coll) {
            return None;
        }
        let id = self.fresh_id("VariableID");
        let name = self.unique_variable_name(coll, name, None);
        let c = self.collections.get_mut(coll)?;
        let values = c.modes.iter().map(|m| (m.id.clone(), default_value(kind))).collect();
        c.variables.push(id.clone());
        self.variables.insert(
            id.clone(),
            Variable {
                id: id.clone(),
                name,
                collection: coll.to_string(),
                kind,
                values,
                description: String::new(),
                scopes: Vec::new(),
                hidden_from_publishing: false,
                key: None,
                code_syntax: Default::default(),
                extensions: BTreeMap::new(),
                mode_extensions: BTreeMap::new(),
            },
        );
        Some(id)
    }

    pub fn rename_variable(&mut self, id: &str, name: &str) {
        if self.is_library_variable(id) {
            return;
        }
        let Some(coll) = self.variables.get(id).map(|v| v.collection.clone()) else { return };
        let name = name.trim().trim_matches('/');
        if name.is_empty() {
            return;
        }
        let name = self.unique_variable_name(&coll, name, Some(id));
        if let Some(v) = self.variables.get_mut(id) {
            v.name = name;
        }
    }

    pub fn duplicate_variable(&mut self, id: &str) -> Option<String> {
        if self.is_library_variable(id) {
            return None;
        }
        let src = self.variables.get(id)?.clone();
        let nid = self.fresh_id("VariableID");
        let name = self.unique_variable_name(&src.collection, &src.name, None);
        let mut copy = src;
        copy.id = nid.clone();
        copy.name = name;
        copy.key = None;
        let c = self.collections.get_mut(&copy.collection)?;
        let pos = c.variables.iter().position(|v| v == id).map(|p| p + 1).unwrap_or(c.variables.len());
        c.variables.insert(pos, nid.clone());
        self.variables.insert(nid.clone(), copy);
        Some(nid)
    }

    /// Literal value of `var` in `mode`, following aliases (each alias in its own
    /// collection's default mode when `mode` is not one of its modes).
    pub fn resolve(&self, var: &str, mode: &str) -> Option<VariableValue> {
        let mut id = var.to_string();
        let mut mode = mode.to_string();
        for _ in 0..16 {
            let v = self.variables.get(&id)?;
            let c = self.collections.get(&v.collection)?;
            let m = if c.modes.iter().any(|x| x.id == mode) { mode.clone() } else { c.default_mode.clone() };
            match v.values.get(&m).or_else(|| v.values.get(&c.default_mode))? {
                VariableValue::Alias(next) => {
                    id = next.clone();
                    mode = m;
                }
                other => return Some(other.clone()),
            }
        }
        None
    }

    /// Whether `var` may alias `target`: same kind, a different variable, no cycle.
    pub fn can_alias(&self, var: &str, target: &str) -> bool {
        let (Some(v), Some(t)) = (self.variables.get(var), self.variables.get(target)) else { return false };
        if var == target || v.kind != t.kind {
            return false;
        }
        // target must not (transitively) reference `var`
        let mut seen = BTreeSet::new();
        let mut stack = vec![target.to_string()];
        while let Some(cur) = stack.pop() {
            if cur == var {
                return false;
            }
            if !seen.insert(cur.clone()) {
                continue;
            }
            if let Some(x) = self.variables.get(&cur) {
                for val in x.values.values() {
                    if let VariableValue::Alias(n) = val {
                        stack.push(n.clone());
                    }
                }
            }
        }
        true
    }

    /// Sets a literal value; ignored when its type does not match the variable.
    pub fn set_value(&mut self, var: &str, mode: &str, value: VariableValue) -> bool {
        if self.is_library_variable(var) {
            return false;
        }
        let Some(v) = self.variables.get_mut(var) else { return false };
        match value_kind(&value) {
            Some(k) if k == v.kind => {}
            _ => return false,
        }
        v.values.insert(mode.to_string(), value);
        v.clear_derived_mode_extensions(mode);
        true
    }

    /// Makes the value in `mode` an alias of `target`.
    pub fn set_alias(&mut self, var: &str, mode: &str, target: &str) -> bool {
        if self.is_library_variable(var) || !self.can_alias(var, target) {
            return false;
        }
        if let Some(v) = self.variables.get_mut(var) {
            v.values.insert(mode.to_string(), VariableValue::Alias(target.to_string()));
            v.clear_derived_mode_extensions(mode);
            return true;
        }
        false
    }

    /// Replaces an alias by the literal it currently resolves to.
    pub fn detach_alias(&mut self, var: &str, mode: &str) -> bool {
        if self.is_library_variable(var) {
            return false;
        }
        let Some(lit) = self.resolve(var, mode) else { return false };
        match self.variables.get_mut(var) {
            Some(v) => {
                v.values.insert(mode.to_string(), lit);
                v.clear_derived_mode_extensions(mode);
                true
            }
            None => false,
        }
    }

    fn remove_variable_inner(&mut self, id: &str) {
        let Some(gone) = self.variables.get(id).cloned() else { return };
        // aliases to the deleted variable become the literal they resolved to
        let affected: Vec<(String, String)> = self
            .variables
            .values()
            .flat_map(|v| {
                v.values.iter().filter(|(_, x)| matches!(x, VariableValue::Alias(t) if t == id)).map(|(m, _)| (v.id.clone(), m.clone())).collect::<Vec<_>>()
            })
            .collect();
        for (vid, mode) in affected {
            let lit = self.resolve(id, &mode).or_else(|| gone.values.values().find(|x| !matches!(x, VariableValue::Alias(_))).cloned());
            if let Some(v) = self.variables.get_mut(&vid) {
                match lit {
                    Some(l) => {
                        v.values.insert(mode, l);
                    }
                    None => {
                        v.values.remove(&mode);
                        v.mode_extensions.remove(&mode);
                    }
                }
            }
        }
        self.variables.remove(id);
        if let Some(c) = self.collections.get_mut(&gone.collection) {
            c.variables.retain(|v| v != id);
        }
    }

    pub fn delete_variable(&mut self, id: &str) {
        if self.is_library_variable(id) {
            return;
        }
        self.remove_variable_inner(id);
    }

    /// Moves `id` to sit right before `before` (or to the end). When the two
    /// live in different folders, `id` joins the folder of `before`.
    pub fn move_variable(&mut self, id: &str, before: Option<&str>) {
        if self.is_library_variable(id) {
            return;
        }
        let Some(coll) = self.variables.get(id).map(|v| v.collection.clone()) else { return };
        if before == Some(id) {
            return;
        }
        let target_folder = before.and_then(|b| self.variables.get(b)).filter(|b| b.collection == coll).map(|b| split_name(&b.name).0.to_string());
        let Some(c) = self.collections.get_mut(&coll) else { return };
        c.variables.retain(|v| v != id);
        let pos = before.and_then(|b| c.variables.iter().position(|v| v == b)).unwrap_or(c.variables.len());
        c.variables.insert(pos, id.to_string());
        if let Some(folder) = target_folder {
            let leaf = split_name(&self.variables[id].name).1.to_string();
            let full = if folder.is_empty() { leaf } else { format!("{folder}/{leaf}") };
            let full = self.unique_variable_name(&coll, &full, Some(id));
            if let Some(v) = self.variables.get_mut(id) {
                v.name = full;
            }
        }
    }

    /// Renames a folder: every variable of `coll` whose name starts with `old/`.
    pub fn rename_folder(&mut self, coll: &str, old: &str, new: &str) {
        if self.is_library_collection(coll) {
            return;
        }
        let new = new.trim().trim_matches('/');
        if new.is_empty() {
            return;
        }
        let ids: Vec<String> = self.collections.get(coll).map(|c| c.variables.clone()).unwrap_or_default();
        for id in ids {
            let Some(name) = self.variables.get(&id).map(|v| v.name.clone()) else { continue };
            let renamed = if name == old {
                new.to_string()
            } else if let Some(rest) = name.strip_prefix(&format!("{old}/")) {
                format!("{new}/{rest}")
            } else {
                continue;
            };
            if let Some(v) = self.variables.get_mut(&id) {
                v.name = renamed;
            }
        }
    }
}

/// Nodes (updated copies) whose bindings or explicit modes pointed at removed
/// variables or collections; commit them in the same batch as the deletion.
pub fn dangling_binding_cleanup(doc: &Document, removed_vars: &[String], removed_colls: &[String]) -> Vec<Node> {
    let mut out = Vec::new();
    for n in doc.nodes.values() {
        let bad_b = n.variables.bindings.values().any(|v| removed_vars.contains(v));
        let bad_m = n.variables.modes.keys().any(|c| removed_colls.contains(c));
        if bad_b || bad_m {
            let mut c = n.clone();
            c.variables.bindings.retain(|_, v| !removed_vars.contains(v));
            c.variables.modes.retain(|k, _| !removed_colls.contains(k));
            out.push(c);
        }
    }
    out
}

/// Writes the current resolved value of every binding that will disappear
/// into the node's raw properties, so detaching keeps the look.
pub fn bake_bindings(doc: &Document, node: &mut Node, keys: &[String]) {
    for k in keys {
        if let Some(v) = doc.bound_value(node.id, k) {
            set_bound_property(node, k, &v);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn store() -> (VariableStore, String, String) {
        let mut s = VariableStore::default();
        let c = s.add_collection("Theme");
        let m2 = s.add_mode(&c, "Dark").unwrap();
        let _ = m2;
        let a = s.add_variable(&c, "Color/Brand/Primary", VariableKind::Color).unwrap();
        (s, c, a)
    }

    #[test]
    fn collections_modes_and_variables() {
        let (mut s, c, a) = store();
        assert_eq!(s.collections[&c].modes.len(), 2);
        assert_eq!(s.variables[&a].values.len(), 2);
        let b = s.add_variable(&c, "Color/Brand/Primary", VariableKind::Color).unwrap();
        assert_eq!(s.variables[&b].name, "Color/Brand/Primary 2");
        // a new mode copies the default mode's values
        s.set_value(&a, &s.collections[&c].default_mode.clone(), VariableValue::Color(Color::WHITE));
        let m3 = s.add_mode(&c, "High contrast").unwrap();
        assert_eq!(s.variables[&a].values[&m3], VariableValue::Color(Color::WHITE));
        // deleting a mode removes its values; the last mode stays
        assert!(s.delete_mode(&c, &m3));
        assert!(!s.variables[&a].values.contains_key(&m3));
        let only = s.add_collection("One");
        let only_mode = s.collections[&only].default_mode.clone();
        assert!(!s.delete_mode(&only, &only_mode));
        // default mode deletion hands over the default
        let d = s.collections[&c].default_mode.clone();
        assert!(s.delete_mode(&c, &d));
        assert_ne!(s.collections[&c].default_mode, d);
    }

    #[test]
    fn rename_duplicate_and_reorder() {
        let (mut s, c, a) = store();
        s.rename_variable(&a, "Color/Brand/Accent");
        assert_eq!(s.variables[&a].name, "Color/Brand/Accent");
        s.rename_variable(&a, "  ");
        assert_eq!(s.variables[&a].name, "Color/Brand/Accent");
        let b = s.add_variable(&c, "Space/Gap", VariableKind::Number).unwrap();
        let d = s.duplicate_variable(&a).unwrap();
        assert_eq!(s.collections[&c].variables, vec![a.clone(), d.clone(), b.clone()]);
        assert_ne!(s.variables[&d].name, s.variables[&a].name);
        // dropping `b` before `a` moves it into a's folder
        s.move_variable(&b, Some(&a));
        assert_eq!(s.collections[&c].variables[0], b);
        assert_eq!(s.variables[&b].name, "Color/Brand/Gap");
        s.move_variable(&b, None);
        assert_eq!(s.collections[&c].variables.last(), Some(&b));
        s.rename_folder(&c, "Color/Brand", "Palette");
        assert_eq!(s.variables[&a].name, "Palette/Accent");
        assert_eq!(s.variables[&b].name, "Palette/Gap");
    }

    #[test]
    fn aliases_types_and_cycles() {
        let (mut s, c, a) = store();
        let b = s.add_variable(&c, "Bg", VariableKind::Color).unwrap();
        let n = s.add_variable(&c, "Num", VariableKind::Number).unwrap();
        let mode = s.collections[&c].default_mode.clone();
        assert!(s.set_alias(&b, &mode, &a));
        assert!(!s.set_alias(&b, &mode, &n), "type mismatch");
        assert!(!s.set_alias(&a, &mode, &b), "cycle");
        assert!(!s.set_alias(&a, &mode, &a));
        assert!(!s.set_value(&b, &mode, VariableValue::Number(1.0)));
        s.set_value(&a, &mode, VariableValue::Color(Color::BLACK));
        assert_eq!(s.resolve(&b, &mode), Some(VariableValue::Color(Color::BLACK)));
        assert!(s.detach_alias(&b, &mode));
        assert_eq!(s.variables[&b].values[&mode], VariableValue::Color(Color::BLACK));
    }

    #[test]
    fn deleting_bakes_aliases_and_cleans_bindings() {
        let mut d = Document::new("t");
        let page = d.add_page("P");
        let rid = d.alloc_id();
        let rect = d.insert(page, Node::rectangle(rid, Rect::new(0.0, 0.0, 10.0, 10.0))).unwrap();
        let (mut s, c, a) = store();
        let b = s.add_variable(&c, "Bg", VariableKind::Color).unwrap();
        let mode = s.collections[&c].default_mode.clone();
        s.set_value(&a, &mode, VariableValue::Color(Color::WHITE));
        s.set_alias(&b, &mode, &a);
        d.get_mut(rect).unwrap().variables.bindings.insert("fills/0".into(), a.clone());
        d.get_mut(rect).unwrap().variables.modes.insert(c.clone(), mode.clone());
        let removed = s.delete_collection(&c);
        assert_eq!(removed.len(), 2);
        assert!(s.variables.is_empty() && !s.collections.contains_key(&c));
        let nodes = dangling_binding_cleanup(&d, &removed, std::slice::from_ref(&c));
        assert_eq!(nodes.len(), 1);
        assert!(nodes[0].variables.is_empty());

        // within a collection: deleting the target keeps dependants' look
        let (mut s, c, a) = store();
        let b = s.add_variable(&c, "Bg", VariableKind::Color).unwrap();
        let mode = s.collections[&c].default_mode.clone();
        s.set_value(&a, &mode, VariableValue::Color(Color::WHITE));
        s.set_alias(&b, &mode, &a);
        s.delete_variable(&a);
        assert_eq!(s.variables[&b].values[&mode], VariableValue::Color(Color::WHITE));
        assert_eq!(s.collections[&c].variables, vec![b]);
    }

    #[test]
    fn duplicate_collection_remaps_modes_and_aliases() {
        let (mut s, c, a) = store();
        let b = s.add_variable(&c, "Bg", VariableKind::Color).unwrap();
        let mode = s.collections[&c].default_mode.clone();
        s.set_alias(&b, &mode, &a);
        let c2 = s.duplicate_collection(&c).unwrap();
        let coll2 = &s.collections[&c2];
        assert_eq!(coll2.modes.len(), 2);
        assert!(coll2.modes.iter().all(|m| !s.collections[&c].modes.iter().any(|o| o.id == m.id)));
        assert!(coll2.modes.iter().any(|m| m.id == coll2.default_mode));
        let b2 = &s.variables[&coll2.variables[1]];
        let a2 = &coll2.variables[0];
        assert_eq!(b2.values[&coll2.default_mode], VariableValue::Alias(a2.clone()));
        // and the edit installs as one undoable command
        let mut d = Document::new("t");
        let mut h = History::default();
        h.exec(&mut d, s.clone().command()).unwrap();
        assert_eq!(d.variables.len(), 4);
        h.undo(&mut d).unwrap();
        assert!(d.variables.is_empty());
    }

    /// A store with a local `Theme` and a subscribed-library `Theme`, each holding one color variable.
    fn local_and_library() -> (VariableStore, (String, String), (String, String)) {
        let (mut s, local, local_var) = store();
        let lib = s.add_collection("Theme");
        let lib_var = s.add_variable(&lib, "Color/Text", VariableKind::Color).unwrap();
        s.collections.get_mut(&lib).unwrap().source_library_key = Some("lk-83c0".into());
        (s, (local, local_var), (lib, lib_var))
    }

    #[test]
    fn library_collections_ignore_every_edit() {
        let (mut s, (local, local_var), (lib, lib_var)) = local_and_library();
        let before = s.clone();
        let mode = s.collections[&lib].default_mode.clone();
        s.rename_collection(&lib, "Mine");
        assert!(s.add_mode(&lib, "Dark").is_none());
        s.rename_mode(&lib, &mode, "Renamed");
        assert!(s.duplicate_mode(&lib, &mode).is_none());
        assert!(!s.delete_mode(&lib, &mode));
        s.set_default_mode(&lib, &mode);
        assert!(s.add_variable(&lib, "New", VariableKind::Number).is_none());
        s.rename_variable(&lib_var, "Renamed");
        assert!(s.duplicate_variable(&lib_var).is_none());
        assert!(!s.set_value(&lib_var, &mode, VariableValue::Color(Color::WHITE)));
        assert!(!s.set_alias(&lib_var, &mode, &local_var));
        assert!(!s.detach_alias(&lib_var, &mode));
        s.move_variable(&lib_var, None);
        s.rename_folder(&lib, "Color", "Paint");
        s.delete_variable(&lib_var);
        assert!(s.delete_collection(&lib).is_empty());
        assert_eq!(s, before, "nothing about the library collection changed");
        // the local collection of the same name still edits
        assert!(s.set_value(&local_var, &s.collections[&local].default_mode.clone(), VariableValue::Color(Color::WHITE)));
    }

    #[test]
    fn a_local_variable_may_alias_a_library_variable() {
        let (mut s, (local, local_var), (_, lib_var)) = local_and_library();
        let mode = s.collections[&local].default_mode.clone();
        assert!(s.set_alias(&local_var, &mode, &lib_var));
        assert_eq!(s.variables[&local_var].values[&mode], VariableValue::Alias(lib_var));
    }

    #[test]
    fn a_duplicate_of_a_library_collection_is_a_local_editable_copy() {
        let (mut s, _, (lib, _)) = local_and_library();
        let copy = s.duplicate_collection(&lib).expect("a library collection can be duplicated");
        assert!(!s.collections[&copy].is_library());
        let var = s.collections[&copy].variables[0].clone();
        assert!(s.set_value(&var, &s.collections[&copy].default_mode.clone(), VariableValue::Color(Color::WHITE)));
    }

    #[test]
    fn enforce_library_read_only_reverts_direct_pokes() {
        let (original, _, (lib, lib_var)) = local_and_library();
        let mut work = original.clone();
        work.variables.get_mut(&lib_var).unwrap().name = "Hacked".into();
        work.collections.get_mut(&lib).unwrap().name = "Hacked".into();
        let ghost = "VariableID:ghost".to_string();
        work.variables.insert(ghost.clone(), Variable { id: ghost.clone(), collection: lib.clone(), ..original.variables[&lib_var].clone() });
        work.collections.get_mut(&lib).unwrap().variables.push(ghost);
        work.enforce_library_read_only(&original);
        assert_eq!(work, original);
        // a library collection removed outright comes back too
        let mut gone = original.clone();
        gone.delete_collection(&lib);
        gone.enforce_library_read_only(&original);
        assert_eq!(gone, original);
        // edits to local collections pass through
        let mut local_edit = original.clone();
        let local_var = original.variables.keys().find(|k| **k != lib_var).unwrap().clone();
        local_edit.variables.get_mut(&local_var).unwrap().name = "Renamed".into();
        let expected = local_edit.clone();
        local_edit.enforce_library_read_only(&original);
        assert_eq!(local_edit, expected);
    }

    #[test]
    fn enforce_library_read_only_restores_a_local_variable_pointed_at_a_library_collection() {
        let (original, (_, local_var), (lib, _)) = local_and_library();
        let mut work = original.clone();
        work.variables.get_mut(&local_var).unwrap().collection = lib;
        work.enforce_library_read_only(&original);
        assert_eq!(work, original, "the local variable is back where it was, not dropped");
    }

    #[test]
    fn enforce_library_read_only_unlinks_library_variables_from_local_collections() {
        let (original, (local, _), (_, lib_var)) = local_and_library();
        let mut work = original.clone();
        work.collections.get_mut(&local).unwrap().variables.push(lib_var);
        work.enforce_library_read_only(&original);
        assert_eq!(work, original);
    }

    #[test]
    fn deleting_a_collection_never_removes_a_library_variable_its_list_names() {
        let (mut s, (local, local_var), (lib, lib_var)) = local_and_library();
        let mode = s.collections[&local].default_mode.clone();
        assert!(s.set_alias(&local_var, &mode, &lib_var));
        let other = s.add_collection("Other");
        s.collections.get_mut(&other).unwrap().variables.push(lib_var.clone());
        let library_before = (s.collections[&lib].clone(), s.variables[&lib_var].clone());
        assert!(s.delete_collection(&other).is_empty(), "nothing of the library was removed");
        assert!(!s.collections.contains_key(&other));
        assert_eq!((s.collections[&lib].clone(), s.variables[&lib_var].clone()), library_before);
        assert_eq!(s.variables[&local_var].values[&mode], VariableValue::Alias(lib_var), "the alias was not baked into a literal");
    }
}
