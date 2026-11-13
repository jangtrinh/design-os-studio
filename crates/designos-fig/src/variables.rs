//! Figma variables: REST `GET /v1/files/:key/variables/local` payloads, node
//! `boundVariables` / `explicitVariableModes`, and a tolerant `.fig` mapping.
//!
//! Ids are kept as Figma writes them (`VariableID:1:23`, mode ids `1:0`), so
//! exporting gives back the same references.

use crate::rest_import::{color, farr, fbool, fstr};
use crate::FigError;
use designos_doc::*;
use serde_json::{json, Map, Value};
use std::collections::{BTreeMap, BTreeSet};

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

/// A value whose variable is not in the payload (an override of a library variable): its kind comes from its shape.
fn infer_value(v: &Value) -> Option<VariableValue> {
    if is_alias(v) {
        return parse_value("COLOR", v);
    }
    match v {
        Value::Bool(_) => parse_value("BOOLEAN", v),
        Value::Number(_) => parse_value("FLOAT", v),
        Value::String(_) => parse_value("STRING", v),
        Value::Object(_) => parse_value("COLOR", v),
        _ => None,
    }
}

/// REST field (ours; Figma's REST has none) marking a collection other collections may extend.
const EXTENDABLE_FLAG: &str = "isExtendable";
/// REST field (ours) naming the Figma key of an extended collection's parent when the file does not hold it.
const PARENT_KEY_FIELD: &str = "parentVariableCollectionKey";

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

/// Figma's REST flag for a variable the editor deleted while the document still refers to it. Payloads built from a
/// `.fig` and written by [`export_variables_meta`] use it for kept soft-deleted variables and collections too.
const REST_DELETED_FLAG: &str = "deletedButReferenced";

/// The extensions of an entry whose payload carries the REST deleted flag: the soft-deleted marker, or none.
fn soft_deleted_extensions(entry: &Value) -> FigmaExtensions {
    let mut extensions = FigmaExtensions::new();
    if fbool(entry, REST_DELETED_FLAG) == Some(true) {
        extensions.insert(SOFT_DELETED_EXTENSION_KEY.to_string(), Value::Bool(true));
    }
    extensions
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
        let mut collection = VariableCollection {
            id: id.clone(),
            name: fstr(c, "name").unwrap_or("Collection").to_string(),
            modes,
            default_mode,
            variables: farr(c, "variableIds").iter().filter_map(|s| s.as_str().map(String::from)).collect(),
            key: fstr(c, "key").map(String::from),
            source_library_key: library_key_of(c),
            extensions: soft_deleted_extensions(c),
        };
        collection.set_extendable(fbool(c, EXTENDABLE_FLAG) == Some(true));
        if fbool(c, "isExtension") == Some(true) {
            collection.set_extension(&CollectionExtension {
                parent: ParentCollection {
                    collection: fstr(c, "parentVariableCollectionId").unwrap_or("").to_string(),
                    set_key: fstr(c, PARENT_KEY_FIELD).filter(|k| !k.is_empty()).map(String::from),
                },
                mode_map: farr(c, "modes").iter().filter_map(|m| Some((fstr(m, "modeId")?.to_string(), fstr(m, "parentModeId")?.to_string()))).collect(),
                overrides: BTreeMap::new(),
            });
        }
        collections.insert(id, collection);
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
                extensions: soft_deleted_extensions(v),
                mode_extensions: Default::default(),
            },
        );
    }
    // An extension of a library collection also remembers the library collection's key.
    let library_keys: BTreeMap<String, String> = collections.values().filter(|c| c.is_library()).filter_map(|c| Some((c.id.clone(), c.key.clone()?))).collect();
    // A parent that is extended allows it: lookups skip the extension scan for a collection that is not extendable.
    let parents: BTreeSet<String> = collections.values().filter_map(|c| c.extension_parent_id_in(&collections).map(String::from)).collect();
    for id in &parents {
        if let Some(parent) = collections.get_mut(id) {
            parent.set_extendable(true);
        }
    }
    for coll in collections.values_mut() {
        if let Some(mut ext) = coll.extension().filter(|e| e.parent.set_key.is_none()) {
            if let Some(key) = library_keys.get(&ext.parent.collection) {
                ext.parent.set_key = Some(key.clone());
                coll.set_extension(&ext);
            }
        }
    }
    for (key, c) in colls {
        let id = fstr(c, "id").unwrap_or(key);
        let Some(overrides) = c.get("variableOverrides").and_then(Value::as_object) else { continue };
        let Some(coll) = collections.get_mut(id) else { continue };
        let Some(mut ext) = coll.extension() else { continue };
        for (var, by_mode) in overrides {
            let ty = variables.get(var).map(|v| type_name(v.kind));
            for (mode, raw) in by_mode.as_object().into_iter().flatten() {
                if let Some(value) = ty.map_or_else(|| infer_value(raw), |ty| parse_value(ty, raw)) {
                    ext.overrides.entry(var.clone()).or_default().insert(mode.clone(), value);
                }
            }
        }
        coll.set_extension(&ext);
    }
    // Display order: the collection's `variableIds`, then anything it left out. A soft-deleted variable is never
    // listed, whatever the payload's list says: it stays in `variables` only so references to it resolve.
    for c in collections.values_mut() {
        c.variables.retain(|id| variables.get(id).is_some_and(|v| v.collection == c.id && !v.is_soft_deleted()));
    }
    for v in variables.values().filter(|v| !v.is_soft_deleted()) {
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
        m.insert("remote".into(), doc.variable_collections.get(&v.collection).is_some_and(VariableCollection::is_library).into());
        m.insert("description".into(), v.description.clone().into());
        m.insert("hiddenFromPublishing".into(), v.hidden_from_publishing.into());
        if v.is_soft_deleted() {
            m.insert(REST_DELETED_FLAG.into(), true.into());
        }
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
        if c.is_soft_deleted() {
            m.insert(REST_DELETED_FLAG.into(), true.into());
        }
        m.insert("variableIds".into(), json!(c.variables));
        if c.is_extendable() {
            m.insert(EXTENDABLE_FLAG.into(), true.into());
        }
        if let Some(ext) = c.extension() {
            insert_extension_fields(doc, c, &ext, &mut m);
        }
        colls.insert(c.id.clone(), Value::Object(m));
    }
    json!({"variables": vars, "variableCollections": colls})
}

/// Figma's extended-collection fields: `isExtension`, the parent and root collection, each mode's `parentModeId`,
/// the inherited variable ids and `variableOverrides` (variable id -> mode id -> value).
fn insert_extension_fields(doc: &Document, c: &VariableCollection, ext: &CollectionExtension, m: &mut Map<String, Value>) {
    m.insert("isExtension".into(), true.into());
    if !ext.parent.collection.is_empty() {
        m.insert("parentVariableCollectionId".into(), ext.parent.collection.clone().into());
    }
    if let Some(key) = &ext.parent.set_key {
        m.insert(PARENT_KEY_FIELD.into(), key.clone().into());
    }
    let mut root = c.extension_parent_id_in(&doc.variable_collections).unwrap_or(ext.parent.collection.as_str()).to_string();
    for _ in 0..8 {
        match doc.variable_collections.get(&root).and_then(|rc| rc.extension_parent_id_in(&doc.variable_collections)) {
            Some(next) => root = next.to_string(),
            None => break,
        }
    }
    if !root.is_empty() {
        m.insert("rootVariableCollectionId".into(), root.into());
    }
    let modes = c
        .modes
        .iter()
        .map(|x| {
            let mut mode = json!({"modeId": x.id, "name": x.name});
            if let Some(parent_mode) = ext.mode_map.get(&x.id) {
                mode["parentModeId"] = parent_mode.clone().into();
            }
            mode
        })
        .collect();
    m.insert("modes".into(), Value::Array(modes));
    m.insert("inheritedVariableIds".into(), json!(doc.inherited_variable_ids(&c.id)));
    let overrides: Map<String, Value> = ext
        .overrides
        .iter()
        .map(|(var, by_mode)| (var.clone(), Value::Object(by_mode.iter().map(|(mode, v)| (mode.clone(), value_json(v))).collect())))
        .collect();
    m.insert("variableOverrides".into(), Value::Object(overrides));
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
                for prop in
                    ["color", "radius", "spread", "offsetX", "offsetY", "opacity", "lightAngle", "lightIntensity", "refraction", "depth", "dispersion", "frost"]
                {
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

/// The kiwi `VariableScope` enum spells the stroke colour scope `STROKE`; the document and
/// REST payloads use `STROKE_COLOR`. Map it back at this boundary so a `.fig` round trip keeps
/// the stored spelling, keeping order and listing the stroke colour scope once.
fn rest_scopes_from_fig(scopes: Option<&Value>) -> Value {
    let Some(list) = scopes.and_then(Value::as_array) else {
        return scopes.cloned().unwrap_or(json!([]));
    };
    let mut mapped: Vec<Value> = Vec::with_capacity(list.len());
    for scope in list {
        let scope = match scope.as_str() {
            Some(name) => json!(crate::variables_dtcg::normalize_scope_on_import(name)),
            None => scope.clone(),
        };
        if scope == "STROKE_COLOR" && mapped.contains(&scope) {
            continue;
        }
        mapped.push(scope);
    }
    Value::Array(mapped)
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
#[cfg(test)]
pub(crate) fn from_fig_entries(entries: &[&Value]) -> Option<Value> {
    from_fig_entries_with_refs(entries, &std::collections::HashSet::new())
}

pub(crate) fn from_fig_entries_with_refs(entries: &[&Value], referenced_vars: &std::collections::HashSet<String>) -> Option<Value> {
    let not_removed = |e: &Value| fstr(e, "phase") != Some("REMOVED");
    let active = |e: &Value| not_removed(e) && fbool(e, "isSoftDeleted") != Some(true);
    let mut refs = Refs { sets: BTreeMap::new(), vars: BTreeMap::new() };
    for e in entries.iter().copied().filter(|e| active(e)) {
        let (Some(key), Some(gid)) = (fstr(e, "key").filter(|k| !k.is_empty()), e.get("guid").and_then(guid)) else { continue };
        match fstr(e, "type") {
            Some("VARIABLE_SET") => refs.sets.insert(key, format!("VariableCollectionId:{gid}")),
            _ => refs.vars.insert(key, format!("VariableID:{gid}")),
        };
    }
    for e in entries.iter().copied().filter(|e| not_removed(e) && !active(e)) {
        let (Some(key), Some(gid)) = (fstr(e, "key").filter(|k| !k.is_empty()), e.get("guid").and_then(guid)) else { continue };
        match fstr(e, "type") {
            Some("VARIABLE_SET") => {
                refs.sets.entry(key).or_insert_with(|| format!("VariableCollectionId:{gid}"));
            }
            _ => {
                refs.vars.entry(key).or_insert_with(|| format!("VariableID:{gid}"));
            }
        }
    }

    let mut var_by_id: BTreeMap<String, &Value> = BTreeMap::new();
    let mut keep_vars: std::collections::HashSet<String> = referenced_vars.clone();
    for &e in entries.iter().filter(|e| not_removed(e) && fstr(e, "type") == Some("VARIABLE")) {
        let Some(gid) = e.get("guid").and_then(guid) else { continue };
        let id = format!("VariableID:{gid}");
        if active(e) {
            keep_vars.insert(id.clone());
            var_by_id.insert(id, e);
        } else {
            var_by_id.entry(id).or_insert(e);
        }
    }
    let mut queue: Vec<String> = keep_vars.iter().cloned().collect();
    while let Some(vid) = queue.pop() {
        let Some(&e) = var_by_id.get(&vid) else { continue };
        let list = e.get("variableDataValues").map(|d| farr(d, "entries")).unwrap_or(&[]);
        for ent in list {
            let Some(alias) = ent.get("variableData").and_then(|d| d.get("value")).and_then(|x| x.get("alias")) else { continue };
            let Some(target) = refs.var(alias) else { continue };
            if keep_vars.insert(target.clone()) {
                queue.push(target);
            }
        }
    }

    let mut keep_sets: std::collections::HashSet<String> = std::collections::HashSet::new();
    for &e in entries.iter().filter(|e| active(e) && fstr(e, "type") == Some("VARIABLE_SET")) {
        if let Some(gid) = e.get("guid").and_then(guid) {
            keep_sets.insert(format!("VariableCollectionId:{gid}"));
        }
    }
    for vid in &keep_vars {
        let Some(&e) = var_by_id.get(vid) else { continue };
        let cid = e
            .get("variableSetID")
            .and_then(|s| refs.set(s))
            .or_else(|| e.get("parentIndex").and_then(|p| p.get("guid")).and_then(guid).map(|g| format!("VariableCollectionId:{g}")));
        if let Some(cid) = cid {
            keep_sets.insert(cid);
        }
    }

    let mut colls = Map::new();
    let mut vars = Map::new();
    for e in entries.iter().copied().filter(|e| not_removed(e)) {
        if fstr(e, "type") != Some("VARIABLE_SET") {
            continue;
        }
        let Some(gid) = e.get("guid").and_then(guid) else { continue };
        let id = format!("VariableCollectionId:{gid}");
        if !keep_sets.contains(&id) || (!active(e) && colls.contains_key(&id)) {
            continue;
        }
        let mut modes: Vec<(&str, Value)> = farr(e, "variableSetModes")
            .iter()
            .filter_map(|m| {
                let mut mode = json!({"modeId": guid(m.get("id")?)?, "name": fstr(m, "name").unwrap_or("Mode")});
                if let Some(parent_mode) = m.get("parentModeId").and_then(guid) {
                    mode["parentModeId"] = parent_mode.into();
                }
                Some((fstr(m, "sortPosition").unwrap_or(""), mode))
            })
            .collect();
        modes.sort_by(|a, b| a.0.cmp(b.0));
        let modes: Vec<Value> = modes.into_iter().map(|m| m.1).collect();
        let default = modes.first().and_then(|m| m.get("modeId")).cloned().unwrap_or(Value::Null);
        let mut c = json!({"id": id, "name": fstr(e, "name").unwrap_or("Collection"), "modes": modes, "defaultModeId": default, "variableIds": []});
        if !active(e) {
            c[REST_DELETED_FLAG] = true.into();
        }
        if let Some(k) = fstr(e, "key").filter(|k| !k.is_empty()) {
            c["key"] = k.into();
        }
        if let Some(lib) = fstr(e, "sourceLibraryKey").filter(|k| !k.is_empty()) {
            c["sourceLibraryKey"] = lib.into();
        }
        if fbool(e, "isCollectionExtendable") == Some(true) {
            c[EXTENDABLE_FLAG] = true.into();
        }
        if let Some(parent) = farr(e, "variableSetModes").iter().find_map(|m| m.get("parentVariableSetId")) {
            c["isExtension"] = true.into();
            if let Some(parent_id) = refs.set(parent) {
                c["parentVariableCollectionId"] = parent_id.into();
            }
            if let Some(key) = crate::fig::asset_key(parent) {
                c[PARENT_KEY_FIELD] = key.into();
            }
        }
        colls.insert(id, c);
    }
    // UNVERIFIED layout (no real extended-collection `.fig` was available; the alternative is `VARIABLE_OVERRIDE` nodes,
    // which `import_fig_with_report` counts and does not read): override values sit on the overridden variable, keyed
    // by the extending collection's mode ids.
    let mut extension_of_mode: BTreeMap<String, String> = BTreeMap::new();
    for &e in entries.iter().filter(|e| active(e) && fstr(e, "type") == Some("VARIABLE_SET")) {
        let Some(gid) = e.get("guid").and_then(guid) else { continue };
        for m in farr(e, "variableSetModes").iter().filter(|m| m.get("parentVariableSetId").is_some()) {
            if let Some(mode) = m.get("id").and_then(guid) {
                extension_of_mode.insert(mode, format!("VariableCollectionId:{gid}"));
            }
        }
    }
    let mut overrides: BTreeMap<String, Map<String, Value>> = BTreeMap::new();
    let mut order: Vec<(&str, &Value)> = entries
        .iter()
        .filter(|e| {
            not_removed(e)
                && fstr(e, "type") == Some("VARIABLE")
                && e.get("guid").and_then(guid).is_some_and(|gid| keep_vars.contains(&format!("VariableID:{gid}")))
        })
        .map(|e| (fstr(e, "sortPosition").unwrap_or(""), *e))
        .collect();
    order.sort_by(|a, b| a.0.cmp(b.0));
    for (_, e) in order {
        let Some(gid) = e.get("guid").and_then(guid) else { continue };
        let id = format!("VariableID:{gid}");
        if !active(e) && vars.contains_key(&id) {
            continue;
        }
        let cid = match e.get("variableSetID") {
            Some(s) => refs.set(s),
            None => e.get("parentIndex").and_then(|p| p.get("guid")).and_then(guid).map(|g| format!("VariableCollectionId:{g}")),
        };
        let Some(ty) = fstr(e, "variableResolvedType").or_else(|| fstr(e, "variableDataType")) else { continue };
        let mut by_mode = Map::new();
        let list = e.get("variableDataValues").map(|d| farr(d, "entries")).unwrap_or(&[]);
        for ent in list {
            let (Some(mode), Some(data)) = (ent.get("modeID").and_then(guid), ent.get("variableData")) else { continue };
            let Some(val) = data.get("value").and_then(|x| fig_value(ty, x, &refs)) else { continue };
            let is_own_mode = cid
                .as_ref()
                .and_then(|c| colls.get(c))
                .and_then(|c| c.get("modes"))
                .and_then(Value::as_array)
                .is_some_and(|ms| ms.iter().any(|m| fstr(m, "modeId") == Some(mode.as_str())));
            match (!is_own_mode).then(|| extension_of_mode.get(&mode)).flatten() {
                Some(ext) => {
                    let by_var = overrides.entry(ext.clone()).or_default().entry(id.clone()).or_insert_with(|| Value::Object(Map::new()));
                    if let Some(o) = by_var.as_object_mut() {
                        o.insert(mode, val);
                    }
                }
                None => {
                    by_mode.insert(mode, val);
                }
            }
        }
        let Some(cid) = cid else { continue };
        let Some(listed) = colls.get_mut(&cid).and_then(|c| c.get_mut("variableIds")).and_then(Value::as_array_mut) else { continue };
        // A soft-deleted variable is kept so the layers, styles and aliases that use it resolve, but it is not one of
        // its collection's variables: it stays out of the list.
        if active(e) && !vars.contains_key(&id) {
            listed.push(json!(id));
        }
        let mut v = json!({"id": id, "name": fstr(e, "name").unwrap_or("Variable"), "variableCollectionId": cid, "resolvedType": ty,
                   "valuesByMode": by_mode, "scopes": rest_scopes_from_fig(e.get("variableScopes")),
                   "description": fstr(e, "description").unwrap_or(""), "hiddenFromPublishing": fbool(e, "isPublishable") == Some(false)});
        if !active(e) {
            v[REST_DELETED_FLAG] = true.into();
        }
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
    for (ext, by_var) in overrides {
        if let Some(c) = colls.get_mut(&ext) {
            c["variableOverrides"] = Value::Object(by_var);
        }
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
    #[test]
    fn fig_stroke_scope_maps_to_rest_spelling_once_and_leaves_others_alone() {
        let set = fig_set(5, "Theme", "K", None);
        let var = |scopes: Value| {
            json!({"type": "VARIABLE", "guid": {"sessionID": 1, "localID": 9}, "name": "edge", "variableResolvedType": "COLOR",
                "variableSetID": {"guid": {"sessionID": 1, "localID": 5}}, "variableScopes": scopes})
        };
        let scopes_of = |scopes: Value| from_fig_entries(&[&set, &var(scopes)]).unwrap()["variables"]["VariableID:1:9"]["scopes"].clone();
        assert_eq!(scopes_of(json!(["STROKE"])), json!(["STROKE_COLOR"]));
        assert_eq!(scopes_of(json!(["ALL_FILLS", "STROKE", "STROKE_FLOAT"])), json!(["ALL_FILLS", "STROKE_COLOR", "STROKE_FLOAT"]));
        assert_eq!(scopes_of(json!(["STROKE", "ALL_FILLS", "STROKE_COLOR"])), json!(["STROKE_COLOR", "ALL_FILLS"]));
        assert_eq!(scopes_of(json!(["TEXT_FILL", "EFFECT_COLOR"])), json!(["TEXT_FILL", "EFFECT_COLOR"]));
        assert_eq!(scopes_of(json!([])), json!([]));
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
    fn exported_variables_in_subscribed_collections_are_remote() {
        let mut doc = Document::new("d");
        let local = fig_set(5, "Local", "K_LOCAL", None);
        let library = fig_set(20, "Library", "K_LIB", Some("lk-83c0"));
        let local_var = json!({"type": "VARIABLE", "guid": {"sessionID": 1, "localID": 9}, "name": "local", "variableResolvedType": "FLOAT",
            "variableSetID": {"guid": {"sessionID": 1, "localID": 5}}, "variableDataValues": {"entries": []}});
        let library_var = json!({"type": "VARIABLE", "guid": {"sessionID": 1, "localID": 30}, "name": "library", "variableResolvedType": "FLOAT",
            "variableSetID": {"assetRef": {"key": "K_LIB"}}, "variableDataValues": {"entries": []}});
        let payload = from_fig_entries(&[&local, &library, &local_var, &library_var]).unwrap();
        import_variables_value(&mut doc, &json!({"variables": payload}));

        let meta = export_variables_meta(&doc);
        assert_eq!(meta["variables"]["VariableID:1:9"]["remote"], false);
        assert_eq!(meta["variables"]["VariableID:1:30"]["remote"], true);
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
                extensions: Default::default(),
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
                extensions: Default::default(),
                mode_extensions: Default::default(),
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
                extensions: Default::default(),
                mode_extensions: Default::default(),
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
