//! Re-measures text the app itself sized with a stand-in font, once the real face arrives.
//!
//! When a user edits a text whose family is still being fetched, the app measures the new box with
//! the stand-in font. The face then arrives and the glyphs are drawn with it, so that box, the
//! Width and Height fields and every auto-layout frame that hugs the text keep the old numbers.
//!
//! The rule that bounds this module: a text box that came from the file is never replaced by the
//! app's own measure, unless the user edits that text or its width. A document that is only opened
//! is never re-measured, whatever is fetched for it. So the pass covers only a recorded set: texts
//! whose box a user edit measured while a face of theirs was missing in this session, each with the
//! box that edit committed and what the measure read of the text then ([`App::note_text_measures`]).
//! A colour or a move that leaves the box alone is not a measure, and a width a layout gave the text
//! is not an edit of it. [`App::poll_fonts`] hands the arrived families to [`App::start_text_refit`],
//! which queues the recorded texts that use them and still hold both the committed box and the
//! committed text. An undo puts the file's text back, and with it the file's box when the edit moved
//! it: that text is the file's again, and its record is dropped. The box alone cannot tell the two
//! apart, because an alignment, an underline or a letter in a wrapped text commits the file's own
//! box. [`App::step_text_refit`] re-fits the queue, then lays out the frames that hug the fitted texts.
//!
//! This is not an edit: nothing enters the undo history. A drag or a text session in progress holds
//! the pass, because the next frame of a drag puts its layers back as they were; the set is kept and
//! the pass goes on when the gesture ends. A slice reads the clock before each text and before each
//! frame it lays out, and what it leaves waits for the next one.

use crate::app::App;
use crate::edit::Edit;
use crate::layout;
use designos_doc::{Node, NodeId, NodeKind, TextAutoResize, TextProps, Vec2};
use designos_fonts::{scripts_in, FontRequest, Script};
use std::cmp::Reverse;
use std::collections::{BTreeMap, BTreeSet};
use std::time::{Duration, Instant};

/// How long one frame spends re-fitting texts before it leaves the rest for the next one.
const FRAME_BUDGET: Duration = Duration::from_millis(8);

/// What the re-fit knows about one document: the texts the app measured with a stand-in, and the
/// ones queued to be measured again.
pub struct TextRefit {
    /// The document the sets belong to; another document on screen leaves them alone.
    doc: u64,
    /// Texts whose box the app measured while a face of theirs was missing, each with the box the
    /// user's edit committed and what the measure read of the text then ([`measure_key`]). The text is
    /// fitted only while it still holds both. A text from the file is never in it.
    stand_in: BTreeMap<NodeId, (Vec2, TextProps)>,
    /// Texts to re-measure, in id order.
    queue: BTreeSet<NodeId>,
    /// The frames that hug a fitted text and are not laid out yet, deepest first, each with one text
    /// to start from.
    parents: BTreeMap<(Reverse<usize>, NodeId), NodeId>,
    /// The faces that arrived and whose pass is not over. A text edited meanwhile may have been
    /// measured before they arrived, so it joins the queue.
    arrived: Option<Arrived>,
    /// Time one slice of the pass may take.
    pub(crate) budget: Duration,
}

impl Default for TextRefit {
    fn default() -> Self {
        Self { doc: 0, stand_in: BTreeMap::new(), queue: BTreeSet::new(), parents: BTreeMap::new(), arrived: None, budget: FRAME_BUDGET }
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

/// What a measure reads of a text: the text with its paints left out. A colour changes how a text is
/// drawn, never how large it is.
fn measure_key(t: &TextProps) -> TextProps {
    let mut key = t.clone();
    key.style.fills.clear();
    for run in &mut key.runs {
        run.style.fills.clear();
    }
    key
}

/// The text of `id`, when this app measures its box: a hugging text and a wrapped one. `fit_text`
/// leaves a fixed or truncated box as it is, so there is nothing to fit in it.
fn auto_sized(doc: &designos_doc::Document, id: NodeId) -> Option<&TextProps> {
    match &doc.get(id)?.kind {
        NodeKind::Text(t) if matches!(t.auto_resize, TextAutoResize::WidthAndHeight | TextAutoResize::Height) => Some(t),
        _ => None,
    }
}

impl App {
    /// Records the texts a user edit just committed whose box the app measured with a stand-in:
    /// the edit left a box that is exactly what the app measures, and a face of the text is not
    /// here yet. The box and the text's measure key are kept with the record: the pass fits the text
    /// only while it still holds both. `before` is the state of every node the edit touched. A text
    /// from the file that the edit left alone is never recorded, and neither is one that only a
    /// colour or a move touched (a colour change that re-measures the text, as the inspector's does,
    /// is a measure). While a gesture holds the pass, only a text the edit itself changed joins it:
    /// one whose box a layout changed was not measured by this edit.
    pub(crate) fn note_text_measures(&mut self, before: &BTreeMap<NodeId, Node>) {
        let fonts = &self.fonts;
        // whether a set of faces has a stand-in is asked once per set, not once per text
        let mut asked: BTreeMap<BTreeSet<FontRequest>, bool> = BTreeMap::new();
        let mut uses_stand_in = |t: &TextProps| *asked.entry(designos_fonts::text_faces(t)).or_insert_with(|| fonts.uses_stand_in(t));
        let mut found: Vec<(NodeId, Vec2, TextProps, bool)> = Vec::new();
        let mut obsolete: Vec<NodeId> = Vec::new();
        for (id, was) in before {
            let (Some(now), Some(t)) = (self.doc.get(*id), auto_sized(&self.doc, *id)) else { continue };
            let NodeKind::Text(was_t) = &was.kind else { continue };
            let edited = t != was_t && measure_key(t) != measure_key(was_t);
            // A colour or a move that leaves the box alone is not a measure. Nor is a width that a layout or a
            // resize gave a wrapped text: this edit's measure keeps the width and finds the height.
            let width_given = t.auto_resize == TextAutoResize::Height && now.size.x != was.size.x;
            if !edited && (now.size == was.size || width_given) {
                continue;
            }
            // a face arrived while a gesture held the pass: the box of an edited text may be from before it.
            // A text whose box only a layout changed was not measured by this edit, so it does not count.
            let waiting = edited && fonts.refit.arrived.as_ref().is_some_and(|a| a.shapes(t));
            if waiting || (uses_stand_in(t) && layout::fit_text(t, now.size) == now.size) {
                found.push((*id, now.size, measure_key(t), waiting));
            } else {
                // the edit left a box that is not a stand-in measure: an earlier record of this text is out of date
                obsolete.push(*id);
            }
        }
        if found.is_empty() && obsolete.is_empty() {
            return;
        }
        let doc = self.doc.runtime_id();
        let refit = &mut self.fonts.refit;
        if !found.is_empty() {
            refit.adopt(doc);
        }
        if refit.doc != doc {
            return;
        }
        for id in obsolete {
            refit.stand_in.remove(&id);
        }
        for (id, committed, key, waiting) in found {
            refit.stand_in.insert(id, (committed, key));
            if waiting {
                refit.queue.insert(id);
            }
        }
    }

    /// Queues the recorded texts that use one of `families` (the faces a load just registered) and
    /// still hold the box and the text their edit committed. One that holds another is the file's
    /// again (an undo put its text back) or was laid out since: it is left as it is, and its record
    /// is dropped.
    pub(crate) fn start_text_refit(&mut self, families: &[String]) {
        if families.is_empty() {
            return;
        }
        let arrived = Arrived::new(families);
        let refit = &mut self.fonts.refit;
        refit.adopt(self.doc.runtime_id());
        let mut dropped = Vec::new();
        for (id, (committed, key)) in &refit.stand_in {
            let Some(t) = auto_sized(&self.doc, *id) else { continue };
            if !arrived.shapes(t) {
                continue;
            }
            if self.doc.get(*id).is_some_and(|n| n.size == *committed) && measure_key(t) == *key {
                refit.queue.insert(*id);
            } else {
                dropped.push(*id);
            }
        }
        for id in dropped {
            refit.stand_in.remove(&id);
        }
        refit.arrived.get_or_insert_with(Arrived::default).merge(arrived);
    }

    /// Texts or frames of the open document are waiting to be re-fitted or laid out: the caller
    /// keeps repainting until they are done.
    pub fn text_refit_pending(&self) -> bool {
        let refit = &self.fonts.refit;
        refit.doc == self.doc.runtime_id() && !(refit.queue.is_empty() && refit.parents.is_empty())
    }

    /// A drag, a text session or a scrub is in progress, so the pass waits for it.
    pub fn text_refit_held(&self) -> bool {
        self.canvas.text.is_some() || !matches!(self.canvas.inter, crate::canvas::Interaction::None) || self.scrub.is_some()
    }

    /// Re-fits queued texts for as long as one frame may take, then lays out the frames that hug
    /// them, as far as the frame's time goes. The clock is read before each text and before each
    /// frame; the first of either always goes, so the pass advances. What is left waits for the next slice.
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
        let budget = refit.budget;
        let in_time = |worked: usize| worked == 0 || start.elapsed() < budget;
        let mut ed = Edit::new();
        let mut worked = 0;
        while in_time(worked) {
            let Some(id) = refit.queue.pop_first() else { break };
            worked += 1;
            // the text is the app's to fit only while it holds the box and the text its edit committed
            let holds = refit.stand_in.get(&id).is_some_and(|(committed, key)| {
                self.doc.get(id).is_some_and(|n| n.size == *committed) && auto_sized(&self.doc, id).is_some_and(|t| measure_key(t) == *key)
            });
            if !holds {
                refit.stand_in.remove(&id);
                continue;
            }
            if self.refit_text(id, &mut ed) {
                if let Some(parent) = self.doc.get(id).and_then(|n| n.parent) {
                    refit.parents.entry((Reverse(self.depth(parent)), parent)).or_insert(id);
                }
            }
            if auto_sized(&self.doc, id).is_some_and(|t| !self.fonts.uses_stand_in(t)) {
                refit.stand_in.remove(&id);
            }
        }
        // Once every text is in place, each frame that hugs one is laid out once, deepest first.
        if refit.queue.is_empty() {
            while in_time(worked) {
                let Some((_, from)) = refit.parents.pop_first() else { break };
                layout::relayout_up(&mut self.doc, &mut ed, from);
                worked += 1;
            }
        }
        // The pass's own writes move boxes a user's edit committed (the text it fitted, a text a layout
        // resized): the record follows the box, so a text that still needs another face is fitted again.
        for (id, was) in ed.before_nodes() {
            if let (Some((committed, _)), Some(now)) = (refit.stand_in.get_mut(id), self.doc.get(*id)) {
                if *committed == was.size {
                    *committed = now.size;
                }
            }
        }
        if !ed.before_nodes().is_empty() {
            // the boxes moved behind the canvas's back, like a deferred relayout
            self.changed();
            self.canvas.tiles.invalidate_all();
            // and behind the history's: an edit still open (the colour picker's) must not replay them as its own
            self.doc.note_layout_written_outside_history();
            // and the recorded steps before it no longer hold the geometry around what it moved
            self.hist.forget_layout_recorded();
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
