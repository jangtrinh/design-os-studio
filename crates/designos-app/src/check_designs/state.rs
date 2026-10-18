//! Panel state: what is being checked and the cached report.

use super::{audit_with_libraries, LibrarySources, Report, Scope};
use crate::app::App;
use designos_doc::{LibraryRef, REMOTE_LIBRARY_PREFIX};

impl LibrarySources {
    /// Collects pinned snapshots for every enabled library in `app.doc.library.enabled`, plus the
    /// display names of any enabled library whose snapshot could not be read.
    pub(super) fn collect(app: &mut App) -> (Self, Vec<String>) {
        let enabled = app.doc.library.enabled.clone();
        let mut libs = Vec::new();
        let mut unreadable = Vec::new();
        for r in enabled.into_iter().filter(|r| !r.id.starts_with(REMOTE_LIBRARY_PREFIX)) {
            match app.snapshot_of(&r.id, r.version) {
                Some(snap) => libs.push((r, snap)),
                None => unreadable.push(r.name),
            }
        }
        (Self { libs }, unreadable)
    }
}

/// Check designs panel state. The scope is fixed when the panel opens, so selecting a finding's
/// layer does not change what is being listed.
#[derive(Default)]
pub struct CheckDesignsState {
    pub open: bool,
    pub scope: Option<Scope>,
    /// "Selection (3 layers)" or "Page 1", shown under the title.
    pub scope_label: String,
    pub report: Report,
    /// `(document revision, scope, enabled libraries)` the cached `report` was computed for.
    audited: Option<(u64, Scope, Vec<LibraryRef>)>,
}

impl CheckDesignsState {
    /// Forces the next frame to audit again.
    pub fn invalidate(&mut self) {
        self.audited = None;
    }
}

impl App {
    /// Opens the panel on the selected layers, or on the whole page when nothing is selected.
    pub fn open_check_designs(&mut self) {
        let layers: Vec<_> = self.selection.iter().copied().filter(|id| self.doc.get(*id).is_some()).collect();
        let (scope, label) = if layers.is_empty() {
            let name = self.doc.get(self.page).map(|p| p.name.clone()).unwrap_or_default();
            (Scope::Page(self.page), format!("Page: {name}"))
        } else {
            let label = if layers.len() == 1 { "Selection (1 layer)".to_string() } else { format!("Selection ({} layers)", layers.len()) };
            (Scope::Layers(layers), label)
        };
        self.check_designs = CheckDesignsState { open: true, scope: Some(scope), scope_label: label, ..CheckDesignsState::default() };
        self.cmdk.open = false;
    }

    /// Re-audits when the document or the scope changed since the cached report. While the user is
    /// mid-gesture (`interacting`) an existing report is kept, so a drag does not audit every frame.
    pub fn refresh_check_designs(&mut self, interacting: bool) {
        // A page scope whose page was deleted would list nothing for ever; follow the page on screen.
        if matches!(self.check_designs.scope, Some(Scope::Page(p)) if self.doc.get(p).is_none()) {
            let name = self.doc.get(self.page).map(|p| p.name.clone()).unwrap_or_default();
            self.check_designs.scope = Some(Scope::Page(self.page));
            self.check_designs.scope_label = format!("Page: {name}");
            self.check_designs.invalidate();
        }
        let Some(scope) = self.check_designs.scope.clone() else { return };
        let key = (self.canvas.tiles.rev(), scope, self.doc.library.enabled.clone());
        if self.check_designs.audited.as_ref() == Some(&key) || (interacting && self.check_designs.audited.is_some()) {
            return;
        }
        let (libs, unreadable) = LibrarySources::collect(self);
        let mut report = audit_with_libraries(&self.doc, &key.1, &libs);
        report.unreadable_libraries = unreadable;
        self.check_designs.report = report;
        self.check_designs.audited = Some(key);
    }
}
