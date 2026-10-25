//! Multi-axis `themes` and order-preserving `variables` import/export and token resolution for `.pen`.

use designos_doc::{Color, Document, Variable, VariableCollection, VariableKind, VariableMode, VariableValue};
use serde_json::{json, Map, Value};
use std::collections::{BTreeMap, HashSet};

/// Scans raw JSON text to extract the direct object keys of a top-level field (such as `"themes"` or
/// `"variables"`) in their exact file appearance order, without enabling `serde_json/preserve_order`
/// across the workspace.
pub fn extract_top_level_object_key_order(raw: &str, target_top_key: &str) -> Vec<String> {
    let bytes = raw.as_bytes();
    let len = bytes.len();
    let mut i = 0;

    fn skip_ws(bytes: &[u8], i: &mut usize) {
        while *i < bytes.len() && matches!(bytes[*i], b' ' | b'\t' | b'\r' | b'\n') {
            *i += 1;
        }
    }

    fn read_json_string(bytes: &[u8], i: &mut usize) -> Option<String> {
        if *i >= bytes.len() || bytes[*i] != b'"' {
            return None;
        }
        let start = *i;
        *i += 1;
        while *i < bytes.len() {
            match bytes[*i] {
                b'\\' => {
                    *i += 2;
                }
                b'"' => {
                    *i += 1;
                    let slice = std::str::from_utf8(&bytes[start..*i]).ok()?;
                    return serde_json::from_str::<String>(slice).ok();
                }
                _ => {
                    *i += 1;
                }
            }
        }
        None
    }

    fn skip_json_value(bytes: &[u8], i: &mut usize) {
        skip_ws(bytes, i);
        if *i >= bytes.len() {
            return;
        }
        match bytes[*i] {
            b'"' => {
                let _ = read_json_string(bytes, i);
            }
            b'{' | b'[' => {
                let mut depth = 0usize;
                while *i < bytes.len() {
                    match bytes[*i] {
                        b'"' => {
                            let _ = read_json_string(bytes, i);
                            continue;
                        }
                        b'{' | b'[' => depth += 1,
                        b'}' | b']' => {
                            depth = depth.saturating_sub(1);
                            if depth == 0 {
                                *i += 1;
                                return;
                            }
                        }
                        _ => {}
                    }
                    *i += 1;
                }
            }
            _ => {
                while *i < bytes.len() && !matches!(bytes[*i], b',' | b'}' | b']' | b' ' | b'\t' | b'\r' | b'\n') {
                    *i += 1;
                }
            }
        }
    }

    skip_ws(bytes, &mut i);
    if i >= len || bytes[i] != b'{' {
        return Vec::new();
    }
    i += 1;

    while i < len {
        skip_ws(bytes, &mut i);
        if i >= len || bytes[i] == b'}' {
            break;
        }
        if bytes[i] == b',' {
            i += 1;
            continue;
        }
        let Some(top_key) = read_json_string(bytes, &mut i) else {
            break;
        };
        skip_ws(bytes, &mut i);
        if i >= len || bytes[i] != b':' {
            break;
        }
        i += 1;
        skip_ws(bytes, &mut i);

        if top_key == target_top_key && i < len && bytes[i] == b'{' {
            i += 1;
            let mut keys = Vec::new();
            while i < len {
                skip_ws(bytes, &mut i);
                if i >= len || bytes[i] == b'}' {
                    break;
                }
                if bytes[i] == b',' {
                    i += 1;
                    continue;
                }
                let Some(inner_key) = read_json_string(bytes, &mut i) else {
                    break;
                };
                keys.push(inner_key);
                skip_ws(bytes, &mut i);
                if i >= len || bytes[i] != b':' {
                    break;
                }
                i += 1;
                skip_json_value(bytes, &mut i);
            }
            return keys;
        } else {
            skip_json_value(bytes, &mut i);
        }
    }

    Vec::new()
}

pub fn import_pen_variables(raw: &str, root: &Value, doc: &mut Document) {
    let themes_order =
        if root.get("themes").and_then(Value::as_object).is_some_and(|m| m.len() > 1) { extract_top_level_object_key_order(raw, "themes") } else { Vec::new() };

    // 1. Build one VariableCollection per theme axis in file order.
    let mut axis_to_col_id: BTreeMap<String, String> = BTreeMap::new();
    let mut ordered_col_ids: Vec<String> = Vec::new();
    let mut ordered_axes: Vec<String> = Vec::new();

    if let Some(themes) = root.get("themes").and_then(Value::as_object) {
        let mut axis_names: Vec<String> = Vec::new();
        for k in &themes_order {
            if themes.contains_key(k) && !axis_names.contains(k) {
                axis_names.push(k.clone());
            }
        }
        for k in themes.keys() {
            if !axis_names.contains(k) {
                axis_names.push(k.clone());
            }
        }

        for (idx, axis) in axis_names.into_iter().enumerate() {
            let Some(arr) = themes.get(&axis).and_then(Value::as_array) else { continue };
            let mut modes = Vec::new();
            for v in arr {
                if let Some(val_str) = v.as_str() {
                    modes.push(VariableMode { id: format!("{axis}:{val_str}"), name: val_str.to_string() });
                }
            }
            if modes.is_empty() {
                continue;
            }
            let default_mode = modes[0].id.clone();
            let col_id = format!("pen:axis:{idx:04}:{axis}");
            doc.variable_collections.insert(
                col_id.clone(),
                VariableCollection {
                    id: col_id.clone(),
                    name: axis.clone(),
                    modes,
                    default_mode,
                    variables: Vec::new(),
                    key: None,
                    source_library_key: None,
                    extensions: Default::default(),
                },
            );
            axis_to_col_id.insert(axis.clone(), col_id.clone());
            ordered_col_ids.push(col_id);
            ordered_axes.push(axis);
        }
    }

    let Some(vars_obj) = root.get("variables").and_then(Value::as_object) else {
        return;
    };
    if vars_obj.is_empty() {
        return;
    }

    let vars_order = if vars_obj.len() > 1 { extract_top_level_object_key_order(raw, "variables") } else { Vec::new() };

    let mut ordered_var_names: Vec<String> = Vec::new();
    for k in &vars_order {
        if vars_obj.contains_key(k) && !ordered_var_names.contains(k) {
            ordered_var_names.push(k.clone());
        }
    }
    for k in vars_obj.keys() {
        if !ordered_var_names.contains(k) {
            ordered_var_names.push(k.clone());
        }
    }

    // Ensure any axis or mode referenced in `variables` that was omitted from `themes` is registered.
    for var_name in &ordered_var_names {
        let Some(vdef) = vars_obj.get(var_name) else { continue };
        let Some(arr) = vdef.get("value").and_then(Value::as_array) else { continue };
        for entry in arr {
            let Some(theme_obj) = entry.get("theme").and_then(Value::as_object) else { continue };
            for (axis, tval) in theme_obj {
                let Some(tstr) = tval.as_str() else { continue };
                let mode_id = format!("{axis}:{tstr}");
                let col_id = if let Some(existing) = axis_to_col_id.get(axis) {
                    existing.clone()
                } else {
                    let idx = ordered_col_ids.len();
                    let cid = format!("pen:axis:{idx:04}:{axis}");
                    doc.variable_collections.insert(
                        cid.clone(),
                        VariableCollection {
                            id: cid.clone(),
                            name: axis.clone(),
                            modes: vec![VariableMode { id: mode_id.clone(), name: tstr.to_string() }],
                            default_mode: mode_id.clone(),
                            variables: Vec::new(),
                            key: None,
                            source_library_key: None,
                            extensions: Default::default(),
                        },
                    );
                    axis_to_col_id.insert(axis.clone(), cid.clone());
                    ordered_col_ids.push(cid.clone());
                    ordered_axes.push(axis.clone());
                    cid
                };
                if let Some(col) = doc.variable_collections.get_mut(&col_id) {
                    if !col.modes.iter().any(|m| m.id == mode_id) {
                        col.modes.push(VariableMode { id: mode_id, name: tstr.to_string() });
                    }
                }
            }
        }
    }

    if ordered_col_ids.is_empty() {
        let col_id = "pen:axis:0000:Theme".to_string();
        doc.variable_collections.insert(
            col_id.clone(),
            VariableCollection {
                id: col_id.clone(),
                name: "Theme".into(),
                modes: vec![VariableMode { id: "mode:Default".into(), name: "Default".into() }],
                default_mode: "mode:Default".into(),
                variables: Vec::new(),
                key: None,
                source_library_key: None,
                extensions: Default::default(),
            },
        );
        ordered_col_ids.push(col_id);
        ordered_axes.push("Theme".into());
    }

    let first_col_id = ordered_col_ids[0].clone();

    for var_name in ordered_var_names {
        let Some(vdef) = vars_obj.get(&var_name) else { continue };
        let vtype = vdef.get("type").and_then(Value::as_str).unwrap_or("string");
        let kind = match vtype {
            "color" => VariableKind::Color,
            "number" | "float" => VariableKind::Number,
            "boolean" => VariableKind::Boolean,
            _ => VariableKind::String,
        };

        let val_node = vdef.get("value");

        // Determine which theme axes this variable switches on, ordered by `ordered_axes`.
        let mut raw_referenced_axes: HashSet<String> = HashSet::new();
        if let Some(arr) = val_node.and_then(Value::as_array) {
            for entry in arr {
                if let Some(theme_obj) = entry.get("theme").and_then(Value::as_object) {
                    for axis_k in theme_obj.keys() {
                        raw_referenced_axes.insert(axis_k.clone());
                    }
                }
            }
        }
        let mut referenced_axes: Vec<String> = ordered_axes.iter().filter(|a| raw_referenced_axes.contains(*a)).cloned().collect();
        for axis_k in raw_referenced_axes {
            if !referenced_axes.contains(&axis_k) {
                referenced_axes.push(axis_k);
            }
        }

        let target_col_id = referenced_axes.first().and_then(|a| axis_to_col_id.get(a).cloned()).unwrap_or_else(|| first_col_id.clone());

        let default_mode_id = doc.variable_collections.get(&target_col_id).map(|c| c.default_mode.clone()).unwrap_or_else(|| "mode:Default".into());

        let mut values = BTreeMap::new();
        match val_node {
            Some(Value::Array(arr)) => {
                for entry in arr {
                    let Some(raw_val) = entry.get("value") else { continue };
                    let Some(parsed) = parse_pen_var_value(kind, raw_val) else { continue };
                    if let Some(theme_obj) = entry.get("theme").and_then(Value::as_object) {
                        if theme_obj.is_empty() {
                            values.entry(default_mode_id.clone()).or_insert(parsed);
                        } else {
                            // Build key in `ordered_axes` order (`axis:val` for 1 axis, `axis1:val1+axis2:val2` for multi-axis).
                            let mut parts: Vec<String> = Vec::new();
                            for axis in &ordered_axes {
                                if let Some(tstr) = theme_obj.get(axis).and_then(Value::as_str) {
                                    parts.push(format!("{axis}:{tstr}"));
                                }
                            }
                            for (axis, tval) in theme_obj {
                                if !ordered_axes.contains(axis) {
                                    if let Some(tstr) = tval.as_str() {
                                        parts.push(format!("{axis}:{tstr}"));
                                    }
                                }
                            }
                            if !parts.is_empty() {
                                let mode_key = parts.join("+");
                                values.entry(mode_key).or_insert(parsed);
                            }
                        }
                    } else {
                        values.entry(default_mode_id.clone()).or_insert(parsed);
                    }
                }
            }
            Some(single) => {
                if let Some(parsed) = parse_pen_var_value(kind, single) {
                    values.insert(default_mode_id.clone(), parsed);
                }
            }
            None => {}
        }

        let var_id = var_name.clone();
        doc.variables.insert(
            var_id.clone(),
            Variable {
                id: var_id.clone(),
                name: var_name,
                collection: target_col_id.clone(),
                kind,
                values,
                description: vdef.get("description").and_then(Value::as_str).unwrap_or("").to_string(),
                scopes: Vec::new(),
                hidden_from_publishing: false,
                key: None,
                code_syntax: BTreeMap::new(),
                extensions: Default::default(),
                mode_extensions: Default::default(),
            },
        );

        if let Some(col) = doc.variable_collections.get_mut(&target_col_id) {
            col.variables.push(var_id);
        }
    }
}

pub fn parse_pen_var_value(kind: VariableKind, v: &Value) -> Option<VariableValue> {
    if let Some(s) = v.as_str() {
        if let Some(alias) = s.strip_prefix('$') {
            return Some(VariableValue::Alias(alias.to_string()));
        }
    }
    match kind {
        VariableKind::Color => v.as_str().and_then(Color::from_hex).map(VariableValue::Color),
        VariableKind::Number => {
            v.as_f64().map(|n| VariableValue::Number(n as f32)).or_else(|| v.as_str().and_then(|s| s.parse::<f32>().ok()).map(VariableValue::Number))
        }
        VariableKind::Boolean => v.as_bool().map(VariableValue::Boolean),
        VariableKind::String => v.as_str().map(|s| VariableValue::String(s.to_string())),
    }
}

fn default_or_first_var_value<'a>(doc: &'a Document, var: &'a Variable) -> Option<&'a VariableValue> {
    if let Some(col) = doc.variable_collections.get(&var.collection) {
        if let Some(v) = var.values.get(&col.default_mode) {
            return Some(v);
        }
    }
    // Check multi-axis composite keys where every segment matches its collection's default_mode.
    for (k, val) in &var.values {
        if !k.contains('+') {
            continue;
        }
        let all_match = k.split('+').all(|seg| {
            let Some((axis, _)) = seg.split_once(':') else {
                return false;
            };
            doc.variable_collections.values().find(|c| c.name == axis || c.modes.iter().any(|m| m.id == seg)).map(|c| c.default_mode == seg).unwrap_or(false)
        });
        if all_match {
            return Some(val);
        }
    }
    var.values.values().next()
}

/// Applies bound variable values across all nodes in `doc` using each node's effective theme modes
/// (`Document::resolve_variable`), so node-level `"theme"` overrides immediately reflect on
/// concrete node properties (`fills`, `stroke`, `size`, `auto_layout`, `text`, etc.).
pub fn apply_pen_bound_variables(doc: &mut Document) {
    if doc.variables.is_empty() {
        return;
    }
    let ids: Vec<designos_doc::NodeId> = doc.nodes.keys().copied().collect();
    for id in ids {
        let bindings = match doc.get(id) {
            Some(n) if !n.variables.bindings.is_empty() => n.variables.bindings.clone(),
            _ => continue,
        };
        let resolved_pairs: Vec<(String, VariableValue)> =
            bindings.into_iter().filter_map(|(k, var_id)| doc.resolve_variable(id, &var_id).map(|v| (k, v))).collect();
        if resolved_pairs.is_empty() {
            continue;
        }
        if let Some(n) = doc.get_mut(id) {
            for (k, v) in resolved_pairs {
                match k.as_str() {
                    "opacity" => {
                        if let VariableValue::Number(x) = v {
                            n.opacity = if x <= 1.0 { x.clamp(0.0, 1.0) } else { (x / 100.0).clamp(0.0, 1.0) };
                        }
                    }
                    "cornerRadius" => {
                        if let VariableValue::Number(x) = v {
                            n.corner_radii = [x.max(0.0); 4];
                        }
                    }
                    _ => designos_doc::set_bound_property(n, &k, &v),
                }
            }
        }
    }
}

pub fn resolve_doc_var(doc: &Document, var_ref: &str) -> Option<f32> {
    let mut name = var_ref.strip_prefix('$').unwrap_or(var_ref);
    let mut visited = HashSet::new();
    while visited.insert(name) {
        let var = doc.variables.get(name)?;
        match default_or_first_var_value(doc, var)? {
            VariableValue::Number(n) => return Some(*n),
            VariableValue::Alias(next) => name = next.strip_prefix('$').unwrap_or(next),
            _ => return None,
        }
    }
    None
}

pub fn resolve_doc_var_color(doc: &Document, var_ref: &str) -> Option<Color> {
    let mut name = var_ref.strip_prefix('$').unwrap_or(var_ref);
    let mut visited = HashSet::new();
    while visited.insert(name) {
        let var = doc.variables.get(name)?;
        match default_or_first_var_value(doc, var)? {
            VariableValue::Color(c) => return Some(*c),
            VariableValue::Alias(next) => name = next.strip_prefix('$').unwrap_or(next),
            _ => return None,
        }
    }
    None
}

pub fn resolve_doc_var_string(doc: &Document, var_ref: &str) -> Option<String> {
    let mut name = var_ref.strip_prefix('$').unwrap_or(var_ref);
    let mut visited = HashSet::new();
    while visited.insert(name) {
        let var = doc.variables.get(name)?;
        match default_or_first_var_value(doc, var)? {
            VariableValue::String(s) => return Some(s.clone()),
            VariableValue::Alias(next) => name = next.strip_prefix('$').unwrap_or(next),
            _ => return None,
        }
    }
    None
}

pub fn resolve_doc_var_bool(doc: &Document, var_ref: &str) -> Option<bool> {
    let mut name = var_ref.strip_prefix('$').unwrap_or(var_ref);
    let mut visited = HashSet::new();
    while visited.insert(name) {
        let var = doc.variables.get(name)?;
        match default_or_first_var_value(doc, var)? {
            VariableValue::Boolean(b) => return Some(*b),
            VariableValue::Alias(next) => name = next.strip_prefix('$').unwrap_or(next),
            _ => return None,
        }
    }
    None
}

fn ordered_themes_entries(doc: &Document) -> Vec<(String, Vec<String>)> {
    let mut out: Vec<(String, Vec<String>)> = Vec::new();
    for coll in doc.variable_collections.values() {
        let mut axis_map: Vec<(String, Vec<String>)> = Vec::new();
        for m in &coll.modes {
            let push_to = |axis_map: &mut Vec<(String, Vec<String>)>, axis: String, val: String| {
                if let Some((_, vec)) = axis_map.iter_mut().find(|(a, _)| *a == axis) {
                    vec.push(val);
                } else {
                    axis_map.push((axis, vec![val]));
                }
            };
            if let Some((axis, val)) = m.id.split_once(':') {
                if axis != "mode" || coll.name == "mode" {
                    push_to(&mut axis_map, axis.to_string(), val.to_string());
                } else if coll.modes.len() > 1 || m.name != "Default" {
                    push_to(&mut axis_map, coll.name.clone(), m.name.clone());
                }
            } else if coll.modes.len() > 1 || m.name != "Default" {
                push_to(&mut axis_map, coll.name.clone(), m.name.clone());
            }
        }
        for (axis, vals) in axis_map {
            if let Some((_, existing)) = out.iter_mut().find(|(a, _)| *a == axis) {
                for v in vals {
                    if !existing.contains(&v) {
                        existing.push(v);
                    }
                }
            } else {
                out.push((axis, vals));
            }
        }
    }
    out
}

fn ordered_variable_ids(doc: &Document) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut ordered_ids: Vec<String> = Vec::new();
    for coll in doc.variable_collections.values() {
        for vid in &coll.variables {
            if doc.variables.contains_key(vid) && seen.insert(vid.clone()) {
                ordered_ids.push(vid.clone());
            }
        }
    }
    for vid in doc.variables.keys() {
        if seen.insert(vid.clone()) {
            ordered_ids.push(vid.clone());
        }
    }
    ordered_ids
}

fn mode_key_sort_rank(doc: &Document, mode_key: &str) -> Vec<(usize, usize)> {
    let colls: Vec<&VariableCollection> = doc.variable_collections.values().collect();
    mode_key
        .split('+')
        .map(|seg| {
            let (axis, _) = seg.split_once(':').unwrap_or(("", seg));
            for (c_idx, coll) in colls.iter().enumerate() {
                if let Some(m_idx) = coll.modes.iter().position(|m| m.id == seg) {
                    return (c_idx, m_idx);
                }
                if coll.name == axis {
                    return (c_idx, usize::MAX);
                }
            }
            (usize::MAX, usize::MAX)
        })
        .collect()
}

fn build_variable_themed_entries<'a>(doc: &Document, var: &'a Variable) -> Vec<(Vec<(String, String)>, &'a VariableValue)> {
    let mut entries: Vec<(&String, &VariableValue)> = var.values.iter().collect();
    entries.sort_by_key(|(k, _)| mode_key_sort_rank(doc, k));
    entries
        .into_iter()
        .map(|(mode_id, val)| {
            let pairs: Vec<(String, String)> = mode_id
                .split('+')
                .map(|seg| {
                    let (axis, tval) = seg.split_once(':').unwrap_or(("mode", seg));
                    (axis.to_string(), tval.to_string())
                })
                .collect();
            (pairs, val)
        })
        .collect()
}

fn is_single_default_var(doc: &Document, var: &Variable) -> bool {
    (var.values.len() == 1 && var.values.contains_key("mode:Default"))
        || (var.values.len() == 1
            && var
                .values
                .keys()
                .next()
                .and_then(|k| doc.variable_collections.get(&var.collection).map(|c| !k.contains('+') && c.modes.len() == 1 && &c.default_mode == k))
                .unwrap_or(false))
        || (var.values.len() <= 1 && !var.values.keys().any(|k| k.contains(':')))
}

pub fn export_themes_and_variables(doc: &Document, root: &mut Map<String, Value>) {
    let theme_entries = ordered_themes_entries(doc);
    if !theme_entries.is_empty() {
        let mut themes = Map::new();
        for (axis, vals) in theme_entries {
            themes.insert(axis, Value::Array(vals.into_iter().map(Value::String).collect()));
        }
        root.insert("themes".into(), Value::Object(themes));
    }

    if !doc.variables.is_empty() {
        let mut vars = Map::new();
        for vid in ordered_variable_ids(doc) {
            let Some(var) = doc.variables.get(&vid) else { continue };
            let vtype = match var.kind {
                VariableKind::Color => "color",
                VariableKind::Number => "number",
                VariableKind::Boolean => "boolean",
                VariableKind::String => "string",
            };

            let value_json = if is_single_default_var(doc, var) {
                var.values.values().next().map(var_val_to_json).unwrap_or(Value::Null)
            } else {
                let mut arr = Vec::new();
                for (pairs, val) in build_variable_themed_entries(doc, var) {
                    let mut theme_obj = Map::new();
                    for (axis, tval) in pairs {
                        theme_obj.insert(axis, Value::String(tval));
                    }
                    arr.push(json!({
                        "theme": Value::Object(theme_obj),
                        "value": var_val_to_json(val),
                    }));
                }
                Value::Array(arr)
            };

            let mut vobj = Map::new();
            vobj.insert("type".into(), Value::String(vtype.into()));
            vobj.insert("value".into(), value_json);
            if !var.description.is_empty() {
                vobj.insert("description".into(), Value::String(var.description.clone()));
            }
            vars.insert(var.name.clone(), Value::Object(vobj));
        }
        root.insert("variables".into(), Value::Object(vars));
    }
}

/// Replaces the top-level `"themes"` and `"variables"` objects in `pretty_json` with blocks
/// whose keys are emitted in exact `doc.variable_collections` and `col.variables` order rather
/// than `BTreeMap` alphabetical order.
pub fn splice_ordered_themes_and_variables(doc: &Document, mut pretty_json: String) -> String {
    let theme_entries = ordered_themes_entries(doc);
    if !theme_entries.is_empty() {
        let mut lines = Vec::new();
        lines.push("{\n".to_string());
        for (i, (axis, vals)) in theme_entries.iter().enumerate() {
            let axis_json = serde_json::to_string(axis).unwrap_or_else(|_| format!("\"{axis}\""));
            let vals_json = serde_json::to_string(vals).unwrap_or_else(|_| "[]".into());
            let comma = if i + 1 < theme_entries.len() { "," } else { "" };
            lines.push(format!("    {axis_json}: {vals_json}{comma}\n"));
        }
        lines.push("  }".to_string());
        let replacement = lines.concat();
        pretty_json = replace_top_level_field_value(&pretty_json, "themes", &replacement);
    }

    if !doc.variables.is_empty() {
        let ordered_ids = ordered_variable_ids(doc);
        let mut lines = Vec::new();
        lines.push("{\n".to_string());
        for (idx, vid) in ordered_ids.iter().enumerate() {
            let Some(var) = doc.variables.get(vid) else { continue };
            let vtype = match var.kind {
                VariableKind::Color => "color",
                VariableKind::Number => "number",
                VariableKind::Boolean => "boolean",
                VariableKind::String => "string",
            };
            let name_json = serde_json::to_string(&var.name).unwrap_or_else(|_| format!("\"{}\"", var.name));
            lines.push(format!("    {name_json}: {{\n"));
            if !var.description.is_empty() {
                let desc_json = serde_json::to_string(&var.description).unwrap_or_default();
                lines.push(format!("      \"description\": {desc_json},\n"));
            }
            lines.push(format!("      \"type\": \"{vtype}\",\n"));
            if is_single_default_var(doc, var) {
                let val_json = var.values.values().next().map(var_val_to_json).and_then(|v| serde_json::to_string(&v).ok()).unwrap_or_else(|| "null".into());
                lines.push(format!("      \"value\": {val_json}\n"));
            } else {
                let themed_entries = build_variable_themed_entries(doc, var);
                lines.push("      \"value\": [\n".to_string());
                for (e_idx, (pairs, val)) in themed_entries.iter().enumerate() {
                    let theme_parts: Vec<String> = pairs
                        .iter()
                        .map(|(a, tv)| {
                            let aj = serde_json::to_string(a).unwrap_or_default();
                            let tvj = serde_json::to_string(tv).unwrap_or_default();
                            format!("{aj}: {tvj}")
                        })
                        .collect();
                    let val_json = serde_json::to_string(&var_val_to_json(val)).unwrap_or_else(|_| "null".into());
                    let e_comma = if e_idx + 1 < themed_entries.len() { "," } else { "" };
                    lines.push(format!("        {{ \"theme\": {{ {} }}, \"value\": {val_json} }}{e_comma}\n", theme_parts.join(", ")));
                }
                lines.push("      ]\n".to_string());
            }
            let comma = if idx + 1 < ordered_ids.len() { "," } else { "" };
            lines.push(format!("    }}{comma}\n"));
        }
        lines.push("  }".to_string());
        let replacement = lines.concat();
        pretty_json = replace_top_level_field_value(&pretty_json, "variables", &replacement);
    }

    pretty_json
}

fn replace_top_level_field_value(raw: &str, target_top_key: &str, replacement_value: &str) -> String {
    let bytes = raw.as_bytes();
    let len = bytes.len();
    let mut i = 0;

    let skip_ws = |i: &mut usize| {
        while *i < len && matches!(bytes[*i], b' ' | b'\t' | b'\r' | b'\n') {
            *i += 1;
        }
    };
    let read_str = |i: &mut usize| -> Option<String> {
        if *i >= len || bytes[*i] != b'"' {
            return None;
        }
        let start = *i;
        *i += 1;
        while *i < len {
            match bytes[*i] {
                b'\\' => *i += 2,
                b'"' => {
                    *i += 1;
                    let s = std::str::from_utf8(&bytes[start..*i]).ok()?;
                    return serde_json::from_str::<String>(s).ok();
                }
                _ => *i += 1,
            }
        }
        None
    };
    let skip_val = |i: &mut usize| {
        skip_ws(i);
        if *i >= len {
            return;
        }
        match bytes[*i] {
            b'"' => {
                let _ = read_str(i);
            }
            b'{' | b'[' => {
                let mut depth = 0usize;
                while *i < len {
                    match bytes[*i] {
                        b'"' => {
                            let _ = read_str(i);
                            continue;
                        }
                        b'{' | b'[' => depth += 1,
                        b'}' | b']' => {
                            depth = depth.saturating_sub(1);
                            if depth == 0 {
                                *i += 1;
                                return;
                            }
                        }
                        _ => {}
                    }
                    *i += 1;
                }
            }
            _ => {
                while *i < len && !matches!(bytes[*i], b',' | b'}' | b']' | b' ' | b'\t' | b'\r' | b'\n') {
                    *i += 1;
                }
            }
        }
    };

    skip_ws(&mut i);
    if i >= len || bytes[i] != b'{' {
        return raw.to_string();
    }
    i += 1;
    while i < len {
        skip_ws(&mut i);
        if i >= len || bytes[i] == b'}' {
            break;
        }
        if bytes[i] == b',' {
            i += 1;
            continue;
        }
        let Some(k) = read_str(&mut i) else { break };
        skip_ws(&mut i);
        if i >= len || bytes[i] != b':' {
            break;
        }
        i += 1;
        skip_ws(&mut i);
        let val_start = i;
        skip_val(&mut i);
        let val_end = i;
        if k == target_top_key {
            let mut out = String::with_capacity(raw.len() + replacement_value.len());
            out.push_str(&raw[..val_start]);
            out.push_str(replacement_value);
            out.push_str(&raw[val_end..]);
            return out;
        }
    }
    raw.to_string()
}

pub fn var_val_to_json(v: &VariableValue) -> Value {
    match v {
        VariableValue::Color(c) => {
            if c.a < 1.0 - 1e-4 {
                let [r, g, b, a] = c.to_u8();
                Value::String(format!("#{r:02X}{g:02X}{b:02X}{a:02X}"))
            } else {
                Value::String(format!("#{}", c.to_hex()))
            }
        }
        VariableValue::Number(n) => json!(*n),
        VariableValue::Boolean(b) => json!(*b),
        VariableValue::String(s) => json!(s),
        VariableValue::Alias(a) => json!(format!("${a}")),
    }
}
