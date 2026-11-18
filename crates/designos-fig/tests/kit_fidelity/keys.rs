//! Stable keys for the layers of an imported document, so two runs name the same layer the same way whatever ids
//! hydration handed out: a layer with a source id is keyed by it, any other layer by its parent's key and its index
//! (`12:34/0/3`). Also which layers sit in a hidden subtree.

use designos_doc::{Document, NodeId};
use std::collections::{HashMap, HashSet};

pub struct NodeKeys {
    pub key: HashMap<NodeId, String>,
    pub hidden: HashSet<NodeId>,
    pub by_source: HashMap<String, NodeId>,
}

impl NodeKeys {
    pub fn build(doc: &Document) -> Self {
        let mut key = HashMap::new();
        let mut hidden = HashSet::new();
        let mut by_source = HashMap::new();
        let mut used: HashSet<String> = HashSet::new();
        let mut stack: Vec<(NodeId, String, bool, bool)> = Vec::new();
        for (i, &page) in doc.pages.iter().enumerate().rev() {
            let k = doc.get(page).and_then(|n| n.source_id.clone()).unwrap_or_else(|| format!("page{i}"));
            stack.push((page, k, false, true));
        }
        while let Some((id, k, parent_hidden, is_page)) = stack.pop() {
            let Some(n) = doc.get(id) else { continue };
            let is_hidden = !is_page && (parent_hidden || !n.visible);
            if is_hidden {
                hidden.insert(id);
            }
            if let Some(s) = &n.source_id {
                by_source.entry(s.clone()).or_insert(id);
            }
            for (i, &c) in n.children.iter().enumerate().rev() {
                let ck = doc.get(c).and_then(|cn| cn.source_id.clone()).filter(|s| !used.contains(s)).unwrap_or_else(|| format!("{k}/{i}"));
                used.insert(ck.clone());
                stack.push((c, ck, is_hidden, false));
            }
            key.insert(id, k);
        }
        NodeKeys { key, hidden, by_source }
    }

    pub fn key_of(&self, id: NodeId) -> String {
        self.key.get(&id).cloned().unwrap_or_else(|| format!("#{}", id.0))
    }

    pub fn is_hidden(&self, id: NodeId) -> bool {
        self.hidden.contains(&id)
    }
}
