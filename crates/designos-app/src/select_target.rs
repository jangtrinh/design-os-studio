//! The one `--select` resolver, shared by the window, `--screenshot` and the headless agent CLI (#1127).
//!
//! Grammar (the prefixes are case-sensitive): `none` selects nothing on purpose, `id:<n>` is a node id, `fig:<id>`
//! is a Figma id (`Node::source_id`), `name:<text>` is a layer name taken literally, and anything else is a layer
//! name. A name is looked up as a page, then a page child (screen), then any layer in page-walk order, then
//! the same walk ignoring case. A bare number that is both a layer name and a node id is an error. One that names
//! no layer is a node id, refused when that id is at or above the load watermark. `id:` never expands deferred
//! pages; `fig:` does so once only under 10,000 nodes; a hit never does. A miss expands them once and retries the name.
use designos_doc::{Document, NodeId};
mod error;
mod miss;
mod resolve;
pub use error::SelectError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum By {
    Id,
    Figma,
    Name,
    NameCaseInsensitive,
    BareId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Target {
    pub node: Option<NodeId>,
    pub page: Option<NodeId>,
    pub matched_by: By,
    /// Layers that matched: 1, unless several share the name.
    pub ambiguous: usize,
    /// False when the id was allocated at or after the load watermark (a name or `fig:` hit on expanded content).
    pub id_stable: bool,
}

impl Target {
    /// What to print on stderr when the selector was resolved by a rule a script may not have meant: a bare number
    /// read as a node id, or a name shared by several layers.
    pub fn note(&self, raw: &str) -> Option<String> {
        let id = self.node?.0;
        match self.matched_by {
            By::BareId => Some(format!("--select {raw} matched no layer name; selected node id {id} (use --select id:{id})")),
            By::Name | By::NameCaseInsensitive if self.ambiguous > 1 => {
                Some(format!("--select \"{raw}\" matched {} layers; selected id {id} (first in page order; use id:<n> for another)", self.ambiguous))
            }
            _ => None,
        }
    }
}

/// Resolves `raw`. `load_next_id` is [`Document::peek_next_id`] from just after the file was loaded, before any
/// page was expanded. A hit does not expand pages. A name miss expands them once and retries the name only.
pub fn resolve(doc: &mut Document, raw: &str, load_next_id: NodeId) -> Result<Target, SelectError> {
    let s = raw.trim();
    match resolve::parse(s)? {
        resolve::Sel::Id(id) => resolve::resolve_id(doc, s, id, load_next_id),
        resolve::Sel::Figma(want) => resolve::resolve_fig(doc, s, want, load_next_id),
        resolve::Sel::Name(v) => resolve::resolve_name(doc, s, v, load_next_id),
        resolve::Sel::Bare(v) => resolve::resolve_bare(doc, s, v, load_next_id),
    }
}

/// `--select`'s value as the command line hands it over: absent or `none` is no target at all.
pub fn resolve_flag(doc: &mut Document, select: Option<&str>, load_next_id: NodeId) -> Result<Option<Target>, SelectError> {
    match select {
        None => Ok(None),
        Some(s) if s.trim().eq_ignore_ascii_case("none") => Ok(None),
        Some(s) => resolve(doc, s, load_next_id).map(Some),
    }
}

#[cfg(test)]
mod tests;
#[cfg(test)]
mod tests_edges;
