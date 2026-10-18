//! Check designs: scans layers for design-system drift and offers a one-click fix per finding.
//!
//! Three rules, as in Figma's "Check designs":
//! - **Hard-coded values**: a fill, stroke, text style or effect list holds a raw value that equals an
//!   existing variable or shared style but is not linked to it. The suggested fix links it.
//! - **Contrast**: text fails WCAG 2.x AA against the backdrop its ancestors paint.
//! - **Detached instances**: a frame that was detached from a main component present in this file;
//!   the reconnect fix is offered only while the frame still equals the component.
//!
//! The audit is a pure function of the [`Document`]. Fixes run through [`crate::edit::Tx`], so each
//! fix (and "Fix all") is exactly one undo step. Layers inside an instance mirror their main
//! component and are skipped: the fix belongs on the main component, and the instance follows.

mod contrast;
mod detached;
mod fix;
pub mod fixture;
mod hardcoded;
mod state;
#[cfg(test)]
mod tests;

pub use fixture::FIXTURE;
pub use state::CheckDesignsState;

use designos_doc::{Color, Document, LibraryRef, LibrarySnapshot, NodeId, NodeKind};
use std::sync::Arc;

/// Layers scanned per audit. A page of several hundred thousand layers must never stall the UI
/// thread, so the audit stops here and the panel says so.
pub const MAX_SCANNED_LAYERS: usize = 20_000;

/// The kind of problem a [`Finding`] reports; also the group it is listed under.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Rule {
    HardCodedValue,
    Contrast,
    DetachedInstance,
}

impl Rule {
    pub const ALL: [Rule; 3] = [Rule::HardCodedValue, Rule::Contrast, Rule::DetachedInstance];

    /// Group heading in the panel.
    pub fn title(self) -> &'static str {
        match self {
            Rule::HardCodedValue => "Hard-coded values",
            Rule::Contrast => "Contrast",
            Rule::DetachedInstance => "Detached instances",
        }
    }
}

/// The edit that resolves a finding.
#[derive(Clone, Debug, PartialEq)]
pub enum Fix {
    /// Bind the property `key` (`fills/0`, `strokes/1`, `itemSpacing`, …) to a variable.
    BindVariable { key: String, variable: String },
    /// Bind several properties at once (uniform corner radius or uniform padding) to a variable.
    BindVariables { keys: Vec<String>, variable: String },
    /// Link the layer to a shared style (fill, stroke, text or effect: the style knows which).
    ApplyStyle { style: String },
    /// Replace the text colour with `color` (alpha is kept).
    SetTextColor { color: Color },
    /// Replace the detached frame with an instance of `component`, keeping its name and size.
    Reconnect { component: NodeId },
}

/// One problem on one layer.
#[derive(Clone, Debug, PartialEq)]
pub struct Finding {
    pub node: NodeId,
    pub node_name: String,
    pub rule: Rule,
    /// What is wrong, as shown in the row.
    pub message: String,
    /// The one-click fix, when there is one.
    pub fix: Option<Fix>,
    /// What the fix does, for its tooltip (empty without a fix).
    pub fix_label: String,
}

impl Finding {
    /// Whether this finding suggests an asset from an enabled library.
    pub fn is_from_library(&self) -> bool {
        self.message.contains(" from library \"")
    }
}

/// What an audit looks at.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Scope {
    /// Every top-level layer of a page.
    Page(NodeId),
    /// These layers and everything below them.
    Layers(Vec<NodeId>),
}

/// Enabled library snapshots collected from the app cache before an audit.
#[derive(Clone, Debug, Default)]
pub struct LibrarySources {
    pub(super) libs: Vec<(LibraryRef, Arc<LibrarySnapshot>)>,
}

/// Result of one audit.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Report {
    pub findings: Vec<Finding>,
    /// Layers examined.
    pub scanned: usize,
    /// The scan stopped at [`MAX_SCANNED_LAYERS`].
    pub truncated: bool,
    /// Display names of enabled libraries whose pinned snapshot could not be read.
    pub unreadable_libraries: Vec<String>,
}

impl Report {
    /// Findings of one rule, in document order.
    pub fn of(&self, rule: Rule) -> impl Iterator<Item = &Finding> {
        self.findings.iter().filter(move |f| f.rule == rule)
    }

    /// Findings that carry a fix.
    pub fn fixable(&self) -> Vec<&Finding> {
        self.findings.iter().filter(|f| f.fix.is_some()).collect()
    }
}

/// Scans `scope` with the document's own variables and styles only (the app always audits with
/// its enabled libraries through [`audit_with_libraries`]). Hidden layers are skipped; layers inside
/// an instance are audited only where their effective properties or backdrop diverge from their
/// main component.
#[cfg(test)]
pub fn audit(doc: &Document, scope: &Scope) -> Report {
    audit_with_libraries(doc, scope, &LibrarySources::default())
}

/// Scans `scope` and includes variables and styles from enabled `libs` snapshots.
pub fn audit_with_libraries(doc: &Document, scope: &Scope, libs: &LibrarySources) -> Report {
    let roots: Vec<NodeId> = match scope {
        Scope::Page(page) => doc.get(*page).map(|p| p.children.clone()).unwrap_or_default(),
        Scope::Layers(ids) => ids.clone(),
    };
    let index = hardcoded::Index::with_libraries(doc, libs);
    let components = detached::Components::new(doc);
    let bounds = contrast::SubtreeBounds::default();
    let mut report = Report::default();
    let mut visited = 0usize;
    let mut stack: Vec<NodeId> = roots.into_iter().rev().collect();
    while let Some(id) = stack.pop() {
        let Some(n) = doc.get(id) else { continue };
        if !n.visible || matches!(n.kind, NodeKind::Page { .. }) {
            continue;
        }
        if visited >= MAX_SCANNED_LAYERS {
            report.scanned = visited;
            report.truncated = true;
            break;
        }
        visited += 1;
        let before = report.findings.len();
        hardcoded::check(doc, &index, n, &mut report.findings);
        contrast::check(doc, &bounds, n, &mut report.findings);
        detached::check(doc, &components, n, &mut report.findings);
        if n.component.source.is_none() || !n.component.overrides.is_empty() || report.findings.len() > before {
            report.scanned += 1;
        }
        stack.extend(n.children.iter().rev().copied());
    }
    report
}
