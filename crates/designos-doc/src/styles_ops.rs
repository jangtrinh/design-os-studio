//! Effect and grid styles as pure document functions.
//!
//! A shared style lives in [`Document::styles`] (values) and
//! [`Document::style_meta`] (name, library key, description, property). A node
//! is linked to it through `style_refs` (`"effect"` / `"grid"`) and carries its
//! own copy of the values, so a detached node keeps its look.
//!
//! Like [`crate::components`] and [`crate::library_ops`], every function
//! returns a [`Command`] and leaves the document alone, so the caller decides
//! how the change joins the undo stack.

use crate::*;
use std::collections::BTreeSet;

/// Why an effect / grid style operation was refused.
#[derive(Debug, thiserror::Error)]
pub enum StyleOpError {
    #[error(transparent)]
    Doc(#[from] DocError),
    #[error("style {0:?} does not exist")]
    UnknownStyle(String),
    #[error("{0:?} is not an effect or grid style")]
    NotEffectOrGrid(StyleProperty),
    #[error("node {0:?} cannot carry that kind of style")]
    WrongNodeKind(NodeId),
    #[error("node {0:?} has no values to turn into a style")]
    NothingToStyle(NodeId),
    #[error("a style name cannot be empty")]
    EmptyName,
    #[error("style {0:?} holds a different kind of values than the ones given")]
    KindMismatch(String),
    #[error("style {0:?} belongs to a library; edit it in the library file")]
    LibraryStyle(String),
    #[error("variable {0:?} does not exist")]
    UnknownVariable(String),
    #[error("property {0:?} cannot be bound on this style")]
    InvalidProperty(String),
}

/// Kind of variable a binding key on `style` accepts, or `None` if `property` is invalid for `style`.
pub fn style_property_kind(style: &Style, property: &str) -> Option<VariableKind> {
    let kind = property_kind(property)?;
    let valid = match style {
        Style::Fill(_) => property.starts_with("fills/"),
        Style::Text(_) => {
            matches!(property, "fontSize" | "lineHeight" | "letterSpacing" | "paragraphSpacing" | "paragraphIndent" | "fontWeight" | "fontFamily" | "fontStyle")
        }
        Style::Effect(_) => property.starts_with("effects/"),
        Style::Grid(_) => false,
    };
    valid.then_some(kind)
}

/// Key of the `style_refs` entry that links a node to a style of `prop`.
pub fn style_ref_key(prop: StyleProperty) -> Option<&'static str> {
    match prop {
        StyleProperty::Effect => Some("effect"),
        StyleProperty::Grid => Some("grid"),
        _ => None,
    }
}

fn effect_or_grid_ref(prop: StyleProperty) -> Result<&'static str, StyleOpError> {
    style_ref_key(prop).ok_or(StyleOpError::NotEffectOrGrid(prop))
}

/// First free key `"<base> <n>"` in `doc.styles`.
pub fn unique_style_key(doc: &Document, base: &str) -> String {
    (1..).map(|n| format!("{base} {n}")).find(|k| !doc.styles.contains_key(k)).unwrap_or_else(|| base.to_string())
}

/// Links `n` to style `key` and copies the style's values onto it. Returns
/// false (node untouched) when `n` cannot carry the style (a grid on a node
/// that is not a frame).
fn link(n: &mut Node, key: &str, style: &Style, prop: StyleProperty, ref_key: &str) -> bool {
    if prop == StyleProperty::Grid && n.kind.frame_props().is_none() {
        return false;
    }
    n.style_refs.insert(ref_key.into(), key.into());
    apply_style_values(n, style, key);
    true
}

/// The node's current values as a style of `prop`, or why it has none to share.
fn values_of_node(n: &Node, prop: StyleProperty) -> Result<Style, StyleOpError> {
    match prop {
        StyleProperty::Effect if n.effects.is_empty() => Err(StyleOpError::NothingToStyle(n.id)),
        StyleProperty::Effect => Ok(Style::Effect(n.effects.clone())),
        StyleProperty::Grid => match n.kind.frame_props() {
            Some(f) if !f.grids.is_empty() => Ok(Style::Grid(f.grids.clone())),
            Some(_) => Err(StyleOpError::NothingToStyle(n.id)),
            None => Err(StyleOpError::WrongNodeKind(n.id)),
        },
        other => Err(StyleOpError::NotEffectOrGrid(other)),
    }
}

fn get_node(doc: &Document, id: NodeId) -> Result<&Node, StyleOpError> {
    Ok(doc.get(id).ok_or(DocError::NotFound(id))?)
}

fn updated(n: &Node, f: impl FnOnce(&mut Node)) -> Command {
    let mut n = n.clone();
    f(&mut n);
    Command::Update { node: Box::new(n) }
}

/// Creates a style of `prop` (effect or grid) from the values of `node`, links
/// the node to it and returns the new key with the command. `name` defaults to
/// "Effect style N" / "Grid style N"; "/" in it groups styles into folders.
pub fn create_style_from_node(doc: &Document, node: NodeId, prop: StyleProperty, name: Option<&str>) -> Result<(String, Command), StyleOpError> {
    let ref_key = effect_or_grid_ref(prop)?;
    let n = get_node(doc, node)?;
    let style = values_of_node(n, prop)?;
    let name = match name.map(str::trim) {
        Some("") => return Err(StyleOpError::EmptyName),
        Some(given) => given.to_string(),
        None => unique_style_key(doc, if prop == StyleProperty::Effect { "Effect style" } else { "Grid style" }),
    };
    let key = if doc.styles.contains_key(&name) { unique_style_key(doc, &name) } else { name.clone() };
    let mut styles = doc.styles.clone();
    let mut style_meta = doc.style_meta.clone();
    styles.insert(key.clone(), style);
    style_meta
        .insert(key.clone(), StyleMeta { name, key: None, description: String::new(), property: prop, bindings: Default::default(), ..Default::default() });
    let link = updated(n, |n| {
        n.style_refs.insert(ref_key.into(), key.clone());
    });
    Ok((key.clone(), Command::Batch(vec![Command::SetStyles { styles, style_meta }, link])))
}

/// Links every node of `nodes` to style `key`, copying its values (grid
/// styles skip nodes that are not frames).
pub fn apply_style(doc: &Document, nodes: &[NodeId], key: &str) -> Result<Command, StyleOpError> {
    let style = doc.styles.get(key).ok_or_else(|| StyleOpError::UnknownStyle(key.into()))?;
    let prop = style_property(doc, key);
    let ref_key = effect_or_grid_ref(prop)?;
    let mut cmds = Vec::with_capacity(nodes.len());
    for id in nodes {
        let n = get_node(doc, *id)?;
        let mut copy = n.clone();
        if link(&mut copy, key, style, prop, ref_key) {
            cmds.push(Command::Update { node: Box::new(copy) });
        }
    }
    Ok(Command::Batch(cmds))
}

/// The `style_refs` entries through which a node can be linked to a style of this kind. The kind
/// of the values decides, so metadata naming another property (a text style filed under the
/// default `Fill`) cannot hide the linked nodes. A colour style can be linked as a fill, as a
/// stroke, or both.
fn ref_keys(style: &Style) -> &'static [&'static str] {
    match style {
        Style::Fill(_) => &["fill", "stroke"],
        Style::Text(_) => &["text"],
        Style::Effect(_) => &["effect"],
        Style::Grid(_) => &["grid"],
    }
}

/// Nodes of this document (excluding copies on the hidden library page) that are linked to style
/// `key` through a `style_refs` slot matching the style's kind.
pub fn linked_style_nodes(doc: &Document, key: &str) -> Vec<NodeId> {
    let Some(stored) = doc.styles.get(key) else {
        return Vec::new();
    };
    let refs = ref_keys(stored);
    doc.nodes.values().filter(|n| refs.iter().any(|r| n.style_refs.get(*r).is_some_and(|k| k == key)) && !is_library_node(doc, n.id)).map(|n| n.id).collect()
}

/// Replaces the values of style `key` and pushes them to every linked node, as one command (one
/// undo step). Works for fill, stroke, text, effect and grid styles; `style` must be the same
/// kind as the stored one. A style published by a library is refused: it is edited in its library
/// file and reaches this one through a library update.
pub fn update_style(doc: &Document, key: &str, style: Style) -> Result<Command, StyleOpError> {
    let stored = doc.styles.get(key).ok_or_else(|| StyleOpError::UnknownStyle(key.into()))?;
    if is_library_style(doc, key) {
        return Err(StyleOpError::LibraryStyle(key.into()));
    }
    if std::mem::discriminant(stored) != std::mem::discriminant(&style) {
        return Err(StyleOpError::KindMismatch(key.into()));
    }
    let mut styles = doc.styles.clone();
    styles.insert(key.to_string(), style.clone());
    let mut cmds = vec![Command::SetStyles { styles, style_meta: doc.style_meta.clone() }];
    for id in linked_style_nodes(doc, key) {
        if let Some(n) = doc.get(id) {
            let mut copy = n.clone();
            if apply_style_values(&mut copy, &style, key) {
                cmds.push(Command::Update { node: Box::new(copy) });
            }
        }
    }
    Ok(Command::Batch(cmds))
}

/// Binds `property` of style `key` to `variable`. Validates that the style
/// exists, the variable exists, and the variable's kind matches the property
/// (colour variable for `fills/0`, number variable for `fontSize`, etc.).
pub fn bind_style_property(doc: &Document, key: &str, property: &str, variable: &str) -> Result<Command, StyleOpError> {
    let stored = doc.styles.get(key).ok_or_else(|| StyleOpError::UnknownStyle(key.into()))?;
    if is_library_style(doc, key) {
        return Err(StyleOpError::LibraryStyle(key.into()));
    }
    let expected_kind = style_property_kind(stored, property).ok_or_else(|| StyleOpError::InvalidProperty(property.into()))?;
    let var = doc.variables.get(variable).ok_or_else(|| StyleOpError::UnknownVariable(variable.into()))?;
    if var.kind != expected_kind {
        return Err(StyleOpError::KindMismatch(format!("{key} ({property}: {variable})")));
    }
    let mut styles = doc.styles.clone();
    let mut style = stored.clone();
    if let Some(val) = doc.resolve_variable_default(variable) {
        set_bound_style_property(&mut style, property, &val);
        styles.insert(key.to_string(), style.clone());
    }
    let prop_kind = style_property(doc, key);
    let mut style_meta = doc.style_meta.clone();
    let meta = style_meta.entry(key.to_string()).or_insert_with(|| StyleMeta {
        name: key.to_string(),
        key: None,
        description: String::new(),
        property: prop_kind,
        bindings: Default::default(),
        ..Default::default()
    });
    meta.bindings.insert(property.to_string(), variable.to_string());
    let mut cmds = vec![Command::SetStyles { styles, style_meta }];
    for id in linked_style_nodes(doc, key) {
        let Some(n) = doc.get(id) else { continue };
        let mut copy = n.clone();
        if let Some(val) = doc.resolve_variable(id, variable) {
            if n.style_refs.get("stroke").is_some_and(|k| k == key) {
                if let Some(rest) = property.strip_prefix("fills/") {
                    let s_key = format!("strokes/{rest}");
                    if !copy.variables.bindings.contains_key(&s_key) {
                        set_bound_property(&mut copy, &s_key, &val);
                    }
                }
            }
            if n.style_refs.get("fill").is_some_and(|k| k == key)
                || n.style_refs.get("text").is_some_and(|k| k == key)
                || n.style_refs.get("effect").is_some_and(|k| k == key)
            {
                if !copy.variables.bindings.contains_key(property) {
                    set_bound_property(&mut copy, property, &val);
                }
            }
        }
        if copy != *n {
            cmds.push(Command::Update { node: Box::new(copy) });
        }
    }
    Ok(if cmds.len() == 1 { cmds.pop().unwrap() } else { Command::Batch(cmds) })
}

/// Removes the variable binding on `property` of style `key`.
pub fn unbind_style_property(doc: &Document, key: &str, property: &str) -> Result<Command, StyleOpError> {
    let stored = doc.styles.get(key).ok_or_else(|| StyleOpError::UnknownStyle(key.into()))?;
    if is_library_style(doc, key) {
        return Err(StyleOpError::LibraryStyle(key.into()));
    }
    if style_property_kind(stored, property).is_none() {
        return Err(StyleOpError::InvalidProperty(property.into()));
    }
    let mut style_meta = doc.style_meta.clone();
    if let Some(meta) = style_meta.get_mut(key) {
        meta.bindings.remove(property);
    }
    Ok(Command::SetStyles { styles: doc.styles.clone(), style_meta })
}

/// Drops the `prop` link of every node of `nodes` that has one. The nodes keep
/// their values and the style itself stays in the document.
pub fn detach_style(doc: &Document, nodes: &[NodeId], prop: StyleProperty) -> Result<Command, StyleOpError> {
    let ref_key = effect_or_grid_ref(prop)?;
    let mut cmds = Vec::new();
    for id in nodes.iter().collect::<BTreeSet<_>>() {
        let n = get_node(doc, *id)?;
        if n.style_refs.contains_key(ref_key) {
            cmds.push(updated(n, |n| {
                n.style_refs.remove(ref_key);
            }));
        }
    }
    Ok(Command::Batch(cmds))
}

/// Renames style `key` (display name only; the key and every link stay).
pub fn rename_style(doc: &Document, key: &str, name: &str) -> Result<Command, StyleOpError> {
    if !doc.styles.contains_key(key) {
        return Err(StyleOpError::UnknownStyle(key.into()));
    }
    if is_library_style(doc, key) {
        return Err(StyleOpError::LibraryStyle(key.into()));
    }
    let property = style_property(doc, key);
    let name = name.trim();
    if name.is_empty() {
        return Err(StyleOpError::EmptyName);
    }
    let mut style_meta = doc.style_meta.clone();
    style_meta.entry(key.to_string()).and_modify(|m| m.name = name.to_string()).or_insert_with(|| StyleMeta {
        name: name.to_string(),
        key: None,
        description: String::new(),
        property,
        bindings: Default::default(),
        ..Default::default()
    });
    Ok(Command::SetStyles { styles: doc.styles.clone(), style_meta })
}

/// Sets the description of style `key` in one [`Command::SetStyles`] undo step, trimming
/// trailing whitespace. Setting the same text is a no-op (`Command::Batch(Vec::new())`).
pub fn set_style_description(doc: &Document, key: &str, text: &str) -> Result<Command, StyleOpError> {
    if !doc.styles.contains_key(key) {
        return Err(StyleOpError::UnknownStyle(key.into()));
    }
    if is_library_style(doc, key) {
        return Err(StyleOpError::LibraryStyle(key.into()));
    }
    let trimmed = text.trim_end();
    let current = doc.style_meta.get(key).map(|m| m.description.as_str()).unwrap_or("");
    if trimmed == current {
        return Ok(Command::Batch(Vec::new()));
    }
    let property = style_property(doc, key);
    let fallback_name = doc.style_name(key).to_string();
    let mut style_meta = doc.style_meta.clone();
    style_meta.entry(key.to_string()).and_modify(|m| m.description = trimmed.to_string()).or_insert_with(|| StyleMeta {
        name: fallback_name,
        key: None,
        description: trimmed.to_string(),
        property,
        bindings: Default::default(),
        ..Default::default()
    });
    Ok(Command::SetStyles { styles: doc.styles.clone(), style_meta })
}

fn clean_folder_path(folder: &str) -> String {
    folder.split('/').map(str::trim).filter(|s| !s.is_empty()).collect::<Vec<_>>().join("/")
}

/// Renames folder `old_folder` to `new_folder`, updating the prefix of every member style in
/// one [`Command::SetStyles`] undo step.
pub fn rename_style_folder(doc: &Document, old_folder: &str, new_folder: &str) -> Result<Command, StyleOpError> {
    let old_clean = clean_folder_path(old_folder);
    let new_clean = clean_folder_path(new_folder);
    if old_clean.is_empty() || new_clean.is_empty() {
        return Err(StyleOpError::EmptyName);
    }
    let old_segs: Vec<&str> = old_clean.split('/').collect();
    let mut matched = Vec::new();
    for key in doc.styles.keys() {
        let raw_name = doc.style_name(key);
        let mut offset = 0usize;
        let mut segs: Vec<(&str, usize)> = Vec::new();
        for (i, part) in raw_name.split('/').enumerate() {
            if i > 0 {
                offset += 1;
            }
            let trimmed = part.trim();
            if !trimmed.is_empty() {
                segs.push((trimmed, offset + part.trim_end().len()));
            }
            offset += part.len();
        }
        if segs.len() > old_segs.len() && segs[..old_segs.len()].iter().map(|(s, _)| *s).eq(old_segs.iter().copied()) {
            if is_library_style(doc, key) {
                return Err(StyleOpError::LibraryStyle(key.clone()));
            }
            let prefix_end = segs[old_segs.len() - 1].1;
            matched.push((key.clone(), format!("{new_clean}{}", &raw_name[prefix_end..])));
        }
    }
    if matched.is_empty() {
        return Err(StyleOpError::UnknownStyle(old_clean));
    }
    let mut style_meta = doc.style_meta.clone();
    for (key, new_name) in matched {
        let property = style_property(doc, &key);
        style_meta.entry(key).and_modify(|m| m.name = new_name.clone()).or_insert_with(|| StyleMeta {
            name: new_name,
            key: None,
            description: String::new(),
            property,
            bindings: Default::default(),
            ..Default::default()
        });
    }
    Ok(Command::SetStyles { styles: doc.styles.clone(), style_meta })
}

/// Returns the keys of styles in `doc` (optionally filtered to `prop`), ordered by
/// [`StyleMeta::sort_position`] when present and falling back to stored map order when absent.
pub fn ordered_style_keys(doc: &Document, prop: Option<StyleProperty>) -> Vec<String> {
    let mut items: Vec<(usize, &String, Option<&str>)> = doc
        .styles
        .keys()
        .enumerate()
        .filter(|(_, k)| prop.is_none_or(|p| style_property(doc, k) == p))
        .map(|(idx, k)| {
            let pos = doc.style_meta.get(k).and_then(|m| m.sort_position.as_deref()).filter(|s| !s.is_empty());
            (idx, k, pos)
        })
        .collect();
    items.sort_by(|(a_idx, _, a_pos), (b_idx, _, b_pos)| match (a_pos, b_pos) {
        (Some(ap), Some(bp)) => ap.cmp(bp).then_with(|| a_idx.cmp(b_idx)),
        (Some(_), None) => std::cmp::Ordering::Less,
        (None, Some(_)) => std::cmp::Ordering::Greater,
        (None, None) => a_idx.cmp(b_idx),
    });
    items.into_iter().map(|(_, k, _)| k.clone()).collect()
}

thread_local! {
    static LIBRARY_PAGE_SCANS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// Returns and resets the number of [`used_on_library_page`] scans on the current thread.
pub fn take_library_page_scans() -> usize {
    LIBRARY_PAGE_SCANS.with(|c| c.replace(0))
}

/// Whether any node on the hidden library page (remote-component copies) links to style `key`.
pub fn used_on_library_page(doc: &Document, key: &str) -> bool {
    LIBRARY_PAGE_SCANS.with(|c| c.set(c.get() + 1));
    let Some(lp) = doc.library_page else { return false };
    let mut stack = vec![lp];
    while let Some(id) = stack.pop() {
        let Some(n) = doc.get(id) else { continue };
        if n.style_refs.values().any(|k| k == key) {
            return true;
        }
        stack.extend(n.children.iter().copied());
    }
    false
}

enum StyleBindingOverride {
    None,
    AllModes,
    SomeModesBake { val: VariableValue, fallback: bool },
}

fn collections_and_modes<'a>(doc: &'a Document, var_ids: &[&str]) -> Vec<(&'a str, Vec<&'a str>)> {
    let mut visited_vars = std::collections::BTreeSet::new();
    let mut queue: std::collections::VecDeque<&str> = var_ids.iter().copied().collect();
    let mut coll_ids: Vec<&'a str> = Vec::new();
    let mut push_coll = |cid: &'a str| {
        if !coll_ids.contains(&cid) {
            coll_ids.push(cid);
        }
    };

    while let Some(id) = queue.pop_front() {
        if visited_vars.len() >= 32 || !visited_vars.insert(id) {
            continue;
        }
        let Some(v) = doc.variables.get(id) else { continue };
        if doc.variable_collections.contains_key(&v.collection) {
            push_coll(v.collection.as_str());
        }
        for (k, val) in &v.values {
            if k.contains('+') {
                for part in k.split('+') {
                    for (cid, coll) in &doc.variable_collections {
                        if coll.default_mode == part || coll.modes.iter().any(|m| m.id == part) {
                            push_coll(cid.as_str());
                        }
                    }
                }
            }
            if let VariableValue::Alias(next) = val {
                queue.push_back(next.as_str());
            }
        }
    }

    let mut out = Vec::with_capacity(coll_ids.len());
    for cid in coll_ids {
        let Some(coll) = doc.variable_collections.get(cid) else { continue };
        let modes: Vec<&'a str> = if !coll.modes.is_empty() {
            coll.modes.iter().map(|m| m.id.as_str()).collect()
        } else if !coll.default_mode.is_empty() {
            vec![coll.default_mode.as_str()]
        } else {
            Vec::new()
        };
        if !modes.is_empty() {
            out.push((cid, modes));
        }
    }
    out
}

/// Classifies how `n`'s existing own bindings interact with moving `node_key` (`style_var`) onto
/// `n.variables.bindings` across every mode combination of every collection that `own` or
/// `style_var` uses.
fn classify_own_binding_override(doc: &Document, id: NodeId, n: &Node, node_key: &str, style_var: &str) -> StyleBindingOverride {
    let own = &n.variables.bindings;
    type OverwritePred = fn(&VariableValue) -> bool;
    let (own_key, overwrites, is_stop): (String, OverwritePred, bool) = if node_key == "fontWeight" {
        ("fontStyle".into(), |val| matches!(val, VariableValue::String(s) if font_style_weight(s).is_some()), false)
    } else if let Some(idx) = node_key.strip_prefix("fills/").and_then(|s| s.strip_suffix("/opacity")) {
        (format!("fills/{idx}"), |val| matches!(val, VariableValue::Color(c) if c.a < 1.0), false)
    } else if let Some(idx) = node_key.strip_prefix("strokes/").and_then(|s| s.strip_suffix("/opacity")) {
        (format!("strokes/{idx}"), |val| matches!(val, VariableValue::Color(c) if c.a < 1.0), false)
    } else if let Some(idx) = node_key.strip_prefix("effects/").and_then(|s| s.strip_suffix("/opacity")) {
        (format!("effects/{idx}/color"), |val| matches!(val, VariableValue::Color(_)), false)
    } else if let Some((idx, _)) = node_key.strip_prefix("fills/").and_then(|s| s.split_once("/stops/")) {
        (format!("fills/{idx}"), |val| matches!(val, VariableValue::Color(_)), true)
    } else if let Some((idx, _)) = node_key.strip_prefix("strokes/").and_then(|s| s.split_once("/stops/")) {
        (format!("strokes/{idx}"), |val| matches!(val, VariableValue::Color(_)), true)
    } else {
        return StyleBindingOverride::None;
    };

    let Some(own_var_id) = own.get(&own_key) else {
        return StyleBindingOverride::None;
    };
    let colls = collections_and_modes(doc, &[own_var_id.as_str(), style_var]);
    if colls.is_empty() {
        return StyleBindingOverride::None;
    }

    let mut combos: Vec<Vec<(&str, &str)>> = vec![Vec::with_capacity(colls.len())];
    for (cid, modes) in &colls {
        let mut next = Vec::with_capacity(combos.len() * modes.len());
        for prefix in &combos {
            for &m in modes {
                let mut c = prefix.clone();
                c.push((*cid, m));
                next.push(c);
            }
        }
        combos = next;
    }

    let mut any_overridden = false;
    let mut all_overridden = true;
    let mut style_won_vals: Vec<VariableValue> = Vec::new();
    for combo in &combos {
        let is_overridden = doc.resolve_variable_with_modes(id, own_var_id, combo).is_some_and(|val| overwrites(&val));
        if is_overridden {
            any_overridden = true;
        } else {
            all_overridden = false;
            if let Some(val) = doc.resolve_variable_with_modes(id, style_var, combo) {
                style_won_vals.push(val);
            }
        }
    }

    if !any_overridden {
        return StyleBindingOverride::None;
    }
    if all_overridden {
        return StyleBindingOverride::AllModes;
    }
    if is_stop {
        return StyleBindingOverride::None;
    }
    let Some(first_val) = style_won_vals.first() else {
        return StyleBindingOverride::AllModes;
    };
    if style_won_vals.iter().all(|v| v == first_val) {
        return StyleBindingOverride::SomeModesBake { val: first_val.clone(), fallback: false };
    }

    let own_wins_at_delete = doc.resolve_variable(id, own_var_id).is_some_and(|val| overwrites(&val));
    if !own_wins_at_delete {
        if let Some(val) = doc.resolve_variable(id, style_var) {
            return StyleBindingOverride::SomeModesBake { val, fallback: true };
        }
    }
    let default_combo: Vec<(&str, &str)> =
        colls.iter().filter_map(|(cid, _)| doc.variable_collections.get(*cid).map(|c| (*cid, c.default_mode.as_str()))).collect();
    let own_wins_in_default = doc.resolve_variable_with_modes(id, own_var_id, &default_combo).is_some_and(|val| overwrites(&val));
    if !own_wins_in_default {
        if let Some(val) = doc.resolve_variable_with_modes(id, style_var, &default_combo) {
            return StyleBindingOverride::SomeModesBake { val, fallback: true };
        }
    }
    StyleBindingOverride::SomeModesBake { val: first_val.clone(), fallback: true }
}

/// Bakes a resolved variable value for a style binding key outside `property_kind` onto `node`
/// so that unrecognized or legacy keys never vanish silently on style deletion.
fn bake_unrecognized_style_binding(node: &mut Node, slot: &str, key: &str, val: &VariableValue) {
    if let Some(stripped) = key.strip_suffix("/0") {
        if property_kind(stripped).is_some() {
            set_bound_property(node, stripped, val);
            return;
        }
    }
    if let Some(prefix) = key.strip_suffix("/color") {
        if (prefix.starts_with("fills/") || prefix.starts_with("strokes/")) && property_kind(prefix).is_some() {
            set_bound_property(node, prefix, val);
            return;
        }
    }
    if let Some(idx) = key.strip_prefix("fillOpacities/") {
        let k = format!("fills/{idx}/opacity");
        if property_kind(&k).is_some() {
            set_bound_property(node, &k, val);
            return;
        }
    }
    if let Some(idx) = key.strip_prefix("strokeOpacities/") {
        let k = format!("strokes/{idx}/opacity");
        if property_kind(&k).is_some() {
            set_bound_property(node, &k, val);
            return;
        }
    }
    if let Some((i, j)) = key.strip_prefix("fillStops/").and_then(|s| s.split_once('/')) {
        let k = format!("fills/{i}/stops/{j}");
        if property_kind(&k).is_some() {
            set_bound_property(node, &k, val);
            return;
        }
    }
    if let Some((i, j)) = key.strip_prefix("strokeStops/").and_then(|s| s.split_once('/')) {
        let k = format!("strokes/{i}/stops/{j}");
        if property_kind(&k).is_some() {
            set_bound_property(node, &k, val);
            return;
        }
    }
    if let Some(rest) = key.strip_prefix("effects/") {
        let mapped = if let Some(i) = rest.strip_suffix("/x").or_else(|| rest.strip_suffix("/offset/x")) {
            Some(format!("effects/{i}/offsetX"))
        } else {
            rest.strip_suffix("/y").or_else(|| rest.strip_suffix("/offset/y")).map(|i| format!("effects/{i}/offsetY"))
        };
        if let Some(k) = mapped {
            if property_kind(&k).is_some() {
                set_bound_property(node, &k, val);
                return;
            }
        }
    }
    match (slot, key) {
        ("fill", "color") => {
            set_bound_property(node, "fills/0", val);
            return;
        }
        ("stroke", "color") => {
            set_bound_property(node, "strokes/0", val);
            return;
        }
        ("fill", "opacity") => {
            set_bound_property(node, "fills/0/opacity", val);
            return;
        }
        ("stroke", "opacity") => {
            set_bound_property(node, "strokes/0/opacity", val);
            return;
        }
        ("effect", "color" | "opacity" | "radius" | "spread" | "offsetX" | "offsetY") => {
            set_bound_property(node, &format!("effects/0/{key}"), val);
            return;
        }
        ("effect", "x") => {
            set_bound_property(node, "effects/0/offsetX", val);
            return;
        }
        ("effect", "y") => {
            set_bound_property(node, "effects/0/offsetY", val);
            return;
        }
        _ => {}
    }
    match key {
        "cornerRadius" => {
            apply_bound_key_to_node(node, "cornerRadius", val);
        }
        "size/x" => set_bound_property(node, "width", val),
        "size/y" => set_bound_property(node, "height", val),
        "individualStrokeWeights/top" => set_bound_property(node, "strokeTopWeight", val),
        "individualStrokeWeights/right" => set_bound_property(node, "strokeRightWeight", val),
        "individualStrokeWeights/bottom" => set_bound_property(node, "strokeBottomWeight", val),
        "individualStrokeWeights/left" => set_bound_property(node, "strokeLeftWeight", val),
        "minWidth" => {
            if let VariableValue::Number(x) = *val {
                node.layout_child.min_w = Some(x.max(0.0));
            }
        }
        "maxWidth" => {
            if let VariableValue::Number(x) = *val {
                node.layout_child.max_w = Some(x.max(0.0));
            }
        }
        "minHeight" => {
            if let VariableValue::Number(x) = *val {
                node.layout_child.min_h = Some(x.max(0.0));
            }
        }
        "maxHeight" => {
            if let VariableValue::Number(x) = *val {
                node.layout_child.max_h = Some(x.max(0.0));
            }
        }
        _ => {
            set_bound_property(node, key, val);
        }
    }
}

/// Deletes style `key`, detaching every linked layer while keeping its current
/// values and moving any variable bindings carried by the style onto each
/// layer's own properties as a single undoable command. A library style (or a
/// style used by a remote-component copy on the hidden library page) is refused.
pub fn delete_style(doc: &Document, key: &str) -> Result<Command, StyleOpError> {
    delete_style_with_fallbacks(doc, key).map(|(cmd, _)| cmd)
}

/// Deletes style `key` like [`delete_style`] and also returns the `(NodeId, key)`
/// pairs where a mixed-mode override used the delete-time/default fallback bake.
pub fn delete_style_with_fallbacks(doc: &Document, key: &str) -> Result<(Command, Vec<(NodeId, String)>), StyleOpError> {
    if !doc.styles.contains_key(key) {
        return Err(StyleOpError::UnknownStyle(key.into()));
    }
    if is_library_style(doc, key) || used_on_library_page(doc, key) {
        return Err(StyleOpError::LibraryStyle(key.into()));
    }
    let meta_bindings = doc.style_meta.get(key).map(|m| &m.bindings);
    let mut styles = doc.styles.clone();
    styles.remove(key);
    let mut style_meta = doc.style_meta.clone();
    style_meta.remove(key);
    let mut cmds = vec![Command::SetStyles { styles, style_meta }];
    let mut fallbacks = Vec::new();
    for id in linked_style_nodes(doc, key) {
        let Some(n) = doc.get(id) else { continue };
        let mut copy = n.clone();
        if let Some(bindings) = meta_bindings {
            for slot in ["fill", "stroke", "text", "effect"] {
                if copy.style_refs.get(slot).is_none_or(|k| k != key) {
                    continue;
                }
                for (bkey, variable) in bindings {
                    let node_key = if slot == "stroke" {
                        let Some(rest) = bkey.strip_prefix("fills/") else { continue };
                        format!("strokes/{rest}")
                    } else {
                        bkey.clone()
                    };
                    if copy.variables.bindings.contains_key(&node_key) {
                        continue;
                    }
                    match classify_own_binding_override(doc, id, n, &node_key, variable) {
                        StyleBindingOverride::AllModes => continue,
                        StyleBindingOverride::SomeModesBake { val, fallback } => {
                            set_bound_property(&mut copy, &node_key, &val);
                            if fallback {
                                fallbacks.push((id, node_key));
                            }
                            continue;
                        }
                        StyleBindingOverride::None => {}
                    }
                    if property_kind(&node_key).is_some() {
                        copy.variables.bindings.insert(node_key, variable.clone());
                    } else if let Some(val) = doc.resolve_variable(id, variable) {
                        bake_unrecognized_style_binding(&mut copy, slot, &node_key, &val);
                    }
                }
            }
        }
        copy.style_refs.retain(|_, v| v != key);
        cmds.push(Command::Update { node: Box::new(copy) });
    }
    Ok((Command::Batch(cmds), fallbacks))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn shadow(radius: f32) -> Effect {
        Effect::DropShadow { color: Color::rgba(0, 0, 0, 64), offset: Vec2::new(0.0, 4.0), radius, spread: 0.0, visible: true, show_behind: false }
    }

    fn cols(count: u32) -> LayoutGrid {
        LayoutGrid::columns(count, 20.0, 0.0, Color::rgba(255, 0, 0, 26))
    }

    /// A page with a shadowed rectangle `a`, a plain rectangle `b`, a frame `f` with a 12 column grid and a plain frame `g`.
    fn fixture() -> (Document, NodeId, NodeId, NodeId, NodeId) {
        let mut d = Document::new("t");
        let page = d.add_page("p");
        let mut ids = Vec::new();
        for (i, (frame, name)) in [(false, "a"), (false, "b"), (true, "f"), (true, "g")].into_iter().enumerate() {
            let id = d.alloc_id();
            let r = Rect::new(10.0 * i as f32, 0.0, 50.0, 50.0);
            let mut n = if frame { Node::frame(id, r) } else { Node::rectangle(id, r) };
            n.name = name.into();
            ids.push(d.insert(page, n).unwrap());
        }
        let (a, b, f, g) = (ids[0], ids[1], ids[2], ids[3]);
        d.get_mut(a).unwrap().effects = vec![shadow(8.0)];
        d.get_mut(f).unwrap().kind.frame_props_mut().unwrap().grids = vec![cols(12)];
        (d, a, b, f, g)
    }

    fn run(d: &mut Document, h: &mut History, c: Result<Command, StyleOpError>) {
        h.exec(d, c.expect("operation accepted")).unwrap();
    }

    fn grids(d: &Document, id: NodeId) -> Vec<LayoutGrid> {
        d.get(id).unwrap().kind.frame_props().unwrap().grids.clone()
    }

    #[test]
    fn effect_style_is_created_from_a_node_and_undone_in_one_step() {
        let (mut d, a, ..) = fixture();
        let mut h = History::default();
        let (key, cmd) = create_style_from_node(&d, a, StyleProperty::Effect, None).unwrap();
        assert_eq!(key, "Effect style 1");
        h.exec(&mut d, cmd).unwrap();
        match &d.styles[&key] {
            Style::Effect(fx) => assert_eq!(fx, &vec![shadow(8.0)]),
            other => panic!("expected an effect style, got {other:?}"),
        }
        assert_eq!(
            d.style_meta[&key],
            StyleMeta {
                name: "Effect style 1".into(),
                key: None,
                description: String::new(),
                property: StyleProperty::Effect,
                bindings: Default::default(),
                ..Default::default()
            }
        );
        assert_eq!(d.get(a).unwrap().style_refs.get("effect"), Some(&key));
        h.undo(&mut d).unwrap();
        assert!(d.styles.is_empty() && d.style_meta.is_empty());
        assert!(d.get(a).unwrap().style_refs.is_empty());
    }

    #[test]
    fn grid_style_is_created_from_a_frame_with_a_chosen_name() {
        let (mut d, _a, _b, f, _g) = fixture();
        let mut h = History::default();
        let (key, cmd) = create_style_from_node(&d, f, StyleProperty::Grid, Some(" Layout/12 col ")).unwrap();
        h.exec(&mut d, cmd).unwrap();
        assert_eq!(key, "Layout/12 col");
        assert_eq!(d.style_name(&key), "Layout/12 col");
        assert!(matches!(&d.styles[&key], Style::Grid(g) if g == &vec![cols(12)]));
        assert_eq!(d.get(f).unwrap().style_refs.get("grid"), Some(&key));
        // A taken name gets a numeric suffix instead of overwriting.
        let (second, _) = create_style_from_node(&d, f, StyleProperty::Grid, Some("Layout/12 col")).unwrap();
        assert_eq!(second, "Layout/12 col 1");
    }

    #[test]
    fn creation_is_refused_without_values_or_on_the_wrong_node() {
        let (d, a, b, f, g) = fixture();
        assert!(matches!(create_style_from_node(&d, b, StyleProperty::Effect, None).unwrap_err(), StyleOpError::NothingToStyle(x) if x == b));
        assert!(matches!(create_style_from_node(&d, g, StyleProperty::Grid, None).unwrap_err(), StyleOpError::NothingToStyle(x) if x == g));
        assert!(matches!(create_style_from_node(&d, a, StyleProperty::Grid, None).unwrap_err(), StyleOpError::WrongNodeKind(x) if x == a));
        assert!(matches!(create_style_from_node(&d, f, StyleProperty::Fill, None).unwrap_err(), StyleOpError::NotEffectOrGrid(StyleProperty::Fill)));
        assert!(matches!(create_style_from_node(&d, a, StyleProperty::Effect, Some("  ")).unwrap_err(), StyleOpError::EmptyName));
        assert!(matches!(create_style_from_node(&d, NodeId(9999), StyleProperty::Effect, None), Err(StyleOpError::Doc(DocError::NotFound(_)))));
    }

    #[test]
    fn applying_links_nodes_and_copies_values() {
        let (mut d, a, b, f, g) = fixture();
        let mut h = History::default();
        let (fx_key, c) = create_style_from_node(&d, a, StyleProperty::Effect, None).unwrap();
        h.exec(&mut d, c).unwrap();
        let (grid_key, c) = create_style_from_node(&d, f, StyleProperty::Grid, None).unwrap();
        h.exec(&mut d, c).unwrap();
        // The effect lands on a frame and a rectangle; the grid skips the rectangle.
        let c = apply_style(&d, &[b, g], &fx_key);
        run(&mut d, &mut h, c);
        let c = apply_style(&d, &[b, g], &grid_key);
        run(&mut d, &mut h, c);
        assert_eq!(d.get(b).unwrap().effects, vec![shadow(8.0)]);
        assert_eq!(d.get(b).unwrap().style_refs.get("effect"), Some(&fx_key));
        assert!(!d.get(b).unwrap().style_refs.contains_key("grid"), "a rectangle has no grids");
        assert_eq!(d.get(g).unwrap().style_refs.get("grid"), Some(&grid_key));
        assert_eq!(grids(&d, g), vec![cols(12)]);
        h.undo(&mut d).unwrap();
        assert!(!d.get(g).unwrap().style_refs.contains_key("grid"));
        assert!(grids(&d, g).is_empty());
        assert!(matches!(apply_style(&d, &[b], "nope").unwrap_err(), StyleOpError::UnknownStyle(k) if k == "nope"));
    }

    #[test]
    fn editing_a_style_updates_every_linked_node_and_only_those() {
        let (mut d, a, b, f, g) = fixture();
        let mut h = History::default();
        let (key, c) = create_style_from_node(&d, a, StyleProperty::Effect, None).unwrap();
        h.exec(&mut d, c).unwrap();
        let c = apply_style(&d, &[f], &key);
        run(&mut d, &mut h, c);
        let c = update_style(&d, &key, Style::Effect(vec![shadow(24.0), Effect::LayerBlur { radius: 2.0, visible: true }]));
        run(&mut d, &mut h, c);
        let expected = vec![shadow(24.0), Effect::LayerBlur { radius: 2.0, visible: true }];
        assert_eq!(d.get(a).unwrap().effects, expected);
        assert_eq!(d.get(f).unwrap().effects, expected);
        assert!(d.get(b).unwrap().effects.is_empty() && d.get(g).unwrap().effects.is_empty(), "unlinked nodes are untouched");
        // A grid style cannot be overwritten by effect values.
        let (gkey, c) = create_style_from_node(&d, f, StyleProperty::Grid, None).unwrap();
        h.exec(&mut d, c).unwrap();
        assert!(update_style(&d, &gkey, Style::Effect(vec![shadow(1.0)])).is_err());
        let c = update_style(&d, &gkey, Style::Grid(vec![cols(4)]));
        run(&mut d, &mut h, c);
        assert_eq!(grids(&d, f), vec![cols(4)]);
        h.undo(&mut d).unwrap();
        assert_eq!(grids(&d, f), vec![cols(12)]);
    }

    /// Page with a linked rectangle per fill / stroke style, a text layer linked to a text and a
    /// fill style, and an unlinked rectangle. Returns the document and the ids in that order.
    fn paint_and_text_fixture() -> (Document, [NodeId; 4]) {
        let red = Paint::solid(Color::rgb(255, 0, 0));
        let mut d = Document::new("t");
        let page = d.add_page("p");
        let mut ids = Vec::new();
        for i in 0..4 {
            let id = d.alloc_id();
            let r = Rect::new(60.0 * i as f32, 0.0, 50.0, 50.0);
            let n = if i == 2 {
                Node::text(id, r, "Hi", TextStyle { font_size: 12.0, fills: vec![red.clone()], ..Default::default() })
            } else {
                Node::rectangle(id, r)
            };
            ids.push(d.insert(page, n).unwrap());
        }
        d.styles.insert("Brand".into(), Style::Fill(vec![red.clone()]));
        d.styles.insert("Line".into(), Style::Fill(vec![Paint::solid(Color::rgb(0, 255, 0))]));
        d.style_meta.insert(
            "Line".into(),
            StyleMeta {
                name: "Line".into(),
                key: None,
                description: String::new(),
                property: StyleProperty::Stroke,
                bindings: Default::default(),
                ..Default::default()
            },
        );
        d.styles.insert("Body".into(), Style::Text(TextStyle { font_size: 12.0, ..Default::default() }));
        // Metadata that never named the property (it defaults to Fill) must not hide the text layers.
        d.style_meta.insert("Body".into(), StyleMeta { name: "Body".into(), ..Default::default() });
        for (id, refs) in [(ids[0], vec![("fill", "Brand")]), (ids[1], vec![("stroke", "Line")]), (ids[2], vec![("text", "Body"), ("fill", "Brand")])] {
            let n = d.get_mut(id).unwrap();
            for (k, v) in refs {
                n.style_refs.insert(k.into(), v.into());
            }
        }
        (d, [ids[0], ids[1], ids[2], ids[3]])
    }

    #[test]
    fn editing_a_fill_stroke_or_text_style_updates_every_linked_node_in_one_undo_step() {
        let (mut d, [fill_node, stroke_node, text_node, loose]) = paint_and_text_fixture();
        let mut h = History::default();
        let loose_fills = d.get(loose).unwrap().fills.clone();
        let stroke_before = d.get(stroke_node).unwrap().stroke.paints.clone();
        let blue = Paint::solid(Color::rgb(0, 0, 255));
        let c = update_style(&d, "Brand", Style::Fill(vec![blue.clone()]));
        run(&mut d, &mut h, c);
        assert!(matches!(&d.styles["Brand"], Style::Fill(p) if p == &vec![blue.clone()]));
        assert_eq!(d.get(fill_node).unwrap().fills, vec![blue.clone()]);
        match &d.get(text_node).unwrap().kind {
            NodeKind::Text(t) => assert_eq!(t.style.fills, vec![blue.clone()], "a linked text layer takes the colour"),
            other => panic!("{other:?}"),
        }
        assert_eq!(d.get(loose).unwrap().fills, loose_fills, "unlinked nodes are untouched");

        let white = Paint::solid(Color::rgb(255, 255, 255));
        let c = update_style(&d, "Line", Style::Fill(vec![white.clone()]));
        run(&mut d, &mut h, c);
        assert_eq!(d.get(stroke_node).unwrap().stroke.paints, vec![white]);
        assert!(d.get(fill_node).unwrap().fills != d.get(stroke_node).unwrap().stroke.paints);

        let c = update_style(&d, "Body", Style::Text(TextStyle { font_size: 20.0, font_weight: 700, ..Default::default() }));
        run(&mut d, &mut h, c);
        match &d.get(text_node).unwrap().kind {
            NodeKind::Text(t) => {
                assert_eq!((t.style.font_size, t.style.font_weight), (20.0, 700));
                assert_eq!(t.style.fills, vec![blue.clone()], "a text style carries no colour");
            }
            other => panic!("{other:?}"),
        }
        // One undo step per edit, each taking the style and all of its layers back together.
        h.undo(&mut d).unwrap();
        assert!(matches!(&d.styles["Body"], Style::Text(t) if t.font_size == 12.0));
        match &d.get(text_node).unwrap().kind {
            NodeKind::Text(t) => assert_eq!(t.style.font_size, 12.0),
            other => panic!("{other:?}"),
        }
        h.undo(&mut d).unwrap();
        assert_eq!(d.get(stroke_node).unwrap().stroke.paints, stroke_before);
    }

    #[test]
    fn a_fill_style_linked_as_fill_and_as_stroke_updates_both_sides() {
        let (mut d, [fill_node, stroke_node, text_node, _loose]) = paint_and_text_fixture();
        let mut h = History::default();
        // The shape imports give one style that is used as a fill and as a stroke the property `Fill`.
        d.style_meta.remove("Brand");
        d.get_mut(stroke_node).unwrap().style_refs.clear();
        d.get_mut(stroke_node).unwrap().style_refs.insert("stroke".into(), "Brand".into());
        let both = d.alloc_id();
        let page = d.pages[0];
        let mut n = Node::rectangle(both, Rect::new(0.0, 100.0, 50.0, 50.0));
        n.style_refs.insert("fill".into(), "Brand".into());
        n.style_refs.insert("stroke".into(), "Brand".into());
        d.insert(page, n).unwrap();
        // A copy on the hidden library page is not the file's own layer: it keeps its values.
        let lp = crate::library_ops::ensure_library_page(&mut d);
        let hidden = d.alloc_id();
        let mut n = Node::rectangle(hidden, Rect::new(0.0, 0.0, 5.0, 5.0));
        n.style_refs.insert("fill".into(), "Brand".into());
        d.insert(lp, n).unwrap();
        let hidden_before = d.get(hidden).unwrap().clone();
        let stroke_before = d.get(stroke_node).unwrap().stroke.paints.clone();
        let blue = Paint::solid(Color::rgb(0, 0, 255));
        let c = update_style(&d, "Brand", Style::Fill(vec![blue.clone()]));
        run(&mut d, &mut h, c);
        assert_eq!(d.get(hidden).unwrap(), &hidden_before);
        assert_eq!(d.get(fill_node).unwrap().fills, vec![blue.clone()]);
        assert_eq!(d.get(stroke_node).unwrap().stroke.paints, vec![blue.clone()], "a stroke-only link is updated");
        assert_eq!(d.get(both).unwrap().fills, vec![blue.clone()]);
        assert_eq!(d.get(both).unwrap().stroke.paints, vec![blue.clone()], "one node linked on both sides gets both");
        match &d.get(text_node).unwrap().kind {
            NodeKind::Text(t) => assert_eq!(t.style.fills, vec![blue]),
            other => panic!("{other:?}"),
        }
        h.undo(&mut d).unwrap();
        assert_eq!(d.get(stroke_node).unwrap().stroke.paints, stroke_before);
    }

    #[test]
    fn a_colour_style_edit_recolours_every_run_of_a_rich_text_layer() {
        let (mut d, [_, _, text_node, _]) = paint_and_text_fixture();
        let mut h = History::default();
        let red = Paint::solid(Color::rgb(255, 0, 0));
        let green = Paint::solid(Color::rgb(0, 255, 0));
        if let NodeKind::Text(t) = &mut d.get_mut(text_node).unwrap().kind {
            t.runs = vec![
                TextRun { start: 0, end: 1, style: TextStyle { font_size: 12.0, fills: vec![red.clone()], ..Default::default() } },
                TextRun { start: 1, end: 2, style: TextStyle { font_size: 12.0, fills: vec![green], ..Default::default() } },
            ];
        }
        let blue = Paint::solid(Color::rgb(0, 0, 255));
        let c = update_style(&d, "Brand", Style::Fill(vec![blue.clone()]));
        run(&mut d, &mut h, c);
        match &d.get(text_node).unwrap().kind {
            NodeKind::Text(t) => {
                assert_eq!(t.style.fills, vec![blue.clone()]);
                assert_eq!(t.runs.len(), 2);
                for r in &t.runs {
                    assert_eq!(r.style.fills, vec![blue.clone()], "run {}..{} keeps its old colour", r.start, r.end);
                    assert_eq!(r.style.font_size, 12.0, "only the colour changes");
                }
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn a_style_that_belongs_to_a_library_cannot_be_edited_in_the_consumer_file() {
        let (mut d, [fill_node, ..]) = paint_and_text_fixture();
        d.library.external.insert(
            "L1/Brand".into(),
            ExternalAsset { library_id: "L1".into(), version: 1, kind: LibraryAssetKind::Style, node: None, local_key: Some("Brand".into()) },
        );
        assert!(is_library_style(&d, "Brand"));
        assert!(!is_library_style(&d, "Line"));
        let blue = Paint::solid(Color::rgb(0, 0, 255));
        assert!(matches!(update_style(&d, "Brand", Style::Fill(vec![blue.clone()])).unwrap_err(), StyleOpError::LibraryStyle(k) if k == "Brand"));
        assert_ne!(d.get(fill_node).unwrap().fills, vec![blue.clone()]);
        // A file's own styles stay editable.
        assert!(update_style(&d, "Line", Style::Fill(vec![blue])).is_ok());
    }

    #[test]
    fn a_style_that_belongs_to_a_library_cannot_be_renamed_in_the_consumer_file() {
        let (mut d, _) = paint_and_text_fixture();
        d.library.external.insert(
            "L1/Brand".into(),
            ExternalAsset { library_id: "L1".into(), version: 1, kind: LibraryAssetKind::Style, node: None, local_key: Some("Brand".into()) },
        );
        assert!(matches!(rename_style(&d, "Brand", "Brand/Renamed").unwrap_err(), StyleOpError::LibraryStyle(k) if k == "Brand"));
        assert_eq!(d.style_name("Brand"), "Brand");
        // A local style can still be renamed.
        let mut h = History::default();
        let c = rename_style(&d, "Line", "Line/Thin");
        run(&mut d, &mut h, c);
        assert_eq!(d.style_name("Line"), "Line/Thin");
    }

    #[test]
    fn editing_a_style_with_the_wrong_kind_or_an_unknown_key_is_refused() {
        let (mut d, _) = paint_and_text_fixture();
        assert!(matches!(update_style(&d, "Brand", Style::Text(TextStyle::default())).unwrap_err(), StyleOpError::KindMismatch(k) if k == "Brand"));
        assert!(matches!(update_style(&d, "Body", Style::Fill(vec![])).unwrap_err(), StyleOpError::KindMismatch(k) if k == "Body"));
        assert!(matches!(update_style(&d, "nope", Style::Fill(vec![])).unwrap_err(), StyleOpError::UnknownStyle(k) if k == "nope"));
        d.variables.insert(
            "spacing".into(),
            Variable {
                id: "spacing".into(),
                name: "spacing".into(),
                collection: "c".into(),
                kind: VariableKind::Number,
                values: Default::default(),
                description: String::new(),
                scopes: vec![],
                hidden_from_publishing: false,
                key: None,
                code_syntax: Default::default(),
                extensions: Default::default(),
                mode_extensions: Default::default(),
            },
        );
        let err = bind_style_property(&d, "Brand", "fills/0", "spacing").unwrap_err();
        assert!(matches!(&err, StyleOpError::KindMismatch(k) if k.contains("fills/0") && k.contains("spacing")), "got: {err:?}");
    }

    #[test]
    fn detaching_drops_the_link_but_keeps_the_values() {
        let (mut d, a, b, ..) = fixture();
        let mut h = History::default();
        let (key, c) = create_style_from_node(&d, a, StyleProperty::Effect, None).unwrap();
        h.exec(&mut d, c).unwrap();
        let c = apply_style(&d, &[b], &key);
        run(&mut d, &mut h, c);
        let c = detach_style(&d, &[a, b, b], StyleProperty::Effect);
        run(&mut d, &mut h, c);
        for id in [a, b] {
            assert!(d.get(id).unwrap().style_refs.is_empty());
            assert_eq!(d.get(id).unwrap().effects, vec![shadow(8.0)]);
        }
        assert!(d.styles.contains_key(&key), "the style itself stays");
        // Editing the style no longer reaches detached nodes.
        let c = update_style(&d, &key, Style::Effect(vec![shadow(1.0)]));
        run(&mut d, &mut h, c);
        assert_eq!(d.get(a).unwrap().effects, vec![shadow(8.0)]);
        assert!(detach_style(&d, &[a], StyleProperty::Text).is_err());
    }

    #[test]
    fn renaming_changes_the_display_name_only() {
        let (mut d, a, ..) = fixture();
        let mut h = History::default();
        let (key, c) = create_style_from_node(&d, a, StyleProperty::Effect, None).unwrap();
        h.exec(&mut d, c).unwrap();
        let c = rename_style(&d, &key, " Shadow/Card ");
        run(&mut d, &mut h, c);
        assert_eq!(d.style_name(&key), "Shadow/Card");
        assert_eq!(d.get(a).unwrap().style_refs.get("effect"), Some(&key));
        assert!(matches!(rename_style(&d, &key, "").unwrap_err(), StyleOpError::EmptyName));
        assert!(matches!(rename_style(&d, "nope", "x").unwrap_err(), StyleOpError::UnknownStyle(k) if k == "nope"));
        h.undo(&mut d).unwrap();
        assert_eq!(d.style_name(&key), "Effect style 1");
        // A style without metadata gets some when it is renamed.
        d.style_meta.remove(&key);
        let c = rename_style(&d, &key, "Plain");
        run(&mut d, &mut h, c);
        assert_eq!(d.style_meta[&key].property, StyleProperty::Effect);
    }

    #[test]
    fn effect_and_grid_styles_survive_a_native_save_and_load() {
        let (mut d, a, _b, f, _g) = fixture();
        let mut h = History::default();
        let (fx, c) = create_style_from_node(&d, a, StyleProperty::Effect, Some("Shadow/Card")).unwrap();
        h.exec(&mut d, c).unwrap();
        let (gr, c) = create_style_from_node(&d, f, StyleProperty::Grid, Some("Layout/12 col")).unwrap();
        h.exec(&mut d, c).unwrap();
        let loaded = Document::from_json(&d.to_json().unwrap()).unwrap();
        assert!(matches!(&loaded.styles[&fx], Style::Effect(v) if v == &vec![shadow(8.0)]));
        assert!(matches!(&loaded.styles[&gr], Style::Grid(v) if v == &vec![cols(12)]));
        assert_eq!(loaded.style_meta[&fx].property, StyleProperty::Effect);
        assert_eq!(loaded.style_meta[&gr].property, StyleProperty::Grid);
        assert_eq!(loaded.style_name(&fx), "Shadow/Card");
        assert_eq!(loaded.get(a).unwrap().style_refs.get("effect"), Some(&fx));
        assert_eq!(loaded.get(f).unwrap().style_refs.get("grid"), Some(&gr));
        // The loaded copy keeps working: editing the style reaches the linked node.
        let c = update_style(&loaded, &fx, Style::Effect(vec![shadow(2.0)])).unwrap();
        let mut loaded = loaded;
        h.exec(&mut loaded, c).unwrap();
        assert_eq!(loaded.get(a).unwrap().effects, vec![shadow(2.0)]);
    }

    #[test]
    fn deleting_fill_text_effect_and_grid_styles_keeps_layer_values_drops_refs_and_undoes_in_one_step() {
        let mut d = Document::new("t");
        let page = d.add_page("p");
        d.variable_collections.insert(
            "c1".into(),
            VariableCollection {
                id: "c1".into(),
                name: "Theme".into(),
                modes: vec![VariableMode { id: "light".into(), name: "Light".into() }, VariableMode { id: "dark".into(), name: "Dark".into() }],
                default_mode: "light".into(),
                variables: vec!["v_fill".into()],
                key: None,
                source_library_key: None,
                extensions: Default::default(),
            },
        );
        let light_col = Color::rgb(10, 20, 30);
        let dark_col = Color::rgb(200, 210, 220);
        d.variables.insert(
            "v_fill".into(),
            Variable {
                id: "v_fill".into(),
                name: "Brand/Surface".into(),
                collection: "c1".into(),
                kind: VariableKind::Color,
                values: [("light".to_string(), VariableValue::Color(light_col)), ("dark".to_string(), VariableValue::Color(dark_col))].into_iter().collect(),
                description: String::new(),
                scopes: vec![],
                hidden_from_publishing: false,
                key: None,
                code_syntax: Default::default(),
                extensions: Default::default(),
                mode_extensions: Default::default(),
            },
        );

        let f1_id = d.alloc_id();
        let mut f1 = Node::frame(f1_id, Rect::new(0.0, 0.0, 100.0, 100.0));
        f1.fills = vec![Paint::solid(light_col)];
        f1.effects = vec![shadow(8.0)];
        f1.kind.frame_props_mut().unwrap().grids = vec![cols(12)];
        f1.style_refs.insert("fill".into(), "FillStyle".into());
        f1.style_refs.insert("effect".into(), "EffectStyle".into());
        f1.style_refs.insert("grid".into(), "GridStyle".into());
        let f1 = d.insert(page, f1).unwrap();

        let f2_id = d.alloc_id();
        let mut f2 = Node::frame(f2_id, Rect::new(120.0, 0.0, 100.0, 100.0));
        f2.variables.modes.insert("c1".into(), "dark".into());
        f2.fills = vec![Paint::solid(light_col)];
        f2.effects = vec![shadow(8.0)];
        f2.kind.frame_props_mut().unwrap().grids = vec![cols(12)];
        f2.style_refs.insert("fill".into(), "FillStyle".into());
        f2.style_refs.insert("effect".into(), "EffectStyle".into());
        f2.style_refs.insert("grid".into(), "GridStyle".into());
        let f2 = d.insert(page, f2).unwrap();

        let ts = TextStyle { font_size: 24.0, font_weight: 700, ..Default::default() };
        let t1_id = d.alloc_id();
        let mut t1 = Node::text(t1_id, Rect::new(0.0, 120.0, 80.0, 30.0), "One", ts.clone());
        t1.style_refs.insert("text".into(), "TextStyle".into());
        let t1 = d.insert(page, t1).unwrap();

        let t2_id = d.alloc_id();
        let mut t2 = Node::text(t2_id, Rect::new(120.0, 120.0, 80.0, 30.0), "Two", ts.clone());
        t2.style_refs.insert("text".into(), "TextStyle".into());
        let t2 = d.insert(page, t2).unwrap();

        d.styles.insert("FillStyle".into(), Style::Fill(vec![Paint::solid(light_col)]));
        d.style_meta.insert(
            "FillStyle".into(),
            StyleMeta {
                name: "FillStyle".into(),
                key: None,
                description: String::new(),
                property: StyleProperty::Fill,
                bindings: [("fills/0".to_string(), "v_fill".to_string())].into_iter().collect(),
                ..Default::default()
            },
        );
        d.styles.insert("TextStyle".into(), Style::Text(ts.clone()));
        d.style_meta.insert("TextStyle".into(), StyleMeta { name: "TextStyle".into(), property: StyleProperty::Text, ..Default::default() });
        d.styles.insert("EffectStyle".into(), Style::Effect(vec![shadow(8.0)]));
        d.style_meta.insert("EffectStyle".into(), StyleMeta { name: "EffectStyle".into(), property: StyleProperty::Effect, ..Default::default() });
        d.styles.insert("GridStyle".into(), Style::Grid(vec![cols(12)]));
        d.style_meta.insert("GridStyle".into(), StyleMeta { name: "GridStyle".into(), property: StyleProperty::Grid, ..Default::default() });

        let mut h = History::default();

        // 1. Delete FillStyle (bound to v_fill: resolves to light_col on f1, dark_col on f2).
        let f1_before = resolve_node(&d, d.get(f1).unwrap()).fills.clone();
        let f2_before = resolve_node(&d, d.get(f2).unwrap()).fills.clone();
        assert_eq!(f1_before, vec![Paint::solid(light_col)]);
        assert_eq!(f2_before, vec![Paint::solid(dark_col)]);

        let c = delete_style(&d, "FillStyle");
        run(&mut d, &mut h, c);
        assert!(!d.styles.contains_key("FillStyle") && !d.style_meta.contains_key("FillStyle"));
        assert!(!d.get(f1).unwrap().style_refs.contains_key("fill"));
        assert!(!d.get(f2).unwrap().style_refs.contains_key("fill"));
        assert_eq!(d.get(f1).unwrap().variables.bindings.get("fills/0").map(String::as_str), Some("v_fill"));
        assert_eq!(d.get(f2).unwrap().variables.bindings.get("fills/0").map(String::as_str), Some("v_fill"));
        assert_eq!(resolve_node(&d, d.get(f1).unwrap()).fills, f1_before, "light-mode layer keeps its look");
        assert_eq!(resolve_node(&d, d.get(f2).unwrap()).fills, f2_before, "dark-mode layer keeps its look");
        assert_eq!(d.get(f2).unwrap().fills, vec![Paint::solid(light_col)], "do not bake a resolved dark literal into raw fills");
        // Switching the layer's mode after delete still changes its colour.
        d.get_mut(f1).unwrap().variables.modes.insert("c1".into(), "dark".into());
        d.get_mut(f2).unwrap().variables.modes.insert("c1".into(), "light".into());
        assert_eq!(resolve_node(&d, d.get(f1).unwrap()).fills, vec![Paint::solid(dark_col)]);
        assert_eq!(resolve_node(&d, d.get(f2).unwrap()).fills, vec![Paint::solid(light_col)]);
        d.get_mut(f1).unwrap().variables.modes.remove("c1");
        d.get_mut(f2).unwrap().variables.modes.insert("c1".into(), "dark".into());

        h.undo(&mut d).unwrap();
        assert!(d.styles.contains_key("FillStyle") && d.style_meta.contains_key("FillStyle"));
        assert_eq!(d.get(f1).unwrap().style_refs.get("fill").map(String::as_str), Some("FillStyle"));
        assert_eq!(d.get(f2).unwrap().style_refs.get("fill").map(String::as_str), Some("FillStyle"));
        assert!(d.get(f1).unwrap().variables.bindings.is_empty());
        assert!(d.get(f2).unwrap().variables.bindings.is_empty());
        h.redo(&mut d).unwrap();
        assert!(!d.styles.contains_key("FillStyle") && !d.get(f1).unwrap().style_refs.contains_key("fill"));

        // 2. Delete TextStyle.
        let c = delete_style(&d, "TextStyle");
        run(&mut d, &mut h, c);
        assert!(!d.styles.contains_key("TextStyle") && !d.style_meta.contains_key("TextStyle"));
        for id in [t1, t2] {
            let n = d.get(id).unwrap();
            assert!(!n.style_refs.contains_key("text"));
            match &n.kind {
                NodeKind::Text(tp) => assert_eq!((tp.style.font_size, tp.style.font_weight), (24.0, 700)),
                other => panic!("expected text, got {other:?}"),
            }
        }
        h.undo(&mut d).unwrap();
        assert!(d.styles.contains_key("TextStyle"));
        assert_eq!(d.get(t1).unwrap().style_refs.get("text").map(String::as_str), Some("TextStyle"));
        assert_eq!(d.get(t2).unwrap().style_refs.get("text").map(String::as_str), Some("TextStyle"));
        h.redo(&mut d).unwrap();

        // 3. Delete EffectStyle.
        let c = delete_style(&d, "EffectStyle");
        run(&mut d, &mut h, c);
        assert!(!d.styles.contains_key("EffectStyle") && !d.style_meta.contains_key("EffectStyle"));
        for id in [f1, f2] {
            let n = d.get(id).unwrap();
            assert!(!n.style_refs.contains_key("effect"));
            assert_eq!(n.effects, vec![shadow(8.0)]);
        }
        h.undo(&mut d).unwrap();
        assert!(d.styles.contains_key("EffectStyle"));
        assert_eq!(d.get(f1).unwrap().style_refs.get("effect").map(String::as_str), Some("EffectStyle"));
        assert_eq!(d.get(f2).unwrap().style_refs.get("effect").map(String::as_str), Some("EffectStyle"));
        h.redo(&mut d).unwrap();

        // 4. Delete GridStyle.
        let c = delete_style(&d, "GridStyle");
        run(&mut d, &mut h, c);
        assert!(!d.styles.contains_key("GridStyle") && !d.style_meta.contains_key("GridStyle"));
        for id in [f1, f2] {
            assert!(!d.get(id).unwrap().style_refs.contains_key("grid"));
            assert_eq!(grids(&d, id), vec![cols(12)]);
        }
        h.undo(&mut d).unwrap();
        assert!(d.styles.contains_key("GridStyle"));
        assert_eq!(d.get(f1).unwrap().style_refs.get("grid").map(String::as_str), Some("GridStyle"));
        assert_eq!(d.get(f2).unwrap().style_refs.get("grid").map(String::as_str), Some("GridStyle"));
    }

    #[test]
    fn a_style_that_belongs_to_a_library_cannot_be_deleted_in_the_consumer_file() {
        let (mut d, [fill_node, ..]) = paint_and_text_fixture();
        d.library.external.insert(
            "L1/Brand".into(),
            ExternalAsset { library_id: "L1".into(), version: 1, kind: LibraryAssetKind::Style, node: None, local_key: Some("Brand".into()) },
        );
        let err = delete_style(&d, "Brand").unwrap_err();
        assert!(matches!(&err, StyleOpError::LibraryStyle(k) if k == "Brand"), "expected LibraryStyle error, got {err:?}");
        assert!(err.to_string().contains("edit it in the library file"));
        assert!(d.styles.contains_key("Brand"));
        assert_eq!(d.get(fill_node).unwrap().style_refs.get("fill").map(String::as_str), Some("Brand"));
        assert!(matches!(delete_style(&d, "nope").unwrap_err(), StyleOpError::UnknownStyle(k) if k == "nope"));
    }

    fn mixed_run_text(d: &mut Document, page: NodeId, base: TextStyle, run2: TextStyle) -> NodeId {
        let id = d.alloc_id();
        let mut t = Node::text(id, Rect::new(0.0, 0.0, 200.0, 30.0), "Hello world", base.clone());
        if let NodeKind::Text(tp) = &mut t.kind {
            tp.runs = vec![TextRun { start: 0, end: 5, style: base }, TextRun { start: 5, end: 11, style: run2 }];
        }
        d.insert(page, t).unwrap()
    }

    fn resolved_runs(d: &Document, id: NodeId) -> Vec<(Vec<Paint>, u16, f32)> {
        let n = resolve_node(d, d.get(id).unwrap()).into_owned();
        match n.kind {
            NodeKind::Text(t) => t.runs.iter().map(|r| (r.style.fills.clone(), r.style.font_weight, r.style.font_size)).collect(),
            other => panic!("expected text, got {other:?}"),
        }
    }

    #[test]
    fn deleting_a_colour_style_on_text_keeps_a_highlighted_run() {
        let mut d = Document::new("t");
        let page = d.add_page("p");
        let black = vec![Paint::solid(Color::BLACK)];
        let red = vec![Paint::solid(Color::rgb(255, 0, 0))];
        let base = TextStyle { fills: black.clone(), ..Default::default() };
        let hi = TextStyle { fills: red.clone(), ..Default::default() };
        let t = mixed_run_text(&mut d, page, base, hi);
        d.get_mut(t).unwrap().style_refs.insert("fill".into(), "Ink".into());
        d.styles.insert("Ink".into(), Style::Fill(black.clone()));
        d.style_meta.insert("Ink".into(), StyleMeta { name: "Ink".into(), property: StyleProperty::Fill, ..Default::default() });

        let before_runs = resolved_runs(&d, t);
        let mut before_node = resolve_node(&d, d.get(t).unwrap()).into_owned();
        before_node.style_refs.clear();

        let mut h = History::default();
        let c = delete_style(&d, "Ink");
        run(&mut d, &mut h, c);

        let after_runs = resolved_runs(&d, t);
        let after_node = resolve_node(&d, d.get(t).unwrap()).into_owned();
        assert_eq!(after_runs, before_runs, "every run keeps its colour after deleting the fill style");
        assert_eq!(after_runs[0].0, black);
        assert_eq!(after_runs[1].0, red);
        assert_eq!(after_node, before_node, "resolved node data matches before and after delete");
    }

    #[test]
    fn deleting_a_text_style_keeps_a_bold_run() {
        let mut d = Document::new("t");
        let page = d.add_page("p");
        let base = TextStyle { font_size: 16.0, font_weight: 400, ..Default::default() };
        let bold = TextStyle { font_weight: 700, ..base.clone() };
        let t = mixed_run_text(&mut d, page, base.clone(), bold);
        d.get_mut(t).unwrap().style_refs.insert("text".into(), "Body".into());
        d.styles.insert("Body".into(), Style::Text(base));
        d.style_meta.insert("Body".into(), StyleMeta { name: "Body".into(), property: StyleProperty::Text, ..Default::default() });

        let before_runs = resolved_runs(&d, t);
        let mut before_node = resolve_node(&d, d.get(t).unwrap()).into_owned();
        before_node.style_refs.clear();

        let mut h = History::default();
        let c = delete_style(&d, "Body");
        run(&mut d, &mut h, c);

        let after_runs = resolved_runs(&d, t);
        let after_node = resolve_node(&d, d.get(t).unwrap()).into_owned();
        assert_eq!(after_runs, before_runs, "every run keeps its weight after deleting the text style");
        assert_eq!((after_runs[0].1, after_runs[1].1), (400, 700));
        assert_eq!(after_node, before_node, "resolved node data matches before and after delete");
    }

    #[test]
    fn deleting_a_fill_style_linked_as_fill_and_stroke_moves_both_variable_bindings_and_follows_mode_switches() {
        let mut d = Document::new("t");
        let page = d.add_page("p");
        d.variable_collections.insert(
            "c1".into(),
            VariableCollection {
                id: "c1".into(),
                name: "Theme".into(),
                modes: vec![VariableMode { id: "light".into(), name: "Light".into() }, VariableMode { id: "dark".into(), name: "Dark".into() }],
                default_mode: "light".into(),
                variables: vec!["v_surface".into()],
                key: None,
                source_library_key: None,
                extensions: Default::default(),
            },
        );
        let light = Color::rgb(250, 250, 250);
        let dark = Color::rgb(10, 10, 10);
        d.variables.insert(
            "v_surface".into(),
            Variable {
                id: "v_surface".into(),
                name: "surface".into(),
                collection: "c1".into(),
                kind: VariableKind::Color,
                values: [("light".to_string(), VariableValue::Color(light)), ("dark".to_string(), VariableValue::Color(dark))].into_iter().collect(),
                description: String::new(),
                scopes: vec![],
                hidden_from_publishing: false,
                key: None,
                code_syntax: Default::default(),
                extensions: Default::default(),
                mode_extensions: Default::default(),
            },
        );
        let fid = d.alloc_id();
        let mut f = Node::frame(fid, Rect::new(0.0, 0.0, 200.0, 200.0));
        f.variables.modes.insert("c1".into(), "dark".into());
        let f = d.insert(page, f).unwrap();
        let rid = d.alloc_id();
        let mut r = Node::rectangle(rid, Rect::new(0.0, 0.0, 50.0, 50.0));
        r.fills = vec![Paint::solid(light)];
        r.stroke.paints = vec![Paint::solid(light)];
        r.style_refs.insert("fill".into(), "S".into());
        r.style_refs.insert("stroke".into(), "S".into());
        r.style_refs.insert("effect".into(), "Other".into());
        let r = d.insert(f, r).unwrap();
        d.styles.insert("S".into(), Style::Fill(vec![Paint::solid(light)]));
        d.style_meta.insert(
            "S".into(),
            StyleMeta {
                name: "S".into(),
                property: StyleProperty::Fill,
                bindings: [("fills/0".to_string(), "v_surface".to_string())].into_iter().collect(),
                ..Default::default()
            },
        );
        d.styles.insert("Other".into(), Style::Effect(vec![]));

        let shown_before = resolve_node(&d, d.get(r).unwrap()).into_owned();
        let mut h = History::default();
        let c = delete_style(&d, "S");
        run(&mut d, &mut h, c);

        let n = d.get(r).unwrap();
        assert_eq!(n.variables.bindings.get("fills/0").map(String::as_str), Some("v_surface"));
        assert_eq!(n.variables.bindings.get("strokes/0").map(String::as_str), Some("v_surface"));
        assert_eq!(n.style_refs.get("effect").map(String::as_str), Some("Other"));
        assert!(!n.style_refs.contains_key("fill") && !n.style_refs.contains_key("stroke"));

        let shown_after = resolve_node(&d, n).into_owned();
        assert_eq!(shown_after.fills, shown_before.fills);
        assert_eq!(shown_after.stroke.paints, shown_before.stroke.paints);

        // Switching the parent frame's mode to light changes both fill and stroke colour.
        d.get_mut(f).unwrap().variables.modes.insert("c1".into(), "light".into());
        let shown_light = resolve_node(&d, d.get(r).unwrap()).into_owned();
        assert_eq!(shown_light.fills, vec![Paint::solid(light)]);
        assert_eq!(shown_light.stroke.paints, vec![Paint::solid(light)]);
        d.get_mut(f).unwrap().variables.modes.insert("c1".into(), "dark".into());

        h.undo(&mut d).unwrap();
        let u = d.get(r).unwrap();
        assert_eq!((u.style_refs.get("fill").map(String::as_str), u.style_refs.get("stroke").map(String::as_str)), (Some("S"), Some("S")));
        assert!(u.variables.bindings.is_empty());
    }

    #[test]
    fn reset_instance_after_refused_delete_of_library_page_style_leaves_no_dangling_ref() {
        let mut d = Document::new("t");
        let page = d.add_page("p");
        let lp = ensure_library_page(&mut d);
        d.styles.insert("S".into(), Style::Effect(vec![Effect::LayerBlur { radius: 4.0, visible: true }]));
        d.style_meta.insert("S".into(), StyleMeta { name: "S".into(), property: StyleProperty::Effect, ..Default::default() });
        let cid = d.alloc_id();
        let mut comp = Node::frame(cid, Rect::new(0.0, 0.0, 100.0, 100.0));
        comp.kind = NodeKind::Component(FrameProps::default());
        comp.name = "Main".into();
        let comp = d.insert(lp, comp).unwrap();
        let kid = d.alloc_id();
        let mut child = Node::rectangle(kid, Rect::new(0.0, 0.0, 10.0, 10.0));
        child.name = "Child".into();
        child.effects = vec![Effect::LayerBlur { radius: 4.0, visible: true }];
        child.style_refs.insert("effect".into(), "S".into());
        d.insert(comp, child).unwrap();

        let mut h = History::default();
        let c = create_instance(&mut d, comp, page, Vec2::ZERO);
        h.exec(&mut d, c).unwrap();
        let inst = *d.get(page).unwrap().children.last().unwrap();

        let err = delete_style(&d, "S").unwrap_err();
        assert!(matches!(&err, StyleOpError::LibraryStyle(k) if k == "S"), "expected LibraryStyle error, got {err:?}");
        assert!(d.styles.contains_key("S"), "refused delete keeps S in doc.styles");

        let c = reset_instance(&mut d, inst);
        h.exec(&mut d, c).unwrap();
        let ichild = d.get(inst).unwrap().children[0];
        let refs = &d.get(ichild).unwrap().style_refs;
        assert_eq!(refs.get("effect").map(String::as_str), Some("S"));
        assert!(refs.values().all(|k| d.styles.contains_key(k)), "reset_instance leaves no dangling link: {refs:?}");
    }

    #[test]
    fn used_on_library_page_walks_only_the_library_page_subtree_and_probes_300k_nodes() {
        let mut d = Document::new("t");
        let p = d.add_page("Page 1");
        let lp = ensure_library_page(&mut d);

        let local_id = d.alloc_id();
        let mut local = Node::rectangle(local_id, Rect::new(0.0, 0.0, 10.0, 10.0));
        local.style_refs.insert("effect".into(), "LocalOnly".into());
        d.insert(p, local).unwrap();

        let comp_id = d.alloc_id();
        let mut comp = Node::frame(comp_id, Rect::new(0.0, 0.0, 20.0, 20.0));
        comp.kind = NodeKind::Component(FrameProps::default());
        let comp = d.insert(lp, comp).unwrap();
        let nested_id = d.alloc_id();
        let mut nested = Node::rectangle(nested_id, Rect::new(0.0, 0.0, 10.0, 10.0));
        nested.style_refs.insert("effect".into(), "LibUsed".into());
        d.insert(comp, nested).unwrap();

        // An unlinked node in doc.nodes that is NOT reachable from lp.children
        // must not be visited when walking only the doc.library_page subtree.
        let orphan_id = d.alloc_id();
        let mut orphan = Node::rectangle(orphan_id, Rect::new(0.0, 0.0, 10.0, 10.0));
        orphan.parent = Some(lp);
        orphan.style_refs.insert("effect".into(), "OrphanStyle".into());
        d.nodes.insert(orphan_id, orphan);

        assert!(used_on_library_page(&d, "LibUsed"));
        assert!(!used_on_library_page(&d, "LocalOnly"));
        assert!(!used_on_library_page(&d, "OrphanStyle"), "used_on_library_page must walk only the doc.library_page subtree, not doc.nodes.values()");

        // Synthetic 302,000-node document with a 2,000-node library page (probe, not a CI timing assertion).
        let mut big = Document::new("perf-probe");
        let big_p = big.add_page("Main");
        let big_lp = ensure_library_page(&mut big);
        for _ in 0..300_000 {
            let id = big.alloc_id();
            let mut n = Node::rectangle(id, Rect::new(0.0, 0.0, 10.0, 10.0));
            n.parent = Some(big_p);
            big.nodes.insert(id, n);
        }
        let mut lp_children = Vec::with_capacity(2_000);
        for i in 0..2_000 {
            let id = big.alloc_id();
            let mut n = Node::rectangle(id, Rect::new(0.0, 0.0, 10.0, 10.0));
            n.parent = Some(big_lp);
            if i == 1_999 {
                n.style_refs.insert("effect".into(), "Target".into());
            }
            big.nodes.insert(id, n);
            lp_children.push(id);
        }
        big.get_mut(big_lp).unwrap().children = lp_children;

        let full_walk = |doc: &Document, key: &str| -> bool {
            doc.library_page.is_some()
                && doc.nodes.values().any(|n| {
                    let mut cur = Some(n.id);
                    let mut on_lp = false;
                    while let Some(id) = cur {
                        if doc.library_page == Some(id) {
                            on_lp = true;
                            break;
                        }
                        cur = doc.get(id).and_then(|node| node.parent);
                    }
                    on_lp && n.style_refs.values().any(|k| k == key)
                })
        };

        let iters = 20u32;
        let t_sub = std::time::Instant::now();
        for _ in 0..iters {
            assert!(!used_on_library_page(&big, "Missing"));
            assert!(used_on_library_page(&big, "Target"));
        }
        let sub_per_call = t_sub.elapsed() / (iters * 2);

        let t_full = std::time::Instant::now();
        for _ in 0..iters {
            assert!(!full_walk(&big, "Missing"));
            assert!(full_walk(&big, "Target"));
        }
        let full_per_call = t_full.elapsed() / (iters * 2);
        eprintln!("[perf-probe] 300k main nodes + 2k library_page nodes: subtree={sub_per_call:?}/call vs full_walk={full_per_call:?}/call");
    }

    #[test]
    fn deleting_a_style_with_mixed_own_and_style_bindings_preserves_resolved_look_in_all_modes() {
        let mut d = Document::new("t");
        let p = d.add_page("p");
        d.variable_collections.insert(
            "c".into(),
            VariableCollection {
                id: "c".into(),
                name: "Theme".into(),
                modes: vec![VariableMode { id: "light".into(), name: "Light".into() }, VariableMode { id: "dark".into(), name: "Dark".into() }],
                default_mode: "light".into(),
                variables: vec!["v_style".into(), "v_weight".into(), "v_fill".into(), "v_op".into()],
                key: None,
                source_library_key: None,
                extensions: Default::default(),
            },
        );
        d.variables.insert(
            "v_style".into(),
            Variable {
                id: "v_style".into(),
                name: "fontStyle".into(),
                collection: "c".into(),
                kind: VariableKind::String,
                values: [
                    ("light".to_string(), VariableValue::String("Bold Italic".into())),
                    ("dark".to_string(), VariableValue::String("SemiBold Italic".into())),
                ]
                .into_iter()
                .collect(),
                description: String::new(),
                scopes: vec![],
                hidden_from_publishing: false,
                key: None,
                code_syntax: Default::default(),
                extensions: Default::default(),
                mode_extensions: Default::default(),
            },
        );
        d.variables.insert(
            "v_weight".into(),
            Variable {
                id: "v_weight".into(),
                name: "fontWeight".into(),
                collection: "c".into(),
                kind: VariableKind::Number,
                values: [("light".to_string(), VariableValue::Number(300.0)), ("dark".to_string(), VariableValue::Number(400.0))].into_iter().collect(),
                description: String::new(),
                scopes: vec![],
                hidden_from_publishing: false,
                key: None,
                code_syntax: Default::default(),
                extensions: Default::default(),
                mode_extensions: Default::default(),
            },
        );
        d.variables.insert(
            "v_fill".into(),
            Variable {
                id: "v_fill".into(),
                name: "fillColor".into(),
                collection: "c".into(),
                kind: VariableKind::Color,
                values: [
                    ("light".to_string(), VariableValue::Color(Color::rgba(200, 100, 50, 128))),
                    ("dark".to_string(), VariableValue::Color(Color::rgba(40, 80, 160, 192))),
                ]
                .into_iter()
                .collect(),
                description: String::new(),
                scopes: vec![],
                hidden_from_publishing: false,
                key: None,
                code_syntax: Default::default(),
                extensions: Default::default(),
                mode_extensions: Default::default(),
            },
        );
        d.variables.insert(
            "v_op".into(),
            Variable {
                id: "v_op".into(),
                name: "fillOpacity".into(),
                collection: "c".into(),
                kind: VariableKind::Number,
                values: [("light".to_string(), VariableValue::Number(0.25)), ("dark".to_string(), VariableValue::Number(0.75))].into_iter().collect(),
                description: String::new(),
                scopes: vec![],
                hidden_from_publishing: false,
                key: None,
                code_syntax: Default::default(),
                extensions: Default::default(),
                mode_extensions: Default::default(),
            },
        );

        let tid = d.alloc_id();
        let mut t = Node::text(tid, Rect::new(0.0, 0.0, 100.0, 30.0), "Hello", TextStyle::default());
        t.style_refs.insert("text".into(), "Heading".into());
        t.variables.bindings.insert("fontStyle".into(), "v_style".into());
        let tid = d.insert(p, t).unwrap();

        let rid = d.alloc_id();
        let mut r = Node::rectangle(rid, Rect::new(0.0, 40.0, 100.0, 40.0));
        r.fills = vec![Paint::solid(Color::BLACK)];
        r.style_refs.insert("fill".into(), "Surface".into());
        r.variables.bindings.insert("fills/0".into(), "v_fill".into());
        let rid = d.insert(p, r).unwrap();

        d.styles.insert("Heading".into(), Style::Text(TextStyle::default()));
        d.style_meta.insert(
            "Heading".into(),
            StyleMeta {
                name: "Heading".into(),
                property: StyleProperty::Text,
                bindings: [("fontWeight".to_string(), "v_weight".to_string())].into_iter().collect(),
                ..Default::default()
            },
        );
        d.styles.insert("Surface".into(), Style::Fill(vec![Paint::solid(Color::BLACK)]));
        d.style_meta.insert(
            "Surface".into(),
            StyleMeta {
                name: "Surface".into(),
                property: StyleProperty::Fill,
                bindings: [("fills/0/opacity".to_string(), "v_op".to_string())].into_iter().collect(),
                ..Default::default()
            },
        );

        let mut text_before_modes = Vec::new();
        let mut rect_before_modes = Vec::new();
        for mode in ["light", "dark"] {
            d.get_mut(tid).unwrap().variables.modes.insert("c".into(), mode.into());
            let mut rt = resolve_node(&d, d.get(tid).unwrap()).into_owned();
            rt.style_refs.clear();
            rt.variables = Default::default();
            text_before_modes.push((mode, rt));

            d.get_mut(rid).unwrap().variables.modes.insert("c".into(), mode.into());
            let mut rr = resolve_node(&d, d.get(rid).unwrap()).into_owned();
            rr.style_refs.clear();
            rr.variables = Default::default();
            rect_before_modes.push((mode, rr));
        }
        d.get_mut(tid).unwrap().variables.modes.insert("c".into(), "light".into());
        d.get_mut(rid).unwrap().variables.modes.insert("c".into(), "light".into());

        let mut h = History::default();
        let c1 = delete_style(&d, "Heading");
        run(&mut d, &mut h, c1);
        let c2 = delete_style(&d, "Surface");
        run(&mut d, &mut h, c2);

        for (mode, expected) in &text_before_modes {
            d.get_mut(tid).unwrap().variables.modes.insert("c".into(), (*mode).into());
            let mut actual = resolve_node(&d, d.get(tid).unwrap()).into_owned();
            actual.variables = Default::default();
            assert_eq!(actual, *expected, "text resolved look in mode {mode} must match before delete");
        }
        for (mode, expected) in &rect_before_modes {
            d.get_mut(rid).unwrap().variables.modes.insert("c".into(), (*mode).into());
            let mut actual = resolve_node(&d, d.get(rid).unwrap()).into_owned();
            actual.variables = Default::default();
            assert_eq!(actual, *expected, "rect resolved look in mode {mode} must match before delete");
        }
    }

    #[test]
    fn deleting_a_style_bakes_unrecognized_binding_keys_instead_of_dropping_them() {
        let mut d = Document::new("t");
        let p = d.add_page("p");
        d.variable_collections.insert(
            "c".into(),
            VariableCollection {
                id: "c".into(),
                name: "C".into(),
                modes: vec![VariableMode { id: "m1".into(), name: "M1".into() }],
                default_mode: "m1".into(),
                variables: vec!["v_sz".into(), "v_col".into(), "v_dx".into(), "v_rad".into(), "v_minw".into()],
                key: None,
                source_library_key: None,
                extensions: Default::default(),
            },
        );
        for (id, kind, val) in [
            ("v_sz", VariableKind::Number, VariableValue::Number(32.0)),
            ("v_col", VariableKind::Color, VariableValue::Color(Color::rgb(12, 34, 56))),
            ("v_dx", VariableKind::Number, VariableValue::Number(14.0)),
            ("v_rad", VariableKind::Number, VariableValue::Number(9.0)),
            ("v_minw", VariableKind::Number, VariableValue::Number(120.0)),
        ] {
            d.variables.insert(
                id.into(),
                Variable {
                    id: id.into(),
                    name: id.into(),
                    collection: "c".into(),
                    kind,
                    values: [("m1".to_string(), val)].into_iter().collect(),
                    description: String::new(),
                    scopes: vec![],
                    hidden_from_publishing: false,
                    key: None,
                    code_syntax: Default::default(),
                    extensions: Default::default(),
                    mode_extensions: Default::default(),
                },
            );
        }

        let tid = d.alloc_id();
        let mut t = Node::text(tid, Rect::new(0.0, 0.0, 100.0, 30.0), "Hi", TextStyle { font_size: 12.0, ..Default::default() });
        t.style_refs.insert("text".into(), "Txt".into());
        let tid = d.insert(p, t).unwrap();

        let rid = d.alloc_id();
        let mut r = Node::rectangle(rid, Rect::new(0.0, 40.0, 100.0, 50.0));
        r.fills = vec![Paint::solid(Color::WHITE)];
        r.effects = vec![shadow(4.0)];
        r.style_refs.insert("fill".into(), "FillS".into());
        r.style_refs.insert("effect".into(), "FxS".into());
        let rid = d.insert(p, r).unwrap();

        d.styles.insert("Txt".into(), Style::Text(TextStyle { font_size: 12.0, ..Default::default() }));
        d.style_meta.insert(
            "Txt".into(),
            StyleMeta {
                name: "Txt".into(),
                property: StyleProperty::Text,
                bindings: [("fontSize/0".to_string(), "v_sz".to_string())].into_iter().collect(),
                ..Default::default()
            },
        );
        d.styles.insert("FillS".into(), Style::Fill(vec![Paint::solid(Color::WHITE)]));
        d.style_meta.insert(
            "FillS".into(),
            StyleMeta {
                name: "FillS".into(),
                property: StyleProperty::Fill,
                bindings: [
                    ("fills/0/color".to_string(), "v_col".to_string()),
                    ("cornerRadius".to_string(), "v_rad".to_string()),
                    ("minWidth".to_string(), "v_minw".to_string()),
                ]
                .into_iter()
                .collect(),
                ..Default::default()
            },
        );
        d.styles.insert("FxS".into(), Style::Effect(vec![shadow(4.0)]));
        d.style_meta.insert(
            "FxS".into(),
            StyleMeta {
                name: "FxS".into(),
                property: StyleProperty::Effect,
                bindings: [("effects/0/x".to_string(), "v_dx".to_string())].into_iter().collect(),
                ..Default::default()
            },
        );

        let mut h = History::default();
        for k in ["Txt", "FillS", "FxS"] {
            let c = delete_style(&d, k);
            run(&mut d, &mut h, c);
        }

        match &d.get(tid).unwrap().kind {
            NodeKind::Text(tp) => assert_eq!(tp.style.font_size, 32.0, "fontSize/0 must bake onto text style"),
            other => panic!("expected text, got {other:?}"),
        }
        let rn = d.get(rid).unwrap();
        assert_eq!(rn.fills, vec![Paint::solid(Color::rgb(12, 34, 56))], "fills/0/color must bake onto fills[0]");
        assert_eq!(rn.corner_radii, [9.0; 4], "cornerRadius must bake onto corner_radii");
        assert_eq!(rn.layout_child.min_w, Some(120.0), "minWidth must bake onto layout_child.min_w");
        match &rn.effects[0] {
            Effect::DropShadow { offset, .. } => assert_eq!(offset.x, 14.0, "effects/0/x must bake onto shadow offset.x"),
            other => panic!("expected drop shadow, got {other:?}"),
        }
    }

    fn make_var(id: &str, kind: VariableKind, light: VariableValue, dark: VariableValue) -> Variable {
        Variable {
            id: id.into(),
            name: id.into(),
            collection: "c".into(),
            kind,
            values: [("light".to_string(), light), ("dark".to_string(), dark)].into_iter().collect(),
            description: String::new(),
            scopes: vec![],
            hidden_from_publishing: false,
            key: None,
            code_syntax: Default::default(),
            extensions: Default::default(),
            mode_extensions: Default::default(),
        }
    }

    fn assert_resolved_matches_across_modes(before_doc: &Document, after_doc: &Document, id: NodeId, context: &str) {
        for mode in ["light", "dark"] {
            let mut bdoc = before_doc.clone();
            bdoc.get_mut(id).unwrap().variables.modes.insert("c".into(), mode.into());
            let mut expected = resolve_node(&bdoc, bdoc.get(id).unwrap()).into_owned();
            expected.style_refs.clear();
            expected.variables = Default::default();

            let mut adoc = after_doc.clone();
            adoc.get_mut(id).unwrap().variables.modes.insert("c".into(), mode.into());
            let mut actual = resolve_node(&adoc, adoc.get(id).unwrap()).into_owned();
            actual.style_refs.clear();
            actual.variables = Default::default();

            assert_eq!(actual, expected, "{context}: resolve_node mismatch in mode '{mode}'");
        }
    }

    #[test]
    fn deleting_in_dark_with_mixed_mode_overrides_preserves_resolved_node_in_light_and_dark() {
        let mut d = Document::new("t");
        let p = d.add_page("p");
        d.variable_collections.insert(
            "c".into(),
            VariableCollection {
                id: "c".into(),
                name: "Theme".into(),
                modes: vec![VariableMode { id: "light".into(), name: "Light".into() }, VariableMode { id: "dark".into(), name: "Dark".into() }],
                default_mode: "light".into(),
                variables: vec!["v_style".into(), "v_weight".into(), "v_fill".into(), "v_op".into(), "v_style_italic".into(), "v_fill_opaque".into()],
                key: None,
                source_library_key: None,
                extensions: Default::default(),
            },
        );
        d.variables.insert(
            "v_style".into(),
            make_var("v_style", VariableKind::String, VariableValue::String("Italic".into()), VariableValue::String("Bold Italic".into())),
        );
        d.variables.insert("v_weight".into(), make_var("v_weight", VariableKind::Number, VariableValue::Number(300.0), VariableValue::Number(800.0)));
        d.variables.insert(
            "v_fill".into(),
            make_var("v_fill", VariableKind::Color, VariableValue::Color(Color::rgba(200, 100, 50, 255)), VariableValue::Color(Color::rgba(40, 80, 160, 128))),
        );
        d.variables.insert("v_op".into(), make_var("v_op", VariableKind::Number, VariableValue::Number(50.0), VariableValue::Number(25.0)));
        d.variables.insert(
            "v_style_italic".into(),
            make_var("v_style_italic", VariableKind::String, VariableValue::String("Italic".into()), VariableValue::String("Oblique".into())),
        );
        d.variables.insert(
            "v_fill_opaque".into(),
            make_var(
                "v_fill_opaque",
                VariableKind::Color,
                VariableValue::Color(Color::rgba(200, 100, 50, 255)),
                VariableValue::Color(Color::rgba(40, 80, 160, 255)),
            ),
        );

        let tid = d.alloc_id();
        let mut t = Node::text(tid, Rect::new(0.0, 0.0, 100.0, 30.0), "Hello", TextStyle::default());
        t.style_refs.insert("text".into(), "Heading".into());
        t.variables.bindings.insert("fontStyle".into(), "v_style".into());
        t.variables.modes.insert("c".into(), "dark".into());
        let tid = d.insert(p, t).unwrap();

        let rid = d.alloc_id();
        let mut r = Node::rectangle(rid, Rect::new(0.0, 40.0, 100.0, 40.0));
        r.fills = vec![Paint::solid(Color::BLACK)];
        r.style_refs.insert("fill".into(), "Surface".into());
        r.variables.bindings.insert("fills/0".into(), "v_fill".into());
        r.variables.modes.insert("c".into(), "dark".into());
        let rid = d.insert(p, r).unwrap();

        let tid_keep = d.alloc_id();
        let mut t_keep = Node::text(tid_keep, Rect::new(0.0, 80.0, 100.0, 30.0), "Keep", TextStyle::default());
        t_keep.style_refs.insert("text".into(), "Heading".into());
        t_keep.variables.bindings.insert("fontStyle".into(), "v_style_italic".into());
        t_keep.variables.modes.insert("c".into(), "dark".into());
        let tid_keep = d.insert(p, t_keep).unwrap();

        let rid_keep = d.alloc_id();
        let mut r_keep = Node::rectangle(rid_keep, Rect::new(0.0, 120.0, 100.0, 40.0));
        r_keep.fills = vec![Paint::solid(Color::BLACK)];
        r_keep.style_refs.insert("fill".into(), "Surface".into());
        r_keep.variables.bindings.insert("fills/0".into(), "v_fill_opaque".into());
        r_keep.variables.modes.insert("c".into(), "dark".into());
        let rid_keep = d.insert(p, r_keep).unwrap();

        d.styles.insert("Heading".into(), Style::Text(TextStyle::default()));
        d.style_meta.insert(
            "Heading".into(),
            StyleMeta {
                name: "Heading".into(),
                property: StyleProperty::Text,
                bindings: [("fontWeight".to_string(), "v_weight".to_string())].into_iter().collect(),
                ..Default::default()
            },
        );
        d.styles.insert("Surface".into(), Style::Fill(vec![Paint::solid(Color::BLACK)]));
        d.style_meta.insert(
            "Surface".into(),
            StyleMeta {
                name: "Surface".into(),
                property: StyleProperty::Fill,
                bindings: [("fills/0/opacity".to_string(), "v_op".to_string())].into_iter().collect(),
                ..Default::default()
            },
        );

        let before = d.clone();
        let mut h = History::default();
        let c1 = delete_style(&d, "Heading");
        run(&mut d, &mut h, c1);
        let c2 = delete_style(&d, "Surface");
        run(&mut d, &mut h, c2);

        assert_resolved_matches_across_modes(&before, &d, tid, "mixed fontStyle/fontWeight deleted in dark");
        assert_resolved_matches_across_modes(&before, &d, rid, "mixed fills/0 + fills/0/opacity deleted in dark");
        assert_resolved_matches_across_modes(&before, &d, tid_keep, "non-overriding fontStyle/fontWeight deleted in dark");
        assert_resolved_matches_across_modes(&before, &d, rid_keep, "non-overriding fills/0 + fills/0/opacity deleted in dark");
        assert_eq!(d.get(tid_keep).unwrap().variables.bindings.get("fontWeight").map(String::as_str), Some("v_weight"));
        assert_eq!(d.get(rid_keep).unwrap().variables.bindings.get("fills/0/opacity").map(String::as_str), Some("v_op"));
    }

    #[test]
    fn deleting_a_stroke_style_skips_non_fills_prefixed_binding_keys_matching_resolve_node() {
        let mut d = Document::new("t");
        let p = d.add_page("p");
        d.variable_collections.insert(
            "c".into(),
            VariableCollection {
                id: "c".into(),
                name: "Theme".into(),
                modes: vec![VariableMode { id: "light".into(), name: "Light".into() }, VariableMode { id: "dark".into(), name: "Dark".into() }],
                default_mode: "light".into(),
                variables: vec!["v_green".into(), "v_blue".into()],
                key: None,
                source_library_key: None,
                extensions: Default::default(),
            },
        );
        d.variables.insert(
            "v_green".into(),
            make_var("v_green", VariableKind::Color, VariableValue::Color(Color::rgb(0, 200, 0)), VariableValue::Color(Color::rgb(0, 120, 0))),
        );
        d.variables.insert(
            "v_blue".into(),
            make_var("v_blue", VariableKind::Color, VariableValue::Color(Color::rgb(0, 0, 200)), VariableValue::Color(Color::rgb(0, 0, 120))),
        );

        let rid = d.alloc_id();
        let mut r = Node::rectangle(rid, Rect::new(0.0, 0.0, 80.0, 40.0));
        r.stroke.paints = vec![Paint::solid(Color::BLACK)];
        r.style_refs.insert("stroke".into(), "Border".into());
        let rid = d.insert(p, r).unwrap();

        d.styles.insert("Border".into(), Style::Fill(vec![Paint::solid(Color::BLACK)]));
        d.style_meta.insert(
            "Border".into(),
            StyleMeta {
                name: "Border".into(),
                property: StyleProperty::Fill,
                bindings: [("strokes/0".to_string(), "v_green".to_string())].into_iter().collect(),
                ..Default::default()
            },
        );

        let before = d.clone();
        let mut h = History::default();
        let c = delete_style(&d, "Border");
        run(&mut d, &mut h, c);

        assert!(!d.get(rid).unwrap().variables.bindings.contains_key("strokes/0"), "strokes/0 on a stroke-slot style must not be moved onto the layer");
        assert_resolved_matches_across_modes(&before, &d, rid, "stroke-slot style with non-fills/ key");
    }

    #[test]
    fn table_test_delete_style_in_each_mode_matches_resolve_node_in_every_mode_for_all_linked_key_types() {
        #[derive(Clone)]
        struct Case {
            name: &'static str,
            slot: &'static str,
            style_prop: StyleProperty,
            style_val: Style,
            style_bindings: Vec<(&'static str, &'static str)>,
            own_bindings: Vec<(&'static str, &'static str)>,
        }

        let gradient_paint = Paint {
            kind: PaintKind::LinearGradient {
                handles: [Vec2::ZERO, Vec2::new(1.0, 0.0), Vec2::new(0.0, 1.0)],
                stops: vec![GradientStop { pos: 0.0, color: Color::BLACK }, GradientStop { pos: 1.0, color: Color::WHITE }],
            },
            opacity: 1.0,
            visible: true,
            blend: BlendMode::Normal,
        };

        let cases = vec![
            Case {
                name: "fill_color",
                slot: "fill",
                style_prop: StyleProperty::Fill,
                style_val: Style::Fill(vec![Paint::solid(Color::BLACK)]),
                style_bindings: vec![("fills/0", "v_col_opaque")],
                own_bindings: vec![],
            },
            Case {
                name: "fill_opacity_no_own",
                slot: "fill",
                style_prop: StyleProperty::Fill,
                style_val: Style::Fill(vec![Paint::solid(Color::BLACK)]),
                style_bindings: vec![("fills/0/opacity", "v_op")],
                own_bindings: vec![],
            },
            Case {
                name: "fill_opacity_with_opaque_own_fill",
                slot: "fill",
                style_prop: StyleProperty::Fill,
                style_val: Style::Fill(vec![Paint::solid(Color::BLACK)]),
                style_bindings: vec![("fills/0/opacity", "v_op")],
                own_bindings: vec![("fills/0", "v_col_opaque")],
            },
            Case {
                name: "fill_opacity_with_translucent_own_fill_all_modes",
                slot: "fill",
                style_prop: StyleProperty::Fill,
                style_val: Style::Fill(vec![Paint::solid(Color::BLACK)]),
                style_bindings: vec![("fills/0/opacity", "v_op")],
                own_bindings: vec![("fills/0", "v_col_trans")],
            },
            Case {
                name: "fill_opacity_with_mixed_opaque_light_translucent_dark_own_fill",
                slot: "fill",
                style_prop: StyleProperty::Fill,
                style_val: Style::Fill(vec![Paint::solid(Color::BLACK)]),
                style_bindings: vec![("fills/0/opacity", "v_op")],
                own_bindings: vec![("fills/0", "v_col_light_opaque_dark_trans")],
            },
            Case {
                name: "fill_opacity_with_mixed_translucent_light_opaque_dark_own_fill",
                slot: "fill",
                style_prop: StyleProperty::Fill,
                style_val: Style::Fill(vec![Paint::solid(Color::BLACK)]),
                style_bindings: vec![("fills/0/opacity", "v_op")],
                own_bindings: vec![("fills/0", "v_col_light_trans_dark_opaque")],
            },
            Case {
                name: "fill_stop_no_own",
                slot: "fill",
                style_prop: StyleProperty::Fill,
                style_val: Style::Fill(vec![gradient_paint.clone()]),
                style_bindings: vec![("fills/0/stops/0", "v_col_opaque")],
                own_bindings: vec![],
            },
            Case {
                name: "fill_stop_overridden_by_own_fill",
                slot: "fill",
                style_prop: StyleProperty::Fill,
                style_val: Style::Fill(vec![gradient_paint.clone()]),
                style_bindings: vec![("fills/0/stops/0", "v_col_opaque")],
                own_bindings: vec![("fills/0", "v_col_trans")],
            },
            Case {
                name: "stroke_color_from_fills_prefix",
                slot: "stroke",
                style_prop: StyleProperty::Fill,
                style_val: Style::Fill(vec![Paint::solid(Color::BLACK)]),
                style_bindings: vec![("fills/0", "v_col_opaque")],
                own_bindings: vec![],
            },
            Case {
                name: "stroke_opacity_with_opaque_own_stroke",
                slot: "stroke",
                style_prop: StyleProperty::Fill,
                style_val: Style::Fill(vec![Paint::solid(Color::BLACK)]),
                style_bindings: vec![("fills/0/opacity", "v_op")],
                own_bindings: vec![("strokes/0", "v_col_opaque")],
            },
            Case {
                name: "stroke_opacity_with_translucent_own_stroke_all_modes",
                slot: "stroke",
                style_prop: StyleProperty::Fill,
                style_val: Style::Fill(vec![Paint::solid(Color::BLACK)]),
                style_bindings: vec![("fills/0/opacity", "v_op")],
                own_bindings: vec![("strokes/0", "v_col_trans")],
            },
            Case {
                name: "stroke_opacity_with_mixed_opaque_light_translucent_dark_own_stroke",
                slot: "stroke",
                style_prop: StyleProperty::Fill,
                style_val: Style::Fill(vec![Paint::solid(Color::BLACK)]),
                style_bindings: vec![("fills/0/opacity", "v_op")],
                own_bindings: vec![("strokes/0", "v_col_light_opaque_dark_trans")],
            },
            Case {
                name: "stroke_opacity_with_mixed_translucent_light_opaque_dark_own_stroke",
                slot: "stroke",
                style_prop: StyleProperty::Fill,
                style_val: Style::Fill(vec![Paint::solid(Color::BLACK)]),
                style_bindings: vec![("fills/0/opacity", "v_op")],
                own_bindings: vec![("strokes/0", "v_col_light_trans_dark_opaque")],
            },
            Case {
                name: "stroke_stop_overridden_by_own_stroke",
                slot: "stroke",
                style_prop: StyleProperty::Fill,
                style_val: Style::Fill(vec![gradient_paint.clone()]),
                style_bindings: vec![("fills/0/stops/0", "v_col_opaque")],
                own_bindings: vec![("strokes/0", "v_col_opaque")],
            },
            Case {
                name: "stroke_non_fills_prefix_skipped",
                slot: "stroke",
                style_prop: StyleProperty::Fill,
                style_val: Style::Fill(vec![Paint::solid(Color::BLACK)]),
                style_bindings: vec![("strokes/0", "v_col_opaque"), ("strokes/0/opacity", "v_op")],
                own_bindings: vec![],
            },
            Case {
                name: "text_fields_no_own",
                slot: "text",
                style_prop: StyleProperty::Text,
                style_val: Style::Text(TextStyle::default()),
                style_bindings: vec![
                    ("fontSize", "v_num_a"),
                    ("lineHeight", "v_num_b"),
                    ("letterSpacing", "v_num_a"),
                    ("paragraphSpacing", "v_num_b"),
                    ("paragraphIndent", "v_num_a"),
                    ("fontFamily", "v_fam"),
                ],
                own_bindings: vec![],
            },
            Case {
                name: "text_weight_with_non_weight_own_font_style",
                slot: "text",
                style_prop: StyleProperty::Text,
                style_val: Style::Text(TextStyle::default()),
                style_bindings: vec![("fontWeight", "v_weight")],
                own_bindings: vec![("fontStyle", "v_style_italic_only")],
            },
            Case {
                name: "text_weight_with_all_weight_own_font_style",
                slot: "text",
                style_prop: StyleProperty::Text,
                style_val: Style::Text(TextStyle::default()),
                style_bindings: vec![("fontWeight", "v_weight")],
                own_bindings: vec![("fontStyle", "v_style_both_weighted")],
            },
            Case {
                name: "text_weight_with_mixed_italic_light_bold_dark_own_font_style",
                slot: "text",
                style_prop: StyleProperty::Text,
                style_val: Style::Text(TextStyle::default()),
                style_bindings: vec![("fontWeight", "v_weight")],
                own_bindings: vec![("fontStyle", "v_style_light_italic_dark_bold")],
            },
            Case {
                name: "text_weight_with_mixed_light_weight_dark_italic_own_font_style",
                slot: "text",
                style_prop: StyleProperty::Text,
                style_val: Style::Text(TextStyle::default()),
                style_bindings: vec![("fontWeight", "v_weight")],
                own_bindings: vec![("fontStyle", "v_style_light_bold_dark_italic")],
            },
            Case {
                name: "text_style_font_style_with_own_font_weight",
                slot: "text",
                style_prop: StyleProperty::Text,
                style_val: Style::Text(TextStyle::default()),
                style_bindings: vec![("fontStyle", "v_style_light_italic_dark_bold")],
                own_bindings: vec![("fontWeight", "v_weight")],
            },
            Case {
                name: "effect_fields_and_opacity_no_own_color",
                slot: "effect",
                style_prop: StyleProperty::Effect,
                style_val: Style::Effect(vec![shadow(4.0)]),
                style_bindings: vec![
                    ("effects/0/opacity", "v_op"),
                    ("effects/0/radius", "v_num_a"),
                    ("effects/0/spread", "v_num_b"),
                    ("effects/0/offsetX", "v_num_a"),
                    ("effects/0/offsetY", "v_num_b"),
                ],
                own_bindings: vec![],
            },
            Case {
                name: "effect_opacity_overridden_by_own_effect_color",
                slot: "effect",
                style_prop: StyleProperty::Effect,
                style_val: Style::Effect(vec![shadow(4.0)]),
                style_bindings: vec![("effects/0/opacity", "v_op")],
                own_bindings: vec![("effects/0/color", "v_col_opaque")],
            },
        ];

        for case in &cases {
            for active_mode in ["light", "dark"] {
                let mut d = Document::new("t");
                let p = d.add_page("p");
                let vars = vec![
                    make_var(
                        "v_col_opaque",
                        VariableKind::Color,
                        VariableValue::Color(Color::rgba(210, 40, 40, 255)),
                        VariableValue::Color(Color::rgba(40, 40, 210, 255)),
                    ),
                    make_var(
                        "v_col_trans",
                        VariableKind::Color,
                        VariableValue::Color(Color::rgba(210, 40, 40, 120)),
                        VariableValue::Color(Color::rgba(40, 40, 210, 180)),
                    ),
                    make_var(
                        "v_col_light_opaque_dark_trans",
                        VariableKind::Color,
                        VariableValue::Color(Color::rgba(210, 40, 40, 255)),
                        VariableValue::Color(Color::rgba(40, 40, 210, 140)),
                    ),
                    make_var(
                        "v_col_light_trans_dark_opaque",
                        VariableKind::Color,
                        VariableValue::Color(Color::rgba(210, 40, 40, 140)),
                        VariableValue::Color(Color::rgba(40, 40, 210, 255)),
                    ),
                    make_var("v_op", VariableKind::Number, VariableValue::Number(50.0), VariableValue::Number(25.0)),
                    make_var("v_num_a", VariableKind::Number, VariableValue::Number(14.0), VariableValue::Number(22.0)),
                    make_var("v_num_b", VariableKind::Number, VariableValue::Number(18.0), VariableValue::Number(28.0)),
                    make_var("v_fam", VariableKind::String, VariableValue::String("Inter".into()), VariableValue::String("Roboto".into())),
                    make_var("v_weight", VariableKind::Number, VariableValue::Number(300.0), VariableValue::Number(800.0)),
                    make_var("v_style_italic_only", VariableKind::String, VariableValue::String("Italic".into()), VariableValue::String("Oblique".into())),
                    make_var(
                        "v_style_both_weighted",
                        VariableKind::String,
                        VariableValue::String("Bold Italic".into()),
                        VariableValue::String("SemiBold Italic".into()),
                    ),
                    make_var(
                        "v_style_light_italic_dark_bold",
                        VariableKind::String,
                        VariableValue::String("Italic".into()),
                        VariableValue::String("Bold Italic".into()),
                    ),
                    make_var(
                        "v_style_light_bold_dark_italic",
                        VariableKind::String,
                        VariableValue::String("Bold Italic".into()),
                        VariableValue::String("Italic".into()),
                    ),
                ];
                let var_ids = vars.iter().map(|v| v.id.clone()).collect();
                d.variable_collections.insert(
                    "c".into(),
                    VariableCollection {
                        id: "c".into(),
                        name: "Theme".into(),
                        modes: vec![VariableMode { id: "light".into(), name: "Light".into() }, VariableMode { id: "dark".into(), name: "Dark".into() }],
                        default_mode: "light".into(),
                        variables: var_ids,
                        key: None,
                        source_library_key: None,
                        extensions: Default::default(),
                    },
                );
                for v in vars {
                    d.variables.insert(v.id.clone(), v);
                }

                let nid = d.alloc_id();
                let mut n = if case.slot == "text" {
                    Node::text(nid, Rect::new(0.0, 0.0, 100.0, 30.0), "Sample", TextStyle::default())
                } else {
                    let mut r = Node::rectangle(nid, Rect::new(0.0, 0.0, 100.0, 40.0));
                    r.fills = vec![if case.name.starts_with("fill_stop") { gradient_paint.clone() } else { Paint::solid(Color::BLACK) }];
                    r.stroke.paints = vec![if case.name.starts_with("stroke_stop") { gradient_paint.clone() } else { Paint::solid(Color::BLACK) }];
                    r.effects = vec![shadow(4.0)];
                    r
                };
                n.style_refs.insert(case.slot.into(), "S".into());
                for (k, v) in &case.own_bindings {
                    n.variables.bindings.insert((*k).into(), (*v).into());
                }
                n.variables.modes.insert("c".into(), active_mode.into());
                let nid = d.insert(p, n).unwrap();

                d.styles.insert("S".into(), case.style_val.clone());
                d.style_meta.insert(
                    "S".into(),
                    StyleMeta {
                        name: "S".into(),
                        property: case.style_prop,
                        bindings: case.style_bindings.iter().map(|(k, v)| ((*k).to_string(), (*v).to_string())).collect(),
                        ..Default::default()
                    },
                );

                let before = d.clone();
                let mut h = History::default();
                let c = delete_style(&d, "S");
                run(&mut d, &mut h, c);

                assert_resolved_matches_across_modes(&before, &d, nid, &format!("case={} deleted_in={active_mode}", case.name));
            }
        }
    }

    fn assert_resolved_matches_across_mode_combos(before_doc: &Document, after_doc: &Document, id: NodeId, combos: &[&[(&str, &str)]], context: &str) {
        for combo in combos {
            let mut bdoc = before_doc.clone();
            for (cid, mid) in *combo {
                bdoc.get_mut(id).unwrap().variables.modes.insert((*cid).into(), (*mid).into());
            }
            let mut expected = resolve_node(&bdoc, bdoc.get(id).unwrap()).into_owned();
            expected.style_refs.clear();
            expected.variables = Default::default();

            let mut adoc = after_doc.clone();
            for (cid, mid) in *combo {
                adoc.get_mut(id).unwrap().variables.modes.insert((*cid).into(), (*mid).into());
            }
            let mut actual = resolve_node(&adoc, adoc.get(id).unwrap()).into_owned();
            actual.style_refs.clear();
            actual.variables = Default::default();

            assert_eq!(actual, expected, "{context}: resolve_node mismatch in mode combo {combo:?}");
        }
    }

    #[test]
    fn r5_row1_three_modes_mixed_override_uses_fallback_preserves_delete_time_mode_and_reports_pair() {
        let build_doc = |default_mode: &str, active_mode: &str| -> (Document, NodeId) {
            let mut d = Document::new("t");
            let p = d.add_page("p");
            d.variable_collections.insert(
                "c".into(),
                VariableCollection {
                    id: "c".into(),
                    name: "Theme".into(),
                    modes: vec![
                        VariableMode { id: "light".into(), name: "Light".into() },
                        VariableMode { id: "dim".into(), name: "Dim".into() },
                        VariableMode { id: "dark".into(), name: "Dark".into() },
                    ],
                    default_mode: default_mode.into(),
                    variables: vec!["v_fill".into(), "v_op".into()],
                    key: None,
                    source_library_key: None,
                    extensions: Default::default(),
                },
            );
            d.variables.insert(
                "v_fill".into(),
                Variable {
                    id: "v_fill".into(),
                    name: "v_fill".into(),
                    collection: "c".into(),
                    kind: VariableKind::Color,
                    values: [
                        ("light".to_string(), VariableValue::Color(Color::rgba(200, 100, 50, 255))),
                        ("dim".to_string(), VariableValue::Color(Color::rgba(100, 150, 200, 255))),
                        ("dark".to_string(), VariableValue::Color(Color::rgba(40, 80, 160, 128))),
                    ]
                    .into_iter()
                    .collect(),
                    description: String::new(),
                    scopes: vec![],
                    hidden_from_publishing: false,
                    key: None,
                    code_syntax: Default::default(),
                    extensions: Default::default(),
                    mode_extensions: Default::default(),
                },
            );
            d.variables.insert(
                "v_op".into(),
                Variable {
                    id: "v_op".into(),
                    name: "v_op".into(),
                    collection: "c".into(),
                    kind: VariableKind::Number,
                    values: [
                        ("light".to_string(), VariableValue::Number(80.0)),
                        ("dim".to_string(), VariableValue::Number(20.0)),
                        ("dark".to_string(), VariableValue::Number(5.0)),
                    ]
                    .into_iter()
                    .collect(),
                    description: String::new(),
                    scopes: vec![],
                    hidden_from_publishing: false,
                    key: None,
                    code_syntax: Default::default(),
                    extensions: Default::default(),
                    mode_extensions: Default::default(),
                },
            );
            let rid = d.alloc_id();
            let mut r = Node::rectangle(rid, Rect::new(0.0, 0.0, 100.0, 40.0));
            r.fills = vec![Paint::solid(Color::BLACK)];
            r.style_refs.insert("fill".into(), "Surface".into());
            r.variables.bindings.insert("fills/0".into(), "v_fill".into());
            r.variables.modes.insert("c".into(), active_mode.into());
            let rid = d.insert(p, r).unwrap();

            d.styles.insert("Surface".into(), Style::Fill(vec![Paint::solid(Color::BLACK)]));
            d.style_meta.insert(
                "Surface".into(),
                StyleMeta {
                    name: "Surface".into(),
                    property: StyleProperty::Fill,
                    bindings: [("fills/0/opacity".to_string(), "v_op".to_string())].into_iter().collect(),
                    ..Default::default()
                },
            );
            (d, rid)
        };

        // 1. Deleted in "light" (style wins at delete time -> bakes 80%, dim gets 0.8).
        let (mut d_light, rid_light) = build_doc("light", "light");
        let before_light = d_light.clone();
        let mut h = History::default();
        let (c_light, fb_light) = delete_style_with_fallbacks(&d_light, "Surface").unwrap();
        assert_eq!(fb_light, vec![(rid_light, "fills/0/opacity".to_string())]);
        run(&mut d_light, &mut h, Ok(c_light));
        assert_resolved_matches_across_mode_combos(&before_light, &d_light, rid_light, &[&[("c", "light")], &[("c", "dark")]], "deleted in light");
        d_light.get_mut(rid_light).unwrap().variables.modes.insert("c".into(), "dim".into());
        let dim_after_light = resolve_node(&d_light, d_light.get(rid_light).unwrap());
        assert!((dim_after_light.fills[0].opacity - 0.8).abs() < 1e-6, "dim gets 0.8 when deleted in light");

        // 2. Deleted in "dim" (style wins at delete time -> bakes 20%, dim is exact at 0.2, light gets 0.2).
        let (mut d_dim, rid_dim) = build_doc("light", "dim");
        let before_dim = d_dim.clone();
        let (c_dim, fb_dim) = delete_style_with_fallbacks(&d_dim, "Surface").unwrap();
        assert_eq!(fb_dim, vec![(rid_dim, "fills/0/opacity".to_string())]);
        run(&mut d_dim, &mut h, Ok(c_dim));
        assert_resolved_matches_across_mode_combos(&before_dim, &d_dim, rid_dim, &[&[("c", "dim")], &[("c", "dark")]], "deleted in dim");
        d_dim.get_mut(rid_dim).unwrap().variables.modes.insert("c".into(), "dim".into());
        let dim_after_dim = resolve_node(&d_dim, d_dim.get(rid_dim).unwrap());
        assert!((dim_after_dim.fills[0].opacity - 0.2).abs() < 1e-6, "dim stays exact at 0.2 when deleted in dim");

        // 3. Deleted in "dark" with default_mode="light" (own wins at delete time -> bakes default mode "light" = 80%, dim gets 0.8).
        let (mut d_dark, rid_dark) = build_doc("light", "dark");
        let before_dark = d_dark.clone();
        let (c_dark, fb_dark) = delete_style_with_fallbacks(&d_dark, "Surface").unwrap();
        assert_eq!(fb_dark, vec![(rid_dark, "fills/0/opacity".to_string())]);
        run(&mut d_dark, &mut h, Ok(c_dark));
        assert_resolved_matches_across_mode_combos(&before_dark, &d_dark, rid_dark, &[&[("c", "dark")], &[("c", "light")]], "deleted in dark");
        d_dark.get_mut(rid_dark).unwrap().variables.modes.insert("c".into(), "dim".into());
        let dim_after_dark = resolve_node(&d_dark, d_dark.get(rid_dark).unwrap());
        assert!((dim_after_dark.fills[0].opacity - 0.8).abs() < 1e-6, "dim gets default mode 0.8 when deleted in dark");

        // 4. Deleted in "dark" when default_mode is ALSO "dark" (style loses in both delete-time and default mode -> bakes first style-won mode "light" = 80%, dim gets 0.8).
        let (mut d_def_loses, rid_def_loses) = build_doc("dark", "dark");
        let before_def_loses = d_def_loses.clone();
        let (c_def_loses, fb_def_loses) = delete_style_with_fallbacks(&d_def_loses, "Surface").unwrap();
        assert_eq!(fb_def_loses, vec![(rid_def_loses, "fills/0/opacity".to_string())]);
        run(&mut d_def_loses, &mut h, Ok(c_def_loses));
        assert_resolved_matches_across_mode_combos(
            &before_def_loses,
            &d_def_loses,
            rid_def_loses,
            &[&[("c", "dark")], &[("c", "light")]],
            "deleted in dark when default loses",
        );
        d_def_loses.get_mut(rid_def_loses).unwrap().variables.modes.insert("c".into(), "dim".into());
        let dim_after_def_loses = resolve_node(&d_def_loses, d_def_loses.get(rid_def_loses).unwrap());
        assert!((dim_after_def_loses.fills[0].opacity - 0.8).abs() < 1e-6, "dim gets first style-won mode 0.8 when default mode also loses");
    }

    #[test]
    fn r5_row2_three_modes_same_style_opacity_in_every_style_won_mode_is_exact_and_reports_nothing() {
        for active_mode in ["light", "dim", "dark"] {
            let mut d = Document::new("t");
            let p = d.add_page("p");
            d.variable_collections.insert(
                "c".into(),
                VariableCollection {
                    id: "c".into(),
                    name: "Theme".into(),
                    modes: vec![
                        VariableMode { id: "light".into(), name: "Light".into() },
                        VariableMode { id: "dim".into(), name: "Dim".into() },
                        VariableMode { id: "dark".into(), name: "Dark".into() },
                    ],
                    default_mode: "light".into(),
                    variables: vec!["v_fill".into(), "v_op".into()],
                    key: None,
                    source_library_key: None,
                    extensions: Default::default(),
                },
            );
            d.variables.insert(
                "v_fill".into(),
                Variable {
                    id: "v_fill".into(),
                    name: "v_fill".into(),
                    collection: "c".into(),
                    kind: VariableKind::Color,
                    values: [
                        ("light".to_string(), VariableValue::Color(Color::rgba(200, 100, 50, 255))),
                        ("dim".to_string(), VariableValue::Color(Color::rgba(100, 150, 200, 255))),
                        ("dark".to_string(), VariableValue::Color(Color::rgba(40, 80, 160, 128))),
                    ]
                    .into_iter()
                    .collect(),
                    description: String::new(),
                    scopes: vec![],
                    hidden_from_publishing: false,
                    key: None,
                    code_syntax: Default::default(),
                    extensions: Default::default(),
                    mode_extensions: Default::default(),
                },
            );
            d.variables.insert(
                "v_op".into(),
                Variable {
                    id: "v_op".into(),
                    name: "v_op".into(),
                    collection: "c".into(),
                    kind: VariableKind::Number,
                    values: [
                        ("light".to_string(), VariableValue::Number(80.0)),
                        ("dim".to_string(), VariableValue::Number(80.0)),
                        ("dark".to_string(), VariableValue::Number(5.0)),
                    ]
                    .into_iter()
                    .collect(),
                    description: String::new(),
                    scopes: vec![],
                    hidden_from_publishing: false,
                    key: None,
                    code_syntax: Default::default(),
                    extensions: Default::default(),
                    mode_extensions: Default::default(),
                },
            );
            let rid = d.alloc_id();
            let mut r = Node::rectangle(rid, Rect::new(0.0, 0.0, 100.0, 40.0));
            r.fills = vec![Paint::solid(Color::BLACK)];
            r.style_refs.insert("fill".into(), "Surface".into());
            r.variables.bindings.insert("fills/0".into(), "v_fill".into());
            r.variables.modes.insert("c".into(), active_mode.into());
            let rid = d.insert(p, r).unwrap();

            d.styles.insert("Surface".into(), Style::Fill(vec![Paint::solid(Color::BLACK)]));
            d.style_meta.insert(
                "Surface".into(),
                StyleMeta {
                    name: "Surface".into(),
                    property: StyleProperty::Fill,
                    bindings: [("fills/0/opacity".to_string(), "v_op".to_string())].into_iter().collect(),
                    ..Default::default()
                },
            );

            let before = d.clone();
            let mut h = History::default();
            let (c, fb) = delete_style_with_fallbacks(&d, "Surface").unwrap();
            assert!(fb.is_empty(), "row2 must report no fallback when all style-won modes agree");
            run(&mut d, &mut h, Ok(c));
            assert_resolved_matches_across_mode_combos(
                &before,
                &d,
                rid,
                &[&[("c", "light")], &[("c", "dim")], &[("c", "dark")]],
                &format!("row2 deleted in {active_mode}"),
            );
        }
    }

    #[test]
    fn r5_row3_style_variable_in_second_collection_brand_and_own_in_theme() {
        let all_four: &[&[(&str, &str)]] = &[
            &[("Theme", "light"), ("Brand", "a")],
            &[("Theme", "light"), ("Brand", "b")],
            &[("Theme", "dark"), ("Brand", "a")],
            &[("Theme", "dark"), ("Brand", "b")],
        ];

        // Sub-case A: own in Theme (light opaque, dark translucent) and style in Brand (a=50, b=10).
        // Style wins in (light, a)=50 and (light, b)=10 -> differs across Brand modes -> fallback!
        // When deleted in (dark, b) (own wins at delete time), fallback bakes default mode (light, a) = 50% (0.5).
        let mut d = Document::new("t");
        let p = d.add_page("p");
        d.variable_collections.insert(
            "Theme".into(),
            VariableCollection {
                id: "Theme".into(),
                name: "Theme".into(),
                modes: vec![VariableMode { id: "light".into(), name: "Light".into() }, VariableMode { id: "dark".into(), name: "Dark".into() }],
                default_mode: "light".into(),
                variables: vec!["v_fill".into()],
                key: None,
                source_library_key: None,
                extensions: Default::default(),
            },
        );
        d.variable_collections.insert(
            "Brand".into(),
            VariableCollection {
                id: "Brand".into(),
                name: "Brand".into(),
                modes: vec![VariableMode { id: "a".into(), name: "A".into() }, VariableMode { id: "b".into(), name: "B".into() }],
                default_mode: "a".into(),
                variables: vec!["v_op".into()],
                key: None,
                source_library_key: None,
                extensions: Default::default(),
            },
        );
        d.variables.insert(
            "v_fill".into(),
            Variable {
                id: "v_fill".into(),
                name: "v_fill".into(),
                collection: "Theme".into(),
                kind: VariableKind::Color,
                values: [
                    ("light".to_string(), VariableValue::Color(Color::rgba(200, 100, 50, 255))),
                    ("dark".to_string(), VariableValue::Color(Color::rgba(40, 80, 160, 128))),
                ]
                .into_iter()
                .collect(),
                description: String::new(),
                scopes: vec![],
                hidden_from_publishing: false,
                key: None,
                code_syntax: Default::default(),
                extensions: Default::default(),
                mode_extensions: Default::default(),
            },
        );
        d.variables.insert(
            "v_op".into(),
            Variable {
                id: "v_op".into(),
                name: "v_op".into(),
                collection: "Brand".into(),
                kind: VariableKind::Number,
                values: [("a".to_string(), VariableValue::Number(50.0)), ("b".to_string(), VariableValue::Number(10.0))].into_iter().collect(),
                description: String::new(),
                scopes: vec![],
                hidden_from_publishing: false,
                key: None,
                code_syntax: Default::default(),
                extensions: Default::default(),
                mode_extensions: Default::default(),
            },
        );
        let rid = d.alloc_id();
        let mut r = Node::rectangle(rid, Rect::new(0.0, 0.0, 100.0, 40.0));
        r.fills = vec![Paint::solid(Color::BLACK)];
        r.style_refs.insert("fill".into(), "Surface".into());
        r.variables.bindings.insert("fills/0".into(), "v_fill".into());
        r.variables.modes.insert("Theme".into(), "dark".into());
        r.variables.modes.insert("Brand".into(), "b".into());
        let rid = d.insert(p, r).unwrap();
        d.styles.insert("Surface".into(), Style::Fill(vec![Paint::solid(Color::BLACK)]));
        d.style_meta.insert(
            "Surface".into(),
            StyleMeta {
                name: "Surface".into(),
                property: StyleProperty::Fill,
                bindings: [("fills/0/opacity".to_string(), "v_op".to_string())].into_iter().collect(),
                ..Default::default()
            },
        );
        let before = d.clone();
        let mut h = History::default();
        let (c, fb) = delete_style_with_fallbacks(&d, "Surface").unwrap();
        assert_eq!(fb, vec![(rid, "fills/0/opacity".to_string())]);
        run(&mut d, &mut h, Ok(c));
        assert_resolved_matches_across_mode_combos(
            &before,
            &d,
            rid,
            &[&[("Theme", "dark"), ("Brand", "a")], &[("Theme", "dark"), ("Brand", "b")], &[("Theme", "light"), ("Brand", "a")]],
            "row3 deleted in (dark, b): delete-time and default combinations are exact",
        );

        // Sub-case A2: deleted in (light, b) (style wins at delete time -> bakes 10%).
        let mut d_lb = before.clone();
        d_lb.get_mut(rid).unwrap().variables.modes.insert("Theme".into(), "light".into());
        d_lb.get_mut(rid).unwrap().variables.modes.insert("Brand".into(), "b".into());
        let before_lb = d_lb.clone();
        let (c_lb, fb_lb) = delete_style_with_fallbacks(&d_lb, "Surface").unwrap();
        assert_eq!(fb_lb, vec![(rid, "fills/0/opacity".to_string())]);
        run(&mut d_lb, &mut h, Ok(c_lb));
        assert_resolved_matches_across_mode_combos(
            &before_lb,
            &d_lb,
            rid,
            &[&[("Theme", "light"), ("Brand", "b")], &[("Theme", "dark"), ("Brand", "a")], &[("Theme", "dark"), ("Brand", "b")]],
            "row3 deleted in (light, b): delete-time and dark combinations are exact",
        );

        // Sub-case B: own in Theme is opaque in both light and dark; style in Brand has a=50, b=10 -> exact over all 4 combinations, nothing reported.
        let mut d_exact = before.clone();
        d_exact.variables.get_mut("v_fill").unwrap().values.insert("dark".into(), VariableValue::Color(Color::rgba(40, 80, 160, 255)));
        let before_exact = d_exact.clone();
        let (c_exact, fb_exact) = delete_style_with_fallbacks(&d_exact, "Surface").unwrap();
        assert!(fb_exact.is_empty());
        run(&mut d_exact, &mut h, Ok(c_exact));
        assert_resolved_matches_across_mode_combos(&before_exact, &d_exact, rid, all_four, "row3 non-overriding own is exact across all 4 combinations");

        // Sub-case C: own in Theme overrides in dark, and style in Brand agrees across a and b (a=50, b=50) -> exact over all 4 combinations, nothing reported.
        let mut d_agree = before.clone();
        d_agree.variables.get_mut("v_op").unwrap().values.insert("b".into(), VariableValue::Number(50.0));
        let before_agree = d_agree.clone();
        let (c_agree, fb_agree) = delete_style_with_fallbacks(&d_agree, "Surface").unwrap();
        assert!(fb_agree.is_empty());
        run(&mut d_agree, &mut h, Ok(c_agree));
        assert_resolved_matches_across_mode_combos(&before_agree, &d_agree, rid, all_four, "row3 agreeing Brand modes is exact across all 4 combinations");
    }

    #[test]
    fn r5_row4_style_font_weight_missing_in_light_with_own_font_style_italic_and_bold_italic() {
        for active_mode in ["light", "dark"] {
            let mut d = Document::new("t");
            let p = d.add_page("p");
            d.variable_collections.insert(
                "c".into(),
                VariableCollection {
                    id: "c".into(),
                    name: "Theme".into(),
                    modes: vec![VariableMode { id: "light".into(), name: "Light".into() }, VariableMode { id: "dark".into(), name: "Dark".into() }],
                    default_mode: "light".into(),
                    variables: vec!["v_style".into(), "v_weight".into()],
                    key: None,
                    source_library_key: None,
                    extensions: Default::default(),
                },
            );
            d.variables.insert(
                "v_style".into(),
                make_var("v_style", VariableKind::String, VariableValue::String("Italic".into()), VariableValue::String("Bold Italic".into())),
            );
            d.variables.insert(
                "v_weight".into(),
                Variable {
                    id: "v_weight".into(),
                    name: "v_weight".into(),
                    collection: "c".into(),
                    kind: VariableKind::Number,
                    values: [("dark".to_string(), VariableValue::Number(800.0))].into_iter().collect(),
                    description: String::new(),
                    scopes: vec![],
                    hidden_from_publishing: false,
                    key: None,
                    code_syntax: Default::default(),
                    extensions: Default::default(),
                    mode_extensions: Default::default(),
                },
            );
            let tid = d.alloc_id();
            let mut t = Node::text(tid, Rect::new(0.0, 0.0, 100.0, 30.0), "Hello", TextStyle::default());
            t.style_refs.insert("text".into(), "Heading".into());
            t.variables.bindings.insert("fontStyle".into(), "v_style".into());
            t.variables.modes.insert("c".into(), active_mode.into());
            let tid = d.insert(p, t).unwrap();

            d.styles.insert("Heading".into(), Style::Text(TextStyle::default()));
            d.style_meta.insert(
                "Heading".into(),
                StyleMeta {
                    name: "Heading".into(),
                    property: StyleProperty::Text,
                    bindings: [("fontWeight".to_string(), "v_weight".to_string())].into_iter().collect(),
                    ..Default::default()
                },
            );

            let before = d.clone();
            let mut h = History::default();
            let (c, fb) = delete_style_with_fallbacks(&d, "Heading").unwrap();
            assert!(fb.is_empty(), "row4 must report no fallback when no style-won mode resolves");
            run(&mut d, &mut h, Ok(c));

            assert_resolved_matches_across_mode_combos(&before, &d, tid, &[&[("c", "light")], &[("c", "dark")]], &format!("row4 deleted in {active_mode}"));
            d.get_mut(tid).unwrap().variables.modes.insert("c".into(), "dark".into());
            let dark_node = resolve_node(&d, d.get(tid).unwrap());
            let NodeKind::Text(ref dark_text) = dark_node.kind else { panic!("expected text") };
            assert_eq!(dark_text.style.font_weight, 700, "dark stays 700 after delete");
        }
    }

    #[test]
    fn r5_row5_value_stored_only_under_light_plus_compact_key_classified_with_resolve_variable() {
        let all_four: &[&[(&str, &str)]] = &[
            &[("theme", "light"), ("density", "compact")],
            &[("theme", "light"), ("density", "cozy")],
            &[("theme", "dark"), ("density", "compact")],
            &[("theme", "dark"), ("density", "cozy")],
        ];

        for start_combo in all_four {
            let mut d = Document::new("t");
            let p = d.add_page("p");
            d.variable_collections.insert(
                "theme".into(),
                VariableCollection {
                    id: "theme".into(),
                    name: "Theme".into(),
                    modes: vec![VariableMode { id: "light".into(), name: "Light".into() }, VariableMode { id: "dark".into(), name: "Dark".into() }],
                    default_mode: "light".into(),
                    variables: vec!["v_fill".into(), "v_op".into()],
                    key: None,
                    source_library_key: None,
                    extensions: Default::default(),
                },
            );
            d.variable_collections.insert(
                "density".into(),
                VariableCollection {
                    id: "density".into(),
                    name: "Density".into(),
                    modes: vec![VariableMode { id: "compact".into(), name: "Compact".into() }, VariableMode { id: "cozy".into(), name: "Cozy".into() }],
                    default_mode: "compact".into(),
                    variables: vec![],
                    key: None,
                    source_library_key: None,
                    extensions: Default::default(),
                },
            );
            d.variables.insert(
                "v_fill".into(),
                Variable {
                    id: "v_fill".into(),
                    name: "v_fill".into(),
                    collection: "theme".into(),
                    kind: VariableKind::Color,
                    values: [
                        ("light+compact".to_string(), VariableValue::Color(Color::rgba(200, 100, 50, 128))),
                        ("light+cozy".to_string(), VariableValue::Color(Color::rgba(200, 100, 50, 255))),
                        ("dark+compact".to_string(), VariableValue::Color(Color::rgba(40, 80, 160, 255))),
                        ("dark+cozy".to_string(), VariableValue::Color(Color::rgba(40, 80, 160, 255))),
                    ]
                    .into_iter()
                    .collect(),
                    description: String::new(),
                    scopes: vec![],
                    hidden_from_publishing: false,
                    key: None,
                    code_syntax: Default::default(),
                    extensions: Default::default(),
                    mode_extensions: Default::default(),
                },
            );
            d.variables.insert(
                "v_op".into(),
                Variable {
                    id: "v_op".into(),
                    name: "v_op".into(),
                    collection: "theme".into(),
                    kind: VariableKind::Number,
                    values: [
                        ("light+compact".to_string(), VariableValue::Number(25.0)),
                        ("light+cozy".to_string(), VariableValue::Number(50.0)),
                        ("dark+compact".to_string(), VariableValue::Number(50.0)),
                        ("dark+cozy".to_string(), VariableValue::Number(50.0)),
                    ]
                    .into_iter()
                    .collect(),
                    description: String::new(),
                    scopes: vec![],
                    hidden_from_publishing: false,
                    key: None,
                    code_syntax: Default::default(),
                    extensions: Default::default(),
                    mode_extensions: Default::default(),
                },
            );

            let rid = d.alloc_id();
            let mut r = Node::rectangle(rid, Rect::new(0.0, 0.0, 100.0, 40.0));
            r.fills = vec![Paint::solid(Color::BLACK)];
            r.style_refs.insert("fill".into(), "Surface".into());
            r.variables.bindings.insert("fills/0".into(), "v_fill".into());
            for (cid, mid) in *start_combo {
                r.variables.modes.insert((*cid).into(), (*mid).into());
            }
            let rid = d.insert(p, r).unwrap();

            d.styles.insert("Surface".into(), Style::Fill(vec![Paint::solid(Color::BLACK)]));
            d.style_meta.insert(
                "Surface".into(),
                StyleMeta {
                    name: "Surface".into(),
                    property: StyleProperty::Fill,
                    bindings: [("fills/0/opacity".to_string(), "v_op".to_string())].into_iter().collect(),
                    ..Default::default()
                },
            );

            let before = d.clone();
            let mut h = History::default();
            let (c, fb) = delete_style_with_fallbacks(&d, "Surface").unwrap();
            assert!(fb.is_empty(), "row5 must report no fallback when all style-won mode combinations agree");
            run(&mut d, &mut h, Ok(c));
            assert_resolved_matches_across_mode_combos(&before, &d, rid, all_four, &format!("row5 deleted in {start_combo:?}"));
        }
    }

    #[test]
    fn r5_row6_mixed_stop_keys_keep_binding_and_match_every_mode() {
        let gradient_paint = Paint {
            kind: PaintKind::LinearGradient {
                handles: [Vec2::ZERO, Vec2::new(1.0, 0.0), Vec2::new(0.0, 1.0)],
                stops: vec![GradientStop { pos: 0.0, color: Color::BLACK }, GradientStop { pos: 1.0, color: Color::WHITE }],
            },
            opacity: 1.0,
            visible: true,
            blend: BlendMode::Normal,
        };

        for active_mode in ["light", "dim", "dark"] {
            let mut d = Document::new("t");
            let p = d.add_page("p");
            d.variable_collections.insert(
                "c".into(),
                VariableCollection {
                    id: "c".into(),
                    name: "Theme".into(),
                    modes: vec![
                        VariableMode { id: "light".into(), name: "Light".into() },
                        VariableMode { id: "dim".into(), name: "Dim".into() },
                        VariableMode { id: "dark".into(), name: "Dark".into() },
                    ],
                    default_mode: "light".into(),
                    variables: vec!["v_fill_dark_only".into(), "v_stop".into()],
                    key: None,
                    source_library_key: None,
                    extensions: Default::default(),
                },
            );
            d.variables.insert(
                "v_fill_dark_only".into(),
                Variable {
                    id: "v_fill_dark_only".into(),
                    name: "v_fill_dark_only".into(),
                    collection: "c".into(),
                    kind: VariableKind::Color,
                    values: [("dark".to_string(), VariableValue::Color(Color::rgba(10, 20, 30, 255)))].into_iter().collect(),
                    description: String::new(),
                    scopes: vec![],
                    hidden_from_publishing: false,
                    key: None,
                    code_syntax: Default::default(),
                    extensions: Default::default(),
                    mode_extensions: Default::default(),
                },
            );
            d.variables.insert(
                "v_stop".into(),
                Variable {
                    id: "v_stop".into(),
                    name: "v_stop".into(),
                    collection: "c".into(),
                    kind: VariableKind::Color,
                    values: [
                        ("light".to_string(), VariableValue::Color(Color::rgba(220, 30, 30, 255))),
                        ("dim".to_string(), VariableValue::Color(Color::rgba(30, 220, 30, 255))),
                        ("dark".to_string(), VariableValue::Color(Color::rgba(30, 30, 220, 255))),
                    ]
                    .into_iter()
                    .collect(),
                    description: String::new(),
                    scopes: vec![],
                    hidden_from_publishing: false,
                    key: None,
                    code_syntax: Default::default(),
                    extensions: Default::default(),
                    mode_extensions: Default::default(),
                },
            );

            let rid = d.alloc_id();
            let mut r = Node::rectangle(rid, Rect::new(0.0, 0.0, 100.0, 40.0));
            r.fills = vec![gradient_paint.clone()];
            r.stroke.paints = vec![gradient_paint.clone()];
            r.style_refs.insert("fill".into(), "Grad".into());
            r.style_refs.insert("stroke".into(), "Grad".into());
            r.variables.bindings.insert("fills/0".into(), "v_fill_dark_only".into());
            r.variables.bindings.insert("strokes/0".into(), "v_fill_dark_only".into());
            r.variables.modes.insert("c".into(), active_mode.into());
            let rid = d.insert(p, r).unwrap();

            d.styles.insert("Grad".into(), Style::Fill(vec![gradient_paint.clone()]));
            d.style_meta.insert(
                "Grad".into(),
                StyleMeta {
                    name: "Grad".into(),
                    property: StyleProperty::Fill,
                    bindings: [("fills/0/stops/0".to_string(), "v_stop".to_string())].into_iter().collect(),
                    ..Default::default()
                },
            );

            let before = d.clone();
            let mut h = History::default();
            let (c, fb) = delete_style_with_fallbacks(&d, "Grad").unwrap();
            assert!(fb.is_empty(), "row6 must report no fallback when stop bindings are kept");
            run(&mut d, &mut h, Ok(c));

            assert_eq!(
                d.get(rid).unwrap().variables.bindings.get("fills/0/stops/0").map(String::as_str),
                Some("v_stop"),
                "mixed fill stop binding must be kept"
            );
            assert_eq!(
                d.get(rid).unwrap().variables.bindings.get("strokes/0/stops/0").map(String::as_str),
                Some("v_stop"),
                "mixed stroke stop binding must be kept"
            );
            assert_resolved_matches_across_mode_combos(
                &before,
                &d,
                rid,
                &[&[("c", "light")], &[("c", "dim")], &[("c", "dark")]],
                &format!("row6 deleted in {active_mode}"),
            );
        }
    }

    #[test]
    fn style_order_kept_through_native_save_and_load_and_old_file_omits_sort_position() {
        let mut d = Document::new("StyleOrderDoc");
        d.add_page("Page 1");
        // Insert in BTreeMap key order: s1, s2, s3, but assign Figma sortPosition so order is s3, s1, s2.
        d.styles.insert("s1".into(), Style::Effect(vec![shadow(8.0)]));
        d.styles.insert("s2".into(), Style::Effect(vec![shadow(16.0)]));
        d.styles.insert("s3".into(), Style::Effect(vec![shadow(4.0)]));
        d.style_meta.insert(
            "s1".into(),
            StyleMeta { name: "Shadows/md".into(), property: StyleProperty::Effect, sort_position: Some("!~~p~oLf".into()), ..Default::default() },
        );
        d.style_meta.insert(
            "s2".into(),
            StyleMeta { name: "Shadows/lg".into(), property: StyleProperty::Effect, sort_position: Some("!~~p~oLr".into()), ..Default::default() },
        );
        d.style_meta.insert(
            "s3".into(),
            StyleMeta { name: "Shadows/xs".into(), property: StyleProperty::Effect, sort_position: Some("!~~p~oL".into()), ..Default::default() },
        );

        let json = d.to_json().unwrap();
        let loaded = Document::from_json(&json).unwrap();
        assert_eq!(loaded.style_meta["s3"].sort_position.as_deref(), Some("!~~p~oL"));
        assert_eq!(loaded.style_meta["s1"].sort_position.as_deref(), Some("!~~p~oLf"));
        assert_eq!(loaded.style_meta["s2"].sort_position.as_deref(), Some("!~~p~oLr"));
        assert_eq!(ordered_style_keys(&loaded, Some(StyleProperty::Effect)), vec!["s3".to_string(), "s1".to_string(), "s2".to_string()]);

        // (d) Old file without `sort_position` loads with stable default order and saving does not add `sort_position`.
        let mut old_doc = Document::new("LegacyDoc");
        old_doc.add_page("Page 1");
        old_doc.styles.insert("b_key".into(), Style::Fill(vec![Paint::solid(Color::WHITE)]));
        old_doc.styles.insert("a_key".into(), Style::Fill(vec![Paint::solid(Color::BLACK)]));
        old_doc.style_meta.insert("b_key".into(), StyleMeta { name: "Brand/Zeta".into(), property: StyleProperty::Fill, ..Default::default() });
        old_doc.style_meta.insert("a_key".into(), StyleMeta { name: "Brand/Alpha".into(), property: StyleProperty::Fill, ..Default::default() });
        let old_json = old_doc.to_json().unwrap();
        assert!(!old_json.contains("sort_position"), "old file without sort_position must omit field on save: {old_json}");
        let old_loaded = Document::from_json(&old_json).unwrap();
        assert_eq!(old_loaded.style_meta["a_key"].sort_position, None);
        assert_eq!(old_loaded.style_meta["b_key"].sort_position, None);
        assert_eq!(ordered_style_keys(&old_loaded, Some(StyleProperty::Fill)), vec!["a_key".to_string(), "b_key".to_string()]);
        let saved_again = old_loaded.to_json().unwrap();
        assert!(!saved_again.contains("sort_position"), "saving loaded legacy file must not add sort_position");
    }

    #[test]
    fn folder_rename_renames_every_member_and_one_undo_restores_all() {
        let mut d = Document::new("FolderRenameDoc");
        d.add_page("Page 1");
        for (key, name) in [
            ("s1", "Brand/Primary"),
            ("s2", "Brand/Secondary"),
            ("s3", "Brand/Nested/Accent"),
            ("s4", "BrandNew/Untouched"),
            ("s5", "Surface"),
            ("s6", "Brand / Primary"),
        ] {
            d.styles.insert(key.into(), Style::Fill(vec![Paint::solid(Color::WHITE)]));
            d.style_meta.insert(
                key.into(),
                StyleMeta { name: name.into(), property: StyleProperty::Fill, sort_position: Some(format!("!00000{key}")), ..Default::default() },
            );
        }

        let mut h = History::default();
        let cmd = rename_style_folder(&d, "Brand", "Theme").unwrap();
        h.exec(&mut d, cmd).unwrap();

        assert_eq!(d.style_name("s1"), "Theme/Primary");
        assert_eq!(d.style_name("s2"), "Theme/Secondary");
        assert_eq!(d.style_name("s3"), "Theme/Nested/Accent");
        assert_eq!(d.style_name("s4"), "BrandNew/Untouched");
        assert_eq!(d.style_name("s5"), "Surface");
        assert_eq!(d.style_name("s6"), "Theme / Primary");
        assert_eq!(d.style_meta["s1"].sort_position.as_deref(), Some("!00000s1"), "sort_position preserved on folder rename");

        // Single undo step restores all members.
        assert!(h.undo(&mut d).unwrap());
        assert_eq!(d.style_name("s1"), "Brand/Primary");
        assert_eq!(d.style_name("s2"), "Brand/Secondary");
        assert_eq!(d.style_name("s3"), "Brand/Nested/Accent");
        assert_eq!(d.style_name("s4"), "BrandNew/Untouched");
        assert_eq!(d.style_name("s5"), "Surface");
        assert_eq!(d.style_name("s6"), "Brand / Primary");
    }

    #[test]
    fn set_style_description_sets_text_in_one_undo_step_and_keeps_other_fields() {
        let mut d = Document::new("StyleDescDoc");
        d.add_page("Page 1");
        d.styles.insert("s1".into(), Style::Fill(vec![Paint::solid(Color::WHITE)]));
        let mut bindings = BTreeMap::new();
        bindings.insert("fills/0".to_string(), "var_primary".to_string());
        d.style_meta.insert(
            "s1".into(),
            StyleMeta {
                name: "Brand/Primary".into(),
                key: Some("orig-key".into()),
                description: "Old description".into(),
                property: StyleProperty::Fill,
                bindings: bindings.clone(),
                sort_position: Some("!~~p~oL".into()),
            },
        );

        let mut h = History::default();
        let cmd = set_style_description(&d, "s1", "Primary brand fill   \n\t ").unwrap();
        assert!(matches!(cmd, Command::SetStyles { .. }), "changing description must return Command::SetStyles");
        h.exec(&mut d, cmd).unwrap();

        let meta = &d.style_meta["s1"];
        assert_eq!(meta.description, "Primary brand fill");
        assert_eq!(meta.name, "Brand/Primary");
        assert_eq!(meta.key.as_deref(), Some("orig-key"));
        assert_eq!(meta.property, StyleProperty::Fill);
        assert_eq!(meta.bindings, bindings);
        assert_eq!(meta.sort_position.as_deref(), Some("!~~p~oL"));

        // Setting the same text (after trailing whitespace trim) is a no-op.
        let noop = set_style_description(&d, "s1", "Primary brand fill   ").unwrap();
        assert!(matches!(noop, Command::Batch(ref v) if v.is_empty()), "same text must be a no-op batch");

        // One undo restores the old description and keeps all other fields.
        assert!(h.undo(&mut d).unwrap());
        let meta = &d.style_meta["s1"];
        assert_eq!(meta.description, "Old description");
        assert_eq!(meta.name, "Brand/Primary");
        assert_eq!(meta.sort_position.as_deref(), Some("!~~p~oL"));
        assert_eq!(meta.bindings, bindings);
        assert!(!h.can_undo(), "must be a single undo step");
    }

    #[test]
    fn set_style_description_refuses_library_style() {
        let (mut d, _) = paint_and_text_fixture();
        d.style_meta.insert(
            "Brand".into(),
            StyleMeta { name: "Brand".into(), description: "Published description".into(), property: StyleProperty::Fill, ..Default::default() },
        );
        d.library.external.insert(
            "L1/Brand".into(),
            ExternalAsset { library_id: "L1".into(), version: 1, kind: LibraryAssetKind::Style, node: None, local_key: Some("Brand".into()) },
        );
        let before = d.clone();
        let err = set_style_description(&d, "Brand", "Local override").unwrap_err();
        assert!(matches!(err, StyleOpError::LibraryStyle(ref k) if k == "Brand"), "expected LibraryStyle, got {err:?}");
        assert_eq!(d.style_meta["Brand"].description, "Published description");
        assert_eq!(d.styles, before.styles);
        assert_eq!(d.style_meta, before.style_meta);
    }
}
