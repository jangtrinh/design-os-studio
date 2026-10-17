//! Undo/redo. Every edit goes through [`Command`]; applying returns its inverse.

use crate::*;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Command {
    /// Insert subtree nodes (root first) under `parent` at `index`.
    Insert {
        parent: NodeId,
        index: usize,
        nodes: Vec<Node>,
    },
    Remove {
        id: NodeId,
    },
    Reparent {
        id: NodeId,
        parent: NodeId,
        index: usize,
    },
    /// Replace a node's own properties (children/parent are preserved).
    Update {
        node: Box<Node>,
    },
    /// Insert a page subtree (page node first) at `index` in `Document::pages`.
    InsertPage {
        index: usize,
        nodes: Vec<Node>,
    },
    RemovePage {
        id: NodeId,
    },
    /// Replace the document's prototype flow list (flow starting points).
    SetFlows {
        flows: Vec<Flow>,
    },
    /// Replace shared styles and their metadata (style create / edit / delete).
    SetStyles {
        styles: BTreeMap<String, Style>,
        style_meta: BTreeMap<String, StyleMeta>,
    },
    /// Replace all comment threads.
    SetComments {
        comments: Vec<CommentThread>,
    },
    /// Replace library bookkeeping (publish info, enabled libraries, asset copies).
    SetLibraryState {
        state: LibraryState,
        library_page: Option<NodeId>,
    },
    /// Replace the guides of one page.
    SetGuides {
        page: NodeId,
        guides: Vec<Guide>,
    },
    /// Replace the prototype device settings.
    SetPrototypeDevice {
        device: PrototypeDevice,
    },
    /// Replace all variables and collections (variable edits).
    SetVariables {
        variables: BTreeMap<String, Variable>,
        collections: BTreeMap<String, VariableCollection>,
    },
    /// Adds embedded image bytes (a hash already present keeps its bytes) and drops the `remove` hashes.
    /// Its inverse removes what was newly added and puts the removed bytes back, so undo/redo of a paste
    /// leaves no orphaned images and loses none.
    SetImages {
        add: BTreeMap<String, Vec<u8>>,
        remove: Vec<String>,
    },
    Batch(Vec<Command>),
}

impl Command {
    /// Applies the command and returns the command that undoes it.
    pub fn apply(self, doc: &mut Document) -> Result<Command, DocError> {
        Ok(match self {
            Command::Insert { parent, index, nodes } => {
                let root = nodes.first().map(|n| n.id).ok_or(DocError::NotFound(parent))?;
                let p = doc.nodes.get_mut(&parent).ok_or(DocError::NotFound(parent))?;
                if !p.kind.is_container() {
                    return Err(DocError::NotAContainer(parent));
                }
                let index = index.min(p.children.len());
                p.children.insert(index, root);
                let many = nodes.len() > 64;
                for mut n in nodes {
                    if n.id == root {
                        n.parent = Some(parent);
                    }
                    doc.nodes.insert(n.id, n);
                }
                doc.bump_ids();
                if many {
                    doc.mark_tree_full_dirty();
                } else {
                    doc.mark_node_dirty(parent);
                    doc.mark_node_dirty(root);
                }
                Command::Remove { id: root }
            }
            Command::Remove { id } => {
                let parent = doc.get(id).ok_or(DocError::NotFound(id))?.parent.ok_or(DocError::NotFound(id))?;
                let index = doc.get(parent).map(|p| p.children.iter().position(|c| *c == id).unwrap_or(0)).unwrap_or(0);
                let nodes = doc.remove(id)?;
                Command::Insert { parent, index, nodes }
            }
            Command::Reparent { id, parent, index } => {
                let n = doc.get(id).ok_or(DocError::NotFound(id))?;
                let old_parent = n.parent.ok_or(DocError::NotFound(id))?;
                let old_index = doc.get(old_parent).and_then(|p| p.children.iter().position(|c| *c == id)).unwrap_or(0);
                doc.reparent(id, parent, index)?;
                Command::Reparent { id, parent: old_parent, index: old_index }
            }
            Command::Update { mut node } => {
                let cur = doc.get_mut(node.id).ok_or(DocError::NotFound(node.id))?;
                node.parent = cur.parent;
                node.children = std::mem::take(&mut cur.children);
                let old = std::mem::replace(cur, *node);
                Command::Update { node: Box::new(Node { children: Vec::new(), ..old }) }
            }
            Command::InsertPage { index, nodes } => {
                let root = nodes.first().ok_or(DocError::NotFound(NodeId(0)))?;
                if !matches!(root.kind, NodeKind::Page { .. }) {
                    return Err(DocError::NotAContainer(root.id));
                }
                let root = root.id;
                for mut n in nodes {
                    if n.id == root {
                        n.parent = None;
                    }
                    doc.nodes.insert(n.id, n);
                }
                let index = index.min(doc.pages.len());
                doc.pages.insert(index, root);
                doc.bump_ids();
                doc.mark_tree_full_dirty();
                Command::RemovePage { id: root }
            }
            Command::RemovePage { id } => {
                let index = doc.pages.iter().position(|p| *p == id).ok_or(DocError::NotFound(id))?;
                let nodes = doc.remove(id)?;
                Command::InsertPage { index, nodes }
            }
            Command::SetFlows { flows } => Command::SetFlows { flows: std::mem::replace(&mut doc.flows, flows) },
            Command::SetStyles { styles, style_meta } => {
                doc.mark_full_dirty();
                Command::SetStyles { styles: std::mem::replace(&mut doc.styles, styles), style_meta: std::mem::replace(&mut doc.style_meta, style_meta) }
            }
            Command::SetComments { comments } => Command::SetComments { comments: std::mem::replace(&mut doc.comments, comments) },
            Command::SetLibraryState { state, library_page } => Command::SetLibraryState {
                state: std::mem::replace(&mut doc.library, state),
                library_page: std::mem::replace(&mut doc.library_page, library_page),
            },
            Command::SetGuides { page, guides } => {
                let old = if guides.is_empty() { doc.guides.remove(&page) } else { doc.guides.insert(page, guides) };
                Command::SetGuides { page, guides: old.unwrap_or_default() }
            }
            Command::SetPrototypeDevice { device } => Command::SetPrototypeDevice { device: std::mem::replace(&mut doc.prototype_device, device) },
            Command::SetVariables { variables, collections } => {
                doc.mark_full_dirty();
                Command::SetVariables {
                    variables: std::mem::replace(&mut doc.variables, variables),
                    collections: std::mem::replace(&mut doc.variable_collections, collections),
                }
            }
            Command::SetImages { add, remove } => {
                let removed: BTreeMap<String, Vec<u8>> = remove.into_iter().filter_map(|h| doc.images.remove(&h).map(|bytes| (h, bytes))).collect();
                let mut added = Vec::new();
                for (hash, bytes) in add {
                    if let std::collections::btree_map::Entry::Vacant(slot) = doc.images.entry(hash) {
                        added.push(slot.key().clone());
                        slot.insert(bytes);
                    }
                }
                Command::SetImages { add: removed, remove: added }
            }
            Command::Batch(cmds) => {
                let mut inv = Vec::with_capacity(cmds.len());
                for c in cmds {
                    inv.push(c.apply(doc)?);
                }
                inv.reverse();
                Command::Batch(inv)
            }
        })
    }
}

impl Document {
    pub(crate) fn bump_ids(&mut self) {
        if let Some((id, _)) = self.nodes.last_key_value() {
            self.next_id = self.next_id.max(id.0);
        }
    }
}

/// Linear undo stack.
#[derive(Default)]
pub struct History {
    undo: Vec<Command>,
    redo: Vec<Command>,
}

impl History {
    pub fn exec(&mut self, doc: &mut Document, cmd: Command) -> Result<(), DocError> {
        let inv = cmd.apply(doc)?;
        self.undo.push(inv);
        self.redo.clear();
        Ok(())
    }
    pub fn undo(&mut self, doc: &mut Document) -> Result<bool, DocError> {
        let Some(c) = self.undo.pop() else { return Ok(false) };
        self.redo.push(c.apply(doc)?);
        Ok(true)
    }
    pub fn redo(&mut self, doc: &mut Document) -> Result<bool, DocError> {
        let Some(c) = self.redo.pop() else { return Ok(false) };
        self.undo.push(c.apply(doc)?);
        Ok(true)
    }
    /// The command [`Self::undo`] would apply next.
    pub fn peek_undo(&self) -> Option<&Command> {
        self.undo.last()
    }
    /// The command [`Self::redo`] would apply next.
    pub fn peek_redo(&self) -> Option<&Command> {
        self.redo.last()
    }
    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }
    /// Number of steps [`Self::undo`] can take.
    pub fn undo_depth(&self) -> usize {
        self.undo.len()
    }
    /// Forgets undo steps past `depth` without applying them, for edits the caller already
    /// reverted another way (by restoring a snapshot of the document).
    pub fn drop_undo_steps_after(&mut self, depth: usize) {
        self.undo.truncate(depth);
    }
    /// The steps [`Self::redo`] can take, oldest first, for [`Self::restore_redo`].
    pub fn redo_steps(&self) -> Vec<Command> {
        self.redo.clone()
    }
    /// Puts back the redo steps an edit cleared, for an edit the caller reverted another way.
    pub fn restore_redo(&mut self, steps: Vec<Command>) {
        self.redo = steps;
    }
    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn undo_redo_update_and_remove() {
        let mut d = Document::new("t");
        let page = d.add_page("P");
        let id = d.alloc_id();
        d.insert(page, Node::rectangle(id, Rect::new(0.0, 0.0, 10.0, 10.0))).unwrap();
        let mut h = History::default();

        let mut n = d.get(id).unwrap().clone();
        n.name = "Renamed".into();
        h.exec(&mut d, Command::Update { node: Box::new(n) }).unwrap();
        assert_eq!(d.get(id).unwrap().name, "Renamed");
        h.undo(&mut d).unwrap();
        assert_eq!(d.get(id).unwrap().name, "Rectangle");

        h.exec(&mut d, Command::Remove { id }).unwrap();
        assert!(d.get(id).is_none());
        h.undo(&mut d).unwrap();
        assert_eq!(d.get(page).unwrap().children, vec![id]);
        h.redo(&mut d).unwrap();
        assert!(d.get(id).is_none());
    }

    #[test]
    fn undo_redo_images_add_only_what_was_missing() {
        let mut d = Document::new("t");
        d.images.insert("old".into(), vec![9]);
        let mut h = History::default();
        let add: BTreeMap<String, Vec<u8>> = [("old".to_string(), vec![0]), ("new".to_string(), vec![1, 2])].into();
        h.exec(&mut d, Command::SetImages { add, remove: vec![] }).unwrap();
        assert_eq!(d.images["old"], vec![9], "an existing hash keeps its bytes");
        assert_eq!(d.images["new"], vec![1, 2]);
        h.undo(&mut d).unwrap();
        assert_eq!(d.images.keys().collect::<Vec<_>>(), ["old"], "undo removes only the newly added image");
        assert_eq!(d.images["old"], vec![9]);
        h.redo(&mut d).unwrap();
        assert_eq!(d.images["new"], vec![1, 2]);
        h.exec(&mut d, Command::SetImages { add: BTreeMap::new(), remove: vec!["new".into(), "missing".into()] }).unwrap();
        assert!(!d.images.contains_key("new"));
        h.undo(&mut d).unwrap();
        assert_eq!(d.images["new"], vec![1, 2], "undoing a removal puts the bytes back");
    }

    #[test]
    fn undo_redo_flows() {
        let mut d = Document::new("t");
        let page = d.add_page("P");
        let fid = d.alloc_id();
        let f = d.insert(page, Node::frame(fid, Rect::new(0.0, 0.0, 10.0, 10.0))).unwrap();
        let flow = Flow { name: "Flow 1".into(), page, start: f, description: String::new() };
        let mut h = History::default();
        h.exec(&mut d, Command::SetFlows { flows: vec![flow.clone()] }).unwrap();
        assert_eq!(d.flows, vec![flow]);
        h.undo(&mut d).unwrap();
        assert!(d.flows.is_empty());
        h.redo(&mut d).unwrap();
        assert_eq!(d.flows.len(), 1);
    }

    #[test]
    fn undo_redo_pages() {
        let mut d = Document::new("t");
        let p1 = d.add_page("P1");
        let pid = d.alloc_id();
        let mut page = Node::new(pid, NodeKind::Page { background: Color::WHITE });
        page.name = "P2".into();
        let mut h = History::default();
        h.exec(&mut d, Command::InsertPage { index: 0, nodes: vec![page] }).unwrap();
        assert_eq!(d.pages, vec![pid, p1]);
        let rid = d.alloc_id();
        d.insert(pid, Node::rectangle(rid, Rect::new(0.0, 0.0, 1.0, 1.0))).unwrap();

        h.exec(&mut d, Command::RemovePage { id: pid }).unwrap();
        assert_eq!(d.pages, vec![p1]);
        assert!(d.get(rid).is_none());
        h.undo(&mut d).unwrap();
        assert_eq!(d.pages, vec![pid, p1]);
        assert_eq!(d.get(pid).unwrap().children, vec![rid]);
        h.undo(&mut d).unwrap();
        assert_eq!(d.pages, vec![p1]);
    }
}

#[cfg(test)]
mod style_tests {
    use crate::*;

    #[test]
    fn set_styles_undo() {
        let mut d = Document::new("t");
        d.add_page("P");
        let mut h = History::default();
        let mut styles = d.styles.clone();
        styles.insert("s1".into(), Style::Fill(vec![Paint::solid(Color::WHITE)]));
        let mut meta = d.style_meta.clone();
        meta.insert("s1".into(), StyleMeta { name: "Brand/White".into(), ..Default::default() });
        h.exec(&mut d, Command::SetStyles { styles, style_meta: meta }).unwrap();
        assert_eq!(d.style_name("s1"), "Brand/White");
        h.undo(&mut d).unwrap();
        assert!(d.styles.is_empty() && d.style_meta.is_empty());
        h.redo(&mut d).unwrap();
        assert_eq!(d.styles.len(), 1);
    }
}
