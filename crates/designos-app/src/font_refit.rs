//! Re-measures text the app itself sized with a stand-in font, once the real face arrives.
//!
//! When a user edits a text whose family is still being fetched, the app measures the new box with
//! the stand-in font. The face then arrives and the glyphs are drawn with it, so that box, the
//! Width and Height fields and every auto-layout frame that hugs the text keep the old numbers.
//!
//! The rule that bounds this module: a text box that came from the file is never replaced by the
//! app's own measure, unless the user edits that text or its width. A document that is only opened
//! is never re-measured, whatever is fetched for it. So the pass covers only a recorded set: texts
//! whose box a user edit committed while a face of theirs was missing in this session
//! ([`App::note_text_measures`]). [`App::poll_fonts`] hands the arrived families to
//! [`App::start_text_refit`], which queues the recorded texts that use them, and
//! [`App::step_text_refit`] re-fits them and lays out their ancestors, each once per slice.
//!
//! This is not an edit: nothing enters the undo history. A drag or a text session in progress holds
//! the pass, because the next frame of a drag puts its layers back as they were; the set is kept and
//! the pass goes on when the gesture ends.

use crate::app::App;
use crate::edit::Edit;
use crate::layout;
use designos_doc::{Node, NodeId, NodeKind, TextAutoResize, TextProps};
use designos_fonts::{scripts_in, Script};
use std::collections::{BTreeMap, BTreeSet};
use std::time::{Duration, Instant};

/// How long one frame spends re-fitting texts before it leaves the rest for the next one.
const FRAME_BUDGET: Duration = Duration::from_millis(8);

/// What the re-fit knows about one document: the texts the app measured with a stand-in, and the
/// ones queued to be measured again.
pub struct TextRefit {
    /// The document the sets belong to; another document on screen leaves them alone.
    doc: u64,
    /// Texts whose box the app measured while a face of theirs was missing. A text from the file is
    /// never in it.
    stand_in: BTreeSet<NodeId>,
    /// Texts to re-measure, in id order.
    queue: BTreeSet<NodeId>,
    /// The faces that arrived and whose pass is not over. A text edited meanwhile may have been
    /// measured before they arrived, so it joins the queue.
    arrived: Option<Arrived>,
    /// Time one slice of the pass may take.
    pub(crate) budget: Duration,
}

impl Default for TextRefit {
    fn default() -> Self {
        Self { doc: 0, stand_in: BTreeSet::new(), queue: BTreeSet::new(), arrived: None, budget: FRAME_BUDGET }
    }
}

impl TextRefit {
    /// Makes the sets those of document `doc`, dropping another document's.
    fn adopt(&mut self, doc: u64) {
        if self.doc != doc {
            *self = Self { doc, budget: self.budget, ..Self::default() };
        }
    }
}

/// The families of the faces that arrived, and the complex scripts whose fallback font is among them.
#[derive(Default)]
struct Arrived {
    /// Lowercase family names: the one the document asked for and the one the font holds.
    families: Vec<String>,
    scripts: BTreeSet<Script>,
}

impl Arrived {
    fn new(families: &[String]) -> Self {
        let families: Vec<String> = families.iter().map(|f| f.to_lowercase()).collect();
        // A text in Inter never names the Noto face that draws its Arabic: match it by its characters.
        let scripts = Script::ALL.into_iter().filter(|s| families.iter().any(|f| f.eq_ignore_ascii_case(s.family()))).collect();
        Self { families, scripts }
    }

    fn merge(&mut self, other: Arrived) {
        self.families.extend(other.families);
        self.scripts.extend(other.scripts);
    }

    /// Whether a new face changes how `t` is shaped: its own family or a run's, or a script it uses.
    fn shapes(&self, t: &TextProps) -> bool {
        let named = |family: &str| self.families.iter().any(|f| f.eq_ignore_ascii_case(family));
        named(&t.style.font_family)
            || t.runs.iter().any(|r| named(&r.style.font_family))
            || (!self.scripts.is_empty() && !t.content.is_ascii() && scripts_in(&t.content).intersection(&self.scripts).next().is_some())
    }
}

/// The text of `id`, when it is an auto-sized one.
fn auto_sized(doc: &designos_doc::Document, id: NodeId) -> Option<&TextProps> {
    match &doc.get(id)?.kind {
        NodeKind::Text(t) if t.auto_resize != TextAutoResize::None => Some(t),
        _ => None,
    }
}

impl App {
    /// Records the texts a user edit just committed whose box the app measured with a stand-in:
    /// the edit changed the text, its box is exactly what the app measures, and a face of the text
    /// is not here yet. `before` is the state of every node the edit touched. A text from the file
    /// that the edit left alone, or only moved, is never recorded.
    pub(crate) fn note_text_measures(&mut self, before: &BTreeMap<NodeId, Node>) {
        let mut found: Vec<(NodeId, bool)> = Vec::new();
        for (id, was) in before {
            let Some(now) = self.doc.get(*id) else { continue };
            let Some(t) = auto_sized(&self.doc, *id) else { continue };
            if now.size == was.size && now.kind == was.kind {
                continue;
            }
            // a face arrived while a gesture held the pass: the box may be from before it
            let waiting = self.fonts.refit.arrived.as_ref().is_some_and(|a| a.shapes(t));
            if waiting || (self.fonts.uses_stand_in(t) && layout::fit_text(t, now.size) == now.size) {
                found.push((*id, waiting));
            }
        }
        if found.is_empty() {
            return;
        }
        let refit = &mut self.fonts.refit;
        refit.adopt(self.doc.runtime_id());
        for (id, waiting) in found {
            refit.stand_in.insert(id);
            if waiting {
                refit.queue.insert(id);
            }
        }
    }

    /// Queues the recorded texts that use one of `families` (the faces a load just registered).
    pub(crate) fn start_text_refit(&mut self, families: &[String]) {
        if families.is_empty() {
            return;
        }
        let arrived = Arrived::new(families);
        let refit = &mut self.fonts.refit;
        refit.adopt(self.doc.runtime_id());
        let doc = &self.doc;
        refit.queue.extend(refit.stand_in.iter().copied().filter(|id| auto_sized(doc, *id).is_some_and(|t| arrived.shapes(t))));
        refit.arrived.get_or_insert_with(Arrived::default).merge(arrived);
    }

    /// Texts of the open document are waiting to be re-fitted: the caller keeps repainting until
    /// they are done.
    pub fn text_refit_pending(&self) -> bool {
        self.fonts.refit.doc == self.doc.runtime_id() && !self.fonts.refit.queue.is_empty()
    }

    /// A drag, a text session or a scrub is in progress, so the pass waits for it.
    pub fn text_refit_held(&self) -> bool {
        self.canvas.text.is_some() || !matches!(self.canvas.inter, crate::canvas::Interaction::None) || self.scrub.is_some()
    }

    /// Re-fits queued texts for as long as one frame may take, then lays out what hugs them.
    pub(crate) fn step_text_refit(&mut self) {
        if self.text_refit_held() {
            return;
        }
        if !self.text_refit_pending() {
            // the pass is over, or there was nothing to do: an edit from now on is measured with what is here
            self.fonts.refit.arrived = None;
            return;
        }
        let mut refit = std::mem::take(&mut self.fonts.refit);
        let start = Instant::now();
        let mut ed = Edit::new();
        let mut parents: BTreeMap<NodeId, NodeId> = BTreeMap::new();
        let mut fitted = 0;
        let mut moved = false;
        // the time is checked before a text, not after its layout; one text always goes, so the pass advances
        while fitted == 0 || start.elapsed() < refit.budget {
            let Some(id) = refit.queue.pop_first() else { break };
            fitted += 1;
            if self.refit_text(id, &mut ed) {
                moved = true;
                if let Some(parent) = self.doc.get(id).and_then(|n| n.parent) {
                    parents.entry(parent).or_insert(id);
                }
            }
            if auto_sized(&self.doc, id).is_some_and(|t| !self.fonts.uses_stand_in(t)) {
                refit.stand_in.remove(&id);
            }
        }
        // Each frame that hugs a fitted text is laid out once, deepest first, with all its texts in place.
        let mut order: Vec<(usize, NodeId, NodeId)> = parents.into_iter().map(|(p, child)| (self.depth(p), p, child)).collect();
        order.sort_by_key(|(depth, _, _)| std::cmp::Reverse(*depth));
        for (_, _, child) in order {
            layout::relayout_up(&mut self.doc, &mut ed, child);
        }
        if moved {
            // the boxes moved behind the canvas's back, like a deferred relayout
            self.changed();
            self.canvas.tiles.invalidate_all();
        }
        if refit.queue.is_empty() {
            refit.arrived = None;
        }
        self.fonts.refit = refit;
    }

    /// Re-measures one text and writes its box through `ed`, which is never committed: the history
    /// stays as it was. Returns whether the box changed.
    fn refit_text(&mut self, id: NodeId, ed: &mut Edit) -> bool {
        let Some(n) = self.doc.get(id) else { return false };
        let Some(t) = auto_sized(&self.doc, id) else { return false };
        let size = layout::fit_text(t, n.size);
        if (size.x - n.size.x).abs() <= 1e-3 && (size.y - n.size.y).abs() <= 1e-3 {
            return false;
        }
        if let Some(n) = ed.node(&mut self.doc, id) {
            n.size = size;
        }
        true
    }

    /// Number of ancestors of `id`.
    fn depth(&self, id: NodeId) -> usize {
        std::iter::successors(self.doc.get(id).and_then(|n| n.parent), |p| self.doc.get(*p).and_then(|n| n.parent)).count()
    }
}

#[cfg(test)]
mod tests;
