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
    let themes_order = extract_top_level_object_key_order(raw, "themes");
    let vars_order = extract_top_level_object_key_order(raw, "variables");

    // 1. Build one VariableCollection per theme axis in file order.
    let mut axis_to_col_id: BTreeMap<String, String> = BTreeMap::new();
    let mut ordered_col_ids: Vec<String> = Vec::new();

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
                VariableCollection { id: col_id.clone(), name: axis.clone(), modes, default_mode, variables: Vec::new(), key: None, source_library_key: None },
            );
            axis_to_col_id.insert(axis, col_id.clone());
            ordered_col_ids.push(col_id);
        }
    }

    let Some(vars_obj) = root.get("variables").and_then(Value::as_object) else {
        return;
    };
    if vars_obj.is_empty() {
        return;
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
            },
        );
        ordered_col_ids.push(col_id);
    }

    let first_col_id = ordered_col_ids[0].clone();

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

        // Determine which theme axes this variable switches on.
        let mut referenced_axes: Vec<String> = Vec::new();
        let mut has_multi_axis_entry = false;
        if let Some(arr) = val_node.and_then(Value::as_array) {
            for entry in arr {
                if let Some(theme_obj) = entry.get("theme").and_then(Value::as_object) {
                    if theme_obj.len() > 1 {
                        has_multi_axis_entry = true;
                    }
                    for axis_k in theme_obj.keys() {
                        if !referenced_axes.contains(axis_k) {
                            referenced_axes.push(axis_k.clone());
                        }
                    }
                }
            }
        }
        if referenced_axes.len() > 1 {
            has_multi_axis_entry = true;
        }

        let target_col_id = if referenced_axes.len() == 1 {
            axis_to_col_id.get(&referenced_axes[0]).cloned().unwrap_or_else(|| first_col_id.clone())
        } else {
            referenced_axes.first().and_then(|a| axis_to_col_id.get(a).cloned()).unwrap_or_else(|| first_col_id.clone())
        };

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
                            for (axis, tval) in theme_obj {
                                if let Some(tstr) = tval.as_str() {
                                    let mode_key = format!("{axis}:{tstr}");
                                    if axis_to_col_id.get(axis) == Some(&target_col_id) {
                                        values.entry(mode_key).or_insert_with(|| parsed.clone());
                                    }
                                }
                            }
                            if values.is_empty() {
                                if let Some((axis, tval)) = theme_obj.iter().next() {
                                    if let Some(tstr) = tval.as_str() {
                                        values.insert(format!("{axis}:{tstr}"), parsed);
                                    }
                                }
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

        let key = if has_multi_axis_entry { val_node.and_then(|v| serde_json::to_string(v).ok()).map(|s| format!("pen:multi:{s}")) } else { None };

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
                key,
                code_syntax: BTreeMap::new(),
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
    var.values.values().next()
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

pub fn export_themes_and_variables(doc: &Document, root: &mut Map<String, Value>) {
    if !doc.variable_collections.is_empty() {
        let mut themes = Map::new();
        for coll in doc.variable_collections.values() {
            let mut axis_map: BTreeMap<String, Vec<Value>> = BTreeMap::new();
            for m in &coll.modes {
                if let Some((axis, val)) = m.id.split_once(':') {
                    if axis != "mode" || coll.name == "mode" {
                        axis_map.entry(axis.to_string()).or_default().push(Value::String(val.to_string()));
                    } else if coll.modes.len() > 1 || m.name != "Default" {
                        axis_map.entry(coll.name.clone()).or_default().push(Value::String(m.name.clone()));
                    }
                } else if coll.modes.len() > 1 || m.name != "Default" {
                    axis_map.entry(coll.name.clone()).or_default().push(Value::String(m.name.clone()));
                }
            }
            for (axis, vals) in axis_map {
                themes.insert(axis, Value::Array(vals));
            }
        }
        if !themes.is_empty() {
            root.insert("themes".into(), Value::Object(themes));
        }
    }

    if !doc.variables.is_empty() {
        let mut vars = Map::new();
        // Follow collection variable order first, then any remaining variables.
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

        for vid in ordered_ids {
            let Some(var) = doc.variables.get(&vid) else { continue };
            let vtype = match var.kind {
                VariableKind::Color => "color",
                VariableKind::Number => "number",
                VariableKind::Boolean => "boolean",
                VariableKind::String => "string",
            };

            let is_single_default = (var.values.len() == 1 && var.values.contains_key("mode:Default"))
                || (var.values.len() == 1
                    && var
                        .values
                        .keys()
                        .next()
                        .and_then(|k| doc.variable_collections.get(&var.collection).map(|c| c.modes.len() == 1 && &c.default_mode == k))
                        .unwrap_or(false))
                || (var.values.len() <= 1 && !var.values.keys().any(|k| k.contains(':')));

            let value_json = if let Some(raw_multi) = var.key.as_deref().and_then(|k| k.strip_prefix("pen:multi:")) {
                serde_json::from_str::<Value>(raw_multi).unwrap_or(Value::Null)
            } else if is_single_default {
                var.values.values().next().map(var_val_to_json).unwrap_or(Value::Null)
            } else {
                // Emit themed array in collection mode order first, then any remaining entries.
                let mut arr = Vec::new();
                let mut emitted_modes = HashSet::new();
                if let Some(coll) = doc.variable_collections.get(&var.collection) {
                    for m in &coll.modes {
                        if let Some(val) = var.values.get(&m.id) {
                            emitted_modes.insert(m.id.clone());
                            let (axis, tval) = m.id.split_once(':').unwrap_or((&coll.name, &m.name));
                            let mut theme_obj = Map::new();
                            theme_obj.insert(axis.to_string(), Value::String(tval.to_string()));
                            arr.push(json!({
                                "theme": Value::Object(theme_obj),
                                "value": var_val_to_json(val),
                            }));
                        }
                    }
                }
                for (mode_id, val) in &var.values {
                    if !emitted_modes.contains(mode_id) {
                        let (axis, tval) = mode_id.split_once(':').unwrap_or(("mode", mode_id.as_str()));
                        let mut theme_obj = Map::new();
                        theme_obj.insert(axis.to_string(), Value::String(tval.to_string()));
                        arr.push(json!({
                            "theme": Value::Object(theme_obj),
                            "value": var_val_to_json(val),
                        }));
                    }
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
