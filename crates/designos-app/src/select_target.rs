//! Shared CLI/GUI node selector resolver.
use designos_doc::{Document, NodeId};

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
    pub ambiguous: usize,
}

fn page_walk(doc: &Document) -> impl Iterator<Item = NodeId> + '_ {
    doc.pages.iter().copied().flat_map(move |pid| {
        let mut ids = Vec::new();
        doc.walk(pid, |n, _| ids.push(n.id));
        ids.into_iter()
    })
}

pub fn resolve(doc: &mut Document, raw: &str) -> Result<Target, String> {
    let s = raw.trim();
    let (kind, value) = if let Some(v) = s.strip_prefix("id:") {
        ("id", v)
    } else if let Some(v) = s.strip_prefix("fig:") {
        ("fig", v)
    } else if let Some(v) = s.strip_prefix("name:") {
        ("name", v)
    } else {
        ("bare", s)
    };
    if value.is_empty() {
        return Err(format!("Invalid --select '{s}': selector value is empty"));
    }
    if kind == "id" {
        let id = value.parse::<u64>().map_err(|_| format!("Invalid --select '{s}': id must be a decimal number"))?;
        let id = NodeId(id);
        if doc.get(id).is_some() {
            return Ok(Target { node: Some(id), page: doc.page_of(id), matched_by: By::Id, ambiguous: 1 });
        }
        if doc.hydrate_all_pages() && doc.get(id).is_some() {
            return Ok(Target { node: Some(id), page: doc.page_of(id), matched_by: By::Id, ambiguous: 1 });
        }
        return Err(format!("No node matches --select '{s}'"));
    }
    if kind == "fig" {
        let ids: Vec<_> = page_walk(doc).filter(|id| doc.get(*id).and_then(|n| n.source_id.as_deref()) == Some(value)).collect();
        return match ids.as_slice() {
            [id] => Ok(Target { node: Some(*id), page: doc.page_of(*id), matched_by: By::Figma, ambiguous: 1 }),
            [] => {
                if doc.hydrate_all_pages() {
                    return resolve(doc, raw);
                }
                Err(format!("No node matches --select '{s}'"))
            }
            _ => {
                let listed = ids.iter().take(10).map(|id| id.0.to_string()).collect::<Vec<_>>().join(", ");
                Err(format!("Figma selector '{s}' is ambiguous: {} matches (ids: {listed})", ids.len()))
            }
        };
    }
    let exact = |needle: &str| {
        doc.pages
            .iter()
            .find_map(|pid| doc.get(*pid).filter(|n| n.name == needle).map(|n| n.id))
            .or_else(|| {
                doc.pages
                    .iter()
                    .find_map(|pid| doc.get(*pid).and_then(|p| p.children.iter().find_map(|cid| doc.get(*cid).filter(|n| n.name == needle).map(|n| n.id))))
            })
            .or_else(|| page_walk(doc).find(|id| doc.get(*id).is_some_and(|n| n.name == needle)))
    };
    let found = exact(value)
        .map(|id| (id, By::Name))
        .or_else(|| page_walk(doc).find(|id| doc.get(*id).is_some_and(|n| n.name.eq_ignore_ascii_case(value))).map(|id| (id, By::NameCaseInsensitive)))
        .or_else(|| (kind == "bare").then(|| value.parse::<u64>().ok().map(NodeId)).flatten().filter(|id| doc.get(*id).is_some()).map(|id| (id, By::BareId)));
    if let Some((id, by)) = found {
        return Ok(Target { node: Some(id), page: doc.page_of(id), matched_by: by, ambiguous: 1 });
    }
    if doc.hydrate_all_pages() {
        return resolve(doc, raw);
    }
    Err(format!("No node matches --select '{s}'"))
}
