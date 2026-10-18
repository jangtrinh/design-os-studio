//! Helpers shared by the code generators (Tailwind, SwiftUI, Compose, React).

use crate::util::{css_color, num};
use designos_doc::*;

/// Indenting line writer.
pub(crate) struct Out {
    unit: &'static str,
    lines: Vec<String>,
    /// The exported root is a function that takes a `modifier` parameter (Compose).
    pub root_takes_modifier: bool,
}

impl Out {
    pub fn new(unit: &'static str) -> Self {
        Self { unit, lines: Vec::new(), root_takes_modifier: false }
    }

    pub fn line(&mut self, depth: usize, s: impl AsRef<str>) {
        self.lines.push(format!("{}{}", self.unit.repeat(depth), s.as_ref()));
    }

    pub fn finish(self) -> String {
        self.lines.join("\n")
    }

    /// An empty writer with the same indent unit, for rendering a nested block.
    pub fn sub(&self) -> Self {
        Self { unit: self.unit, lines: Vec::new(), root_takes_modifier: self.root_takes_modifier }
    }

    pub fn into_lines(self) -> Vec<String> {
        self.lines
    }
}

/// Splits an identifier-ish string into words (non-alphanumerics and lower->upper boundaries).
fn words(s: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut cur = String::new();
    let mut prev_lower = false;
    for c in s.chars() {
        if c.is_alphanumeric() {
            if c.is_uppercase() && prev_lower && !cur.is_empty() {
                out.push(std::mem::take(&mut cur));
            }
            prev_lower = c.is_lowercase() || c.is_ascii_digit();
            cur.push(c);
        } else {
            prev_lower = false;
            if !cur.is_empty() {
                out.push(std::mem::take(&mut cur));
            }
        }
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

fn cap(w: &str) -> String {
    let mut c = w.chars();
    match c.next() {
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
        None => String::new(),
    }
}

pub(crate) fn pascal(s: &str) -> String {
    let p: String = words(s).iter().map(|w| cap(w)).collect();
    match p.chars().next() {
        None => "Layer".into(),
        Some(c) if c.is_ascii_digit() => format!("V{p}"),
        _ => p,
    }
}

pub(crate) fn camel(s: &str) -> String {
    let p = pascal(s);
    let mut c = p.chars();
    match c.next() {
        Some(f) => f.to_lowercase().collect::<String>() + c.as_str(),
        None => String::new(),
    }
}

pub(crate) fn kebab(s: &str) -> String {
    let w: Vec<String> = words(s).iter().map(|w| w.to_lowercase()).collect();
    if w.is_empty() {
        "layer".into()
    } else {
        w.join("-")
    }
}

pub(crate) fn snake(s: &str) -> String {
    kebab(s).replace('-', "_")
}

/// Name of the view/component a node becomes: the component for instances (the set for
/// variants), else the node's own name.
pub(crate) fn view_name(doc: &Document, n: &Node) -> String {
    if let Some(c) = instance_component(n).and_then(|c| doc.get(c)) {
        let set = c.parent.and_then(|p| doc.get(p)).filter(|p| matches!(p.kind, NodeKind::ComponentSet(_)));
        return pascal(&set.unwrap_or(c).name);
    }
    pascal(&n.name)
}

pub(crate) fn bound<'a>(doc: &'a Document, n: &Node, key: &str) -> Option<&'a Variable> {
    doc.variables.get(n.variables.bindings.get(key)?)
}

fn segments(v: &Variable) -> Vec<String> {
    let s: Vec<String> = v.name.split('/').map(kebab).filter(|s| !s.is_empty()).collect();
    if s.is_empty() {
        vec![kebab(&v.id)]
    } else {
        s
    }
}

/// Figma's custom-property name sanitizer for a variable name (`Color/Primary` -> `Color-Primary`):
/// 1. strip non-ASCII characters, quotes and newlines, then trim;
/// 2. turn runs of whitespace and `/` into `-`;
/// 3. a result made only of `[-A-Za-z0-9_]` keeps its case;
/// 4. anything else becomes lower kebab-case (camelCase split) without leading non-alphanumerics;
/// 5. an empty result is `unnamed`.
pub(crate) fn css_var_name(name: &str) -> String {
    let stripped: String = name.chars().filter(|c| c.is_ascii() && !matches!(c, '"' | '\'' | '\n' | '\r')).collect();
    let mut joined = String::new();
    let mut in_run = false;
    for c in stripped.trim().chars() {
        if c.is_whitespace() || c == '/' {
            if !in_run {
                joined.push('-');
            }
            in_run = true;
        } else {
            joined.push(c);
            in_run = false;
        }
    }
    let plain = joined.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_');
    let out = if plain {
        joined
    } else {
        let mut kebab = String::new();
        let mut prev: Option<char> = None;
        for c in joined.chars() {
            if c.is_ascii_uppercase() && prev.is_some_and(|p| p.is_ascii_lowercase() || p.is_ascii_digit()) {
                kebab.push('-');
            }
            kebab.push(c.to_ascii_lowercase());
            prev = Some(c);
        }
        kebab.trim_start_matches(|c: char| !c.is_ascii_alphanumeric()).to_string()
    };
    if out.is_empty() {
        "unnamed".into()
    } else {
        out
    }
}

/// `var(--Color-Primary)` for a variable named `Color/Primary`, with no fallback (what Tailwind
/// arbitrary values can hold). Ignores the variable's code syntax.
pub fn css_var(v: &Variable) -> String {
    format!("var(--{})", css_var_name(&v.name))
}

/// How a bound variable's resolved value is spelled as the `var()` fallback.
#[derive(Clone, Copy)]
enum Fallback {
    Color,
    /// A length in pixels.
    Px,
    /// A bare number (font weight, opacity percent).
    Raw,
}

fn fallback_text(value: &VariableValue, fmt: Fallback) -> Option<String> {
    match (value, fmt) {
        (VariableValue::Color(c), _) => Some(css_color(c, 1.0)),
        (VariableValue::Number(x), Fallback::Px) => Some(format!("{}px", num(*x))),
        (VariableValue::Number(x), Fallback::Raw) => Some(num(*x)),
        _ => None,
    }
}

/// The reference CSS prints for a variable, as Figma Dev Mode does: a WEB code syntax is printed
/// verbatim (when it contains `var(--` this keeps the user's own fallback, otherwise it is any
/// custom text), else `var(--<name>)` without a value.
pub fn css_var_display(v: &Variable) -> String {
    match v.code_syntax_for(CodeSyntaxPlatform::Web) {
        Some(code) => code.to_string(),
        None => css_var(v),
    }
}

/// [`css_var_display`] plus, when there is no code syntax, the value `v` resolves to at `n` as the
/// fallback: `var(--Color-Primary, #0D99FF)`.
fn css_var_with_fallback(doc: &Document, n: &Node, v: &Variable, fmt: Fallback) -> String {
    if let Some(code) = v.code_syntax_for(CodeSyntaxPlatform::Web) {
        return code.to_string();
    }
    let name = css_var_name(&v.name);
    match doc.resolve_variable(n.id, &v.id).and_then(|value| fallback_text(&value, fmt)) {
        Some(fallback) => format!("var(--{name}, {fallback})"),
        None => format!("var(--{name})"),
    }
}

/// What a bound variable prints as in `platform`'s code: its code syntax for that platform verbatim
/// when set, else the generator's own token reference (`fallback`).
pub(crate) fn token_text(v: &Variable, platform: CodeSyntaxPlatform, fallback: impl FnOnce() -> String) -> String {
    v.code_syntax_for(platform).map(str::to_string).unwrap_or_else(fallback)
}

/// camelCase token name, dropping a leading `color` segment (`Color/Brand/Primary` -> `brandPrimary`).
pub(crate) fn token_camel(v: &Variable, drop_color: bool) -> String {
    let mut s = segments(v);
    if drop_color && s.len() > 1 && (s[0] == "color" || s[0] == "colors") {
        s.remove(0);
    }
    camel(&s.join(" "))
}

pub(crate) fn first_paint(paints: &[Paint]) -> Option<&Paint> {
    paints.iter().rev().find(|p| p.visible && p.opacity > 0.0)
}

pub(crate) fn visible_children<'a>(doc: &'a Document, n: &Node) -> Vec<&'a Node> {
    n.children.iter().filter_map(|c| doc.get(*c)).filter(|c| c.visible).collect()
}

pub(crate) fn is_vector_kind(k: &NodeKind) -> bool {
    matches!(k, NodeKind::Vector { .. } | NodeKind::BooleanOp(_) | NodeKind::Star { .. } | NodeKind::Polygon { .. })
}

/// Whether a layer exports as an icon/vector asset: a vector shape, or a small container holding
/// only vector shapes.
pub fn is_vector_asset(doc: &Document, node: NodeId) -> bool {
    let Some(n) = doc.get(node) else { return false };
    if is_vector_kind(&n.kind) {
        return true;
    }
    if !matches!(n.kind, NodeKind::Frame(_) | NodeKind::Group | NodeKind::Component(_) | NodeKind::Instance { .. }) || n.children.is_empty() {
        return false;
    }
    if n.size.x > 128.0 || n.size.y > 128.0 {
        return false;
    }
    let mut all = true;
    let mut leaves = 0;
    for c in n.children.iter().filter_map(|c| doc.get(*c)).filter(|c| c.visible) {
        if is_vector_kind(&c.kind) || matches!(c.kind, NodeKind::Line | NodeKind::Rectangle | NodeKind::Ellipse(_)) {
            leaves += 1;
        } else if !is_vector_asset(doc, c.id) {
            all = false;
        }
    }
    all && leaves > 0
}

/// Vector-asset layers inside `root` (outermost only), in document order.
pub fn vector_assets(doc: &Document, root: NodeId) -> Vec<NodeId> {
    fn rec(doc: &Document, id: NodeId, out: &mut Vec<NodeId>) {
        let Some(n) = doc.get(id) else { return };
        if !n.visible {
            return;
        }
        if is_vector_asset(doc, id) {
            out.push(id);
            return;
        }
        for c in &n.children {
            rec(doc, *c, out);
        }
    }
    let mut out = Vec::new();
    rec(doc, root, &mut out);
    out
}

/// A literal value of an instance's component property.
pub(crate) enum PropLit {
    Bool(bool),
    Text(String),
    Instance(String),
}

/// Component properties of an instance worth passing as arguments: values that differ from the
/// default, plus variant selections. Names are camelCased without Figma's `#id` suffix.
pub(crate) fn instance_args(doc: &Document, n: &Node) -> Vec<(String, PropLit)> {
    let Some(comp) = instance_component(n) else { return Vec::new() };
    let mut out = Vec::new();
    for def in prop_defs(doc, comp) {
        let value = prop_value(doc, n.id, &def);
        let name = camel(def.name.split('#').next().unwrap_or(&def.name));
        let lit = match (&def.kind, &value) {
            (PropKind::Variant { .. }, PropValue::Variant(v)) => PropLit::Text(v.clone()),
            _ if value == def.default => continue,
            (_, PropValue::Bool(b)) => PropLit::Bool(*b),
            (_, PropValue::Text(t)) => PropLit::Text(t.clone()),
            (_, PropValue::Variant(v)) => PropLit::Text(v.clone()),
            (_, PropValue::Instance(c)) => PropLit::Instance(doc.get(*c).map(|c| pascal(&c.name)).unwrap_or_else(|| "Icon".into())),
            (_, PropValue::Slot(_)) => continue,
        };
        out.push((name, lit));
    }
    out
}

/// Sizing of one axis.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Dim {
    Fixed(f32),
    Hug,
    /// Fill the parent (flex grow on the main axis, stretch on the cross axis).
    Fill,
}

/// The auto-layout parent context of a node.
#[derive(Clone, Copy)]
pub(crate) struct Parent<'a> {
    pub al: Option<&'a AutoLayout>,
    pub root: bool,
}

impl<'a> Parent<'a> {
    pub fn root() -> Self {
        Self { al: None, root: true }
    }
    pub fn of(n: &'a Node) -> Self {
        Self { al: n.kind.frame_props().and_then(|f| f.auto_layout.as_ref()), root: false }
    }
}

pub(crate) struct Sizes {
    pub w: Dim,
    pub h: Dim,
    /// Placed by flow of an auto-layout parent.
    pub in_flow: bool,
    /// Whether the parent lays out in a row (main axis horizontal).
    pub row_parent: bool,
    /// A child that stretches across a parent hugging its counter axis (`AlignSelf::Stretch` on a
    /// non-Fill child). The parent has nothing to stretch against, so the cross axis keeps the child's
    /// own size (`w`/`h` are never `Fill` there); web output also keeps the stretch declaration.
    pub cross_stretch: bool,
}

/// Sizing as the stack-based generators (SwiftUI, UIKit, Compose, Android XML) need it. They emit a
/// grid as a column, so every parent that is not `Horizontal` counts as one.
pub(crate) fn sizes(n: &Node, parent: Parent<'_>) -> Sizes {
    let row_parent = parent.al.map(|a| a.direction == LayoutDirection::Horizontal).unwrap_or(true);
    sizes_along(n, parent, row_parent)
}

/// Sizing as a browser lays the node out (CSS and Tailwind). It differs from [`sizes`] only for grids:
/// - a grid runs along the width, as in the layout solver (`AutoLayout::primary_is_horizontal`), so
///   `grow` stretches an item across its cell and leaves its height alone;
/// - a grid frame that hugs still exports the size the solver gave it, because its `1fr` tracks
///   only split an explicit size (the tracks themselves stay fractions of that size).
pub(crate) fn web_sizes(n: &Node, parent: Parent<'_>) -> Sizes {
    let row_parent = parent.al.map(|a| a.primary_is_horizontal()).unwrap_or(true);
    let mut sz = sizes_along(n, parent, row_parent);
    if n.kind.frame_props().and_then(|f| f.auto_layout.as_ref()).is_some_and(|a| a.direction == LayoutDirection::Grid) {
        if sz.w == Dim::Hug {
            sz.w = Dim::Fixed(n.size.x);
        }
        if sz.h == Dim::Hug {
            sz.h = Dim::Fixed(n.size.y);
        }
    }
    sz
}

fn sizes_along(n: &Node, parent: Parent<'_>, row_parent: bool) -> Sizes {
    let in_flow = parent.al.is_some() && !n.layout_child.absolute && !parent.root;
    let own_al = n.kind.frame_props().and_then(|f| f.auto_layout.as_ref());
    let hug_w = own_al.is_some_and(|a| a.sizing_along(true) == Sizing::Hug);
    let hug_h = own_al.is_some_and(|a| a.sizing_along(false) == Sizing::Hug);
    let text_w = matches!(&n.kind, NodeKind::Text(t) if t.auto_resize == TextAutoResize::WidthAndHeight);
    let text_h = matches!(&n.kind, NodeKind::Text(t) if matches!(t.auto_resize, TextAutoResize::WidthAndHeight | TextAutoResize::Height));
    let grows = in_flow && n.layout_child.grow > 0.0;
    // A flow parent stretches its children along its counter axis; `AlignSelf::Stretch` and counter
    // `Sizing::Fill` are the same stretch there. A grid is different: its auto row sizes from the
    // child, so a stretch child only fills that row and solves to its own size. Codegen therefore never
    // turns `AlignSelf::Stretch` into a fill for a grid child and exports the solved pixels, as it does
    // for a hug flex parent (`keeps_h`). `row_parent` only picks `grow` and the emitted stack kind.
    let grid_parent = parent.al.is_some_and(|a| a.direction == LayoutDirection::Grid);
    let counter_horiz = parent.al.filter(|_| in_flow && !grid_parent).map(|a| !a.primary_is_horizontal());
    let lc = &n.layout_child;
    // The solver sizes a stretch child to the line, and a hugging counter axis derives that line from
    // the children, so the child's own size is what keeps the frame from collapsing.
    let cross_stretch = in_flow
        && lc.align_self == AlignSelf::Stretch
        && parent.al.is_some_and(|a| a.direction != LayoutDirection::Grid && a.counter_sizing == Sizing::Hug)
        && (if row_parent { lc.vertical } else { lc.horizontal }) != Sizing::Fill;
    let keeps_w = cross_stretch && !row_parent;
    let keeps_h = cross_stretch && row_parent;
    let fill_w = in_flow && ((lc.effective_sizing(true, counter_horiz) == Sizing::Fill && !keeps_w) || (grows && row_parent));
    let fill_h = in_flow && ((lc.effective_sizing(false, counter_horiz) == Sizing::Fill && !keeps_h) || (grows && !row_parent));
    let w = if fill_w {
        Dim::Fill
    } else if hug_w || text_w {
        Dim::Hug
    } else {
        Dim::Fixed(n.size.x)
    };
    let h = if fill_h {
        Dim::Fill
    } else if hug_h || text_h {
        Dim::Hug
    } else if matches!(n.kind, NodeKind::Line) {
        Dim::Fixed(0.0)
    } else {
        Dim::Fixed(n.size.y)
    };
    Sizes { w, h, in_flow, row_parent, cross_stretch }
}

/// Position offset for children that are not placed by auto layout.
pub(crate) fn offset(n: &Node, parent: Parent<'_>, sz: &Sizes) -> Option<(f32, f32)> {
    if parent.root || sz.in_flow {
        return None;
    }
    Some((n.transform.tx, n.transform.ty))
}

pub(crate) fn rotation_deg(n: &Node) -> f32 {
    let r = n.transform.rotation_rad().to_degrees();
    if r.abs() > 0.01 {
        r
    } else {
        0.0
    }
}

pub(crate) fn text_of(n: &Node) -> Option<&TextProps> {
    match &n.kind {
        NodeKind::Text(t) => Some(t),
        _ => None,
    }
}

/// Resolved `"` / `\` / newline escaping for Swift and Kotlin string literals.
pub(crate) fn quote(s: &str) -> String {
    let mut o = String::from("\"");
    for c in s.chars() {
        match c {
            '"' => o.push_str("\\\""),
            '\\' => o.push_str("\\\\"),
            '\n' => o.push_str("\\n"),
            '$' => o.push_str("\\$"),
            c => o.push(c),
        }
    }
    o.push('"');
    o
}

/// A Swift string literal. Escapes the backslash, `"`, newline, carriage return, tab and every other control
/// character (as `\u{..}`), so a layer name or font family can never end the literal early. Unlike
/// [`quote`] it leaves `$` alone, which Swift does not treat as special.
pub(crate) fn swift_quote(s: &str) -> String {
    let mut o = String::from("\"");
    for c in s.chars() {
        match c {
            '"' => o.push_str("\\\""),
            '\\' => o.push_str("\\\\"),
            '\n' => o.push_str("\\n"),
            '\r' => o.push_str("\\r"),
            '\t' => o.push_str("\\t"),
            c if c.is_control() => o.push_str(&format!("\\u{{{:x}}}", c as u32)),
            c => o.push(c),
        }
    }
    o.push('"');
    o
}

/// Visible effects of a given sort.
pub(crate) fn drop_shadows(n: &Node) -> Vec<(&Color, &Vec2, f32, f32)> {
    n.effects
        .iter()
        .filter_map(|e| match e {
            Effect::DropShadow { color, offset, radius, spread, visible: true, .. } => Some((color, offset, *radius, *spread)),
            _ => None,
        })
        .collect()
}

// ---------------------------------------------------------------- Grid templates and placement

/// Columns the layout solver lays out for `al`: the stored count (3 when unset) kept within
/// `1..=MAX_GRID_TRACKS`. The export writes this count, never the raw stored one.
pub(crate) fn grid_column_count(al: &AutoLayout) -> usize {
    let stored = if al.grid_columns > 0 { al.grid_columns as usize } else { 3 };
    stored.clamp(1, MAX_GRID_TRACKS)
}

/// One track. A flex track is `minmax(0, {n}fr)`: the solver hands it a weighted share of the inner
/// width and lets it shrink to a pixel, while a bare `{n}fr` is `minmax(auto, {n}fr)` and cannot
/// shrink under its content.
pub(crate) fn grid_track_token(t: GridTrack) -> String {
    match t {
        GridTrack::Flex(f) => format!("minmax(0, {}fr)", num(if f.is_finite() && f > 0.0 { f } else { 1.0 })),
        GridTrack::Fixed(px) => format!("{}px", num(if px.is_finite() && px >= 0.0 { px } else { 0.0 })),
        GridTrack::Hug => "auto".into(),
    }
}

/// Whether the first `count` tracks are all the default `1fr` (a track the document does not store
/// is the default, so only the stored ones are looked at).
pub(crate) fn grid_tracks_are_default(tracks: &[GridTrack], count: usize) -> bool {
    tracks.iter().take(count).all(|t| *t == GridTrack::default())
}

/// The value of `grid-template-columns` / `-rows` for `count` tracks. All `1fr` tracks are one
/// `repeat()`. Otherwise every other track is listed and a run of two or more default tracks (the
/// ones past the stored list included) folds into a `repeat()`, so the text depends on the tracks
/// alone and the work on the stored tracks, never on `count`.
pub(crate) fn grid_tracks_css(tracks: &[GridTrack], count: usize) -> String {
    let default_track = grid_track_token(GridTrack::default());
    if grid_tracks_are_default(tracks, count) {
        return format!("repeat({count}, {default_track})");
    }
    fn flush(parts: &mut Vec<String>, run: &mut usize, default_track: &str) {
        match std::mem::take(run) {
            0 => {}
            1 => parts.push(default_track.to_string()),
            n => parts.push(format!("repeat({n}, {default_track})")),
        }
    }
    let stored = count.min(tracks.len());
    let (mut parts, mut run) = (Vec::new(), 0usize);
    for t in &tracks[..stored] {
        if *t == GridTrack::default() {
            run += 1;
        } else {
            flush(&mut parts, &mut run, &default_track);
            parts.push(grid_track_token(*t));
        }
    }
    run += count - stored;
    flush(&mut parts, &mut run, &default_track);
    parts.join(" ")
}

/// A track list as one Tailwind arbitrary value: no spaces, `_` between tracks.
pub(crate) fn tailwind_tracks_value(tracks_css: &str) -> String {
    tracks_css.replace(", ", ",").replace(' ', "_")
}

/// A child's grid placement after running the same normalisation as `Want::of` in the grid solver:
/// a column pin at or past the column count is dropped, `col_span` is clamped to `cols`, a pinned
/// column whose span would overflow `cols` is pulled back to `cols - col_span`, and `row_span` is
/// clamped to `MAX_GRID_TRACKS`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct GridPlacement {
    pub column: Option<u16>,
    pub col_span: u16,
    pub row: Option<u16>,
    pub row_span: u16,
}

pub(crate) fn normalise_grid_placement(lc: &LayoutChild, al: &AutoLayout) -> GridPlacement {
    let cols = grid_column_count(al);
    let col_span = (lc.col_span.max(1) as usize).min(cols) as u16;
    let row_span = (lc.row_span.max(1) as usize).min(MAX_GRID_TRACKS) as u16;
    let column = lc.grid_column.map(usize::from).filter(|c| *c < cols).map(|c| c.min(cols - usize::from(col_span)) as u16);
    GridPlacement { column, col_span, row: lc.grid_row, row_span }
}

pub(crate) fn grid_placement_css(anchor: Option<u16>, span: u16) -> Option<String> {
    let span = span.max(1);
    match anchor {
        Some(idx) => Some(format!("{} / span {span}", u32::from(idx) + 1)),
        None if span > 1 => Some(format!("span {span}")),
        None => None,
    }
}

// ---------------------------------------------------------------- CSS variable references

fn set_decl(lines: &mut Vec<String>, prop: &str, value: String) {
    let pre = format!("{prop}: ");
    if let Some(l) = lines.iter_mut().find(|l| l.starts_with(&pre)) {
        *l = format!("{prop}: {value};");
    } else {
        lines.push(format!("{prop}: {value};"));
    }
}

fn has_decl(lines: &[String], prop: &str) -> bool {
    let pre = format!("{prop}: ");
    lines.iter().any(|l| l.starts_with(&pre))
}

/// Replaces literal values in CSS declarations by `var(--token)` references for properties
/// bound to variables.
pub(crate) fn apply_css_vars(doc: &Document, n: &Node, lines: &mut Vec<String>) {
    if n.variables.bindings.is_empty() {
        return;
    }
    let b = |key: &str, fmt: Fallback| bound(doc, n, key).map(|v| css_var_with_fallback(doc, n, v, fmt));
    let is_text = matches!(n.kind, NodeKind::Text(_));
    let visible_fills = if is_text {
        text_of(n).map(|t| t.style.fills.iter().filter(|p| p.visible).count()).unwrap_or(0)
    } else {
        n.fills.iter().filter(|p| p.visible).count()
    };
    if let Some(v) = b("fills/0", Fallback::Color) {
        if is_text {
            if has_decl(lines, "color") {
                set_decl(lines, "color", v);
            }
        } else if visible_fills == 1 && has_decl(lines, "background") {
            set_decl(lines, "background", v);
        }
    }
    if let Some(v) = b("strokes/0", Fallback::Color) {
        for prop in ["border", "outline"] {
            let pre = format!("{prop}: ");
            if let Some(l) = lines.iter_mut().find(|l| l.starts_with(&pre)) {
                let body = l[pre.len()..].trim_end_matches(';');
                let mut parts = body.splitn(3, ' ');
                let (w, style) = (parts.next().unwrap_or(""), parts.next().unwrap_or("solid"));
                *l = format!("{prop}: {w} {style} {v};");
            }
        }
        for prop in ["border-color", "-webkit-text-stroke-color"] {
            if has_decl(lines, prop) {
                set_decl(lines, prop, v.clone());
            }
        }
    }
    for (key, prop, fmt) in [
        ("width", "width", Fallback::Px),
        ("height", "height", Fallback::Px),
        ("fontSize", "font-size", Fallback::Px),
        ("fontWeight", "font-weight", Fallback::Raw),
        ("lineHeight", "line-height", Fallback::Px),
        ("letterSpacing", "letter-spacing", Fallback::Px),
    ] {
        if let Some(v) = b(key, fmt) {
            if has_decl(lines, prop) {
                set_decl(lines, prop, v);
            }
        }
    }
    if let Some(v) = b("opacity", Fallback::Raw) {
        set_decl(lines, "opacity", format!("calc({} / 100)", v.trim()));
    }
    if has_decl(lines, "gap") {
        let item_v = b("itemSpacing", Fallback::Px);
        let counter_v = b("counterAxisSpacing", Fallback::Px);
        if item_v.is_some() || counter_v.is_some() {
            if let Some(al) = n.kind.frame_props().and_then(|f| f.auto_layout.as_ref()) {
                if al.direction == LayoutDirection::Grid {
                    let col_fb = format!("{}px", num(al.item_spacing));
                    let row_px = if al.counter_spacing > 0.0 { al.counter_spacing } else { al.item_spacing };
                    let row_fb = format!("{}px", num(row_px));
                    // The column gap is `itemSpacing`, the row gap `counterAxisSpacing`. An unbound row keeps
                    // its own pixel gap; only a row with no spacing of its own (`counterAxisSpacing` unset,
                    // so the solver reuses `itemSpacing`) follows the item variable.
                    let row_follows_item = al.counter_spacing <= 0.0;
                    let col_s = item_v.clone().unwrap_or(col_fb);
                    let row_s = counter_v.or(item_v.filter(|_| row_follows_item)).unwrap_or(row_fb);
                    set_decl(lines, "gap", if row_s == col_s { row_s } else { format!("{row_s} {col_s}") });
                } else if let Some(v) = item_v {
                    set_decl(lines, "gap", v);
                }
            } else if let Some(v) = item_v {
                set_decl(lines, "gap", v);
            }
        }
    }
    // padding: expand to a 4-value shorthand when any side is bound
    let sides = ["paddingTop", "paddingRight", "paddingBottom", "paddingLeft"];
    if sides.iter().any(|k| bound(doc, n, k).is_some()) {
        if let Some(al) = n.kind.frame_props().and_then(|f| f.auto_layout.as_ref()) {
            let vals: Vec<String> = sides.iter().enumerate().map(|(i, k)| b(k, Fallback::Px).unwrap_or_else(|| format!("{}px", num(al.padding[i])))).collect();
            lines.retain(|l| !l.starts_with("padding: "));
            let value = css_box(&vals);
            // keep declaration order: padding follows the size declarations
            let at = lines
                .iter()
                .position(|l| {
                    l.starts_with("flex-direction: ")
                        || l.starts_with("justify-content: ")
                        || l.starts_with("align-items: ")
                        || l.starts_with("grid-template-columns: ")
                })
                .unwrap_or(lines.len());
            lines.insert(at, format!("padding: {value};"));
        }
    }
    let corners = ["topLeftRadius", "topRightRadius", "bottomRightRadius", "bottomLeftRadius"];
    if corners.iter().any(|k| bound(doc, n, k).is_some()) {
        let vals: Vec<String> =
            corners.iter().enumerate().map(|(i, k)| b(k, Fallback::Px).unwrap_or_else(|| format!("{}px", num(n.corner_radii[i])))).collect();
        set_decl(lines, "border-radius", css_box(&vals));
    }
}

fn css_box(v: &[String]) -> String {
    if v.iter().all(|x| *x == v[0]) {
        v[0].clone()
    } else if v[0] == v[2] && v[1] == v[3] {
        format!("{} {}", v[0], v[1])
    } else if v[1] == v[3] {
        format!("{} {} {}", v[0], v[1], v[2])
    } else {
        v.join(" ")
    }
}

/// Fixture used by the generators' golden tests: an auto-layout card with text, an instance and
/// bound variables.
#[cfg(test)]
pub(crate) mod fixture {
    use super::*;

    /// An empty grid frame on a fresh page. Insert children with [`add_rect`], then call
    /// `Document::relayout_subtree` so the frame and its children hold the solver's output.
    pub fn grid_frame(al: AutoLayout, size: Vec2) -> (Document, NodeId) {
        let mut d = Document::new("t");
        let page = d.add_page("p");
        let id = d.alloc_id();
        let mut g = Node::frame(id, Rect::new(0.0, 0.0, size.x, size.y));
        g.fills.clear();
        if let NodeKind::Frame(fp) = &mut g.kind {
            fp.auto_layout = Some(AutoLayout { direction: LayoutDirection::Grid, ..al });
        }
        let id = d.insert(page, g).unwrap();
        (d, id)
    }

    /// A fixed 400x300 two-column grid holding one 30x20 `AlignSelf::Stretch` rectangle, already solved.
    /// `row_tracks` is the stored row track (`None` keeps the default auto row, which the child sizes).
    /// `imported_fill` also stores `Sizing::Fill` on the height, as an imported `.fig` STRETCH child does.
    /// Returns `(document, grid, child)`.
    pub fn grid_stretch_child(row_track: Option<GridTrack>, imported_fill: bool) -> (Document, NodeId, NodeId) {
        let mut al = AutoLayout { grid_columns: 2, primary_sizing: Sizing::Fixed, counter_sizing: Sizing::Fixed, ..Default::default() };
        if let Some(track) = row_track {
            al.grid_rows = 1;
            al.grid_row_tracks.0[0] = track;
        }
        let (mut d, grid) = grid_frame(al, Vec2::new(400.0, 300.0));
        let rect = add_rect(&mut d, grid, Vec2::new(30.0, 20.0));
        let lc = &mut d.get_mut(rect).unwrap().layout_child;
        lc.align_self = AlignSelf::Stretch;
        if imported_fill {
            lc.vertical = Sizing::Fill;
        }
        d.relayout_subtree(grid);
        (d, grid, rect)
    }

    pub fn add_rect(d: &mut Document, parent: NodeId, size: Vec2) -> NodeId {
        let id = d.alloc_id();
        d.insert(parent, Node::rectangle(id, Rect::new(0.0, 0.0, size.x, size.y))).unwrap()
    }

    /// A horizontal 200x100 auto-layout row whose counter axis is `counter_sizing`, holding one
    /// `AlignSelf::Stretch` rectangle per entry of `heights` (each 30 wide, fixed height), already solved.
    pub fn row_with_stretch_children(counter_sizing: Sizing, heights: &[f32]) -> (Document, NodeId, Vec<NodeId>) {
        let mut d = Document::new("t");
        let page = d.add_page("p");
        let fid = d.alloc_id();
        let mut row = Node::frame(fid, Rect::new(0.0, 0.0, 200.0, 100.0));
        row.fills.clear();
        if let NodeKind::Frame(fp) = &mut row.kind {
            fp.auto_layout = Some(AutoLayout {
                direction: LayoutDirection::Horizontal,
                counter_align: LayoutAlign::Start,
                primary_sizing: Sizing::Fixed,
                counter_sizing,
                ..Default::default()
            });
        }
        let fid = d.insert(page, row).unwrap();
        let kids = heights
            .iter()
            .map(|h| {
                let id = add_rect(&mut d, fid, Vec2::new(30.0, *h));
                d.get_mut(id).unwrap().layout_child.align_self = AlignSelf::Stretch;
                id
            })
            .collect();
        d.relayout_subtree(fid);
        (d, fid, kids)
    }

    pub fn card() -> (Document, NodeId) {
        let mut d = Document::new("t");
        let page = d.add_page("p");
        d.variable_collections.insert(
            "c1".into(),
            VariableCollection {
                id: "c1".into(),
                name: "Tokens".into(),
                modes: vec![VariableMode { id: "light".into(), name: "Light".into() }, VariableMode { id: "dark".into(), name: "Dark".into() }],
                default_mode: "light".into(),
                variables: vec!["primary".into(), "muted".into(), "md".into()],
                key: None,
                source_library_key: None,
            },
        );
        let var = |id: &str, name: &str, kind, values: Vec<(&str, VariableValue)>| Variable {
            id: id.into(),
            name: name.into(),
            collection: "c1".into(),
            kind,
            values: values.into_iter().map(|(k, v)| (k.to_string(), v)).collect(),
            description: String::new(),
            scopes: vec![],
            hidden_from_publishing: false,
            key: None,

            code_syntax: Default::default(),
        };
        d.variables.insert(
            "primary".into(),
            var(
                "primary",
                "Color/Primary",
                VariableKind::Color,
                vec![("light", VariableValue::Color(Color::rgb(0x0D, 0x99, 0xFF))), ("dark", VariableValue::Color(Color::rgb(0x4D, 0xB8, 0xFF)))],
            ),
        );
        d.variables.insert(
            "muted".into(),
            var(
                "muted",
                "Color/Text/Muted",
                VariableKind::Color,
                vec![("light", VariableValue::Color(Color::rgb(0x66, 0x66, 0x66))), ("dark", VariableValue::Color(Color::rgb(0xAA, 0xAA, 0xAA)))],
            ),
        );
        d.variables.insert(
            "md".into(),
            var("md", "Spacing/Md", VariableKind::Number, vec![("light", VariableValue::Number(12.0)), ("dark", VariableValue::Number(12.0))]),
        );

        let comp_id = d.alloc_id();
        let mut comp = Node::new(comp_id, NodeKind::Component(FrameProps::default()));
        comp.name = "Button".into();
        comp.size = Vec2::new(80.0, 32.0);
        comp.component.props = vec![
            ComponentPropDef { name: "Label".into(), kind: PropKind::Text, default: PropValue::Text("Button".into()) },
            ComponentPropDef { name: "Disabled".into(), kind: PropKind::Boolean, default: PropValue::Bool(false) },
        ];
        let lib_page = d.add_page("Components");
        let comp_id = d.insert(lib_page, comp).unwrap();

        let card_id = d.alloc_id();
        let mut card = Node::frame(card_id, Rect::new(0.0, 0.0, 312.0, 200.0));
        card.name = "Card".into();
        card.corner_radii = [8.0; 4];
        card.stroke.paints = vec![Paint::solid(Color::rgb(0xE5, 0xE5, 0xE5))];
        card.stroke.weight = 1.0;
        card.effects = vec![Effect::DropShadow {
            color: Color::rgba(0, 0, 0, 64),
            offset: Vec2::new(0.0, 4.0),
            radius: 8.0,
            spread: 0.0,
            visible: true,
            show_behind: false,
        }];
        if let NodeKind::Frame(f) = &mut card.kind {
            f.clips_content = true;
            f.auto_layout = Some(AutoLayout {
                direction: LayoutDirection::Vertical,
                item_spacing: 8.0,
                padding: [16.0; 4],
                counter_sizing: Sizing::Fixed,
                primary_sizing: Sizing::Fixed,
                ..Default::default()
            });
        }
        let card_id = d.insert(page, card).unwrap();

        let title_id = d.alloc_id();
        let mut title = Node::text(
            title_id,
            Rect::new(0.0, 0.0, 120.0, 24.0),
            "Hello world",
            TextStyle {
                font_family: "Inter".into(),
                font_weight: 600,
                font_size: 20.0,
                fills: vec![Paint::solid(Color::rgb(0x11, 0x11, 0x11))],
                ..Default::default()
            },
        );
        title.name = "Title".into();
        d.insert(card_id, title).unwrap();

        let body_id = d.alloc_id();
        let mut body = Node::text(
            body_id,
            Rect::new(0.0, 0.0, 280.0, 20.0),
            "Body copy",
            TextStyle {
                font_family: "Inter".into(),
                font_size: 14.0,
                line_height: LineHeight::Pixels(20.0),
                align_h: TextAlignH::Center,
                fills: vec![Paint::solid(Color::rgb(0x66, 0x66, 0x66))],
                ..Default::default()
            },
        );
        body.name = "Body".into();
        body.layout_child.horizontal = Sizing::Fill;
        body.variables.bindings.insert("fills/0".into(), "muted".into());
        if let NodeKind::Text(t) = &mut body.kind {
            t.auto_resize = TextAutoResize::Height;
        }
        d.insert(card_id, body).unwrap();

        let row_id = d.alloc_id();
        let mut row = Node::frame(row_id, Rect::new(0.0, 0.0, 280.0, 48.0));
        row.name = "Actions".into();
        row.fills = vec![Paint::solid(Color::rgb(0x0D, 0x99, 0xFF))];
        row.corner_radii = [4.0; 4];
        row.layout_child.horizontal = Sizing::Fill;
        row.variables.bindings.insert("fills/0".into(), "primary".into());
        row.variables.bindings.insert("itemSpacing".into(), "md".into());
        if let NodeKind::Frame(f) = &mut row.kind {
            f.auto_layout = Some(AutoLayout {
                direction: LayoutDirection::Horizontal,
                item_spacing: 12.0,
                padding: [8.0, 12.0, 8.0, 12.0],
                primary_align: LayoutAlign::Center,
                counter_align: LayoutAlign::Center,
                primary_sizing: Sizing::Fixed,
                counter_sizing: Sizing::Hug,
                ..Default::default()
            });
        }
        let row_id = d.insert(card_id, row).unwrap();

        let inst_id = d.alloc_id();
        let mut inst = Node::new(inst_id, NodeKind::Instance { frame: FrameProps::default(), component: Some(comp_id), component_key: None });
        inst.name = "Button".into();
        inst.size = Vec2::new(80.0, 32.0);
        inst.component.values.insert("Label".into(), PropValue::Text("Save".into()));
        inst.component.values.insert("Disabled".into(), PropValue::Bool(true));
        d.insert(row_id, inst).unwrap();

        let dot_id = d.alloc_id();
        let mut dot = Node::ellipse(dot_id, Rect::new(0.0, 0.0, 24.0, 24.0));
        dot.fills = vec![Paint::solid(Color::rgb(0xFF, 0x00, 0x00))];
        dot.name = "Dot".into();
        d.insert(row_id, dot).unwrap();
        (d, card_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identifier_helpers() {
        assert_eq!(pascal("Primary button/large"), "PrimaryButtonLarge");
        assert_eq!(camel("Color/Brand Primary"), "colorBrandPrimary");
        assert_eq!(kebab("primaryDark Blue"), "primary-dark-blue");
        assert_eq!(pascal("123 go"), "V123Go");
        assert_eq!(snake("Chevron Down"), "chevron_down");
    }

    #[test]
    fn css_uses_variable_references() {
        let (d, card) = fixture::card();
        let row = d.get(card).unwrap().children[2];
        let css = crate::css_for(&d, row);
        assert!(css.contains("background: var(--Color-Primary, #0D99FF);"), "{css}");
        assert!(css.contains("gap: var(--Spacing-Md, 12px);"), "{css}");
        let body = d.get(card).unwrap().children[1];
        assert!(crate::css_for(&d, body).contains("color: var(--Color-Text-Muted, #666);"));
        // unbound properties keep their literal values
        assert!(crate::css_for(&d, card).contains("background: #FFF;"));
    }

    fn named(name: &str, kind: VariableKind) -> Variable {
        Variable {
            id: "x".into(),
            name: name.into(),
            collection: "c1".into(),
            kind,
            values: Default::default(),
            description: String::new(),
            scopes: vec![],
            hidden_from_publishing: false,
            key: None,
            code_syntax: Default::default(),
        }
    }

    /// Figma's custom-property name sanitizer: separators become `-`, a name made only of
    /// `[-A-Za-z0-9_]` keeps its case, anything else becomes lower kebab-case.
    #[test]
    fn css_variable_names_follow_figmas_sanitizer() {
        for (name, expected) in [
            ("Color/Primary", "Color-Primary"),
            ("spacing / md", "spacing-md"),
            ("Brand  Primary", "Brand-Primary"),
            ("snake_case-ok", "snake_case-ok"),
            ("Color/BrandPrimary (dark)", "color-brand-primary-(dark)"),
            ("Radius.lg", "radius.lg"),
            ("\u{201C}Q\u{201D} \"quoted\" 'x'\nline", "Q-quoted-xline"),
            ("Caf\u{e9}/Bar", "Caf-Bar"),
            ("!!bang", "bang"),
            ("@@@", "unnamed"),
            ("   ", "unnamed"),
            ("", "unnamed"),
        ] {
            assert_eq!(crate::css_variable_reference(&named(name, VariableKind::Number)), format!("var(--{expected})"), "{name:?}");
        }
    }

    /// A bound variable prints `var(--name, resolved value)`; a WEB code syntax is printed verbatim
    /// (a user fallback inside `var(--` stays as written).
    #[test]
    fn css_prints_code_syntax_verbatim_else_name_with_resolved_fallback() {
        let (mut d, card) = fixture::card();
        let row = d.get(card).unwrap().children[2];
        let body = d.get(card).unwrap().children[1];
        let css = crate::css_for(&d, row);
        assert!(css.contains("background: var(--Color-Primary, #0D99FF);"), "{css}");
        assert!(css.contains("gap: var(--Spacing-Md, 12px);"), "{css}");
        assert!(crate::css_for(&d, body).contains("color: var(--Color-Text-Muted, #666);"));

        let web = |d: &mut Document, id: &str, code: &str| {
            d.variables.get_mut(id).unwrap().code_syntax.insert(CodeSyntaxPlatform::Web, code.into());
        };
        web(&mut d, "primary", "var(--brand-primary, #123456)");
        web(&mut d, "md", "tokens.space.md");
        let css = crate::css_for(&d, row);
        assert!(css.contains("background: var(--brand-primary, #123456);"), "{css}");
        assert!(css.contains("gap: tokens.space.md;"), "{css}");
        // other platforms never leak into CSS
        d.variables.get_mut("muted").unwrap().code_syntax.insert(CodeSyntaxPlatform::Ios, "Color.muted".into());
        assert!(crate::css_for(&d, body).contains("color: var(--Color-Text-Muted, #666);"));
    }

    /// The fallback is the value in the mode the node sees.
    #[test]
    fn css_fallback_follows_the_node_mode() {
        let (mut d, card) = fixture::card();
        let row = d.get(card).unwrap().children[2];
        d.get_mut(card).unwrap().variables.modes.insert("c1".into(), "dark".into());
        assert!(crate::css_for(&d, row).contains("background: var(--Color-Primary, #4DB8FF);"));
    }

    #[test]
    fn icons_become_asset_references_in_every_language() {
        let (mut d, card) = fixture::card();
        let id = d.alloc_id();
        let mut v = Node::new(id, NodeKind::Vector { paths: vec![], network: None });
        v.name = "Chevron Down".into();
        v.size = Vec2::new(16.0, 16.0);
        d.insert(card, v).unwrap();
        assert!(crate::tailwind_for(&d, card).contains(r#"<img src="/assets/chevron-down.svg" alt="Chevron Down" width="16" height="16""#));
        assert!(crate::swiftui_for(&d, card).contains(
            "Image(\"chevron-down\")
        .resizable()"
        ));
        assert!(crate::compose_for(&d, card).contains("painter = painterResource(R.drawable.chevron_down)"));
        assert!(crate::react_for(&d, card).contains("<img"));
        assert_eq!(vector_assets(&d, card), vec![id]);
    }

    #[test]
    fn variable_names() {
        let (d, _) = fixture::card();
        let v = &d.variables["primary"];
        assert_eq!(css_var(v), "var(--Color-Primary)");
        assert_eq!(token_camel(&d.variables["muted"], true), "textMuted");
        assert_eq!(css_var(&d.variables["md"]), "var(--Spacing-Md)");
    }

    #[test]
    fn grid_items_are_sized_as_a_column_by_the_stack_generators_and_along_the_width_by_the_web_ones() {
        let al = AutoLayout { direction: LayoutDirection::Grid, primary_sizing: Sizing::Hug, counter_sizing: Sizing::Hug, ..Default::default() };
        let mut item = Node::rectangle(NodeId(2), Rect::new(0.0, 0.0, 80.0, 40.0));
        item.layout_child.grow = 1.0;
        let parent = Parent { al: Some(&al), root: false };
        // SwiftUI, UIKit, Compose and Android XML emit a grid as a column, where `grow` fills the height.
        let stack = sizes(&item, parent);
        assert!(!stack.row_parent);
        assert_eq!((stack.w, stack.h), (Dim::Fixed(80.0), Dim::Fill));
        // The browser runs a grid along the width, like the layout solver: `grow` fills the width.
        let web = web_sizes(&item, parent);
        assert!(web.row_parent);
        assert_eq!((web.w, web.h), (Dim::Fill, Dim::Fixed(40.0)));

        // A grid frame always exports the size the solver gave it, even when it hugs.
        let mut grid = Node::frame(NodeId(3), Rect::new(0.0, 0.0, 120.0, 30.0));
        if let NodeKind::Frame(fp) = &mut grid.kind {
            fp.auto_layout = Some(al.clone());
        }
        assert_eq!((sizes(&grid, Parent::root()).w, sizes(&grid, Parent::root()).h), (Dim::Hug, Dim::Hug));
        assert_eq!((web_sizes(&grid, Parent::root()).w, web_sizes(&grid, Parent::root()).h), (Dim::Fixed(120.0), Dim::Fixed(30.0)));
    }

    #[test]
    fn grid_stretch_child_solved_sizes() {
        // The auto row sizes from the child, so stretching fills only that row.
        let (d, _, rect) = fixture::grid_stretch_child(None, false);
        assert_eq!(d.get(rect).unwrap().size, Vec2::new(30.0, 20.0));
        // A row taller than the child is where a stretch changes the height.
        let (d, _, rect) = fixture::grid_stretch_child(Some(GridTrack::Fixed(100.0)), false);
        assert_eq!(d.get(rect).unwrap().size, Vec2::new(30.0, 100.0));
    }
}
