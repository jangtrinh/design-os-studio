//! Variables (design tokens) with collections and modes, as in Figma.
//!
//! A node binds a property to a variable through [`NodeVariables::bindings`],
//! keyed by Figma's `boundVariables` field names ("fills/0", "width",
//! "itemSpacing", "topLeftRadius", "visible", "characters", ...). The value used
//! for a node comes from the mode chosen on the nearest ancestor (or the node
//! itself) in [`NodeVariables::modes`], else the collection's default mode.

use crate::{Color, Document, NodeId};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Verbatim Figma extension keys (`key -> JSON value`) (`E1-9` §4).
pub type FigmaExtensions = BTreeMap<String, serde_json::Value>;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct VariableMode {
    pub id: String,
    pub name: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct VariableCollection {
    pub id: String,
    pub name: String,
    pub modes: Vec<VariableMode>,
    pub default_mode: String,
    /// Variable ids in display order.
    pub variables: Vec<String>,
    /// Figma library key, when imported from a published library.
    pub key: Option<String>,
    /// Key of the shared library file this collection is a subscribed copy of (Figma's
    /// `sourceLibraryKey`). `None` for a collection the file owns.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_library_key: Option<String>,
    /// Collection-level Figma keys (`com.figma.parentVariableSetId`,
    /// `com.figma.parentVariableSetName`, etc.) (`E1-9` §4).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub extensions: FigmaExtensions,
}

impl VariableCollection {
    /// True for a subscribed copy of a shared library's collection. Such a collection is edited in
    /// its library file and reaches this one through a library update.
    pub fn is_library(&self) -> bool {
        self.source_library_key.is_some()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum VariableKind {
    Color,
    Number,
    String,
    Boolean,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum VariableValue {
    Color(Color),
    Number(f32),
    String(String),
    Boolean(bool),
    /// Another variable's id; resolved in the same mode context.
    Alias(String),
}

/// Platform a variable's code syntax is written for. Serialized with Figma's labels
/// (`WEB`, `ANDROID`, `iOS`), as in the REST `codeSyntax` object.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum CodeSyntaxPlatform {
    #[serde(rename = "WEB")]
    Web,
    #[serde(rename = "ANDROID")]
    Android,
    #[serde(rename = "iOS")]
    Ios,
}

impl CodeSyntaxPlatform {
    pub const ALL: [CodeSyntaxPlatform; 3] = [Self::Web, Self::Android, Self::Ios];

    /// Figma's label: `WEB`, `ANDROID` or `iOS`.
    pub fn label(self) -> &'static str {
        match self {
            Self::Web => "WEB",
            Self::Android => "ANDROID",
            Self::Ios => "iOS",
        }
    }

    pub fn from_label(s: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|p| p.label() == s)
    }
}

/// Per-platform code a variable prints as in Dev Mode (`var(--color-primary)`, `Color.primary`, ...).
pub type CodeSyntax = BTreeMap<CodeSyntaxPlatform, String>;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Variable {
    pub id: String,
    /// May contain "/" for grouping ("Color/Brand/Primary").
    pub name: String,
    pub collection: String,
    pub kind: VariableKind,
    /// Value per mode id.
    pub values: BTreeMap<String, VariableValue>,
    pub description: String,
    /// Figma scopes ("ALL_SCOPES", "CORNER_RADIUS", "GAP", ...); empty = all.
    pub scopes: Vec<String>,
    pub hidden_from_publishing: bool,
    pub key: Option<String>,
    /// Code written per platform; an empty string is not stored.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub code_syntax: CodeSyntax,
    /// Variable-level extension keys (e.g. `org.designos.figmaTokenType`: `"easing"` or `"duration"`) (`E1-9` §4).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub extensions: FigmaExtensions,
    /// Per-mode Figma extension keys: outer key = mode id, inner map = `com.figma.*` keys for that mode (`E1-9` §4).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub mode_extensions: BTreeMap<String, FigmaExtensions>,
}

impl Variable {
    /// The code syntax for `platform`, when one is set.
    pub fn code_syntax_for(&self, platform: CodeSyntaxPlatform) -> Option<&str> {
        self.code_syntax.get(&platform).map(String::as_str).filter(|s| !s.is_empty())
    }

    pub fn matches_scope(&self, target_scope: &str) -> bool {
        self.scopes.is_empty()
            || target_scope.is_empty()
            || self.scopes.iter().any(|s| {
                s == "ALL_SCOPES"
                    || s.eq_ignore_ascii_case(target_scope)
                    || (s == "ALL_FILLS" && matches!(target_scope, "FRAME_FILL" | "SHAPE_FILL" | "TEXT_FILL" | "FILL"))
            })
    }

    pub fn is_publishable(&self) -> bool {
        !self.hidden_from_publishing
    }

    /// Clears per-mode extension keys that describe how the previous literal value was derived
    /// (`com.figma.aliasData` and `com.figma.composedColor`), keeping `com.figma.paint`,
    /// `com.figma.paintAliasData`, `com.figma.isOverride`, and any other keys (`E1-9` §4).
    pub fn clear_derived_mode_extensions(&mut self, mode: &str) {
        let mut remove_mode = false;
        if let Some(ext) = self.mode_extensions.get_mut(mode) {
            ext.remove("com.figma.aliasData");
            ext.remove("com.figma.composedColor");
            remove_mode = ext.is_empty();
        }
        if remove_mode {
            self.mode_extensions.remove(mode);
        }
    }
}

/// Per-node variable bindings and explicit mode choices.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct NodeVariables {
    /// Property key (Figma `boundVariables` name) -> variable id.
    pub bindings: BTreeMap<String, String>,
    /// Collection id -> mode id, inherited by descendants.
    pub modes: BTreeMap<String, String>,
}

impl NodeVariables {
    pub fn is_empty(&self) -> bool {
        self.bindings.is_empty() && self.modes.is_empty()
    }
}

/// Maximum alias chain length followed before giving up (cycles).
const MAX_ALIAS_DEPTH: usize = 16;

impl Document {
    /// Mode in effect for `collection` at `node`: the nearest explicit choice
    /// on the node or an ancestor, else the collection default.
    pub fn variable_mode(&self, node: NodeId, collection: &str) -> Option<String> {
        let coll = self.variable_collections.get(collection)?;
        if coll.modes.len() <= 1 {
            return Some(coll.default_mode.clone());
        }
        let mut cur = Some(node);
        while let Some(id) = cur {
            let n = self.get(id)?;
            if let Some(m) = n.variables.modes.get(collection) {
                return Some(m.clone());
            }
            cur = n.parent;
        }
        Some(coll.default_mode.clone())
    }

    /// Resolves a variable to a concrete value as seen from `node`, following
    /// aliases (each alias resolves in its own collection's mode at `node`).
    pub fn resolve_variable(&self, node: NodeId, variable: &str) -> Option<VariableValue> {
        let mut id = variable.to_string();
        for _ in 0..MAX_ALIAS_DEPTH {
            let v = self.variables.get(&id)?;
            let mode = self.variable_mode(node, &v.collection)?;
            let coll = self.variable_collections.get(&v.collection)?;
            let value = v.values.get(&mode).or_else(|| v.values.get(&coll.default_mode))?;
            match value {
                VariableValue::Alias(next) => id = next.clone(),
                other => return Some(other.clone()),
            }
        }
        None
    }

    /// Resolves the variable bound to `property` on `node`, if any.
    pub fn bound_value(&self, node: NodeId, property: &str) -> Option<VariableValue> {
        let var = self.get(node)?.variables.bindings.get(property)?;
        self.resolve_variable(node, var)
    }
}

#[cfg(test)]
mod tests {
    use crate::*;

    fn setup() -> (Document, NodeId, NodeId) {
        let mut d = Document::new("t");
        let page = d.add_page("P");
        let fid = d.alloc_id();
        let frame = d.insert(page, Node::frame(fid, Rect::new(0.0, 0.0, 100.0, 100.0))).unwrap();
        let rid = d.alloc_id();
        let rect = d.insert(frame, Node::rectangle(rid, Rect::new(0.0, 0.0, 10.0, 10.0))).unwrap();
        d.variable_collections.insert(
            "c1".into(),
            VariableCollection {
                id: "c1".into(),
                name: "Theme".into(),
                modes: vec![VariableMode { id: "light".into(), name: "Light".into() }, VariableMode { id: "dark".into(), name: "Dark".into() }],
                default_mode: "light".into(),
                variables: vec!["base".into(), "bg".into()],
                key: None,
                source_library_key: None,
                extensions: Default::default(),
            },
        );
        let var = |id: &str, values: Vec<(&str, VariableValue)>| Variable {
            id: id.into(),
            name: id.into(),
            collection: "c1".into(),
            kind: VariableKind::Color,
            values: values.into_iter().map(|(k, v)| (k.to_string(), v)).collect(),
            description: String::new(),
            scopes: vec![],
            hidden_from_publishing: false,
            key: None,
            code_syntax: Default::default(),
            extensions: Default::default(),
            mode_extensions: Default::default(),
        };
        d.variables.insert("base".into(), var("base", vec![("light", VariableValue::Color(Color::WHITE)), ("dark", VariableValue::Color(Color::BLACK))]));
        d.variables.insert("bg".into(), var("bg", vec![("light", VariableValue::Alias("base".into())), ("dark", VariableValue::Alias("base".into()))]));
        d.get_mut(rect).unwrap().variables.bindings.insert("fills/0".into(), "bg".into());
        (d, frame, rect)
    }

    #[test]
    fn resolves_default_mode_and_alias() {
        let (d, _, rect) = setup();
        assert_eq!(d.bound_value(rect, "fills/0"), Some(VariableValue::Color(Color::WHITE)));
    }

    #[test]
    fn ancestor_mode_is_inherited() {
        let (mut d, frame, rect) = setup();
        d.get_mut(frame).unwrap().variables.modes.insert("c1".into(), "dark".into());
        assert_eq!(d.bound_value(rect, "fills/0"), Some(VariableValue::Color(Color::BLACK)));
    }

    #[test]
    fn alias_cycle_gives_none() {
        let (mut d, _, rect) = setup();
        d.variables.get_mut("base").unwrap().values.insert("light".into(), VariableValue::Alias("bg".into()));
        assert_eq!(d.bound_value(rect, "fills/0"), None);
    }

    #[test]
    fn set_variables_undo() {
        let (mut d, _, _) = setup();
        let mut h = History::default();
        h.exec(&mut d, Command::SetVariables { variables: Default::default(), collections: Default::default() }).unwrap();
        assert!(d.variables.is_empty());
        h.undo(&mut d).unwrap();
        assert_eq!(d.variables.len(), 2);
        assert_eq!(d.variable_collections.len(), 1);
    }
}
