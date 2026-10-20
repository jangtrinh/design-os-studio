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
mod library_assets;
mod state;
#[cfg(test)]
mod tests;
mod text_styles;

pub use fixture::FIXTURE;
pub use state::CheckDesignsState;

use designos_doc::{CheckDesignsSettings, Color, Document, IgnoreKey, LibraryRef, LibrarySnapshot, Node, NodeId, NodeKind};
use std::collections::HashMap;
use std::ops::Bound;
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Default per-frame time slice budget (2.0 ms) for incremental page scanning.
pub const SLICE_BUDGET: Duration = Duration::from_millis(2);

/// The kind of problem a [`Finding`] reports; also the group it is listed under.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Rule {
    HardCodedValue,
    Contrast,
    DetachedInstance,
    LibraryAssets,
    TextStyles,
    CornerRadius,
    Padding,
    HorizontalSpacing,
    VerticalSpacing,
    TypographyAndStroke,
}

impl Rule {
    pub const ALL: [Rule; 10] = [
        Rule::HardCodedValue,
        Rule::Contrast,
        Rule::DetachedInstance,
        Rule::LibraryAssets,
        Rule::TextStyles,
        Rule::CornerRadius,
        Rule::Padding,
        Rule::HorizontalSpacing,
        Rule::VerticalSpacing,
        Rule::TypographyAndStroke,
    ];

    /// Group heading in the panel.
    pub fn title(self) -> &'static str {
        match self {
            Rule::HardCodedValue => "Colours",
            Rule::Contrast => "Contrast",
            Rule::DetachedInstance => "Detached instances",
            Rule::LibraryAssets => "Library assets",
            Rule::TextStyles => "Text styles",
            Rule::CornerRadius => "Corner radius",
            Rule::Padding => "Padding",
            Rule::HorizontalSpacing => "Horizontal spacing",
            Rule::VerticalSpacing => "Vertical spacing",
            Rule::TypographyAndStroke => "Typography and stroke",
        }
    }

    /// Stable Figma design-linter rule identifier used in `CheckDesignsSettings::disabled_rules`
    /// and `IgnoreKey::rule`.
    pub fn figma_id(self) -> &'static str {
        match self {
            Rule::HardCodedValue | Rule::Contrast => "RECOMMEND_VARIABLES",
            Rule::DetachedInstance => "REQUIRE_NO_DETACHED_COMPONENTS",
            Rule::LibraryAssets => "REQUIRE_ASSETS_FROM_SELECTED_LIBRARIES",
            Rule::TextStyles => "REQUIRE_TEXT_STYLES",
            Rule::CornerRadius => "REQUIRE_CORNER_RADIUS_VARIABLES",
            Rule::Padding => "REQUIRE_PADDING_VARIABLES",
            Rule::HorizontalSpacing => "REQUIRE_HORIZONTAL_SPACING_VARIABLES",
            Rule::VerticalSpacing => "REQUIRE_VERTICAL_SPACING_VARIABLES",
            Rule::TypographyAndStroke => "REQUIRE_TYPOGRAPHY_AND_STROKE_VARIABLES",
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
    /// Property key within the rule (`"fills/0"`, `"paddingTop"`, `""` for whole-node rules).
    pub property: String,
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

    /// Persistent ignore key for this finding.
    pub fn ignore_key(&self) -> IgnoreKey {
        IgnoreKey { node: self.node, rule: self.rule.figma_id().to_string(), property: self.property.clone() }
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
    pub(super) known_names: HashMap<String, String>,
}

/// Result of one audit.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Report {
    pub findings: Vec<Finding>,
    /// Findings in the audited scope that were suppressed by `doc.check_designs.ignored`.
    pub ignored: Vec<Finding>,
    /// Layers examined.
    pub scanned: usize,
    /// Whether the scan has finished visiting every layer in the scope.
    pub complete: bool,
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

/// Phase of an incremental scan.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    Prepare,
    Walk,
    Done,
}

/// The comparison of one detached frame with its main component, paused between slices. The rest
/// of the frame's own checks wait for it, so the findings come out in the order a one-shot audit
/// gives.
struct DetachJob {
    node: NodeId,
    component: NodeId,
    findings_before: usize,
    comparison: detached::Comparison,
}

/// Incremental, time-sliced audit cursor.
pub struct ScanCursor {
    pub phase: Phase,
    pub stack: Vec<NodeId>,
    pub node_iter_from: Option<NodeId>,
    pub total: usize,
    /// Layers the walk has visited so far, out of `total`; drives the progress bar.
    pub scanned: usize,
    /// Layers counted into [`Report::scanned`]: those of an instance that match their main component and have no finding are not.
    pub(super) examined: usize,
    pub partial: Vec<Finding>,
    prep_stack: Vec<NodeId>,
    components_done: bool,
    components: detached::Components,
    index: Option<hardcoded::Index>,
    bounds: contrast::SubtreeBounds,
    libs: LibrarySources,
    /// A detached frame whose comparison with its main component is still running.
    detach: Option<DetachJob>,
    pub(super) unreadable_libraries: Vec<String>,
    pub(super) full_key: Option<(u64, Scope, Vec<LibraryRef>, CheckDesignsSettings)>,
    pub(super) updating: bool,
}

impl ScanCursor {
    #[cfg(test)]
    pub fn new(doc: &Document, scope: Scope) -> Self {
        Self::with_libraries(doc, scope, LibrarySources::default())
    }

    pub fn with_libraries(doc: &Document, scope: Scope, libs: LibrarySources) -> Self {
        let roots: Vec<NodeId> = match &scope {
            Scope::Page(page) => doc.get(*page).map(|p| p.children.clone()).unwrap_or_default(),
            Scope::Layers(ids) => ids.clone(),
        };
        let rev_roots: Vec<NodeId> = roots.into_iter().rev().collect();
        Self {
            phase: Phase::Prepare,
            stack: rev_roots.clone(),
            node_iter_from: None,
            total: 0,
            scanned: 0,
            examined: 0,
            partial: Vec::new(),
            prep_stack: rev_roots,
            components_done: false,
            components: detached::Components::default(),
            index: None,
            bounds: contrast::SubtreeBounds::default(),
            libs,
            detach: None,
            unreadable_libraries: Vec::new(),
            full_key: None,
            updating: false,
        }
    }

    /// Converts the finished (or partial) cursor into a [`Report`], partitioning ignored findings.
    pub fn into_report(self, doc: &Document) -> Report {
        let mut report = Report {
            findings: Vec::new(),
            ignored: Vec::new(),
            scanned: self.examined,
            complete: self.phase == Phase::Done,
            unreadable_libraries: self.unreadable_libraries,
        };
        for f in self.partial {
            if doc.check_designs.ignored.contains(&f.ignore_key()) {
                report.ignored.push(f);
            } else {
                report.findings.push(f);
            }
        }
        report
    }
}

/// What a layer still gets once the checks that can pause are done: the library-asset check, the
/// count of layers examined, and its children queued.
fn finish_layer(doc: &Document, cur: &mut ScanCursor, n: &Node, findings_before: usize) {
    if !doc.check_designs.disabled_rules.contains(Rule::LibraryAssets.figma_id()) {
        library_assets::check(doc, &cur.libs, n, n.component.source.and_then(|s| doc.get(s)), &mut cur.partial);
    }
    if n.component.source.is_none() || !n.component.overrides.is_empty() || cur.partial.len() > findings_before {
        cur.examined += 1;
    }
    cur.stack.extend(n.children.iter().rev().copied());
}

/// A document with at most this many layers always gets a few dozen layers per slice, so a
/// scheduler stall on a small file cannot split its scan; a larger one yields as soon as the budget is spent.
const SMALL_DOCUMENT_NODES: usize = 512;

/// Advances `cur` for up to `budget` and returns `true` when the scan reaches [`Phase::Done`].
pub fn audit_step(doc: &Document, cur: &mut ScanCursor, budget: Duration) -> bool {
    if cur.phase == Phase::Done {
        return true;
    }
    let start = Instant::now();
    // Small fixtures (<= 32 nodes) under the normal >=1ms frame budget always finish in one slice
    // even if the OS scheduler preempts the test thread; sub-millisecond budgets yield immediately.
    let min_steps = if budget >= Duration::from_millis(1) && doc.nodes.len() <= SMALL_DOCUMENT_NODES { 32 } else { 1 };
    let prep_mask = if budget >= Duration::from_millis(1) { 31 } else { 0 };
    let mut steps = 0usize;

    if cur.phase == Phase::Prepare {
        if !cur.components_done {
            let iter = match cur.node_iter_from {
                Some(after) => doc.nodes.range((Bound::Excluded(after), Bound::Unbounded)),
                None => doc.nodes.range(..),
            };
            let mut last_seen = None;
            let mut exhausted = true;
            for (&id, node) in iter {
                cur.components.insert_node(node);
                last_seen = Some(id);
                steps += 1;
                if steps >= min_steps && (steps & prep_mask) == 0 && start.elapsed() >= budget {
                    exhausted = false;
                    break;
                }
            }
            if exhausted {
                cur.components_done = true;
                cur.node_iter_from = None;
            } else {
                cur.node_iter_from = last_seen;
                return false;
            }
        }

        while let Some(id) = cur.prep_stack.pop() {
            if steps >= min_steps && (steps & prep_mask) == 0 && start.elapsed() >= budget {
                cur.prep_stack.push(id);
                return false;
            }
            steps += 1;
            let Some(n) = doc.get(id) else { continue };
            if !n.visible || matches!(n.kind, NodeKind::Page { .. }) {
                continue;
            }
            cur.total += 1;
            cur.prep_stack.extend(n.children.iter().rev().copied());
        }

        cur.index = Some(hardcoded::Index::with_libraries(doc, &cur.libs));
        cur.phase = Phase::Walk;
        if !cur.stack.is_empty() && steps >= min_steps && start.elapsed() >= budget {
            return false;
        }
    }

    if cur.phase == Phase::Walk {
        let contrast_on = !doc.check_designs.disabled_rules.contains(Rule::Contrast.figma_id());
        let detached_on = !doc.check_designs.disabled_rules.contains(Rule::DetachedInstance.figma_id());
        loop {
            if let Some(mut job) = cur.detach.take() {
                let Some(equal) = job.comparison.run(doc, start + budget) else {
                    cur.detach = Some(job);
                    return false;
                };
                if let Some(n) = doc.get(job.node) {
                    cur.partial.push(detached::compared(doc, n, job.component, equal));
                    finish_layer(doc, cur, n, job.findings_before);
                }
                if !cur.stack.is_empty() && start.elapsed() >= budget {
                    return false;
                }
            }
            let Some(id) = cur.stack.pop() else { break };
            let Some(n) = doc.get(id) else { continue };
            if !n.visible || matches!(n.kind, NodeKind::Page { .. }) {
                continue;
            }
            if contrast_on && matches!(n.kind, NodeKind::Text(_)) && !cur.bounds.warm_for_text(doc, id, start + budget) {
                cur.stack.push(id);
                return false;
            }
            cur.scanned += 1;
            steps += 1;
            let before = cur.partial.len();
            if let Some(index) = cur.index.as_ref() {
                hardcoded::check(doc, index, n, &mut cur.partial);
            }
            if contrast_on {
                contrast::check(doc, &cur.bounds, n, &mut cur.partial);
            }
            if detached_on {
                match detached::plan(doc, &cur.components, n) {
                    detached::Detached::Nothing => {}
                    detached::Detached::Found(f) => cur.partial.push(f),
                    detached::Detached::Compare { component } => {
                        cur.detach = Some(DetachJob { node: id, component, findings_before: before, comparison: detached::Comparison::new(id, component) });
                        continue;
                    }
                }
            }
            finish_layer(doc, cur, n, before);
            if !cur.stack.is_empty() && steps >= min_steps && start.elapsed() >= budget {
                return false;
            }
        }
        cur.phase = Phase::Done;
    }

    cur.phase == Phase::Done
}

/// Scans `scope` to the end in one go, with the document's own variables and styles only (the app
/// audits incrementally with its enabled libraries through [`ScanCursor`] and [`audit_step`]).
/// Hidden layers are skipped; layers inside an instance are audited only where their effective
/// properties or backdrop diverge from their main component.
#[cfg(test)]
pub fn audit(doc: &Document, scope: &Scope) -> Report {
    audit_with_libraries(doc, scope, &LibrarySources::default())
}

/// Scans `scope` to the end in one go and includes variables and styles from enabled `libs` snapshots.
#[cfg(test)]
pub fn audit_with_libraries(doc: &Document, scope: &Scope, libs: &LibrarySources) -> Report {
    let mut cur = ScanCursor::with_libraries(doc, scope.clone(), libs.clone());
    while !audit_step(doc, &mut cur, Duration::from_secs(3600)) {}
    cur.into_report(doc)
}
