//! `designos_get_guidelines` MCP tool, surface guide front-matter parser, and archetype schema validator (T7.40).
use crate::prompts::{estimate_tokens, generate_design_system_rules, RulesOptions, DEFAULT_MAX_TOKENS};
use designos_doc::Document;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

pub const SURFACE_TOPICS: &[(&str, &str)] = &[
    ("code", "General code generation rules and structure."),
    ("design-system-guide", "How to structure tokens, collections, and component sets."),
    ("landing-page", "Marketing and landing page layout, hero, and section guidelines."),
    ("mobile-app", "iOS and Android mobile app screen layout, navigation, and touch target guidelines."),
    ("slides", "Presentation slide deck layout, typography, and visual hierarchy guidelines."),
    ("table", "Data table layout, column alignment, density, and pagination guidelines."),
    ("tailwind", "Tailwind CSS utility mapping and token usage guidelines."),
    ("web-app", "Application shell, dashboard, form, and dense workspace layout guidelines."),
];

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GuidelinesArgs {
    #[serde(default)]
    pub topic: Option<String>,
    #[serde(default)]
    pub platform: Option<String>,
    #[serde(default)]
    pub max_tokens: Option<usize>,
}

/// YAML front-matter header for `crates/designos-mcp/guidelines/surfaces/<topic>.md` (§4.1).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SurfaceFrontMatter {
    pub id: String,
    pub title: String,
    pub summary: String,
    pub tokens_estimate: u32,
}

/// Top-level schema for `crates/designos-mcp/guidelines/archetypes/<id>.toml` (§4.2).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ArchetypeFile {
    pub archetype: ArchetypeMeta,
    pub tokens: ArchetypeTokens,
    #[serde(default)]
    pub component: Vec<ArchetypeComponent>,
    pub content: ArchetypeContent,
    pub code: ArchetypeCode,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ArchetypeMeta {
    pub id: String,
    pub name: String,
    pub category: String,
    pub summary: String,
    pub density: String,
    pub motion: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ArchetypeTokens {
    pub font_sans: String,
    pub font_mono: String,
    pub font_display: Option<String>,
    pub radius_sm: f32,
    pub radius_md: f32,
    pub radius_lg: f32,
    pub spacing_unit: f32,
    pub surface_light: String,
    pub surface_dark: String,
    pub text_primary_light: String,
    pub text_primary_dark: String,
    pub accent_primary: String,
    pub semantic_positive: String,
    pub semantic_warning: String,
    pub semantic_danger: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ArchetypeComponent {
    pub name: String,
    pub required_variants: Vec<String>,
    pub required_states: Vec<String>,
    pub notes: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ArchetypeContent {
    pub forbidden_strings: Vec<String>,
    pub number_format: String,
    pub samples: BTreeMap<String, Vec<String>>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ArchetypeCode {
    pub layout_rules: Vec<String>,
    pub a11y_rules: Vec<String>,
}

pub fn guidelines_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("guidelines")
}

/// Executes `designos_get_guidelines` against `doc`.
pub fn call_get_guidelines(doc: &Document, raw_args: Value) -> Result<Value, String> {
    call_get_guidelines_with_dir(doc, raw_args, &guidelines_dir())
}

pub fn call_get_guidelines_with_dir(doc: &Document, raw_args: Value, base_dir: &Path) -> Result<Value, String> {
    let args: GuidelinesArgs = serde_json::from_value(raw_args).map_err(|e| format!("INVALID_ARGUMENTS: {e}"))?;
    let opts = RulesOptions::parse(Some("file"), args.platform.as_deref(), Some(args.max_tokens.unwrap_or(DEFAULT_MAX_TOKENS)))?;

    let Some(topic) = args.topic.as_deref().map(str::trim).filter(|s| !s.is_empty()) else {
        return Ok(list_topics_value(base_dir));
    };

    if topic == "design-system" {
        let generated = generate_design_system_rules(doc, &opts)?;
        return Ok(json!({
            "topic": "design-system",
            "content": generated.content,
            "truncated": generated.truncated,
            "sources": generated.sources,
        }));
    }

    if let Some((_, _default_summary)) = SURFACE_TOPICS.iter().find(|(id, _)| *id == topic) {
        let path = base_dir.join("surfaces").join(format!("{topic}.md"));
        if !path.is_file() {
            return Err(format!("TOPIC_NOT_AVAILABLE: topic '{topic}' is not available yet (provided by E15-21)"));
        }
        let raw = fs::read_to_string(&path).map_err(|e| format!("Failed to read surface guide '{topic}': {e}"))?;
        let (fm, body) = parse_surface_guide(&raw)?;
        let (content, truncated) = truncate_markdown(&body, opts.max_tokens, &fm.title);
        return Ok(json!({
            "topic": topic,
            "title": fm.title,
            "summary": fm.summary,
            "content": content,
            "truncated": truncated,
            "sources": ["guidelines/surfaces"],
        }));
    }

    if topic == "archetypes" {
        let arch_dir = base_dir.join("archetypes");
        let archetypes = validate_archetypes_dir(&arch_dir)?;
        if archetypes.is_empty() {
            return Err("TOPIC_NOT_AVAILABLE: topic 'archetypes' is not available yet (provided by E15-18)".to_string());
        }
        let mut md = String::from("# Design Archetypes\n\n");
        for a in &archetypes {
            md.push_str(&format!("- **`{}`** ({}, {}): {}\n", a.archetype.id, a.archetype.name, a.archetype.category, a.archetype.summary));
        }
        let (content, truncated) = truncate_markdown(&md, opts.max_tokens, "archetypes");
        return Ok(json!({
            "topic": "archetypes",
            "content": content,
            "truncated": truncated,
            "sources": ["guidelines/archetypes"],
        }));
    }

    if let Some(arch_id) = topic.strip_prefix("archetype:") {
        let arch_id = arch_id.trim();
        if arch_id.is_empty() || !arch_id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_') {
            return Err(format!("UNKNOWN_TOPIC: invalid archetype topic '{topic}'; valid_options: {}", valid_options_json()));
        }
        let path = base_dir.join("archetypes").join(format!("{arch_id}.toml"));
        if !path.is_file() {
            return Err(format!("TOPIC_NOT_AVAILABLE: topic '{topic}' is not available yet (provided by E15-18)"));
        }
        let raw = fs::read_to_string(&path).map_err(|e| format!("Failed to read archetype '{arch_id}': {e}"))?;
        let parsed = parse_archetype_toml(&raw)?;
        let body = serde_json::to_string_pretty(&parsed).map_err(|e| e.to_string())?;
        let (content, truncated) = truncate_markdown(&body, opts.max_tokens, topic);
        return Ok(json!({
            "topic": topic,
            "archetype": parsed,
            "content": content,
            "truncated": truncated,
            "sources": [format!("guidelines/archetypes/{arch_id}.toml")],
        }));
    }

    if topic == "rules" {
        let path = base_dir.join("rules.md");
        if !path.is_file() {
            return Err("TOPIC_NOT_AVAILABLE: topic 'rules' is not available yet (provided by E15-15)".to_string());
        }
        let raw = fs::read_to_string(&path).map_err(|e| format!("Failed to read rules guide: {e}"))?;
        let (content, truncated) = truncate_markdown(&raw, opts.max_tokens, "rules");
        return Ok(json!({
            "topic": "rules",
            "content": content,
            "truncated": truncated,
            "sources": ["guidelines/rules.md"],
        }));
    }

    Err(format!("UNKNOWN_TOPIC: unknown topic '{topic}'; valid_options: {}", valid_options_json()))
}

fn valid_options_json() -> String {
    let mut opts = vec!["design-system"];
    for (id, _) in SURFACE_TOPICS {
        opts.push(id);
    }
    opts.push("archetypes");
    opts.push("archetype:<id>");
    opts.push("rules");
    serde_json::to_string(&opts).unwrap_or_default()
}

fn list_topics_value(base_dir: &Path) -> Value {
    let mut topics = Vec::new();
    topics.push(json!({
        "id": "design-system",
        "summary": "Design-system rules generated from the open document's variables, styles, and components.",
        "available": true,
        "provided_by": "T7.40",
    }));

    for (id, default_summary) in SURFACE_TOPICS {
        let path = base_dir.join("surfaces").join(format!("{id}.md"));
        let (available, summary) = if path.is_file() {
            match fs::read_to_string(&path).and_then(|s| parse_surface_guide(&s).map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))) {
                Ok((fm, _)) => (true, fm.summary),
                Err(_) => (true, (*default_summary).to_string()),
            }
        } else {
            (false, (*default_summary).to_string())
        };
        topics.push(json!({
            "id": id,
            "summary": summary,
            "available": available,
            "provided_by": "E15-21",
        }));
    }

    let arch_dir = base_dir.join("archetypes");
    let has_archetypes = has_toml_files(&arch_dir);
    topics.push(json!({
        "id": "archetypes",
        "summary": "Index of vertical design archetypes (fintech, saas, healthcare, consumer, etc.).",
        "available": has_archetypes,
        "provided_by": "E15-18",
    }));

    let has_rules = base_dir.join("rules.md").is_file();
    topics.push(json!({
        "id": "rules",
        "summary": "Catalogue of 85 automated design critique and linting rules.",
        "available": has_rules,
        "provided_by": "E15-15",
    }));

    json!({ "topics": topics })
}

fn has_toml_files(dir: &Path) -> bool {
    let Ok(entries) = fs::read_dir(dir) else {
        return false;
    };
    entries.flatten().any(|e| e.path().extension().and_then(|x| x.to_str()) == Some("toml"))
}

fn truncate_markdown(body: &str, max_tokens: usize, section_name: &str) -> (String, bool) {
    let total = estimate_tokens(body);
    if total <= max_tokens {
        return (body.to_string(), false);
    }
    let budget_chars = max_tokens.saturating_mul(4).saturating_sub(64);
    let mut out = String::new();
    for line in body.lines() {
        if out.chars().count() + line.chars().count() + 1 > budget_chars {
            break;
        }
        out.push_str(line);
        out.push('\n');
    }
    let kept = estimate_tokens(&out);
    let omitted = total.saturating_sub(kept).max(1);
    (format!("{}\n… truncated: {omitted} more tokens in {section_name}", out.trim_end()), true)
}

/// Parses a surface guide markdown file with YAML front-matter (§4.1).
pub fn parse_surface_guide(raw: &str) -> Result<(SurfaceFrontMatter, String), String> {
    let trimmed = raw.trim_start_matches('\u{feff}');
    let Some(rest) = trimmed.strip_prefix("---\n").or_else(|| trimmed.strip_prefix("---\r\n")) else {
        return Err("Surface guide is missing opening '---' YAML front-matter delimiter".into());
    };
    let Some(end_idx) = rest.find("\n---") else {
        return Err("Surface guide is missing closing '---' YAML front-matter delimiter".into());
    };
    let yaml_block = &rest[..end_idx];
    let after = &rest[end_idx + 4..];
    let body = after.strip_prefix("\r\n").or_else(|| after.strip_prefix('\n')).unwrap_or(after).to_string();

    let mut id = None;
    let mut title = None;
    let mut summary = None;
    let mut tokens_estimate = None;

    for line in yaml_block.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((k, v)) = line.split_once(':') else {
            return Err(format!("Malformed front-matter line: '{line}'"));
        };
        let key = k.trim();
        let val = unquote(v.trim());
        match key {
            "id" => id = Some(val),
            "title" => title = Some(val),
            "summary" => summary = Some(val),
            "tokens_estimate" => {
                let n = val.parse::<u32>().map_err(|_| format!("Front-matter 'tokens_estimate' must be an integer, got '{val}'"))?;
                tokens_estimate = Some(n);
            }
            _ => {}
        }
    }

    Ok((
        SurfaceFrontMatter {
            id: id.ok_or("Front-matter missing 'id'")?,
            title: title.ok_or("Front-matter missing 'title'")?,
            summary: summary.ok_or("Front-matter missing 'summary'")?,
            tokens_estimate: tokens_estimate.ok_or("Front-matter missing 'tokens_estimate'")?,
        },
        body,
    ))
}

fn unquote(s: &str) -> String {
    let s = s.trim();
    if (s.starts_with('"') && s.ends_with('"') && s.len() >= 2) || (s.starts_with('\'') && s.ends_with('\'') && s.len() >= 2) {
        s[1..s.len() - 1].to_string()
    } else {
        s.to_string()
    }
}

/// Validates and parses all `.toml` files in `crates/designos-mcp/guidelines/archetypes/`.
pub fn validate_archetypes_dir(dir: &Path) -> Result<Vec<ArchetypeFile>, String> {
    if !dir.exists() {
        return Ok(Vec::new());
    }
    let mut paths: Vec<PathBuf> = fs::read_dir(dir)
        .map_err(|e| format!("Cannot read archetypes directory '{}': {e}", dir.display()))?
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().and_then(|x| x.to_str()) == Some("toml"))
        .collect();
    paths.sort();

    let mut out = Vec::with_capacity(paths.len());
    for path in paths {
        let raw = fs::read_to_string(&path).map_err(|e| format!("Cannot read '{}': {e}", path.display()))?;
        let parsed = parse_archetype_toml(&raw).map_err(|e| format!("Invalid archetype '{}': {e}", path.display()))?;
        out.push(parsed);
    }
    Ok(out)
}

/// Parses a TOML (or JSON) archetype definition into [`ArchetypeFile`] (§4.2).
pub fn parse_archetype_toml(raw: &str) -> Result<ArchetypeFile, String> {
    let trimmed = raw.trim();
    if trimmed.starts_with('{') {
        return serde_json::from_str(trimmed).map_err(|e| e.to_string());
    }
    let root = toml_subset_to_json(trimmed)?;
    serde_json::from_value(root).map_err(|e| format!("Archetype schema validation failed: {e}"))
}

fn toml_subset_to_json(raw: &str) -> Result<Value, String> {
    let mut root = serde_json::Map::new();
    let mut current_path: Vec<String> = Vec::new();
    let mut current_is_array_item = false;

    let mut lines = raw.lines().enumerate().peekable();
    while let Some((line_idx, raw_line)) = lines.next() {
        let line_no = line_idx + 1;
        let line = strip_toml_comment(raw_line).trim();
        if line.is_empty() {
            continue;
        }
        if line.starts_with("[[") && line.ends_with("]]") {
            let inner = line[2..line.len() - 2].trim();
            if inner.is_empty() {
                return Err(format!("line {line_no}: empty [[array]] header"));
            }
            current_path = inner.split('.').map(|s| unquote(s.trim())).collect();
            current_is_array_item = true;
            ensure_array_entry(&mut root, &current_path)?;
            continue;
        }
        if line.starts_with('[') && line.ends_with(']') {
            let inner = line[1..line.len() - 1].trim();
            if inner.is_empty() {
                return Err(format!("line {line_no}: empty [table] header"));
            }
            current_path = inner.split('.').map(|s| unquote(s.trim())).collect();
            current_is_array_item = false;
            ensure_table(&mut root, &current_path)?;
            continue;
        }

        let Some((raw_key, raw_val)) = line.split_once('=') else {
            return Err(format!("line {line_no}: expected 'key = value', got '{line}'"));
        };
        let key = unquote(raw_key.trim());
        let mut val_acc = raw_val.trim().to_string();
        while !brackets_balanced(&val_acc) {
            let Some((_, next_raw)) = lines.next() else {
                return Err(format!("line {line_no}: unclosed array/table value for '{key}'"));
            };
            let next_clean = strip_toml_comment(next_raw).trim();
            if !next_clean.is_empty() {
                val_acc.push(' ');
                val_acc.push_str(next_clean);
            }
        }
        let val = parse_toml_value(val_acc.trim(), line_no)?;
        let target = get_target_map(&mut root, &current_path, current_is_array_item)?;
        target.insert(key, val);
    }

    Ok(Value::Object(root))
}

fn strip_toml_comment(line: &str) -> &str {
    let mut in_double = false;
    let mut in_single = false;
    let mut prev_escape = false;
    for (idx, ch) in line.char_indices() {
        if prev_escape {
            prev_escape = false;
            continue;
        }
        if ch == '\\' && in_double {
            prev_escape = true;
            continue;
        }
        if ch == '"' && !in_single {
            in_double = !in_double;
        } else if ch == '\'' && !in_double {
            in_single = !in_single;
        } else if ch == '#' && !in_double && !in_single {
            return &line[..idx];
        }
    }
    line
}

fn brackets_balanced(s: &str) -> bool {
    let mut depth = 0i32;
    let mut in_double = false;
    let mut in_single = false;
    let mut prev_escape = false;
    for ch in s.chars() {
        if prev_escape {
            prev_escape = false;
            continue;
        }
        if ch == '\\' && in_double {
            prev_escape = true;
            continue;
        }
        if ch == '"' && !in_single {
            in_double = !in_double;
        } else if ch == '\'' && !in_double {
            in_single = !in_single;
        } else if !in_double && !in_single {
            if ch == '[' || ch == '{' {
                depth += 1;
            } else if ch == ']' || ch == '}' {
                depth -= 1;
            }
        }
    }
    depth <= 0 && !in_double && !in_single
}

fn parse_toml_value(s: &str, line_no: usize) -> Result<Value, String> {
    let s = s.trim();
    if s.starts_with('"') {
        return serde_json::from_str::<String>(s).map(Value::String).map_err(|e| format!("line {line_no}: invalid string {s}: {e}"));
    }
    if s.starts_with('\'') && s.ends_with('\'') && s.len() >= 2 {
        return Ok(Value::String(s[1..s.len() - 1].to_string()));
    }
    if s == "true" {
        return Ok(Value::Bool(true));
    }
    if s == "false" {
        return Ok(Value::Bool(false));
    }
    if s.starts_with('[') && s.ends_with(']') {
        let inner = s[1..s.len() - 1].trim();
        if inner.is_empty() {
            return Ok(Value::Array(Vec::new()));
        }
        let items = split_top_level(inner, ',');
        let mut arr = Vec::new();
        for item in items {
            let item = item.trim();
            if !item.is_empty() {
                arr.push(parse_toml_value(item, line_no)?);
            }
        }
        return Ok(Value::Array(arr));
    }
    if s.starts_with('{') && s.ends_with('}') {
        let inner = s[1..s.len() - 1].trim();
        let mut map = serde_json::Map::new();
        if !inner.is_empty() {
            for entry in split_top_level(inner, ',') {
                let entry = entry.trim();
                if entry.is_empty() {
                    continue;
                }
                let Some((k, v)) = entry.split_once('=') else {
                    return Err(format!("line {line_no}: invalid inline table entry '{entry}'"));
                };
                map.insert(unquote(k.trim()), parse_toml_value(v.trim(), line_no)?);
            }
        }
        return Ok(Value::Object(map));
    }
    let clean_num = s.replace('_', "");
    if let Ok(i) = clean_num.parse::<i64>() {
        return Ok(Value::Number(i.into()));
    }
    if let Ok(f) = clean_num.parse::<f64>() {
        if let Some(n) = serde_json::Number::from_f64(f) {
            return Ok(Value::Number(n));
        }
    }
    Err(format!("line {line_no}: unsupported TOML value '{s}'"))
}

fn split_top_level(s: &str, sep: char) -> Vec<&str> {
    let mut out = Vec::new();
    let mut depth = 0i32;
    let mut in_double = false;
    let mut in_single = false;
    let mut prev_escape = false;
    let mut start = 0usize;
    for (idx, ch) in s.char_indices() {
        if prev_escape {
            prev_escape = false;
            continue;
        }
        if ch == '\\' && in_double {
            prev_escape = true;
            continue;
        }
        if ch == '"' && !in_single {
            in_double = !in_double;
        } else if ch == '\'' && !in_double {
            in_single = !in_single;
        } else if !in_double && !in_single {
            if ch == '[' || ch == '{' {
                depth += 1;
            } else if ch == ']' || ch == '}' {
                depth -= 1;
            } else if ch == sep && depth == 0 {
                out.push(&s[start..idx]);
                start = idx + ch.len_utf8();
            }
        }
    }
    out.push(&s[start..]);
    out
}

fn ensure_table(root: &mut serde_json::Map<String, Value>, path: &[String]) -> Result<(), String> {
    let mut cur = root;
    for seg in path {
        let entry = cur.entry(seg.clone()).or_insert_with(|| Value::Object(serde_json::Map::new()));
        cur = match entry {
            Value::Object(m) => m,
            Value::Array(arr) => arr.last_mut().and_then(Value::as_object_mut).ok_or_else(|| format!("Table path segment '{seg}' is not an object"))?,
            _ => return Err(format!("Table path segment '{seg}' conflicts with scalar")),
        };
    }
    Ok(())
}

fn ensure_array_entry(root: &mut serde_json::Map<String, Value>, path: &[String]) -> Result<(), String> {
    let Some((last, prefix)) = path.split_last() else {
        return Err("Empty array path".into());
    };
    let mut cur = root;
    for seg in prefix {
        let entry = cur.entry(seg.clone()).or_insert_with(|| Value::Object(serde_json::Map::new()));
        cur = entry.as_object_mut().ok_or_else(|| format!("Segment '{seg}' is not a table"))?;
    }
    let arr = cur.entry(last.clone()).or_insert_with(|| Value::Array(Vec::new())).as_array_mut().ok_or_else(|| format!("Segment '{last}' is not an array"))?;
    arr.push(Value::Object(serde_json::Map::new()));
    Ok(())
}

fn get_target_map<'a>(
    root: &'a mut serde_json::Map<String, Value>,
    path: &[String],
    is_array_item: bool,
) -> Result<&'a mut serde_json::Map<String, Value>, String> {
    if path.is_empty() {
        return Ok(root);
    }
    if is_array_item {
        let Some((last, prefix)) = path.split_last() else {
            return Ok(root);
        };
        let mut cur = root;
        for seg in prefix {
            cur = cur.get_mut(seg).and_then(Value::as_object_mut).ok_or_else(|| format!("Missing table '{seg}'"))?;
        }
        let arr = cur.get_mut(last).and_then(Value::as_array_mut).ok_or_else(|| format!("Missing array '{last}'"))?;
        return arr.last_mut().and_then(Value::as_object_mut).ok_or_else(|| format!("Empty array '{last}'"));
    }
    let mut cur = root;
    for seg in path {
        cur = match cur.get_mut(seg) {
            Some(Value::Object(m)) => m,
            Some(Value::Array(arr)) => arr.last_mut().and_then(Value::as_object_mut).ok_or_else(|| format!("Array '{seg}' has no active table"))?,
            _ => return Err(format!("Missing table '{seg}'")),
        };
    }
    Ok(cur)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn topic_list_and_unavailable() {
        let doc = Document::new("Test");
        let list = call_get_guidelines(&doc, json!({})).unwrap();
        let topics = list["topics"].as_array().expect("topics array");

        let ds = topics.iter().find(|t| t["id"] == "design-system").expect("design-system topic");
        assert_eq!(ds["available"], true);
        assert_eq!(ds["provided_by"], "T7.40");

        let lp = topics.iter().find(|t| t["id"] == "landing-page").expect("landing-page topic");
        assert_eq!(lp["available"], false);
        assert_eq!(lp["provided_by"], "E15-21");

        // Requesting an unavailable surface topic returns TOPIC_NOT_AVAILABLE naming E15-21
        let err = call_get_guidelines(&doc, json!({"topic": "landing-page"})).unwrap_err();
        assert!(err.contains("TOPIC_NOT_AVAILABLE") && err.contains("E15-21"), "unexpected error: {err}");

        // Requesting unavailable archetypes returns TOPIC_NOT_AVAILABLE naming E15-18
        let err_arch = call_get_guidelines(&doc, json!({"topic": "archetypes"})).unwrap_err();
        assert!(err_arch.contains("TOPIC_NOT_AVAILABLE") && err_arch.contains("E15-18"), "unexpected error: {err_arch}");
        let err_arch_id = call_get_guidelines(&doc, json!({"topic": "archetype:fintech"})).unwrap_err();
        assert!(err_arch_id.contains("TOPIC_NOT_AVAILABLE") && err_arch_id.contains("E15-18"), "unexpected error: {err_arch_id}");

        // Requesting unavailable rules returns TOPIC_NOT_AVAILABLE naming E15-15
        let err_rules = call_get_guidelines(&doc, json!({"topic": "rules"})).unwrap_err();
        assert!(err_rules.contains("TOPIC_NOT_AVAILABLE") && err_rules.contains("E15-15"), "unexpected error: {err_rules}");

        // Unknown topic returns UNKNOWN_TOPIC with valid_options
        let err_unknown = call_get_guidelines(&doc, json!({"topic": "not-a-topic"})).unwrap_err();
        assert!(err_unknown.contains("UNKNOWN_TOPIC") && err_unknown.contains("valid_options"), "unexpected error: {err_unknown}");

        // design-system topic succeeds and returns generated rules
        let ds_res = call_get_guidelines(&doc, json!({"topic": "design-system"})).unwrap();
        assert_eq!(ds_res["topic"], "design-system");
        assert_eq!(ds_res["truncated"], false);
        assert!(ds_res["content"].as_str().unwrap().contains("# Design system rules"));
    }

    #[test]
    fn archetype_files_validate() {
        // 1. Validate every file currently under `crates/designos-mcp/guidelines/archetypes/`
        let dir = guidelines_dir().join("archetypes");
        let files = validate_archetypes_dir(&dir).expect("all archetype files must validate");
        let _ = files;

        // 2. Also verify the TOML schema parser against a complete sample ArchetypeFile (§4.2)
        let sample_toml = r##"
[archetype]
id = "fintech-precision"
name = "Fintech Precision"
category = "fintech"
summary = "High-trust financial workspace with tabular numbers and crisp borders."
density = "compact"
motion = "subtle"

[tokens]
font_sans = "Inter"
font_mono = "JetBrains Mono"
font_display = "Plus Jakarta Sans"
radius_sm = 4.0
radius_md = 6.0
radius_lg = 10.0
spacing_unit = 4.0
surface_light = "#FFFFFF"
surface_dark = "#0B0F17"
text_primary_light = "#0F172A"
text_primary_dark = "#F8FAFC"
accent_primary = "#2563EB"
semantic_positive = "#16A34A"
semantic_warning = "#D97706"
semantic_danger = "#DC2626"

[[component]]
name = "Button"
required_variants = ["primary", "secondary", "destructive"]
required_states = ["default", "hover", "disabled"]
notes = "Use 32px height in dense tables."

[content]
forbidden_strings = ["Lorem ipsum", "John Doe", "$99.99"]
number_format = "tabular-nums"

[content.samples]
account_names = ["Operating Account · 4821", "Treasury Reserve · 9014"]

[code]
layout_rules = ["Align currency columns right with tabular-nums"]
a11y_rules = ["Maintain >= 4.5:1 text contrast on all status badges"]
"##;

        let parsed = parse_archetype_toml(sample_toml).expect("valid sample archetype TOML");
        assert_eq!(parsed.archetype.id, "fintech-precision");
        assert_eq!(parsed.tokens.radius_md, 6.0);
        assert_eq!(parsed.component.len(), 1);
        assert_eq!(parsed.component[0].name, "Button");
        assert_eq!(parsed.content.samples.get("account_names").unwrap().len(), 2);
        assert_eq!(parsed.code.layout_rules.len(), 1);
    }

    #[test]
    fn surface_front_matter_parses() {
        let sample = "---\nid: landing-page\ntitle: Landing Page Guidelines\nsummary: Layout and rhythm for marketing pages.\ntokens_estimate: 1200\n---\n# Landing Page\nUse clear hero hierarchy.\n";
        let (fm, body) = parse_surface_guide(sample).unwrap();
        assert_eq!(fm.id, "landing-page");
        assert_eq!(fm.title, "Landing Page Guidelines");
        assert_eq!(fm.tokens_estimate, 1200);
        assert!(body.starts_with("# Landing Page"));
    }
}
