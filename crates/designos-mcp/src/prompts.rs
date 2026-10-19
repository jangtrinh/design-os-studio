//! MCP prompt `create_design_system_rules` and design-system rules generator (T7.40).
use designos_doc::{
    CodeSyntaxPlatform, Color, Document, Effect, LayoutGrid, LineHeight, Node, NodeId, NodeKind, Paint, PaintKind, PropKind, PropValue, Style, TextStyle,
    Variable, VariableCollection, VariableKind, VariableMode, VariableValue,
};
use rmcp::model::{GetPromptResult, JsonObject, Prompt, PromptArgument, PromptMessage, Role};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

pub const CREATE_DESIGN_SYSTEM_RULES: &str = "create_design_system_rules";
pub const MIN_MAX_TOKENS: usize = 500;
pub const MAX_MAX_TOKENS: usize = 25_000;
pub const DEFAULT_MAX_TOKENS: usize = 8_000;

const SPACING_SCALE_PX: &[u32] = &[0, 2, 4, 6, 8, 12, 16, 20, 24, 32, 40, 48, 64, 80, 96];
const TYPE_SCALE_PX: &[u32] = &[10, 11, 12, 13, 14, 15, 16, 18, 20, 24, 28, 30, 32, 36, 40, 48, 56, 60, 64, 72, 80, 96];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RulesScope {
    File,
    Page(NodeId),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RulesOptions {
    pub scope: RulesScope,
    pub platform: CodeSyntaxPlatform,
    pub platform_label: &'static str,
    pub max_tokens: usize,
}

impl Default for RulesOptions {
    fn default() -> Self {
        Self { scope: RulesScope::File, platform: CodeSyntaxPlatform::Web, platform_label: "web", max_tokens: DEFAULT_MAX_TOKENS }
    }
}

impl RulesOptions {
    pub fn parse(scope: Option<&str>, platform: Option<&str>, max_tokens: Option<usize>) -> Result<Self, String> {
        let parsed_scope = match scope.map(str::trim).filter(|s| !s.is_empty()) {
            None | Some("file") => RulesScope::File,
            Some(s) => {
                if let Some(rest) = s.strip_prefix("page:") {
                    let id = rest.trim().parse::<u64>().map_err(|_| format!("INVALID_ARGUMENTS: invalid scope '{s}'; expected 'file' or 'page:<id>'"))?;
                    if id == 0 {
                        return Err("INVALID_ARGUMENTS: page id in 'page:<id>' must be >= 1".to_string());
                    }
                    RulesScope::Page(NodeId(id))
                } else {
                    return Err(format!("INVALID_ARGUMENTS: invalid scope '{s}'; expected 'file' or 'page:<id>'"));
                }
            }
        };

        let (plat, label) = match platform.map(str::trim).filter(|s| !s.is_empty()) {
            None | Some("web") => (CodeSyntaxPlatform::Web, "web"),
            Some("ios") => (CodeSyntaxPlatform::Ios, "ios"),
            Some("android") => (CodeSyntaxPlatform::Android, "android"),
            Some(other) => {
                return Err(format!("INVALID_ARGUMENTS: invalid platform '{other}'; valid_options: [\"web\", \"ios\", \"android\"]"));
            }
        };

        let max_tokens = max_tokens.unwrap_or(DEFAULT_MAX_TOKENS);
        if !(MIN_MAX_TOKENS..=MAX_MAX_TOKENS).contains(&max_tokens) {
            return Err(format!("INVALID_ARGUMENTS: max_tokens ({max_tokens}) must be between {MIN_MAX_TOKENS} and {MAX_MAX_TOKENS}"));
        }

        Ok(Self { scope: parsed_scope, platform: plat, platform_label: label, max_tokens })
    }

    pub fn from_prompt_arguments(args: Option<&JsonObject>) -> Result<Self, String> {
        let Some(map) = args else {
            return Ok(Self::default());
        };
        for key in map.keys() {
            if !matches!(key.as_str(), "scope" | "platform" | "max_tokens") {
                return Err(format!("INVALID_ARGUMENTS: unknown prompt argument '{key}'; valid_options: [\"scope\", \"platform\", \"max_tokens\"]"));
            }
        }
        let scope = match map.get("scope") {
            None | Some(Value::Null) => None,
            Some(Value::String(s)) => Some(s.as_str()),
            Some(_) => return Err("INVALID_ARGUMENTS: 'scope' must be a string".into()),
        };
        let platform = match map.get("platform") {
            None | Some(Value::Null) => None,
            Some(Value::String(s)) => Some(s.as_str()),
            Some(_) => return Err("INVALID_ARGUMENTS: 'platform' must be a string".into()),
        };
        let max_tokens = match map.get("max_tokens") {
            None | Some(Value::Null) => None,
            Some(Value::Number(n)) => Some(n.as_u64().ok_or_else(|| "INVALID_ARGUMENTS: 'max_tokens' must be a positive integer".to_string())? as usize),
            Some(Value::String(s)) if !s.trim().is_empty() => {
                Some(s.trim().parse::<usize>().map_err(|_| format!("INVALID_ARGUMENTS: 'max_tokens' ('{s}') must be an integer"))?)
            }
            Some(Value::String(_)) => None,
            Some(_) => return Err("INVALID_ARGUMENTS: 'max_tokens' must be an integer".into()),
        };
        Self::parse(scope, platform, max_tokens)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GeneratedRules {
    pub content: String,
    pub truncated: bool,
    pub sources: Vec<String>,
}

/// Returns the catalog of MCP prompts exposed by `designos-mcp`.
pub fn prompts() -> Vec<Prompt> {
    vec![Prompt::new(
        CREATE_DESIGN_SYSTEM_RULES,
        Some("Generate design-system rules from this file's variables, styles and components, naming every token source."),
        Some(vec![
            PromptArgument::new("scope").with_description("Scope to inspect: 'file' (default) or 'page:<id>'.").with_required(false),
            PromptArgument::new("platform").with_description("Target platform for code syntax: 'web' (default), 'ios', or 'android'.").with_required(false),
            PromptArgument::new("max_tokens").with_description("Maximum token budget (500..=25000, default 8000).").with_required(false),
        ]),
    )]
}

/// Executes an MCP `prompts/get` request against `doc`.
pub fn get_prompt(doc: &Document, name: &str, arguments: Option<&JsonObject>) -> Result<GetPromptResult, String> {
    if name != CREATE_DESIGN_SYSTEM_RULES {
        return Err(format!("UNKNOWN_PROMPT: unknown prompt '{name}'; valid_options: [\"{CREATE_DESIGN_SYSTEM_RULES}\"]"));
    }
    let opts = RulesOptions::from_prompt_arguments(arguments)?;
    let generated = generate_design_system_rules(doc, &opts)?;
    let instruction = format!(
        "Save the following design-system rules to your code repository's agent instructions or rules file (for example `CLAUDE.md`, `AGENTS.md`, or `.cursorrules`) and enforce every token, style, and component rule when generating `{}` UI code.\n\n{}",
        opts.platform_label, generated.content
    );
    Ok(GetPromptResult::new(vec![PromptMessage::new_text(Role::User, instruction)]).with_description(format!(
        "Design-system rules for '{}' ({}, sources: {})",
        doc.name,
        opts.platform_label,
        generated.sources.join(", ")
    )))
}

struct ResolvedCollection<'a> {
    col: &'a VariableCollection,
    origin: String,
    vars: Vec<&'a Variable>,
}

/// Generates the markdown design-system rules document from `doc` and `opts`.
pub fn generate_design_system_rules(doc: &Document, opts: &RulesOptions) -> Result<GeneratedRules, String> {
    if let RulesScope::Page(page_id) = opts.scope {
        if !doc.pages.contains(&page_id) {
            return Err(format!("INVALID_ARGUMENTS: page {} does not exist in document '{}'", page_id.0, doc.name));
        }
    }

    let mut sources_set = BTreeSet::new();
    sources_set.insert("This file".to_string());

    // Build unified variable map (local variables).
    let mut all_vars: BTreeMap<String, &Variable> = BTreeMap::new();
    for (k, v) in &doc.variables {
        all_vars.insert(k.clone(), v);
    }

    // Gather collections (local and subscribed library collections).
    let mut resolved_collections: Vec<ResolvedCollection<'_>> = Vec::new();
    let mut seen_col_ids = BTreeSet::new();

    for col in doc.variable_collections.values() {
        seen_col_ids.insert(col.id.clone());
        let origin = collection_origin(doc, col);
        sources_set.insert(origin.clone());
        let vars = collect_collection_variables(col, &doc.variables, opts.scope);
        resolved_collections.push(ResolvedCollection { col, origin, vars });
    }

    // Also handle any orphan variables whose collection is missing from `variable_collections`.
    let orphan_vars: Vec<&Variable> = doc
        .variables
        .values()
        .filter(|v| !seen_col_ids.contains(&v.collection))
        .filter(|v| !(matches!(opts.scope, RulesScope::Page(_)) && v.hidden_from_publishing))
        .collect();

    // Section 1: Token sources
    let mut token_section = String::from("## Token sources\n\n");
    let total_vars: usize = resolved_collections.iter().map(|c| c.vars.len()).sum::<usize>() + orphan_vars.len();
    let mut usage_rules: Vec<String> = Vec::new();

    if total_vars == 0 && resolved_collections.is_empty() {
        token_section.push_str("No variables in this file\n");
    } else if total_vars == 0 {
        for rc in &resolved_collections {
            token_section.push_str(&format!("### {} ({})\n\n", rc.col.name, rc.origin));
            token_section.push_str("_No variables in this collection._\n\n");
        }
        token_section.push_str("No variables in this file\n");
    } else {
        for rc in &resolved_collections {
            let modes: Vec<VariableMode> = if rc.col.modes.is_empty() {
                vec![VariableMode { id: if rc.col.default_mode.is_empty() { "default".into() } else { rc.col.default_mode.clone() }, name: "Default".into() }]
            } else {
                rc.col.modes.clone()
            };
            let mode_names: Vec<&str> = modes.iter().map(|m| m.name.as_str()).collect();
            token_section.push_str(&format!("### {} ({})\n\n", rc.col.name, rc.origin));
            token_section.push_str(&format!("- **Origin:** {}\n- **Modes:** {}\n\n", rc.origin, mode_names.join(", ")));

            if rc.vars.is_empty() {
                token_section.push_str("_No variables in this collection._\n\n");
                continue;
            }

            // Table header: | Name | Origin | Type | <Modes...> | Code syntax (<platform>) |
            token_section.push_str("| Name | Origin | Type |");
            for m in &modes {
                token_section.push_str(&format!(" {} |", escape_table_cell(&m.name)));
            }
            token_section.push_str(&format!(" Code syntax ({}) |\n", opts.platform_label));

            token_section.push_str("| --- | --- | --- |");
            for _ in &modes {
                token_section.push_str(" --- |");
            }
            token_section.push_str(" --- |\n");

            let default_mode_id = if !rc.col.default_mode.is_empty() { rc.col.default_mode.as_str() } else { modes[0].id.as_str() };

            for var in &rc.vars {
                let name_cell = if var.hidden_from_publishing {
                    format!("`{}` (not published)", escape_table_cell(&var.name))
                } else {
                    format!("`{}`", escape_table_cell(&var.name))
                };
                let kind_str = variable_kind_label(var.kind);
                let (code_token, code_cell) = format_code_syntax(var, opts.platform, opts.platform_label);

                token_section.push_str(&format!("| {} | {} | {} |", name_cell, escape_table_cell(&rc.origin), kind_str));
                for mode in &modes {
                    let val_cell = format_variable_mode_value(var, &mode.id, &all_vars);
                    token_section.push_str(&format!(" {} |", escape_table_cell(&val_cell)));
                }
                token_section.push_str(&format!(" {} |\n", code_cell));

                if matches!(var.kind, VariableKind::Color | VariableKind::Number) {
                    if let Some(lit) = resolve_literal_string(var, default_mode_id, &all_vars) {
                        usage_rules.push(format!("- Use `{}` instead of literal `{}` (source: {})", code_token, lit, rc.origin));
                    }
                }
            }
            token_section.push('\n');
        }

        if !orphan_vars.is_empty() {
            let origin = "This file";
            token_section.push_str(&format!("### Ungrouped ({origin})\n\n"));
            token_section.push_str(&format!("| Name | Origin | Type | Default | Code syntax ({}) |\n", opts.platform_label));
            token_section.push_str("| --- | --- | --- | --- | --- |\n");
            for var in &orphan_vars {
                let name_cell = if var.hidden_from_publishing {
                    format!("`{}` (not published)", escape_table_cell(&var.name))
                } else {
                    format!("`{}`", escape_table_cell(&var.name))
                };
                let mode_id = var.values.keys().next().map(String::as_str).unwrap_or("default");
                let val_cell = format_variable_mode_value(var, mode_id, &all_vars);
                let (code_token, code_cell) = format_code_syntax(var, opts.platform, opts.platform_label);
                token_section.push_str(&format!(
                    "| {} | {} | {} | {} | {} |\n",
                    name_cell,
                    origin,
                    variable_kind_label(var.kind),
                    escape_table_cell(&val_cell),
                    code_cell
                ));
                if matches!(var.kind, VariableKind::Color | VariableKind::Number) {
                    if let Some(lit) = resolve_literal_string(var, mode_id, &all_vars) {
                        usage_rules.push(format!("- Use `{}` instead of literal `{}` (source: {})", code_token, lit, origin));
                    }
                }
            }
            token_section.push('\n');
        }
    }

    // Section 2: Styles
    let mut styles_section = String::from("## Styles\n\n");
    let style_rows = collect_style_rows(doc, &mut sources_set);
    if style_rows.is_empty() {
        styles_section.push_str("No styles in this file\n");
    } else {
        styles_section.push_str("| Name | Origin | Kind | Value |\n");
        styles_section.push_str("| --- | --- | --- | --- |\n");
        for (name, origin, kind, val) in &style_rows {
            styles_section.push_str(&format!("| `{}` | {} | {} | {} |\n", escape_table_cell(name), escape_table_cell(origin), kind, escape_table_cell(val)));
        }
    }

    // Section 3: Components
    let mut components_section = String::from("## Components\n\n");
    let component_entries = collect_component_entries(doc, opts.scope, &mut sources_set);
    if component_entries.is_empty() {
        components_section.push_str("No components in this file\n");
    } else {
        for entry in &component_entries {
            components_section.push_str(entry);
            components_section.push('\n');
        }
    }

    // Section 4: Usage rules
    let mut usage_section = String::from("## Usage rules\n\n");
    if usage_rules.is_empty() {
        usage_section.push_str("- Define colour and spacing variables before introducing raw hex or pixel literals.\n");
    } else {
        for rule in &usage_rules {
            usage_section.push_str(rule);
            usage_section.push('\n');
        }
    }
    let spacing_list: Vec<String> = SPACING_SCALE_PX.iter().map(|v| format!("{v}px")).collect();
    let type_list: Vec<String> = TYPE_SCALE_PX.iter().map(|v| format!("{v}px")).collect();
    usage_section.push_str(&format!("- Spacing scale: {}\n", spacing_list.join(", ")));
    usage_section.push_str(&format!("- Type scale: {}\n", type_list.join(", ")));

    let header = format!(
        "# Design system rules — {}\n\n- **Scope:** {}\n- **Platform:** `{}`\n\n",
        doc.name,
        match opts.scope {
            RulesScope::File => "file".to_string(),
            RulesScope::Page(id) => format!("page:{}", id.0),
        },
        opts.platform_label
    );

    let sections = [
        ("Token sources", token_section.trim_end()),
        ("Styles", styles_section.trim_end()),
        ("Components", components_section.trim_end()),
        ("Usage rules", usage_section.trim_end()),
    ];

    let (content, truncated) = assemble_with_budget(&header, &sections, opts.max_tokens);
    let sources: Vec<String> = sources_set.into_iter().collect();

    Ok(GeneratedRules { content, truncated, sources })
}

fn format_library_origin(doc: &Document, lib_key: &str, version_hint: Option<u32>) -> String {
    if let Some(r) = doc.library.enabled.iter().find(|r| r.id == lib_key || r.name == lib_key) {
        return format!("Library: {} v{}", r.name, version_hint.unwrap_or(r.version));
    }
    if let Some(ext) = doc.library.external.get(lib_key).or_else(|| doc.library.external.values().find(|e| e.library_id == lib_key)) {
        let name = doc.library.enabled.iter().find(|r| r.id == ext.library_id).map(|r| r.name.as_str()).unwrap_or(ext.library_id.as_str());
        return format!("Library: {name} v{}", version_hint.unwrap_or(ext.version));
    }
    if let Some(stripped) = lib_key.strip_prefix("Library:") {
        let trimmed = stripped.trim();
        if has_version_suffix(trimmed) {
            return format!("Library: {trimmed}");
        }
        return format!("Library: {trimmed} v{}", version_hint.unwrap_or(1));
    }
    if has_version_suffix(lib_key) {
        return format!("Library: {lib_key}");
    }
    format!("Library: {lib_key} v{}", version_hint.unwrap_or(1))
}

fn collection_origin(doc: &Document, col: &VariableCollection) -> String {
    if let Some(lib_key) = col.source_library_key.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        return format_library_origin(doc, lib_key, None);
    }
    for vid in &col.variables {
        if let Some(ext) = doc.library.external.values().find(|e| e.local_key.as_deref() == Some(vid.as_str())) {
            return format_library_origin(doc, &ext.library_id, Some(ext.version));
        }
    }
    "This file".to_string()
}

fn has_version_suffix(s: &str) -> bool {
    if let Some((_, right)) = s.rsplit_once(" v") {
        !right.is_empty() && right.chars().all(|c| c.is_ascii_digit() || c == '.')
    } else {
        false
    }
}

fn collect_collection_variables<'a>(col: &VariableCollection, vars_map: &'a BTreeMap<String, Variable>, scope: RulesScope) -> Vec<&'a Variable> {
    let mut out = Vec::new();
    let mut seen = BTreeSet::new();
    for var_id in &col.variables {
        if let Some(v) = vars_map.get(var_id) {
            if v.collection == col.id {
                seen.insert(v.id.clone());
                if !(matches!(scope, RulesScope::Page(_)) && v.hidden_from_publishing) {
                    out.push(v);
                }
            }
        }
    }
    for v in vars_map.values() {
        if v.collection == col.id && seen.insert(v.id.clone()) && !(matches!(scope, RulesScope::Page(_)) && v.hidden_from_publishing) {
            out.push(v);
        }
    }
    out
}

fn variable_kind_label(kind: VariableKind) -> &'static str {
    match kind {
        VariableKind::Color => "color",
        VariableKind::Number => "number",
        VariableKind::String => "string",
        VariableKind::Boolean => "boolean",
    }
}

fn format_variable_mode_value(var: &Variable, mode_id: &str, all_vars: &BTreeMap<String, &Variable>) -> String {
    let val = var.values.get(mode_id).or_else(|| var.values.values().next());
    let Some(val) = val else {
        return "—".to_string();
    };
    match val {
        VariableValue::Alias(target_id) => {
            let target_name = all_vars.get(target_id.as_str()).map(|t| t.name.as_str()).unwrap_or(target_id.as_str());
            let resolved = resolve_alias_value(target_id, mode_id, all_vars).unwrap_or_else(|| "unresolved".to_string());
            format!("→ {target_name} ({resolved})")
        }
        other => format_literal_value(other),
    }
}

fn resolve_literal_string(var: &Variable, mode_id: &str, all_vars: &BTreeMap<String, &Variable>) -> Option<String> {
    let val = var.values.get(mode_id).or_else(|| var.values.values().next())?;
    match val {
        VariableValue::Alias(target_id) => resolve_alias_value(target_id, mode_id, all_vars),
        other => Some(format_literal_value(other)),
    }
}

fn resolve_alias_value(start_id: &str, preferred_mode_id: &str, all_vars: &BTreeMap<String, &Variable>) -> Option<String> {
    let mut cur_id = start_id;
    let mut visited = BTreeSet::new();
    for _ in 0..16 {
        if !visited.insert(cur_id) {
            return None;
        }
        let target = all_vars.get(cur_id)?;
        let val = target.values.get(preferred_mode_id).or_else(|| target.values.values().next())?;
        match val {
            VariableValue::Alias(next_id) => cur_id = next_id.as_str(),
            other => return Some(format_literal_value(other)),
        }
    }
    None
}

fn format_literal_value(val: &VariableValue) -> String {
    match val {
        VariableValue::Color(c) => format_color(c),
        VariableValue::Number(n) => format_number(*n),
        VariableValue::String(s) => format!("\"{s}\""),
        VariableValue::Boolean(b) => b.to_string(),
        VariableValue::Alias(id) => format!("→ {id}"),
    }
}

fn format_color(c: &Color) -> String {
    let [r, g, b, a] = c.to_u8();
    if a == 255 {
        format!("#{r:02X}{g:02X}{b:02X}")
    } else {
        format!("#{r:02X}{g:02X}{b:02X}{a:02X}")
    }
}

fn format_number(n: f32) -> String {
    if (n - n.round()).abs() < 1e-4 {
        format!("{}", n.round() as i64)
    } else {
        format!("{:.2}", n).trim_end_matches('0').trim_end_matches('.').to_string()
    }
}

/// Returns `(raw_code_token, formatted_markdown_cell)`.
fn format_code_syntax(var: &Variable, platform: CodeSyntaxPlatform, platform_label: &str) -> (String, String) {
    if let Some(syntax) = var.code_syntax.get(&platform).map(String::as_str).map(str::trim).filter(|s| !s.is_empty()) {
        return (syntax.to_string(), format!("`{}`", escape_table_cell(syntax)));
    }
    let fallback = sanitize_token_name(&var.name, platform_label);
    let cell = format!("`{}` (sanitized fallback)", escape_table_cell(&fallback));
    (fallback, cell)
}

pub fn sanitize_token_name(name: &str, platform_label: &str) -> String {
    let words: Vec<String> = name.split(|c: char| !c.is_ascii_alphanumeric()).filter(|s| !s.is_empty()).map(|s| s.to_ascii_lowercase()).collect();
    if words.is_empty() {
        return match platform_label {
            "ios" | "android" => "token".to_string(),
            _ => "--token".to_string(),
        };
    }
    match platform_label {
        "ios" => {
            let mut out = words[0].clone();
            for w in &words[1..] {
                let mut chars = w.chars();
                if let Some(first) = chars.next() {
                    out.push(first.to_ascii_uppercase());
                    out.push_str(chars.as_str());
                }
            }
            out
        }
        "android" => words.join("_"),
        _ => format!("--{}", words.join("-")),
    }
}

fn collect_style_rows(doc: &Document, sources_set: &mut BTreeSet<String>) -> Vec<(String, String, String, String)> {
    let mut rows = Vec::new();
    for (key, style) in &doc.styles {
        let name = doc.style_name(key).to_string();
        let meta = doc.style_meta.get(key);
        let origin = style_origin(doc, key, meta.and_then(|m| m.key.as_deref()));
        sources_set.insert(origin.clone());
        let prop = designos_doc::style_property(doc, key);
        let kind = format!("{prop:?}");
        let val = format_style_value(style);
        rows.push((name, origin, kind, val));
    }
    rows
}

fn style_origin(doc: &Document, style_key: &str, library_key: Option<&str>) -> String {
    if let Some(lk) = library_key.map(str::trim).filter(|s| !s.is_empty()) {
        return format_library_origin(doc, lk, None);
    }
    if let Some(ext) = doc.library.external.values().find(|e| e.local_key.as_deref() == Some(style_key)) {
        return format_library_origin(doc, &ext.library_id, Some(ext.version));
    }
    "This file".to_string()
}

fn format_style_value(style: &Style) -> String {
    match style {
        Style::Fill(paints) => format_paints(paints),
        Style::Text(ts) => format_text_style(ts),
        Style::Effect(effects) => format_effects(effects),
        Style::Grid(grids) => format_grids(grids),
    }
}

fn format_paints(paints: &[Paint]) -> String {
    let visible: Vec<String> = paints
        .iter()
        .filter(|p| p.visible)
        .map(|p| match &p.kind {
            PaintKind::Solid(c) => format_color(c),
            PaintKind::LinearGradient { stops, .. } => {
                format!("linear-gradient({} stops)", stops.len())
            }
            PaintKind::RadialGradient { stops, .. } => {
                format!("radial-gradient({} stops)", stops.len())
            }
            PaintKind::AngularGradient { stops, .. } => {
                format!("angular-gradient({} stops)", stops.len())
            }
            PaintKind::DiamondGradient { stops, .. } => {
                format!("diamond-gradient({} stops)", stops.len())
            }
            PaintKind::Image { image_ref, .. } => format!("image({image_ref})"),
        })
        .collect();
    if visible.is_empty() {
        "none".to_string()
    } else {
        visible.join(", ")
    }
}

fn format_text_style(ts: &TextStyle) -> String {
    let lh = match ts.line_height {
        LineHeight::Auto => "auto".to_string(),
        LineHeight::Pixels(px) => format!("{}px", format_number(px)),
        LineHeight::Percent(pct) => format!("{}%", format_number(pct)),
    };
    let italic = if ts.italic { " italic" } else { "" };
    format!(
        "{} {}px/{} (weight {}{}, tracking {}px)",
        ts.font_family,
        format_number(ts.font_size),
        lh,
        ts.font_weight,
        italic,
        format_number(ts.letter_spacing)
    )
}

fn format_effects(effects: &[Effect]) -> String {
    let list: Vec<String> = effects
        .iter()
        .map(|e| match e {
            Effect::DropShadow { color, offset, radius, spread, .. } => format!(
                "drop-shadow({}px {}px blur {}px spread {}px {})",
                format_number(offset.x),
                format_number(offset.y),
                format_number(*radius),
                format_number(*spread),
                format_color(color)
            ),
            Effect::InnerShadow { color, offset, radius, spread, .. } => format!(
                "inner-shadow({}px {}px blur {}px spread {}px {})",
                format_number(offset.x),
                format_number(offset.y),
                format_number(*radius),
                format_number(*spread),
                format_color(color)
            ),
            Effect::LayerBlur { radius, .. } => format!("layer-blur({}px)", format_number(*radius)),
            Effect::BackgroundBlur { radius, .. } => {
                format!("background-blur({}px)", format_number(*radius))
            }
            Effect::Noise(n) => format!("noise(density {})", format_number(n.density)),
            Effect::Texture(t) => format!("texture(radius {}px)", format_number(t.radius)),
            Effect::Glass(g) => format!("glass(refraction {})", format_number(g.refraction)),
        })
        .collect();
    if list.is_empty() {
        "none".to_string()
    } else {
        list.join(", ")
    }
}

fn format_grids(grids: &[LayoutGrid]) -> String {
    let list: Vec<String> = grids
        .iter()
        .map(|g| match g {
            LayoutGrid::Columns { count, gutter, .. } => {
                format!("{count} columns (gutter {}px)", format_number(*gutter))
            }
            LayoutGrid::Rows { count, gutter, .. } => {
                format!("{count} rows (gutter {}px)", format_number(*gutter))
            }
            LayoutGrid::Grid { size, .. } => format!("grid({}px)", format_number(*size)),
        })
        .collect();
    if list.is_empty() {
        "none".to_string()
    } else {
        list.join(", ")
    }
}

fn collect_component_entries(doc: &Document, scope: RulesScope, sources_set: &mut BTreeSet<String>) -> Vec<String> {
    let mut entries = Vec::new();
    for node in doc.nodes.values() {
        let is_set = matches!(node.kind, NodeKind::ComponentSet(_));
        let is_standalone_comp = matches!(node.kind, NodeKind::Component(_))
            && node.parent.and_then(|pid| doc.get(pid)).is_none_or(|p| !matches!(p.kind, NodeKind::ComponentSet(_)));
        if !is_set && !is_standalone_comp {
            continue;
        }
        if let RulesScope::Page(page_id) = scope {
            if doc.page_of(node.id) != Some(page_id) {
                continue;
            }
        }
        let origin = component_origin(doc, node);
        sources_set.insert(origin.clone());

        let kind_label = if is_set { "ComponentSet" } else { "Component" };
        let mut block = format!("### `{}` ({kind_label})\n\n", node.name);
        block.push_str(&format!("- **Origin:** {origin}\n"));
        if !node.component.description.trim().is_empty() {
            block.push_str(&format!("- **Description:** {}\n", node.component.description.trim()));
        }

        if is_set {
            let variants: Vec<&str> =
                node.children.iter().filter_map(|cid| doc.get(*cid)).filter(|c| matches!(c.kind, NodeKind::Component(_))).map(|c| c.name.as_str()).collect();
            if !variants.is_empty() {
                block.push_str(&format!("- **Variants:** {}\n", variants.join(", ")));
            }
        }

        if !node.component.props.is_empty() {
            block.push_str("- **Properties:**\n");
            for prop in &node.component.props {
                match &prop.kind {
                    PropKind::Variant { options } => {
                        let def = match &prop.default {
                            PropValue::Variant(s) => s.as_str(),
                            _ => "",
                        };
                        block.push_str(&format!("  - `{}` (variant): options [{}], default `{}`\n", prop.name, options.join(", "), def));
                    }
                    PropKind::Boolean => {
                        let def = matches!(prop.default, PropValue::Bool(true));
                        block.push_str(&format!("  - `{}` (boolean): default `{def}`\n", prop.name));
                    }
                    PropKind::Text => {
                        let def = match &prop.default {
                            PropValue::Text(s) => s.as_str(),
                            _ => "",
                        };
                        block.push_str(&format!("  - `{}` (text): default `\"{}\"`\n", prop.name, def));
                    }
                    PropKind::InstanceSwap { preferred } => {
                        let names = resolve_node_names(doc, preferred);
                        block.push_str(&format!("  - `{}` (instance swap): preferred [{}]\n", prop.name, names.join(", ")));
                    }
                    PropKind::Slot => {
                        let preferred = node.component.slot_configs.get(&prop.name).map(|cfg| resolve_node_names(doc, &cfg.preferred)).unwrap_or_default();
                        let rec = if preferred.is_empty() { "any".to_string() } else { preferred.join(", ") };
                        block.push_str(&format!("  - `{}` (slot): recommended components [{rec}]\n", prop.name));
                    }
                }
            }
        }

        // Also surface any `slot_configs` not already listed in `props`.
        for (slot_name, cfg) in &node.component.slot_configs {
            if !node.component.props.iter().any(|p| &p.name == slot_name) {
                let names = resolve_node_names(doc, &cfg.preferred);
                let rec = if names.is_empty() { "any".to_string() } else { names.join(", ") };
                block.push_str(&format!("- **Slot `{slot_name}`:** recommended components [{rec}]\n"));
            }
        }

        entries.push(block);
    }
    entries
}

fn component_origin(doc: &Document, node: &Node) -> String {
    if let Some(ext) = doc.library.external.values().find(|e| e.node == Some(node.id)) {
        return format_library_origin(doc, &ext.library_id, Some(ext.version));
    }
    if doc.library_page.is_some() && doc.page_of(node.id) == doc.library_page {
        if let Some(r) = doc.library.enabled.first() {
            return format!("Library: {} v{}", r.name, r.version);
        }
    }
    "This file".to_string()
}

fn resolve_node_names(doc: &Document, ids: &[NodeId]) -> Vec<String> {
    ids.iter().map(|id| doc.get(*id).map(|n| format!("`{}`", n.name)).unwrap_or_else(|| format!("#{}", id.0))).collect()
}

fn escape_table_cell(s: &str) -> String {
    s.replace('|', "\\|").replace('\n', " ")
}

/// Estimates token count using the standard 4-chars-per-token heuristic.
pub fn estimate_tokens(text: &str) -> usize {
    text.chars().count().div_ceil(4)
}

fn assemble_with_budget(header: &str, sections: &[(&str, &str)], max_tokens: usize) -> (String, bool) {
    let mut full = String::from(header);
    for (idx, (_, body)) in sections.iter().enumerate() {
        if idx > 0 {
            full.push_str("\n\n");
        }
        full.push_str(body);
    }
    full.push('\n');

    let total_tokens = estimate_tokens(&full);
    if total_tokens <= max_tokens {
        return (full, false);
    }

    // Reserve ~16 tokens (64 chars) for the truncation footer line.
    let budget_chars = max_tokens.saturating_mul(4).saturating_sub(64);
    let mut out = String::from(header);
    let mut truncated_section = sections.last().map(|(name, _)| *name).unwrap_or("Usage rules");

    for (idx, (section_name, body)) in sections.iter().enumerate() {
        let sep = if idx > 0 { "\n\n" } else { "" };
        if out.chars().count() + sep.len() + body.chars().count() <= budget_chars {
            out.push_str(sep);
            out.push_str(body);
        } else {
            truncated_section = section_name;
            out.push_str(sep);
            for line in body.lines() {
                if out.chars().count() + line.chars().count() + 1 > budget_chars {
                    break;
                }
                out.push_str(line);
                out.push('\n');
            }
            break;
        }
    }

    let kept_tokens = estimate_tokens(&out);
    let omitted_tokens = total_tokens.saturating_sub(kept_tokens).max(1);
    let trimmed = out.trim_end();
    let final_doc = format!("{trimmed}\n… truncated: {omitted_tokens} more tokens in {truncated_section}");
    (final_doc, true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use designos_doc::{ComponentPropDef, FrameProps, LibraryRef, Rect, SlotPropConfig, StyleMeta, StyleProperty, Vec2};

    fn build_tokens_fixture() -> Document {
        let mut doc = Document::new("Acme Design System");
        let page = doc.add_page("Foundations");

        // Collection 1: Local collection ("Primitives")
        doc.variable_collections.insert(
            "col_local".into(),
            VariableCollection {
                id: "col_local".into(),
                name: "Primitives".into(),
                modes: vec![VariableMode { id: "light".into(), name: "Light".into() }, VariableMode { id: "dark".into(), name: "Dark".into() }],
                default_mode: "light".into(),
                variables: vec!["v_blue".into(), "v_space_md".into(), "v_secret".into()],
                key: None,
                source_library_key: None,
            },
        );

        let mut blue_syntax = BTreeMap::new();
        blue_syntax.insert(CodeSyntaxPlatform::Web, "var(--color-blue-500)".into());
        blue_syntax.insert(CodeSyntaxPlatform::Ios, "Color.blue500".into());
        doc.variables.insert(
            "v_blue".into(),
            Variable {
                id: "v_blue".into(),
                name: "color/blue/500".into(),
                collection: "col_local".into(),
                kind: VariableKind::Color,
                values: [
                    ("light".into(), VariableValue::Color(Color::rgb(0x1E, 0x88, 0xE5))),
                    ("dark".into(), VariableValue::Color(Color::rgb(0x64, 0xB5, 0xF6))),
                ]
                .into_iter()
                .collect(),
                description: "Core brand blue".into(),
                scopes: vec![],
                hidden_from_publishing: false,
                key: None,
                code_syntax: blue_syntax,
            },
        );

        // No explicit code_syntax -> tests sanitized fallback!
        doc.variables.insert(
            "v_space_md".into(),
            Variable {
                id: "v_space_md".into(),
                name: "space/md".into(),
                collection: "col_local".into(),
                kind: VariableKind::Number,
                values: [("light".into(), VariableValue::Number(16.0)), ("dark".into(), VariableValue::Number(16.0))].into_iter().collect(),
                description: "Medium spacing".into(),
                scopes: vec![],
                hidden_from_publishing: false,
                key: None,
                code_syntax: BTreeMap::new(),
            },
        );

        // Hidden-from-publishing variable
        doc.variables.insert(
            "v_secret".into(),
            Variable {
                id: "v_secret".into(),
                name: "internal/debug-grid".into(),
                collection: "col_local".into(),
                kind: VariableKind::Number,
                values: [("light".into(), VariableValue::Number(8.0))].into_iter().collect(),
                description: String::new(),
                scopes: vec![],
                hidden_from_publishing: true,
                key: None,
                code_syntax: BTreeMap::new(),
            },
        );

        // Collection 2: Subscribed library collection ("Semantic") linked to LibraryRef
        doc.library.enabled.push(LibraryRef { id: "lib_brand".into(), name: "Brand UI".into(), version: 3 });
        doc.variable_collections.insert(
            "col_lib".into(),
            VariableCollection {
                id: "col_lib".into(),
                name: "Semantic".into(),
                modes: vec![VariableMode { id: "light".into(), name: "Light".into() }],
                default_mode: "light".into(),
                variables: vec!["v_primary".into()],
                key: Some("col_lib_key".into()),
                source_library_key: Some("lib_brand".into()),
            },
        );

        let mut primary_syntax = BTreeMap::new();
        primary_syntax.insert(CodeSyntaxPlatform::Web, "var(--semantic-primary)".into());
        doc.variables.insert(
            "v_primary".into(),
            Variable {
                id: "v_primary".into(),
                name: "semantic/primary".into(),
                collection: "col_lib".into(),
                kind: VariableKind::Color,
                values: [("light".into(), VariableValue::Alias("v_blue".into()))].into_iter().collect(),
                description: "Primary interactive colour".into(),
                scopes: vec![],
                hidden_from_publishing: false,
                key: None,
                code_syntax: primary_syntax,
            },
        );

        // 3 Styles: Fill, Text, Effect
        doc.styles.insert("Fill/Surface".into(), Style::Fill(vec![Paint::solid(Color::rgb(0xFF, 0xFF, 0xFF))]));
        doc.style_meta
            .insert("Fill/Surface".into(), StyleMeta { name: "Fill/Surface".into(), key: None, description: String::new(), property: StyleProperty::Fill });

        doc.styles
            .insert("Text/Heading".into(), Style::Text(TextStyle { font_family: "Inter".into(), font_weight: 700, font_size: 24.0, ..TextStyle::default() }));
        doc.style_meta
            .insert("Text/Heading".into(), StyleMeta { name: "Text/Heading".into(), key: None, description: String::new(), property: StyleProperty::Text });

        doc.styles.insert(
            "Effect/CardShadow".into(),
            Style::Effect(vec![Effect::DropShadow {
                color: Color::rgba(0, 0, 0, 64),
                offset: Vec2::new(0.0, 4.0),
                radius: 12.0,
                spread: 0.0,
                visible: true,
                show_behind: false,
            }]),
        );
        doc.style_meta.insert(
            "Effect/CardShadow".into(),
            StyleMeta { name: "Effect/CardShadow".into(), key: Some("lib_brand".into()), description: String::new(), property: StyleProperty::Effect },
        );

        // 1 ComponentSet ("Button") with a variant child and a slot recommending an Icon component
        let icon_id = doc.alloc_id();
        let mut icon = Node::new(icon_id, NodeKind::Component(FrameProps::default()));
        icon.name = "Icon/Check".into();
        doc.insert(page, icon).unwrap();

        let set_id = doc.alloc_id();
        let mut set = Node::new(set_id, NodeKind::ComponentSet(FrameProps::default()));
        set.name = "Button".into();
        set.component.description = "Primary action button".into();
        set.component.props.push(ComponentPropDef {
            name: "Size".into(),
            kind: PropKind::Variant { options: vec!["sm".into(), "md".into(), "lg".into()] },
            default: PropValue::Variant("md".into()),
        });
        set.component.props.push(ComponentPropDef { name: "LeadingSlot".into(), kind: PropKind::Slot, default: PropValue::Slot(vec![]) });
        set.component.slot_configs.insert("LeadingSlot".into(), SlotPropConfig { preferred: vec![icon_id], ..SlotPropConfig::default() });
        doc.insert(page, set).unwrap();

        let var_id = doc.alloc_id();
        let mut var_comp = Node::new(var_id, NodeKind::Component(FrameProps::default()));
        var_comp.name = "Size=md".into();
        var_comp.size = Vec2::new(120.0, 40.0);
        let _ = Rect::new(0.0, 0.0, 120.0, 40.0);
        doc.insert(set_id, var_comp).unwrap();

        doc
    }

    #[test]
    fn rules_from_fixture() {
        let doc = build_tokens_fixture();
        let out = generate_design_system_rules(&doc, &RulesOptions::default()).unwrap();
        assert!(!out.truncated);

        // Every variable name with its code syntax (or sanitized fallback)
        assert!(out.content.contains("`color/blue/500`"), "{}", out.content);
        assert!(out.content.contains("`var(--color-blue-500)`"), "{}", out.content);
        assert!(out.content.contains("`space/md`"), "{}", out.content);
        assert!(out.content.contains("`--space-md` (sanitized fallback)"), "{}", out.content);
        assert!(out.content.contains("`internal/debug-grid` (not published)"), "{}", out.content);
        assert!(out.content.contains("`semantic/primary`"), "{}", out.content);
        assert!(out.content.contains("`var(--semantic-primary)`"), "{}", out.content);
        // Alias printed as `→ <target name> (<resolved value>)`
        assert!(out.content.contains("→ color/blue/500 (#1E88E5)"), "{}", out.content);

        // All 3 styles
        assert!(out.content.contains("`Fill/Surface`"), "{}", out.content);
        assert!(out.content.contains("`Text/Heading`"), "{}", out.content);
        assert!(out.content.contains("`Effect/CardShadow`"), "{}", out.content);

        // Component set and slot recommendation
        assert!(out.content.contains("### `Button` (ComponentSet)"), "{}", out.content);
        assert!(out.content.contains("options [sm, md, lg]"), "{}", out.content);
        assert!(out.content.contains("`LeadingSlot` (slot): recommended components [`Icon/Check`]"), "{}", out.content);
    }

    #[test]
    fn every_token_names_its_source() {
        let mut doc = build_tokens_fixture();
        // Add a library collection with the SAME name ("Primitives") as the local collection (§8.2).
        doc.variable_collections.insert(
            "col_lib_same_name".into(),
            VariableCollection {
                id: "col_lib_same_name".into(),
                name: "Primitives".into(),
                modes: vec![VariableMode { id: "light".into(), name: "Light".into() }],
                default_mode: "light".into(),
                variables: vec!["v_lib_prim".into()],
                key: None,
                source_library_key: Some("lib_brand".into()),
            },
        );
        doc.variables.insert(
            "v_lib_prim".into(),
            Variable {
                id: "v_lib_prim".into(),
                name: "color/neutral/900".into(),
                collection: "col_lib_same_name".into(),
                kind: VariableKind::Color,
                values: [("light".into(), VariableValue::Color(Color::rgb(0x11, 0x18, 0x27)))].into_iter().collect(),
                description: String::new(),
                scopes: vec![],
                hidden_from_publishing: false,
                key: None,
                code_syntax: BTreeMap::new(),
            },
        );

        let out = generate_design_system_rules(&doc, &RulesOptions::default()).unwrap();
        assert!(out.sources.contains(&"This file".to_string()), "{:?}", out.sources);
        assert!(out.sources.contains(&"Library: Brand UI v3".to_string()), "{:?}", out.sources);
        // Same-named local and library collections get separate sections, each naming its origin.
        assert!(out.content.contains("### Primitives (This file)"), "{}", out.content);
        assert!(out.content.contains("### Primitives (Library: Brand UI v3)"), "{}", out.content);

        // Every token row in every token table has a non-empty Origin column value.
        let token_part = out.content.split("## Styles").next().expect("Token sources section before Styles");
        let mut checked_rows = 0usize;
        let mut saw_library_row = false;
        for line in token_part.lines() {
            if line.starts_with("| `") {
                let cols: Vec<&str> = line.split('|').map(str::trim).collect();
                // cols[0] is "", cols[1] is Name, cols[2] is Origin
                assert!(cols.len() >= 4, "malformed token row: {line}");
                let origin = cols[2];
                assert!(origin == "This file" || origin.starts_with("Library: "), "unexpected origin '{origin}' in row: {line}");
                if origin == "Library: Brand UI v3" {
                    saw_library_row = true;
                }
                checked_rows += 1;
            }
        }
        assert!(checked_rows >= 5, "expected >= 5 token rows, got {checked_rows}");
        assert!(saw_library_row, "expected library row with 'Library: Brand UI v3'");
    }

    #[test]
    fn truncation_is_marked() {
        let mut doc = build_tokens_fixture();
        for i in 0..60 {
            let id = format!("v_extra_{i:02}");
            doc.variable_collections.get_mut("col_local").unwrap().variables.push(id.clone());
            doc.variables.insert(
                id.clone(),
                Variable {
                    id,
                    name: format!("color/palette/swatch-{i:02}"),
                    collection: "col_local".into(),
                    kind: VariableKind::Color,
                    values: [
                        ("light".into(), VariableValue::Color(Color::rgb((i * 3) as u8, 100, 200))),
                        ("dark".into(), VariableValue::Color(Color::rgb((i * 2) as u8, 50, 150))),
                    ]
                    .into_iter()
                    .collect(),
                    description: String::new(),
                    scopes: vec![],
                    hidden_from_publishing: false,
                    key: None,
                    code_syntax: BTreeMap::new(),
                },
            );
        }

        let opts = RulesOptions::parse(Some("file"), Some("web"), Some(500)).unwrap();
        let out = generate_design_system_rules(&doc, &opts).unwrap();
        assert!(out.truncated);
        let last_line = out.content.lines().last().unwrap();
        assert!(last_line.starts_with("… truncated:") && last_line.contains("more tokens in"), "unexpected last line: {last_line}");
    }

    #[test]
    fn empty_document_reports_empty_sections() {
        let mut doc = Document::new("Empty");
        doc.add_page("Page 1");
        let out = generate_design_system_rules(&doc, &RulesOptions::default()).unwrap();
        assert!(out.content.contains("No variables in this file"));
        assert!(out.content.contains("No styles in this file"));
        assert!(out.content.contains("No components in this file"));
    }
}
