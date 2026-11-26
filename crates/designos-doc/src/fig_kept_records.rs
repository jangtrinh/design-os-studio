//! The value of each style, variable and variable set a `.fig` open kept a record for, as it was when the open finished.
//!
//! A layer's value is snapshotted node by node (see [`crate::NodeSnapshot`]). A style, a variable and a variable set are not
//! nodes: each is an entry of the document's tables, and a save writes its record from what it reads there. The snapshot is the
//! hash of everything the `.fig` writer reads for one entry, so a later save can tell an entry nobody touched from one that was
//! edited. It is a value, not a flag: undo puts the entry back and the hash matches again.
//!
//! - A **style** record is written from the style, its metadata (name, description, key, order, the variables it binds) and, for
//!   a style with no paints of its own, the values of the variables it binds.
//! - A **variable** record is written from the variable, and from the overrides an extending collection holds for it.
//! - A **variable set** record is written from the collection, except the list of its variables: adding a variable to a
//!   collection does not change the collection's own record.
//!
//! The snapshot is runtime-only, like the store it belongs to. It is serializable so that whatever later carries the store
//! through a native save carries it too: the values are plain 64-bit hashes and read back exactly.

use crate::branch_diff::StableHasher;
use crate::{Document, NodeId, Style, StyleMeta, Variable, VariableCollection};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// The values of the entries a record was kept for, by the id the document gives the entry.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecordSnapshots {
    styles: BTreeMap<String, u64>,
    variables: BTreeMap<String, u64>,
    collections: BTreeMap<String, u64>,
}

impl RecordSnapshots {
    /// How many entries have a snapshot, as (styles, variables, collections).
    pub fn counts(&self) -> (usize, usize, usize) {
        (self.styles.len(), self.variables.len(), self.collections.len())
    }

    pub fn is_empty(&self) -> bool {
        self.styles.is_empty() && self.variables.is_empty() && self.collections.is_empty()
    }
}

/// The guid an id of the document names: `"s:l"` for a style, `"VariableID:s:l"` for a variable, `"VariableCollectionId:s:l"` for a
/// collection. Any other id (one made in Studio, or a library key) names no record.
fn guid_named(id: &str) -> Option<(u32, u32)> {
    let digits = id.strip_prefix("VariableID:").or_else(|| id.strip_prefix("VariableCollectionId:")).unwrap_or(id);
    let (session, local) = digits.split_once(':')?;
    let number = |s: &str| (!s.is_empty() && s.bytes().all(|b| b.is_ascii_digit())).then(|| s.parse::<u32>().ok()).flatten();
    Some((number(session)?, number(local)?))
}

/// The value of one style, as the writer reads it.
fn style_hash(doc: &Document, style: &Style, meta: Option<&StyleMeta>) -> u64 {
    let mut h = StableHasher::new();
    h.add(style);
    h.add(&meta);
    // A style with no paints of its own is written with the colour of the variable it binds, so that colour is part of its value.
    if let Some(meta) = meta {
        for var in meta.bindings.values() {
            h.add(&doc.resolve_variable(NodeId(0), var));
        }
    }
    h.finish()
}

/// What the extending collections of `doc` override, as typed extensions.
fn extensions_of(doc: &Document) -> Vec<crate::CollectionExtension> {
    doc.variable_collections.values().filter(|c| c.is_extension()).filter_map(VariableCollection::extension).collect()
}

/// The value of one variable, as the writer reads it.
fn variable_hash(var: &Variable, extensions: &[crate::CollectionExtension]) -> u64 {
    let mut h = StableHasher::new();
    h.add(var);
    for ext in extensions {
        h.add(&ext.overrides.get(&var.id));
    }
    h.finish()
}

/// The value of one collection, as the writer reads it: everything but the list of its variables.
fn collection_hash(c: &VariableCollection) -> u64 {
    let mut h = StableHasher::new();
    h.add(&c.id);
    h.add(&c.name);
    h.add(&c.modes);
    h.add(&c.default_mode);
    h.add(&c.key);
    h.add(&c.source_library_key);
    h.add(&c.extensions);
    h.finish()
}

impl Document {
    /// Takes the value of every style, variable and variable set the store has a record for, replacing the snapshots there were.
    /// The import calls it together with [`Self::take_fig_kept_snapshot`].
    pub(crate) fn take_fig_kept_record_snapshots(&mut self) {
        let store = self.fig_kept.clone();
        let has_record = |id: &str| guid_named(id).is_some_and(|g| store.record_by_guid(g).is_some());
        let mut out = RecordSnapshots::default();
        for (id, style) in &self.styles {
            if has_record(id) {
                out.styles.insert(id.clone(), style_hash(self, style, self.style_meta.get(id)));
            }
        }
        let extensions = extensions_of(self);
        for (id, var) in &self.variables {
            if has_record(id) {
                out.variables.insert(id.clone(), variable_hash(var, &extensions));
            }
        }
        for (id, c) in &self.variable_collections {
            if has_record(id) {
                out.collections.insert(id.clone(), collection_hash(c));
            }
        }
        self.fig_kept_records = std::sync::Arc::new(out);
    }

    /// The snapshots of the entries a record was kept for.
    pub fn fig_kept_record_snapshots(&self) -> &RecordSnapshots {
        &self.fig_kept_records
    }

    /// Whether style `id` still has the value its snapshot took: `Some(true)` for an unedited style, `Some(false)` for an edited
    /// one, `None` when no record was kept for it.
    pub fn fig_kept_style_matches_snapshot(&self, id: &str) -> Option<bool> {
        let was = *self.fig_kept_records.styles.get(id)?;
        let style = self.styles.get(id)?;
        Some(style_hash(self, style, self.style_meta.get(id)) == was)
    }

    /// Whether variable `id` still has the value its snapshot took (see [`Self::fig_kept_style_matches_snapshot`]).
    pub fn fig_kept_variable_matches_snapshot(&self, id: &str) -> Option<bool> {
        let was = *self.fig_kept_records.variables.get(id)?;
        let var = self.variables.get(id)?;
        Some(variable_hash(var, &extensions_of(self)) == was)
    }

    /// Whether variable collection `id` still has the value its snapshot took (see [`Self::fig_kept_style_matches_snapshot`]).
    pub fn fig_kept_collection_matches_snapshot(&self, id: &str) -> Option<bool> {
        let was = *self.fig_kept_records.collections.get(id)?;
        Some(collection_hash(self.variable_collections.get(id)?) == was)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_id_names_a_guid_only_when_it_is_two_numbers() {
        assert_eq!(guid_named("12:34"), Some((12, 34)));
        assert_eq!(guid_named("VariableID:5:6"), Some((5, 6)));
        assert_eq!(guid_named("VariableCollectionId:7:8"), Some((7, 8)));
        assert_eq!(guid_named("S:abc"), None);
        assert_eq!(guid_named("key:abc"), None);
        assert_eq!(guid_named("1:2:3"), None);
        assert_eq!(guid_named("1:"), None);
        assert_eq!(guid_named("99999999999:1"), None);
    }

    use crate::{Color, FigKeptStore, KeptArena, KeptCounts, KeptData, KeptRecord, KeptRecordKind, Paint, VariableKind, VariableMode, VariableValue};
    use std::sync::Arc;

    fn variable(id: &str, collection: &str, value: VariableValue) -> Variable {
        Variable {
            id: id.into(),
            name: "Primary".into(),
            collection: collection.into(),
            kind: VariableKind::Color,
            values: BTreeMap::from([("1:9".to_string(), value)]),
            description: String::new(),
            scopes: Vec::new(),
            hidden_from_publishing: false,
            key: None,
            code_syntax: Default::default(),
            extensions: BTreeMap::new(),
            mode_extensions: BTreeMap::new(),
        }
    }

    /// A document with a style, a variable and a collection, and a store holding a record under each one's guid and a snapshot of all three.
    fn document_with_records() -> Document {
        let guids = [(1, 1), (1, 2), (1, 3)];
        let records: Vec<KeptRecord> = guids.iter().map(|g| KeptRecord::new(*g, KeptRecordKind::Other, 0, 0)).collect();
        let counts = KeptCounts { records_seen: 3, records_kept: 3, ..Default::default() };
        let data = KeptData::new(KeptArena::in_memory(vec![0; 8]), records, Vec::new(), Vec::new(), counts);
        let mut doc = Document::new("records");
        doc.styles.insert("1:1".into(), Style::Fill(vec![Paint::solid(Color::rgb(1, 2, 3))]));
        doc.style_meta.insert("1:1".into(), StyleMeta { name: "Brand".into(), ..Default::default() });
        doc.styles.insert("S:orphan".into(), Style::Fill(vec![Paint::solid(Color::rgb(9, 9, 9))]));
        doc.variable_collections.insert(
            "VariableCollectionId:1:3".into(),
            VariableCollection {
                id: "VariableCollectionId:1:3".into(),
                name: "Colors".into(),
                modes: vec![VariableMode { id: "1:9".into(), name: "Light".into() }],
                default_mode: "1:9".into(),
                variables: vec!["VariableID:1:2".into()],
                key: None,
                source_library_key: None,
                extensions: BTreeMap::new(),
            },
        );
        doc.variables.insert("VariableID:1:2".into(), variable("VariableID:1:2", "VariableCollectionId:1:3", VariableValue::Color(Color::rgb(4, 5, 6))));
        doc.fig_kept = Arc::new(FigKeptStore::new(data));
        doc.take_fig_kept_snapshot();
        doc
    }

    #[test]
    fn an_entry_nobody_touched_matches_its_snapshot_and_an_entry_with_no_record_has_none() {
        let doc = document_with_records();
        assert_eq!(doc.fig_kept_record_snapshots().counts(), (1, 1, 1));
        assert_eq!(doc.fig_kept_style_matches_snapshot("1:1"), Some(true));
        assert_eq!(doc.fig_kept_variable_matches_snapshot("VariableID:1:2"), Some(true));
        assert_eq!(doc.fig_kept_collection_matches_snapshot("VariableCollectionId:1:3"), Some(true));
        assert_eq!(doc.fig_kept_style_matches_snapshot("S:orphan"), None, "a style with no guid has no record");
        assert_eq!(doc.fig_kept_style_matches_snapshot("9:9"), None, "an id the document does not hold");
    }

    #[test]
    fn each_thing_the_writer_reads_for_an_entry_changes_its_value() {
        type Edit<T> = Vec<(&'static str, Box<dyn Fn(&mut T)>)>;
        let edits: Edit<Document> = vec![
            ("style paint", Box::new(|d| drop(d.styles.insert("1:1".into(), Style::Fill(vec![Paint::solid(Color::rgb(9, 9, 9))]))))),
            ("style name", Box::new(|d| d.style_meta.get_mut("1:1").expect("meta").name = "Other".into())),
            ("style description", Box::new(|d| d.style_meta.get_mut("1:1").expect("meta").description = "text".into())),
            ("style order", Box::new(|d| d.style_meta.get_mut("1:1").expect("meta").sort_position = Some("~".into()))),
            ("style binding", Box::new(|d| drop(d.style_meta.get_mut("1:1").expect("meta").bindings.insert("fills/0".into(), "VariableID:1:2".into())))),
        ];
        for (what, edit) in edits {
            let mut doc = document_with_records();
            edit(&mut doc);
            assert_eq!(doc.fig_kept_style_matches_snapshot("1:1"), Some(false), "{what}");
        }
        let variable_edits: Edit<Variable> = vec![
            ("variable value", Box::new(|v| drop(v.values.insert("1:9".into(), VariableValue::Color(Color::rgb(7, 7, 7)))))),
            ("variable removed mode value", Box::new(|v| drop(v.values.remove("1:9")))),
            ("variable name", Box::new(|v| v.name = "Other".into())),
            ("variable description", Box::new(|v| v.description = "text".into())),
            ("variable scopes", Box::new(|v| v.scopes.push("GAP".into()))),
            ("variable publishing", Box::new(|v| v.hidden_from_publishing = true)),
            ("variable code syntax", Box::new(|v| drop(v.code_syntax.insert(crate::CodeSyntaxPlatform::Web, "var(--x)".into())))),
        ];
        for (what, edit) in variable_edits {
            let mut doc = document_with_records();
            edit(doc.variables.get_mut("VariableID:1:2").expect("variable"));
            assert_eq!(doc.fig_kept_variable_matches_snapshot("VariableID:1:2"), Some(false), "{what}");
        }
        let collection_edits: Edit<VariableCollection> = vec![
            ("collection name", Box::new(|c| c.name = "Other".into())),
            ("collection modes", Box::new(|c| c.modes.push(VariableMode { id: "1:10".into(), name: "Dark".into() }))),
            ("collection default mode", Box::new(|c| c.default_mode = "1:10".into())),
            ("collection publishing", Box::new(|c| c.set_hidden_from_publishing(true))),
        ];
        for (what, edit) in collection_edits {
            let mut doc = document_with_records();
            edit(doc.variable_collections.get_mut("VariableCollectionId:1:3").expect("collection"));
            assert_eq!(doc.fig_kept_collection_matches_snapshot("VariableCollectionId:1:3"), Some(false), "{what}");
        }
    }

    #[test]
    fn a_bound_variable_that_changes_value_changes_the_style_that_binds_it_and_putting_it_back_restores_the_match() {
        let mut doc = document_with_records();
        doc.style_meta.get_mut("1:1").expect("meta").bindings.insert("fills/0".into(), "VariableID:1:2".into());
        doc.take_fig_kept_record_snapshots();
        assert_eq!(doc.fig_kept_style_matches_snapshot("1:1"), Some(true));
        let was = doc.variables["VariableID:1:2"].values["1:9"].clone();
        doc.variables.get_mut("VariableID:1:2").expect("variable").values.insert("1:9".into(), VariableValue::Color(Color::rgb(200, 0, 0)));
        assert_eq!(doc.fig_kept_style_matches_snapshot("1:1"), Some(false), "the style's colour is the variable's");
        doc.variables.get_mut("VariableID:1:2").expect("variable").values.insert("1:9".into(), was);
        assert_eq!(doc.fig_kept_style_matches_snapshot("1:1"), Some(true), "an undo puts the value back");
    }

    #[test]
    fn adding_a_variable_to_a_collection_does_not_change_the_collections_own_record() {
        let mut doc = document_with_records();
        let c = doc.variable_collections.get_mut("VariableCollectionId:1:3").expect("collection");
        c.variables.push("VariableID:1:77".into());
        assert_eq!(doc.fig_kept_collection_matches_snapshot("VariableCollectionId:1:3"), Some(true));
    }

    #[test]
    fn a_clone_shares_the_snapshots() {
        let doc = document_with_records();
        let copy = doc.clone();
        assert_eq!(copy.fig_kept_record_snapshots(), doc.fig_kept_record_snapshots());
        assert_eq!(copy.fig_kept_style_matches_snapshot("1:1"), Some(true));
    }

    #[test]
    fn a_snapshot_reads_back_equal_through_serde() {
        let mut s = RecordSnapshots::default();
        s.styles.insert("1:2".into(), u64::MAX);
        s.variables.insert("VariableID:3:4".into(), 0x0123_4567_89ab_cdef);
        s.collections.insert("VariableCollectionId:5:6".into(), 1);
        let text = serde_json::to_string(&s).expect("serialize");
        let back: RecordSnapshots = serde_json::from_str(&text).expect("deserialize");
        assert_eq!(back, s, "a 64-bit hash survives a text round trip");
        assert_eq!(back.counts(), (1, 1, 1));
    }
}
