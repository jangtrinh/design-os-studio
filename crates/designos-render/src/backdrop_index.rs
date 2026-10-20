//! What a lint pass keeps of one page to render many small windows of it ([`crate::render_backdrop_indexed`]):
//! the scene index, and the top-level layers in a tree by the box they paint in, so that finding the
//! layers over a window does not walk every layer of the page.

use crate::index::B;
use crate::SceneIndex;
use designos_doc::lint::rect_tree::{RectEntry, RectTree};
use designos_doc::{Document, NodeId, Rect};

/// Slack, in page units, around a window when asking the tree for the layers over it. The tree
/// compares with `Rect` arithmetic and the scene index with its own edges; the layers that really
/// miss the window are skipped afterwards, one by one, by the scene index.
const WINDOW_SLACK: f32 = 0.5;

pub struct BackdropIndex {
    scene: SceneIndex,
    children: Box<[NodeId]>,
    tops: RectTree,
}

impl BackdropIndex {
    /// The index of `page` of `doc`, for as long as `doc` is not edited.
    pub fn new(doc: &Document, page: NodeId) -> Self {
        let mut scene = SceneIndex::new();
        scene.ensure(doc, page, 1);
        let children: Box<[NodeId]> = doc.get(page).map(|p| p.children.as_slice().into()).unwrap_or_default();
        let entries = children
            .iter()
            .enumerate()
            .filter_map(|(pos, id)| scene.bounds_of(*id).map(|b| RectEntry { pos, rect: Rect::new(b.0, b.1, b.2 - b.0, b.3 - b.1) }))
            .collect();
        BackdropIndex { scene, children, tops: RectTree::build(entries) }
    }

    pub(crate) fn scene(&self) -> &SceneIndex {
        &self.scene
    }

    /// The top-level layers that may touch `win`, back to front. A page with a top-level mask keeps
    /// all of its layers, because a mask acts on the layers drawn after it.
    pub(crate) fn top_level_near(&self, win: B) -> Vec<NodeId> {
        if self.scene.has_top_mask() {
            return self.children.to_vec();
        }
        let query = Rect::new(win.0 - WINDOW_SLACK, win.1 - WINDOW_SLACK, win.2 - win.0 + 2.0 * WINDOW_SLACK, win.3 - win.1 + 2.0 * WINDOW_SLACK);
        let mut at = Vec::new();
        self.tops.query(&query, &mut at);
        at.sort_unstable();
        at.into_iter().map(|pos| self.children[pos]).collect()
    }
}
