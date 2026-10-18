//! Figma-native (`.tokens.json`) and W3C Design Tokens Community Group (DTCG 2025.10)
//! variable import, export, format detection, and validation (`E1-9` / Issue `#138`).

use crate::variables::{export_variables_json, import_variables_json};
use crate::FigError;
use designos_doc::{CodeSyntax, CodeSyntaxPlatform, Color, Document, Variable, VariableCollection, VariableKind, VariableMode, VariableValue};
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::io::{Cursor, Read, Write};

/// Supported variable serialization dialects (`E1-9` §3.1).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum VariablesFormat {
    #[default]
    Auto,
    FigmaRest,
    FigmaNative,
    Dtcg,
}

impl VariablesFormat {
    pub fn label(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::FigmaRest => "figma-rest",
            Self::FigmaNative => "figma-native",
            Self::Dtcg => "dtcg",
        }
    }

    pub fn parse_label(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "auto" => Some(Self::Auto),
            "figma-rest" | "rest" => Some(Self::FigmaRest),
            "figma-native" | "figma" | "native" => Some(Self::FigmaNative),
            "dtcg" | "w3c" | "tokens" => Some(Self::Dtcg),
            _ => None,
        }
    }
}

/// Options controlling how [`import_variables`] merges or resolves multi-file bundles (`E1-9` §3.3).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImportVariablesOptions {
    /// When `true`, replaces `doc.variables` and `doc.variable_collections` instead of merging.
    pub replace: bool,
    /// Optional collection name hint when importing a single-mode file that has no collection header.
    pub collection_name: Option<String>,
    /// Optional mode name derived from the filename (e.g. `Light.tokens.json` -> `"Light"`).
    pub mode_name: Option<String>,
    /// Additional `(filename, bytes)` files when importing a multi-file `figma-native` or `dtcg` set.
    pub additional_files: Vec<(String, Vec<u8>)>,
}

/// A token or property that could not be mapped cleanly or triggered a conversion/conflict notice (`E1-9` §3.3, §8).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SkippedVariable {
    pub path: String,
    pub reason: String,
}

/// Result of importing variables into a [`Document`] (`E1-9` §3.3, §4, §7).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImportReport {
    pub format: VariablesFormat,
    pub added: usize,
    pub updated: usize,
    pub unchanged: usize,
    pub skipped: Vec<SkippedVariable>,
}

impl ImportReport {
    pub fn total_imported(&self) -> usize {
        self.added + self.updated
    }

    /// Formats the UI toast message per `E1-9` §7:
    /// `"Imported 120 variables (4 skipped — details)"` or `"Imported 120 variables"`.
    pub fn toast_message(&self) -> String {
        let count = self.added + self.updated + self.unchanged;
        if self.skipped.is_empty() {
            format!("Imported {count} variables")
        } else {
            format!("Imported {count} variables ({} skipped — details)", self.skipped.len())
        }
    }
}

/// Output of [`export_variables`] (`E1-9` §4).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExportBundle {
    pub format: VariablesFormat,
    /// Generated `(filename, utf8_or_zip_bytes)` entries.
    pub files: Vec<(String, Vec<u8>)>,
}

impl ExportBundle {
    /// Packages all files in the bundle into a single `.zip` archive in memory.
    pub fn to_zip_bytes(&self) -> Result<Vec<u8>, FigError> {
        let mut buf = Cursor::new(Vec::new());
        {
            let mut zip = zip::ZipWriter::new(&mut buf);
            let opts = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
            for (name, bytes) in &self.files {
                zip.start_file(name, opts).map_err(|e| FigError::Format(format!("zip start_file {name}: {e}")))?;
                zip.write_all(bytes).map_err(|e| FigError::Format(format!("zip write {name}: {e}")))?;
            }
            zip.finish().map_err(|e| FigError::Format(format!("zip finish: {e}")))?;
        }
        Ok(buf.into_inner())
    }
}

/// Validation error reported by [`validate_dtcg_value`] (`E1-9` §6).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DtcgValidationError {
    pub path: String,
    pub message: String,
}

const GEOMETRIC_SCOPES: &[&str] =
    &["WIDTH_HEIGHT", "GAP", "CORNER_RADIUS", "STROKE_FLOAT", "FONT_SIZE", "LINE_HEIGHT", "LETTER_SPACING", "PARAGRAPH_SPACING", "PARAGRAPH_INDENT"];

fn is_all_geometric_scopes(scopes: &[String]) -> bool {
    !scopes.is_empty() && scopes.iter().all(|s| GEOMETRIC_SCOPES.contains(&s.as_str()))
}

/// Normalizes scope names between Figma editor (`STROKE`) and REST (`STROKE_COLOR`) (`E1-9` §14.8).
fn normalize_scope_on_import(scope: &str) -> String {
    match scope {
        "STROKE" => "STROKE_COLOR".to_string(),
        other => other.to_string(),
    }
}

fn figma_editor_scope(scope: &str) -> String {
    match scope {
        "STROKE_COLOR" => "STROKE".to_string(),
        other => other.to_string(),
    }
}

/// Detects which variable format `text` uses (`E1-9` §3.1).
pub fn detect_variables_format(text: &str) -> Result<VariablesFormat, FigError> {
    let v: Value = crate::rest_import::parse_json_text(text)?;
    detect_variables_format_value(&v)
}

pub fn detect_variables_format_value(v: &Value) -> Result<VariablesFormat, FigError> {
    let Some(obj) = v.as_object() else {
        return Err(FigError::Format("UNKNOWN_TOKENS_FORMAT: expected figma-rest, figma-native, or dtcg".into()));
    };

    // 1. REST: `meta.variables` / `meta.variableCollections` / `variableCollections`
    let is_rest = obj.get("meta").and_then(Value::as_object).is_some_and(|m| m.contains_key("variables") || m.contains_key("variableCollections"))
        || obj.contains_key("variableCollections")
        || obj.get("variables").and_then(Value::as_object).is_some_and(|inner| {
            inner.contains_key("variableCollections") || inner.get("meta").and_then(Value::as_object).is_some_and(|m| m.contains_key("variableCollections"))
        });
    if is_rest {
        return Ok(VariablesFormat::FigmaRest);
    }

    // 2. Figma-native: top-level `$extensions["com.figma.modeName"]` or any token with `com.figma.*`
    if has_figma_native_markers(v) {
        return Ok(VariablesFormat::FigmaNative);
    }

    // 3. DTCG: manifest (`schema == "designos-dtcg-manifest"` or `format == "dtcg"`), `$schema` pointing to DTCG, or any `$value`/`$type`
    if is_dtcg_manifest(obj)
        || obj.get("$schema").and_then(Value::as_str).is_some_and(|s| s.contains("designtokens") || s.contains("dtcg") || s.contains("design-tokens"))
        || has_dtcg_token_markers(v)
    {
        return Ok(VariablesFormat::Dtcg);
    }

    Err(FigError::Format("UNKNOWN_TOKENS_FORMAT: expected figma-rest, figma-native, or dtcg".into()))
}

fn is_dtcg_manifest(obj: &Map<String, Value>) -> bool {
    (obj.get("schema").and_then(Value::as_str) == Some("designos-dtcg-manifest") || obj.get("format").and_then(Value::as_str) == Some("dtcg"))
        && obj.get("collections").is_some_and(Value::is_array)
}

fn has_figma_native_markers(v: &Value) -> bool {
    let Some(obj) = v.as_object() else {
        return false;
    };
    if let Some(ext) = obj.get("$extensions").and_then(Value::as_object) {
        if ext.contains_key("com.figma.modeName") || ext.contains_key("com.figma.variableId") || ext.keys().any(|k| k.starts_with("com.figma.")) {
            return true;
        }
    }
    for (k, child) in obj {
        if !k.starts_with('$') && has_figma_native_markers(child) {
            return true;
        }
    }
    false
}

fn has_dtcg_token_markers(v: &Value) -> bool {
    let Some(obj) = v.as_object() else {
        return false;
    };
    if obj.contains_key("$value") || obj.contains_key("$type") {
        return true;
    }
    for (k, child) in obj {
        if !k.starts_with('$') && has_dtcg_token_markers(child) {
            return true;
        }
    }
    false
}

/// Strips `.tokens.json`, `.tokens (N).json`, or `.json` from a file name (`E1-9` §14.2).
pub fn mode_name_from_filename(filename: &str) -> String {
    let base = filename.rsplit(['/', '\\']).next().unwrap_or(filename).trim();
    if let Some(stem) = base.strip_suffix(".tokens.json") {
        // Also handle `<collection>.<mode>.tokens.json`
        if let Some((_, mode)) = stem.rsplit_once('.') {
            if !mode.is_empty() {
                return mode.to_string();
            }
        }
        return stem.to_string();
    }
    if let Some(stem) = base.strip_suffix(".json") {
        if let Some(idx) = stem.find(".tokens (") {
            if stem.ends_with(')') {
                return stem[..idx].to_string();
            }
        }
        return stem.to_string();
    }
    if base.is_empty() {
        "Mode 1".to_string()
    } else {
        base.to_string()
    }
}

// ============================================================================
// DTCG 2025.10 Validator (`E1-9` §6)
// ============================================================================

const VALID_DTCG_TYPES: &[&str] = &[
    "color",
    "dimension",
    "fontFamily",
    "fontWeight",
    "duration",
    "cubicBezier",
    "number",
    "strokeStyle",
    "border",
    "transition",
    "shadow",
    "gradient",
    "typography",
];

/// Validates a parsed JSON value against W3C DTCG 2025.10 structural, naming, type, and alias rules (`E1-9` §6).
pub fn validate_dtcg_value(root: &Value) -> Vec<DtcgValidationError> {
    let mut errors = Vec::new();
    let Some(obj) = root.as_object() else {
        errors.push(DtcgValidationError { path: String::new(), message: "root must be a JSON object".into() });
        return errors;
    };
    if is_dtcg_manifest(obj) {
        return errors;
    }
    validate_dtcg_group(obj, "", None, &mut errors);
    errors
}

pub fn validate_dtcg_json(text: &str) -> Result<Vec<DtcgValidationError>, FigError> {
    let v = crate::rest_import::parse_json_text(text)?;
    Ok(validate_dtcg_value(&v))
}

fn validate_dtcg_group(obj: &Map<String, Value>, path: &str, inherited_type: Option<&str>, errors: &mut Vec<DtcgValidationError>) {
    let is_token = obj.contains_key("$value");
    let local_type = obj.get("$type").and_then(Value::as_str);
    if let Some(ty) = local_type {
        if !VALID_DTCG_TYPES.contains(&ty) {
            errors.push(DtcgValidationError { path: path.to_string(), message: format!("invalid DTCG $type '{ty}'") });
        }
    }
    let effective_type = local_type.or(inherited_type);

    // Check for forbidden com.figma.* keys in strict DTCG
    if let Some(ext) = obj.get("$extensions").and_then(Value::as_object) {
        for k in ext.keys() {
            if k.starts_with("com.figma.") {
                errors.push(DtcgValidationError { path: path.to_string(), message: format!("forbidden Figma key '{k}' in strict DTCG $extensions") });
            }
        }
    }

    if is_token {
        // Token must not also contain non-`$` child groups/tokens
        for k in obj.keys() {
            if !k.starts_with('$') {
                errors.push(DtcgValidationError { path: path.to_string(), message: format!("token object must not contain child group/token key '{k}'") });
            }
        }
        let val = &obj["$value"];
        validate_dtcg_token_value(val, effective_type, obj, path, errors);
        return;
    }

    for (k, child) in obj {
        if k == "$root" {
            if let Some(child_obj) = child.as_object() {
                let child_path = if path.is_empty() { "$root".to_string() } else { format!("{path}.$root") };
                validate_dtcg_group(child_obj, &child_path, effective_type, errors);
            }
            continue;
        }
        if k.starts_with('$') {
            continue;
        }
        // DTCG name restrictions: no `{`, `}`, `.` and no leading `$`
        if k.is_empty() || k.contains('{') || k.contains('}') || k.contains('.') {
            errors.push(DtcgValidationError {
                path: if path.is_empty() { k.clone() } else { format!("{path}.{k}") },
                message: format!("invalid DTCG group/token name '{k}' (must not contain '{{', '}}', or '.')"),
            });
        }
        let child_path = if path.is_empty() { k.clone() } else { format!("{path}.{k}") };
        if let Some(child_obj) = child.as_object() {
            validate_dtcg_group(child_obj, &child_path, effective_type, errors);
        } else {
            errors.push(DtcgValidationError { path: child_path, message: "group member must be a JSON object".into() });
        }
    }
}

fn is_valid_dtcg_alias_str(s: &str) -> bool {
    if !s.starts_with('{') || !s.ends_with('}') {
        return false;
    }
    let inner = &s[1..s.len() - 1];
    !inner.is_empty() && !inner.contains('{') && !inner.contains('}') && inner.split('.').all(|seg| !seg.is_empty())
}

fn validate_dtcg_token_value(val: &Value, effective_type: Option<&str>, token_obj: &Map<String, Value>, path: &str, errors: &mut Vec<DtcgValidationError>) {
    if let Some(s) = val.as_str() {
        if s.starts_with('{') || s.ends_with('}') {
            if !is_valid_dtcg_alias_str(s) {
                errors.push(DtcgValidationError { path: path.to_string(), message: format!("malformed DTCG alias reference '{s}'") });
            }
            return;
        }
    }

    match effective_type {
        Some("color") => {
            let Some(co) = val.as_object() else {
                errors.push(DtcgValidationError { path: path.to_string(), message: "color $value must be a 2025.10 colour object or alias".into() });
                return;
            };
            let cs = co.get("colorSpace").and_then(Value::as_str).unwrap_or("");
            let comps_ok = co.get("components").and_then(Value::as_array).is_some_and(|a| a.len() == 3 && a.iter().all(|x| x.is_number()));
            if cs.is_empty() || !comps_ok {
                errors.push(DtcgValidationError { path: path.to_string(), message: "color $value requires 'colorSpace' and 3 numeric 'components'".into() });
            }
        }
        Some("dimension") => {
            let ok = val
                .as_object()
                .is_some_and(|o| o.get("value").is_some_and(Value::is_number) && matches!(o.get("unit").and_then(Value::as_str), Some("px" | "rem")));
            if !ok {
                errors.push(DtcgValidationError {
                    path: path.to_string(),
                    message: "dimension $value requires numeric 'value' and 'unit' ('px' or 'rem')".into(),
                });
            }
        }
        Some("number") => {
            if !val.is_number() {
                errors.push(DtcgValidationError { path: path.to_string(), message: "number $value must be a JSON number".into() });
            }
        }
        Some("fontFamily") => {
            let ok = val.is_string() || val.as_array().is_some_and(|a| !a.is_empty() && a.iter().all(Value::is_string));
            if !ok {
                errors.push(DtcgValidationError { path: path.to_string(), message: "fontFamily $value must be a string or array of strings".into() });
            }
        }
        Some("duration") => {
            let ok = val
                .as_object()
                .is_some_and(|o| o.get("value").is_some_and(Value::is_number) && matches!(o.get("unit").and_then(Value::as_str), Some("ms" | "s")));
            if !ok {
                errors
                    .push(DtcgValidationError { path: path.to_string(), message: "duration $value requires numeric 'value' and 'unit' ('ms' or 's')".into() });
            }
        }
        None => {
            // Untyped tokens are allowed for plain strings and booleans carrying `$extensions["org.designos"]`
            let has_designos_ext = token_obj
                .get("$extensions")
                .and_then(Value::as_object)
                .and_then(|e| e.get("org.designos"))
                .and_then(Value::as_object)
                .and_then(|d| d.get("kind"))
                .and_then(Value::as_str)
                .is_some_and(|k| matches!(k, "boolean" | "string"));
            if !has_designos_ext && !val.is_string() && !val.is_boolean() {
                errors.push(DtcgValidationError { path: path.to_string(), message: "token without $type must carry org.designos kind extension".into() });
            }
        }
        _ => {}
    }
}

// ============================================================================
// Export (`figma-rest`, `figma-native`, `dtcg`)
// ============================================================================

fn clamp01(x: f32) -> f32 {
    if x.is_finite() {
        x.clamp(0.0, 1.0)
    } else {
        0.0
    }
}

fn finite_f64(x: f32) -> f64 {
    if x.is_finite() {
        f64::from(x)
    } else {
        0.0
    }
}

fn color_to_dtcg_obj(c: Color) -> Value {
    let r = clamp01(c.r);
    let g = clamp01(c.g);
    let b = clamp01(c.b);
    let a = if c.a.is_finite() { c.a.clamp(0.0, 1.0) } else { 1.0 };
    let ri = (r * 255.0).round() as u8;
    let gi = (g * 255.0).round() as u8;
    let bi = (b * 255.0).round() as u8;
    let hex = format!("#{ri:02X}{gi:02X}{bi:02X}");
    json!({
        "colorSpace": "srgb",
        "components": [r, g, b],
        "alpha": a,
        "hex": hex,
    })
}

fn sanitize_dtcg_segment(seg: &str) -> (String, bool) {
    let mut changed = false;
    let mut out = String::with_capacity(seg.len());
    for (idx, ch) in seg.chars().enumerate() {
        if (idx == 0 && ch == '$') || ch == '.' || ch == '{' || ch == '}' {
            out.push('_');
            changed = true;
        } else {
            out.push(ch);
        }
    }
    if out.is_empty() {
        ("_".to_string(), true)
    } else {
        (out, changed)
    }
}

fn sanitize_dtcg_path(name: &str) -> (Vec<String>, bool) {
    let mut any_changed = false;
    let segs: Vec<String> = name
        .split('/')
        .map(|s| {
            let (clean, ch) = sanitize_dtcg_segment(s);
            any_changed |= ch;
            clean
        })
        .collect();
    (segs, any_changed)
}

/// Resolves the literal value of `var_id` in `mode_id` (falling back to collection default mode).
fn resolve_literal_in_mode(doc: &Document, var_id: &str, mode_id: &str) -> Option<(VariableKind, VariableValue)> {
    let mut cur_id = var_id.to_string();
    let mut cur_mode = mode_id.to_string();
    for _ in 0..16 {
        let v = doc.variables.get(&cur_id)?;
        let coll = doc.variable_collections.get(&v.collection)?;
        let val = v.values.get(&cur_mode).or_else(|| v.values.get(&coll.default_mode)).or_else(|| v.values.values().next())?;
        match val {
            VariableValue::Alias(next_id) => {
                let next_var = doc.variables.get(next_id)?;
                if next_var.collection != v.collection {
                    if let Some(next_coll) = doc.variable_collections.get(&next_var.collection) {
                        cur_mode = next_coll
                            .modes
                            .iter()
                            .find(|m| coll.modes.iter().find(|cm| cm.id == cur_mode).is_some_and(|cm| cm.name == m.name))
                            .map(|m| m.id.clone())
                            .unwrap_or_else(|| next_coll.default_mode.clone());
                    }
                }
                cur_id = next_id.clone();
            }
            other => return Some((v.kind, other.clone())),
        }
    }
    None
}

/// Inserts a token object into a nested JSON group tree at `segments`.
/// If `is_group_prefix` is true (another variable has `segments` as a strict prefix), inserts at `$root`.
fn insert_token_at_path(root: &mut Map<String, Value>, segments: &[String], is_group_prefix: bool, token: Value) {
    if segments.is_empty() {
        return;
    }
    let mut cur = root;
    for seg in &segments[..segments.len() - 1] {
        let entry = cur.entry(seg.clone()).or_insert_with(|| Value::Object(Map::new()));
        if !entry.is_object() {
            *entry = Value::Object(Map::new());
        }
        cur = entry.as_object_mut().unwrap();
    }
    let last = &segments[segments.len() - 1];
    if is_group_prefix {
        let entry = cur.entry(last.clone()).or_insert_with(|| Value::Object(Map::new()));
        if !entry.is_object() {
            *entry = Value::Object(Map::new());
        }
        entry.as_object_mut().unwrap().insert("$root".to_string(), token);
    } else {
        cur.insert(last.clone(), token);
    }
}

/// Computes the set of variable names in `vars` that are strict group prefixes of another variable in `vars`.
fn compute_group_prefixes(vars: &[&Variable]) -> BTreeSet<String> {
    let mut prefixes = BTreeSet::new();
    for v in vars {
        let mut acc = String::new();
        let parts: Vec<&str> = v.name.split('/').collect();
        for seg in &parts[..parts.len().saturating_sub(1)] {
            if !acc.is_empty() {
                acc.push('/');
            }
            acc.push_str(seg);
            prefixes.insert(acc.clone());
        }
    }
    prefixes
}

/// Exports variables from `doc` in the requested [`VariablesFormat`] (`E1-9` §3.1, §3.2, §4, §14).
pub fn export_variables(doc: &Document, format: VariablesFormat) -> Result<ExportBundle, FigError> {
    match format {
        VariablesFormat::Auto | VariablesFormat::FigmaRest => {
            let val = export_variables_json(doc);
            let bytes = serde_json::to_vec_pretty(&val)?;
            Ok(ExportBundle { format: VariablesFormat::FigmaRest, files: vec![("variables.json".to_string(), bytes)] })
        }
        VariablesFormat::FigmaNative => export_figma_native(doc),
        VariablesFormat::Dtcg => export_dtcg(doc),
    }
}

fn ordered_collection_variables<'a>(doc: &'a Document, coll: &VariableCollection) -> Vec<&'a Variable> {
    let mut out = Vec::new();
    let mut seen = BTreeSet::new();
    for vid in &coll.variables {
        if let Some(v) = doc.variables.get(vid) {
            if v.collection == coll.id && seen.insert(v.id.as_str()) {
                out.push(v);
            }
        }
    }
    for v in doc.variables.values() {
        if v.collection == coll.id && seen.insert(v.id.as_str()) {
            out.push(v);
        }
    }
    out
}

fn export_figma_native(doc: &Document) -> Result<ExportBundle, FigError> {
    let mut files = Vec::new();
    let multi_coll = doc.variable_collections.len() > 1;
    let mut used_filenames = BTreeSet::new();

    for coll in doc.variable_collections.values() {
        let vars = ordered_collection_variables(doc, coll);
        let group_prefixes = compute_group_prefixes(&vars);

        for mode in &coll.modes {
            let mut root = Map::new();
            for v in &vars {
                let Some(val) = v.values.get(&mode.id).or_else(|| v.values.get(&coll.default_mode)) else {
                    continue;
                };

                let mut ext = Map::new();
                ext.insert("com.figma.variableId".into(), Value::String(v.id.clone()));
                if !v.scopes.is_empty() {
                    let scopes: Vec<Value> = v.scopes.iter().map(|s| Value::String(figma_editor_scope(s))).collect();
                    ext.insert("com.figma.scopes".into(), Value::Array(scopes));
                }
                if !v.code_syntax.is_empty() {
                    let cs: Map<String, Value> = v.code_syntax.iter().map(|(p, code)| (p.label().to_string(), Value::String(code.clone()))).collect();
                    ext.insert("com.figma.codeSyntax".into(), Value::Object(cs));
                }
                if v.hidden_from_publishing {
                    ext.insert("com.figma.hiddenFromPublishing".into(), Value::Bool(true));
                }
                if let Some(key) = &v.key {
                    ext.insert("com.figma.key".into(), Value::String(key.clone()));
                }

                let (ty_str, val_json) = match val {
                    VariableValue::Color(c) => ("color", color_to_dtcg_obj(*c)),
                    VariableValue::Number(n) => ("number", json!(finite_f64(*n))),
                    VariableValue::Boolean(b) => {
                        ext.insert("com.figma.type".into(), Value::String("boolean".into()));
                        ("number", json!(if *b { 1 } else { 0 }))
                    }
                    VariableValue::String(s) => {
                        ext.insert("com.figma.type".into(), Value::String("string".into()));
                        ("string", Value::String(s.clone()))
                    }
                    VariableValue::Alias(target_id) => {
                        if let Some(target_var) = doc.variables.get(target_id) {
                            match target_var.kind {
                                VariableKind::Boolean => {
                                    ext.insert("com.figma.type".into(), Value::String("boolean".into()));
                                }
                                VariableKind::String => {
                                    ext.insert("com.figma.type".into(), Value::String("string".into()));
                                }
                                _ => {}
                            }
                            if target_var.collection == coll.id {
                                // Same-collection alias: `{Group.Sub.Name}` (or `{Group.Sub.$root}`)
                                let mut ref_path = target_var.name.replace('/', ".");
                                if group_prefixes.contains(&target_var.name) {
                                    ref_path.push_str(".$root");
                                }
                                let ty = match target_var.kind {
                                    VariableKind::Color => "color",
                                    VariableKind::Number | VariableKind::Boolean => "number",
                                    VariableKind::String => "string",
                                };
                                (ty, Value::String(format!("{{{ref_path}}}")))
                            } else {
                                // Cross-collection alias: resolved literal + com.figma.aliasData (§14.6)
                                let target_coll = doc.variable_collections.get(&target_var.collection);
                                let target_var_id_or_key = target_var.key.clone().unwrap_or_else(|| target_var.id.clone());
                                ext.insert(
                                    "com.figma.aliasData".into(),
                                    json!({
                                        "targetVariableId": target_var_id_or_key,
                                        "targetVariableName": target_var.name,
                                        "targetVariableSetId": target_var.collection,
                                        "targetVariableSetName": target_coll.map(|c| c.name.as_str()).unwrap_or("Collection"),
                                    }),
                                );
                                match resolve_literal_in_mode(doc, target_id, &mode.id) {
                                    Some((_, VariableValue::Color(c))) => ("color", color_to_dtcg_obj(c)),
                                    Some((_, VariableValue::Number(n))) => ("number", json!(finite_f64(n))),
                                    Some((_, VariableValue::Boolean(b))) => ("number", json!(if b { 1 } else { 0 })),
                                    Some((_, VariableValue::String(s))) => ("string", Value::String(s)),
                                    _ => ("string", Value::String(String::new())),
                                }
                            }
                        } else {
                            ("string", Value::String(format!("{{{target_id}}}")))
                        }
                    }
                };

                let mut token = Map::new();
                token.insert("$type".into(), Value::String(ty_str.into()));
                token.insert("$value".into(), val_json);
                if !v.description.is_empty() {
                    token.insert("$description".into(), Value::String(v.description.clone()));
                }
                token.insert("$extensions".into(), Value::Object(ext));

                let segments: Vec<String> = v.name.split('/').map(String::from).collect();
                let is_prefix = group_prefixes.contains(&v.name);
                insert_token_at_path(&mut root, &segments, is_prefix, Value::Object(token));
            }

            let mut root_ext = Map::new();
            root_ext.insert("com.figma.modeName".into(), Value::String(mode.name.clone()));
            root_ext.insert("com.figma.modeId".into(), Value::String(mode.id.clone()));
            root_ext.insert("com.figma.collectionId".into(), Value::String(coll.id.clone()));
            root_ext.insert("com.figma.collectionName".into(), Value::String(coll.name.clone()));
            if let Some(k) = &coll.key {
                root_ext.insert("com.figma.collectionKey".into(), Value::String(k.clone()));
            }
            if let Some(lib) = &coll.source_library_key {
                root_ext.insert("com.figma.sourceLibraryKey".into(), Value::String(lib.clone()));
            }
            if coll.default_mode == mode.id {
                root_ext.insert("com.figma.isDefaultMode".into(), Value::Bool(true));
            }
            root.insert("$extensions".into(), Value::Object(root_ext));

            let base_name = if multi_coll { format!("{}.{}.tokens.json", coll.name, mode.name) } else { format!("{}.tokens.json", mode.name) };
            let mut file_name = base_name.clone();
            let mut counter = 2;
            while !used_filenames.insert(file_name.clone()) {
                file_name = format!("{}.tokens ({counter}).json", mode.name);
                counter += 1;
            }
            let bytes = serde_json::to_vec_pretty(&Value::Object(root))?;
            files.push((file_name, bytes));
        }
    }

    Ok(ExportBundle { format: VariablesFormat::FigmaNative, files })
}

fn export_dtcg(doc: &Document) -> Result<ExportBundle, FigError> {
    let mut files = Vec::new();
    let mut manifest_collections = Vec::new();

    // Map each collection id -> sanitized DTCG group name
    let mut coll_group_names: BTreeMap<String, String> = BTreeMap::new();
    for coll in doc.variable_collections.values() {
        let (clean_coll, _) = sanitize_dtcg_segment(&coll.name);
        coll_group_names.insert(coll.id.clone(), clean_coll);
    }

    // Precompute group prefixes per collection for `$root` handling
    let mut coll_prefixes: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for coll in doc.variable_collections.values() {
        let vars = ordered_collection_variables(doc, coll);
        coll_prefixes.insert(coll.id.clone(), compute_group_prefixes(&vars));
    }

    for coll in doc.variable_collections.values() {
        let vars = ordered_collection_variables(doc, coll);
        let group_prefixes = &coll_prefixes[&coll.id];
        let clean_coll_name = &coll_group_names[&coll.id];
        let mut mode_entries = Vec::new();

        for mode in &coll.modes {
            let mut coll_group = Map::new();

            for v in &vars {
                let Some(val) = v.values.get(&mode.id).or_else(|| v.values.get(&coll.default_mode)) else {
                    continue;
                };

                let (segments, name_escaped) = sanitize_dtcg_path(&v.name);
                let is_prefix = group_prefixes.contains(&v.name);

                let mut dos_ext = Map::new();
                dos_ext.insert("id".into(), Value::String(v.id.clone()));
                dos_ext.insert("collection_id".into(), Value::String(coll.id.clone()));
                if name_escaped {
                    dos_ext.insert("original_name".into(), Value::String(v.name.clone()));
                }
                if !v.scopes.is_empty() {
                    dos_ext.insert("scopes".into(), json!(v.scopes));
                }
                if !v.code_syntax.is_empty() {
                    let cs: Map<String, Value> = v.code_syntax.iter().map(|(p, code)| (p.label().to_string(), Value::String(code.clone()))).collect();
                    dos_ext.insert("code_syntax".into(), Value::Object(cs));
                }
                if v.hidden_from_publishing {
                    dos_ext.insert("hidden_from_publishing".into(), Value::Bool(true));
                }
                if let Some(k) = &v.key {
                    dos_ext.insert("key".into(), Value::String(k.clone()));
                }

                let (opt_type, val_json) = match val {
                    VariableValue::Color(c) => (Some("color"), color_to_dtcg_obj(*c)),
                    VariableValue::Number(n) => {
                        if is_all_geometric_scopes(&v.scopes) {
                            (Some("dimension"), json!({ "value": finite_f64(*n), "unit": "px" }))
                        } else {
                            (Some("number"), json!(finite_f64(*n)))
                        }
                    }
                    VariableValue::String(s) => {
                        if v.scopes.iter().any(|sc| sc == "FONT_FAMILY") {
                            (Some("fontFamily"), Value::String(s.clone()))
                        } else {
                            dos_ext.insert("kind".into(), Value::String("string".into()));
                            (None, Value::String(s.clone()))
                        }
                    }
                    VariableValue::Boolean(b) => {
                        dos_ext.insert("kind".into(), Value::String("boolean".into()));
                        dos_ext.insert("boolean".into(), Value::Bool(*b));
                        (None, Value::Bool(*b))
                    }
                    VariableValue::Alias(target_id) => {
                        let ref_str = if let Some(target_var) = doc.variables.get(target_id) {
                            let target_coll_group = coll_group_names.get(&target_var.collection).cloned().unwrap_or_else(|| "Collection".to_string());
                            let (target_segs, _) = sanitize_dtcg_path(&target_var.name);
                            let mut full_ref = format!("{}.{}", target_coll_group, target_segs.join("."));
                            if coll_prefixes.get(&target_var.collection).is_some_and(|p| p.contains(&target_var.name)) {
                                full_ref.push_str(".$root");
                            }
                            format!("{{{full_ref}}}")
                        } else {
                            let (clean_id, _) = sanitize_dtcg_segment(target_id);
                            format!("{{{clean_id}}}")
                        };
                        let opt_ty = match v.kind {
                            VariableKind::Color => Some("color"),
                            VariableKind::Number => {
                                if is_all_geometric_scopes(&v.scopes) {
                                    Some("dimension")
                                } else {
                                    Some("number")
                                }
                            }
                            VariableKind::String => {
                                if v.scopes.iter().any(|sc| sc == "FONT_FAMILY") {
                                    Some("fontFamily")
                                } else {
                                    dos_ext.insert("kind".into(), Value::String("string".into()));
                                    None
                                }
                            }
                            VariableKind::Boolean => {
                                dos_ext.insert("kind".into(), Value::String("boolean".into()));
                                None
                            }
                        };
                        (opt_ty, Value::String(ref_str))
                    }
                };

                let mut token = Map::new();
                if let Some(ty) = opt_type {
                    token.insert("$type".into(), Value::String(ty.into()));
                }
                token.insert("$value".into(), val_json);
                if !v.description.is_empty() {
                    token.insert("$description".into(), Value::String(v.description.clone()));
                }
                let mut ext = Map::new();
                ext.insert("org.designos".into(), Value::Object(dos_ext));
                token.insert("$extensions".into(), Value::Object(ext));

                insert_token_at_path(&mut coll_group, &segments, is_prefix, Value::Object(token));
            }

            let mut file_root = Map::new();
            file_root.insert(clean_coll_name.clone(), Value::Object(coll_group));

            let (clean_mode_file, _) = sanitize_dtcg_segment(&mode.name);
            let file_name = format!("{clean_coll_name}.{clean_mode_file}.tokens.json");
            let bytes = serde_json::to_vec_pretty(&Value::Object(file_root))?;
            files.push((file_name.clone(), bytes));

            mode_entries.push(json!({
                "id": mode.id,
                "name": mode.name,
                "file": file_name,
            }));
        }

        manifest_collections.push(json!({
            "id": coll.id,
            "name": coll.name,
            "group": clean_coll_name,
            "default_mode": coll.default_mode,
            "key": coll.key,
            "source_library_key": coll.source_library_key,
            "modes": mode_entries,
        }));
    }

    let manifest = json!({
        "schema": "designos-dtcg-manifest",
        "format": "dtcg",
        "version": "2025.10",
        "collections": manifest_collections,
    });
    let manifest_bytes = serde_json::to_vec_pretty(&manifest)?;
    files.insert(0, ("tokens.manifest.json".to_string(), manifest_bytes));

    Ok(ExportBundle { format: VariablesFormat::Dtcg, files })
}

// ============================================================================
// Import (`figma-rest`, `figma-native`, `dtcg`)
// ============================================================================

/// Imports variables from raw bytes (JSON or `.zip` bundle of `.tokens.json` files) (`E1-9` §3.1–§3.3).
pub fn import_variables_bytes(
    doc: &mut Document,
    bytes: &[u8],
    file_name: Option<&str>,
    format: VariablesFormat,
    mut opts: ImportVariablesOptions,
) -> Result<ImportReport, FigError> {
    if opts.mode_name.is_none() {
        if let Some(name) = file_name {
            if !name.ends_with(".zip") && !name.ends_with("tokens.manifest.json") {
                opts.mode_name = Some(mode_name_from_filename(name));
            }
        }
    }

    // Check for ZIP magic header `PK\x03\x04`
    if bytes.starts_with(b"PK\x03\x04") {
        let reader = Cursor::new(bytes);
        let mut archive = zip::ZipArchive::new(reader).map_err(|e| FigError::Format(format!("invalid zip archive: {e}")))?;
        let mut extracted: Vec<(String, Vec<u8>)> = Vec::new();
        for idx in 0..archive.len() {
            let mut file = archive.by_index(idx).map_err(|e| FigError::Format(format!("zip entry {idx}: {e}")))?;
            let entry_name = file.name().map_err(|e| FigError::Format(format!("zip entry {idx} name: {e}")))?.to_string();
            if file.is_dir() || !entry_name.ends_with(".json") {
                continue;
            }
            let mut buf = Vec::new();
            file.read_to_end(&mut buf).map_err(|e| FigError::Format(format!("zip read {entry_name}: {e}")))?;
            extracted.push((entry_name, buf));
        }
        if extracted.is_empty() {
            return Err(FigError::Format("no .json files found in zip archive".into()));
        }
        // Prefer tokens.manifest.json as primary if present
        let primary_idx = extracted.iter().position(|(n, _)| n.ends_with("tokens.manifest.json")).unwrap_or(0);
        let (primary_name, primary_bytes) = extracted.remove(primary_idx);
        opts.additional_files.extend(extracted);
        if opts.mode_name.is_none() && !primary_name.ends_with("tokens.manifest.json") {
            opts.mode_name = Some(mode_name_from_filename(&primary_name));
        }
        let primary_text = std::str::from_utf8(&primary_bytes).map_err(|e| FigError::Format(format!("invalid UTF-8 in {primary_name}: {e}")))?;
        return import_variables(doc, primary_text, format, opts);
    }

    let text = std::str::from_utf8(bytes).map_err(|e| FigError::Format(format!("invalid UTF-8 in variables file: {e}")))?;
    import_variables(doc, text, format, opts)
}

/// Imports variables from JSON text in `figma-rest`, `figma-native`, or `dtcg` format (`E1-9` §3.1–§3.3, §4).
pub fn import_variables(doc: &mut Document, text: &str, format: VariablesFormat, opts: ImportVariablesOptions) -> Result<ImportReport, FigError> {
    let resolved_format = match format {
        VariablesFormat::Auto => detect_variables_format(text)?,
        explicit => explicit,
    };

    match resolved_format {
        VariablesFormat::Auto => unreachable!(),
        VariablesFormat::FigmaRest => import_rest_with_report(doc, text, &opts),
        VariablesFormat::FigmaNative => import_figma_native_files(doc, text, &opts),
        VariablesFormat::Dtcg => import_dtcg_files(doc, text, &opts),
    }
}

fn import_rest_with_report(doc: &mut Document, text: &str, opts: &ImportVariablesOptions) -> Result<ImportReport, FigError> {
    let mut staging = Document::new("staging");
    let _ = import_variables_json(&mut staging, text)?;
    let mut report = ImportReport { format: VariablesFormat::FigmaRest, ..Default::default() };
    merge_staged_into_doc(doc, staging, opts.replace, &mut report);
    Ok(report)
}

fn merge_staged_into_doc(doc: &mut Document, staging: Document, replace: bool, report: &mut ImportReport) {
    if replace {
        report.added = staging.variables.len();
        doc.variable_collections = staging.variable_collections;
        doc.variables = staging.variables;
        return;
    }

    // Map staged collection id -> destination collection id in `doc`
    let mut coll_id_map: HashMap<String, String> = HashMap::new();
    // Map (dest_coll_id, staged_mode_id) -> dest_mode_id
    let mut mode_id_map: HashMap<(String, String), String> = HashMap::new();

    for (_, staged_coll) in staging.variable_collections {
        let existing_coll_id = if doc.variable_collections.contains_key(&staged_coll.id) {
            Some(staged_coll.id.clone())
        } else {
            doc.variable_collections.values().find(|c| c.name == staged_coll.name).map(|c| c.id.clone())
        };

        if let Some(dest_cid) = existing_coll_id {
            coll_id_map.insert(staged_coll.id.clone(), dest_cid.clone());
            let dest_coll = doc.variable_collections.get_mut(&dest_cid).unwrap();
            if dest_coll.key.is_none() && staged_coll.key.is_some() {
                dest_coll.key = staged_coll.key.clone();
            }
            if dest_coll.source_library_key.is_none() && staged_coll.source_library_key.is_some() {
                dest_coll.source_library_key = staged_coll.source_library_key.clone();
            }
            for sm in &staged_coll.modes {
                if let Some(existing_mode) = dest_coll.modes.iter().find(|m| m.id == sm.id || m.name == sm.name) {
                    mode_id_map.insert((dest_cid.clone(), sm.id.clone()), existing_mode.id.clone());
                } else {
                    dest_coll.modes.push(sm.clone());
                    mode_id_map.insert((dest_cid.clone(), sm.id.clone()), sm.id.clone());
                }
            }
        } else {
            let dest_cid = staged_coll.id.clone();
            coll_id_map.insert(staged_coll.id.clone(), dest_cid.clone());
            for sm in &staged_coll.modes {
                mode_id_map.insert((dest_cid.clone(), sm.id.clone()), sm.id.clone());
            }
            let mut new_coll = staged_coll;
            new_coll.variables.clear();
            doc.variable_collections.insert(dest_cid, new_coll);
        }
    }

    // First pass: map staged variable ids -> destination variable ids
    let mut var_id_map: HashMap<String, String> = HashMap::new();
    for staged_var in staging.variables.values() {
        let dest_cid = coll_id_map.get(&staged_var.collection).cloned().unwrap_or_else(|| staged_var.collection.clone());
        let existing_vid = if doc.variables.contains_key(&staged_var.id) {
            Some(staged_var.id.clone())
        } else {
            doc.variables.values().find(|v| v.collection == dest_cid && v.name == staged_var.name).map(|v| v.id.clone())
        };
        let dest_vid = existing_vid.unwrap_or_else(|| staged_var.id.clone());
        var_id_map.insert(staged_var.id.clone(), dest_vid);
    }

    // Second pass: insert or merge each variable
    for (_, mut staged_var) in staging.variables {
        let dest_cid = coll_id_map.get(&staged_var.collection).cloned().unwrap_or_else(|| staged_var.collection.clone());
        let dest_vid = var_id_map.get(&staged_var.id).cloned().unwrap_or_else(|| staged_var.id.clone());

        let mut mapped_values = BTreeMap::new();
        for (smode, sval) in staged_var.values {
            let dmode = mode_id_map.get(&(dest_cid.clone(), smode.clone())).cloned().unwrap_or(smode);
            let dval = match sval {
                VariableValue::Alias(tid) => VariableValue::Alias(var_id_map.get(&tid).cloned().unwrap_or(tid)),
                other => other,
            };
            mapped_values.insert(dmode, dval);
        }

        staged_var.id = dest_vid.clone();
        staged_var.collection = dest_cid.clone();
        staged_var.values = mapped_values;

        if let Some(existing) = doc.variables.get_mut(&dest_vid) {
            let before = existing.clone();
            existing.name = staged_var.name;
            existing.kind = staged_var.kind;
            for (m, v) in staged_var.values {
                existing.values.insert(m, v);
            }
            if !staged_var.description.is_empty() {
                existing.description = staged_var.description;
            }
            if !staged_var.scopes.is_empty() {
                existing.scopes = staged_var.scopes;
            }
            existing.hidden_from_publishing = staged_var.hidden_from_publishing;
            if staged_var.key.is_some() {
                existing.key = staged_var.key;
            }
            for (p, code) in staged_var.code_syntax {
                existing.code_syntax.insert(p, code);
            }
            if *existing == before {
                report.unchanged += 1;
            } else {
                report.updated += 1;
            }
        } else {
            doc.variables.insert(dest_vid.clone(), staged_var);
            report.added += 1;
        }

        if let Some(coll) = doc.variable_collections.get_mut(&dest_cid) {
            if !coll.variables.contains(&dest_vid) {
                coll.variables.push(dest_vid);
            }
        }
    }
}

// ============================================================================
// Shared Token Parsing Helpers (`figma-native` & `dtcg`)
// ============================================================================

#[derive(Clone, Debug)]
struct ParsedToken {
    collection_id: String,
    collection_name: String,
    collection_key: Option<String>,
    source_library_key: Option<String>,
    mode_id: String,
    mode_name: String,
    is_default_mode: bool,
    var_id: String,
    var_name: String,
    kind: VariableKind,
    raw_value: RawTokenValue,
    description: String,
    scopes: Vec<String>,
    hidden_from_publishing: bool,
    key: Option<String>,
    code_syntax: CodeSyntax,
}

#[derive(Clone, Debug)]
enum RawTokenValue {
    Literal(VariableValue),
    /// Reference path `{a.b.c}` with optional fallback literal value.
    AliasPath {
        target_path: String,
        same_collection_only: bool,
    },
    /// Cross-collection `com.figma.aliasData` with fallback literal value.
    FigmaCrossAlias {
        target_id: String,
        target_name: String,
        target_set_id: Option<String>,
        target_set_name: Option<String>,
        fallback: VariableValue,
    },
}

fn hsl_to_rgb(h_deg: f32, s_pct: f32, l_pct: f32) -> (f32, f32, f32) {
    let h = ((h_deg % 360.0) + 360.0) % 360.0;
    let s = (s_pct / 100.0).clamp(0.0, 1.0);
    let l = (l_pct / 100.0).clamp(0.0, 1.0);
    let c = (1.0 - (2.0 * l - 1.0).abs()) * s;
    let x = c * (1.0 - (((h / 60.0) % 2.0) - 1.0).abs());
    let m = l - c / 2.0;
    let (r1, g1, b1) = if h < 60.0 {
        (c, x, 0.0)
    } else if h < 120.0 {
        (x, c, 0.0)
    } else if h < 180.0 {
        (0.0, c, x)
    } else if h < 240.0 {
        (0.0, x, c)
    } else if h < 300.0 {
        (x, 0.0, c)
    } else {
        (c, 0.0, x)
    };
    ((r1 + m).clamp(0.0, 1.0), (g1 + m).clamp(0.0, 1.0), (b1 + m).clamp(0.0, 1.0))
}

fn display_p3_to_srgb(r: f32, g: f32, b: f32) -> (f32, f32, f32) {
    // Approximate linear transfer for P3 -> sRGB gamut mapping with clamping (`E1-9` §8.4)
    let to_lin = |u: f32| -> f32 {
        if u <= 0.04045 {
            u / 12.92
        } else {
            ((u + 0.055) / 1.055).powf(2.4)
        }
    };
    let from_lin = |u: f32| -> f32 {
        let u = u.clamp(0.0, 1.0);
        if u <= 0.0031308 {
            12.92 * u
        } else {
            1.055 * u.powf(1.0 / 2.4) - 0.055
        }
    };
    let lr = to_lin(r);
    let lg = to_lin(g);
    let lb = to_lin(b);
    let sr = 1.2249401 * lr - 0.2249402 * lg;
    let sg = -0.0420569 * lr + 1.0420571 * lg;
    let sb = -0.0196376 * lr - 0.0786361 * lg + 1.0982735 * lb;
    (from_lin(sr), from_lin(sg), from_lin(sb))
}

fn parse_hex_color(hex: &str, alpha_override: Option<f32>) -> Option<Color> {
    let h = hex.trim().strip_prefix('#')?;
    let (r, g, b, a) = match h.len() {
        6 => (u8::from_str_radix(&h[0..2], 16).ok()?, u8::from_str_radix(&h[2..4], 16).ok()?, u8::from_str_radix(&h[4..6], 16).ok()?, 255u8),
        8 => (
            u8::from_str_radix(&h[0..2], 16).ok()?,
            u8::from_str_radix(&h[2..4], 16).ok()?,
            u8::from_str_radix(&h[4..6], 16).ok()?,
            u8::from_str_radix(&h[6..8], 16).ok()?,
        ),
        _ => return None,
    };
    Some(Color { r: f32::from(r) / 255.0, g: f32::from(g) / 255.0, b: f32::from(b) / 255.0, a: alpha_override.unwrap_or(f32::from(a) / 255.0) })
}

fn parse_token_color(val: &Value, token_path: &str, skipped: &mut Vec<SkippedVariable>) -> Result<Color, String> {
    if let Some(s) = val.as_str() {
        return parse_hex_color(s, None).ok_or_else(|| "unsupported-color-space".to_string());
    }
    let Some(obj) = val.as_object() else {
        return Err("unsupported-color-space".into());
    };
    let alpha = obj.get("alpha").and_then(Value::as_f64).map(|a| (a as f32).clamp(0.0, 1.0)).unwrap_or(1.0);
    let cs = obj.get("colorSpace").and_then(Value::as_str).unwrap_or("srgb").to_ascii_lowercase();
    let comps: Option<Vec<f32>> = obj.get("components").and_then(Value::as_array).map(|arr| arr.iter().filter_map(|x| x.as_f64().map(|n| n as f32)).collect());

    match cs.as_str() {
        "srgb" => {
            if let Some(c) = comps.filter(|c| c.len() == 3) {
                Ok(Color { r: c[0].clamp(0.0, 1.0), g: c[1].clamp(0.0, 1.0), b: c[2].clamp(0.0, 1.0), a: alpha })
            } else if let Some(hex) = obj.get("hex").and_then(Value::as_str) {
                parse_hex_color(hex, Some(alpha)).ok_or_else(|| "unsupported-color-space".to_string())
            } else {
                Err("unsupported-color-space".into())
            }
        }
        "hsl" => {
            let c = comps.filter(|c| c.len() == 3).ok_or_else(|| "unsupported-color-space".to_string())?;
            let (r, g, b) = hsl_to_rgb(c[0], c[1], c[2]);
            Ok(Color { r, g, b, a: alpha })
        }
        "display-p3" => {
            let c = comps.filter(|c| c.len() == 3).ok_or_else(|| "unsupported-color-space".to_string())?;
            let (r, g, b) = display_p3_to_srgb(c[0], c[1], c[2]);
            skipped.push(SkippedVariable { path: token_path.to_string(), reason: "converted display-p3 colour to srgb with clamping".into() });
            Ok(Color { r, g, b, a: alpha })
        }
        _ => Err("unsupported-color-space".into()),
    }
}

fn is_valid_easing(s: &str) -> bool {
    let t = s.trim();
    matches!(t, "linear" | "ease" | "ease-in" | "ease-out" | "ease-in-out")
        || (t.starts_with("cubic-bezier(") && t.ends_with(')'))
        || (t.starts_with("spring(") && t.ends_with(')'))
}

fn parse_code_syntax_obj(v: Option<&Value>) -> CodeSyntax {
    let Some(o) = v.and_then(Value::as_object) else {
        return CodeSyntax::new();
    };
    o.iter().filter_map(|(label, code)| Some((CodeSyntaxPlatform::from_label(label)?, code.as_str().filter(|c| !c.is_empty())?.to_string()))).collect()
}

fn import_figma_native_files(doc: &mut Document, primary_text: &str, opts: &ImportVariablesOptions) -> Result<ImportReport, FigError> {
    let mut report = ImportReport { format: VariablesFormat::FigmaNative, ..Default::default() };

    let mut raw_files: Vec<(Option<String>, Value)> = Vec::new();
    let primary_val = crate::rest_import::parse_json_text(primary_text)?;
    raw_files.push((opts.mode_name.clone(), primary_val));

    for (fname, fbytes) in &opts.additional_files {
        let ftext = std::str::from_utf8(fbytes).map_err(|e| FigError::Format(format!("invalid UTF-8 in {fname}: {e}")))?;
        let fval = crate::rest_import::parse_json_text(ftext)?;
        raw_files.push((Some(mode_name_from_filename(fname)), fval));
    }

    let mut parsed_tokens: Vec<ParsedToken> = Vec::new();
    for (file_idx, (mode_hint, root_val)) in raw_files.iter().enumerate() {
        let Some(root_obj) = root_val.as_object() else {
            continue;
        };
        let root_ext = root_obj.get("$extensions").and_then(Value::as_object);
        let mode_name = root_ext
            .and_then(|e| e.get("com.figma.modeName"))
            .and_then(Value::as_str)
            .map(String::from)
            .or_else(|| mode_hint.clone())
            .unwrap_or_else(|| format!("Mode {}", file_idx + 1));
        let mode_id = root_ext
            .and_then(|e| e.get("com.figma.modeId"))
            .and_then(Value::as_str)
            .map(String::from)
            .unwrap_or_else(|| format!("mode:{}", mode_name.to_ascii_lowercase().replace(' ', "_")));
        let coll_name = root_ext
            .and_then(|e| e.get("com.figma.collectionName"))
            .or_else(|| root_ext.and_then(|e| e.get("com.figma.parentVariableSetName")))
            .and_then(Value::as_str)
            .map(String::from)
            .or_else(|| opts.collection_name.clone())
            .unwrap_or_else(|| "Tokens".to_string());
        let coll_id = root_ext
            .and_then(|e| e.get("com.figma.collectionId"))
            .or_else(|| root_ext.and_then(|e| e.get("com.figma.parentVariableSetId")))
            .and_then(Value::as_str)
            .map(String::from)
            .unwrap_or_else(|| format!("VariableCollectionId:{}", coll_name.to_ascii_lowercase().replace(' ', "_")));
        let coll_key = root_ext.and_then(|e| e.get("com.figma.collectionKey")).and_then(Value::as_str).map(String::from);
        let source_library_key = root_ext.and_then(|e| e.get("com.figma.sourceLibraryKey")).and_then(Value::as_str).map(String::from);
        let is_default_mode = root_ext.and_then(|e| e.get("com.figma.isDefaultMode")).and_then(Value::as_bool).unwrap_or(file_idx == 0);

        let ctx = WalkContext {
            dialect: VariablesFormat::FigmaNative,
            collection_id: coll_id,
            collection_name: coll_name,
            collection_key: coll_key,
            source_library_key,
            mode_id,
            mode_name,
            is_default_mode,
        };
        walk_tokens_group(root_obj, &[], None, &ctx, &mut parsed_tokens, &mut report.skipped);
    }

    let staging = assemble_staged_document(parsed_tokens, &mut report.skipped);
    merge_staged_into_doc(doc, staging, opts.replace, &mut report);
    Ok(report)
}

fn import_dtcg_files(doc: &mut Document, primary_text: &str, opts: &ImportVariablesOptions) -> Result<ImportReport, FigError> {
    let mut report = ImportReport { format: VariablesFormat::Dtcg, ..Default::default() };

    let primary_val = crate::rest_import::parse_json_text(primary_text)?;
    let Some(primary_obj) = primary_val.as_object() else {
        return Err(FigError::Format("DTCG root must be a JSON object".into()));
    };

    let additional_by_name: HashMap<String, Value> = opts
        .additional_files
        .iter()
        .filter_map(|(name, bytes)| {
            let text = std::str::from_utf8(bytes).ok()?;
            let val = crate::rest_import::parse_json_text(text).ok()?;
            let short = name.rsplit(['/', '\\']).next().unwrap_or(name).to_string();
            Some((short, val))
        })
        .collect();

    let mut parsed_tokens: Vec<ParsedToken> = Vec::new();

    if is_dtcg_manifest(primary_obj) {
        if let Some(colls) = primary_obj.get("collections").and_then(Value::as_array) {
            for coll_entry in colls {
                let Some(co) = coll_entry.as_object() else {
                    continue;
                };
                let coll_id = co.get("id").and_then(Value::as_str).unwrap_or("VariableCollectionId:1:0").to_string();
                let coll_name = co.get("name").and_then(Value::as_str).unwrap_or("Tokens").to_string();
                let group_key = co.get("group").and_then(Value::as_str).unwrap_or(&coll_name).to_string();
                let default_mode = co.get("default_mode").and_then(Value::as_str).unwrap_or("").to_string();
                let coll_key = co.get("key").and_then(Value::as_str).map(String::from);
                let source_library_key = co.get("source_library_key").and_then(Value::as_str).map(String::from);

                if let Some(modes) = co.get("modes").and_then(Value::as_array) {
                    for (m_idx, mode_entry) in modes.iter().enumerate() {
                        let Some(mo) = mode_entry.as_object() else {
                            continue;
                        };
                        let mode_id = mo.get("id").and_then(Value::as_str).unwrap_or("1:0").to_string();
                        let mode_name = mo.get("name").and_then(Value::as_str).unwrap_or("Mode 1").to_string();
                        let file_name = mo.get("file").and_then(Value::as_str).unwrap_or("");
                        let short_file = file_name.rsplit(['/', '\\']).next().unwrap_or(file_name);
                        let Some(file_val) = additional_by_name.get(short_file) else {
                            continue;
                        };
                        let Some(file_obj) = file_val.as_object() else {
                            continue;
                        };
                        let group_obj = file_obj
                            .get(&group_key)
                            .and_then(Value::as_object)
                            .or_else(|| file_obj.get(&coll_name).and_then(Value::as_object))
                            .unwrap_or(file_obj);

                        let ctx = WalkContext {
                            dialect: VariablesFormat::Dtcg,
                            collection_id: coll_id.clone(),
                            collection_name: coll_name.clone(),
                            collection_key: coll_key.clone(),
                            source_library_key: source_library_key.clone(),
                            mode_id: mode_id.clone(),
                            mode_name,
                            is_default_mode: mode_id == default_mode || m_idx == 0,
                        };
                        walk_tokens_group(group_obj, &[], None, &ctx, &mut parsed_tokens, &mut report.skipped);
                    }
                }
            }
        }
    } else {
        // Single DTCG token file (or multiple files without a manifest)
        let mut all_files: Vec<(String, &Map<String, Value>)> = Vec::new();
        let first_mode = opts.mode_name.clone().unwrap_or_else(|| "Mode 1".to_string());
        all_files.push((first_mode, primary_obj));
        for (fname, fval) in &additional_by_name {
            if let Some(fobj) = fval.as_object() {
                all_files.push((mode_name_from_filename(fname), fobj));
            }
        }

        for (file_idx, (mode_name, root_obj)) in all_files.into_iter().enumerate() {
            let mode_id = format!("mode:{}", mode_name.to_ascii_lowercase().replace(' ', "_"));
            // In DTCG §3.2, top-level keys (that are not `$` and not leaf tokens) represent collections
            // if they wrap nested token groups; otherwise fall back to a single collection.
            let top_groups: Vec<(&String, &Map<String, Value>)> = root_obj
                .iter()
                .filter(|(k, _)| !k.starts_with('$'))
                .filter_map(|(k, v)| {
                    let o = v.as_object()?;
                    (!o.contains_key("$value")).then_some((k, o))
                })
                .collect();

            if !top_groups.is_empty() && opts.collection_name.is_none() {
                for (coll_name, group_obj) in top_groups {
                    let coll_id = format!("VariableCollectionId:{}", coll_name.to_ascii_lowercase().replace(' ', "_"));
                    let ctx = WalkContext {
                        dialect: VariablesFormat::Dtcg,
                        collection_id: coll_id,
                        collection_name: coll_name.clone(),
                        collection_key: None,
                        source_library_key: None,
                        mode_id: mode_id.clone(),
                        mode_name: mode_name.clone(),
                        is_default_mode: file_idx == 0,
                    };
                    walk_tokens_group(group_obj, &[], None, &ctx, &mut parsed_tokens, &mut report.skipped);
                }
            } else {
                let coll_name = opts.collection_name.clone().unwrap_or_else(|| "Tokens".to_string());
                let coll_id = format!("VariableCollectionId:{}", coll_name.to_ascii_lowercase().replace(' ', "_"));
                let ctx = WalkContext {
                    dialect: VariablesFormat::Dtcg,
                    collection_id: coll_id,
                    collection_name: coll_name,
                    collection_key: None,
                    source_library_key: None,
                    mode_id,
                    mode_name,
                    is_default_mode: file_idx == 0,
                };
                walk_tokens_group(root_obj, &[], None, &ctx, &mut parsed_tokens, &mut report.skipped);
            }
        }
    }

    let staging = assemble_staged_document(parsed_tokens, &mut report.skipped);
    merge_staged_into_doc(doc, staging, opts.replace, &mut report);
    Ok(report)
}

#[derive(Clone, Debug)]
struct WalkContext {
    dialect: VariablesFormat,
    collection_id: String,
    collection_name: String,
    collection_key: Option<String>,
    source_library_key: Option<String>,
    mode_id: String,
    mode_name: String,
    is_default_mode: bool,
}

fn walk_tokens_group(
    obj: &Map<String, Value>,
    path: &[String],
    inherited_type: Option<&str>,
    ctx: &WalkContext,
    out: &mut Vec<ParsedToken>,
    skipped: &mut Vec<SkippedVariable>,
) {
    let local_type = obj.get("$type").and_then(Value::as_str).or(inherited_type);

    if obj.contains_key("$value") {
        parse_single_token(obj, path, local_type, ctx, out, skipped);
        return;
    }

    if let Some(root_tok) = obj.get("$root").and_then(Value::as_object) {
        if root_tok.contains_key("$value") {
            let tok_type = root_tok.get("$type").and_then(Value::as_str).or(local_type);
            parse_single_token(root_tok, path, tok_type, ctx, out, skipped);
        }
    }

    for (k, child) in obj {
        if k.starts_with('$') {
            continue;
        }
        if let Some(child_obj) = child.as_object() {
            let mut next_path = path.to_vec();
            next_path.push(k.clone());
            walk_tokens_group(child_obj, &next_path, local_type, ctx, out, skipped);
        }
    }
}

fn parse_single_token(
    obj: &Map<String, Value>,
    path: &[String],
    effective_type: Option<&str>,
    ctx: &WalkContext,
    out: &mut Vec<ParsedToken>,
    skipped: &mut Vec<SkippedVariable>,
) {
    let raw_name = path.join("/");
    let token_path = if ctx.collection_name.is_empty() { raw_name.clone() } else { format!("{}/{}", ctx.collection_name, raw_name) };
    let val = &obj["$value"];
    let ext = obj.get("$extensions").and_then(Value::as_object);
    let dos_ext = ext.and_then(|e| e.get("org.designos")).and_then(Value::as_object);

    let var_name = dos_ext.and_then(|d| d.get("original_name")).and_then(Value::as_str).map(String::from).unwrap_or(raw_name);

    let var_id = ext
        .and_then(|e| e.get("com.figma.variableId"))
        .and_then(Value::as_str)
        .or_else(|| dos_ext.and_then(|d| d.get("id")).and_then(Value::as_str))
        .map(String::from)
        .unwrap_or_else(|| {
            format!("VariableID:{}:{}", ctx.collection_name.to_ascii_lowercase().replace(' ', "_"), var_name.to_ascii_lowercase().replace('/', "_"))
        });

    let collection_id = dos_ext.and_then(|d| d.get("collection_id")).and_then(Value::as_str).map(String::from).unwrap_or_else(|| ctx.collection_id.clone());

    let description = obj.get("$description").and_then(Value::as_str).unwrap_or("").to_string();

    let mut scopes: Vec<String> = ext
        .and_then(|e| e.get("com.figma.scopes"))
        .or_else(|| dos_ext.and_then(|d| d.get("scopes")))
        .and_then(Value::as_array)
        .map(|arr| arr.iter().filter_map(|s| s.as_str()).filter(|s| *s != "ALL_SCOPES").map(normalize_scope_on_import).collect())
        .unwrap_or_default();

    let hidden_from_publishing = ext
        .and_then(|e| e.get("com.figma.hiddenFromPublishing"))
        .or_else(|| dos_ext.and_then(|d| d.get("hidden_from_publishing")))
        .and_then(Value::as_bool)
        .unwrap_or(false);

    let key = ext.and_then(|e| e.get("com.figma.key")).or_else(|| dos_ext.and_then(|d| d.get("key"))).and_then(Value::as_str).map(String::from);

    let code_syntax = parse_code_syntax_obj(ext.and_then(|e| e.get("com.figma.codeSyntax")).or_else(|| dos_ext.and_then(|d| d.get("code_syntax"))));

    let figma_type_ext = ext.and_then(|e| e.get("com.figma.type")).and_then(Value::as_str);
    let dos_kind_ext = dos_ext.and_then(|d| d.get("kind")).and_then(Value::as_str);

    // Check for cross-collection `com.figma.aliasData` first (`E1-9` §14.6)
    let figma_alias_data = ext.and_then(|e| e.get("com.figma.aliasData")).and_then(Value::as_object);

    // Check for curly-brace alias `$value: "{a.b.c}"`
    if let Some(s) = val.as_str() {
        if is_valid_dtcg_alias_str(s) {
            let inner = &s[1..s.len() - 1];
            let kind = infer_token_kind(effective_type, figma_type_ext, dos_kind_ext, val, &scopes);
            out.push(ParsedToken {
                collection_id,
                collection_name: ctx.collection_name.clone(),
                collection_key: ctx.collection_key.clone(),
                source_library_key: ctx.source_library_key.clone(),
                mode_id: ctx.mode_id.clone(),
                mode_name: ctx.mode_name.clone(),
                is_default_mode: ctx.is_default_mode,
                var_id,
                var_name,
                kind,
                raw_value: RawTokenValue::AliasPath { target_path: inner.to_string(), same_collection_only: ctx.dialect == VariablesFormat::FigmaNative },
                description,
                scopes,
                hidden_from_publishing,
                key,
                code_syntax,
            });
            return;
        }
    }

    // Parse literal value according to `$type` / extension / inference (`E1-9` §14.7 & §3.2)
    let parsed_literal: Result<(VariableKind, VariableValue), String> = match effective_type {
        Some("color") => parse_token_color(val, &token_path, skipped).map(|c| (VariableKind::Color, VariableValue::Color(c))),
        Some("number") => {
            if figma_type_ext == Some("boolean") || dos_kind_ext == Some("boolean") {
                if let Some(b) = val.as_bool().or_else(|| val.as_i64().map(|n| n != 0)).or_else(|| val.as_f64().map(|n| n != 0.0)) {
                    Ok((VariableKind::Boolean, VariableValue::Boolean(b)))
                } else {
                    Err("invalid boolean number value".into())
                }
            } else if let Some(n) = val.as_f64() {
                Ok((VariableKind::Number, VariableValue::Number(n as f32)))
            } else {
                Err("invalid number value".into())
            }
        }
        Some("dimension") => {
            if let Some(o) = val.as_object() {
                let unit = o.get("unit").and_then(Value::as_str).unwrap_or("");
                let num = o.get("value").and_then(Value::as_f64);
                match (num, unit) {
                    (Some(n), "px") => Ok((VariableKind::Number, VariableValue::Number(n as f32))),
                    _ => Err("unsupported-dimension-unit".into()),
                }
            } else {
                Err("unsupported-dimension-unit".into())
            }
        }
        Some("duration") => {
            if let Some(o) = val.as_object() {
                let unit = o.get("unit").and_then(Value::as_str).unwrap_or("");
                let num = o.get("value").and_then(Value::as_f64);
                match (num, unit) {
                    (Some(n), "ms") => Ok((VariableKind::Number, VariableValue::Number(n as f32))),
                    (Some(n), "s") => Ok((VariableKind::Number, VariableValue::Number((n * 1000.0) as f32))),
                    _ => Err("unsupported-duration-unit".into()),
                }
            } else {
                Err("unsupported-duration-unit".into())
            }
        }
        Some("fontFamily") => {
            if let Some(s) = val.as_str() {
                if !scopes.iter().any(|sc| sc == "FONT_FAMILY") {
                    scopes.push("FONT_FAMILY".to_string());
                }
                Ok((VariableKind::String, VariableValue::String(s.to_string())))
            } else {
                Err("unsupported-font-family-value-type".into())
            }
        }
        Some("easing") => {
            if let Some(s) = val.as_str().filter(|s| is_valid_easing(s)) {
                Ok((VariableKind::String, VariableValue::String(s.to_string())))
            } else {
                Err("unsupported-easing-token-value".into())
            }
        }
        Some("string") => {
            if let Some(s) = val.as_str() {
                Ok((VariableKind::String, VariableValue::String(s.to_string())))
            } else {
                Err("invalid string value".into())
            }
        }
        Some("boolean") => {
            if let Some(b) = val.as_bool() {
                Ok((VariableKind::Boolean, VariableValue::Boolean(b)))
            } else {
                Err("invalid boolean value".into())
            }
        }
        None => {
            if dos_kind_ext == Some("boolean") || figma_type_ext == Some("boolean") {
                let b = dos_ext
                    .and_then(|d| d.get("boolean"))
                    .and_then(Value::as_bool)
                    .or_else(|| val.as_bool())
                    .or_else(|| val.as_i64().map(|n| n != 0))
                    .unwrap_or(false);
                Ok((VariableKind::Boolean, VariableValue::Boolean(b)))
            } else if let Some(b) = val.as_bool() {
                Ok((VariableKind::Boolean, VariableValue::Boolean(b)))
            } else if let Some(n) = val.as_f64() {
                Ok((VariableKind::Number, VariableValue::Number(n as f32)))
            } else if let Some(s) = val.as_str() {
                Ok((VariableKind::String, VariableValue::String(s.to_string())))
            } else if val.is_object() {
                parse_token_color(val, &token_path, skipped).map(|c| (VariableKind::Color, VariableValue::Color(c)))
            } else {
                Err("unsupported-token-value".into())
            }
        }
        Some(other) => Err(format!("unsupported-token-type:{other}")),
    };

    let (kind, literal_val) = match parsed_literal {
        Ok(pair) => pair,
        Err(reason) => {
            skipped.push(SkippedVariable { path: token_path, reason });
            return;
        }
    };

    let raw_value = if let Some(ad) = figma_alias_data {
        let target_id = ad.get("targetVariableId").and_then(Value::as_str).unwrap_or("").to_string();
        let target_name = ad.get("targetVariableName").and_then(Value::as_str).unwrap_or("").to_string();
        let target_set_id = ad.get("targetVariableSetId").and_then(Value::as_str).map(String::from);
        let target_set_name = ad.get("targetVariableSetName").and_then(Value::as_str).map(String::from);
        RawTokenValue::FigmaCrossAlias { target_id, target_name, target_set_id, target_set_name, fallback: literal_val }
    } else {
        RawTokenValue::Literal(literal_val)
    };

    out.push(ParsedToken {
        collection_id,
        collection_name: ctx.collection_name.clone(),
        collection_key: ctx.collection_key.clone(),
        source_library_key: ctx.source_library_key.clone(),
        mode_id: ctx.mode_id.clone(),
        mode_name: ctx.mode_name.clone(),
        is_default_mode: ctx.is_default_mode,
        var_id,
        var_name,
        kind,
        raw_value,
        description,
        scopes,
        hidden_from_publishing,
        key,
        code_syntax,
    });
}

fn infer_token_kind(effective_type: Option<&str>, figma_type_ext: Option<&str>, dos_kind_ext: Option<&str>, val: &Value, scopes: &[String]) -> VariableKind {
    if figma_type_ext == Some("boolean") || dos_kind_ext == Some("boolean") {
        return VariableKind::Boolean;
    }
    if figma_type_ext == Some("string") || dos_kind_ext == Some("string") {
        return VariableKind::String;
    }
    match effective_type {
        Some("color") => VariableKind::Color,
        Some("number" | "dimension" | "duration") => VariableKind::Number,
        Some("string" | "fontFamily" | "easing") => VariableKind::String,
        Some("boolean") => VariableKind::Boolean,
        _ => {
            if scopes.iter().any(|s| s == "FONT_FAMILY" || s == "TEXT_CONTENT") {
                VariableKind::String
            } else if val.is_boolean() {
                VariableKind::Boolean
            } else if val.is_number() {
                VariableKind::Number
            } else {
                VariableKind::Color
            }
        }
    }
}

fn assemble_staged_document(tokens: Vec<ParsedToken>, skipped: &mut Vec<SkippedVariable>) -> Document {
    let mut doc = Document::new("staged");
    let mut pending_aliases: Vec<(String, String, String, RawTokenValue)> = Vec::new();

    for tok in tokens {
        let coll = doc.variable_collections.entry(tok.collection_id.clone()).or_insert_with(|| VariableCollection {
            id: tok.collection_id.clone(),
            name: tok.collection_name.clone(),
            modes: Vec::new(),
            default_mode: tok.mode_id.clone(),
            variables: Vec::new(),
            key: tok.collection_key.clone(),
            source_library_key: tok.source_library_key.clone(),
        });

        if !coll.modes.iter().any(|m| m.id == tok.mode_id) {
            coll.modes.push(VariableMode { id: tok.mode_id.clone(), name: tok.mode_name.clone() });
        }
        if tok.is_default_mode {
            coll.default_mode = tok.mode_id.clone();
        }
        if !coll.variables.contains(&tok.var_id) {
            coll.variables.push(tok.var_id.clone());
        }

        let var = doc.variables.entry(tok.var_id.clone()).or_insert_with(|| Variable {
            id: tok.var_id.clone(),
            name: tok.var_name.clone(),
            collection: tok.collection_id.clone(),
            kind: tok.kind,
            values: BTreeMap::new(),
            description: tok.description.clone(),
            scopes: tok.scopes.clone(),
            hidden_from_publishing: tok.hidden_from_publishing,
            key: tok.key.clone(),
            code_syntax: tok.code_syntax.clone(),
        });

        if var.description.is_empty() && !tok.description.is_empty() {
            var.description = tok.description;
        }
        if var.scopes.is_empty() && !tok.scopes.is_empty() {
            var.scopes = tok.scopes;
        }
        if !tok.code_syntax.is_empty() {
            for (p, c) in tok.code_syntax {
                var.code_syntax.insert(p, c);
            }
        }
        if var.key.is_none() && tok.key.is_some() {
            var.key = tok.key;
        }
        var.hidden_from_publishing |= tok.hidden_from_publishing;

        match tok.raw_value {
            RawTokenValue::Literal(lit) => {
                if let Some(existing_val) = var.values.get(&tok.mode_id) {
                    if *existing_val != lit {
                        // Conflict in the same mode: first wins, report conflict (`E1-9` §8.5)
                        skipped.push(SkippedVariable {
                            path: format!("{}/{} ({})", tok.collection_name, tok.var_name, tok.mode_name),
                            reason: "conflicting value in same mode (first value kept)".into(),
                        });
                    }
                } else {
                    var.values.insert(tok.mode_id, lit);
                }
            }
            alias_raw => {
                pending_aliases.push((tok.var_id, tok.collection_id, tok.mode_id, alias_raw));
            }
        }
    }

    // Build lookup indexes for resolving aliases
    let mut by_coll_and_name: HashMap<(String, String), String> = HashMap::new();
    let mut by_coll_and_sanitized: HashMap<(String, String), String> = HashMap::new();
    let mut by_key_or_id: HashMap<String, String> = HashMap::new();
    let mut coll_by_group_name: HashMap<String, String> = HashMap::new();

    for c in doc.variable_collections.values() {
        coll_by_group_name.insert(c.name.clone(), c.id.clone());
        let (clean, _) = sanitize_dtcg_segment(&c.name);
        coll_by_group_name.insert(clean, c.id.clone());
    }
    for v in doc.variables.values() {
        by_coll_and_name.insert((v.collection.clone(), v.name.clone()), v.id.clone());
        let (segs, _) = sanitize_dtcg_path(&v.name);
        by_coll_and_sanitized.insert((v.collection.clone(), segs.join("/")), v.id.clone());
        by_key_or_id.insert(v.id.clone(), v.id.clone());
        if let Some(k) = &v.key {
            by_key_or_id.insert(k.clone(), v.id.clone());
        }
    }

    for (var_id, coll_id, mode_id, raw) in pending_aliases {
        match raw {
            RawTokenValue::AliasPath { target_path, same_collection_only } => {
                let clean_target = target_path.strip_suffix(".$root").unwrap_or(&target_path);
                let resolved_target = if same_collection_only {
                    let slash_path = clean_target.replace('.', "/");
                    by_coll_and_name.get(&(coll_id.clone(), slash_path.clone())).or_else(|| by_coll_and_sanitized.get(&(coll_id.clone(), slash_path))).cloned()
                } else {
                    // DTCG reference `{Collection.Group.Name}` or same-collection `{Group.Name}`
                    let resolved_cross = clean_target.split_once('.').and_then(|(first_seg, rest)| {
                        let target_cid = coll_by_group_name.get(first_seg)?;
                        let slash_rest = rest.replace('.', "/");
                        by_coll_and_name
                            .get(&(target_cid.clone(), slash_rest.clone()))
                            .or_else(|| by_coll_and_sanitized.get(&(target_cid.clone(), slash_rest)))
                            .cloned()
                    });
                    resolved_cross.or_else(|| {
                        let slash_path = clean_target.replace('.', "/");
                        by_coll_and_name
                            .get(&(coll_id.clone(), slash_path.clone()))
                            .or_else(|| by_coll_and_sanitized.get(&(coll_id.clone(), slash_path)))
                            .cloned()
                    })
                };

                if let Some(tid) = resolved_target {
                    // Update variable kind to match target variable kind if needed
                    let target_kind = doc.variables.get(&tid).map(|tv| tv.kind);
                    if let Some(v) = doc.variables.get_mut(&var_id) {
                        if let Some(tk) = target_kind {
                            v.kind = tk;
                        }
                        v.values.entry(mode_id).or_insert(VariableValue::Alias(tid));
                    }
                } else {
                    skipped.push(SkippedVariable { path: var_id, reason: format!("unresolved alias '{{{target_path}}}'") });
                }
            }
            RawTokenValue::FigmaCrossAlias { target_id, target_name, target_set_id, target_set_name, fallback } => {
                let resolved = by_key_or_id
                    .get(&target_id)
                    .cloned()
                    .or_else(|| {
                        let tcid = target_set_id.as_ref().cloned().or_else(|| target_set_name.as_ref().and_then(|n| coll_by_group_name.get(n).cloned()))?;
                        by_coll_and_name.get(&(tcid, target_name.clone())).cloned()
                    })
                    .unwrap_or(target_id);
                let has_resolved = doc.variables.contains_key(&resolved) || !resolved.is_empty();
                if let Some(v) = doc.variables.get_mut(&var_id) {
                    if has_resolved {
                        v.values.entry(mode_id).or_insert(VariableValue::Alias(resolved));
                    } else {
                        v.values.entry(mode_id).or_insert(fallback);
                    }
                }
            }
            RawTokenValue::Literal(_) => {}
        }
    }

    // Detect alias cycles (`E1-9` §8.3)
    detect_and_break_alias_cycles(&mut doc, skipped);

    doc
}

fn detect_and_break_alias_cycles(doc: &mut Document, skipped: &mut Vec<SkippedVariable>) {
    let var_ids: Vec<String> = doc.variables.keys().cloned().collect();
    for vid in var_ids {
        let Some(modes) = doc.variables.get(&vid).map(|v| v.values.keys().cloned().collect::<Vec<_>>()) else {
            continue;
        };
        for mode_id in modes {
            let mut visited = BTreeSet::new();
            visited.insert(vid.clone());
            let mut cur = vid.clone();
            let mut is_cycle = false;
            for _ in 0..32 {
                let Some(next) = doc.variables.get(&cur).and_then(|v| match v.values.get(&mode_id) {
                    Some(VariableValue::Alias(t)) => Some(t.clone()),
                    _ => None,
                }) else {
                    break;
                };
                if !visited.insert(next.clone()) {
                    is_cycle = true;
                    break;
                }
                cur = next;
            }
            if is_cycle {
                if let Some(v) = doc.variables.get_mut(&vid) {
                    let fallback = v.values.values().find(|val| !matches!(val, VariableValue::Alias(_))).cloned().unwrap_or(match v.kind {
                        VariableKind::Color => VariableValue::Color(Color::rgb(0, 0, 0)),
                        VariableKind::Number => VariableValue::Number(0.0),
                        VariableKind::String => VariableValue::String(String::new()),
                        VariableKind::Boolean => VariableValue::Boolean(false),
                    });
                    v.values.insert(mode_id.clone(), fallback);
                    skipped.push(SkippedVariable { path: v.name.clone(), reason: format!("alias-cycle detected in mode '{mode_id}'") });
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_figma_native_light_json() -> &'static str {
        r##"{
  "Color": {
    "Brand": {
      "$root": {
        "$type": "color",
        "$value": {
          "colorSpace": "srgb",
          "components": [0.050980393, 0.6, 1.0],
          "alpha": 1.0,
          "hex": "#0D99FF"
        },
        "$description": "Primary brand blue",
        "$extensions": {
          "com.figma.variableId": "VariableID:10:1",
          "com.figma.scopes": ["ALL_FILLS", "STROKE"],
          "com.figma.codeSyntax": {
            "WEB": "var(--color-brand)",
            "iOS": "Color.brand"
          }
        }
      },
      "Accent": {
        "$type": "color",
        "$value": "{Color.Brand.$root}",
        "$extensions": {
          "com.figma.variableId": "VariableID:10:2",
          "com.figma.scopes": ["FRAME_FILL"]
        }
      }
    }
  },
  "Space": {
    "1.5x": {
      "$type": "number",
      "$value": 12.0,
      "$extensions": {
        "com.figma.variableId": "VariableID:10:3",
        "com.figma.scopes": ["GAP", "WIDTH_HEIGHT"]
      }
    }
  },
  "Feature": {
    "ShowBadge": {
      "$type": "number",
      "$value": 1,
      "$extensions": {
        "com.figma.variableId": "VariableID:10:4",
        "com.figma.type": "boolean",
        "com.figma.hiddenFromPublishing": true
      }
    },
    "Label": {
      "$type": "string",
      "$value": "Pro Plan",
      "$extensions": {
        "com.figma.variableId": "VariableID:10:5",
        "com.figma.type": "string",
        "com.figma.scopes": ["TEXT_CONTENT"]
      }
    }
  },
  "$extensions": {
    "com.figma.modeName": "Light",
    "com.figma.modeId": "10:0",
    "com.figma.collectionId": "VariableCollectionId:10:0",
    "com.figma.collectionName": "Theme"
  }
}"##
    }

    fn sample_figma_native_dark_json() -> &'static str {
        r##"{
  "Color": {
    "Brand": {
      "$root": {
        "$type": "color",
        "$value": {
          "colorSpace": "srgb",
          "components": [0.2, 0.7, 1.0],
          "alpha": 1.0,
          "hex": "#33B3FF"
        },
        "$extensions": {
          "com.figma.variableId": "VariableID:10:1"
        }
      },
      "Accent": {
        "$type": "color",
        "$value": "{Color.Brand.$root}",
        "$extensions": {
          "com.figma.variableId": "VariableID:10:2"
        }
      }
    }
  },
  "Space": {
    "1.5x": {
      "$type": "number",
      "$value": 16.0,
      "$extensions": {
        "com.figma.variableId": "VariableID:10:3"
      }
    }
  },
  "Feature": {
    "ShowBadge": {
      "$type": "number",
      "$value": 0,
      "$extensions": {
        "com.figma.variableId": "VariableID:10:4",
        "com.figma.type": "boolean"
      }
    },
    "Label": {
      "$type": "string",
      "$value": "Pro Plan Dark",
      "$extensions": {
        "com.figma.variableId": "VariableID:10:5",
        "com.figma.type": "string"
      }
    }
  },
  "$extensions": {
    "com.figma.modeName": "Dark",
    "com.figma.modeId": "10:1",
    "com.figma.collectionId": "VariableCollectionId:10:0",
    "com.figma.collectionName": "Theme"
  }
}"##
    }

    #[test]
    fn figma_native_import_lossless() {
        let mut doc = Document::new("d");
        let opts = ImportVariablesOptions {
            additional_files: vec![("Dark.tokens.json".into(), sample_figma_native_dark_json().as_bytes().to_vec())],
            ..Default::default()
        };
        let report = import_variables(&mut doc, sample_figma_native_light_json(), VariablesFormat::Auto, opts).unwrap();
        assert_eq!(report.format, VariablesFormat::FigmaNative);
        assert_eq!(report.added, 5);
        assert!(report.skipped.is_empty(), "skipped: {:?}", report.skipped);

        let coll = &doc.variable_collections["VariableCollectionId:10:0"];
        assert_eq!(coll.name, "Theme");
        assert_eq!(coll.modes.len(), 2);
        assert_eq!(coll.modes[0].name, "Light");
        assert_eq!(coll.modes[1].name, "Dark");

        let brand = &doc.variables["VariableID:10:1"];
        assert_eq!(brand.name, "Color/Brand");
        assert_eq!(brand.kind, VariableKind::Color);
        assert_eq!(brand.description, "Primary brand blue");
        assert_eq!(brand.scopes, vec!["ALL_FILLS".to_string(), "STROKE_COLOR".to_string()]);
        assert_eq!(brand.code_syntax_for(CodeSyntaxPlatform::Web), Some("var(--color-brand)"));
        assert_eq!(brand.code_syntax_for(CodeSyntaxPlatform::Ios), Some("Color.brand"));

        let accent = &doc.variables["VariableID:10:2"];
        assert_eq!(accent.name, "Color/Brand/Accent");
        assert_eq!(accent.values.get("10:0"), Some(&VariableValue::Alias("VariableID:10:1".into())));

        let badge = &doc.variables["VariableID:10:4"];
        assert_eq!(badge.kind, VariableKind::Boolean);
        assert_eq!(badge.values.get("10:0"), Some(&VariableValue::Boolean(true)));
        assert_eq!(badge.values.get("10:1"), Some(&VariableValue::Boolean(false)));
        assert!(badge.hidden_from_publishing);

        let label = &doc.variables["VariableID:10:5"];
        assert_eq!(label.kind, VariableKind::String);
        assert_eq!(label.values.get("10:0"), Some(&VariableValue::String("Pro Plan".into())));
        assert_eq!(label.values.get("10:1"), Some(&VariableValue::String("Pro Plan Dark".into())));
    }

    #[test]
    fn dtcg_export_validates() {
        let mut doc = Document::new("d");
        import_variables(&mut doc, sample_figma_native_light_json(), VariablesFormat::FigmaNative, ImportVariablesOptions::default()).unwrap();

        let bundle = export_variables(&doc, VariablesFormat::Dtcg).unwrap();
        assert_eq!(bundle.format, VariablesFormat::Dtcg);
        assert!(bundle.files.len() >= 2);
        for (fname, bytes) in &bundle.files {
            let text = std::str::from_utf8(bytes).unwrap();
            let errs = validate_dtcg_json(text).unwrap();
            assert!(errs.is_empty(), "DTCG validation failed for {fname}: {errs:?}\n{text}");
        }
    }

    #[test]
    fn dtcg_round_trip_lossless() {
        let mut doc = Document::new("d");
        let opts = ImportVariablesOptions {
            additional_files: vec![("Dark.tokens.json".into(), sample_figma_native_dark_json().as_bytes().to_vec())],
            ..Default::default()
        };
        import_variables(&mut doc, sample_figma_native_light_json(), VariablesFormat::FigmaNative, opts).unwrap();

        let bundle = export_variables(&doc, VariablesFormat::Dtcg).unwrap();
        let zip_bytes = bundle.to_zip_bytes().unwrap();

        let mut reimported = Document::new("reimported");
        let rep = import_variables_bytes(&mut reimported, &zip_bytes, Some("tokens.zip"), VariablesFormat::Auto, ImportVariablesOptions::default()).unwrap();
        assert_eq!(rep.format, VariablesFormat::Dtcg);
        assert!(rep.skipped.is_empty(), "skipped: {:?}", rep.skipped);
        assert_eq!(reimported.variable_collections, doc.variable_collections);
        assert_eq!(reimported.variables, doc.variables);
    }

    #[test]
    fn dtcg_export_has_no_figma_keys() {
        let mut doc = Document::new("d");
        import_variables(&mut doc, sample_figma_native_light_json(), VariablesFormat::FigmaNative, ImportVariablesOptions::default()).unwrap();

        let bundle = export_variables(&doc, VariablesFormat::Dtcg).unwrap();
        for (fname, bytes) in &bundle.files {
            let text = std::str::from_utf8(bytes).unwrap();
            assert!(!text.contains("com.figma."), "DTCG file {fname} must not contain com.figma.* keys:\n{text}");
            if fname.ends_with(".tokens.json") {
                let v: Value = serde_json::from_str(text).unwrap();
                let show_badge = &v["Theme"]["Feature"]["ShowBadge"];
                assert!(show_badge.get("$type").is_none(), "DTCG boolean must omit $type");
                assert_eq!(show_badge["$extensions"]["org.designos"]["kind"], Value::String("boolean".into()));
            }
        }
    }

    #[test]
    fn figma_native_export_matches_figma_dialect() {
        let mut doc = Document::new("d");
        import_variables(&mut doc, sample_figma_native_light_json(), VariablesFormat::FigmaNative, ImportVariablesOptions::default()).unwrap();

        let bundle = export_variables(&doc, VariablesFormat::FigmaNative).unwrap();
        assert_eq!(bundle.files.len(), 1);
        assert_eq!(bundle.files[0].0, "Light.tokens.json");
        let v: Value = serde_json::from_slice(&bundle.files[0].1).unwrap();
        let show_badge = &v["Feature"]["ShowBadge"];
        assert_eq!(show_badge["$type"], "number");
        assert_eq!(show_badge["$value"], 1);
        assert_eq!(show_badge["$extensions"]["com.figma.type"], "boolean");

        let label = &v["Feature"]["Label"];
        assert_eq!(label["$type"], "string");
        assert_eq!(label["$value"], "Pro Plan");
        assert_eq!(label["$extensions"]["com.figma.type"], "string");
    }

    #[test]
    fn rest_round_trip_unchanged() {
        let mut doc = Document::new("d");
        import_variables(&mut doc, sample_figma_native_light_json(), VariablesFormat::FigmaNative, ImportVariablesOptions::default()).unwrap();

        let rest_bundle = export_variables(&doc, VariablesFormat::FigmaRest).unwrap();
        let rest_text = std::str::from_utf8(&rest_bundle.files[0].1).unwrap();
        assert_eq!(detect_variables_format(rest_text).unwrap(), VariablesFormat::FigmaRest);

        let mut other = Document::new("other");
        let rep = import_variables(&mut other, rest_text, VariablesFormat::Auto, ImportVariablesOptions { replace: true, ..Default::default() }).unwrap();
        assert_eq!(rep.format, VariablesFormat::FigmaRest);
        assert_eq!(other.variables, doc.variables);
        assert_eq!(other.variable_collections, doc.variable_collections);
    }

    #[test]
    fn merge_not_replace() {
        let mut doc = Document::new("d");
        doc.variable_collections.insert(
            "c_extra".into(),
            VariableCollection {
                id: "c_extra".into(),
                name: "Existing".into(),
                modes: vec![VariableMode { id: "m_extra".into(), name: "Default".into() }],
                default_mode: "m_extra".into(),
                variables: vec!["v_extra".into()],
                key: None,
                source_library_key: None,
            },
        );
        doc.variables.insert(
            "v_extra".into(),
            Variable {
                id: "v_extra".into(),
                name: "KeepMe".into(),
                collection: "c_extra".into(),
                kind: VariableKind::Number,
                values: BTreeMap::from([("m_extra".into(), VariableValue::Number(42.0))]),
                description: String::new(),
                scopes: Vec::new(),
                hidden_from_publishing: false,
                key: None,
                code_syntax: CodeSyntax::new(),
            },
        );

        let rep = import_variables(&mut doc, sample_figma_native_light_json(), VariablesFormat::Auto, ImportVariablesOptions::default()).unwrap();
        assert_eq!(rep.added, 5);
        assert!(doc.variables.contains_key("v_extra"), "existing variable must be preserved on merge");
        assert_eq!(doc.variables.len(), 6);
    }

    #[test]
    fn alias_cycle_and_display_p3_reported() {
        let dtcg_with_cycle_and_p3 = r#"{
  "Theme": {
    "LoopA": {
      "$type": "color",
      "$value": "{Theme.LoopB}"
    },
    "LoopB": {
      "$type": "color",
      "$value": "{Theme.LoopA}"
    },
    "Vivid": {
      "$type": "color",
      "$value": {
        "colorSpace": "display-p3",
        "components": [1.0, 0.2, 0.1],
        "alpha": 1.0
      }
    }
  }
}"#;
        let mut doc = Document::new("d");
        let rep = import_variables(&mut doc, dtcg_with_cycle_and_p3, VariablesFormat::Auto, ImportVariablesOptions::default()).unwrap();
        assert!(rep.skipped.iter().any(|s| s.reason.contains("display-p3")), "expected display-p3 notice in skipped: {:?}", rep.skipped);
        assert!(rep.skipped.iter().any(|s| s.reason.contains("alias-cycle")), "expected alias-cycle notice in skipped: {:?}", rep.skipped);
    }

    #[test]
    fn perf_5000_vars_4_modes_within_budget() {
        let mut doc = Document::new("perf");
        let modes: Vec<VariableMode> = (0..4).map(|i| VariableMode { id: format!("m{i}"), name: format!("Mode{i}") }).collect();
        let mut var_ids = Vec::with_capacity(5000);
        for i in 0..5000 {
            let vid = format!("v_{i}");
            var_ids.push(vid.clone());
            let mut values = BTreeMap::new();
            for m in 0..4 {
                values.insert(format!("m{m}"), VariableValue::Number((i + m) as f32));
            }
            doc.variables.insert(
                vid.clone(),
                Variable {
                    id: vid,
                    name: format!("Group{}/Var{i}", i / 100),
                    collection: "c_perf".into(),
                    kind: VariableKind::Number,
                    values,
                    description: String::new(),
                    scopes: vec!["GAP".into()],
                    hidden_from_publishing: false,
                    key: None,
                    code_syntax: CodeSyntax::new(),
                },
            );
        }
        doc.variable_collections.insert(
            "c_perf".into(),
            VariableCollection {
                id: "c_perf".into(),
                name: "Scale".into(),
                modes,
                default_mode: "m0".into(),
                variables: var_ids,
                key: None,
                source_library_key: None,
            },
        );

        let t0 = std::time::Instant::now();
        let bundle = export_variables(&doc, VariablesFormat::Dtcg).unwrap();
        let export_ms = t0.elapsed().as_secs_f64() * 1000.0;
        assert_eq!(bundle.files.len(), 5);

        let zip_bytes = bundle.to_zip_bytes().unwrap();
        let t1 = std::time::Instant::now();
        let mut imported = Document::new("imported");
        let rep = import_variables_bytes(&mut imported, &zip_bytes, Some("tokens.zip"), VariablesFormat::Dtcg, ImportVariablesOptions::default()).unwrap();
        let import_ms = t1.elapsed().as_secs_f64() * 1000.0;
        assert_eq!(rep.added, 5000);
        // Debug builds on CI/shared runners get a 1500ms ceiling; release builds run well under 200ms.
        let budget_ms = if cfg!(debug_assertions) { 1500.0 } else { 200.0 };
        assert!(export_ms <= budget_ms, "export_ms={export_ms:.1}ms exceeded {budget_ms}ms");
        assert!(import_ms <= budget_ms, "import_ms={import_ms:.1}ms exceeded {budget_ms}ms");
    }
}
