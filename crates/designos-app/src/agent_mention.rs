//! `@`-mention search, reference grammar (`designos:<kind>/<id>`), and prompt footer builder
//! for the Agent composer and terminal (`#655`).

use crate::icons::Icon;
use designos_doc::{Document, Node, NodeId, NodeKind, StyleProperty, VariableKind};
use std::time::{Duration, Instant};

/// Maximum candidates returned for a single [`MentionCategory`].
pub const MAX_PER_CATEGORY: usize = 8;

/// Maximum candidates returned across all categories combined.
pub const MAX_TOTAL_RESULTS: usize = 30;

/// Per-frame time budget for scanning large documents (`4 ms`).
pub const DEFAULT_FRAME_BUDGET: Duration = Duration::from_millis(4);

/// Maximum nodes inspected in a single budgeted step before yielding to the next frame,
/// guaranteeing multi-step incremental progress even on very fast CPUs.
pub const MAX_NODES_PER_STEP: usize = 20_000;

/// Stable reference kind in the `designos:<kind>/<id>` grammar.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum MentionKind {
    Node,
    Variable,
    Style,
}

impl MentionKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Node => "node",
            Self::Variable => "variable",
            Self::Style => "style",
        }
    }

    #[cfg(test)]
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "node" => Some(Self::Node),
            "variable" => Some(Self::Variable),
            "style" => Some(Self::Style),
            _ => None,
        }
    }
}

/// Visual grouping category in the `@`-mention popup.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum MentionCategory {
    Layers,
    Pages,
    Components,
    Variables,
    Styles,
}

impl MentionCategory {
    pub const ALL: [MentionCategory; 5] = [Self::Layers, Self::Pages, Self::Components, Self::Variables, Self::Styles];

    pub fn heading(self) -> &'static str {
        match self {
            Self::Layers => "Layers",
            Self::Pages => "Pages",
            Self::Components => "Components",
            Self::Variables => "Variables",
            Self::Styles => "Styles",
        }
    }

    fn slot_index(self) -> usize {
        match self {
            Self::Layers => 0,
            Self::Pages => 1,
            Self::Components => 2,
            Self::Variables => 3,
            Self::Styles => 4,
        }
    }
}

/// Active mention recorded on the composer state.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Mention {
    pub label: String,
    pub kind: MentionKind,
    pub id: String,
    pub name: String,
    pub page: Option<String>,
}

impl Mention {
    pub fn from_candidate(c: &MentionCandidate) -> Self {
        Self { label: c.label(), kind: c.kind, id: c.id.clone(), name: c.name.clone(), page: c.page.clone() }
    }
}

/// Match quality tier (lower is better: `Exact < Prefix < WordPrefix < Substring`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum MatchTier {
    Exact = 0,
    Prefix = 1,
    WordPrefix = 2,
    Substring = 3,
}

/// One ranked candidate shown in the `@`-mention popup.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MentionCandidate {
    pub category: MentionCategory,
    pub kind: MentionKind,
    pub id: String,
    pub name: String,
    pub page: Option<String>,
    pub caption: String,
    pub icon: Icon,
    pub hidden: bool,
    pub on_current_page: bool,
    pub match_tier: MatchTier,
}

impl MentionCandidate {
    /// Compact token label inserted after `@` in the composer (whitespace within a name is
    /// collapsed to `/` or `_` only when needed; otherwise the item's name is used directly).
    pub fn label(&self) -> String {
        self.name.trim().to_string()
    }

    fn rank_key(&self) -> (MatchTier, bool, bool, usize, &str, &str) {
        (self.match_tier, !self.on_current_page, self.hidden, self.name.len(), self.name.as_str(), self.id.as_str())
    }
}

fn cmp_candidates(a: &MentionCandidate, b: &MentionCandidate) -> std::cmp::Ordering {
    a.rank_key().cmp(&b.rank_key())
}

/// Case-insensitive match tier without heap allocation on ASCII names.
pub fn match_name_tier(name: &str, query_lower: &str) -> Option<MatchTier> {
    if query_lower.is_empty() {
        return Some(MatchTier::Exact);
    }
    let name_trimmed = name.trim();
    if name_trimmed.is_empty() {
        return None;
    }
    if name_trimmed.is_ascii() && query_lower.is_ascii() {
        let nb = name_trimmed.as_bytes();
        let qb = query_lower.as_bytes();
        if nb.len() < qb.len() {
            return None;
        }
        if nb.eq_ignore_ascii_case(qb) {
            return Some(MatchTier::Exact);
        }
        if nb[..qb.len()].eq_ignore_ascii_case(qb) {
            return Some(MatchTier::Prefix);
        }
        let mut found_sub = false;
        let last_start = nb.len() - qb.len();
        for i in 1..=last_start {
            if nb[i..i + qb.len()].eq_ignore_ascii_case(qb) {
                let prev = nb[i - 1];
                if matches!(prev, b'/' | b' ' | b'-' | b'_' | b'.' | b':') {
                    return Some(MatchTier::WordPrefix);
                }
                found_sub = true;
            }
        }
        return if found_sub { Some(MatchTier::Substring) } else { None };
    }

    let lower = name_trimmed.to_lowercase();
    if lower == query_lower {
        return Some(MatchTier::Exact);
    }
    if lower.starts_with(query_lower) {
        return Some(MatchTier::Prefix);
    }
    for part in lower.split(['/', ' ', '-', '_', '.', ':']) {
        if !part.is_empty() && part.starts_with(query_lower) {
            return Some(MatchTier::WordPrefix);
        }
    }
    if lower.contains(query_lower) {
        Some(MatchTier::Substring)
    } else {
        None
    }
}

pub fn icon_for_node_kind(kind: &NodeKind) -> Icon {
    match kind {
        NodeKind::Page { .. } => Icon::Page,
        NodeKind::Frame(_) => Icon::Frame,
        NodeKind::Group => Icon::Group,
        NodeKind::Section => Icon::Section,
        NodeKind::Rectangle => Icon::Rectangle,
        NodeKind::Ellipse(_) => Icon::Ellipse,
        NodeKind::Line => Icon::Line,
        NodeKind::Polygon { .. } => Icon::Polygon,
        NodeKind::Star { .. } => Icon::Star,
        NodeKind::Component(_) => Icon::Component,
        NodeKind::ComponentSet(_) => Icon::ComponentSet,
        NodeKind::Instance { .. } => Icon::Instance,
        NodeKind::Text(_) => Icon::Text,
        NodeKind::Vector { .. } => Icon::Vector,
        NodeKind::BooleanOp(_) => Icon::Boolean,
        NodeKind::Sticky(_) => Icon::Sticky,
        NodeKind::ShapeWithText(_) => Icon::Shapes,
        NodeKind::Connector(_) => Icon::Connector,
        NodeKind::Stamp { .. } => Icon::Stamp,
        NodeKind::Unknown { .. } => Icon::Rectangle,
    }
}

fn variable_kind_label(kind: VariableKind) -> &'static str {
    match kind {
        VariableKind::Color => "Color",
        VariableKind::Number => "Number",
        VariableKind::String => "String",
        VariableKind::Boolean => "Boolean",
    }
}

fn style_property_label(prop: StyleProperty) -> &'static str {
    match prop {
        StyleProperty::Fill => "Fill style",
        StyleProperty::Stroke => "Stroke style",
        StyleProperty::Text => "Text style",
        StyleProperty::Effect => "Effect style",
        StyleProperty::Grid => "Grid style",
    }
}

fn insert_bounded(bucket: &mut Vec<MentionCandidate>, candidate: MentionCandidate) {
    if bucket.len() < MAX_PER_CATEGORY {
        bucket.push(candidate);
        bucket.sort_by(cmp_candidates);
        return;
    }
    if let Some(worst) = bucket.last() {
        if cmp_candidates(&candidate, worst).is_lt() {
            bucket.pop();
            bucket.push(candidate);
            bucket.sort_by(cmp_candidates);
        }
    }
}

fn worse_than_bucket_tail(bucket: &[MentionCandidate], tier: MatchTier, on_current_page: bool, hidden: bool) -> bool {
    if bucket.len() < MAX_PER_CATEGORY {
        return false;
    }
    let Some(worst) = bucket.last() else { return false };
    (tier, !on_current_page, hidden) > (worst.match_tier, !worst.on_current_page, worst.hidden)
}

#[derive(Clone, Debug)]
struct DfsEntry {
    node_id: NodeId,
    next_child_idx: usize,
    hidden_in_chain: bool,
}

/// Incremental, per-frame budgeted search job over a [`Document`].
#[derive(Clone, Debug)]
pub struct MentionSearchJob {
    query_lower: String,
    current_page: NodeId,
    globals_scanned: bool,
    root_pages: Vec<NodeId>,
    page_idx: usize,
    current_root_page: NodeId,
    current_root_page_name: String,
    dfs_stack: Vec<DfsEntry>,
    buckets: [Vec<MentionCandidate>; 5],
    finished: bool,
    pub steps_taken: usize,
    pub nodes_visited: usize,
}

impl MentionSearchJob {
    /// Creates a new incremental search job and runs one budgeted step immediately.
    pub fn start(doc: &Document, current_page: NodeId, query: &str, budget: Duration) -> Self {
        let mut root_pages = doc.pages.clone();
        if let Some(lp) = doc.library_page {
            if !root_pages.contains(&lp) {
                root_pages.push(lp);
            }
        }
        // Scan `current_page` first so partial results from step 1 already prioritize the active page.
        if let Some(pos) = root_pages.iter().position(|&p| p == current_page) {
            if pos > 0 {
                root_pages.swap(0, pos);
            }
        }
        let clean_q = query.trim();
        let mut job = Self {
            query_lower: clean_q.to_lowercase(),
            current_page,
            globals_scanned: false,
            root_pages,
            page_idx: 0,
            current_root_page: NodeId(0),
            current_root_page_name: String::new(),
            dfs_stack: Vec::new(),
            buckets: Default::default(),
            finished: false,
            steps_taken: 0,
            nodes_visited: 0,
        };
        job.step(doc, current_page, budget);
        job
    }

    pub fn is_finished(&self) -> bool {
        self.finished
    }

    /// Advances the scan within `budget` (and at most [`MAX_NODES_PER_STEP`] nodes).
    /// Returns `true` when the entire document has been scanned.
    pub fn step(&mut self, doc: &Document, current_page: NodeId, budget: Duration) -> bool {
        if self.finished {
            return true;
        }
        self.current_page = current_page;
        self.steps_taken += 1;
        let start = Instant::now();
        let mut step_nodes = 0usize;

        if !self.globals_scanned {
            self.scan_globals(doc);
            self.globals_scanned = true;
        }

        loop {
            if self.dfs_stack.is_empty() {
                if self.page_idx >= self.root_pages.len() {
                    self.finished = true;
                    return true;
                }
                let page_id = self.root_pages[self.page_idx];
                self.page_idx += 1;
                if let Some(page_node) = doc.nodes.get(&page_id) {
                    self.current_root_page = page_id;
                    self.current_root_page_name = page_node.name.clone();
                    self.dfs_stack.push(DfsEntry { node_id: page_id, next_child_idx: 0, hidden_in_chain: !page_node.visible });
                }
                continue;
            }

            if step_nodes >= MAX_NODES_PER_STEP || (step_nodes.is_multiple_of(64) && step_nodes > 0 && start.elapsed() >= budget) {
                return false;
            }

            let Some(top) = self.dfs_stack.last_mut() else {
                continue;
            };
            let parent_id = top.node_id;
            let child_idx = top.next_child_idx;
            let parent_hidden = top.hidden_in_chain;

            let Some(parent_node) = doc.nodes.get(&parent_id) else {
                self.dfs_stack.pop();
                continue;
            };

            if child_idx >= parent_node.children.len() {
                self.dfs_stack.pop();
                continue;
            }

            let child_id = parent_node.children[child_idx];
            top.next_child_idx += 1;

            let Some(child_node) = doc.nodes.get(&child_id) else {
                continue;
            };

            step_nodes += 1;
            self.nodes_visited += 1;
            let child_hidden = parent_hidden || !child_node.visible;
            self.inspect_node(doc, child_node, child_hidden);

            if !child_node.children.is_empty() {
                self.dfs_stack.push(DfsEntry { node_id: child_id, next_child_idx: 0, hidden_in_chain: child_hidden });
            }
        }
    }

    fn scan_globals(&mut self, doc: &Document) {
        // 1. Pages
        for &page_id in &doc.pages {
            let Some(page_node) = doc.nodes.get(&page_id) else { continue };
            let Some(tier) = match_name_tier(&page_node.name, &self.query_lower) else { continue };
            let on_current = page_id == self.current_page;
            let caption = if on_current { "Current page".to_string() } else { "Page".to_string() };
            insert_bounded(
                &mut self.buckets[MentionCategory::Pages.slot_index()],
                MentionCandidate {
                    category: MentionCategory::Pages,
                    kind: MentionKind::Node,
                    id: page_id.0.to_string(),
                    name: page_node.name.clone(),
                    page: Some(page_node.name.clone()),
                    caption,
                    icon: Icon::Page,
                    hidden: !page_node.visible,
                    on_current_page: on_current,
                    match_tier: tier,
                },
            );
        }

        // 2. Variables
        for (var_key, var) in &doc.variables {
            let Some(tier) = match_name_tier(&var.name, &self.query_lower) else { continue };
            let col_name = doc.variable_collections.get(&var.collection).map(|c| c.name.as_str()).unwrap_or("Variables");
            let id = if var.id.is_empty() { var_key.clone() } else { var.id.clone() };
            insert_bounded(
                &mut self.buckets[MentionCategory::Variables.slot_index()],
                MentionCandidate {
                    category: MentionCategory::Variables,
                    kind: MentionKind::Variable,
                    id,
                    name: var.name.clone(),
                    page: Some(col_name.to_string()),
                    caption: format!("{col_name} · {}", variable_kind_label(var.kind)),
                    icon: Icon::Variables,
                    hidden: var.hidden_from_publishing,
                    on_current_page: false,
                    match_tier: tier,
                },
            );
        }

        // 3. Styles
        for style_key in doc.styles.keys() {
            let style_name = doc.style_name(style_key);
            let Some(tier) = match_name_tier(style_name, &self.query_lower) else { continue };
            let prop = designos_doc::style_property(doc, style_key);
            let prop_label = style_property_label(prop);
            insert_bounded(
                &mut self.buckets[MentionCategory::Styles.slot_index()],
                MentionCandidate {
                    category: MentionCategory::Styles,
                    kind: MentionKind::Style,
                    id: style_key.clone(),
                    name: style_name.to_string(),
                    page: Some(prop_label.to_string()),
                    caption: prop_label.to_string(),
                    icon: Icon::Styles,
                    hidden: false,
                    on_current_page: false,
                    match_tier: tier,
                },
            );
        }
    }

    fn inspect_node(&mut self, doc: &Document, node: &Node, hidden: bool) {
        if matches!(node.kind, NodeKind::Page { .. }) {
            return;
        }
        let category =
            if matches!(node.kind, NodeKind::Component(_) | NodeKind::ComponentSet(_)) { MentionCategory::Components } else { MentionCategory::Layers };
        let on_current = self.current_root_page == self.current_page;
        let Some(tier) = match_name_tier(&node.name, &self.query_lower) else {
            return;
        };
        let slot = category.slot_index();
        if worse_than_bucket_tail(&self.buckets[slot], tier, on_current, hidden) {
            return;
        }

        let caption = self.build_node_caption(doc, hidden);
        insert_bounded(
            &mut self.buckets[slot],
            MentionCandidate {
                category,
                kind: MentionKind::Node,
                id: node.id.0.to_string(),
                name: node.name.clone(),
                page: Some(self.current_root_page_name.clone()),
                caption,
                icon: icon_for_node_kind(&node.kind),
                hidden,
                on_current_page: on_current,
                match_tier: tier,
            },
        );
    }

    fn build_node_caption(&self, doc: &Document, hidden: bool) -> String {
        let mut path = self.current_root_page_name.clone();
        // Include up to 2 ancestor container names below the page for a concise path hint.
        let ancestors: Vec<&str> = self.dfs_stack.iter().skip(1).filter_map(|e| doc.nodes.get(&e.node_id).map(|n| n.name.as_str())).collect();
        if !ancestors.is_empty() {
            let start = ancestors.len().saturating_sub(2);
            for anc in &ancestors[start..] {
                path.push_str(" / ");
                path.push_str(anc);
            }
        }
        if hidden {
            path.push_str(" · Hidden");
        }
        path
    }

    /// Returns the currently accumulated candidates in category display order
    /// (`Layers`, `Pages`, `Components`, `Variables`, `Styles`), capped at 8 per category and 30 total.
    pub fn candidates(&self) -> Vec<MentionCandidate> {
        let mut out = Vec::with_capacity(MAX_TOTAL_RESULTS);
        for cat in MentionCategory::ALL {
            for item in self.buckets[cat.slot_index()].iter().take(MAX_PER_CATEGORY) {
                if out.len() >= MAX_TOTAL_RESULTS {
                    return out;
                }
                out.push(item.clone());
            }
        }
        out
    }
}

/// Runs a complete search across all 5 categories in `doc` (used by unit tests and small documents).
#[cfg(test)]
pub fn search_document(doc: &Document, current_page: NodeId, query: &str) -> Vec<MentionCandidate> {
    let mut job = MentionSearchJob::start(doc, current_page, query, Duration::from_secs(5));
    while !job.is_finished() {
        job.step(doc, current_page, Duration::from_secs(5));
    }
    job.candidates()
}

/// Detects an active `@<query>` trigger at `cursor_byte` in `text`.
///
/// Trigger rules (`#655` §5):
/// - Typing `@` at the start of the text (`at_idx == 0`) or after whitespace opens the popup.
/// - `@` preceded by a non-whitespace character (e.g. `a@b.com`) does NOT trigger.
/// - Returns `Some((at_byte_index, query_slice))` when the cursor is inside an `@`-token.
pub fn detect_at_query(text: &str, cursor_byte: usize) -> Option<(usize, &str)> {
    let mut end = cursor_byte.min(text.len());
    while end > 0 && !text.is_char_boundary(end) {
        end -= 1;
    }
    let before = &text[..end];
    // Find the start of the current whitespace-delimited token before `end`.
    let token_start = before.rfind(char::is_whitespace).map(|idx| idx + before[idx..].chars().next().map_or(1, char::len_utf8)).unwrap_or(0);
    let token = &before[token_start..];
    let query = token.strip_prefix('@')?;
    // Confirm `@` is at byte 0 or preceded by whitespace (true by construction of `token_start`).
    if token_start > 0 {
        let prev_char = before[..token_start].chars().next_back()?;
        if !prev_char.is_whitespace() {
            return None;
        }
    }
    Some((token_start, query))
}

/// Formats a stable `designos:<kind>/<id>` reference URI (`#655` §8).
pub fn format_reference_uri(kind: MentionKind, id: &str) -> String {
    format!("designos:{}/{}", kind.as_str(), id.trim())
}

/// Parses a `designos:<kind>/<id>` URI or a `References:` footer line containing one,
/// returning `(MentionKind, id)`.
#[cfg(test)]
pub fn parse_reference_uri(input: &str) -> Option<(MentionKind, String)> {
    let marker = "designos:";
    let pos = input.find(marker)?;
    let after = &input[pos + marker.len()..];
    let token_end = after.find(|c: char| c.is_whitespace() || c == ')' || c == ',').unwrap_or(after.len());
    let token = &after[..token_end];
    let (kind_str, id_str) = token.split_once('/')?;
    let kind = MentionKind::parse(kind_str)?;
    if id_str.is_empty() {
        return None;
    }
    Some((kind, id_str.to_string()))
}

/// Formats one `Mention` entry for the `References:` footer.
pub fn format_mention_reference_line(mention: &Mention) -> String {
    let uri = format_reference_uri(mention.kind, &mention.id);
    let detail = match (mention.kind, mention.page.as_deref()) {
        (MentionKind::Node, Some(page)) if !page.is_empty() => format!("layer \"{}\", page \"{}\"", mention.name, page),
        (MentionKind::Node, _) => format!("layer \"{}\"", mention.name),
        (MentionKind::Variable, Some(col)) if !col.is_empty() => format!("variable \"{}\", collection \"{}\"", mention.name, col),
        (MentionKind::Variable, _) => format!("variable \"{}\"", mention.name),
        (MentionKind::Style, Some(prop)) if !prop.is_empty() => format!("{} \"{}\"", prop.to_lowercase(), mention.name),
        (MentionKind::Style, _) => format!("style \"{}\"", mention.name),
    };
    format!("References: @{} = {uri} ({detail})", mention.label)
}

/// Formats the reference snippet pasted into the active agent PTY tab when picking from the
/// terminal header `Mention` button (`#655` §4).
pub fn format_terminal_paste_mention(mention: &Mention) -> String {
    let uri = format_reference_uri(mention.kind, &mention.id);
    format!("@{} ({uri}) ", mention.label)
}

/// True when `@<label>` is still present as a mention token in `prompt`.
pub fn mention_present_in_prompt(prompt: &str, label: &str) -> bool {
    let clean = label.trim();
    if clean.is_empty() {
        return false;
    }
    let needle = format!("@{clean}");
    let mut search_from = 0usize;
    while let Some(rel) = prompt[search_from..].find(&needle) {
        let start = search_from + rel;
        let end = start + needle.len();
        let valid_before = if start == 0 { true } else { prompt[..start].chars().next_back().is_some_and(char::is_whitespace) };
        let valid_after = if end >= prompt.len() {
            true
        } else {
            prompt[end..].chars().next().is_some_and(|c| c.is_whitespace() || matches!(c, ',' | '.' | ';' | ':' | '!' | '?' | ')' | ']'))
        };
        if valid_before && valid_after {
            return true;
        }
        search_from = start + 1;
        while search_from < prompt.len() && !prompt.is_char_boundary(search_from) {
            search_from += 1;
        }
    }
    false
}

/// Drops any [`Mention`] whose `@<label>` was deleted from `prompt`, and deduplicates
/// repeated mentions so each unique `(label, kind, id)` is kept once.
pub fn prune_deleted_mentions(prompt: &str, mentions: &mut Vec<Mention>) {
    let mut seen = Vec::<(String, MentionKind, String)>::new();
    mentions.retain(|m| {
        if !mention_present_in_prompt(prompt, &m.label) {
            return false;
        }
        let key = (m.label.clone(), m.kind, m.id.clone());
        if seen.contains(&key) {
            return false;
        }
        seen.push(key);
        true
    });
}

/// Builds the `References:` footer block for `prompt` and `mentions`, or `None` if no active
/// mentions remain in `prompt`.
pub fn format_references_footer(prompt: &str, mentions: &[Mention]) -> Option<String> {
    let mut active = mentions.to_vec();
    prune_deleted_mentions(prompt, &mut active);
    if active.is_empty() {
        return None;
    }
    let lines: Vec<String> = active.iter().map(format_mention_reference_line).collect();
    Some(lines.join("\n"))
}

/// Appends the `References:` footer block to `prompt` when active mentions are present in `prompt`.
pub fn build_prompt_with_mentions(prompt: &str, mentions: &[Mention]) -> String {
    let trimmed = prompt.trim();
    match format_references_footer(trimmed, mentions) {
        Some(footer) => format!("{trimmed}\n\n{footer}"),
        None => trimmed.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use designos_doc::{Color, Paint, Rect as DocRect, Style, StyleMeta, Variable, VariableCollection, VariableMode, VariableValue};
    use std::collections::BTreeMap;

    fn sample_five_kind_doc() -> (Document, NodeId, NodeId, NodeId, String, String) {
        let mut doc = Document::new("Mention Test Doc");
        let page1 = doc.add_page("Landing Showcase");
        let page2 = doc.add_page("Checkout");

        let layer_id = doc.alloc_id();
        let mut layer = Node::frame(layer_id, DocRect::new(0.0, 0.0, 200.0, 80.0));
        layer.name = "Hero/Title Showcase".to_string();
        doc.insert(page1, layer).unwrap();

        let comp_id = doc.alloc_id();
        let mut comp = Node::new(comp_id, NodeKind::Component(designos_doc::FrameProps::default()));
        comp.name = "Button/Showcase".to_string();
        doc.insert(page1, comp).unwrap();

        let var_id = "var-showcase-1".to_string();
        doc.variable_collections.insert(
            "col-1".to_string(),
            VariableCollection {
                id: "col-1".to_string(),
                name: "Tokens".to_string(),
                modes: vec![VariableMode { id: "m1".to_string(), name: "Light".to_string() }],
                default_mode: "m1".to_string(),
                variables: vec![var_id.clone()],
                key: None,
                source_library_key: None,
                extensions: Default::default(),
            },
        );
        let mut values = BTreeMap::new();
        values.insert("m1".to_string(), VariableValue::Color(Color::rgba(13, 153, 255, 255)));
        doc.variables.insert(
            var_id.clone(),
            Variable {
                id: var_id.clone(),
                name: "Color/Showcase".to_string(),
                collection: "col-1".to_string(),
                kind: VariableKind::Color,
                values,
                description: String::new(),
                scopes: Vec::new(),
                hidden_from_publishing: false,
                key: None,
                code_syntax: BTreeMap::new(),
                extensions: Default::default(),
                mode_extensions: Default::default(),
            },
        );

        let style_key = "style-showcase-1".to_string();
        doc.styles.insert(style_key.clone(), Style::Fill(vec![Paint::solid(Color::rgba(24, 24, 27, 255))]));
        doc.style_meta.insert(
            style_key.clone(),
            StyleMeta {
                name: "Brand/Showcase".to_string(),
                key: None,
                description: String::new(),
                property: StyleProperty::Fill,
                bindings: Default::default(),
            },
        );

        let _ = page2;
        (doc, page1, layer_id, comp_id, var_id, style_key)
    }

    #[test]
    fn search_finds_all_five_kinds() {
        let (doc, page1, layer_id, comp_id, var_id, style_key) = sample_five_kind_doc();
        let results = search_document(&doc, page1, "showcase");

        assert!(results.iter().any(|c| c.category == MentionCategory::Layers && c.id == layer_id.0.to_string()));
        assert!(results.iter().any(|c| c.category == MentionCategory::Pages && c.id == page1.0.to_string()));
        assert!(results.iter().any(|c| c.category == MentionCategory::Components && c.id == comp_id.0.to_string()));
        assert!(results.iter().any(|c| c.category == MentionCategory::Variables && c.id == var_id));
        assert!(results.iter().any(|c| c.category == MentionCategory::Styles && c.id == style_key));
    }

    #[test]
    fn search_ranks_current_page_first() {
        let mut doc = Document::new("Rank Test");
        let page1 = doc.add_page("Page 1");
        let page2 = doc.add_page("Page 2");

        let n1 = doc.alloc_id();
        let mut node1 = Node::frame(n1, DocRect::new(0.0, 0.0, 100.0, 40.0));
        node1.name = "Header Banner".to_string();
        doc.insert(page1, node1).unwrap();

        let n2 = doc.alloc_id();
        let mut node2 = Node::frame(n2, DocRect::new(0.0, 0.0, 100.0, 40.0));
        node2.name = "Header Banner".to_string();
        doc.insert(page2, node2).unwrap();

        let results = search_document(&doc, page2, "header");
        let layers: Vec<&MentionCandidate> = results.iter().filter(|c| c.category == MentionCategory::Layers).collect();
        assert_eq!(layers.len(), 2);
        assert_eq!(layers[0].id, n2.0.to_string(), "layer on current_page must rank above layer on another page");
        assert_eq!(layers[1].id, n1.0.to_string());
    }

    #[test]
    fn search_is_budgeted_on_a_huge_document() {
        let mut doc = Document::new("Huge Doc");
        let page = doc.add_page("Huge Page");
        for i in 0..100_000u64 {
            let id = NodeId(10 + i);
            let mut node = Node::frame(id, DocRect::new(0.0, 0.0, 10.0, 10.0));
            node.parent = Some(page);
            node.name = if i == 500 || i == 99_999 { format!("TargetNeedle {i}") } else { format!("Item {i}") };
            doc.nodes.insert(id, node);
            if let Some(p) = doc.nodes.get_mut(&page) {
                p.children.push(id);
            }
        }

        let mut job = MentionSearchJob::start(&doc, page, "TargetNeedle", DEFAULT_FRAME_BUDGET);
        assert!(!job.is_finished(), "100,000-node scan must yield partial results after the first budgeted step");
        let partial = job.candidates();
        assert!(partial.iter().any(|c| c.name == "TargetNeedle 500"), "first budgeted step must already include early matches");

        let mut safety_steps = 0usize;
        while !job.is_finished() {
            job.step(&doc, page, DEFAULT_FRAME_BUDGET);
            safety_steps += 1;
            assert!(safety_steps < 1000, "budgeted search must finish in bounded steps");
        }
        let final_results = job.candidates();
        assert!(final_results.iter().any(|c| c.name == "TargetNeedle 500"));
        assert!(final_results.iter().any(|c| c.name == "TargetNeedle 99999"));
    }

    #[test]
    fn footer_lists_each_mention_once() {
        let m = Mention {
            label: "Hero/Title".to_string(),
            kind: MentionKind::Node,
            id: "4521".to_string(),
            name: "Hero/Title".to_string(),
            page: Some("Landing".to_string()),
        };
        let prompt = "Compare @Hero/Title with @Hero/Title on mobile";
        let built = build_prompt_with_mentions(prompt, &[m.clone(), m]);
        let ref_lines: Vec<&str> = built.lines().filter(|l| l.starts_with("References:")).collect();
        assert_eq!(ref_lines.len(), 1, "duplicate mention must be emitted only once in References footer: {built}");
        assert!(ref_lines[0].contains("@Hero/Title = designos:node/4521"));
    }

    #[test]
    fn deleted_label_is_not_sent() {
        let m = Mention {
            label: "Hero/Title".to_string(),
            kind: MentionKind::Node,
            id: "4521".to_string(),
            name: "Hero/Title".to_string(),
            page: Some("Landing".to_string()),
        };
        // Simulate picking `@Hero/Title` then backspacing over the last character.
        let edited_prompt = "Make @Hero/Titl bold";
        let built = build_prompt_with_mentions(edited_prompt, &[m]);
        assert!(!built.contains("References:"), "deleted mention label must not emit a References block: {built}");
        assert_eq!(built, edited_prompt);
    }

    #[test]
    fn grammar_round_trip() {
        let mentions = vec![
            Mention {
                label: "Hero/Title".to_string(),
                kind: MentionKind::Node,
                id: "4521".to_string(),
                name: "Hero/Title".to_string(),
                page: Some("Landing".to_string()),
            },
            Mention {
                label: "Color/Primary".to_string(),
                kind: MentionKind::Variable,
                id: "var-primary-1".to_string(),
                name: "Color/Primary".to_string(),
                page: Some("Tokens".to_string()),
            },
            Mention {
                label: "Brand/Heading".to_string(),
                kind: MentionKind::Style,
                id: "S:heading-1".to_string(),
                name: "Brand/Heading".to_string(),
                page: Some("Text style".to_string()),
            },
        ];
        let prompt = "Use @Hero/Title with @Color/Primary and @Brand/Heading";
        let footer = format_references_footer(prompt, &mentions).expect("footer should be emitted");
        let parsed: Vec<(MentionKind, String)> =
            footer.lines().map(|line| parse_reference_uri(line).expect("every footer line must round-trip through parse_reference_uri")).collect();
        assert_eq!(
            parsed,
            vec![
                (MentionKind::Node, "4521".to_string()),
                (MentionKind::Variable, "var-primary-1".to_string()),
                (MentionKind::Style, "S:heading-1".to_string()),
            ]
        );
    }

    #[test]
    fn mentioned_layer_is_resolved_through_designos_mcp() {
        let (doc, page1, layer_id, _comp_id, _var_id, _style_key) = sample_five_kind_doc();
        let candidates = search_document(&doc, page1, "Hero/Title Showcase");
        let cand = candidates.iter().find(|c| c.id == layer_id.0.to_string()).expect("layer candidate found");
        let mention = Mention::from_candidate(cand);
        let prompt = format!("Inspect @{}", mention.label);
        let footer = format_references_footer(&prompt, &[mention]).expect("footer generated");
        let (kind, id_str) = parse_reference_uri(&footer).expect("parsed reference URI");
        assert_eq!(kind, MentionKind::Node);
        let parsed_node_id: u64 = id_str.parse().expect("u64 node id");

        let mut engine = designos_mcp::engine::Engine { doc, ..Default::default() };
        let res = engine.call("designos_get_node", serde_json::json!({ "node_id": parsed_node_id })).expect("designos_get_node must succeed");
        assert_eq!(res["node"]["name"], "Hero/Title Showcase");
        assert_eq!(res["node"]["id"], layer_id.0);
    }

    #[test]
    fn style_and_variable_resolution_use_existing_tools() {
        let (doc, page1, _layer_id, _comp_id, var_id, style_key) = sample_five_kind_doc();
        let candidates = search_document(&doc, page1, "showcase");
        let var_cand = candidates.iter().find(|c| c.category == MentionCategory::Variables).expect("variable candidate");
        let style_cand = candidates.iter().find(|c| c.category == MentionCategory::Styles).expect("style candidate");

        let var_mention = Mention::from_candidate(var_cand);
        let style_mention = Mention::from_candidate(style_cand);
        let prompt = format!("Check @{} and @{}", var_mention.label, style_mention.label);
        let footer = format_references_footer(&prompt, &[var_mention, style_mention]).expect("footer");
        let lines: Vec<&str> = footer.lines().collect();
        assert_eq!(lines.len(), 2);

        let (v_kind, v_id) = parse_reference_uri(lines[0]).unwrap();
        let (s_kind, s_id) = parse_reference_uri(lines[1]).unwrap();
        assert_eq!((v_kind, v_id.as_str()), (MentionKind::Variable, var_id.as_str()));
        assert_eq!((s_kind, s_id.as_str()), (MentionKind::Style, style_key.as_str()));

        let mut engine = designos_mcp::engine::Engine { doc, ..Default::default() };

        let vars_res = engine.call("designos_get_variables", serde_json::json!({})).expect("designos_get_variables must succeed");
        assert_eq!(vars_res["variables"][&v_id]["name"], "Color/Showcase");

        let doc_res = engine.call("designos_get_document", serde_json::json!({})).expect("designos_get_document must succeed");
        let style_entry = doc_res["styles"]
            .as_array()
            .expect("styles array")
            .iter()
            .find(|s| s["key"] == s_id)
            .expect("style key must exist in designos_get_document styles");
        assert_eq!(style_entry["name"], "Brand/Showcase");
    }

    #[test]
    fn no_popup_inside_an_email() {
        let text = "contact a@b.com";
        for cursor in 0..=text.len() {
            assert_eq!(detect_at_query(text, cursor), None, "email address must never open the @-mention popup at cursor={cursor}");
        }
        assert_eq!(detect_at_query("@hero", 5), Some((0, "hero")));
        assert_eq!(detect_at_query("check @button", 13), Some((6, "button")));
    }
}
