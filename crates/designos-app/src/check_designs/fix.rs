//! Applies Check designs fixes through the editor's transaction, so one fix (or "Fix all") is one
//! undo step. Every fix re-validates the layer as it is NOW and refuses when it drifted since the
//! audit, so a stale finding can never overwrite an edit made in between.

use super::{contrast, detached, hardcoded, Finding, Fix};
use crate::app::App;
use crate::edit::Tx;
use crate::layout;
use crate::libraries::is_noop;
use designos_doc::{import_asset, local_key, set_bound_property, set_text_fills_everywhere, Command, NodeId, NodeKind, OverrideField, PaintKind, Vec2};

/// Order fixes run in within "Fix all": recolouring first, then links to variables and styles, and
/// the swap of a detached frame for an instance last. The swap only proceeds while the frame still
/// equals its main component, so any fix made to the frame or to a layer inside it first makes the
/// swap refuse, instead of the swap deleting the layer a fix was just applied to (or a later fix
/// finding it gone). A bind that a recolour made stale is likewise refused rather than undoing it.
fn priority(fix: &Fix) -> u8 {
    match fix {
        Fix::SetTextColor { .. } => 0,
        Fix::BindVariable { .. } | Fix::BindVariables { .. } | Fix::ApplyStyle { .. } => 1,
        Fix::Reconnect { .. } => 2,
    }
}

impl App {
    /// Applies one finding's fix as a single undo step. False when nothing changed.
    pub fn check_designs_fix(&mut self, finding: &Finding) -> bool {
        let mut tx = Tx::new();
        if !self.apply_check_fix(&mut tx, finding) {
            tx.cancel(&mut self.doc);
            return false;
        }
        self.run_tx(tx)
    }

    /// Applies every fixable finding as ONE undo step; returns how many fixes landed.
    pub fn check_designs_fix_all(&mut self, findings: &[Finding]) -> usize {
        let mut todo: Vec<&Finding> = findings.iter().filter(|f| f.fix.is_some()).collect();
        todo.sort_by_key(|f| f.fix.as_ref().map_or(u8::MAX, priority));
        let mut tx = Tx::new();
        let mut applied = 0;
        for f in todo {
            if self.apply_check_fix(&mut tx, f) {
                applied += 1;
            } else if tx.is_poisoned() {
                tx.cancel(&mut self.doc);
                return 0;
            }
        }
        if applied == 0 {
            tx.cancel(&mut self.doc);
            return 0;
        }
        if self.run_tx(tx) {
            applied
        } else {
            0
        }
    }

    fn apply_check_fix(&mut self, tx: &mut Tx, finding: &Finding) -> bool {
        let Some(fix) = &finding.fix else { return false };
        let id = finding.node;
        match fix {
            // A library asset is imported and then bound. When the bind is refused, the import goes
            // with it, so "Fix all" never commits an imported asset that nothing uses.
            Fix::BindVariable { .. } | Fix::BindVariables { .. } | Fix::ApplyStyle { .. } => {
                let mark = tx.mark();
                if self.import_and_link(tx, id, fix) {
                    return true;
                }
                if tx.rollback_to(&mut self.doc, mark).is_err() {
                    self.show_toast("Couldn't undo a partial import".to_string(), true);
                }
                false
            }
            Fix::SetTextColor { color } => {
                // The stored colour is what the user was shown; apply it only while the audit would
                // still suggest exactly that for this layer.
                let Some((paint, fresh)) = contrast::fresh_fix(&self.doc, id).filter(|(_, fresh)| fresh == color) else { return false };
                let Some(n) = tx.ed.node(&mut self.doc, id) else { return false };
                let NodeKind::Text(t) = &mut n.kind else { return false };
                set_text_fills_everywhere(t, paint, PaintKind::Solid(fresh))
            }
            Fix::Reconnect { component } => self.reconnect_detached(tx, id, *component),
        }
    }

    /// Imports the library asset a variable or style fix needs (when it is not in the document yet),
    /// then links the layer. False when either step is refused; the caller rolls back the import.
    fn import_and_link(&mut self, tx: &mut Tx, id: NodeId, fix: &Fix) -> bool {
        match fix {
            Fix::BindVariable { key, variable } => {
                if !self.doc.variables.contains_key(variable) && !self.import_library_variable_for_fix(tx, id, std::slice::from_ref(key), variable) {
                    return false;
                }
                let keys = hardcoded::keys_to_bind(&self.doc, id, key, variable);
                self.bind_keys(tx, id, keys.into_iter().map(str::to_string).collect(), variable)
            }
            Fix::BindVariables { keys, variable } => {
                if !self.doc.variables.contains_key(variable) && !self.import_library_variable_for_fix(tx, id, keys, variable) {
                    return false;
                }
                if !keys.iter().all(|k| hardcoded::bind_is_current(&self.doc, id, k, variable)) {
                    return false;
                }
                self.bind_keys(tx, id, keys.clone(), variable)
            }
            Fix::ApplyStyle { style } => {
                if !self.doc.styles.contains_key(style) && !self.import_library_style_for_fix(tx, id, style) {
                    return false;
                }
                if !hardcoded::style_is_current(&self.doc, id, style) {
                    return false;
                }
                self.apply_style_in(tx, &[id], style);
                true
            }
            _ => false,
        }
    }

    fn bind_keys(&mut self, tx: &mut Tx, id: NodeId, keys: Vec<String>, variable: &str) -> bool {
        if keys.is_empty() {
            return false;
        }
        let Some(value) = self.doc.resolve_variable(id, variable) else { return false };
        let Some(n) = tx.ed.node(&mut self.doc, id) else { return false };
        for k in &keys {
            n.variables.bindings.insert(k.clone(), variable.to_string());
            set_bound_property(n, k, &value);
        }
        true
    }

    fn import_pinned_asset_in(&mut self, tx: &mut Tx, library_id: &str, version: u32, key: &str) -> Option<String> {
        let ekey = local_key(library_id, key);
        if let Some(l) = self.doc.library.external.get(&ekey).and_then(|e| e.local_key.clone()) {
            return Some(l);
        }
        let snap = self.snapshot_of(library_id, version)?;
        let cmd = import_asset(&mut self.doc, &snap, key, None);
        if !is_noop(&cmd) && tx.cmd(&mut self.doc, cmd).is_err() {
            return None;
        }
        self.doc.library.external.get(&ekey).and_then(|e| e.local_key.clone())
    }

    fn import_library_variable_for_fix(&mut self, tx: &mut Tx, id: NodeId, keys: &[String], variable: &str) -> bool {
        let enabled = self.doc.library.enabled.clone();
        for r in enabled {
            let prefix = format!("{}/", r.id);
            let Some(raw) = variable.strip_prefix(&prefix) else { continue };
            let Some(snap) = self.snapshot_of(&r.id, r.version) else {
                self.show_toast(format!("Couldn't import \"{raw}\" from {}", r.name), true);
                return false;
            };
            if keys.is_empty() || !keys.iter().all(|k| hardcoded::library_bind_is_current(&self.doc, &snap, id, k, raw)) {
                return false;
            }
            let asset_name = snap.variables.get(raw).map(|v| v.name.clone()).unwrap_or_else(|| raw.to_string());
            let imported = self.import_pinned_asset_in(tx, &r.id, r.version, &format!("var:{raw}")).is_some();
            if imported {
                self.libs.invalidate();
            } else {
                self.show_toast(format!("Couldn't import \"{asset_name}\" from {}", r.name), true);
            }
            return imported;
        }
        false
    }

    fn import_library_style_for_fix(&mut self, tx: &mut Tx, id: NodeId, style: &str) -> bool {
        let enabled = self.doc.library.enabled.clone();
        for r in enabled {
            let prefix = format!("{}/", r.id);
            let Some(raw) = style.strip_prefix(&prefix) else { continue };
            let Some(snap) = self.snapshot_of(&r.id, r.version) else {
                self.show_toast(format!("Couldn't import \"{raw}\" from {}", r.name), true);
                return false;
            };
            if !hardcoded::library_style_is_current(&self.doc, &snap, id, raw) {
                return false;
            }
            let asset_name = snap.style_meta.get(raw).map(|m| m.name.clone()).unwrap_or_else(|| raw.to_string());
            let imported = self.import_pinned_asset_in(tx, &r.id, r.version, &format!("style:{raw}")).is_some();
            if imported {
                self.libs.invalidate();
            } else {
                self.show_toast(format!("Couldn't import \"{asset_name}\" from {}", r.name), true);
            }
            return imported;
        }
        false
    }

    /// Replaces the detached frame `id` with an instance of `component` in the same slot, keeping
    /// its name, size, transform, constraints and auto-layout child settings. Refuses unless the
    /// frame still equals the component apart from name and placement, so no edit is lost.
    fn reconnect_detached(&mut self, tx: &mut Tx, id: NodeId, component: NodeId) -> bool {
        let Some(old) = self.doc.get(id).cloned() else { return false };
        let still_detached = detached::symbol_of(&old).is_some_and(|s| detached::is_main_component(&self.doc, s, component));
        // A mirror inside an instance would be removed again by the next sync from its main.
        let Some(parent) = old.parent.filter(|_| still_detached && old.component.source.is_none() && detached::matches_component(&self.doc, id, component))
        else {
            return false;
        };
        let Some(index) = self.doc.get(parent).and_then(|p| p.children.iter().position(|c| *c == id)) else { return false };
        let cmd = designos_doc::create_instance(&mut self.doc, component, parent, Vec2::new(old.transform.tx, old.transform.ty));
        let Command::Insert { mut nodes, .. } = cmd else { return false };
        let Some(root) = nodes.first_mut() else { return false };
        root.name = old.name.clone();
        // The name is the layer's own. Without the override, a sync from the component (it runs
        // when the same step also edits the component) copies the component's name over it.
        root.component.overrides.insert(OverrideField::Name);
        root.size = old.size;
        root.transform = old.transform;
        root.constraints = old.constraints;
        root.layout_child = old.layout_child;
        let new_id = root.id;
        // Both steps or neither: a half-applied swap would leave a duplicate in a "Fix all".
        if let Err(e) = tx.cmds_all_or_none(&mut self.doc, vec![Command::Insert { parent, index, nodes }, Command::Remove { id }]) {
            self.show_toast(format!("Edit failed: {e}"), true);
            return false;
        }
        layout::relayout_around(&mut self.doc, &mut tx.ed, new_id);
        true
    }

    /// Reverts a single row of `self.check_designs.last_auto_fix` as one undo step labeled
    /// `Revert auto-fix: <rule> on <layer>`. Returns `false` if the row is missing or the node
    /// property has drifted since the fix (`can_revert_fix_row` is `false`).
    pub fn revert_auto_fix_row(&mut self, idx: usize) -> bool {
        let Some(row) = self.check_designs.last_auto_fix.as_ref().and_then(|r| r.rows.get(idx)).cloned() else {
            return false;
        };
        if !can_revert_fix_row(&self.doc, &row) {
            return false;
        }
        let mut tx = Tx::new();
        if !self.apply_revert_fix_row(&mut tx, &row) {
            tx.cancel(&mut self.doc);
            return false;
        }
        if !self.run_tx(tx) {
            return false;
        }
        self.hist.set_top_undo_label(format!("Revert auto-fix: {} on {}", row.rule, row.node_name));
        true
    }

    fn apply_revert_fix_row(&mut self, tx: &mut Tx, row: &super::FixRow) -> bool {
        let id = row.node;
        let prop = row.property.as_str();
        if matches!(
            prop,
            "layout_child.horizontal"
                | "horizontal_sizing"
                | "layout_child.vertical"
                | "vertical_sizing"
                | "layout.primary_sizing"
                | "auto_layout.primary_sizing"
                | "layout.counter_sizing"
                | "auto_layout.counter_sizing"
        ) {
            let Some(sizing) = parse_sizing_value(&row.before) else { return false };
            let Some(n) = tx.ed.node(&mut self.doc, id) else { return false };
            match prop {
                "layout_child.horizontal" | "horizontal_sizing" => n.layout_child.horizontal = sizing,
                "layout_child.vertical" | "vertical_sizing" => n.layout_child.vertical = sizing,
                "layout.primary_sizing" | "auto_layout.primary_sizing" => {
                    let Some(al) = n.kind.frame_props_mut().and_then(|f| f.auto_layout.as_mut()) else { return false };
                    al.primary_sizing = sizing;
                }
                _ => {
                    let Some(al) = n.kind.frame_props_mut().and_then(|f| f.auto_layout.as_mut()) else { return false };
                    al.counter_sizing = sizing;
                }
            }
            layout::relayout_around(&mut self.doc, &mut tx.ed, id);
            return true;
        }

        if let Some(ref_key) = style_ref_key(prop) {
            let Some(n) = tx.ed.node(&mut self.doc, id) else { return false };
            match row.before.as_str() {
                Some(s) if !s.is_empty() && !s.starts_with('#') => {
                    n.style_refs.insert(ref_key.to_string(), s.to_string());
                }
                _ => {
                    n.style_refs.remove(ref_key);
                }
            }
            return true;
        }

        let keys = expanded_binding_keys(prop);
        let Some(n) = tx.ed.node(&mut self.doc, id) else { return false };
        for &k in &keys {
            n.variables.bindings.remove(k);
        }
        if let Some(c) = parse_color_value(&row.before) {
            for &k in &keys {
                set_bound_property(n, k, &designos_doc::VariableValue::Color(c));
            }
            return true;
        }
        if let Some(num) = parse_number_value(&row.before) {
            for &k in &keys {
                set_bound_property(n, k, &designos_doc::VariableValue::Number(num));
            }
            layout::relayout_around(&mut self.doc, &mut tx.ed, id);
            return true;
        }
        false
    }
}

/// True when `row.node` still exists in `doc` and its property named by `row.property` still
/// holds `row.after` (so reverting `row` will not overwrite an edit made after the auto-fix).
pub fn can_revert_fix_row(doc: &designos_doc::Document, row: &super::FixRow) -> bool {
    let Some(n) = doc.get(row.node) else { return false };
    let prop = row.property.as_str();

    if matches!(
        prop,
        "layout_child.horizontal"
            | "horizontal_sizing"
            | "layout_child.vertical"
            | "vertical_sizing"
            | "layout.primary_sizing"
            | "auto_layout.primary_sizing"
            | "layout.counter_sizing"
            | "auto_layout.counter_sizing"
    ) {
        let Some(expected) = parse_sizing_value(&row.after) else { return false };
        let actual = match prop {
            "layout_child.horizontal" | "horizontal_sizing" => Some(n.layout_child.horizontal),
            "layout_child.vertical" | "vertical_sizing" => Some(n.layout_child.vertical),
            "layout.primary_sizing" | "auto_layout.primary_sizing" => n.kind.frame_props().and_then(|f| f.auto_layout.as_ref()).map(|al| al.primary_sizing),
            _ => n.kind.frame_props().and_then(|f| f.auto_layout.as_ref()).map(|al| al.counter_sizing),
        };
        return actual == Some(expected);
    }

    if let Some(ref_key) = style_ref_key(prop) {
        let Some(expected) = row.after.as_str() else { return false };
        let exp = expected.strip_prefix("style:").unwrap_or(expected);
        return n.style_refs.get(ref_key).is_some_and(|cur| cur == exp || doc.style_name(cur) == exp);
    }

    let keys = expanded_binding_keys(prop);
    if let Some(after_str) = row.after.as_str() {
        let var_ref = after_str.strip_prefix('$').unwrap_or(after_str);
        if after_str.starts_with('$') || doc.variables.values().any(|v| v.id == var_ref || v.name == var_ref) {
            for &k in &keys {
                let Some(bound_id) = n.variables.bindings.get(k) else { return false };
                let matches_var = bound_id == var_ref || doc.variables.get(bound_id).is_some_and(|v| v.id == var_ref || v.name == var_ref);
                if !matches_var {
                    return false;
                }
                if let Some(resolved) = doc.resolve_variable(n.id, bound_id) {
                    if !raw_property_matches(n, k, &resolved) {
                        return false;
                    }
                }
            }
            return true;
        }
    }

    if let Some(c) = parse_color_value(&row.after) {
        return keys.iter().all(|&k| !n.variables.bindings.contains_key(k) && raw_property_matches(n, k, &designos_doc::VariableValue::Color(c)));
    }
    if let Some(num) = parse_number_value(&row.after) {
        return keys.iter().all(|&k| !n.variables.bindings.contains_key(k) && raw_property_matches(n, k, &designos_doc::VariableValue::Number(num)));
    }
    false
}

fn expanded_binding_keys(prop: &str) -> Vec<&str> {
    match prop {
        "padding" => vec!["paddingTop", "paddingRight", "paddingBottom", "paddingLeft"],
        "cornerRadius" => vec!["topLeftRadius", "topRightRadius", "bottomRightRadius", "bottomLeftRadius"],
        "size.x" => vec!["width"],
        "size.y" => vec!["height"],
        other => vec![other],
    }
}

fn style_ref_key(prop: &str) -> Option<&'static str> {
    match prop {
        "fill_style" | "style_refs.fill" => Some("fill"),
        "stroke_style" | "style_refs.stroke" => Some("stroke"),
        "text_style" | "style_refs.text" => Some("text"),
        "effect_style" | "style_refs.effect" => Some("effect"),
        _ => None,
    }
}

fn parse_sizing_value(val: &serde_json::Value) -> Option<designos_doc::Sizing> {
    let s = val.as_str()?.trim();
    match s.to_ascii_lowercase().as_str() {
        "fixed" => Some(designos_doc::Sizing::Fixed),
        "hug" | "hug_contents" | "hugcontents" => Some(designos_doc::Sizing::Hug),
        "fill" | "fill_container" | "fillcontainer" => Some(designos_doc::Sizing::Fill),
        _ => None,
    }
}

fn parse_color_value(val: &serde_json::Value) -> Option<designos_doc::Color> {
    if let Some(s) = val.as_str() {
        let hex = s.trim().strip_prefix('#').unwrap_or(s.trim());
        if hex.len() == 6 {
            let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
            let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
            let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
            return Some(designos_doc::Color::rgba(r, g, b, 255));
        }
        if hex.len() == 8 {
            let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
            let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
            let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
            let a = u8::from_str_radix(&hex[6..8], 16).ok()?;
            return Some(designos_doc::Color::rgba(r, g, b, a));
        }
        return None;
    }
    if val.is_object() {
        return serde_json::from_value(val.clone()).ok();
    }
    None
}

fn parse_number_value(val: &serde_json::Value) -> Option<f32> {
    if let Some(n) = val.as_f64() {
        return Some(n as f32);
    }
    if let Some(s) = val.as_str() {
        let clean = s.trim().trim_end_matches("px").trim_end_matches('%').trim();
        return clean.parse::<f32>().ok();
    }
    None
}

fn raw_property_matches(n: &designos_doc::Node, key: &str, expected: &designos_doc::VariableValue) -> bool {
    match expected {
        designos_doc::VariableValue::Color(ec) => {
            if let Some(idx_str) = key.strip_prefix("fills/") {
                let Ok(idx) = idx_str.parse::<usize>() else { return false };
                let paints = match &n.kind {
                    NodeKind::Text(t) => &t.style.fills,
                    _ => &n.fills,
                };
                let Some(p) = paints.get(idx) else { return false };
                let PaintKind::Solid(c) = &p.kind else { return false };
                return c.to_hex().eq_ignore_ascii_case(&ec.to_hex());
            }
            if let Some(idx_str) = key.strip_prefix("strokes/") {
                let Ok(idx) = idx_str.parse::<usize>() else { return false };
                let Some(p) = n.stroke.paints.get(idx) else { return false };
                let PaintKind::Solid(c) = &p.kind else { return false };
                return c.to_hex().eq_ignore_ascii_case(&ec.to_hex());
            }
            false
        }
        designos_doc::VariableValue::Number(en) => {
            let actual = match key {
                "width" => Some(n.size.x),
                "height" => Some(n.size.y),
                "opacity" => Some(n.opacity * 100.0),
                "topLeftRadius" => Some(n.corner_radii[0]),
                "topRightRadius" => Some(n.corner_radii[1]),
                "bottomRightRadius" => Some(n.corner_radii[2]),
                "bottomLeftRadius" => Some(n.corner_radii[3]),
                "strokeWeight" => Some(n.stroke.weight),
                "strokeTopWeight" => n.stroke.side_weights.map(|s| s[0]),
                "strokeRightWeight" => n.stroke.side_weights.map(|s| s[1]),
                "strokeBottomWeight" => n.stroke.side_weights.map(|s| s[2]),
                "strokeLeftWeight" => n.stroke.side_weights.map(|s| s[3]),
                "itemSpacing" => n.kind.frame_props().and_then(|f| f.auto_layout.as_ref()).map(|al| al.item_spacing),
                "counterAxisSpacing" => n.kind.frame_props().and_then(|f| f.auto_layout.as_ref()).map(|al| al.counter_spacing),
                "paddingTop" => n.kind.frame_props().and_then(|f| f.auto_layout.as_ref()).map(|al| al.padding[0]),
                "paddingRight" => n.kind.frame_props().and_then(|f| f.auto_layout.as_ref()).map(|al| al.padding[1]),
                "paddingBottom" => n.kind.frame_props().and_then(|f| f.auto_layout.as_ref()).map(|al| al.padding[2]),
                "paddingLeft" => n.kind.frame_props().and_then(|f| f.auto_layout.as_ref()).map(|al| al.padding[3]),
                "fontSize" => match &n.kind {
                    NodeKind::Text(t) => Some(t.style.font_size),
                    _ => None,
                },
                "lineHeight" => match &n.kind {
                    NodeKind::Text(t) => match t.style.line_height {
                        designos_doc::LineHeight::Pixels(px) => Some(px),
                        _ => None,
                    },
                    _ => None,
                },
                _ => None,
            };
            actual.is_some_and(|a| (a - *en).abs() < 0.01)
        }
        designos_doc::VariableValue::Boolean(eb) => key == "visible" && n.visible == *eb,
        designos_doc::VariableValue::String(es) => match (&n.kind, key) {
            (NodeKind::Text(t), "characters") => &t.content == es,
            _ => false,
        },
        designos_doc::VariableValue::Alias(_) => false,
    }
}
