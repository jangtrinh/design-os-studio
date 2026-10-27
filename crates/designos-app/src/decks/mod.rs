//! Native slide documents: ordinary editable frame trees with presentation metadata.
use crate::{app::App, clipboard, edit::Tx};
use designos_doc::*;

#[cfg(any(test, feature = "fixtures"))]
pub mod demo;
pub mod export;
pub mod ui;

pub const SLIDE_WIDTH: f32 = 1280.0;
pub const SLIDE_HEIGHT: f32 = 720.0;
const SLIDE_GAP: f32 = 120.0;

#[derive(Default)]
pub struct DecksState {
    pub design_mode: bool,
    pub grid: bool,
    pub active: Option<NodeId>,
    pub notes_draft: String,
    pub notes_saved: String,
    pub notes_for: Option<NodeId>,
}

fn slide_node(id: NodeId, number: usize) -> Node {
    let mut node = Node::frame(id, Rect::new(0.0, 0.0, SLIDE_WIDTH, SLIDE_HEIGHT));
    node.name = format!("Slide {number}");
    node.slide = Some(SlideProps::default());
    if let NodeKind::Frame(props) = &mut node.kind {
        props.clips_content = true;
    }
    node
}

pub fn new_document(name: &str) -> Document {
    let mut doc = Document::new(name);
    doc.kind = DocumentKind::Deck;
    let page = doc.add_page("Slides");
    let id = doc.alloc_id();
    doc.insert(page, slide_node(id, 1)).expect("new page accepts slide");
    doc
}

fn slides(doc: &Document, page: NodeId) -> Vec<NodeId> {
    doc.get(page)
        .map(|p| p.children.iter().copied().filter(|id| doc.get(*id).is_some_and(|n| n.slide.is_some() && matches!(n.kind, NodeKind::Frame(_)))).collect())
        .unwrap_or_default()
}

fn arrange(doc: &mut Document, page: NodeId, tx: &mut Tx) {
    for (index, id) in slides(doc, page).into_iter().enumerate() {
        if let Some(node) = tx.ed.node(doc, id) {
            node.transform.tx = (index % 3) as f32 * (SLIDE_WIDTH + SLIDE_GAP);
            node.transform.ty = (index / 3) as f32 * (SLIDE_HEIGHT + SLIDE_GAP);
        }
    }
}

impl App {
    pub fn is_deck(&self) -> bool {
        self.doc.kind == DocumentKind::Deck
    }

    pub fn new_deck_document(&mut self) {
        self.replace_document(new_document("Untitled deck"), None);
        self.decks = DecksState::default();
        if let Some(id) = self.deck_slides().first().copied() {
            self.select_slide(id);
        }
    }

    pub fn deck_slides(&self) -> Vec<NodeId> {
        if self.is_deck() {
            slides(&self.doc, self.page)
        } else {
            Vec::new()
        }
    }

    pub fn active_slide(&self) -> Option<NodeId> {
        let slides = self.deck_slides();
        for selected in &self.selection {
            let mut current = Some(*selected);
            // Bound traversal so corrupt documents cannot trap the editor.
            for _ in 0..self.doc.nodes.len() {
                let Some(id) = current else { break };
                if slides.contains(&id) {
                    return Some(id);
                }
                current = self.doc.get(id).and_then(|node| node.parent);
            }
        }
        self.decks.active.filter(|id| slides.contains(id)).or_else(|| slides.first().copied())
    }

    fn deck_editable(&self) -> bool {
        self.is_deck() && self.home.is_none() && self.history.viewing.is_none() && self.proto.present.is_none() && self.scrub.is_none() && self.dialog.is_none()
    }

    pub fn select_slide(&mut self, id: NodeId) {
        if !self.deck_slides().contains(&id) {
            return;
        }
        ui::flush_notes(self);
        crate::canvas::interact::finish_text(self, true);
        self.decks.active = Some(id);
        self.selection = vec![id];
        self.decks.notes_for = Some(id);
        self.decks.notes_draft = self.doc.get(id).and_then(|n| n.slide.as_ref()).map(|s| s.notes.clone()).unwrap_or_default();
        self.decks.notes_saved = self.decks.notes_draft.clone();
        self.fit_selection();
    }

    pub fn add_slide(&mut self) {
        if !self.deck_editable() {
            return;
        }
        crate::canvas::interact::finish_text(self, true);
        let list = self.deck_slides();
        let index = self.active_slide().and_then(|id| self.doc.get(self.page)?.children.iter().position(|child| *child == id)).map_or(usize::MAX, |i| i + 1);
        let id = self.doc.alloc_id();
        let mut tx = Tx::new();
        if tx.cmd(&mut self.doc, Command::Insert { parent: self.page, index, nodes: vec![slide_node(id, list.len() + 1)] }).is_err() {
            tx.cancel(&mut self.doc);
            return;
        }
        arrange(&mut self.doc, self.page, &mut tx);
        self.run_tx(tx);
        self.select_slide(id);
    }

    pub fn duplicate_slide(&mut self) {
        if !self.deck_editable() {
            return;
        }
        ui::flush_notes(self);
        crate::canvas::interact::finish_text(self, true);
        let Some(id) = self.active_slide() else { return };
        let Some(clip) = clipboard::capture(&self.doc, &[id]) else { return };
        let Some(mut nodes) = clipboard::instantiate(&mut self.doc, &clip).into_iter().next() else { return };
        let Some(root) = nodes.first_mut() else { return };
        root.name = format!("{} copy", root.name);
        let new_id = root.id;
        let index = self.doc.get(self.page).and_then(|p| p.children.iter().position(|child| *child == id)).map_or(usize::MAX, |i| i + 1);
        let mut tx = Tx::new();
        if tx.cmd(&mut self.doc, Command::Insert { parent: self.page, index, nodes }).is_err() {
            tx.cancel(&mut self.doc);
            return;
        }
        arrange(&mut self.doc, self.page, &mut tx);
        self.run_tx(tx);
        self.select_slide(new_id);
    }

    pub fn delete_slide(&mut self) {
        if !self.deck_editable() {
            return;
        }
        ui::flush_notes(self);
        crate::canvas::interact::finish_text(self, true);
        let list = self.deck_slides();
        let Some(id) = self.active_slide() else { return };
        let index = list.iter().position(|slide| *slide == id).unwrap_or(0);
        let mut tx = Tx::new();
        if tx.cmd(&mut self.doc, Command::Remove { id }).is_err() {
            tx.cancel(&mut self.doc);
            return;
        }
        arrange(&mut self.doc, self.page, &mut tx);
        self.run_tx(tx);
        self.selection.clear();
        self.decks.active = None;
        self.decks.notes_for = None;
        self.decks.notes_draft.clear();
        self.decks.notes_saved.clear();
        let list = self.deck_slides();
        if let Some(id) = list.get(index.min(list.len().saturating_sub(1))).copied() {
            self.select_slide(id);
        }
    }

    pub fn move_slide(&mut self, delta: isize) {
        if !self.deck_editable() {
            return;
        }
        let list = self.deck_slides();
        let Some(id) = self.active_slide() else { return };
        let Some(index) = list.iter().position(|slide| *slide == id) else { return };
        let next = index.saturating_add_signed(delta).min(list.len().saturating_sub(1));
        if next == index {
            return;
        }
        crate::canvas::interact::finish_text(self, true);
        let target = list[next];
        let Some(index) = self.doc.get(self.page).and_then(|page| page.children.iter().position(|child| *child == target)) else { return };
        let mut tx = Tx::new();
        if tx.cmd(&mut self.doc, Command::Reparent { id, parent: self.page, index }).is_err() {
            tx.cancel(&mut self.doc);
            return;
        }
        arrange(&mut self.doc, self.page, &mut tx);
        self.run_tx(tx);
        self.select_slide(id);
    }

    pub fn set_slide_notes(&mut self, id: NodeId, text: String) {
        if !self.deck_editable() || !self.deck_slides().contains(&id) {
            return;
        }
        let mut tx = Tx::new();
        if let Some(slide) = tx.ed.node(&mut self.doc, id).and_then(|node| node.slide.as_mut()) {
            slide.notes = text.clone();
        }
        self.run_tx(tx);
        if self.decks.notes_for == Some(id) {
            self.decks.notes_saved = text.clone();
            self.decks.notes_draft = text;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn older_design_files_load_without_slide_metadata() {
        let mut value = serde_json::to_value(new_document("Legacy")).unwrap();
        value.as_object_mut().unwrap().remove("kind");
        for node in value["nodes"].as_object_mut().unwrap().values_mut() {
            node.as_object_mut().unwrap().remove("slide");
        }
        let doc: Document = serde_json::from_value(value).unwrap();
        assert_eq!(doc.kind, DocumentKind::Design);
        assert!(doc.nodes.values().all(|node| node.slide.is_none()));
    }

    #[test]
    fn deck_metadata_and_notes_round_trip_and_undo() {
        let mut app = App::new(new_document("Talk"));
        let id = app.active_slide().unwrap();
        assert_eq!(app.doc.get(id).unwrap().size, Vec2::new(SLIDE_WIDTH, SLIDE_HEIGHT));
        app.set_slide_notes(id, "Opening remarks".into());
        let restored: Document = serde_json::from_str(&app.doc.to_json().unwrap()).unwrap();
        assert_eq!(restored.kind, DocumentKind::Deck);
        assert_eq!(restored.get(id).unwrap().slide.as_ref().unwrap().notes, "Opening remarks");
        app.undo();
        assert!(app.doc.get(id).unwrap().slide.as_ref().unwrap().notes.is_empty());
        app.redo();
        assert_eq!(app.doc.get(id).unwrap().slide.as_ref().unwrap().notes, "Opening remarks");
    }

    #[test]
    fn slide_order_and_layout_restore_together_on_undo() {
        let mut app = App::new(new_document("Talk"));
        let first = app.active_slide().unwrap();
        app.add_slide();
        let second = app.active_slide().unwrap();
        app.add_slide();
        let third = app.active_slide().unwrap();
        app.move_slide(-2);
        assert_eq!(app.deck_slides(), vec![third, first, second]);
        assert_eq!(app.doc.get(third).unwrap().transform.tx, 0.0);
        app.undo();
        assert_eq!(app.deck_slides(), vec![first, second, third]);
        assert_eq!(app.doc.get(third).unwrap().transform.tx, 2.0 * (SLIDE_WIDTH + SLIDE_GAP));
        app.select_slide(first);
        app.move_slide(2);
        assert_eq!(app.deck_slides(), vec![second, third, first]);
        app.undo();
        assert_eq!(app.deck_slides(), vec![first, second, third]);
    }

    #[test]
    fn duplicate_copies_notes_and_children_with_fresh_ids_as_one_undo() {
        let mut app = App::new(new_document("Talk"));
        let first = app.active_slide().unwrap();
        let child = app.doc.alloc_id();
        app.doc.insert(first, Node::rectangle(child, Rect::new(10.0, 20.0, 50.0, 60.0))).unwrap();
        app.set_slide_notes(first, "Speak slowly".into());
        app.duplicate_slide();
        let copy = app.active_slide().unwrap();
        assert_ne!(copy, first);
        let copied = app.doc.get(copy).unwrap();
        assert_eq!(copied.slide.as_ref().unwrap().notes, "Speak slowly");
        assert_eq!(copied.children.len(), 1);
        assert_ne!(copied.children[0], child);
        assert_eq!(app.doc.get(copied.children[0]).unwrap().parent, Some(copy));
        app.undo();
        assert_eq!(app.deck_slides(), vec![first]);
        assert!(app.doc.get(copy).is_none());
        app.redo();
        assert_eq!(app.deck_slides(), vec![first, copy]);
        app.delete_slide();
        assert_eq!(app.deck_slides(), vec![first]);
        app.undo();
        assert_eq!(app.deck_slides(), vec![first, copy]);
    }

    #[test]
    fn active_slide_follows_descendant_selection_and_empty_deck_can_recover() {
        let mut app = App::new(new_document("Talk"));
        let first = app.active_slide().unwrap();
        let child = app.doc.alloc_id();
        app.doc.insert(first, Node::rectangle(child, Rect::new(0.0, 0.0, 10.0, 10.0))).unwrap();
        app.selection = vec![child];
        assert_eq!(app.active_slide(), Some(first));
        app.delete_slide();
        assert!(app.deck_slides().is_empty());
        assert_eq!(app.active_slide(), None);
        app.add_slide();
        assert_eq!(app.deck_slides().len(), 1);
    }

    #[test]
    fn pending_notes_survive_page_creation_and_switching() {
        let mut app = App::new(new_document("Talk"));
        let original_page = app.page;
        let slide = app.active_slide().unwrap();
        app.select_slide(slide);
        app.decks.notes_draft = "Before adding a page".into();
        let next_page = app.add_page().unwrap();
        assert_eq!(app.doc.get(slide).unwrap().slide.as_ref().unwrap().notes, "Before adding a page");
        app.switch_page(original_page);
        app.select_slide(slide);
        app.decks.notes_draft = "Before switching pages".into();
        app.switch_page(next_page);
        assert_eq!(app.doc.get(slide).unwrap().slide.as_ref().unwrap().notes, "Before switching pages");
        app.switch_page(original_page);
        app.select_slide(slide);
        assert_eq!(app.decks.notes_draft, "Before switching pages");
    }

    #[test]
    fn pending_notes_are_copied_and_restored_with_pages() {
        let mut app = App::new(new_document("Talk"));
        let original_page = app.page;
        let slide = app.active_slide().unwrap();
        app.select_slide(slide);
        app.decks.notes_draft = "Copy these notes".into();
        let copy_page = app.duplicate_page(original_page).unwrap();
        let copy_slide = app.active_slide().unwrap();
        assert_eq!(app.doc.get(copy_slide).unwrap().slide.as_ref().unwrap().notes, "Copy these notes");
        app.select_slide(copy_slide);
        app.decks.notes_draft = "Keep these notes on undo".into();
        assert!(app.delete_page(copy_page));
        app.undo();
        assert_eq!(app.doc.get(copy_slide).unwrap().slide.as_ref().unwrap().notes, "Keep these notes on undo");
    }
}
