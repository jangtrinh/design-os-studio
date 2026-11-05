//! Name lookup, bare-id parse, the load-watermark check, and the one hydrate-and-retry.

use super::miss::miss;
use super::{By, SelectError, Target};
use designos_doc::{Document, Node, NodeId};

/// A file this size or larger keeps no Figma id on instance children, so expanding pages cannot add a `fig:` match.
pub(super) const LARGE_FILE_NODES: usize = 10_000;

pub(super) enum Sel<'a> {
    Id(NodeId),
    Figma(&'a str),
    Name(&'a str),
    Bare(&'a str),
}

pub(super) fn parse(s: &str) -> Result<Sel<'_>, SelectError> {
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

pub(super) fn for_each_layer(doc: &Document, mut f: impl FnMut(&Node)) {
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

/// Expands every deferred page once and looks the name up again. The bool is whether an expansion ran and changed the document.
fn retry_name(doc: &mut Document, needle: &str, load_next_id: NodeId) -> (Option<Target>, bool) {
    let expanded = doc.page_hydrator.is_some() && doc.hydrate_all_pages();
    let found = if expanded { by_name(doc, needle).map(|target| stamp(target, load_next_id)) } else { None };
    (found, expanded)
}

pub(super) fn resolve_id(doc: &Document, raw: &str, id: NodeId, load_next_id: NodeId) -> Result<Target, SelectError> {
    match doc.get(id) {
        None => Err(miss(doc, &Sel::Id(id), raw, false)),
        Some(_) if unstable(id, load_next_id) => Err(SelectError::Unstable { id: id.0 }),
        Some(_) => Ok(stamp(hit(doc, id, By::Id, 1), load_next_id)),
    }
}

pub(super) fn resolve_fig(doc: &mut Document, raw: &str, want: &str, load_next_id: NodeId) -> Result<Target, SelectError> {
    if let Some(target) = by_figma_id(doc, raw, want)? {
        return Ok(stamp(target, load_next_id));
    }
    let expanded = doc.nodes.len() < LARGE_FILE_NODES && doc.page_hydrator.is_some() && doc.hydrate_all_pages();
    if expanded {
        if let Some(target) = by_figma_id(doc, raw, want)? {
            return Ok(stamp(target, load_next_id));
        }
    }
    Err(miss(doc, &Sel::Figma(want), raw, expanded))
}

pub(super) fn resolve_name(doc: &mut Document, raw: &str, needle: &str, load_next_id: NodeId) -> Result<Target, SelectError> {
    if let Some(target) = by_name(doc, needle) {
        return Ok(stamp(target, load_next_id));
    }
    let (found, expanded) = retry_name(doc, needle, load_next_id);
    found.ok_or_else(|| miss(doc, &Sel::Name(needle), raw, expanded))
}

pub(super) fn resolve_bare(doc: &mut Document, raw: &str, v: &str, load_next_id: NodeId) -> Result<Target, SelectError> {
    let name_hit = by_name(doc, v);
    let id_hit = v.parse::<u64>().ok().map(NodeId).filter(|id| doc.get(*id).is_some());
    match (name_hit, id_hit) {
        (Some(name), Some(id)) => {
            let layer = name.node.map(|n| n.0).unwrap_or(id.0);
            Err(SelectError::NameAndId { raw: raw.into(), layer, id: id.0, id_stable: !unstable(id, load_next_id) })
        }
        (Some(name), None) => Ok(stamp(name, load_next_id)),
        (None, Some(id)) if unstable(id, load_next_id) => Err(SelectError::Unstable { id: id.0 }),
        (None, Some(id)) => Ok(stamp(hit(doc, id, By::BareId, 1), load_next_id)),
        (None, None) => {
            let (found, expanded) = retry_name(doc, v, load_next_id);
            found.ok_or_else(|| miss(doc, &Sel::Bare(v), raw, expanded))
        }
    }
}
