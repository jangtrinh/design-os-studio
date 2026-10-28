//! Why a `--select` resolved to no node, and how each case reads on stderr and in a toast.

use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SelectError {
    /// `--select ""`, or a prefix with nothing after it.
    Empty,
    /// `id:` followed by something that is not a whole number.
    BadId(String),
    /// Nothing matched; `searched` says where it looked.
    NoMatch { raw: String, searched: String },
    /// Several nodes carry the Figma id, which is meant to be unique.
    Ambiguous { raw: String, count: usize, ids: String },
    /// A bare number is both a layer name and a node id. `layer` is the name hit, `id` the node. An `id_stable: false`
    /// node came from expanded content, so `id:` would be refused and the hint offers only the name.
    NameAndId { raw: String, layer: u64, id: u64, id_stable: bool },
    /// `id:` or a bare id of a node that exists above the load watermark. Those ids change with what was expanded. An id
    /// no node has is a [`Self::NoMatch`].
    Unstable { id: u64 },
}

impl fmt::Display for SelectError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => write!(f, "--select needs a layer name, id:<n> or fig:<figma id>"),
            Self::BadId(raw) => write!(f, "--select \"{raw}\" is not a node id; use id: with a whole number, like id:5"),
            Self::NoMatch { raw, searched } => write!(f, "--select \"{raw}\" matched no node {searched}"),
            Self::Ambiguous { raw, count, ids } => {
                write!(f, "--select \"{raw}\" is ambiguous: {count} nodes have that Figma id (ids: {ids}; use id:<n> to pick one)")
            }
            Self::NameAndId { raw, layer, id, id_stable: true } => {
                write!(f, "--select {raw} is ambiguous: layer \"{raw}\" (id {layer}) and node id {id}; use id:{id} or name:{raw}")
            }
            Self::NameAndId { raw, layer, id, id_stable: false } => {
                write!(f, "--select {raw} is ambiguous: layer \"{raw}\" (id {layer}) and node id {id}, which is per session and not stable across runs; use name:{raw}")
            }
            Self::Unstable { id } => {
                write!(f, "id {id} belongs to an expanded instance and is not stable across runs; use fig:<id> or a name")
            }
        }
    }
}

impl From<SelectError> for String {
    fn from(e: SelectError) -> String {
        e.to_string()
    }
}

impl SelectError {
    /// The short text for the toast of an interactive launch.
    pub fn toast(&self) -> String {
        match self {
            Self::NoMatch { raw, .. } => format!("No layer matches \"{raw}\". Check the name, or use id: or fig:"),
            other => other.to_string(),
        }
    }
}
