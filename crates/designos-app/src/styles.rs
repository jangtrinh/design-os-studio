//! Shared styles (grouping, applying) and selection colors.

use crate::app::App;
use crate::edit::Edit;
use crate::layout;
use designos_doc::*;

/// A style as listed in the UI: display name split at the last "/" into a
/// folder and a leaf, like Figma's style folders.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StyleEntry {
    pub key: String,
    pub folder: String,
    pub leaf: String,
}

pub fn split_name(name: &str) -> (String, String) {
    match name.rsplit_once('/') {
        Some((f, l)) => (f.trim().to_string(), l.trim().to_string()),
        None => (String::new(), name.trim().to_string()),
    }
}

/// Which group of shared styles a list shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StyleKind {
    Text,
    Color,
    Effect,
    Grid,
}

impl StyleKind {
    fn holds(self, s: &Style) -> bool {
        matches!((self, s), (Self::Text, Style::Text(_)) | (Self::Color, Style::Fill(_)) | (Self::Effect, Style::Effect(_)) | (Self::Grid, Style::Grid(_)))
    }

    /// Key of the `style_refs` entry a node links to a style of this kind with.
    pub fn ref_key(self) -> &'static str {
        match self {
            Self::Text => "text",
            Self::Color => "fill",
            Self::Effect => "effect",
            Self::Grid => "grid",
        }
    }
}

/// Text styles (`text == true`) or paint styles, ordered by folder then name.
pub fn style_entries(doc: &Document, text: bool) -> Vec<StyleEntry> {
    style_entries_of(doc, if text { StyleKind::Text } else { StyleKind::Color })
}

/// The styles of one kind, root-level ones first and then folders alphabetically.
pub fn style_entries_of(doc: &Document, kind: StyleKind) -> Vec<StyleEntry> {
    let mut out: Vec<StyleEntry> = doc
        .styles
        .iter()
        .filter(|(_, s)| kind.holds(s))
        .map(|(k, _)| {
            let (folder, leaf) = split_name(doc.style_name(k));
            StyleEntry { key: k.clone(), folder, leaf }
        })
        .collect();
    out.sort_by(|a, b| {
        (!a.folder.is_empty(), a.folder.to_lowercase(), a.leaf.to_lowercase()).cmp(&(!b.folder.is_empty(), b.folder.to_lowercase(), b.leaf.to_lowercase()))
    });
    out
}

impl App {
    /// Applies the shared style `key` to `sel`: records the reference in
    /// `style_refs` and copies the style's values onto the nodes.
    pub fn apply_style(&mut self, sel: &[NodeId], key: &str) {
        let mut tx = crate::edit::Tx::new();
        self.apply_style_in(&mut tx, sel, key);
        self.run_tx(tx);
    }

    /// [`App::apply_style`] inside a caller-owned transaction.
    pub fn apply_style_in(&mut self, tx: &mut crate::edit::Tx, sel: &[NodeId], key: &str) {
        let Some(style) = self.doc.styles.get(key).cloned() else { return };
        let prop = style_property(&self.doc, key);
        let f = |n: &mut Node| match &style {
            Style::Fill(paints) if prop == StyleProperty::Stroke => {
                n.stroke.paints = paints.clone();
                n.style_refs.insert("stroke".into(), key.into());
            }
            Style::Fill(paints) => {
                match &mut n.kind {
                    NodeKind::Text(t) => t.style.fills = paints.clone(),
                    _ => n.fills = paints.clone(),
                }
                n.style_refs.insert("fill".into(), key.into());
            }
            Style::Text(ts) => {
                if let NodeKind::Text(t) = &mut n.kind {
                    // A text style carries no colour: keep the node's own.
                    t.style = TextStyle { fills: t.style.fills.clone(), ..ts.clone() };
                    for r in &mut t.runs {
                        r.style = TextStyle { fills: r.style.fills.clone(), ..ts.clone() };
                    }
                    n.size = layout::fit_text(t, n.size);
                    n.style_refs.insert("text".into(), key.into());
                }
            }
            Style::Effect(fx) => {
                n.effects = fx.clone();
                n.style_refs.insert("effect".into(), key.into());
            }
            Style::Grid(g) => {
                if let Some(f) = n.kind.frame_props_mut() {
                    f.grids = g.clone();
                    n.style_refs.insert("grid".into(), key.into());
                }
            }
        };
        for id in sel {
            if let Some(n) = tx.ed.node(&mut self.doc, *id) {
                f(n);
            }
            layout::relayout_around(&mut self.doc, &mut tx.ed, *id);
        }
    }

    /// Runs the outcome of a style operation as one undo step; false (with a toast) when refused.
    fn run_style_op(&mut self, op: Result<Command, StyleOpError>) -> bool {
        let mut tx = crate::edit::Tx::new();
        let applied = op.map_err(|e| e.to_string()).and_then(|cmd| tx.cmd(&mut self.doc, cmd).map_err(|e| e.to_string()));
        match applied {
            Ok(()) => self.run_tx(tx),
            Err(e) => {
                tx.cancel(&mut self.doc);
                self.show_toast(format!("Style not changed: {e}"), true);
                false
            }
        }
    }

    /// Creates an effect or grid style from the values of `node` and links the node to it.
    /// Returns the new style's key.
    pub fn create_style_from(&mut self, node: NodeId, prop: StyleProperty) -> Option<String> {
        match create_style_from_node(&self.doc, node, prop, None) {
            Ok((key, cmd)) => self.run_style_op(Ok(cmd)).then(|| {
                self.show_toast(format!("Created \"{key}\""), false);
                key
            }),
            Err(e) => {
                self.run_style_op(Err(e));
                None
            }
        }
    }

    /// Unlinks the effect or grid style of `sel`; the nodes keep their values.
    pub fn detach_style_of(&mut self, sel: &[NodeId], prop: StyleProperty) -> bool {
        let op = detach_style(&self.doc, sel, prop);
        self.run_style_op(op)
    }

    /// Renames style `key` (the display name; folders come from "/").
    pub fn rename_style_to(&mut self, key: &str, name: &str) -> bool {
        let op = rename_style(&self.doc, key, name);
        self.run_style_op(op)
    }

    /// Replaces solid colour `old` by `new` in the whole subtree of the selection (one undo step).
    pub fn replace_selection_color(&mut self, sel: &[NodeId], old: Color, new: Color) {
        let mut tx = crate::edit::Tx::new();
        replace_color(&mut self.doc, &mut tx.ed, sel, old, new);
        self.run_tx(tx);
    }
}

#[inline]
pub fn rgb_key(c: &Color) -> u32 {
    let ch = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u32;
    (ch(c.r) << 16) | (ch(c.g) << 8) | ch(c.b)
}

#[inline]
#[allow(dead_code)]
fn same_rgb(a: &Color, b: &Color) -> bool {
    rgb_key(a) == rgb_key(b)
}

fn paint_lists(n: &Node) -> Vec<&Vec<Paint>> {
    let mut v = vec![&n.fills, &n.stroke.paints];
    if let NodeKind::Text(t) = &n.kind {
        v.push(&t.style.fills);
        v.extend(t.runs.iter().map(|r| &r.style.fills));
    }
    v
}

pub const MAX_COLOR_WALK_NODES: usize = 512;
pub const MAX_UNIQUE_SELECTION_COLORS: usize = 64;

/// Collects unique solid colours (opaque RGB) and visible usage counts across the
/// subtrees of `roots` in pre-order, stopping after `max_nodes` visited nodes
/// with zero heap String allocations.
pub fn selection_colors_with_counts(doc: &Document, roots: &[NodeId], max_nodes: usize) -> (Vec<Color>, Vec<usize>) {
    let mut keys: Vec<u32> = Vec::new();
    let mut out: Vec<Color> = Vec::new();
    let mut counts: Vec<usize> = Vec::new();
    let mut stack: Vec<NodeId> = Vec::with_capacity(64);
    let mut visited = 0usize;

    let mut record_paint = |p: &Paint| {
        if let PaintKind::Solid(c) = &p.kind {
            let k = rgb_key(c);
            if let Some(idx) = keys.iter().position(|&ek| ek == k) {
                if p.visible {
                    counts[idx] += 1;
                }
            } else if out.len() < MAX_UNIQUE_SELECTION_COLORS {
                keys.push(k);
                out.push(Color { a: 1.0, ..*c });
                counts.push(usize::from(p.visible));
            }
        }
    };

    for &r in roots.iter().rev() {
        stack.push(r);
    }
    while let Some(id) = stack.pop() {
        if visited >= max_nodes {
            break;
        }
        let Some(n) = doc.get(id) else { continue };
        visited += 1;
        for p in &n.fills {
            record_paint(p);
        }
        for p in &n.stroke.paints {
            record_paint(p);
        }
        if let NodeKind::Text(t) = &n.kind {
            for p in &t.style.fills {
                record_paint(p);
            }
            for r in &t.runs {
                for p in &r.style.fills {
                    record_paint(p);
                }
            }
        }
        for &cid in n.children.iter().rev() {
            stack.push(cid);
        }
    }
    (out, counts)
}

/// Unique solid colours (opaque RGB) used by the subtrees of `roots`, in order of first use.
pub fn selection_colors(doc: &Document, roots: &[NodeId]) -> Vec<Color> {
    selection_colors_with_counts(doc, roots, MAX_COLOR_WALK_NODES).0
}

/// Replaces solid colour `old` with `new` (alpha of each paint is kept) in the subtrees of `roots`.
pub fn replace_color(doc: &mut Document, ed: &mut Edit, roots: &[NodeId], old: Color, new: Color) {
    let old_key = rgb_key(&old);
    let mut ids = Vec::new();
    for r in roots {
        doc.walk(*r, |n, _| ids.push(n.id));
    }
    let swap = |list: &mut Vec<Paint>| {
        for p in list {
            if let PaintKind::Solid(c) = &mut p.kind {
                if rgb_key(c) == old_key {
                    *c = Color { a: c.a, ..new };
                }
            }
        }
    };
    for id in ids {
        let has = doc
            .get(id)
            .map(|n| paint_lists(n).iter().any(|l| l.iter().any(|p| matches!(&p.kind, PaintKind::Solid(c) if rgb_key(c) == old_key))))
            .unwrap_or(false);
        if !has {
            continue;
        }
        if let Some(n) = ed.node(doc, id) {
            swap(&mut n.fills);
            swap(&mut n.stroke.paints);
            if let NodeKind::Text(t) = &mut n.kind {
                swap(&mut t.style.fills);
                for r in &mut t.runs {
                    swap(&mut r.style.fills);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn app() -> (App, NodeId, NodeId, NodeId) {
        let mut d = Document::new("t");
        let page = d.add_page("p");
        let fid = d.alloc_id();
        let mut f = Node::frame(fid, Rect::new(0.0, 0.0, 200.0, 200.0));
        f.fills = vec![Paint::solid(Color::rgb(255, 0, 0))];
        let f = d.insert(page, f).unwrap();
        let rid = d.alloc_id();
        let mut r = Node::rectangle(rid, Rect::new(10.0, 10.0, 50.0, 50.0));
        r.fills = vec![Paint::solid(Color::rgb(255, 0, 0)), Paint::solid(Color::rgb(0, 0, 255))];
        r.stroke.paints = vec![Paint::solid(Color::rgb(0, 255, 0))];
        let r = d.insert(f, r).unwrap();
        let tid = d.alloc_id();
        let t = d
            .insert(
                f,
                Node::text(tid, Rect::new(0.0, 100.0, 50.0, 20.0), "Hi", TextStyle { fills: vec![Paint::solid(Color::rgb(0, 0, 255))], ..Default::default() }),
            )
            .unwrap();
        d.styles.insert("Brand/Primary".into(), Style::Fill(vec![Paint::solid(Color::rgb(1, 2, 3))]));
        d.styles.insert("s2".into(), Style::Text(TextStyle { font_size: 40.0, font_weight: 700, ..Default::default() }));
        d.style_meta.insert("s2".into(), StyleMeta { name: "Heading/Large".into(), ..Default::default() });
        (App::new(d), f, r, t)
    }

    #[test]
    fn entries_use_display_names_and_group_by_prefix() {
        let (app, ..) = app();
        let fills = style_entries(&app.doc, false);
        assert_eq!(fills, vec![StyleEntry { key: "Brand/Primary".into(), folder: "Brand".into(), leaf: "Primary".into() }]);
        let texts = style_entries(&app.doc, true);
        assert_eq!(texts[0].key, "s2");
        assert_eq!((texts[0].folder.as_str(), texts[0].leaf.as_str()), ("Heading", "Large"));
        assert_eq!(split_name("Plain"), (String::new(), "Plain".to_string()));
        assert_eq!(split_name("a/b/c"), ("a/b".to_string(), "c".to_string()));
    }

    #[test]
    fn applying_fill_style_sets_ref_and_values_and_undoes() {
        let (mut app, _f, r, t) = app();
        app.apply_style(&[r, t], "Brand/Primary");
        let rn = app.doc.get(r).unwrap();
        assert_eq!(rn.style_refs.get("fill").map(String::as_str), Some("Brand/Primary"));
        assert_eq!(rn.fills, vec![Paint::solid(Color::rgb(1, 2, 3))]);
        match &app.doc.get(t).unwrap().kind {
            NodeKind::Text(tp) => assert_eq!(tp.style.fills, vec![Paint::solid(Color::rgb(1, 2, 3))]),
            _ => unreachable!(),
        }
        app.undo();
        assert!(app.doc.get(r).unwrap().style_refs.is_empty());
        assert_eq!(app.doc.get(r).unwrap().fills.len(), 2);
    }

    #[test]
    fn applying_text_style_keeps_color_and_skips_shapes() {
        let (mut app, _f, r, t) = app();
        app.apply_style(&[r, t], "s2");
        assert!(app.doc.get(r).unwrap().style_refs.is_empty());
        let n = app.doc.get(t).unwrap();
        assert_eq!(n.style_refs.get("text").map(String::as_str), Some("s2"));
        match &n.kind {
            NodeKind::Text(tp) => {
                assert_eq!(tp.style.font_size, 40.0);
                assert_eq!(tp.style.fills, vec![Paint::solid(Color::rgb(0, 0, 255))]);
                assert!(n.size.y >= 40.0, "auto-resized to the new size");
            }
            _ => unreachable!(),
        }
    }

    #[test]
    fn selection_colors_are_unique_across_subtree() {
        let (app, f, ..) = app();
        let cols: Vec<String> = selection_colors(&app.doc, &[f]).iter().map(|c| c.to_hex()).collect();
        assert_eq!(cols, vec!["FF0000", "0000FF", "00FF00"]);
    }

    #[test]
    fn replacing_a_selection_color_edits_everywhere_in_one_step() {
        let (mut app, f, r, t) = app();
        app.replace_selection_color(&[f], Color::rgb(255, 0, 0), Color::rgb(9, 9, 9));
        assert_eq!(app.doc.get(f).unwrap().fills[0].kind, PaintKind::Solid(Color::rgb(9, 9, 9)));
        assert_eq!(app.doc.get(r).unwrap().fills[0].kind, PaintKind::Solid(Color::rgb(9, 9, 9)));
        assert_eq!(app.doc.get(r).unwrap().fills[1].kind, PaintKind::Solid(Color::rgb(0, 0, 255)));
        app.replace_selection_color(&[f], Color::rgb(0, 0, 255), Color::rgb(7, 7, 7));
        match &app.doc.get(t).unwrap().kind {
            NodeKind::Text(tp) => assert_eq!(tp.style.fills[0].kind, PaintKind::Solid(Color::rgb(7, 7, 7))),
            _ => unreachable!(),
        }
        app.undo();
        app.undo();
        assert_eq!(app.doc.get(f).unwrap().fills[0].kind, PaintKind::Solid(Color::rgb(255, 0, 0)));
        assert!(!app.hist.can_undo());
    }

    #[test]
    fn effect_and_grid_styles_have_their_own_groups() {
        let (mut ed, f, r, _t) = app();
        ed.doc.styles.insert("Shadow/Card".into(), Style::Effect(vec![Effect::LayerBlur { radius: 4.0, visible: true }]));
        ed.doc.styles.insert("g".into(), Style::Grid(vec![LayoutGrid::Grid { size: 8.0, color: Color::rgba(255, 0, 0, 26) }]));
        ed.doc.style_meta.insert("g".into(), StyleMeta { name: "Layout/8pt".into(), property: StyleProperty::Grid, ..Default::default() });
        let keys = |k| style_entries_of(&ed.doc, k).into_iter().map(|e| (e.key, e.folder, e.leaf)).collect::<Vec<_>>();
        assert_eq!(keys(StyleKind::Effect), vec![("Shadow/Card".to_string(), "Shadow".to_string(), "Card".to_string())]);
        assert_eq!(keys(StyleKind::Grid), vec![("g".to_string(), "Layout".to_string(), "8pt".to_string())]);
        assert_eq!(keys(StyleKind::Color).len(), 1, "colour styles do not list effect or grid styles");
        assert_eq!(keys(StyleKind::Text).len(), 1);
        // Applying through the shared path links the layer and, for grids, only frames take it.
        ed.apply_style(&[r, f], "Shadow/Card");
        assert_eq!(ed.doc.get(r).unwrap().style_refs.get("effect").map(String::as_str), Some("Shadow/Card"));
        ed.apply_style(&[r, f], "g");
        assert!(!ed.doc.get(r).unwrap().style_refs.contains_key("grid"), "a rectangle has no layout guides");
        assert_eq!(ed.doc.get(f).unwrap().style_refs.get("grid").map(String::as_str), Some("g"));
    }

    #[test]
    fn creating_renaming_and_detaching_an_effect_style_are_single_undo_steps() {
        let (mut ed, _f, r, _t) = app();
        ed.edit_nodes(&[r], |n| n.effects = vec![Effect::LayerBlur { radius: 6.0, visible: true }]);
        let key = ed.create_style_from(r, StyleProperty::Effect).expect("created");
        assert!(matches!(&ed.doc.styles[&key], Style::Effect(fx) if fx == &vec![Effect::LayerBlur { radius: 6.0, visible: true }]));
        assert!(ed.rename_style_to(&key, "Blur/Soft"));
        assert_eq!(ed.doc.style_name(&key), "Blur/Soft");
        assert!(ed.detach_style_of(&[r], StyleProperty::Effect));
        assert!(ed.doc.get(r).unwrap().style_refs.is_empty());
        assert_eq!(ed.doc.get(r).unwrap().effects.len(), 1, "detaching keeps the effect");
        ed.undo();
        assert_eq!(ed.doc.get(r).unwrap().style_refs.get("effect"), Some(&key));
        ed.undo();
        assert_eq!(ed.doc.style_name(&key), key, "the rename is undone on its own");
        ed.undo();
        assert!(!ed.doc.styles.contains_key(&key) && ed.doc.get(r).unwrap().style_refs.is_empty(), "creation and link undo together");
        // A layer without effects has nothing to turn into a style.
        let (mut empty, _f, r, _t) = app();
        assert!(empty.create_style_from(r, StyleProperty::Effect).is_none());
        assert!(empty.toast.is_some(), "the refusal is shown");
    }
}
