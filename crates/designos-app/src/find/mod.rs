//! Find and replace (Ctrl+F), the layers name filter and the selection commands
//! built on `designos_doc::find`.

pub mod filter;
pub mod panel;
#[cfg(test)]
mod replace_tests;
mod select;

use crate::app::{App, View};
use crate::edit::Tx;
use crate::layout;
use crate::util::world_aabb;
use designos_doc::find::{self, FindOptions, Match, MatchField, Matcher, Scope};
use designos_doc::{NodeId, NodeKind};

/// What the search covers.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SearchIn {
    #[default]
    Page,
    AllPages,
    Selection,
}

impl SearchIn {
    pub fn label(self) -> &'static str {
        match self {
            SearchIn::Page => "This page",
            SearchIn::AllPages => "All pages",
            SearchIn::Selection => "Selection",
        }
    }

    fn next(self) -> Self {
        match self {
            SearchIn::Page => SearchIn::AllPages,
            SearchIn::AllPages => SearchIn::Selection,
            SearchIn::Selection => SearchIn::Page,
        }
    }
}

/// Stable identity of a match across re-runs of the search.
type MatchKey = (NodeId, MatchField, usize);

#[derive(Default)]
pub struct FindState {
    pub open: bool,
    pub query: String,
    pub replacement: String,
    /// The replace row is shown (chevron toggle).
    pub show_replace: bool,
    pub opts: FindOptions,
    pub search_in: SearchIn,
    /// The layers searched when `search_in` is [`SearchIn::Selection`], frozen when chosen
    /// (navigating to a result changes the selection).
    pub roots: Vec<NodeId>,
    pub results: Vec<Match>,
    /// Why the query cannot run (an invalid regex); empty queries are not errors.
    pub error: Option<String>,
    pub current: Option<usize>,
    /// Focus the search field on the next frame.
    pub focus: bool,
    pub scroll: f32,
    /// The result the list was last scrolled to.
    pub scrolled_to: Option<usize>,
    /// Inputs of the last search, and when it ran.
    ran: Option<(String, FindOptions, SearchIn, NodeId, Vec<NodeId>)>,
    ran_at: f64,
}

impl FindState {
    fn key_of(&self, i: usize) -> Option<MatchKey> {
        self.results.get(i).map(|m| (m.node, m.field, m.range.start))
    }
}

/// The document scope the current search runs over.
pub fn doc_scope(app: &App) -> Scope {
    match app.find.search_in {
        SearchIn::Page => Scope::Page(app.page),
        SearchIn::AllPages => Scope::Document,
        SearchIn::Selection => Scope::Nodes(app.find.roots.clone()),
    }
}

impl App {
    /// Opens the Find and replace panel (Ctrl+F) with the search field focused.
    pub fn open_find(&mut self) {
        self.find.open = true;
        self.find.focus = true;
        self.ui.left_collapsed = false;
        if self.ui.rail.is_some_and(|r| r != crate::app::RailItem::File) {
            self.ui.rail = Some(crate::app::RailItem::File);
        }
        if self.find.search_in == SearchIn::Selection && self.find.roots.is_empty() {
            self.find.search_in = SearchIn::Page;
        }
    }

    pub fn close_find(&mut self) {
        self.find.open = false;
        self.find.focus = false;
    }

    /// Cycles the search scope; "Selection" freezes the currently selected layers.
    pub fn cycle_find_scope(&mut self) {
        let mut next = self.find.search_in.next();
        if next == SearchIn::Selection {
            if self.selection.is_empty() {
                next = next.next();
            } else {
                self.find.roots = self.selection.clone();
            }
        }
        self.find.search_in = next;
        self.find_refresh(true);
    }

    /// Re-runs the search when its inputs changed, or every so often to follow edits.
    pub fn find_refresh(&mut self, force: bool) {
        let inputs = (self.find.query.clone(), self.find.opts, self.find.search_in, self.page, self.find.roots.clone());
        let stale_after = if self.doc.nodes.len() > 5000 { 1.0 } else { 0.2 };
        let due = self.now - self.find.ran_at > stale_after;
        if !force && !due && self.find.ran.as_ref() == Some(&inputs) {
            return;
        }
        let keep = self.find.current.and_then(|i| self.find.key_of(i));
        let (results, error) = match Matcher::new(&self.find.query, self.find.opts) {
            Ok(m) => (find::find_with(&self.doc, &doc_scope(self), &m), None),
            Err(e) => (Vec::new(), (!e.is_empty()).then_some(e)),
        };
        self.find.results = results;
        self.find.error = error;
        self.find.current = keep
            .and_then(|k| self.find.results.iter().position(|m| (m.node, m.field, m.range.start) == k))
            .or(self.find.current.filter(|i| *i < self.find.results.len()));
        self.find.ran = Some(inputs);
        self.find.ran_at = self.now;
    }

    /// Selects result `i`, switching page if needed, and brings it into view.
    pub fn find_goto(&mut self, i: usize) {
        let Some(m) = self.find.results.get(i).cloned() else { return };
        if let Some(pg) = self.doc.page_of(m.node) {
            if pg != self.page {
                crate::decks::ui::flush_notes(self);
                self.page = pg;
                self.canvas.reset_interaction();
            }
        }
        self.find.current = Some(i);
        self.select_only(m.node);
        self.reveal(m.node);
    }

    /// Next (`dir = 1`) or previous (`dir = -1`) result, wrapping around.
    pub fn find_step(&mut self, dir: i32) {
        self.find_refresh(false);
        let n = self.find.results.len();
        if n == 0 {
            return;
        }
        let i = match self.find.current {
            Some(c) => (c as i64 + dir as i64).rem_euclid(n as i64) as usize,
            None if dir < 0 => n - 1,
            None => 0,
        };
        self.find_goto(i);
    }

    /// Replaces the current result (or the first one) and moves on to the next.
    pub fn find_replace_one(&mut self) {
        self.find_refresh(true);
        let Some(i) = self.find.current.or((!self.find.results.is_empty()).then_some(0)) else { return };
        let m = self.find.results[i].clone();
        if self.replace_matches(std::slice::from_ref(&m)) == 0 {
            return;
        }
        self.find_refresh(true);
        let n = self.find.results.len();
        if n > 0 {
            self.find_goto(i.min(n - 1));
        } else {
            self.find.current = None;
        }
    }

    /// Replaces every result, as one undo step. Returns the number of replacements.
    pub fn find_replace_all(&mut self) -> usize {
        self.find_refresh(true);
        let ms = self.find.results.clone();
        let n = self.replace_matches(&ms);
        self.find.current = None;
        self.find_refresh(true);
        if n > 0 {
            self.show_toast(format!("Replaced {n} {}", if n == 1 { "match" } else { "matches" }), false);
        }
        n
    }

    /// Applies `find::replace` as one undoable step and refits the edited text layers.
    fn replace_matches(&mut self, ms: &[Match]) -> usize {
        if ms.is_empty() {
            return 0;
        }
        let cmd = find::replace(&self.doc, ms, &self.find.replacement);
        let mut tx = Tx::new();
        if let Err(e) = tx.cmd(&mut self.doc, cmd) {
            self.show_toast(format!("Replace failed: {e}"), true);
            return 0;
        }
        let mut touched: Vec<NodeId> = ms.iter().map(|m| m.node).collect();
        touched.sort();
        touched.dedup();
        for id in touched {
            let Some(n) = tx.ed.node(&mut self.doc, id) else { continue };
            let fitted = match &n.kind {
                NodeKind::Text(t) => Some(layout::fit_text(t, n.size)),
                _ => None,
            };
            if let Some(size) = fitted {
                n.size = size;
            }
            layout::relayout_around(&mut self.doc, &mut tx.ed, id);
        }
        self.run_tx(tx);
        ms.len()
    }

    /// Centers the canvas on `id`, zooming only as far as needed to show it.
    pub fn reveal(&mut self, id: NodeId) {
        let b = world_aabb(&self.doc, id);
        let size = self.canvas_rect.size();
        let cur = self.view.zoom;
        let fit = View::fit(b, size).zoom;
        let fits_now = b.w * cur <= size.x * 0.8 && b.h * cur <= size.y * 0.8;
        let zoom = if fits_now { cur.max(fit.min(1.0)) } else { fit };
        let z = zoom.clamp(crate::theme::ZOOM_MIN, crate::theme::ZOOM_MAX);
        self.view = View { zoom: z, offset: designos_doc::Vec2::new(b.x + b.w / 2.0 - size.x / 2.0 / z, b.y + b.h / 2.0 - size.y / 2.0 / z) };
        self.changed();
    }
}
