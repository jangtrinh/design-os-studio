//! Compacts node ids that sit too close to `u64::MAX` for new ids to be allocated above them.
//!
//! A damaged or hostile file can carry a node id near `u64::MAX`. Load-time repair only reports
//! that ([`RepairReport::id_space_exhausted`]) because an unattended consumer such as the MCP server
//! must refuse the file. An interactive editor instead calls [`Document::renumber_ids_for_headroom`]
//! right after loading, so the counter has room again and nothing is lost.

use crate::{Action, ComponentLink, Document, Node, NodeId, NodeKind, PropKind, PropValue, MIN_ID_HEADROOM};
use std::collections::BTreeMap;

/// Ids above this value leave less than [`MIN_ID_HEADROOM`] ids to allocate.
const LAST_SAFE_ID: u64 = u64::MAX - MIN_ID_HEADROOM;

impl Document {
    /// Gives every node id above the allocation limit a fresh id just past the largest id that is
    /// fine, rewrites every reference to the old ids, and resets the id counter. Ids that are
    /// already safe never change, so keys derived from them stay valid. Returns how many nodes
    /// were renumbered; a document without such ids is only given a sane counter.
    ///
    /// If the fresh ids would not fit below the limit (more than 2^32 nodes, not reachable in
    /// memory) every node is renumbered `1..=n` in its existing order instead.
    ///
    /// References to ids that no node uses (already dangling) are left as they are.
    pub fn renumber_ids_for_headroom(&mut self) -> usize {
        let too_high: Vec<NodeId> = self.nodes.range(NodeId(LAST_SAFE_ID + 1)..).map(|(id, _)| *id).collect();
        let kept_max = self.nodes.range(..=NodeId(LAST_SAFE_ID)).next_back().map_or(0, |(id, _)| id.0);
        let fits = kept_max.checked_add(too_high.len() as u64).is_some_and(|top| top <= LAST_SAFE_ID);
        let map: BTreeMap<NodeId, NodeId> = if fits {
            too_high.iter().enumerate().map(|(i, id)| (*id, NodeId(kept_max + 1 + i as u64))).collect()
        } else {
            self.nodes.keys().enumerate().map(|(i, id)| (*id, NodeId(i as u64 + 1))).collect()
        };
        let remapped = map.iter().filter(|(old, new)| old != new).count();
        if remapped > 0 {
            self.apply_id_map(&map);
        }
        let largest = self.nodes.keys().next_back().map_or(0, |id| id.0);
        // A counter past the limit carries no history worth keeping; a lower one is never moved back.
        let counter = if self.next_id > LAST_SAFE_ID { 0 } else { self.next_id };
        self.next_id = counter.max(largest);
        self.mark_full_dirty();
        remapped
    }

    /// Rewrites every stored [`NodeId`] through `map`; ids missing from the map stay.
    fn apply_id_map(&mut self, map: &BTreeMap<NodeId, NodeId>) {
        let f = |id: NodeId| map.get(&id).copied().unwrap_or(id);
        let old_nodes = std::mem::take(&mut self.nodes);
        for (id, mut node) in old_nodes {
            remap_node(&mut node, &f);
            self.nodes.insert(f(id), node);
        }
        self.pages.iter_mut().for_each(|p| *p = f(*p));
        self.library_page = self.library_page.map(f);
        for flow in &mut self.flows {
            flow.page = f(flow.page);
            flow.start = f(flow.start);
        }
        self.guides = std::mem::take(&mut self.guides)
            .into_iter()
            .map(|(page, mut guides)| {
                guides.iter_mut().for_each(|g| g.frame = g.frame.map(f));
                (f(page), guides)
            })
            .collect();
        for thread in &mut self.comments {
            thread.anchor.page = f(thread.anchor.page);
            thread.anchor.node = thread.anchor.node.map(f);
        }
        for asset in self.library.external.values_mut() {
            asset.node = asset.node.map(f);
        }
        self.check_designs.ignored = std::mem::take(&mut self.check_designs.ignored)
            .into_iter()
            .map(|mut k| {
                k.node = f(k.node);
                k
            })
            .collect();
    }
}

fn remap_node(node: &mut Node, f: &impl Fn(NodeId) -> NodeId) {
    node.id = f(node.id);
    node.parent = node.parent.map(f);
    node.children.iter_mut().for_each(|c| *c = f(*c));
    match &mut node.kind {
        NodeKind::Instance { component, .. } => *component = component.map(f),
        NodeKind::Connector(c) => {
            c.start.node = c.start.node.map(f);
            c.end.node = c.end.node.map(f);
        }
        _ => {}
    }
    remap_component_link(&mut node.component, f);
    node.prototype.fixed_children.iter_mut().for_each(|c| *c = f(*c));
    for reaction in &mut node.prototype.reactions {
        reaction.actions.iter_mut().for_each(|a| remap_action(a, f));
    }
}

fn remap_component_link(link: &mut ComponentLink, f: &impl Fn(NodeId) -> NodeId) {
    link.source = link.source.map(f);
    for prop in &mut link.props {
        remap_prop_value(&mut prop.default, f);
        if let PropKind::InstanceSwap { preferred } = &mut prop.kind {
            preferred.iter_mut().for_each(|p| *p = f(*p));
        }
    }
    link.values.values_mut().for_each(|v| remap_prop_value(v, f));
    for cfg in link.slot_configs.values_mut() {
        cfg.preferred.iter_mut().for_each(|p| *p = f(*p));
    }
}

fn remap_prop_value(value: &mut PropValue, f: &impl Fn(NodeId) -> NodeId) {
    match value {
        PropValue::Instance(id) => *id = f(*id),
        PropValue::Slot(ids) => ids.iter_mut().for_each(|id| *id = f(*id)),
        PropValue::Bool(_) | PropValue::Text(_) | PropValue::Variant(_) => {}
    }
}

fn remap_action(action: &mut Action, f: &impl Fn(NodeId) -> NodeId) {
    match action {
        Action::Navigate { dest, .. }
        | Action::OpenOverlay { dest, .. }
        | Action::SwapOverlay { dest }
        | Action::ScrollTo { dest, .. }
        | Action::ChangeTo { dest }
        | Action::MediaControl { dest, .. } => *dest = f(*dest),
        Action::Conditional { branches, otherwise } => {
            for (_, actions) in branches {
                actions.iter_mut().for_each(|a| remap_action(a, f));
            }
            otherwise.iter_mut().for_each(|a| remap_action(a, f));
        }
        Action::CloseOverlay | Action::Back | Action::OpenUrl { .. } | Action::SetVariable { .. } | Action::SetVariableMode { .. } => {}
    }
}
