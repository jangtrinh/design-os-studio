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
        // A first pass can see one match while another matching node is still inside a
        // deferred page. Expand once before accepting a supposedly unique Figma id.
        if ids.len() <= 1 && doc.hydrate_all_pages() {
            return resolve(doc, raw);
        }
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
    if kind == "bare" {
        // Preserve the pre-existing CLI contract: a bare decimal selector addresses
        // the numeric node ID before it is treated as a layer name. Callers that need
        // the literal name can use the explicit `name:` form.
        if let Some(id) = value.parse::<u64>().ok().map(NodeId).filter(|id| doc.get(*id).is_some()) {
            return Ok(Target { node: Some(id), page: doc.page_of(id), matched_by: By::BareId, ambiguous: 1 });
        }
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
    let exact_matches: Vec<_> = page_walk(doc).filter(|id| doc.get(*id).is_some_and(|n| n.name == value)).collect();
    let insensitive_matches: Vec<_> = if exact_matches.is_empty() {
        page_walk(doc).filter(|id| doc.get(*id).is_some_and(|n| n.name.eq_ignore_ascii_case(value))).collect()
    } else {
        Vec::new()
    };
    let found = exact(value)
        .map(|id| (id, By::Name, exact_matches.len()))
        .or_else(|| insensitive_matches.first().copied().map(|id| (id, By::NameCaseInsensitive, insensitive_matches.len())));
    if let Some((id, by, ambiguous)) = found {
        return Ok(Target { node: Some(id), page: doc.page_of(id), matched_by: by, ambiguous });
    }
    if doc.hydrate_all_pages() {
        return resolve(doc, raw);
    }
    Err(format!("No node matches --select '{s}'"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use designos_doc::{Node, NodeKind};

    #[test]
    fn bare_numeric_id_precedes_layer_name() {
        let mut doc = Document::new("numeric selector");
        let page = doc.add_page("Page");
        let mut named = Node::new(NodeId(2), NodeKind::Rectangle);
        named.name = "4".into();
        doc.insert(page, named).unwrap();
        doc.insert(page, Node::new(NodeId(4), NodeKind::Rectangle)).unwrap();
        let target = resolve(&mut doc, "4").unwrap();
        assert_eq!(target.node, Some(NodeId(4)));
        assert_eq!(target.matched_by, By::BareId);

        let explicit_name = resolve(&mut doc, "name:4").unwrap();
        assert_eq!(explicit_name.node, Some(NodeId(2)));
    }

    #[test]
    fn duplicate_figma_ids_are_rejected() {
        let mut doc = Document::new("duplicate figma ids");
        let page = doc.add_page("Page");
        for id in [2, 3] {
            let mut node = Node::new(NodeId(id), NodeKind::Rectangle);
            node.source_id = Some("12:34".into());
            doc.insert(page, node).unwrap();
        }
        let err = resolve(&mut doc, "fig:12:34").unwrap_err();
        assert!(err.contains("ambiguous"), "{err}");
        assert!(err.contains("2") && err.contains("3"), "{err}");
    }

    #[test]
    fn duplicate_names_report_count_while_selecting_first() {
        let mut doc = Document::new("duplicate names");
        let page = doc.add_page("Page");
        for id in [2, 3] {
            let mut node = Node::new(NodeId(id), NodeKind::Rectangle);
            node.name = "Card".into();
            doc.insert(page, node).unwrap();
        }
        let target = resolve(&mut doc, "name:Card").unwrap();
        assert_eq!(target.node, Some(NodeId(2)));
        assert_eq!(target.ambiguous, 2);
    }
}
