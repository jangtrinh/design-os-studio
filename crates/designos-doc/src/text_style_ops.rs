//! Text styles as pure document functions: create one from text layers and detach it again.
//!
//! `designos_doc::styles_ops` does this for effect and grid styles only. Like those, each function
//! returns a [`Command`] and leaves the document alone, so the caller decides how the change joins
//! the undo stack.

use crate::lint::token_match::{same_text_style, same_text_style_props};
use crate::*;
use std::collections::BTreeMap;

/// Key of the `style_refs` entry that links a text layer to its text style.
const TEXT_REF: &str = "text";

/// Whether every run of `t` has the base style's values, colour aside: the layer then has one text
/// style to share. A layer whose runs differ has none, and a style made from its base would not
/// match what is drawn.
pub fn has_single_text_style(t: &TextProps) -> bool {
    t.runs.iter().all(|r| same_text_style(&r.style, &t.style))
}

/// The text layers among `nodes`, each once and in the order given. Other nodes are skipped.
fn text_layers<'a>(doc: &'a Document, nodes: &[NodeId]) -> Result<Vec<(&'a Node, &'a TextProps)>, StyleOpError> {
    let mut out: Vec<(&Node, &TextProps)> = Vec::new();
    for id in nodes {
        let n = doc.get(*id).ok_or(DocError::NotFound(*id))?;
        if let (NodeKind::Text(t), false) = (&n.kind, out.iter().any(|(seen, _)| seen.id == *id)) {
            out.push((n, t));
        }
    }
    Ok(out)
}

/// The base style the text layers among `nodes` share: each has one style (see
/// [`has_single_text_style`]) and all are alike, colour aside. `None` when there is no text layer or
/// they differ, so no single style describes them.
pub fn shared_text_style(doc: &Document, nodes: &[NodeId]) -> Option<TextStyle> {
    shared_text_style_data(doc, nodes).map(|(style, _)| style)
}

fn typography_bindings(doc: &Document, node: &Node) -> BTreeMap<String, String> {
    TEXT_STYLE_BINDING_KEYS.iter().filter_map(|key| doc.effective_binding(node.id, key).map(|value| ((*key).to_string(), value.to_string()))).collect()
}

fn shared_text_style_data(doc: &Document, nodes: &[NodeId]) -> Option<(TextStyle, BTreeMap<String, String>)> {
    let layers = text_layers(doc, nodes).ok()?;
    let (first, _) = *layers.first()?;
    let first_resolved = resolve_node(doc, first).into_owned();
    let NodeKind::Text(first_text) = first_resolved.kind else { return None };
    let mut style = first_text.style.clone();
    style.align_h = TextAlignH::Left;
    style.align_v = TextAlignV::Top;
    style.paragraph_indent = first_text.paragraph.indent;
    style.list_spacing = first_text.paragraph.list_spacing;
    style.leading_trim = first_text.paragraph.leading_trim;
    let first_bindings = typography_bindings(doc, first);
    for (node, _) in &layers {
        let resolved = resolve_node(doc, node).into_owned();
        let NodeKind::Text(text) = resolved.kind else { return None };
        if !has_single_text_style(&text) || !same_text_style_props(&text, &style) {
            return None;
        }
        if typography_bindings(doc, node) != first_bindings {
            return None;
        }
    }
    Some((style, first_bindings))
}

/// First `Text style N` that is neither the key nor the display name of a style of `doc`, so a style
/// renamed to a name in the series does not get a twin.
fn next_free_name(doc: &Document) -> String {
    (1..)
        .map(|n| format!("Text style {n}"))
        .find(|name| !doc.styles.contains_key(name) && !doc.styles.keys().any(|k| doc.style_name(k) == name))
        .unwrap_or_else(|| "Text style".to_string())
}

/// Creates the text style `Text style N` from the text layers among `nodes` (their shared base
/// style: family, size, weight, line height, letter spacing and the rest), links every one of them
/// to it and returns the new key with the command. The layers take the style's values (what they draw, so a bound property is stored as the value it resolves to). Refused when no
/// node is a text layer, or when their type is not one style (see [`shared_text_style`]).
pub fn create_text_style_from_nodes(doc: &Document, nodes: &[NodeId]) -> Result<(String, Command), StyleOpError> {
    let layers = text_layers(doc, nodes)?;
    let Some((first, _)) = layers.first() else {
        return Err(StyleOpError::WrongNodeKind(nodes.first().copied().unwrap_or_default()));
    };
    let (style, bindings) = shared_text_style_data(doc, nodes).ok_or(StyleOpError::NothingToStyle(first.id))?;
    let name = next_free_name(doc);
    let mut styles = doc.styles.clone();
    let mut style_meta = doc.style_meta.clone();
    let created = Style::Text(style.clone());
    styles.insert(name.clone(), Style::Text(style));
    style_meta.insert(
        name.clone(),
        StyleMeta {
            name: name.clone(),
            key: None,
            description: String::new(),
            property: StyleProperty::Text,
            bindings,
            text_paragraph_fields: Some(0b111),
            ..Default::default()
        },
    );
    let mut cmds = vec![Command::SetStyles { styles, style_meta }];
    for (n, _) in layers {
        let mut linked = n.clone();
        detach_text_style_bindings(&mut linked);
        linked.style_refs.insert(TEXT_REF.into(), name.clone());
        // The style holds the drawn (resolved) values, so the layer takes them too: a layer whose
        // stored size was 18 under a binding that drew 32 would otherwise differ from its own style.
        apply_style_values_with_fields(&mut linked, &created, &name, Some(0b111));
        cmds.push(Command::Update { node: Box::new(linked) });
    }
    Ok((name, Command::Batch(cmds)))
}

/// "Reapply text style": every text layer among `nodes` that overrides its linked text style gets the
/// style's values back, loses the variable bindings the style owns and has no overrides left. The link
/// stays. Layers with no overrides are left alone, so the command is empty when nothing is overridden.
/// The caller re-fits the layers' boxes.
pub fn reapply_text_style(doc: &Document, nodes: &[NodeId]) -> Result<Command, StyleOpError> {
    let mut cmds = Vec::new();
    for id in nodes.iter().collect::<std::collections::BTreeSet<_>>() {
        let n = doc.get(*id).ok_or(DocError::NotFound(*id))?;
        let (Some(key), false) = (n.style_refs.get(TEXT_REF), text_style_overrides(doc, n).is_empty()) else { continue };
        let Some(style @ Style::Text(_)) = doc.styles.get(key) else { continue };
        let mut copy = n.clone();
        detach_text_style_bindings(&mut copy);
        // A hyperlink is never the style's own value, so clearing the overrides clears it too.
        copy.hyperlink = None;
        apply_style_values_with_fields(&mut copy, style, key, doc.style_meta.get(key).and_then(|m| m.text_paragraph_fields));
        cmds.push(Command::Update { node: Box::new(copy) });
    }
    Ok(Command::Batch(cmds))
}

/// Drops the text style link of every node of `nodes` that has one. The nodes keep their values and
/// the style itself stays in the document.
pub fn detach_text_style(doc: &Document, nodes: &[NodeId]) -> Result<Command, StyleOpError> {
    let mut cmds = Vec::new();
    for id in nodes.iter().collect::<std::collections::BTreeSet<_>>() {
        let n = doc.get(*id).ok_or(DocError::NotFound(*id))?;
        if n.style_refs.contains_key(TEXT_REF) {
            let mut unlinked = n.clone();
            unlinked.style_refs.remove(TEXT_REF);
            unlinked.text_style_overrides = OwnedFieldSet::default();
            cmds.push(Command::Update { node: Box::new(unlinked) });
        }
    }
    Ok(Command::Batch(cmds))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A page with a text layer `t` and a rectangle `r`.
    fn fixture() -> (Document, NodeId, NodeId) {
        let mut d = Document::new("t");
        let page = d.add_page("p");
        let rid = d.alloc_id();
        let r = d.insert(page, Node::rectangle(rid, Rect::new(0.0, 0.0, 10.0, 10.0))).unwrap();
        let tid = d.alloc_id();
        let t = d.insert(page, Node::text(tid, Rect::new(0.0, 20.0, 100.0, 20.0), "Hello", TextStyle { font_size: 21.0, ..Default::default() })).unwrap();
        (d, t, r)
    }

    /// A second text layer with the type of `like`.
    fn add_text_like(d: &mut Document, like: NodeId, content: &str) -> NodeId {
        let style = match &d.get(like).unwrap().kind {
            NodeKind::Text(t) => t.style.clone(),
            other => panic!("{other:?}"),
        };
        let id = d.alloc_id();
        let page = d.pages[0];
        d.insert(page, Node::text(id, Rect::new(0.0, 60.0, 100.0, 20.0), content, style)).unwrap()
    }

    fn apply(d: &mut Document, cmd: Command) {
        History::default().exec(d, cmd).expect("the command applies");
    }

    #[test]
    fn creating_copies_the_layer_style_and_links_the_layer() {
        let (mut d, t, _) = fixture();
        let (key, cmd) = create_text_style_from_nodes(&d, &[t]).unwrap();
        assert_eq!(key, "Text style 1");
        apply(&mut d, cmd);
        let NodeKind::Text(tp) = &d.get(t).unwrap().kind else { panic!("text layer") };
        assert!(matches!(&d.styles[&key], Style::Text(ts) if ts == &tp.style));
        assert_eq!(d.style_meta[&key].property, StyleProperty::Text);
        assert_eq!(d.get(t).unwrap().style_refs.get("text"), Some(&key));
    }

    #[test]
    fn names_skip_keys_and_display_names_in_use() {
        let (mut d, t, _) = fixture();
        let (_, cmd) = create_text_style_from_nodes(&d, &[t]).unwrap();
        apply(&mut d, cmd);
        // Key "Text style 1" now shows as "Text style 2", and a library style is keyed "Text style 3".
        d.style_meta.get_mut("Text style 1").unwrap().name = "Text style 2".into();
        d.styles.insert("Text style 3".into(), Style::Text(TextStyle::default()));
        let (key, _) = create_text_style_from_nodes(&d, &[t]).unwrap();
        assert_eq!(key, "Text style 4");
    }

    #[test]
    fn only_a_text_layer_with_one_style_can_be_turned_into_a_style() {
        let (d, t, r) = fixture();
        assert!(matches!(create_text_style_from_nodes(&d, &[r]), Err(StyleOpError::WrongNodeKind(id)) if id == r));
        assert!(matches!(create_text_style_from_nodes(&d, &[NodeId(9999)]), Err(StyleOpError::Doc(_))));
        assert!(matches!(create_text_style_from_nodes(&d, &[]), Err(StyleOpError::WrongNodeKind(_))));
        let mut mixed = d.clone();
        if let NodeKind::Text(tp) = &mut mixed.get_mut(t).unwrap().kind {
            tp.runs.push(TextRun { start: 0, end: 2, style: TextStyle { font_weight: 700, ..tp.style.clone() } });
        }
        assert!(matches!(create_text_style_from_nodes(&mixed, &[t]), Err(StyleOpError::NothingToStyle(_))));
        // Colour is not part of a text style.
        if let NodeKind::Text(tp) = &mut mixed.get_mut(t).unwrap().kind {
            tp.runs[0].style = TextStyle { fills: vec![Paint::solid(Color::rgb(255, 0, 0))], ..tp.style.clone() };
        }
        assert!(create_text_style_from_nodes(&mixed, &[t]).is_ok());
    }

    #[test]
    fn several_layers_with_the_same_type_get_one_style_and_all_link_to_it() {
        let (mut d, t, r) = fixture();
        let u = add_text_like(&mut d, t, "World");
        // A rectangle in the selection is skipped, and a node given twice is linked once.
        let (key, cmd) = create_text_style_from_nodes(&d, &[t, r, u, t]).unwrap();
        apply(&mut d, cmd);
        assert_eq!(d.styles.len(), 1);
        for id in [t, u] {
            assert_eq!(d.get(id).unwrap().style_refs.get("text"), Some(&key));
        }
        assert!(d.get(r).unwrap().style_refs.is_empty());
    }

    #[test]
    fn several_layers_with_different_type_have_no_single_style() {
        let (mut d, t, _) = fixture();
        let u = add_text_like(&mut d, t, "World");
        if let NodeKind::Text(tp) = &mut d.get_mut(u).unwrap().kind {
            tp.style.font_size += 1.0;
        }
        assert!(shared_text_style(&d, &[t, u]).is_none());
        assert!(matches!(create_text_style_from_nodes(&d, &[t, u]), Err(StyleOpError::NothingToStyle(_))));
        assert!(shared_text_style(&d, &[t]).is_some());
    }

    #[test]
    fn detaching_drops_the_link_and_keeps_the_style_and_the_values() {
        let (mut d, t, r) = fixture();
        let (key, cmd) = create_text_style_from_nodes(&d, &[t]).unwrap();
        apply(&mut d, cmd);
        let before = d.get(t).unwrap().kind.clone();
        let cmd = detach_text_style(&d, &[t, r, t]).unwrap();
        apply(&mut d, cmd);
        assert!(d.get(t).unwrap().style_refs.is_empty());
        assert!(d.styles.contains_key(&key));
        assert_eq!(d.get(t).unwrap().kind, before);
    }

    #[test]
    fn create_and_detach_round_trip_on_plain_document() {
        let mut doc = Document::new("plain");
        let page = doc.add_page("Page 1");
        let tid = doc.alloc_id();
        let mut text = Node::text(
            tid,
            Rect::new(0.0, 0.0, 120.0, 24.0),
            "Heading",
            TextStyle { font_size: 28.0, font_weight: 600, align_h: TextAlignH::Center, ..Default::default() },
        );
        if let NodeKind::Text(tp) = &mut text.kind {
            tp.paragraph.indent = 8.0;
        }
        let text_id = doc.insert(page, text).unwrap();

        let mut hist = History::default();
        let (key, create_cmd) = crate::create_text_style_from_nodes(&doc, &[text_id]).unwrap();
        hist.exec(&mut doc, create_cmd).unwrap();
        assert_eq!(doc.get(text_id).unwrap().style_refs.get("text"), Some(&key));
        assert!(matches!(&doc.styles[&key], Style::Text(ts) if ts.font_size == 28.0 && ts.font_weight == 600 && ts.paragraph_indent == 8.0));

        let detach_cmd = crate::detach_text_style(&doc, &[text_id]).unwrap();
        hist.exec(&mut doc, detach_cmd).unwrap();
        assert!(!doc.get(text_id).unwrap().style_refs.contains_key("text"));
        assert!(doc.styles.contains_key(&key));

        assert!(hist.undo(&mut doc).unwrap());
        assert_eq!(doc.get(text_id).unwrap().style_refs.get("text"), Some(&key));
        assert!(hist.undo(&mut doc).unwrap());
        assert!(doc.styles.is_empty());
        assert!(!doc.get(text_id).unwrap().style_refs.contains_key("text"));
    }

    /// Text layer `t` stored at 18 and bound to variable `size` (Default 16, Dark 32); its frame is in Dark.
    fn bound_fixture() -> (Document, NodeId) {
        let mut d = Document::new("bound");
        let page = d.add_page("p");
        let fid = d.alloc_id();
        let mut frame = Node::frame(fid, Rect::new(0.0, 0.0, 300.0, 300.0));
        frame.variables.modes.insert("typo".into(), "dark".into());
        let fid = d.insert(page, frame).unwrap();
        let tid = d.alloc_id();
        let mut text = Node::text(tid, Rect::new(0.0, 0.0, 120.0, 30.0), "Hello", TextStyle { font_size: 18.0, ..Default::default() });
        text.opacity = 0.5;
        text.variables.bindings.insert("fontSize".into(), "size".into());
        text.variables.bindings.insert("opacity".into(), "size".into());
        let tid = d.insert(fid, text).unwrap();
        d.variable_collections.insert(
            "typo".into(),
            VariableCollection {
                id: "typo".into(),
                name: "Typo".into(),
                modes: vec![VariableMode { id: "default".into(), name: "Default".into() }, VariableMode { id: "dark".into(), name: "Dark".into() }],
                default_mode: "default".into(),
                variables: vec!["size".into()],
                key: None,
                source_library_key: None,
                extensions: Default::default(),
            },
        );
        d.variables.insert(
            "size".into(),
            Variable {
                id: "size".into(),
                name: "Size".into(),
                collection: "typo".into(),
                kind: VariableKind::Number,
                values: [("default".into(), VariableValue::Number(16.0)), ("dark".into(), VariableValue::Number(32.0))].into_iter().collect(),
                description: String::new(),
                scopes: Vec::new(),
                hidden_from_publishing: false,
                key: None,
                code_syntax: Default::default(),
                extensions: Default::default(),
                mode_extensions: Default::default(),
            },
        );
        (d, tid)
    }

    fn stored_and_drawn_size(d: &Document, id: NodeId) -> (f32, f32) {
        let n = d.get(id).unwrap();
        (n.kind.any_text().unwrap().style.font_size, resolve_node(d, n).kind.any_text().unwrap().style.font_size)
    }

    #[test]
    fn creating_from_a_bound_layer_carries_the_binding_and_the_drawn_value() {
        let (mut d, t) = bound_fixture();
        assert_eq!(stored_and_drawn_size(&d, t), (18.0, 32.0));
        let (key, cmd) = create_text_style_from_nodes(&d, &[t]).unwrap();
        apply(&mut d, cmd);
        let Style::Text(ts) = &d.styles[&key] else { panic!("text style") };
        assert_eq!(ts.font_size, 32.0, "the style holds the drawn size");
        assert_eq!(d.style_meta[&key].bindings.get("fontSize").map(String::as_str), Some("size"));
        let n = d.get(t).unwrap();
        assert!(!n.variables.bindings.contains_key("fontSize"), "the style now carries the binding");
        assert_eq!(n.variables.bindings.get("opacity").map(String::as_str), Some("size"), "bindings the style does not own stay");
        assert_eq!(n.style_refs.get("text"), Some(&key));
        // The layer agrees with its own style: stored 32, drawn 32, so nothing differs from the style.
        assert_eq!(stored_and_drawn_size(&d, t), (32.0, 32.0));
    }

    #[test]
    fn a_run_that_differs_from_the_base_blocks_creating_a_style() {
        let (mut d, t, _) = fixture();
        let size = d.get(t).unwrap().kind.any_text().unwrap().style.font_size;
        if let NodeKind::Text(tp) = &mut d.get_mut(t).unwrap().kind {
            tp.runs.push(TextRun { start: 0, end: 2, style: TextStyle { font_size: size + 4.0, ..tp.style.clone() } });
        }
        assert!(matches!(create_text_style_from_nodes(&d, &[t]), Err(StyleOpError::NothingToStyle(_))));
        assert!(shared_text_style(&d, &[t]).is_none());
    }

    /// A text layer `t` (16 px) and a second `u` (16 px), both linked to the style "Body" (16 px).
    fn linked_pair() -> (Document, NodeId, NodeId) {
        let (mut d, t, _) = fixture();
        let u = add_text_like(&mut d, t, "World");
        for id in [t, u] {
            if let NodeKind::Text(tp) = &mut d.get_mut(id).unwrap().kind {
                tp.style.font_size = 16.0;
            }
            d.get_mut(id).unwrap().style_refs.insert("text".into(), "Body".into());
        }
        d.styles.insert("Body".into(), Style::Text(TextStyle { font_size: 16.0, ..Default::default() }));
        d.style_meta.insert("Body".into(), StyleMeta { name: "Body".into(), property: StyleProperty::Text, ..Default::default() });
        (d, t, u)
    }

    fn set_weight_override(d: &mut Document, id: NodeId, weight: u16) {
        let n = d.get_mut(id).unwrap();
        if let NodeKind::Text(tp) = &mut n.kind {
            tp.style.set_font_weight(weight);
        }
        n.text_style_overrides.insert(OwnedField::Weight);
    }

    fn size_of(d: &Document, id: NodeId) -> f32 {
        d.get(id).unwrap().kind.any_text().unwrap().style.font_size
    }

    fn weight_of(d: &Document, id: NodeId) -> u16 {
        d.get(id).unwrap().kind.any_text().unwrap().style.font_weight
    }

    #[test]
    fn editing_the_style_keeps_the_weight_a_layer_overrides_and_updates_its_size() {
        let (mut d, t, u) = linked_pair();
        set_weight_override(&mut d, u, 700);
        let cmd = update_style(&d, "Body", Style::Text(TextStyle { font_size: 18.0, font_weight: 500, ..Default::default() })).unwrap();
        apply(&mut d, cmd);
        assert_eq!((size_of(&d, t), weight_of(&d, t)), (18.0, 500), "the plain layer follows the style");
        assert_eq!((size_of(&d, u), weight_of(&d, u)), (18.0, 700), "the layer keeps its weight and takes the new size");
        assert_eq!(d.get(u).unwrap().text_style_overrides.labels(), vec!["Weight"]);
        assert_eq!(d.get(u).unwrap().style_refs.get("text").map(String::as_str), Some("Body"));
    }

    #[test]
    fn editing_the_style_keeps_a_bold_range_and_moves_the_rest_with_the_style() {
        let (mut d, t, _) = linked_pair();
        if let NodeKind::Text(tp) = &mut d.get_mut(t).unwrap().kind {
            // "Hel" bold, and "lo" in a colour-only range: only the bold range overrides the weight.
            tp.runs.push(TextRun { start: 0, end: 3, style: TextStyle { font_weight: 700, ..tp.style.clone() } });
            tp.runs.push(TextRun { start: 3, end: 5, style: TextStyle { fills: vec![Paint::solid(Color::rgb(255, 0, 0))], ..tp.style.clone() } });
        }
        let cmd = update_style(&d, "Body", Style::Text(TextStyle { font_size: 18.0, font_weight: 500, ..Default::default() })).unwrap();
        apply(&mut d, cmd);
        let tp = d.get(t).unwrap().kind.any_text().unwrap();
        assert_eq!((tp.style.font_size, tp.style.font_weight), (18.0, 500));
        assert_eq!((tp.runs[0].style.font_weight, tp.runs[0].style.font_size), (700, 18.0), "the bold range keeps its weight");
        assert_eq!((tp.runs[1].style.font_weight, tp.runs[1].style.font_size), (500, 18.0), "a colour-only range follows the style");
        assert!(d.get(t).unwrap().text_style_overrides.is_empty(), "a range override needs no stored bit");
        assert_eq!(text_style_overrides(&d, d.get(t).unwrap()).labels(), vec!["Weight"]);
    }

    #[test]
    fn a_bold_turned_off_again_follows_a_later_style_edit() {
        let (mut d, t, _) = linked_pair();
        // Bold on, then off: the stored bit stays but the layer shows the style's own weight.
        set_weight_override(&mut d, t, 700);
        set_weight_override(&mut d, t, 400);
        assert!(text_style_overrides(&d, d.get(t).unwrap()).is_empty(), "nothing is overridden any more");
        let cmd = update_style(&d, "Body", Style::Text(TextStyle { font_size: 16.0, font_weight: 500, ..Default::default() })).unwrap();
        apply(&mut d, cmd);
        assert_eq!(weight_of(&d, t), 500, "the edit reaches a layer that follows its style");
        assert!(d.get(t).unwrap().text_style_overrides.is_empty());
    }

    #[test]
    fn reapplying_restores_the_style_values_clears_bindings_hyperlink_and_overrides_and_keeps_the_link() {
        let (mut d, t, u) = linked_pair();
        set_weight_override(&mut d, t, 700);
        d.get_mut(t).unwrap().hyperlink = Some("https://example.com".into());
        d.get_mut(t).unwrap().variables.bindings.insert("fontWeight".into(), "weight".into());
        d.get_mut(t).unwrap().variables.bindings.insert("fills/0".into(), "colour".into());
        let before = d.get(t).unwrap().clone();
        let mut hist = History::default();
        let cmd = reapply_text_style(&d, &[t, u]).unwrap();
        hist.exec(&mut d, cmd).unwrap();
        let n = d.get(t).unwrap();
        assert_eq!(weight_of(&d, t), 400);
        assert!(n.text_style_overrides.is_empty() && n.hyperlink.is_none());
        assert!(!n.variables.bindings.contains_key("fontWeight"));
        assert_eq!(n.variables.bindings.get("fills/0").map(String::as_str), Some("colour"), "bindings the style does not own stay");
        assert_eq!(n.style_refs.get("text").map(String::as_str), Some("Body"));
        assert!(hist.undo(&mut d).unwrap());
        assert_eq!(d.get(t).unwrap(), &before, "one undo restores the overrides");
        assert!(!hist.undo(&mut d).unwrap(), "and it was a single step");
    }

    #[test]
    fn reapplying_clears_a_bold_range_too() {
        let (mut d, t, _) = linked_pair();
        if let NodeKind::Text(tp) = &mut d.get_mut(t).unwrap().kind {
            tp.runs.push(TextRun { start: 0, end: 3, style: TextStyle { font_weight: 700, ..tp.style.clone() } });
        }
        let cmd = reapply_text_style(&d, &[t]).unwrap();
        apply(&mut d, cmd);
        assert!(text_style_overrides(&d, d.get(t).unwrap()).is_empty());
        assert_eq!(d.get(t).unwrap().kind.any_text().unwrap().runs[0].style.font_weight, 400);
    }

    #[test]
    fn reapplying_with_nothing_overridden_is_an_empty_command() {
        let (d, t, u) = linked_pair();
        assert!(matches!(reapply_text_style(&d, &[t, u]).unwrap(), Command::Batch(v) if v.is_empty()));
    }

    #[test]
    fn detaching_clears_the_overrides_and_keeps_the_values() {
        let (mut d, t, _) = linked_pair();
        set_weight_override(&mut d, t, 700);
        let cmd = detach_text_style(&d, &[t]).unwrap();
        apply(&mut d, cmd);
        let n = d.get(t).unwrap();
        assert!(n.style_refs.is_empty() && n.text_style_overrides.is_empty());
        assert_eq!(weight_of(&d, t), 700);
    }

    #[test]
    fn applying_a_style_clears_the_overrides_it_replaces() {
        let (mut d, t, _) = linked_pair();
        set_weight_override(&mut d, t, 700);
        let mut copy = d.get(t).unwrap().clone();
        apply_style_values_with_fields(&mut copy, &d.styles["Body"].clone(), "Body", None);
        assert!(copy.text_style_overrides.is_empty());
        assert_eq!(copy.kind.any_text().unwrap().style.font_weight, 400);
    }

    #[test]
    fn binding_a_style_variable_in_a_non_default_mode_makes_no_override() {
        let (mut d, t) = bound_fixture();
        // Link the layer (stored 18, drawn under its own binding) to a style, then bind the style.
        d.get_mut(t).unwrap().variables.bindings.remove("fontSize");
        d.styles.insert("Body".into(), Style::Text(TextStyle { font_size: 18.0, ..Default::default() }));
        d.style_meta.insert("Body".into(), StyleMeta { name: "Body".into(), property: StyleProperty::Text, ..Default::default() });
        d.get_mut(t).unwrap().style_refs.insert("text".into(), "Body".into());
        let cmd = bind_style_property(&d, "Body", "fontSize", "size").unwrap();
        apply(&mut d, cmd);
        let n = d.get(t).unwrap();
        assert_eq!(stored_and_drawn_size(&d, t), (32.0, 32.0), "the layer sits in Dark and draws the dark value");
        assert!(n.text_style_overrides.is_empty() && text_style_overrides(&d, n).is_empty(), "binding the style is not an override");
    }

    #[test]
    fn the_overrides_serialize_only_when_present() {
        let (mut d, t, _) = linked_pair();
        let plain = serde_json::to_value(d.get(t).unwrap()).unwrap();
        assert!(plain.get("text_style_overrides").is_none());
        set_weight_override(&mut d, t, 700);
        let node: Node = serde_json::from_value(serde_json::to_value(d.get(t).unwrap()).unwrap()).unwrap();
        assert_eq!(node.text_style_overrides.labels(), vec!["Weight"]);
    }

    #[test]
    fn binding_a_style_weight_or_font_style_keeps_layer_and_range_overrides_and_reaches_a_turned_off_bold() {
        let (mut d, t, u) = linked_pair();
        let v = add_text_like(&mut d, t, "Third");
        d.get_mut(v).unwrap().style_refs.insert("text".into(), "Body".into());

        // `t` overrides Italic on the layer and Weight on a character range.
        if let NodeKind::Text(tp) = &mut d.get_mut(t).unwrap().kind {
            tp.style.italic = true;
            tp.runs.push(TextRun { start: 0, end: 3, style: TextStyle { font_weight: 700, italic: true, ..tp.style.clone() } });
        }
        d.get_mut(t).unwrap().text_style_overrides.insert(OwnedField::Italic);

        // `u` had bold turned on and off again, so its stored bit equals the style's 400.
        set_weight_override(&mut d, u, 700);
        set_weight_override(&mut d, u, 400);

        // `v` overrides Weight on the whole layer.
        set_weight_override(&mut d, v, 700);

        d.variable_collections.insert(
            "typo".into(),
            VariableCollection {
                id: "typo".into(),
                name: "Typo".into(),
                modes: vec![VariableMode { id: "default".into(), name: "Default".into() }],
                default_mode: "default".into(),
                variables: vec!["w".into(), "fs".into()],
                key: None,
                source_library_key: None,
                extensions: Default::default(),
            },
        );
        d.variables.insert(
            "w".into(),
            Variable {
                id: "w".into(),
                name: "Weight".into(),
                collection: "typo".into(),
                kind: VariableKind::Number,
                values: [("default".into(), VariableValue::Number(500.0))].into_iter().collect(),
                description: String::new(),
                scopes: Vec::new(),
                hidden_from_publishing: false,
                key: None,
                code_syntax: Default::default(),
                extensions: Default::default(),
                mode_extensions: Default::default(),
            },
        );
        d.variables.insert(
            "fs".into(),
            Variable {
                id: "fs".into(),
                name: "Style".into(),
                collection: "typo".into(),
                kind: VariableKind::String,
                values: [("default".into(), VariableValue::String("SemiBold".into()))].into_iter().collect(),
                description: String::new(),
                scopes: Vec::new(),
                hidden_from_publishing: false,
                key: None,
                code_syntax: Default::default(),
                extensions: Default::default(),
                mode_extensions: Default::default(),
            },
        );

        let before_t = d.get(t).unwrap().clone();
        let before_u = d.get(u).unwrap().clone();
        let before_v = d.get(v).unwrap().clone();
        let mut hist = History::default();

        let cmd = bind_style_property(&d, "Body", "fontStyle", "fs").unwrap();
        hist.exec(&mut d, cmd).unwrap();

        let tp_t = d.get(t).unwrap().kind.any_text().unwrap();
        assert_eq!(tp_t.style.font_weight, 600, "base weight follows the bound style");
        assert!(tp_t.style.italic, "layer Italic override stays when fontStyle is bound");
        assert_eq!(tp_t.runs[0].style.font_weight, 700, "character-range weight override stays");
        assert!(tp_t.runs[0].style.italic, "character-range italic follows the layer's Italic override");
        let drawn_t = resolve_node(&d, d.get(t).unwrap());
        let drawn_tp_t = drawn_t.kind.any_text().unwrap();
        assert!(drawn_tp_t.style.italic && drawn_tp_t.runs[0].style.font_weight == 700, "resolve_node keeps layer and range overrides");

        assert_eq!(weight_of(&d, u), 600, "a bold turned off again follows the newly bound style weight");
        assert!(d.get(u).unwrap().text_style_overrides.is_empty(), "stale override bit is dropped");
        assert_eq!(weight_of(&d, v), 700, "layer-wide weight override stays");

        assert!(hist.undo(&mut d).unwrap());
        assert_eq!(d.get(t).unwrap(), &before_t);
        assert_eq!(d.get(u).unwrap(), &before_u);
        assert_eq!(d.get(v).unwrap(), &before_v);
    }
}
