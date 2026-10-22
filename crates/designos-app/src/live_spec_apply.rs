//! Hot-applying an agent's `live-spec.json`: a rewrite of the file must not stack another copy of the design on the
//! page. An unchanged rewrite changes nothing, and a changed one replaces the layer the file's earlier content made.

use crate::app::App;
use designos_doc::{NodeId, NodeSpec};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::Path;

/// The layer one live spec file last put on the canvas, and the content it came from.
#[derive(Clone, Debug)]
pub struct LiveSpecOutput {
    content_hash: u64,
    root: NodeId,
    /// Name and page of the root when it was made. Layer ids restart in every document, so an id alone could
    /// name an unrelated layer after another document was opened; the layer only counts as this output while it
    /// still carries the name and sits on the page it was made with.
    name: String,
    page: NodeId,
}

fn content_hash(raw: &str) -> u64 {
    let mut h = DefaultHasher::new();
    raw.hash(&mut h);
    h.finish()
}

impl App {
    /// The root layer of `output` when it is still in the open document, as it was made.
    fn live_output_root(&self, output: &LiveSpecOutput) -> Option<NodeId> {
        let root = self.doc.get(output.root)?;
        (root.name == output.name && root.parent == Some(output.page)).then_some(output.root)
    }

    /// Applies the `raw` content (parsed as `spec`) of the live spec file at `path` to the active page.
    ///
    /// - The same content as the file's last application, while its output is still on the canvas: nothing happens,
    ///   with no undo step, no toast and no refit.
    /// - Other content: the new design takes the earlier output's place in one undo step.
    /// - No earlier output in this document: the design is added at the end of the page.
    pub fn apply_live_spec(&mut self, path: &Path, raw: &str, spec: &NodeSpec, label: &str) {
        let hash = content_hash(raw);
        let earlier = self.specs.live_outputs.get(path).and_then(|o| self.live_output_root(o).map(|root| (o.content_hash, root)));
        if earlier.is_some_and(|(earlier_hash, _)| earlier_hash == hash) {
            return;
        }
        let page = self.page;
        if let Some(root) = self.apply_node_spec_replacing(spec, label, earlier.map(|(_, root)| root)) {
            let name = self.doc.get(root).map(|n| n.name.clone()).unwrap_or_default();
            self.specs.live_outputs.insert(path.to_path_buf(), LiveSpecOutput { content_hash: hash, root, name, page });
        }
    }
}
