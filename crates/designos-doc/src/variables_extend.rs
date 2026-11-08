//! Extended variable collections: a collection that inherits every variable of a parent collection
//! (in this file or in a library) and overrides single values per mode.
//!
//! The link lives in the collection's `extensions` under [`EXTENSION_KEY`], so undo, branching, publishing and the
//! document file format carry it with no new field. Readers on the resolution path look into that JSON without
//! building the typed [`CollectionExtension`].

use crate::*;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

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

    /// The parent mode shown for own `mode` when nothing overrides it.
    fn extension_parent_mode(&self, mode: &str) -> Option<&str> {
        self.extensions.get(EXTENSION_KEY)?.get("mode_map")?.get(mode)?.as_str()
    }

    /// The value this extension overrides `variable` with in `mode`.
    pub fn extension_override(&self, variable: &str, mode: &str) -> Option<VariableValue> {
        let v = self.extensions.get(EXTENSION_KEY)?.get("overrides")?.get(variable)?.get(mode)?;
        serde_json::from_value(v.clone()).ok()
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
        let parent_id = c.extension_parent_id()?;
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
        match collections.get(cur).and_then(VariableCollection::extension_parent_id) {
            Some(p) => cur = p,
            None => return false,
        }
    }
    false
}

impl Document {
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
        if !self.variable_collections.values().any(VariableCollection::is_extension) {
            return None;
        }
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
        let Some(parent) = c.extension_parent_id() else {
            return c.variables.iter().filter(|id| variables.get(*id).is_some_and(|v| !v.is_soft_deleted())).cloned().collect();
        };
        cur = parent;
    }
    Vec::new()
}

impl VariableStore {
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
        ext.set_extension(&CollectionExtension { parent: ParentCollection { collection: parent.to_string(), set_key: parent_key }, mode_map, overrides: BTreeMap::new() });
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
        let Some(c) = self.collections.get_mut(ext) else { return false };
        let Some(mut e) = c.extension() else { return false };
        e.overrides.entry(variable.to_string()).or_default().insert(mode.to_string(), value);
        c.set_extension(&e);
        true
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

    fn can_override(&self, ext: &str, variable: &str, mode: &str) -> bool {
        let Some(c) = self.collections.get(ext).filter(|c| c.is_extension() && !c.is_library()) else { return false };
        c.modes.iter().any(|m| m.id == mode) && self.inherited_variable_ids(ext).iter().any(|v| v == variable)
    }

    /// Drops the overrides of `variable`, and the overrides that alias it, from every extended collection.
    pub(crate) fn forget_overrides_of(&mut self, variable: &str) {
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

    /// Drops the overrides stored for `mode` in the extended collection `coll`, and the mode mapping entry.
    pub(crate) fn forget_override_mode(&mut self, coll: &str, mode: &str) {
        let Some(c) = self.collections.get_mut(coll) else { return };
        let Some(mut e) = c.extension() else { return };
        for modes in e.overrides.values_mut() {
            modes.remove(mode);
        }
        e.overrides.retain(|_, m| !m.is_empty());
        e.mode_map.remove(mode);
        c.set_extension(&e);
    }

    /// Removes every collection that extends `parent` (a deleted parent leaves nothing to inherit from).
    pub(crate) fn remove_extensions_of(&mut self, parent: &str) -> Vec<String> {
        let gone: Vec<String> =
            self.collections.values().filter(|c| c.id != parent && c.is_extension() && extends(&self.collections, &c.id, parent)).map(|c| c.id.clone()).collect();
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
        assert!(s.set_value(&bg, &s.collections[&brand].extension().unwrap().mode_map[&s.collections[&brand].modes[0].id].clone(), VariableValue::Color(Color::rgb(1, 2, 3))));
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
}
