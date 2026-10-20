//! Figma variables: REST `GET /v1/files/:key/variables/local` payloads, node
//! `boundVariables` / `explicitVariableModes`, and a tolerant `.fig` mapping.
//!
//! Ids are kept as Figma writes them (`VariableID:1:23`, mode ids `1:0`), so
//! exporting gives back the same references.

use crate::rest_import::{color, farr, fbool, fstr};
use crate::FigError;
use designos_doc::*;
use serde_json::{json, Map, Value};
use std::collections::BTreeMap;

fn is_alias(v: &Value) -> bool {
    fstr(v, "type") == Some("VARIABLE_ALIAS")
}

/// Finds the `{variables, variableCollections}` maps in a REST response
/// (`{meta: {...}}`), a bare meta object, or a file with a top-level
/// `variables` key holding either.
fn find_payload(v: &Value) -> Option<&Value> {
    let has_maps = |x: &Value| x.get("variableCollections").is_some_and(Value::is_object);
    if let Some(meta) = v.get("meta").filter(|m| m.get("variables").is_some() || m.get("variableCollections").is_some()) {
        return Some(meta);
    }
    if let Some(inner) = v.get("variables").filter(|x| x.is_object()) {
        if let Some(meta) = inner.get("meta").filter(|m| m.is_object()) {
            return Some(meta);
        }
        if inner.get("variableCollections").is_some() || inner.get("variables").is_some() {
            return Some(inner);
        }
    }
    if has_maps(v) {
        return Some(v);
    }
    None
}

fn parse_value(ty: &str, v: &Value) -> Option<VariableValue> {
    if is_alias(v) {
        return Some(VariableValue::Alias(fstr(v, "id")?.to_string()));
    }
    Some(match ty {
        "COLOR" => VariableValue::Color(color(v)?),
        "FLOAT" => VariableValue::Number(v.as_f64()? as f32),
        "STRING" => VariableValue::String(v.as_str()?.to_string()),
        "BOOLEAN" => VariableValue::Boolean(v.as_bool()?),
        _ => return None,
    })
}

fn kind_of(ty: &str) -> Option<VariableKind> {
    Some(match ty {
        "COLOR" => VariableKind::Color,
        "FLOAT" => VariableKind::Number,
        "STRING" => VariableKind::String,
        "BOOLEAN" => VariableKind::Boolean,
        _ => return None,
    })
}

/// REST `codeSyntax`: `{WEB?, ANDROID?, iOS?}` with non-empty strings.
fn parse_code_syntax(v: Option<&Value>) -> CodeSyntax {
    let Some(o) = v.and_then(Value::as_object) else { return CodeSyntax::new() };
    o.iter().filter_map(|(label, code)| Some((CodeSyntaxPlatform::from_label(label)?, code.as_str().filter(|c| !c.is_empty())?.to_string()))).collect()
}

fn code_syntax_json(c: &CodeSyntax) -> Value {
    Value::Object(c.iter().map(|(p, code)| (p.label().to_string(), Value::String(code.clone()))).collect())
}

fn type_name(k: VariableKind) -> &'static str {
    match k {
        VariableKind::Color => "COLOR",
        VariableKind::Number => "FLOAT",
        VariableKind::String => "STRING",
        VariableKind::Boolean => "BOOLEAN",
    }
}

/// The shared library a collection is a subscribed copy of. `.fig` files and our own export write
/// `sourceLibraryKey`; a REST `variables/local` response flags such collections `remote: true` and
/// names no library, so the collection's own key stands in.
fn library_key_of(c: &Value) -> Option<String> {
    let explicit = fstr(c, "sourceLibraryKey").filter(|k| !k.is_empty());
    let remote = fbool(c, "remote") == Some(true);
    explicit.or_else(|| remote.then(|| fstr(c, "key").or_else(|| fstr(c, "id")).unwrap_or("remote"))).map(String::from)
}

/// Maps `meta.variableCollections` / `meta.variables` into `doc`, replacing
/// whatever it had. Returns the number of variables imported.
pub fn import_variables_value(doc: &mut Document, v: &Value) -> usize {
    let Some(p) = find_payload(v) else { return 0 };
    let mut collections: BTreeMap<String, VariableCollection> = BTreeMap::new();
    let mut variables: BTreeMap<String, Variable> = BTreeMap::new();
    let empty = Map::new();
    let colls = p.get("variableCollections").and_then(Value::as_object).unwrap_or(&empty);
    for (key, c) in colls {
        let id = fstr(c, "id").unwrap_or(key).to_string();
        let modes: Vec<VariableMode> = farr(c, "modes")
            .iter()
            .filter_map(|m| Some(VariableMode { id: fstr(m, "modeId")?.to_string(), name: fstr(m, "name").unwrap_or("Mode").to_string() }))
            .collect();
        let default_mode = fstr(c, "defaultModeId").map(String::from).or_else(|| modes.first().map(|m| m.id.clone())).unwrap_or_default();
        collections.insert(
            id.clone(),
            VariableCollection {
                id,
                name: fstr(c, "name").unwrap_or("Collection").to_string(),
                modes,
                default_mode,
                variables: farr(c, "variableIds").iter().filter_map(|s| s.as_str().map(String::from)).collect(),
                key: fstr(c, "key").map(String::from),
                source_library_key: library_key_of(c),
            },
        );
    }
    let vars = p.get("variables").and_then(Value::as_object).unwrap_or(&empty);
    for (key, v) in vars {
        let id = fstr(v, "id").unwrap_or(key).to_string();
        let Some(ty) = fstr(v, "resolvedType") else { continue };
        let Some(kind) = kind_of(ty) else { continue };
        let Some(coll) = fstr(v, "variableCollectionId") else { continue };
        if !collections.contains_key(coll) {
            continue;
        }
        let values = v
            .get("valuesByMode")
            .and_then(Value::as_object)
            .map(|m| m.iter().filter_map(|(mode, x)| Some((mode.clone(), parse_value(ty, x)?))).collect())
            .unwrap_or_default();
        variables.insert(
            id.clone(),
            Variable {
                id,
                name: fstr(v, "name").unwrap_or("Variable").to_string(),
                collection: coll.to_string(),
                kind,
                values,
                description: fstr(v, "description").unwrap_or("").to_string(),
                // an empty list means "all", which Figma spells ALL_SCOPES
                scopes: farr(v, "scopes").iter().filter_map(|s| s.as_str().filter(|s| *s != "ALL_SCOPES").map(String::from)).collect(),
                hidden_from_publishing: fbool(v, "hiddenFromPublishing").unwrap_or(false),
                key: fstr(v, "key").map(String::from),
                code_syntax: parse_code_syntax(v.get("codeSyntax")),
            },
        );
    }
    // Display order: the collection's `variableIds`, then anything it left out.
    for c in collections.values_mut() {
        c.variables.retain(|id| variables.get(id).is_some_and(|v| v.collection == c.id));
    }
    for v in variables.values() {
        if let Some(c) = collections.get_mut(&v.collection) {
            if !c.variables.contains(&v.id) {
                c.variables.push(v.id.clone());
            }
        }
    }
    let n = variables.len();
    doc.variables = variables;
    doc.variable_collections = collections;
    n
}

/// Reads a `variables/local` response (or any shape [`import_variables_value`]
/// accepts) from JSON text, e.g. a file opened alongside the design.
pub fn import_variables_json(doc: &mut Document, json: &str) -> Result<usize, FigError> {
    let v = crate::rest_import::parse_json_text(json)?;
    if find_payload(&v).is_none() {
        return Err(FigError::Format("no `meta.variables` / `variableCollections` in JSON".into()));
    }
    Ok(import_variables_value(doc, &v))
}

fn finite_or(x: f32, neutral: f32) -> f32 {
    if x.is_finite() {
        x
    } else {
        neutral
    }
}

fn value_json(v: &VariableValue) -> Value {
    match v {
        VariableValue::Color(c) => json!({
            "r": finite_or(c.r, 0.0),
            "g": finite_or(c.g, 0.0),
            "b": finite_or(c.b, 0.0),
            "a": finite_or(c.a, 1.0),
        }),
        VariableValue::Number(n) => json!(finite_or(*n, 0.0)),
        VariableValue::String(s) => json!(s),
        VariableValue::Boolean(b) => json!(b),
        VariableValue::Alias(id) => json!({"type": "VARIABLE_ALIAS", "id": id}),
    }
}

/// `{variables, variableCollections}` as in a response's `meta`.
pub fn export_variables_meta(doc: &Document) -> Value {
    let mut vars = Map::new();
    for v in doc.variables.values() {
        let mut m = Map::new();
        m.insert("id".into(), v.id.clone().into());
        m.insert("name".into(), v.name.clone().into());
        if let Some(k) = &v.key {
            m.insert("key".into(), k.clone().into());
        }
        m.insert("variableCollectionId".into(), v.collection.clone().into());
        m.insert("resolvedType".into(), type_name(v.kind).into());
        m.insert("valuesByMode".into(), Value::Object(v.values.iter().map(|(k, x)| (k.clone(), value_json(x))).collect()));
        m.insert("remote".into(), false.into());
        m.insert("description".into(), v.description.clone().into());
        m.insert("hiddenFromPublishing".into(), v.hidden_from_publishing.into());
        m.insert("scopes".into(), json!(if v.scopes.is_empty() { vec!["ALL_SCOPES".to_string()] } else { v.scopes.clone() }));
        m.insert("codeSyntax".into(), code_syntax_json(&v.code_syntax));
        vars.insert(v.id.clone(), Value::Object(m));
    }
    let mut colls = Map::new();
    for c in doc.variable_collections.values() {
        let mut m = Map::new();
        m.insert("id".into(), c.id.clone().into());
        m.insert("name".into(), c.name.clone().into());
        if let Some(k) = &c.key {
            m.insert("key".into(), k.clone().into());
        }
        m.insert("modes".into(), Value::Array(c.modes.iter().map(|x| json!({"modeId": x.id, "name": x.name})).collect()));
        m.insert("defaultModeId".into(), c.default_mode.clone().into());
        if let Some(lib) = &c.source_library_key {
            m.insert("sourceLibraryKey".into(), lib.clone().into());
        }
        m.insert("remote".into(), c.is_library().into());
        m.insert("hiddenFromPublishing".into(), false.into());
        m.insert("variableIds".into(), json!(c.variables));
        colls.insert(c.id.clone(), Value::Object(m));
    }
    json!({"variables": vars, "variableCollections": colls})
}

/// Full `GET /v1/files/:key/variables/local` response body.
pub fn export_variables_json(doc: &Document) -> Value {
    json!({"status": 200, "error": false, "meta": export_variables_meta(doc)})
}

// ------------------------------------------------------------ node bindings

fn collect_aliases(v: &Value, key: String, out: &mut BTreeMap<String, String>) {
    if is_alias(v) {
        if let Some(id) = fstr(v, "id") {
            out.insert(key, id.to_string());
        }
        return;
    }
    match v {
        Value::Array(a) => a.iter().enumerate().for_each(|(i, e)| collect_aliases(e, format!("{key}/{i}"), out)),
        Value::Object(o) => o.iter().for_each(|(k, e)| collect_aliases(e, format!("{key}/{k}"), out)),
        _ => {}
    }
}

/// REST has no slot for a paint's opacity variable, so `boundVariables` carries it as
/// `fillOpacities[i]` / `strokeOpacities[i]` (effects use `effects[i].opacity`). These are the
/// matching binding keys, `fills/i/opacity` etc.
fn opacity_binding_key(rest_key: &str) -> Option<String> {
    if let Some(rest) = rest_key.strip_prefix("fillStops/") {
        let (i, j) = rest.split_once('/')?;
        return Some(format!("fills/{i}/stops/{j}"));
    }
    if let Some(rest) = rest_key.strip_prefix("strokeStops/") {
        let (i, j) = rest.split_once('/')?;
        return Some(format!("strokes/{i}/stops/{j}"));
    }
    let (list, i) = rest_key.split_once('/')?;
    let list = match list {
        "fillOpacities" => "fills",
        "strokeOpacities" => "strokes",
        "effects" if i.ends_with("/opacity") => return Some(rest_key.to_string()),
        _ => return None,
    };
    Some(format!("{list}/{i}/opacity"))
}

fn opacity_rest_path(binding_key: &str) -> Option<String> {
    if let Some(rest) = binding_key.strip_prefix("fills/") {
        if let Some((i, j)) = rest.split_once("/stops/") {
            return Some(format!("fillStops/{i}/{j}"));
        }
    }
    if let Some(rest) = binding_key.strip_prefix("strokes/") {
        if let Some((i, j)) = rest.split_once("/stops/") {
            return Some(format!("strokeStops/{i}/{j}"));
        }
    }
    let rest = binding_key.strip_suffix("/opacity")?;
    let (list, i) = rest.split_once('/')?;
    match list {
        "fills" => Some(format!("fillOpacities/{i}")),
        "strokes" => Some(format!("strokeOpacities/{i}")),
        "effects" => Some(binding_key.to_string()),
        _ => None,
    }
}

/// `boundVariables` and `explicitVariableModes` of a REST node.
pub(crate) fn parse_node_variables(v: &Value) -> NodeVariables {
    let mut nv = NodeVariables::default();
    let mut raw = BTreeMap::new();
    if let Some(b) = v.get("boundVariables").and_then(Value::as_object) {
        for (k, e) in b {
            collect_aliases(e, k.clone(), &mut raw);
        }
    }
    if let Some(b) = v.get("style").and_then(|s| s.get("boundVariables")).and_then(Value::as_object) {
        for (k, e) in b {
            collect_aliases(e, k.clone(), &mut raw);
        }
    }
    for (k, id) in raw {
        let k = match k.as_str() {
            "size/x" => "width".to_string(),
            "size/y" => "height".to_string(),
            "fontFamily/0" => "fontFamily".to_string(),
            "fontSize/0" => "fontSize".to_string(),
            "fontStyle/0" => "fontStyle".to_string(),
            "fontWeight/0" => "fontWeight".to_string(),
            "letterSpacing/0" => "letterSpacing".to_string(),
            "lineHeight/0" => "lineHeight".to_string(),
            "paragraphSpacing/0" => "paragraphSpacing".to_string(),
            "paragraphIndent/0" => "paragraphIndent".to_string(),
            _ => opacity_binding_key(&k).unwrap_or(k),
        };
        nv.bindings.insert(k, id);
    }
    for (arr_key, prefix) in [("fills", "fills"), ("strokes", "strokes")] {
        if let Some(arr) = v.get(arr_key).and_then(Value::as_array) {
            for (i, p) in arr.iter().enumerate() {
                if let Some(bv) = p.get("boundVariables") {
                    if let Some(c) = bv.get("color").and_then(|a| fstr(a, "id")) {
                        nv.bindings.entry(format!("{prefix}/{i}")).or_insert_with(|| c.to_string());
                    }
                    if let Some(op) = bv.get("opacity").and_then(|a| fstr(a, "id")) {
                        nv.bindings.entry(format!("{prefix}/{i}/opacity")).or_insert_with(|| op.to_string());
                    }
                }
                for stops_key in ["gradientStops", "stops"] {
                    if let Some(stops) = p.get(stops_key).and_then(Value::as_array) {
                        for (j, st) in stops.iter().enumerate() {
                            if let Some(c) = st.get("boundVariables").and_then(|b| b.get("color")).and_then(|a| fstr(a, "id")) {
                                nv.bindings.entry(format!("{prefix}/{i}/stops/{j}")).or_insert_with(|| c.to_string());
                            }
                        }
                    }
                }
            }
        }
    }
    if let Some(arr) = v.get("effects").and_then(Value::as_array) {
        for (i, e) in arr.iter().enumerate() {
            if let Some(bv) = e.get("boundVariables") {
                for prop in ["color", "opacity", "radius", "spread", "offsetX", "offsetY"] {
                    if let Some(id) = bv.get(prop).and_then(|a| fstr(a, "id")) {
                        nv.bindings.entry(format!("effects/{i}/{prop}")).or_insert_with(|| id.to_string());
                    }
                }
            }
        }
    }
    if let Some(m) = v.get("explicitVariableModes").and_then(Value::as_object) {
        for (c, mode) in m {
            if let Some(mode) = mode.as_str() {
                nv.modes.insert(c.clone(), mode.to_string());
            }
        }
    }
    nv
}

fn put(root: &mut Value, path: &[&str], leaf: Value) {
    let Some((first, rest)) = path.split_first() else { return };
    let idx = first.parse::<usize>().ok();
    if let Some(i) = idx {
        if !root.is_array() {
            *root = Value::Array(Vec::new());
        }
        let a = root.as_array_mut().unwrap();
        while a.len() <= i {
            a.push(Value::Null);
        }
        if rest.is_empty() {
            a[i] = leaf;
        } else {
            put(&mut a[i], rest, leaf);
        }
    } else {
        if !root.is_object() {
            *root = Value::Object(Map::new());
        }
        let o = root.as_object_mut().unwrap();
        let slot = o.entry(first.to_string()).or_insert(Value::Null);
        if rest.is_empty() {
            *slot = leaf;
        } else {
            put(slot, rest, leaf);
        }
    }
}

/// Writes `boundVariables` / `explicitVariableModes` for a node.
pub(crate) fn export_node_variables(node: &Node, m: &mut Map<String, Value>) {
    if !node.variables.bindings.is_empty() {
        let mut root = Value::Object(Map::new());
        for (k, id) in &node.variables.bindings {
            let rest_path = opacity_rest_path(k);
            let key = match k.as_str() {
                "width" => "size/x",
                "height" => "size/y",
                other => rest_path.as_deref().unwrap_or(other),
            };
            let parts: Vec<&str> = key.split('/').collect();
            put(&mut root, &parts, json!({"type": "VARIABLE_ALIAS", "id": id}));
        }
        m.insert("boundVariables".into(), root);
    }
    if !node.variables.modes.is_empty() {
        m.insert("explicitVariableModes".into(), json!(node.variables.modes));
    }
}

// ------------------------------------------------------------ .fig

fn guid(v: &Value) -> Option<String> {
    let g = v.get("guid").unwrap_or(v);
    Some(format!("{}:{}", g.get("sessionID")?.as_u64()?, g.get("localID")?.as_u64()?))
}

/// Id lookups for `.fig` variables: library assets are referenced by
/// `{assetRef: {key}}` (the target node's `key`), local ones by `{guid}`.
struct Refs<'a> {
    sets: BTreeMap<&'a str, String>,
    vars: BTreeMap<&'a str, String>,
}

impl Refs<'_> {
    fn set(&self, r: &Value) -> Option<String> {
        match crate::fig::asset_key(r) {
            Some(k) => self.sets.get(k).cloned(),
            None => Some(format!("VariableCollectionId:{}", guid(r)?)),
        }
    }

    fn var(&self, r: &Value) -> Option<String> {
        match crate::fig::asset_key(r) {
            Some(k) => self.vars.get(k).cloned(),
            None => Some(format!("VariableID:{}", guid(r)?)),
        }
    }
}

fn fig_value(ty: &str, v: &Value, refs: &Refs) -> Option<Value> {
    if let Some(a) = v.get("alias") {
        return Some(json!({"type": "VARIABLE_ALIAS", "id": refs.var(a)?}));
    }
    match ty {
        "COLOR" => v.get("colorValue").cloned(),
        "FLOAT" => v.get("floatValue").cloned(),
        "STRING" => v.get("textValue").cloned(),
        "BOOLEAN" => v.get("boolValue").cloned(),
        _ => None,
    }
}

/// Builds a REST-shaped variables payload from the `VARIABLE_SET` and
/// `VARIABLE` node changes of a `.fig` message (given as JSON).
///
/// A set has `variableSetModes: [{id, name, sortPosition}]`. A variable has
/// `variableSetID` (an `assetRef` to the set's `key`, or a local `guid`),
/// `variableResolvedType` (COLOR / FLOAT / STRING / BOOLEAN),
/// `variableScopes` and `variableDataValues.entries: [{modeID, variableData}]`
/// where `variableData.value` is `colorValue`, `floatValue`, `textValue`,
/// `boolValue` or `alias`.
pub(crate) fn from_fig_entries(entries: &[&Value]) -> Option<Value> {
    let active = |e: &Value| fstr(e, "phase") != Some("REMOVED") && fbool(e, "isSoftDeleted") != Some(true);
    let mut refs = Refs { sets: BTreeMap::new(), vars: BTreeMap::new() };
    for e in entries.iter().copied().filter(|e| active(e)) {
        let (Some(key), Some(gid)) = (fstr(e, "key").filter(|k| !k.is_empty()), e.get("guid").and_then(guid)) else { continue };
        match fstr(e, "type") {
            Some("VARIABLE_SET") => refs.sets.insert(key, format!("VariableCollectionId:{gid}")),
            _ => refs.vars.insert(key, format!("VariableID:{gid}")),
        };
    }
    let mut colls = Map::new();
    let mut vars = Map::new();
    for e in entries.iter().copied().filter(|e| active(e)) {
        if fstr(e, "type") != Some("VARIABLE_SET") {
            continue;
        }
        let Some(gid) = e.get("guid").and_then(guid) else { continue };
        let id = format!("VariableCollectionId:{gid}");
        let mut modes: Vec<(&str, Value)> = farr(e, "variableSetModes")
            .iter()
            .filter_map(|m| Some((fstr(m, "sortPosition").unwrap_or(""), json!({"modeId": guid(m.get("id")?)?, "name": fstr(m, "name").unwrap_or("Mode")}))))
            .collect();
        modes.sort_by(|a, b| a.0.cmp(b.0));
        let modes: Vec<Value> = modes.into_iter().map(|m| m.1).collect();
        let default = modes.first().and_then(|m| m.get("modeId")).cloned().unwrap_or(Value::Null);
        let mut c = json!({"id": id, "name": fstr(e, "name").unwrap_or("Collection"), "modes": modes, "defaultModeId": default, "variableIds": []});
        if let Some(k) = fstr(e, "key").filter(|k| !k.is_empty()) {
            c["key"] = k.into();
        }
        if let Some(lib) = fstr(e, "sourceLibraryKey").filter(|k| !k.is_empty()) {
            c["sourceLibraryKey"] = lib.into();
        }
        colls.insert(id, c);
    }
    let mut order: Vec<(&str, &Value)> =
        entries.iter().filter(|e| active(e) && fstr(e, "type") == Some("VARIABLE")).map(|e| (fstr(e, "sortPosition").unwrap_or(""), *e)).collect();
    order.sort_by(|a, b| a.0.cmp(b.0));
    for (_, e) in order {
        let Some(gid) = e.get("guid").and_then(guid) else { continue };
        let cid = e
            .get("variableSetID")
            .and_then(|s| refs.set(s))
            .or_else(|| e.get("parentIndex").and_then(|p| p.get("guid")).and_then(guid).map(|g| format!("VariableCollectionId:{g}")));
        let Some(cid) = cid else { continue };
        let Some(ty) = fstr(e, "variableResolvedType").or_else(|| fstr(e, "variableDataType")) else { continue };
        let mut by_mode = Map::new();
        let list = e.get("variableDataValues").map(|d| farr(d, "entries")).unwrap_or(&[]);
        for ent in list {
            let (Some(mode), Some(data)) = (ent.get("modeID").and_then(guid), ent.get("variableData")) else { continue };
            let Some(val) = data.get("value").and_then(|x| fig_value(ty, x, &refs)) else { continue };
            by_mode.insert(mode, val);
        }
        let id = format!("VariableID:{gid}");
        if let Some(c) = colls.get_mut(&cid).and_then(|c| c.get_mut("variableIds")).and_then(Value::as_array_mut) {
            c.push(json!(id));
        } else {
            continue;
        }
        let mut v = json!({"id": id, "name": fstr(e, "name").unwrap_or("Variable"), "variableCollectionId": cid, "resolvedType": ty,
                   "valuesByMode": by_mode, "scopes": e.get("variableScopes").cloned().unwrap_or(json!([])),
                   "description": fstr(e, "description").unwrap_or(""), "hiddenFromPublishing": false});
        if let Some(k) = fstr(e, "key").filter(|k| !k.is_empty()) {
            v["key"] = k.into();
        }
        let syntax: Map<String, Value> = e
            .get("codeSyntax")
            .map(|m| farr(m, "entries"))
            .unwrap_or(&[])
            .iter()
            .filter_map(|x| Some((fstr(x, "platform")?.to_string(), Value::String(fstr(x, "value")?.to_string()))))
            .collect();
        if !syntax.is_empty() {
            v["codeSyntax"] = Value::Object(syntax);
        }
        vars.insert(id, v);
    }
    (!colls.is_empty()).then(|| json!({"variables": vars, "variableCollections": colls}))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fig_variable_nodes_map_tolerantly() {
        let set = json!({"type": "VARIABLE_SET", "guid": {"sessionID": 1, "localID": 5}, "name": "Theme",
            "variableSetModes": [{"id": {"sessionID": 1, "localID": 6}, "name": "Light"}, {"id": {"sessionID": 1, "localID": 7}, "name": "Dark"}]});
        let var = json!({"type": "VARIABLE", "guid": {"sessionID": 1, "localID": 9}, "name": "Bg", "variableResolvedType": "FLOAT",
            "variableSetID": {"guid": {"sessionID": 1, "localID": 5}},
            "variableDataValues": {"entries": [
                {"modeID": {"sessionID": 1, "localID": 6}, "variableData": {"value": {"floatValue": 4.0}}},
                {"modeID": {"sessionID": 1, "localID": 7}, "variableData": {"value": {"alias": {"guid": {"sessionID": 1, "localID": 9}}}}}]}});
        let junk = json!({"type": "VARIABLE", "guid": {"sessionID": 1, "localID": 10}});
        let payload = from_fig_entries(&[&set, &var, &junk]).unwrap();
        let mut doc = Document::new("d");
        assert_eq!(import_variables_value(&mut doc, &json!({ "variables": payload })), 1);
        let c = &doc.variable_collections["VariableCollectionId:1:5"];
        assert_eq!((c.modes.len(), c.default_mode.as_str()), (2, "1:6"));
        let v = &doc.variables["VariableID:1:9"];
        assert_eq!(v.values["1:6"], VariableValue::Number(4.0));
        assert_eq!(v.values["1:7"], VariableValue::Alias("VariableID:1:9".into()));
        assert!(from_fig_entries(&[]).is_none());
    }
    fn fig_set(local: u64, name: &str, key: &str, source_library: Option<&str>) -> Value {
        let mut set = json!({"type": "VARIABLE_SET", "guid": {"sessionID": 1, "localID": local}, "name": name, "key": key,
            "variableSetModes": [{"id": {"sessionID": 1, "localID": local + 100}, "name": "Mode 1"}]});
        if let Some(lib) = source_library {
            set["sourceLibraryKey"] = lib.into();
        }
        set
    }

    #[test]
    fn subscribed_library_sets_are_marked_and_keep_their_variables() {
        let local = fig_set(5, "Theme", "K_LOCAL", None);
        let library = fig_set(20, "Theme", "K_LIB", Some("lk-83c0"));
        let local_var = json!({"type": "VARIABLE", "guid": {"sessionID": 1, "localID": 9}, "name": "bg", "variableResolvedType": "FLOAT",
            "variableSetID": {"guid": {"sessionID": 1, "localID": 5}}});
        let library_var = json!({"type": "VARIABLE", "guid": {"sessionID": 1, "localID": 30}, "name": "fg", "variableResolvedType": "FLOAT",
            "variableSetID": {"assetRef": {"key": "K_LIB"}}});
        let payload = from_fig_entries(&[&local, &library, &local_var, &library_var]).unwrap();
        let mut doc = Document::new("d");
        assert_eq!(import_variables_value(&mut doc, &json!({ "variables": payload })), 2);
        let (l, lib) = (&doc.variable_collections["VariableCollectionId:1:5"], &doc.variable_collections["VariableCollectionId:1:20"]);
        assert_eq!((l.source_library_key.as_deref(), l.is_library()), (None, false));
        assert_eq!((lib.source_library_key.as_deref(), lib.is_library()), (Some("lk-83c0"), true));
        assert_eq!((l.variables.len(), lib.variables.len()), (1, 1));
        // the publish key stays what it was
        assert_eq!((l.key.as_deref(), lib.key.as_deref()), (Some("K_LOCAL"), Some("K_LIB")));
    }

    #[test]
    fn library_marker_survives_the_rest_shaped_export() {
        let mut doc = Document::new("d");
        let payload = from_fig_entries(&[&fig_set(5, "Theme", "K_LOCAL", None), &fig_set(20, "Theme", "K_LIB", Some("lk-83c0"))]).unwrap();
        import_variables_value(&mut doc, &json!({ "variables": payload }));
        let meta = export_variables_meta(&doc);
        assert_eq!(meta["variableCollections"]["VariableCollectionId:1:20"]["sourceLibraryKey"], "lk-83c0");
        assert!(meta["variableCollections"]["VariableCollectionId:1:5"].get("sourceLibraryKey").is_none());
        let mut again = Document::new("d2");
        import_variables_value(&mut again, &json!({ "variables": meta }));
        assert_eq!(again.variable_collections, doc.variable_collections);
    }

    #[test]
    fn export_variables_json_replaces_non_finite_numbers_and_roundtrips() {
        let mut doc = Document::new("vars");
        doc.variable_collections.insert(
            "c1".into(),
            VariableCollection {
                id: "c1".into(),
                name: "Tokens".into(),
                modes: vec![VariableMode { id: "m1".into(), name: "Light".into() }, VariableMode { id: "m2".into(), name: "Dark".into() }],
                default_mode: "m1".into(),
                variables: vec!["v_num".into(), "v_col".into()],
                key: None,
                source_library_key: None,
            },
        );
        doc.variables.insert(
            "v_num".into(),
            Variable {
                id: "v_num".into(),
                name: "spacing".into(),
                collection: "c1".into(),
                kind: VariableKind::Number,
                values: BTreeMap::from([("m1".into(), VariableValue::Number(f32::NAN)), ("m2".into(), VariableValue::Number(f32::INFINITY))]),
                description: String::new(),
                scopes: Vec::new(),
                hidden_from_publishing: false,
                key: None,
                code_syntax: CodeSyntax::new(),
            },
        );
        doc.variables.insert(
            "v_col".into(),
            Variable {
                id: "v_col".into(),
                name: "accent".into(),
                collection: "c1".into(),
                kind: VariableKind::Color,
                values: BTreeMap::from([("m1".into(), VariableValue::Color(Color { r: f32::NAN, g: 0.5, b: f32::NEG_INFINITY, a: f32::NAN }))]),
                description: String::new(),
                scopes: Vec::new(),
                hidden_from_publishing: false,
                key: None,
                code_syntax: CodeSyntax::new(),
            },
        );

        let exported = export_variables_json(&doc);
        let text = serde_json::to_string(&exported).unwrap();
        assert!(!text.contains("null"), "variable export must not write null for non-finite numbers: {text}");

        let mut reimported = Document::new("back");
        assert_eq!(import_variables_json(&mut reimported, &text).unwrap(), 2);
        assert_eq!(reimported.variables["v_num"].values.get("m1"), Some(&VariableValue::Number(0.0)));
        assert_eq!(reimported.variables["v_num"].values.get("m2"), Some(&VariableValue::Number(0.0)));
        assert_eq!(reimported.variables["v_col"].values.get("m1"), Some(&VariableValue::Color(Color { r: 0.0, g: 0.5, b: 0.0, a: 1.0 })));
    }
}
