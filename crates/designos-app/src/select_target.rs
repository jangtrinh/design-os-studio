//! The one `--select` resolver, shared by the window, `--screenshot` and the headless agent CLI (#1127).
//!
//! Grammar (the prefixes are case-sensitive): `none` selects nothing on purpose, `id:<n>` is a node id, `fig:<id>`
//! is a Figma id (`Node::source_id`), `name:<text>` is a layer name taken literally, and anything else is a layer
//! name. A name is looked up as a page, then a page child (screen), then any layer in page-walk order, then
//! the same walk ignoring case. A bare number that is both a layer name and a node id is an error. One that names
//! no layer is a node id, refused when that id is at or above the load watermark. `id:` never expands deferred
//! pages; `fig:` does so once only under 10,000 nodes; a hit never does. A miss expands them once and retries the name.
use designos_doc::{Document, Node, NodeId};
mod error;
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

enum Sel<'a> {
    Id(NodeId),
    Figma(&'a str),
    Name(&'a str),
    Bare(&'a str),
}

fn parse(s: &str) -> Result<Sel<'_>, SelectError> {
    let value = |v: &'static str| s.strip_prefix(v).map(|rest| if rest.is_empty() { Err(SelectError::Empty) } else { Ok(rest) });
    if s.is_empty() {
        return Err(SelectError::Empty);
    }
    if let Some(v) = value("id:") {
        let digits = v?;
        return match digits.bytes().all(|b| b.is_ascii_digit()).then(|| digits.parse::<u64>().ok()).flatten() {
            Some(n) => Ok(Sel::Id(NodeId(n))),
            None => Err(SelectError::BadId(s.to_string())),
        };
    }
    if let Some(v) = value("fig:") {
        return Ok(Sel::Figma(v?));
    }
    if let Some(v) = value("name:") {
        return Ok(Sel::Name(v?));
    }
    Ok(Sel::Bare(s))
}

fn for_each_layer(doc: &Document, mut f: impl FnMut(&Node)) {
    for &page in &doc.pages {
        doc.walk(page, |n, _| f(n));
    }
}

fn hit(doc: &Document, id: NodeId, matched_by: By, ambiguous: usize) -> Target {
    Target { node: Some(id), page: doc.page_of(id), matched_by, ambiguous, id_stable: true }
}

fn by_name(doc: &Document, needle: &str) -> Option<Target> {
    let (mut exact, mut loose) = (0, 0);
    let (mut first_exact, mut first_loose) = (None, None);
    for_each_layer(doc, |n| {
        if n.name == needle {
            exact += 1;
            first_exact.get_or_insert(n.id);
        } else if n.name.eq_ignore_ascii_case(needle) {
            loose += 1;
            first_loose.get_or_insert(n.id);
        }
    });
    let named = |id: &NodeId| doc.get(*id).is_some_and(|n| n.name == needle);
    if let Some(first) = first_exact {
        let page = doc.pages.iter().find(|p| named(p));
        let screen = doc.pages.iter().filter_map(|p| doc.get(*p)).find_map(|p| p.children.iter().find(|c| named(c)));
        return Some(hit(doc, page.or(screen).copied().unwrap_or(first), By::Name, exact));
    }
    first_loose.map(|id| hit(doc, id, By::NameCaseInsensitive, loose))
}

fn by_figma_id(doc: &Document, raw: &str, want: &str) -> Result<Option<Target>, SelectError> {
    let (mut count, mut shown) = (0, Vec::new());
    for_each_layer(doc, |n| {
        if n.source_id.as_deref() == Some(want) {
            count += 1;
            if shown.len() < 10 {
                shown.push(n.id);
            }
        }
    });
    match (count, shown.first()) {
        (1, Some(id)) => Ok(Some(hit(doc, *id, By::Figma, 1))),
        (0, _) | (_, None) => Ok(None),
        _ => Err(SelectError::Ambiguous { raw: raw.into(), count, ids: shown.iter().map(|id| id.0.to_string()).collect::<Vec<_>>().join(", ") }),
    }
}

fn stamp(mut target: Target, load_next_id: NodeId) -> Target {
    target.id_stable = target.node.is_none_or(|id| id.0 < load_next_id.0);
    target
}

/// Ids allocated while expanding instances. `id:` and a bare id in this range are refused; a name or `fig:` hit is kept.
fn unstable(id: NodeId, load_next_id: NodeId) -> bool {
    id.0 >= load_next_id.0
}

fn miss(doc: &Document, sel: &Sel, raw: &str, expanded: bool) -> SelectError {
    let mut layers = 0;
    for_each_layer(doc, |_| layers += 1);
    let pages = doc.pages.len();
    let by = match sel {
        Sel::Id(_) => "id",
        Sel::Figma(_) => "Figma id",
        Sel::Bare(v) if v.parse::<u64>().is_ok() => "name or id",
        _ => "name",
    };
    let mut searched = format!(
        "(searched {layers} layers on {pages} page{} by {by}; use --inspect-json to list screens, or id:<n> / fig:<id>)",
        if pages == 1 { "" } else { "s" }
    );
    if expanded {
        let left = doc.hydration_unexpanded_instances();
        if left > 0 {
            searched.push_str(&format!(" (all pages expanded; {left} instances left unexpanded by the clone budget)"));
        } else {
            searched.push_str(" (all pages expanded)");
        }
    }
    if matches!(sel, Sel::Figma(_)) {
        if !doc.nodes.values().any(|n| n.source_id.is_some()) {
            searched.push_str(" (this document has no Figma ids)");
        }
        if doc.nodes.len() > 10_000 {
            searched.push_str(" (instance sublayers in files over 10,000 nodes keep no Figma id; select the instance and use id:<n>)");
        }
    }
    SelectError::NoMatch { raw: raw.into(), searched }
}

fn retry_name(doc: &mut Document, needle: &str, load_next_id: NodeId) -> (Option<Target>, bool) {
    let deferred = doc.page_hydrator.is_some();
    if deferred && doc.hydrate_all_pages() {
        if let Some(target) = by_name(doc, needle) {
            return (Some(stamp(target, load_next_id)), true);
        }
    }
    (None, deferred)
}

fn resolve_id(doc: &Document, raw: &str, id: NodeId, load_next_id: NodeId) -> Result<Target, SelectError> {
    if unstable(id, load_next_id) {
        return Err(SelectError::Unstable { id: id.0 });
    }
    match doc.get(id) {
        Some(_) => Ok(stamp(hit(doc, id, By::Id, 1), load_next_id)),
        None => Err(miss(doc, &Sel::Id(id), raw, false)),
    }
}

fn resolve_fig(doc: &mut Document, raw: &str, want: &str, load_next_id: NodeId) -> Result<Target, SelectError> {
    if let Some(target) = by_figma_id(doc, raw, want)? {
        return Ok(stamp(target, load_next_id));
    }
    // Instance children in a large import keep no Figma id, so expanding pages cannot add a match.
    let deferred = doc.nodes.len() < 10_000 && doc.page_hydrator.is_some();
    if deferred && doc.hydrate_all_pages() {
        if let Some(target) = by_figma_id(doc, raw, want)? {
            return Ok(stamp(target, load_next_id));
        }
    }
    Err(miss(doc, &Sel::Figma(want), raw, deferred))
}

fn resolve_name(doc: &mut Document, raw: &str, needle: &str, load_next_id: NodeId) -> Result<Target, SelectError> {
    if let Some(target) = by_name(doc, needle) {
        return Ok(stamp(target, load_next_id));
    }
    let (found, deferred) = retry_name(doc, needle, load_next_id);
    found.ok_or_else(|| miss(doc, &Sel::Name(needle), raw, deferred))
}

fn resolve_bare(doc: &mut Document, raw: &str, v: &str, load_next_id: NodeId) -> Result<Target, SelectError> {
    let name_hit = by_name(doc, v);
    let id_hit = v.parse::<u64>().ok().map(NodeId).filter(|id| doc.get(*id).is_some());
    match (name_hit, id_hit) {
        (Some(name), Some(id)) => {
            let layer = name.node.map(|n| n.0).unwrap_or(id.0);
            Err(SelectError::NameAndId { raw: raw.into(), layer, id: id.0 })
        }
        (Some(name), None) => Ok(stamp(name, load_next_id)),
        (None, Some(id)) if unstable(id, load_next_id) => Err(SelectError::Unstable { id: id.0 }),
        (None, Some(id)) => Ok(stamp(hit(doc, id, By::BareId, 1), load_next_id)),
        (None, None) => {
            let (found, deferred) = retry_name(doc, v, load_next_id);
            found.ok_or_else(|| miss(doc, &Sel::Bare(v), raw, deferred))
        }
    }
}

/// Resolves `raw`. `load_next_id` is [`Document::peek_next_id`] from just after the file was loaded, before any
/// page was expanded. A hit does not expand pages. A name miss expands them once and retries the name only.
pub fn resolve(doc: &mut Document, raw: &str, load_next_id: NodeId) -> Result<Target, SelectError> {
    let s = raw.trim();
    match parse(s)? {
        Sel::Id(id) => resolve_id(doc, s, id, load_next_id),
        Sel::Figma(want) => resolve_fig(doc, s, want, load_next_id),
        Sel::Name(v) => resolve_name(doc, s, v, load_next_id),
        Sel::Bare(v) => resolve_bare(doc, s, v, load_next_id),
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
