//! Panel state: what is being checked, session dismissals, and the cached report.

use super::{audit_step, Finding, LibrarySources, Report, Rule, ScanCursor, Scope, SLICE_BUDGET};
use crate::app::App;
use crate::edit::Tx;
use designos_doc::{CheckDesignsSettings, Command, ContrastLevel, IgnoreKey, LibraryRef, REMOTE_LIBRARY_PREFIX};
use std::collections::{BTreeSet, HashMap};
use std::time::Duration;

impl LibrarySources {
    /// Collects pinned snapshots for every enabled library in `app.doc.library.enabled`, plus the
    /// display names of any enabled library whose snapshot could not be read and any disabled
    /// library referenced by `app.doc.library.external`.
    pub(super) fn collect(app: &mut App) -> (Self, Vec<String>) {
        let enabled = app.doc.library.enabled.clone();
        let mut libs = Vec::new();
        let mut unreadable = Vec::new();
        let mut known_names = HashMap::new();
        for r in enabled.into_iter().filter(|r| !r.id.starts_with(REMOTE_LIBRARY_PREFIX)) {
            known_names.insert(r.id.clone(), r.name.clone());
            match app.snapshot_of(&r.id, r.version) {
                Some(snap) => libs.push((r, snap)),
                None => unreadable.push(r.name),
            }
        }
        let external_libs: Vec<(String, u32)> = app.doc.library.external.values().map(|e| (e.library_id.clone(), e.version)).collect();
        for (lib_id, version) in external_libs {
            if !known_names.contains_key(&lib_id) && !lib_id.starts_with(REMOTE_LIBRARY_PREFIX) {
                if let Some(snap) = app.snapshot_of(&lib_id, version) {
                    known_names.insert(lib_id, snap.name.clone());
                }
            }
        }
        (Self { libs, known_names }, unreadable)
    }
}

/// Check designs panel state. The scope is fixed when the panel opens, so selecting a finding's
/// layer does not change what is being listed.
pub struct CheckDesignsState {
    pub open: bool,
    pub scope: Option<Scope>,
    /// "Selection (3 layers)" or "Page 1", shown under the title.
    pub scope_label: String,
    pub report: Report,
    /// Active incremental scan cursor while a scan is in flight.
    pub cursor: Option<ScanCursor>,
    /// Per-frame time budget for [`audit_step`] (defaults to [`SLICE_BUDGET`] = 2.0 ms).
    pub slice_budget: Duration,
    /// Session-only dismissed findings; cleared when the panel is reopened.
    pub dismissed: BTreeSet<IgnoreKey>,
    /// Whether the Rules popover is open.
    pub rules_open: bool,
    /// Whether the `{n} ignored` footer is expanded to show ignored findings.
    pub show_ignored: bool,
    /// `(document revision, scope, enabled libraries, check_designs settings)` the cached `report` was computed for.
    audited: Option<(u64, Scope, Vec<LibraryRef>, CheckDesignsSettings)>,
}

impl Default for CheckDesignsState {
    fn default() -> Self {
        Self {
            open: false,
            scope: None,
            scope_label: String::new(),
            report: Report::default(),
            cursor: None,
            slice_budget: SLICE_BUDGET,
            dismissed: BTreeSet::new(),
            rules_open: false,
            show_ignored: false,
            audited: None,
        }
    }
}

impl CheckDesignsState {
    /// Forces the next frame to audit again.
    pub fn invalidate(&mut self) {
        self.audited = None;
        self.cursor = None;
    }

    /// Whether an incremental scan is currently in progress.
    #[cfg(test)]
    pub fn is_scanning(&self) -> bool {
        self.cursor.is_some()
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

    /// Advances or starts the incremental audit when the document or the scope changed since the
    /// cached report. While the user is mid-gesture (`interacting`), slicing pauses and any
    /// existing report is kept so a drag does not audit every frame.
    pub fn refresh_check_designs(&mut self, interacting: bool, ctx: &egui::Context) {
        if !self.check_designs.open {
            self.check_designs.cursor = None;
            return;
        }
        // A page scope whose page was deleted would list nothing for ever; follow the page on screen.
        if matches!(self.check_designs.scope, Some(Scope::Page(p)) if self.doc.get(p).is_none()) {
            let name = self.doc.get(self.page).map(|p| p.name.clone()).unwrap_or_default();
            self.check_designs.scope = Some(Scope::Page(self.page));
            self.check_designs.scope_label = format!("Page: {name}");
            self.check_designs.invalidate();
        }
        let Some(scope) = self.check_designs.scope.clone() else { return };
        let key = (self.canvas.tiles.rev(), scope.clone(), self.doc.library.enabled.clone(), self.doc.check_designs.clone());
        if self.check_designs.cursor.is_none() && self.check_designs.audited.as_ref() == Some(&key) {
            return;
        }
        if interacting && (self.check_designs.audited.is_some() || self.check_designs.cursor.is_some()) {
            return;
        }
        let need_new_cursor = match self.check_designs.cursor.as_ref() {
            Some(cur) => cur.full_key.as_ref() != Some(&key),
            None => true,
        };
        if need_new_cursor {
            let (libs, unreadable) = LibrarySources::collect(self);
            let mut cur = ScanCursor::with_libraries(&self.doc, scope, libs);
            cur.full_key = Some(key.clone());
            cur.unreadable_libraries = unreadable;
            cur.updating = self.check_designs.report.complete;
            self.check_designs.cursor = Some(cur);
        }
        let budget = self.check_designs.slice_budget;
        let done = if let Some(cur) = self.check_designs.cursor.as_mut() { audit_step(&self.doc, cur, budget) } else { true };
        if done {
            if let Some(cur) = self.check_designs.cursor.take() {
                let full_key = cur.full_key.clone().unwrap_or(key);
                let mut report = cur.into_report(&self.doc);
                report.findings.retain(|f| !self.check_designs.dismissed.contains(&f.ignore_key()));
                self.check_designs.report = report;
                self.check_designs.audited = Some(full_key);
            }
        } else {
            ctx.request_repaint();
        }
    }

    fn apply_check_designs_settings(&mut self, next: CheckDesignsSettings) -> bool {
        if self.doc.check_designs == next {
            return false;
        }
        let mut tx = Tx::new();
        if tx.cmd(&mut self.doc, Command::SetCheckDesignsSettings(next)).is_err() {
            tx.cancel(&mut self.doc);
            return false;
        }
        let ok = self.run_tx(tx);
        self.check_designs.invalidate();
        ok
    }

    /// Enables or disables `rule` (by its `figma_id()`) as one undo step.
    pub fn set_check_designs_rule(&mut self, rule: Rule, on: bool) -> bool {
        let mut next = self.doc.check_designs.clone();
        if on {
            next.disabled_rules.remove(rule.figma_id());
        } else {
            next.disabled_rules.insert(rule.figma_id().to_string());
        }
        self.apply_check_designs_settings(next)
    }

    /// Sets the WCAG contrast threshold (`AA` or `AAA`) as one undo step.
    pub fn set_contrast_level(&mut self, level: ContrastLevel) -> bool {
        let mut next = self.doc.check_designs.clone();
        next.contrast_level = level;
        self.apply_check_designs_settings(next)
    }

    /// Persistently ignores `finding` in the document as one undo step.
    pub fn check_designs_ignore(&mut self, finding: &Finding) -> bool {
        let mut next = self.doc.check_designs.clone();
        next.ignored.insert(finding.ignore_key());
        self.apply_check_designs_settings(next)
    }

    /// Restores a previously ignored finding key as one undo step.
    pub fn check_designs_restore(&mut self, key: &IgnoreKey) -> bool {
        let mut next = self.doc.check_designs.clone();
        next.ignored.remove(key);
        self.apply_check_designs_settings(next)
    }

    /// Dismisses `finding` for the current panel session without touching the document or undo stack.
    pub fn check_designs_dismiss(&mut self, finding: &Finding) {
        let key = finding.ignore_key();
        self.check_designs.dismissed.insert(key.clone());
        self.check_designs.report.findings.retain(|f| f.ignore_key() != key);
    }
}
