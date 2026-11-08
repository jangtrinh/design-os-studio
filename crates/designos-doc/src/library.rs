//! Shared libraries: what a file publishes, which libraries it uses, and the
//! local copies of library assets its instances depend on.
//!
//! Library components used by a file are copied into a hidden page
//! ([`Document::library_page`]) that is not listed in [`Document::pages`], so
//! instance sync, rendering and import work exactly as for local components.

use crate::NodeId;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum LibraryAssetKind {
    Component,
    ComponentSet,
    Style,
    Variable,
    VariableCollection,
}

/// A library enabled in this file, pinned to the version last accepted.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LibraryRef {
    pub id: String,
    pub name: String,
    pub version: u32,
}

/// Local copy of one library asset used by this file.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ExternalAsset {
    pub library_id: String,
    /// Library version the copy was taken from.
    pub version: u32,
    pub kind: LibraryAssetKind,
    /// Root node of the copy on the library page (components and sets).
    pub node: Option<NodeId>,
    /// Key of the copy in `styles`, `variables` or `variable_collections`.
    pub local_key: Option<String>,
}

/// Set when this file is published as a library.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct PublishInfo {
    pub library_id: String,
    pub name: String,
    pub version: u32,
    pub description: String,
    /// RFC 3339 timestamp of the last publish.
    pub published_at: String,
}

/// All library bookkeeping of a document; empty for most files.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct LibraryState {
    pub published: Option<PublishInfo>,
    pub enabled: Vec<LibraryRef>,
    /// Asset key (component key, style key or variable key) -> local copy.
    pub external: BTreeMap<String, ExternalAsset>,
    /// For a component copy that carries Motion, by the key it has in `external`: the id each timeline and track has in
    /// the library file, to the id the local copy gave it. An update reuses them, so a reference to a local timeline
    /// survives it. Absent (and not written) for files with no library Motion; a file without it updates as before.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub motion_ids: BTreeMap<String, BTreeMap<String, String>>,
}

impl LibraryState {
    pub fn is_empty(&self) -> bool {
        self.published.is_none() && self.enabled.is_empty() && self.external.is_empty() && self.motion_ids.is_empty()
    }
}
