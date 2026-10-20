//! The canonical XPath of a layer ([`crate::openpencil::node_to_xpath`]), for the many findings of
//! one lint pass: the position of each child among the siblings of its type is worked out once per
//! parent, not once per finding.

use crate::lint::PassCache;
use crate::openpencil::xpath_type_name;
use crate::{Document, NodeId, NodeKind};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// Per parent, the 1-based position of each child among the children of its type.
#[derive(Default)]
struct SiblingPositions(Mutex<HashMap<NodeId, Arc<HashMap<NodeId, usize>>>>);

impl SiblingPositions {
    fn of(&self, doc: &Document, parent: NodeId) -> Arc<HashMap<NodeId, usize>> {
        let mut parents = self.0.lock().unwrap_or_else(|e| e.into_inner());
        Arc::clone(parents.entry(parent).or_insert_with(|| {
            let mut counts: HashMap<&'static str, usize> = HashMap::new();
            let mut at = HashMap::new();
            for &sib_id in doc.get(parent).map_or(&[][..], |p| p.children.as_slice()) {
                let Some(sib) = doc.get(sib_id) else { continue };
                let count = counts.entry(xpath_type_name(&sib.kind)).or_insert(0);
                *count += 1;
                at.entry(sib_id).or_insert(*count);
            }
            Arc::new(at)
        }))
    }
}

/// `node_to_xpath(doc, id)`, with the sibling positions kept in `pass`.
pub fn xpath_in_pass(doc: &Document, id: NodeId, pass: &PassCache) -> String {
    let Some(node) = doc.get(id) else { return String::new() };
    if matches!(node.kind, NodeKind::Page { .. }) {
        let idx = doc.pages.iter().position(|&p| p == id).map_or(1, |i| i + 1);
        return format!("/PAGE[{idx}]");
    }
    let positions = pass.get_or_insert_with(SiblingPositions::default);
    let mut chain = Vec::new();
    let mut cur = Some(id);
    while let Some(cid) = cur {
        let Some(cn) = doc.get(cid) else { break };
        if matches!(cn.kind, NodeKind::Page { .. }) {
            break;
        }
        let idx = cn.parent.filter(|p| doc.get(*p).is_some()).map_or(1, |p| positions.of(doc, p).get(&cid).copied().unwrap_or(1));
        chain.push(format!("/{}[{idx}]", xpath_type_name(&cn.kind)));
        cur = cn.parent;
    }
    chain.reverse();
    if chain.is_empty() {
        format!("/{}[1]", xpath_type_name(&node.kind))
    } else {
        chain.concat()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::openpencil::node_to_xpath;
    use crate::{Node, Rect, TextStyle};

    #[test]
    fn matches_node_to_xpath_for_every_layer() {
        let mut doc = Document::new("xpath");
        let page = doc.add_page("One");
        let second = doc.add_page("Two");
        let mut parents = vec![page, second];
        for i in 0..60u32 {
            let parent = parents[(i as usize * 7) % parents.len()];
            let id = doc.alloc_id();
            let rect = Rect::new(0.0, 0.0, 10.0, 10.0);
            let node = match i % 3 {
                0 => Node::rectangle(id, rect),
                1 => Node::text(id, rect, "t", TextStyle::default()),
                _ => Node::frame(id, rect),
            };
            let is_frame = matches!(node.kind, NodeKind::Frame(_));
            let id = doc.insert(parent, node).unwrap();
            if is_frame {
                parents.push(id);
            }
        }
        // A child that is listed but does not exist, and one listed twice.
        let ghost = doc.alloc_id();
        let first_child = doc.get(page).and_then(|p| p.children.first().copied());
        if let Some(p) = doc.get_mut(page) {
            p.children.insert(0, ghost);
            p.children.extend(first_child);
        }
        let pass = PassCache::default();
        let ids: Vec<NodeId> = doc.nodes.keys().copied().chain([ghost]).collect();
        assert!(ids.len() > 60);
        for id in ids {
            assert_eq!(xpath_in_pass(&doc, id, &pass), node_to_xpath(&doc, id), "layer {id:?}");
        }
    }
}
