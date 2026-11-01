//! Find and replace over text layers and layer names, and "select matching
//! layers" queries. Everything here is pure: searches read a [`Document`] and
//! edits come back as a [`Command`], so the app records them as one undo step.
//!
//! Ranges are UTF-8 byte ranges that always sit on char boundaries, so they can
//! be used to slice `TextProps::content` (or `Node::name`) directly.

use crate::*;
use regex::{Regex, RegexBuilder};
use std::collections::{BTreeMap, HashSet};
use std::ops::Range;

/// Options of a text search.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FindOptions {
    /// Compare case sensitively (default: Unicode case-insensitive).
    pub match_case: bool,
    /// Only accept matches that are not glued to a letter, digit or `_`.
    pub whole_word: bool,
    /// Treat the query as a regular expression (`^`/`$` match at line ends).
    pub regex: bool,
}

/// Where to search.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Scope {
    /// Every layer below a page.
    Page(NodeId),
    /// These layers and everything below them (for example the selection).
    Nodes(Vec<NodeId>),
    /// Every page of the document.
    Document,
}

/// What a match was found in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MatchField {
    /// The characters of a text layer.
    Text,
    /// The layer name.
    Name,
}

/// One hit of [`find_text`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Match {
    pub node: NodeId,
    pub field: MatchField,
    /// UTF-8 byte range in the text content or in the name, on char boundaries.
    pub range: Range<usize>,
    /// Regex searches only: capture groups `0..` (0 is the whole match), used
    /// to expand `$1` in the replacement.
    pub groups: Option<Vec<Option<String>>>,
}

/// A compiled query. Build it once and run it over many strings.
#[derive(Clone, Debug)]
pub struct Matcher {
    re: Regex,
    whole_word: bool,
    captures: bool,
}

impl Matcher {
    /// Compiles `query`. An empty query and an invalid regex are errors; the
    /// message of the latter is the regex engine's.
    pub fn new(query: &str, opts: FindOptions) -> Result<Matcher, String> {
        if query.is_empty() {
            return Err(String::new());
        }
        let pattern = if opts.regex { query.to_string() } else { regex::escape(query) };
        let re = RegexBuilder::new(&pattern)
            .case_insensitive(!opts.match_case)
            .multi_line(true)
            .size_limit(1 << 20)
            .build()
            .map_err(|e| first_line(&e.to_string()))?;
        Ok(Matcher { re, whole_word: opts.whole_word, captures: opts.regex })
    }

    /// All non-overlapping, non-empty matches in `text`, left to right, with
    /// their capture groups when this is a regex search.
    #[allow(clippy::type_complexity)]
    pub fn find_all(&self, text: &str) -> Vec<(Range<usize>, Option<Vec<Option<String>>>)> {
        let mut out = Vec::new();
        let mut pos = 0;
        while pos <= text.len() {
            let (range, groups) = if self.captures {
                let Some(caps) = self.re.captures_at(text, pos) else { break };
                let m = caps.get(0).expect("group 0 always exists");
                let groups = caps.iter().map(|g| g.map(|g| g.as_str().to_string())).collect();
                (m.range(), Some(groups))
            } else {
                let Some(m) = self.re.find_at(text, pos) else { break };
                (m.range(), None)
            };
            if range.is_empty() {
                pos = next_boundary(text, range.end);
            } else if self.whole_word && !is_whole_word(text, &range) {
                pos = next_boundary(text, range.start);
            } else {
                pos = range.end;
                out.push((range, groups));
            }
        }
        out
    }
}

fn first_line(s: &str) -> String {
    // The regex crate prints a multi-line report; its last "error:" line is the useful one.
    s.lines().rev().find_map(|l| l.trim().strip_prefix("error: ")).unwrap_or_else(|| s.lines().next().unwrap_or("invalid pattern")).to_string()
}

/// Index of the char boundary after the char starting at `i` (`len + 1` past the end).
fn next_boundary(text: &str, i: usize) -> usize {
    text[i.min(text.len())..].chars().next().map_or(text.len() + 1, |c| i + c.len_utf8())
}

fn is_word_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

fn is_whole_word(text: &str, r: &Range<usize>) -> bool {
    let before = text[..r.start].chars().next_back();
    let after = text[r.end..].chars().next();
    !before.is_some_and(is_word_char) && !after.is_some_and(is_word_char)
}

// ----------------------------------------------------------------- traversal
/// Layers of `scope` in layers-panel order: top-most first, parents before their children.
fn scope_nodes(doc: &Document, scope: &Scope) -> Vec<NodeId> {
    fn rec(doc: &Document, id: NodeId, seen: &mut HashSet<NodeId>, out: &mut Vec<NodeId>) {
        let Some(n) = doc.get(id) else { return };
        if !seen.insert(id) {
            return;
        }
        if !matches!(n.kind, NodeKind::Page { .. }) {
            out.push(id);
        }
        for c in n.children.iter().rev() {
            rec(doc, *c, seen, out);
        }
    }
    let roots: Vec<NodeId> = match scope {
        Scope::Page(p) => vec![*p],
        Scope::Nodes(v) => v.clone(),
        Scope::Document => doc.pages.clone(),
    };
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for r in roots {
        rec(doc, r, &mut seen, &mut out);
    }
    out
}

/// Searches the text content and the names of the layers in `scope`. Results
/// follow the layers panel order (top-most layer first). A text layer whose
/// name is just its own text is reported once, for its content.
pub fn find_text(doc: &Document, scope: &Scope, query: &str, opts: FindOptions) -> Vec<Match> {
    let Ok(matcher) = Matcher::new(query, opts) else { return Vec::new() };
    find_with(doc, scope, &matcher)
}

/// [`find_text`] with an already compiled query.
pub fn find_with(doc: &Document, scope: &Scope, matcher: &Matcher) -> Vec<Match> {
    let mut out = Vec::new();
    for id in scope_nodes(doc, scope) {
        let Some(n) = doc.get(id) else { continue };
        // text layers, and the text of stickies, shapes and connector labels
        let own_text = n.kind.any_text();
        if let Some(t) = own_text {
            for (range, groups) in matcher.find_all(&t.content) {
                out.push(Match { node: id, field: MatchField::Text, range, groups });
            }
            if n.name == t.content {
                continue;
            }
        }
        for (range, groups) in matcher.find_all(&n.name) {
            out.push(Match { node: id, field: MatchField::Name, range, groups });
        }
    }
    out
}

// ------------------------------------------------------------------- replace
/// Expands `$0`..`$9`, `${n}` and `$$` in `template` from regex capture groups.
fn expand(template: &str, groups: &[Option<String>]) -> String {
    let mut out = String::with_capacity(template.len());
    let mut it = template.char_indices().peekable();
    while let Some((i, c)) = it.next() {
        if c != '$' {
            out.push(c);
            continue;
        }
        match it.peek().copied() {
            Some((_, '$')) => {
                it.next();
                out.push('$');
            }
            Some((_, d)) if d.is_ascii_digit() => {
                it.next();
                out.push_str(groups.get(d as usize - '0' as usize).and_then(|g| g.as_deref()).unwrap_or(""));
            }
            Some((_, '{')) => {
                let rest = &template[i + 2..];
                match rest.find('}').filter(|e| *e > 0 && rest[..*e].bytes().all(|b| b.is_ascii_digit())) {
                    Some(e) => {
                        let n: usize = rest[..e].parse().unwrap_or(usize::MAX);
                        out.push_str(groups.get(n).and_then(|g| g.as_deref()).unwrap_or(""));
                        for _ in 0..e + 2 {
                            it.next();
                        }
                    }
                    None => out.push('$'),
                }
            }
            _ => out.push('$'),
        }
    }
    out
}

/// Replaces `range` of the text and keeps the runs and lists aligned: the new
/// text takes the style of the first replaced char (its run, or the base style).
fn splice_text(t: &mut TextProps, range: Range<usize>, new: &str) {
    crate::replace_text_range(t, range, new);
}

fn valid_range(s: &str, r: &Range<usize>) -> bool {
    r.start < r.end && r.end <= s.len() && s.is_char_boundary(r.start) && s.is_char_boundary(r.end)
}

/// Replaces every match with `replacement` as one [`Command`] (a single undo
/// step). In regex searches `$1`, `${1}` and `$$` expand from the capture
/// groups. Replaced text keeps the run styles around it and takes the style of
/// the first char it replaces; the node sizes are left for the caller to refit.
/// Stale matches (ranges no longer valid for the document) are skipped.
pub fn replace(doc: &Document, matches: &[Match], replacement: &str) -> Command {
    let mut by_node: BTreeMap<NodeId, Vec<&Match>> = BTreeMap::new();
    for m in matches {
        by_node.entry(m.node).or_default().push(m);
    }
    let mut cmds = Vec::new();
    for (id, mut hits) in by_node {
        let Some(orig) = doc.get(id) else { continue };
        let mut node = orig.clone();
        // Back to front so earlier ranges stay valid.
        hits.sort_by_key(|m| std::cmp::Reverse((m.field == MatchField::Name, m.range.start)));
        let mut floor_text = usize::MAX;
        let mut floor_name = usize::MAX;
        for m in hits {
            let new = match &m.groups {
                Some(g) => expand(replacement, g),
                None => replacement.to_string(),
            };
            match m.field {
                MatchField::Text => {
                    let Some(t) = node.kind.any_text_mut() else { continue };
                    if m.range.end > floor_text || !valid_range(&t.content, &m.range) {
                        continue;
                    }
                    splice_text(t, m.range.clone(), &new);
                    floor_text = m.range.start;
                }
                MatchField::Name => {
                    if m.range.end > floor_name || !valid_range(&node.name, &m.range) {
                        continue;
                    }
                    node.name.replace_range(m.range.clone(), &new);
                    floor_name = m.range.start;
                }
            }
        }
        // A text layer named after its content keeps following it.
        if let (NodeKind::Text(old), NodeKind::Text(new)) = (&orig.kind, &node.kind) {
            if orig.name == old.content && node.name == orig.name {
                node.name = new.content.clone();
            }
        }
        if node != *orig {
            cmds.push(Command::Update { node: Box::new(node) });
        }
    }
    Command::Batch(cmds)
}

// ------------------------------------------------------------ select matching
/// What "select matching layers" compares.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MatchCriteria {
    Fill,
    Stroke,
    Effect,
    /// Text style properties on text layers.
    TextProperties,
    /// Font family, weight and size of text layers.
    Font,
    /// Instances of the same main component or component set.
    Instance,
    /// Instances of the same variant of a component set.
    Variant,
    /// Same kind of layer (rectangle, frame, text, ...).
    Type,
    Name,
    /// Same type and the same visual properties.
    Properties,
}

fn fills_of(n: &Node) -> &[Paint] {
    match &n.kind {
        NodeKind::Text(t) => &t.style.fills,
        _ => &n.fills,
    }
}

fn main_of(n: &Node) -> Option<NodeId> {
    match &n.kind {
        NodeKind::Instance { component, .. } => *component,
        NodeKind::Component(_) => Some(n.id),
        _ => None,
    }
}

fn enclosing_instance_or_component_node(doc: &Document, mut id: NodeId) -> Option<NodeId> {
    loop {
        let n = doc.get(id)?;
        if matches!(n.kind, NodeKind::Instance { component: Some(_), .. } | NodeKind::Component(_)) {
            return Some(id);
        }
        id = n.parent?;
    }
}

fn enclosing_main_of(doc: &Document, id: NodeId) -> Option<NodeId> {
    let holder = enclosing_instance_or_component_node(doc, id)?;
    doc.get(holder).and_then(main_of)
}

fn component_family_of(doc: &Document, main_id: NodeId) -> NodeId {
    doc.get(main_id).and_then(|n| n.parent).filter(|&p| doc.get(p).is_some_and(|pn| matches!(pn.kind, NodeKind::ComponentSet(_)))).unwrap_or(main_id)
}

fn variant_main_of(doc: &Document, id: NodeId) -> Option<NodeId> {
    let main_id = enclosing_main_of(doc, id)?;
    let parent = doc.get(main_id)?.parent?;
    doc.get(parent).is_some_and(|pn| matches!(pn.kind, NodeKind::ComponentSet(_))).then_some(main_id)
}

/// True when `Select all with same instance` should be enabled for `selection`
/// (a single node that is, or sits inside, an instance or component).
pub fn can_select_same_instance(doc: &Document, selection: &[NodeId]) -> bool {
    selection.len() == 1 && enclosing_main_of(doc, selection[0]).is_some()
}

/// True when `Select all with same variant` should be enabled for `selection`
/// (a single node that is, or sits inside, a component-set variant instance).
pub fn can_select_same_variant(doc: &Document, selection: &[NodeId]) -> bool {
    selection.len() == 1 && variant_main_of(doc, selection[0]).is_some()
}

fn same_kind(a: &Node, b: &Node) -> bool {
    std::mem::discriminant(&a.kind) == std::mem::discriminant(&b.kind)
}

fn same_fill(a: &Node, b: &Node) -> bool {
    fills_of(a) == fills_of(b) && a.style_refs.get("fill") == b.style_refs.get("fill")
}

fn same_stroke(a: &Node, b: &Node) -> bool {
    a.stroke == b.stroke && a.style_refs.get("stroke") == b.style_refs.get("stroke")
}

fn same_effects(a: &Node, b: &Node) -> bool {
    a.effects == b.effects && a.style_refs.get("effect") == b.style_refs.get("effect")
}

fn same_text_properties(a: &Node, b: &Node) -> bool {
    match (&a.kind, &b.kind) {
        (NodeKind::Text(x), NodeKind::Text(y)) => x.style == y.style && a.style_refs.get("text") == b.style_refs.get("text"),
        _ => false,
    }
}

fn same_font(a: &Node, b: &Node) -> bool {
    match (&a.kind, &b.kind) {
        (NodeKind::Text(x), NodeKind::Text(y)) => {
            x.style.font_family == y.style.font_family && x.style.font_weight == y.style.font_weight && x.style.font_size == y.style.font_size
        }
        _ => false,
    }
}

fn same_properties(a: &Node, b: &Node) -> bool {
    if !same_kind(a, b) || !same_fill(a, b) || !same_stroke(a, b) || !same_effects(a, b) {
        return false;
    }
    let shape = a.corner_radii == b.corner_radii
        && a.corner_smoothing == b.corner_smoothing
        && a.opacity == b.opacity
        && a.blend == b.blend
        && a.is_mask == b.is_mask
        && a.style_refs == b.style_refs;
    let kind = match (&a.kind, &b.kind) {
        (NodeKind::Text(x), NodeKind::Text(y)) => x.style == y.style,
        _ => a.kind.frame_props().map(|f| f.clips_content) == b.kind.frame_props().map(|f| f.clips_content),
    };
    shape && kind
}

fn has_criteria(doc: &Document, n: &Node, c: MatchCriteria) -> bool {
    match c {
        MatchCriteria::Fill => !fills_of(n).is_empty(),
        MatchCriteria::Stroke => !n.stroke.paints.is_empty(),
        MatchCriteria::Effect => !n.effects.is_empty(),
        MatchCriteria::TextProperties | MatchCriteria::Font => matches!(n.kind, NodeKind::Text(_)),
        MatchCriteria::Instance => enclosing_main_of(doc, n.id).is_some(),
        MatchCriteria::Variant => variant_main_of(doc, n.id).is_some(),
        MatchCriteria::Type | MatchCriteria::Name | MatchCriteria::Properties => true,
    }
}

fn matches_criteria(doc: &Document, reference: &Node, n: &Node, c: MatchCriteria) -> bool {
    match c {
        MatchCriteria::Fill => same_fill(reference, n),
        MatchCriteria::Stroke => same_stroke(reference, n),
        MatchCriteria::Effect => same_effects(reference, n),
        MatchCriteria::TextProperties => same_text_properties(reference, n),
        MatchCriteria::Font => same_font(reference, n),
        MatchCriteria::Instance => {
            let Some(ref_main) = enclosing_main_of(doc, reference.id) else { return false };
            let Some(cand_main) = main_of(n) else { return false };
            component_family_of(doc, ref_main) == component_family_of(doc, cand_main) && (matches!(n.kind, NodeKind::Instance { .. }) || n.id == reference.id)
        }
        MatchCriteria::Variant => {
            let Some(ref_var) = variant_main_of(doc, reference.id) else { return false };
            main_of(n) == Some(ref_var) && (matches!(n.kind, NodeKind::Instance { .. }) || n.id == reference.id)
        }
        MatchCriteria::Type => same_kind(reference, n),
        MatchCriteria::Name => reference.name == n.name,
        MatchCriteria::Properties => same_properties(reference, n),
    }
}

/// Whether the canvas lets you select `id` right now (shown and unlocked).
pub fn selectable(doc: &Document, id: NodeId) -> bool {
    doc.get(id).is_some_and(|n| n.visible && !n.locked && !matches!(n.kind, NodeKind::Page { .. }))
}

fn selectable_in_hierarchy(doc: &Document, mut id: NodeId) -> bool {
    loop {
        let Some(n) = doc.get(id) else { return false };
        if matches!(n.kind, NodeKind::Page { .. }) {
            return true;
        }
        if !n.visible || n.locked {
            return false;
        }
        let Some(p) = n.parent else { return false };
        id = p;
    }
}

/// Layers that match `node` on `criteria`, on the page of `node` (the layer
/// itself included). A reference without the property (no fill, no stroke...)
/// matches only itself. Hidden and locked layers are left out.
pub fn select_matching(doc: &Document, node: NodeId, criteria: MatchCriteria) -> Vec<NodeId> {
    match doc.page_of(node) {
        Some(page) => select_matching_in(doc, node, criteria, &Scope::Page(page)),
        None => vec![node],
    }
}

/// [`select_matching`] restricted to `scope` (for example the selection's subtree).
pub fn select_matching_in(doc: &Document, node: NodeId, criteria: MatchCriteria, scope: &Scope) -> Vec<NodeId> {
    let ref_id = match criteria {
        MatchCriteria::Instance | MatchCriteria::Variant => enclosing_instance_or_component_node(doc, node).unwrap_or(node),
        _ => node,
    };
    let Some(reference) = doc.get(ref_id) else { return Vec::new() };
    if !has_criteria(doc, reference, criteria) {
        return vec![node];
    }
    let mut out: Vec<NodeId> = scope_nodes(doc, scope)
        .into_iter()
        .filter(|id| *id == ref_id || (selectable(doc, *id) && doc.get(*id).is_some_and(|n| matches_criteria(doc, reference, n, criteria))))
        .collect();
    if !out.contains(&ref_id) {
        out.push(ref_id);
    }
    out
}

/// Like [`select_matching`] for a whole selection: the union of what each
/// selected layer matches, without duplicates.
pub fn select_matching_all(doc: &Document, nodes: &[NodeId], criteria: MatchCriteria) -> Vec<NodeId> {
    let mut seen = HashSet::new();
    nodes.iter().flat_map(|n| select_matching(doc, *n, criteria)).filter(|id| seen.insert(*id)).collect()
}

// ------------------------------------------------ matching across containers
/// Scope within which cross-container matching layers are searched.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MatchScope {
    Page(NodeId),
    Section(NodeId),
    ComponentSet(NodeId),
}

/// Returns `(scope, scope_root)` for `id`, where `scope_root` is the containing
/// component-set variant (`Component` inside `ComponentSet`) or top-level `Frame`
/// or `Group` (inside a `Section` or `Page`). Returns `None` if `id` is not inside
/// a frame, group, or component variant.
pub fn matching_scope_of(doc: &Document, id: NodeId) -> Option<(MatchScope, NodeId)> {
    let n = doc.get(id)?;
    let mut cur = n.parent;
    let mut top_container: Option<(NodeId, NodeId)> = None;
    while let Some(cid) = cur {
        let cn = doc.get(cid)?;
        let parent_id = cn.parent?;
        let pn = doc.get(parent_id)?;
        if matches!(cn.kind, NodeKind::Component(_)) && matches!(pn.kind, NodeKind::ComponentSet(_)) {
            return Some((MatchScope::ComponentSet(parent_id), cid));
        }
        if matches!(cn.kind, NodeKind::Frame(_) | NodeKind::Group) && matches!(pn.kind, NodeKind::Page { .. } | NodeKind::Section) {
            top_container = Some((cid, parent_id));
        }
        if matches!(pn.kind, NodeKind::Page { .. }) {
            break;
        }
        cur = cn.parent;
    }
    let (root, parent_id) = top_container?;
    let pn = doc.get(parent_id)?;
    match pn.kind {
        NodeKind::Section => Some((MatchScope::Section(parent_id), root)),
        NodeKind::Page { .. } => Some((MatchScope::Page(parent_id), root)),
        _ => None,
    }
}

/// Effective layer name used when matching across containers: if an instance or
/// sublayer has been renamed (`OverrideField::Name`), uses its original source
/// component or layer name.
fn effective_matching_name<'a>(doc: &'a Document, n: &'a Node) -> &'a str {
    if n.component.overrides.contains(&OverrideField::Name) {
        if let Some(src_id) = n.component.source.or_else(|| main_of(n)) {
            if let Some(src) = doc.get(src_id) {
                if matches!(n.kind, NodeKind::Instance { .. }) {
                    if let Some(cs) = src.parent.and_then(|p| doc.get(p)).filter(|p| matches!(p.kind, NodeKind::ComponentSet(_))) {
                        return cs.name.as_str();
                    }
                }
                return src.name.as_str();
            }
        }
    }
    n.name.as_str()
}

fn same_named_sibling_index(doc: &Document, n: &Node) -> usize {
    let Some(pid) = n.parent else { return 0 };
    let Some(pn) = doc.get(pid) else { return 0 };
    let target = effective_matching_name(doc, n);
    let mut idx = 0;
    for &sib_id in &pn.children {
        if sib_id == n.id {
            break;
        }
        if let Some(sib) = doc.get(sib_id) {
            if effective_matching_name(doc, sib) == target {
                idx += 1;
            }
        }
    }
    idx
}

/// Matching layers must sit at the same name path from their scope root, with same-named sibling ties broken by sibling index; to be confirmed against Figma.
fn matching_hierarchy_position<'a>(doc: &'a Document, id: NodeId, scope_root: NodeId) -> Option<Vec<(&'a str, usize)>> {
    let mut path = Vec::new();
    let mut cur = doc.get(id)?.parent;
    while let Some(cid) = cur {
        if cid == scope_root {
            path.reverse();
            return Some(path);
        }
        let cn = doc.get(cid)?;
        path.push((effective_matching_name(doc, cn), same_named_sibling_index(doc, cn)));
        cur = cn.parent;
    }
    None
}

fn is_never_renamed_text(n: &Node) -> Option<&TextProps> {
    let NodeKind::Text(t) = &n.kind else { return None };
    if n.component.overrides.contains(&OverrideField::Name) {
        return None;
    }
    (n.name == t.content).then_some(t)
}

fn relative_pos_in_scope(doc: &Document, id: NodeId, scope_root: NodeId) -> Vec2 {
    let mut chain = Vec::new();
    let mut cur = Some(id);
    while let Some(cid) = cur {
        if cid == scope_root {
            break;
        }
        let Some(n) = doc.get(cid) else { break };
        chain.push(n.transform);
        cur = n.parent;
    }
    let xf = chain.iter().rev().fold(Affine::IDENTITY, |acc, t| acc * *t);
    xf.apply(Vec2::ZERO)
}

fn scope_compatible(doc: &Document, s_scope: MatchScope, c_scope: MatchScope) -> bool {
    match s_scope {
        MatchScope::ComponentSet(cs) => c_scope == MatchScope::ComponentSet(cs),
        MatchScope::Section(sec) => c_scope == MatchScope::Section(sec),
        MatchScope::Page(page) => match c_scope {
            MatchScope::Page(p) => p == page,
            MatchScope::Section(sec) => doc.page_of(sec) == Some(page),
            MatchScope::ComponentSet(_) => false,
        },
    }
}

/// True when candidate `c` matches selected layer `s` across top-level containers
/// (or variants within a component set).
pub fn matches_layer_across_containers(doc: &Document, s: NodeId, c: NodeId) -> bool {
    if s == c || !selectable_in_hierarchy(doc, c) {
        return false;
    }
    let Some(sn) = doc.get(s) else { return false };
    let Some(cn) = doc.get(c) else { return false };
    if !same_kind(sn, cn) {
        return false;
    }
    let Some((s_scope, s_root)) = matching_scope_of(doc, s) else { return false };
    let Some((c_scope, c_root)) = matching_scope_of(doc, c) else { return false };
    if s_root == c_root || !scope_compatible(doc, s_scope, c_scope) {
        return false;
    }
    let Some(s_path) = matching_hierarchy_position(doc, s, s_root) else { return false };
    let Some(c_path) = matching_hierarchy_position(doc, c, c_root) else { return false };
    if s_path != c_path {
        return false;
    }
    if let (Some(st), Some(ct)) = (is_never_renamed_text(sn), is_never_renamed_text(cn)) {
        if st.style != ct.style {
            return false;
        }
        let Some(c_parent) = cn.parent.and_then(|p| doc.get(p)) else { return false };
        let s_pos = relative_pos_in_scope(doc, s, s_root);
        let c_pos = relative_pos_in_scope(doc, c, c_root);
        let c_dist = (c_pos.x - s_pos.x).powi(2) + (c_pos.y - s_pos.y).powi(2);
        for &sib_id in &c_parent.children {
            if sib_id == c || !selectable_in_hierarchy(doc, sib_id) {
                continue;
            }
            let Some(sib) = doc.get(sib_id) else { continue };
            if let Some(sibt) = is_never_renamed_text(sib) {
                if sibt.style == st.style {
                    let sib_pos = relative_pos_in_scope(doc, sib_id, c_root);
                    let sib_dist = (sib_pos.x - s_pos.x).powi(2) + (sib_pos.y - s_pos.y).powi(2);
                    if sib_dist < c_dist - 1e-4 {
                        return false;
                    }
                }
            }
        }
        true
    } else {
        effective_matching_name(doc, sn) == effective_matching_name(doc, cn) && same_named_sibling_index(doc, sn) == same_named_sibling_index(doc, cn)
    }
}

/// Finds all selectable layers on the active page of `selection` that match at least
/// one layer in `selection` across containers, excluding layers already in `selection`.
pub fn find_matching_layers(doc: &Document, selection: &[NodeId]) -> Vec<NodeId> {
    let Some(&first) = selection.first() else { return Vec::new() };
    let Some(page) = doc.page_of(first) else { return Vec::new() };
    let candidates = scope_nodes(doc, &Scope::Page(page));
    let mut out = Vec::new();
    for c in candidates {
        if selection.contains(&c) {
            continue;
        }
        if selection.iter().any(|&s| matches_layer_across_containers(doc, s, c)) {
            out.push(c);
        }
    }
    out
}

// -------------------------------------------------------- structural selection
fn selectable_children(doc: &Document, parent: NodeId) -> Vec<NodeId> {
    doc.get(parent).map(|p| p.children.iter().copied().filter(|c| selectable(doc, *c)).collect()).unwrap_or_default()
}

/// Ctrl+A: every selectable layer next to the selection (inside the parent of
/// the first selected layer), or the page's top-level layers when nothing is selected.
pub fn select_all(doc: &Document, page: NodeId, selection: &[NodeId]) -> Vec<NodeId> {
    let parent = selection.first().and_then(|s| doc.get(*s)).and_then(|n| n.parent).unwrap_or(page);
    selectable_children(doc, parent)
}

/// Ctrl+Shift+A: the selectable siblings of the selection that are not selected.
pub fn select_inverse(doc: &Document, page: NodeId, selection: &[NodeId]) -> Vec<NodeId> {
    let parent = selection.first().and_then(|s| doc.get(*s)).and_then(|n| n.parent).unwrap_or(page);
    selectable_children(doc, parent).into_iter().filter(|c| !selection.contains(c)).collect()
}

/// Enter: the children of the selected layers (empty when none has children).
pub fn select_children(doc: &Document, selection: &[NodeId]) -> Vec<NodeId> {
    let mut seen = HashSet::new();
    selection.iter().flat_map(|s| selectable_children(doc, *s)).filter(|c| seen.insert(*c)).collect()
}

/// Shift+Enter / `\`: the parents of the selected layers, never a page.
pub fn select_parent(doc: &Document, selection: &[NodeId]) -> Vec<NodeId> {
    let mut seen = HashSet::new();
    selection
        .iter()
        .filter_map(|s| doc.get(*s).and_then(|n| n.parent))
        .filter(|p| doc.get(*p).is_some_and(|n| !matches!(n.kind, NodeKind::Page { .. })))
        .filter(|p| seen.insert(*p))
        .collect()
}

/// `Tab` (`forward = true`) / `Shift+Tab` (`forward = false`): selects the next or
/// previous visible, unlocked sibling of `selection[0]` in Layers panel order, wrapping around.
pub fn select_sibling(doc: &Document, page: NodeId, selection: &[NodeId], forward: bool) -> Vec<NodeId> {
    let Some(&first) = selection.first() else {
        let siblings = selectable_children(doc, page);
        let pick = if forward { siblings.last().copied() } else { siblings.first().copied() };
        return pick.into_iter().collect();
    };
    let parent = doc.get(first).and_then(|n| n.parent).unwrap_or(page);
    let siblings = selectable_children(doc, parent);
    if siblings.is_empty() {
        return Vec::new();
    }
    let len = siblings.len();
    match siblings.iter().position(|&id| id == first) {
        Some(idx) => {
            let next_idx = if forward { (idx + len - 1) % len } else { (idx + 1) % len };
            vec![siblings[next_idx]]
        }
        None => {
            let pick = if forward { siblings.last().copied() } else { siblings.first().copied() };
            pick.into_iter().collect()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Fx {
        doc: Document,
        page: NodeId,
    }

    fn style() -> TextStyle {
        TextStyle::default()
    }

    impl Fx {
        fn new() -> Self {
            let mut doc = Document::new("t");
            let page = doc.add_page("Page 1");
            Fx { doc, page }
        }
        fn add(&mut self, parent: NodeId, mut n: Node, name: &str) -> NodeId {
            n.name = name.into();
            self.doc.insert(parent, n).unwrap()
        }
        fn id(&mut self) -> NodeId {
            self.doc.alloc_id()
        }
        fn text(&mut self, parent: NodeId, content: &str) -> NodeId {
            let id = self.id();
            let n = Node::text(id, Rect::new(0.0, 0.0, 50.0, 20.0), content, style());
            self.add(parent, n, content)
        }
        fn rect(&mut self, parent: NodeId, name: &str, fill: Color) -> NodeId {
            let id = self.id();
            let mut n = Node::rectangle(id, Rect::new(0.0, 0.0, 10.0, 10.0));
            n.fills = vec![Paint::solid(fill)];
            self.add(parent, n, name)
        }
        fn frame(&mut self, parent: NodeId, name: &str) -> NodeId {
            let id = self.id();
            self.add(parent, Node::frame(id, Rect::new(0.0, 0.0, 100.0, 100.0)), name)
        }
        fn content(&self, id: NodeId) -> String {
            match &self.doc.get(id).unwrap().kind {
                NodeKind::Text(t) => t.content.clone(),
                _ => panic!("not text"),
            }
        }
    }

    fn o(match_case: bool, whole_word: bool, regex: bool) -> FindOptions {
        FindOptions { match_case, whole_word, regex }
    }

    fn texts(doc: &Document, ms: &[Match]) -> Vec<String> {
        ms.iter()
            .map(|m| {
                let n = doc.get(m.node).unwrap();
                let s = match (&n.kind, m.field) {
                    (NodeKind::Text(t), MatchField::Text) => &t.content,
                    _ => &n.name,
                };
                s[m.range.clone()].to_string()
            })
            .collect()
    }

    #[test]
    fn plain_search_is_case_insensitive_by_default() {
        let mut f = Fx::new();
        let t = f.text(f.page, "Hello hello HELLO");
        let ms = find_text(&f.doc, &Scope::Page(f.page), "hello", o(false, false, false));
        assert_eq!(ms.len(), 3);
        assert!(ms.iter().all(|m| m.node == t && m.field == MatchField::Text));
        assert_eq!(texts(&f.doc, &ms), ["Hello", "hello", "HELLO"]);
        let cased = find_text(&f.doc, &Scope::Page(f.page), "hello", o(true, false, false));
        assert_eq!(cased.len(), 1);
        assert_eq!(cased[0].range, 6..11);
    }

    #[test]
    fn empty_and_invalid_queries_find_nothing() {
        let mut f = Fx::new();
        f.text(f.page, "abc");
        assert!(find_text(&f.doc, &Scope::Document, "", FindOptions::default()).is_empty());
        assert!(find_text(&f.doc, &Scope::Document, "(", o(false, false, true)).is_empty());
        assert!(Matcher::new("(", o(false, false, true)).is_err());
        // The same text is a perfectly good literal.
        assert_eq!(find_text(&f.doc, &Scope::Document, "(", FindOptions::default()).len(), 0);
        f.text(f.page, "f(x)");
        assert_eq!(find_text(&f.doc, &Scope::Document, "(", FindOptions::default()).len(), 1);
    }

    #[test]
    fn whole_word_rejects_glued_matches_but_keeps_later_ones() {
        let mut f = Fx::new();
        f.text(f.page, "cat concat cat_ cat, (cat) scat cat");
        let ms = find_text(&f.doc, &Scope::Page(f.page), "cat", o(false, true, false));
        let starts: Vec<usize> = ms.iter().map(|m| m.range.start).collect();
        assert_eq!(starts, [0, 16, 22, 32]);
        // Whole word also holds for the match found after a rejected overlap.
        let mut f = Fx::new();
        f.text(f.page, "aaa aa");
        let ms = find_text(&f.doc, &Scope::Page(f.page), "aa", o(false, true, false));
        assert_eq!(ms.len(), 1);
        assert_eq!(ms[0].range, 4..6);
    }

    #[test]
    fn whole_word_treats_unicode_letters_as_word_chars() {
        let mut f = Fx::new();
        f.text(f.page, "über Übung ü");
        let ms = find_text(&f.doc, &Scope::Page(f.page), "ü", o(false, true, false));
        assert_eq!(ms.len(), 1);
        assert_eq!(ms[0].range, 13..15);
    }

    #[test]
    fn regex_search_with_anchors_and_classes() {
        let mut f = Fx::new();
        f.text(f.page, "id-42 and id-7\nid-100");
        let ms = find_text(&f.doc, &Scope::Page(f.page), r"id-\d+", o(false, false, true));
        assert_eq!(texts(&f.doc, &ms), ["id-42", "id-7", "id-100"]);
        // ^ and $ work per line.
        let ms = find_text(&f.doc, &Scope::Page(f.page), r"^id-\d+", o(false, false, true));
        assert_eq!(texts(&f.doc, &ms), ["id-42", "id-100"]);
        // Empty matches are ignored instead of looping.
        let ms = find_text(&f.doc, &Scope::Page(f.page), r"x*", o(false, false, true));
        assert!(ms.is_empty());
    }

    #[test]
    fn multibyte_and_emoji_ranges_sit_on_char_boundaries() {
        let mut f = Fx::new();
        let t = f.text(f.page, "Héllo 🎉 wörld 🎉🎉 日本語");
        let ms = find_text(&f.doc, &Scope::Page(f.page), "🎉", FindOptions::default());
        assert_eq!(ms.len(), 3);
        let content = f.content(t);
        for m in &ms {
            assert!(content.is_char_boundary(m.range.start) && content.is_char_boundary(m.range.end));
            assert_eq!(&content[m.range.clone()], "🎉");
        }
        assert_eq!(ms[0].range, 7..11);
        let ms = find_text(&f.doc, &Scope::Page(f.page), "日本", FindOptions::default());
        assert_eq!(&content[ms[0].range.clone()], "日本");
        let ms = find_text(&f.doc, &Scope::Page(f.page), "WÖRLD", o(false, false, false));
        assert_eq!(&content[ms[0].range.clone()], "wörld");
        // A regex dot consumes a whole emoji, never half of one.
        let ms = find_text(&f.doc, &Scope::Page(f.page), "o .", o(false, false, true));
        assert_eq!(&content[ms[0].range.clone()], "o 🎉");
    }

    #[test]
    fn layer_names_are_searched_but_default_text_names_are_not_doubled() {
        let mut f = Fx::new();
        let fr = f.frame(f.page, "Login form");
        let t = f.text(fr, "Login"); // name == content
        let custom = f.id();
        let n = Node::text(custom, Rect::new(0.0, 0.0, 50.0, 20.0), "Sign in", style());
        let custom = f.add(fr, n, "login title");
        let ms = find_text(&f.doc, &Scope::Page(f.page), "login", FindOptions::default());
        let found: Vec<(NodeId, MatchField)> = ms.iter().map(|m| (m.node, m.field)).collect();
        // Top-most layer first; the frame name, then the custom-named text's name, then "Login".
        assert_eq!(found, [(fr, MatchField::Name), (custom, MatchField::Name), (t, MatchField::Text)]);
    }

    #[test]
    fn scopes_page_nodes_and_document() {
        let mut f = Fx::new();
        let p2 = f.doc.add_page("Page 2");
        let a = f.frame(f.page, "A");
        let b = f.frame(f.page, "B");
        f.text(a, "needle in A");
        f.text(b, "needle in B");
        f.text(p2, "needle on page 2");
        let q = |scope: Scope| find_text(&f.doc, &scope, "needle", FindOptions::default()).len();
        assert_eq!(q(Scope::Page(f.page)), 2);
        assert_eq!(q(Scope::Page(p2)), 1);
        assert_eq!(q(Scope::Document), 3);
        assert_eq!(q(Scope::Nodes(vec![a])), 1);
        // Overlapping roots do not duplicate hits.
        assert_eq!(q(Scope::Nodes(vec![a, f.page])), 2);
    }

    #[test]
    fn replace_is_one_batch_that_undoes_in_one_step() {
        let mut f = Fx::new();
        let a = f.text(f.page, "foo bar foo");
        let b = f.text(f.page, "a foo");
        let fr = f.frame(f.page, "foo frame");
        let ms = find_text(&f.doc, &Scope::Page(f.page), "foo", FindOptions::default());
        assert_eq!(ms.len(), 4);
        let mut hist = History::default();
        let cmd = replace(&f.doc, &ms, "quux!");
        hist.exec(&mut f.doc, cmd).unwrap();
        assert_eq!(f.content(a), "quux! bar quux!");
        assert_eq!(f.content(b), "a quux!");
        // A text layer named after its content keeps following it.
        assert_eq!(f.doc.get(a).unwrap().name, "quux! bar quux!");
        assert_eq!(f.doc.get(fr).unwrap().name, "quux! frame");
        assert!(hist.undo(&mut f.doc).unwrap());
        assert_eq!(f.content(a), "foo bar foo");
        assert_eq!(f.doc.get(fr).unwrap().name, "foo frame");
        assert!(!hist.can_undo(), "one step covered every node");
        hist.redo(&mut f.doc).unwrap();
        assert_eq!(f.content(b), "a quux!");
    }

    #[test]
    fn replace_with_multibyte_text_keeps_boundaries() {
        let mut f = Fx::new();
        let t = f.text(f.page, "a🎉b🎉c");
        let ms = find_text(&f.doc, &Scope::Page(f.page), "🎉", FindOptions::default());
        replace(&f.doc, &ms, "é").apply(&mut f.doc).unwrap();
        assert_eq!(f.content(t), "aébéc");
        let ms = find_text(&f.doc, &Scope::Page(f.page), "é", FindOptions::default());
        replace(&f.doc, &ms, "").apply(&mut f.doc).unwrap();
        assert_eq!(f.content(t), "abc");
    }

    fn bold() -> TextStyle {
        TextStyle { font_weight: 700, ..style() }
    }

    fn set_runs(f: &mut Fx, id: NodeId, runs: Vec<(usize, usize, TextStyle)>) {
        if let NodeKind::Text(t) = &mut f.doc.get_mut(id).unwrap().kind {
            t.runs = runs.into_iter().map(|(start, end, style)| TextRun { start, end, style }).collect();
        }
    }

    fn runs(f: &Fx, id: NodeId) -> Vec<(usize, usize, u16)> {
        match &f.doc.get(id).unwrap().kind {
            NodeKind::Text(t) => t.runs.iter().map(|r| (r.start, r.end, r.style.font_weight)).collect(),
            _ => panic!(),
        }
    }

    #[test]
    fn replace_preserves_run_styles() {
        // "Hello world": "world" is bold.
        let mut f = Fx::new();
        let t = f.text(f.page, "Hello world");
        set_runs(&mut f, t, vec![(6, 11, bold())]);
        // Shorter text before the run shifts it.
        let ms = find_text(&f.doc, &Scope::Page(f.page), "Hello", FindOptions::default());
        replace(&f.doc, &ms, "Hi").apply(&mut f.doc).unwrap();
        assert_eq!(f.content(t), "Hi world");
        assert_eq!(runs(&f, t), [(3, 8, 700)]);
        // Replacing inside the bold run stays bold.
        let ms = find_text(&f.doc, &Scope::Page(f.page), "world", FindOptions::default());
        replace(&f.doc, &ms, "everyone").apply(&mut f.doc).unwrap();
        assert_eq!(f.content(t), "Hi everyone");
        assert_eq!(runs(&f, t), [(3, 11, 700)]);
    }

    #[test]
    fn replaced_range_takes_the_style_of_its_first_char() {
        // "plain bold": a match spanning plain -> bold becomes plain, the tail stays bold.
        let mut f = Fx::new();
        let t = f.text(f.page, "plain bold!");
        set_runs(&mut f, t, vec![(6, 10, bold())]);
        let ms = find_text(&f.doc, &Scope::Page(f.page), "n bo", FindOptions::default());
        assert_eq!(ms[0].range, 4..8);
        replace(&f.doc, &ms, "XYZ").apply(&mut f.doc).unwrap();
        assert_eq!(f.content(t), "plaiXYZld!");
        assert_eq!(runs(&f, t), [(7, 9, 700)]);
        // A match starting in the bold run swallows the plain tail into the run.
        let mut f = Fx::new();
        let t = f.text(f.page, "plain bold!");
        set_runs(&mut f, t, vec![(6, 10, bold())]);
        let ms = find_text(&f.doc, &Scope::Page(f.page), "old!", FindOptions::default());
        replace(&f.doc, &ms, "ER").apply(&mut f.doc).unwrap();
        assert_eq!(f.content(t), "plain bER");
        assert_eq!(runs(&f, t), [(6, 9, 700)]);
        // A run wholly inside the match disappears.
        let mut f = Fx::new();
        let t = f.text(f.page, "a bold b");
        set_runs(&mut f, t, vec![(2, 6, bold())]);
        let ms = find_text(&f.doc, &Scope::Page(f.page), "a bold b", FindOptions::default());
        replace(&f.doc, &ms, "x").apply(&mut f.doc).unwrap();
        assert_eq!(f.content(t), "x");
        assert!(runs(&f, t).is_empty());
    }

    #[test]
    fn multiple_replacements_in_runs_keep_all_runs_aligned() {
        let mut f = Fx::new();
        let t = f.text(f.page, "ab ab ab");
        set_runs(&mut f, t, vec![(3, 5, bold())]); // the middle "ab"
        let ms = find_text(&f.doc, &Scope::Page(f.page), "ab", FindOptions::default());
        replace(&f.doc, &ms, "xyz").apply(&mut f.doc).unwrap();
        assert_eq!(f.content(t), "xyz xyz xyz");
        assert_eq!(runs(&f, t), [(4, 7, 700)]);
    }

    #[test]
    fn regex_replacement_expands_groups() {
        let mut f = Fx::new();
        let t = f.text(f.page, "john smith, jane doe");
        let ms = find_text(&f.doc, &Scope::Page(f.page), r"(\w+) (\w+)", o(false, false, true));
        replace(&f.doc, &ms, "$2 $1 ($$${0})").apply(&mut f.doc).unwrap();
        assert_eq!(f.content(t), "smith john ($john smith), doe jane ($jane doe)");
        // A literal replacement keeps its dollars when the search is plain.
        let mut f = Fx::new();
        let t = f.text(f.page, "price");
        let ms = find_text(&f.doc, &Scope::Page(f.page), "price", FindOptions::default());
        replace(&f.doc, &ms, "$1").apply(&mut f.doc).unwrap();
        assert_eq!(f.content(t), "$1");
    }

    #[test]
    fn stale_matches_are_skipped() {
        let mut f = Fx::new();
        let t = f.text(f.page, "hello world");
        let ms = find_text(&f.doc, &Scope::Page(f.page), "world", FindOptions::default());
        if let NodeKind::Text(tp) = &mut f.doc.get_mut(t).unwrap().kind {
            tp.content = "hi".into();
        }
        replace(&f.doc, &ms, "x").apply(&mut f.doc).unwrap();
        assert_eq!(f.content(t), "hi");
    }

    // ---- select matching
    fn sample() -> (Fx, [NodeId; 6]) {
        let mut f = Fx::new();
        let red = Color::rgb(255, 0, 0);
        let blue = Color::rgb(0, 0, 255);
        let a = f.rect(f.page, "Box", red);
        let b = f.rect(f.page, "Box", blue);
        let fr = f.frame(f.page, "Frame");
        let c = f.rect(fr, "Inner", red);
        let t = f.text(fr, "Title");
        let id = f.id();
        let e = f.add(f.page, Node::ellipse(id, Rect::new(0.0, 0.0, 10.0, 10.0)), "Dot");
        f.doc.get_mut(e).unwrap().fills = vec![Paint::solid(red)];
        (f, [a, b, c, t, e, fr])
    }

    #[test]
    fn same_fill_across_types_and_frames() {
        let (f, [a, _b, c, _t, e, _]) = sample();
        let mut got = select_matching(&f.doc, a, MatchCriteria::Fill);
        got.sort();
        let mut want = vec![a, c, e];
        want.sort();
        assert_eq!(got, want);
    }

    #[test]
    fn same_fill_of_text_uses_the_text_color() {
        let (mut f, [_, _, _, t, ..]) = sample();
        let t2 = f.text(f.page, "Other");
        let red = vec![Paint::solid(Color::rgb(255, 0, 0))];
        for id in [t, t2] {
            if let NodeKind::Text(tp) = &mut f.doc.get_mut(id).unwrap().kind {
                tp.style.fills = red.clone();
            }
        }
        let got = select_matching(&f.doc, t, MatchCriteria::Fill);
        assert!(got.contains(&t) && got.contains(&t2));
    }

    #[test]
    fn same_stroke_and_effect_need_a_stroke_or_effect() {
        let (mut f, [a, b, c, ..]) = sample();
        // None have strokes: a lone reference selects only itself.
        assert_eq!(select_matching(&f.doc, a, MatchCriteria::Stroke), vec![a]);
        for id in [a, c] {
            let n = f.doc.get_mut(id).unwrap();
            n.stroke.paints = vec![Paint::solid(Color::BLACK)];
            n.stroke.weight = 2.0;
            n.effects = vec![Effect::LayerBlur { radius: 4.0, visible: true }];
        }
        f.doc.get_mut(b).unwrap().stroke.paints = vec![Paint::solid(Color::BLACK)]; // weight differs
        let mut s = select_matching(&f.doc, a, MatchCriteria::Stroke);
        s.sort();
        assert_eq!(s, vec![a, c]);
        let mut e = select_matching(&f.doc, c, MatchCriteria::Effect);
        e.sort();
        assert_eq!(e, vec![a, c]);
        assert_eq!(select_matching(&f.doc, b, MatchCriteria::Effect), vec![b]);
    }

    #[test]
    fn same_font_compares_family_weight_and_size() {
        let (mut f, [_, _, _, t, ..]) = sample();
        let t2 = f.text(f.page, "Same");
        let t3 = f.text(f.page, "Bigger");
        let t4 = f.text(f.page, "Bolder");
        let set = |f: &mut Fx, id: NodeId, w: u16, s: f32| {
            if let NodeKind::Text(tp) = &mut f.doc.get_mut(id).unwrap().kind {
                tp.style.font_weight = w;
                tp.style.font_size = s;
            }
        };
        set(&mut f, t, 400, 16.0);
        set(&mut f, t2, 400, 16.0);
        set(&mut f, t3, 400, 24.0);
        set(&mut f, t4, 700, 16.0);
        let mut got = select_matching(&f.doc, t, MatchCriteria::Font);
        got.sort();
        let mut want = vec![t, t2];
        want.sort();
        assert_eq!(got, want);
        // Not a text layer: nothing to match.
        let (f, [a, ..]) = sample();
        assert_eq!(select_matching(&f.doc, a, MatchCriteria::Font), vec![a]);
    }

    #[test]
    fn same_instance_follows_the_main_component() {
        let mut f = Fx::new();
        let page = f.page;
        let mk = |f: &mut Fx, parent: NodeId, comp: Option<NodeId>, name: &str| {
            let id = f.id();
            let kind = NodeKind::Instance { frame: FrameProps::default(), component: comp, component_key: None };
            f.add(parent, Node::new(id, kind), name)
        };
        let id = f.id();
        let c1 = f.add(f.page, Node::new(id, NodeKind::Component(FrameProps::default())), "Button");
        let id = f.id();
        let c2 = f.add(f.page, Node::new(id, NodeKind::Component(FrameProps::default())), "Chip");
        let fr = f.frame(f.page, "Frame");
        let i1 = mk(&mut f, page, Some(c1), "i1");
        let i2 = mk(&mut f, fr, Some(c1), "i2");
        let i3 = mk(&mut f, page, Some(c2), "i3");
        let mut got = select_matching(&f.doc, i1, MatchCriteria::Instance);
        got.sort();
        assert_eq!(got, vec![i1, i2]);
        assert_eq!(select_matching(&f.doc, i3, MatchCriteria::Instance), vec![i3]);
        // From the main component: it and its instances.
        let mut got = select_matching(&f.doc, c1, MatchCriteria::Instance);
        got.sort();
        assert_eq!(got, vec![c1, i1, i2]);
    }

    #[test]
    fn same_type_and_same_name() {
        let (f, [a, b, c, t, e, fr]) = sample();
        let mut rects = select_matching(&f.doc, a, MatchCriteria::Type);
        rects.sort();
        assert_eq!(rects, vec![a, b, c]);
        assert_eq!(select_matching(&f.doc, t, MatchCriteria::Type), vec![t]);
        let mut named = select_matching(&f.doc, a, MatchCriteria::Name);
        named.sort();
        assert_eq!(named, vec![a, b]);
        assert_eq!(select_matching(&f.doc, e, MatchCriteria::Name), vec![e]);
        assert_eq!(select_matching(&f.doc, fr, MatchCriteria::Type), vec![fr]);
    }

    #[test]
    fn all_properties_needs_type_and_visuals() {
        let (mut f, [a, b, c, _, e, _]) = sample();
        // a and c are red rectangles; b is blue; e is a red ellipse.
        let mut got = select_matching(&f.doc, a, MatchCriteria::Properties);
        got.sort();
        assert_eq!(got, vec![a, c]);
        // A different corner radius or opacity breaks the match.
        f.doc.get_mut(c).unwrap().corner_radii = [4.0; 4];
        assert_eq!(select_matching(&f.doc, a, MatchCriteria::Properties), vec![a]);
        f.doc.get_mut(c).unwrap().corner_radii = [0.0; 4];
        f.doc.get_mut(c).unwrap().opacity = 0.5;
        assert_eq!(select_matching(&f.doc, a, MatchCriteria::Properties), vec![a]);
        assert!(!select_matching(&f.doc, e, MatchCriteria::Properties).contains(&a));
        let _ = b;
    }

    #[test]
    fn hidden_and_locked_layers_are_not_selected_and_scope_limits_the_search() {
        let (mut f, [a, _, c, _, e, fr]) = sample();
        f.doc.get_mut(e).unwrap().visible = false;
        f.doc.get_mut(c).unwrap().locked = true;
        assert_eq!(select_matching(&f.doc, a, MatchCriteria::Fill), vec![a]);
        f.doc.get_mut(c).unwrap().locked = false;
        // Only inside the frame.
        let inside = select_matching_in(&f.doc, c, MatchCriteria::Fill, &Scope::Nodes(vec![fr]));
        assert_eq!(inside, vec![c]);
        // Another page is out of reach.
        let p2 = f.doc.add_page("P2");
        let other = f.rect(p2, "Box", Color::rgb(255, 0, 0));
        assert!(!select_matching(&f.doc, a, MatchCriteria::Fill).contains(&other));
    }

    #[test]
    fn select_matching_all_unions_without_duplicates() {
        let (f, [a, b, c, _, e, _]) = sample();
        let got = select_matching_all(&f.doc, &[a, c, b], MatchCriteria::Fill);
        let mut sorted = got.clone();
        sorted.sort();
        sorted.dedup();
        assert_eq!(sorted.len(), got.len());
        for id in [a, b, c, e] {
            assert!(got.contains(&id));
        }
    }

    #[test]
    fn structural_selection_commands() {
        let (f, [a, b, c, t, e, fr]) = sample();
        // Page level: everything but the selection.
        let mut inv = select_inverse(&f.doc, f.page, &[a]);
        inv.sort();
        let mut want = vec![b, fr, e];
        want.sort();
        assert_eq!(inv, want);
        // Inside the frame: the sibling only.
        assert_eq!(select_inverse(&f.doc, f.page, &[c]), vec![t]);
        // Nothing selected: the inverse of nothing is everything on the page.
        assert_eq!(select_inverse(&f.doc, f.page, &[]).len(), 4);
        // Select all stays inside the parent of the selection.
        assert_eq!(select_all(&f.doc, f.page, &[c]), vec![c, t]);
        assert_eq!(select_all(&f.doc, f.page, &[]).len(), 4);
        assert_eq!(select_children(&f.doc, &[fr]), vec![c, t]);
        assert!(select_children(&f.doc, &[a]).is_empty());
        assert_eq!(select_parent(&f.doc, &[c, t]), vec![fr]);
        assert!(select_parent(&f.doc, &[a]).is_empty(), "a page is not a selectable parent");
    }

    #[test]
    fn regex_error_message_is_short() {
        let e = Matcher::new("(", o(false, false, true)).unwrap_err();
        assert!(!e.is_empty() && !e.contains('\n'), "{e}");
    }

    #[test]
    fn select_sibling_wraps_in_layer_order() {
        let (mut f, [a, b, _c, _t, e, fr]) = sample();
        // Top-level children of page in z-order: [a, b, fr, e].
        // Layers panel order (top to bottom) is .rev(): [e, fr, b, a].
        assert_eq!(select_sibling(&f.doc, f.page, &[], true), vec![e]);
        assert_eq!(select_sibling(&f.doc, f.page, &[e], true), vec![fr]);
        assert_eq!(select_sibling(&f.doc, f.page, &[fr], true), vec![b]);
        assert_eq!(select_sibling(&f.doc, f.page, &[b], true), vec![a]);
        assert_eq!(select_sibling(&f.doc, f.page, &[a], true), vec![e]);

        // Shift+Tab (forward = false) steps upward in Layers panel order: [a -> b -> fr -> e -> a].
        assert_eq!(select_sibling(&f.doc, f.page, &[a], false), vec![b]);
        assert_eq!(select_sibling(&f.doc, f.page, &[b], false), vec![fr]);
        assert_eq!(select_sibling(&f.doc, f.page, &[fr], false), vec![e]);
        assert_eq!(select_sibling(&f.doc, f.page, &[e], false), vec![a]);

        // Locked or hidden siblings are skipped.
        f.doc.get_mut(fr).unwrap().locked = true;
        assert_eq!(select_sibling(&f.doc, f.page, &[e], true), vec![b]);
    }

    #[test]
    fn same_text_properties_and_variant() {
        let mut f = Fx::new();
        let t1 = f.text(f.page, "Heading 1");
        let t2 = f.text(f.page, "Heading 2");
        let t3 = f.text(f.page, "Subheading");
        if let NodeKind::Text(tp) = &mut f.doc.get_mut(t3).unwrap().kind {
            tp.style.letter_spacing = 2.0;
        }
        let mut got = select_matching(&f.doc, t1, MatchCriteria::TextProperties);
        got.sort();
        let mut want = vec![t1, t2];
        want.sort();
        assert_eq!(got, want);

        let set_id = f.id();
        let set = f.add(f.page, Node::new(set_id, NodeKind::ComponentSet(FrameProps::default())), "ButtonSet");
        let v1_id = f.id();
        let v1 = f.add(set, Node::new(v1_id, NodeKind::Component(FrameProps::default())), "Primary");
        let v2_id = f.id();
        let v2 = f.add(set, Node::new(v2_id, NodeKind::Component(FrameProps::default())), "Secondary");
        let mk_inst = |f: &mut Fx, comp: NodeId, name: &str| {
            let id = f.id();
            let kind = NodeKind::Instance { frame: FrameProps::default(), component: Some(comp), component_key: None };
            f.add(f.page, Node::new(id, kind), name)
        };
        let i_v1_a = mk_inst(&mut f, v1, "Button");
        let i_v1_b = mk_inst(&mut f, v1, "Button");
        let i_v2 = mk_inst(&mut f, v2, "Button");

        let mut same_var = select_matching(&f.doc, i_v1_a, MatchCriteria::Variant);
        same_var.sort();
        let mut want_var = vec![i_v1_a, i_v1_b];
        want_var.sort();
        assert_eq!(same_var, want_var);

        let mut same_inst = select_matching(&f.doc, i_v1_a, MatchCriteria::Instance);
        same_inst.sort();
        let mut want_inst = vec![i_v1_a, i_v1_b, i_v2];
        want_inst.sort();
        assert_eq!(same_inst, want_inst);
    }

    #[test]
    fn find_matching_layers_matches_hierarchy_and_unnamed_text_by_style_and_position() {
        let mut f = Fx::new();
        let fa = f.frame(f.page, "Frame A");
        let fb = f.frame(f.page, "Frame B");
        let ca = f.frame(fa, "Card");
        let cb = f.frame(fb, "Card");
        let r_a = f.rect(ca, "Badge", Color::rgb(255, 0, 0));
        let r_b = f.rect(cb, "Badge", Color::rgb(0, 0, 255));
        assert_eq!(find_matching_layers(&f.doc, &[r_a]), vec![r_b]);

        let t_a = f.text(ca, "Alice");
        let t_b = f.text(cb, "Bob");
        assert_eq!(find_matching_layers(&f.doc, &[t_a]), vec![t_b]);
    }
}
