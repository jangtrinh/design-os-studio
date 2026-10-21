//! Streaming document query visitor (`designos_query_xpath`) with paging,
//! node budget, compact rows/objects, and solver-backed linter predicates (E15-17 / #602).

use crate::lint::{canonical_rule_id, resolve_rule, Finding, LintConfig, LintEnv, LintPreset, NoLintEnv, RuleNeeds, SingleNodeLinter};
use crate::openpencil::{
    eval_predicate_expr, is_exact_node_type_token, is_node_kind_word, is_node_type_test, layout_mode_str, matches_node_test, parse_xpath, xpath_type_name,
    XPathAxis, XPathStep,
};
use crate::{Document, Node, NodeId, NodeKind};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};

pub const DEFAULT_LIMIT: usize = 100;
pub const MAX_LIMIT: usize = 500;
pub const DEFAULT_MAX_SCANNED: usize = 50_000;
pub const MIN_MAX_SCANNED: usize = 1_000;
pub const MAX_MAX_SCANNED: usize = 200_000;

pub const ROW_COLUMNS: [&str; 9] = ["id", "name", "type", "x", "y", "w", "h", "problems", "in_progress"];

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QueryFormat {
    #[default]
    Rows,
    Objects,
}

impl QueryFormat {
    pub fn parse(raw: Option<&str>) -> Result<Self, QueryError> {
        match raw.unwrap_or("rows").trim().to_ascii_lowercase().as_str() {
            "" | "rows" | "compact" => Ok(Self::Rows),
            "objects" | "full" | "summary" => Ok(Self::Objects),
            other => Err(QueryError::InvalidSelector {
                message: format!("unknown format '{other}'; expected 'rows' or 'objects'"),
                corrected_example: "//FRAME[@name='Card']".to_string(),
            }),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Projection {
    #[default]
    Compact,
    Summary,
    Full,
}

#[derive(Clone, Debug, Default)]
pub struct QueryOptions {
    pub selector: String,
    pub page: Option<NodeId>,
    pub within: Option<NodeId>,
    pub limit: Option<usize>,
    pub max_scanned: Option<usize>,
    pub cursor: Option<String>,
    pub format: QueryFormat,
    pub with_problems: bool,
    pub hardcoded: Option<bool>,
    pub projection: Projection,
    pub fields: Option<Vec<String>>,
    pub max_depth: Option<u32>,
    pub node_budget: Option<u64>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct QueryPage {
    pub columns: Vec<String>,
    pub rows: Vec<Value>,
    pub matches: Vec<Value>,
    pub count: usize,
    pub returned: usize,
    pub scanned: usize,
    pub scanned_total: usize,
    pub scope_size: usize,
    pub remaining: usize,
    pub done: bool,
    pub complete: bool,
    pub truncated: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cursor: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_cursor: Option<String>,
    pub document_epoch: u64,
    pub elapsed_ms: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum QueryError {
    InvalidSelector { message: String, corrected_example: String },
    InvalidCursor { message: String },
    CursorStale { cursor_epoch: u64, document_epoch: u64 },
    UnknownRule { rule: String },
    RuleUnavailable { rule: String, reason: String },
    NodeNotFound { id: NodeId },
    ExactlyOneScope,
}

impl QueryError {
    pub fn code(&self) -> &'static str {
        match self {
            QueryError::InvalidSelector { .. } => "INVALID_SELECTOR",
            QueryError::InvalidCursor { .. } => "INVALID_CURSOR",
            QueryError::CursorStale { .. } => "CURSOR_STALE",
            QueryError::UnknownRule { .. } => "UNKNOWN_RULE",
            QueryError::RuleUnavailable { .. } => "RULE_UNAVAILABLE",
            QueryError::NodeNotFound { .. } => "NODE_NOT_FOUND",
            QueryError::ExactlyOneScope => "EXACTLY_ONE_SCOPE",
        }
    }
}

impl std::fmt::Display for QueryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            QueryError::InvalidSelector {
                message,
                corrected_example,
            } => write!(
                f,
                "INVALID_SELECTOR: {message} (path: /selector, corrected_example: {corrected_example}, safe_to_retry: true)"
            ),
            QueryError::InvalidCursor { message } => {
                write!(f, "INVALID_CURSOR: {message} (safe_to_retry: true)")
            }
            QueryError::CursorStale {
                cursor_epoch,
                document_epoch,
            } => write!(
                f,
                "CURSOR_STALE: cursor revision {cursor_epoch} does not match document revision {document_epoch}; restart query without cursor (safe_to_retry: true)"
            ),
            QueryError::UnknownRule { rule } => write!(
                f,
                "UNKNOWN_RULE: unknown lint rule '{rule}' in has-problem() predicate (safe_to_retry: true)"
            ),
            QueryError::RuleUnavailable { rule, reason } => write!(
                f,
                "RULE_UNAVAILABLE: lint rule '{rule}' cannot be evaluated ({reason})"
            ),
            QueryError::NodeNotFound { id } => write!(
                f,
                "NODE_NOT_FOUND: scope node {} does not exist (safe_to_retry: true)",
                id.0
            ),
            QueryError::ExactlyOneScope => write!(
                f,
                "EXACTLY_ONE_SCOPE: provide at most one of page_id or node_id (safe_to_retry: true)"
            ),
        }
    }
}

impl std::error::Error for QueryError {}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
struct CursorToken {
    v: u8,
    doc_revision: u64,
    scope_root: Option<u64>,
    expr_hash: u64,
    scope_size: usize,
    scanned_total: usize,
    stack: Vec<(u64, usize, u64)>,
}

fn fnv1a64(bytes: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in bytes {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

const B64_URL: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";

fn b64url_encode(input: &[u8]) -> String {
    let mut out = String::with_capacity((input.len() * 4).div_ceil(3));
    let mut i = 0;
    while i + 3 <= input.len() {
        let n = ((input[i] as u32) << 16) | ((input[i + 1] as u32) << 8) | (input[i + 2] as u32);
        out.push(B64_URL[((n >> 18) & 0x3f) as usize] as char);
        out.push(B64_URL[((n >> 12) & 0x3f) as usize] as char);
        out.push(B64_URL[((n >> 6) & 0x3f) as usize] as char);
        out.push(B64_URL[(n & 0x3f) as usize] as char);
        i += 3;
    }
    let rem = input.len() - i;
    if rem == 1 {
        let n = (input[i] as u32) << 16;
        out.push(B64_URL[((n >> 18) & 0x3f) as usize] as char);
        out.push(B64_URL[((n >> 12) & 0x3f) as usize] as char);
    } else if rem == 2 {
        let n = ((input[i] as u32) << 16) | ((input[i + 1] as u32) << 8);
        out.push(B64_URL[((n >> 18) & 0x3f) as usize] as char);
        out.push(B64_URL[((n >> 12) & 0x3f) as usize] as char);
        out.push(B64_URL[((n >> 6) & 0x3f) as usize] as char);
    }
    out
}

fn b64url_decode(raw: &str) -> Result<Vec<u8>, String> {
    let s = raw.trim().trim_end_matches('=');
    let mut vals = Vec::with_capacity(s.len());
    for b in s.bytes() {
        let v = match b {
            b'A'..=b'Z' => b - b'A',
            b'a'..=b'z' => b - b'a' + 26,
            b'0'..=b'9' => b - b'0' + 52,
            b'-' | b'+' => 62,
            b'_' | b'/' => 63,
            _ => return Err(format!("invalid base64 byte {b:#x}")),
        };
        vals.push(v);
    }
    if vals.len() % 4 == 1 {
        return Err("invalid base64 length".to_string());
    }
    let mut out = Vec::with_capacity(vals.len() * 3 / 4);
    let mut i = 0;
    while i + 4 <= vals.len() {
        let n = ((vals[i] as u32) << 18) | ((vals[i + 1] as u32) << 12) | ((vals[i + 2] as u32) << 6) | (vals[i + 3] as u32);
        out.push(((n >> 16) & 0xff) as u8);
        out.push(((n >> 8) & 0xff) as u8);
        out.push((n & 0xff) as u8);
        i += 4;
    }
    let rem = vals.len() - i;
    if rem == 2 {
        let n = ((vals[i] as u32) << 18) | ((vals[i + 1] as u32) << 12);
        out.push(((n >> 16) & 0xff) as u8);
    } else if rem == 3 {
        let n = ((vals[i] as u32) << 18) | ((vals[i + 1] as u32) << 12) | ((vals[i + 2] as u32) << 6);
        out.push(((n >> 16) & 0xff) as u8);
        out.push(((n >> 8) & 0xff) as u8);
    }
    Ok(out)
}

fn encode_cursor(token: &CursorToken) -> String {
    let json_bytes = serde_json::to_vec(token).unwrap_or_default();
    b64url_encode(&json_bytes)
}

fn decode_cursor(raw: &str) -> Result<CursorToken, QueryError> {
    let bytes = b64url_decode(raw).map_err(|e| QueryError::InvalidCursor { message: format!("base64 decode failed: {e}") })?;
    let tok: CursorToken = serde_json::from_slice(&bytes).map_err(|e| QueryError::InvalidCursor { message: format!("cursor JSON invalid: {e}") })?;
    if tok.v != 1 {
        return Err(QueryError::InvalidCursor { message: format!("unsupported cursor version {}", tok.v) });
    }
    Ok(tok)
}

fn unquote_arg(s: &str) -> String {
    let t = s.trim();
    if (t.starts_with('\'') && t.ends_with('\'')) || (t.starts_with('"') && t.ends_with('"')) {
        if t.len() >= 2 {
            return t[1..t.len() - 1].to_string();
        }
    }
    t.to_string()
}

struct ScanBudget<'a> {
    scanned: usize,
    max_scanned: usize,
    doc: &'a Document,
    /// One linter per rule set the selector asks about, so the document-wide state a rule needs
    /// (token index, scales, contrast bounds) is built once per call, not once per candidate node.
    linters: HashMap<String, SingleNodeLinter<'a>>,
}

impl<'a> ScanBudget<'a> {
    fn try_charge(&mut self, units: usize) -> Result<(), StepError> {
        if self.scanned.saturating_add(units) > self.max_scanned {
            Err(StepError::BudgetExceeded)
        } else {
            self.scanned += units;
            Ok(())
        }
    }

    /// Findings on `node` for the rule set named `key` (`config` builds it on first use).
    fn lint_node(&mut self, key: &str, config: impl FnOnce() -> LintConfig, node: NodeId, env: &dyn LintEnv) -> Vec<Finding> {
        let doc = self.doc;
        if !self.linters.contains_key(key) {
            self.linters.insert(key.to_string(), SingleNodeLinter::new(doc, &config()));
        }
        self.linters[key].lint(node, env)
    }
}

enum StepError {
    BudgetExceeded,
    Query(QueryError),
}

impl From<QueryError> for StepError {
    fn from(e: QueryError) -> Self {
        Self::Query(e)
    }
}

/// A parsed step plus what the evaluator decided about its node test once per call. The exact
/// upper-case node-type token (`FRAME`, `TEXT`, `*`, `RECT`) tests the node's kind and nothing else,
/// which is what E15-17 3.1 needs (`//TEXT` counts text nodes, not the rectangle "Text field"). Any
/// other word (`Group`, `Page`, `Card`, `Union`) follows `matches_single_step` of the previous query tool
/// (release `engine.rs`), whatever its casing:
/// - a bare step matches a node whose default kind name equals the word, or whose name contains it;
/// - a kind word with predicates (`//Group[@width>90]`) matches the kind only;
/// - a plain name with predicates (`//Card[@width>90]`) keeps matching names, which the previous tool
///   answered with nothing, so a name is never an empty success.
///
/// The name needle is lower-cased once.
struct QueryStep {
    step: XPathStep,
    word: Option<WordTest>,
}

/// How a step that is not an exact node-type token reads a node.
struct WordTest {
    needle: String,
    reads_names: bool,
}

impl QueryStep {
    fn new(step: XPathStep) -> Self {
        let word = (!is_exact_node_type_token(&step.node_test))
            .then(|| WordTest { needle: step.node_test.to_lowercase(), reads_names: step.predicates.is_empty() || !is_node_kind_word(&step.node_test) });
        Self { step, word }
    }

    fn test_matches(&self, node: &Node) -> bool {
        let test = self.step.node_test.as_str();
        match &self.word {
            None => matches_node_test(&node.kind, test),
            Some(word) => node.kind.default_name().eq_ignore_ascii_case(test) || (word.reads_names && node.name.to_lowercase().contains(word.needle.as_str())),
        }
    }

    /// Whether a page can answer this step, so a page at the scope root has to be visited: a `PAGE` node
    /// test, or a bare word (the previous tool matched a page by its kind name or its name like any node).
    fn can_match_a_page(&self) -> bool {
        self.step.node_test.eq_ignore_ascii_case("PAGE") || (self.word.is_some() && self.step.predicates.is_empty())
    }
}

impl std::ops::Deref for QueryStep {
    type Target = XPathStep;

    fn deref(&self) -> &XPathStep {
        &self.step
    }
}

/// Characters XPath gives a meaning to, so a node test holding one is a malformed selector, not a name.
const NODE_TEST_SYNTAX: &[char] = &['(', ')', '[', ']', '@', '=', '<', '>', '!', ',', '\'', '"', '*', '\\'];

/// A selector that is only a name, optionally behind `//` (`Hero`, `//Card`, `Hero Banner`): the
/// previous query tool matched it against node names by substring. Returns the name, which may hold
/// spaces that the XPath parser would split on. `None` for a node-type token or anything with XPath syntax.
fn bare_name_selector(selector: &str) -> Option<&str> {
    let trimmed = selector.trim();
    let name = trimmed.strip_prefix("//").unwrap_or(trimmed).trim();
    let is_name = !name.is_empty() && !name.contains('/') && !name.contains(NODE_TEST_SYNTAX) && !is_node_type_test(name);
    is_name.then_some(name)
}

/// A corrected example for a selector the parser rejected because a name inside it holds spaces
/// (`//Hero Banner//TEXT`): each such segment becomes a `contains(@name, ...)` step.
fn spaced_name_example(selector: &str) -> Option<String> {
    let mut changed = false;
    let segments: Vec<String> = selector
        .trim()
        .split("//")
        .map(|segment| {
            let segment = segment.trim();
            if segment.contains(char::is_whitespace) && !segment.contains('/') && !segment.contains(NODE_TEST_SYNTAX) {
                changed = true;
                format!("*[contains(@name,'{segment}')]")
            } else {
                segment.to_string()
            }
        })
        .collect();
    changed.then(|| segments.join("//"))
}

/// Parses `selector` into steps. A bare name becomes one descendant name step.
fn parse_selector(selector: &str) -> Result<Vec<XPathStep>, QueryError> {
    if let Some(name) = bare_name_selector(selector) {
        return Ok(vec![XPathStep { axis: XPathAxis::Descendant, node_test: name.to_string(), predicates: Vec::new() }]);
    }
    parse_xpath(selector).map_err(|message| QueryError::InvalidSelector {
        message,
        corrected_example: spaced_name_example(selector).unwrap_or_else(|| "//FRAME[@name='Card']".to_string()),
    })
}

/// Validates steps and checks `has-problem` rules and environment availability upfront.
fn validate_steps(steps: &[XPathStep], doc: &Document, env: Option<&dyn LintEnv>) -> Result<(), QueryError> {
    if steps.is_empty() || steps.len() > 63 {
        return Err(QueryError::InvalidSelector {
            message: "selector must have between 1 and 63 steps".to_string(),
            corrected_example: "//TEXT[has-problem('text-truncated')]".to_string(),
        });
    }
    let dummy_node = Node::frame(NodeId(1), crate::Rect::new(0.0, 0.0, 10.0, 10.0));
    for step in steps {
        for ch in step.node_test.chars() {
            if step.node_test != "*" && NODE_TEST_SYNTAX.contains(&ch) {
                return Err(QueryError::InvalidSelector {
                    message: format!("invalid character '{ch}' in node test '{}'", step.node_test),
                    corrected_example: "//FRAME[@name='Card']".to_string(),
                });
            }
        }
        for pred in &step.predicates {
            validate_predicate(pred, doc, &dummy_node, env)?;
        }
    }
    Ok(())
}

fn validate_predicate(pred: &str, doc: &Document, dummy_node: &Node, env: Option<&dyn LintEnv>) -> Result<(), QueryError> {
    let s = pred.trim();
    if s.is_empty() {
        return Err(QueryError::InvalidSelector {
            message: "empty predicate '[]' is not valid".to_string(),
            corrected_example: "//FRAME[@name='Card']".to_string(),
        });
    }
    let mut rest = s;
    while let Some(pos) = rest.find("has-problem(") {
        let after = &rest[pos + "has-problem(".len()..];
        let Some(end) = after.find(')') else {
            return Err(QueryError::InvalidSelector {
                message: "unclosed has-problem(...) call".to_string(),
                corrected_example: "//TEXT[has-problem('text-truncated')]".to_string(),
            });
        };
        let rule_raw = unquote_arg(&after[..end]);
        let Some(canonical) = canonical_rule_id(&rule_raw) else {
            return Err(QueryError::UnknownRule { rule: rule_raw });
        };
        if let Some(spec) = resolve_rule(canonical) {
            if spec.needs == RuleNeeds::Text {
                let has_text = env.is_some_and(|e| e.has_text_engine());
                if !has_text {
                    return Err(QueryError::RuleUnavailable { rule: canonical.to_string(), reason: "no_text_engine".to_string() });
                }
            }
        }
        rest = &after[end + 1..];
    }

    if s.contains("@inProgress") || s.contains("@in_progress") {
        if env.is_none() && doc.in_progress.is_empty() {
            return Err(QueryError::RuleUnavailable { rule: "inProgress".to_string(), reason: "no_in_progress_store".to_string() });
        }
        return Ok(());
    }

    if s.contains("has-problem(") || s.contains("is-bound(") || s.contains("has-hardcoded(") || s.contains("@problems") {
        return Ok(());
    }

    eval_predicate_expr(doc, dummy_node, s)
        .map_err(|message| QueryError::InvalidSelector { message, corrected_example: "//FRAME[@name='Card']".to_string() })?;
    Ok(())
}

fn is_property_bound(node: &Node, prop_key: &str) -> bool {
    let key = prop_key.trim();
    if key == "*" {
        return !node.variables.bindings.is_empty();
    }
    if node.variables.bindings.contains_key(key) {
        return true;
    }
    match key {
        "fills/0" | "fill" | "fills" => {
            node.variables.bindings.contains_key("fills/0") || node.variables.bindings.contains_key("fill") || node.variables.bindings.contains_key("fills")
        }
        "strokes/0" | "stroke" | "strokes" => {
            node.variables.bindings.contains_key("strokes/0")
                || node.variables.bindings.contains_key("stroke")
                || node.variables.bindings.contains_key("strokes")
        }
        _ => false,
    }
}

/// Lint configuration that enables only the rule `canonical`.
fn single_rule_config(canonical: &str) -> LintConfig {
    LintConfig::from_rule_list(&[canonical.to_string()]).unwrap_or_else(|_| LintConfig::from_preset(LintPreset::Agent))
}

/// `true` when `prop_key` names the property a `hardcoded-value` finding reports. `*` names any
/// property; `fill`/`fills` and `stroke`/`strokes` name every paint of that group, the shared-style
/// finding (`fill`) included; any other key must equal the finding's property (`fills/0`, `itemSpacing`).
fn hardcoded_property_matches(prop_key: &str, finding_property: Option<&str>) -> bool {
    let key = prop_key.trim();
    if key == "*" {
        return true;
    }
    let Some(prop) = finding_property else { return false };
    match key {
        "fill" | "fills" => prop == "fill" || prop.starts_with("fills/"),
        "stroke" | "strokes" => prop == "stroke" || prop.starts_with("strokes/"),
        _ => prop == key,
    }
}

/// `has-hardcoded(...)` and `hardcoded: true`: the E15-11 `hardcoded-value` finding on `node` (a
/// value that matches a variable or style but is not bound to it) on property `prop_key`. The linter
/// has already dropped what Check designs ignores.
fn has_hardcoded_property(node: &Node, prop_key: &str, env: Option<&dyn LintEnv>, budget: &mut ScanBudget) -> bool {
    let no_env = NoLintEnv;
    let env_ref: &dyn LintEnv = env.unwrap_or(&no_env);
    let findings = budget.lint_node("hardcoded-value", || single_rule_config("hardcoded-value"), node.id, env_ref);
    findings.iter().any(|f| hardcoded_property_matches(prop_key, f.property.as_deref()))
}

fn compute_node_problems(node: &Node, rule_filter: Option<&str>, env: Option<&dyn LintEnv>, budget: &mut ScanBudget) -> Result<Vec<String>, StepError> {
    let no_env = NoLintEnv;
    let env_ref: &dyn LintEnv = env.unwrap_or(&no_env);

    let findings = if let Some(rule_raw) = rule_filter {
        let Some(canonical) = canonical_rule_id(rule_raw) else {
            return Err(StepError::Query(QueryError::UnknownRule { rule: rule_raw.to_string() }));
        };
        if let Some(spec) = resolve_rule(canonical) {
            if spec.needs == RuleNeeds::Text {
                if !env_ref.has_text_engine() {
                    return Err(StepError::Query(QueryError::RuleUnavailable { rule: canonical.to_string(), reason: "no_text_engine".to_string() }));
                }
                budget.try_charge(20)?;
            }
            if spec.needs == RuleNeeds::Pixels {
                budget.try_charge(200)?;
            }
        }
        budget.lint_node(canonical, || single_rule_config(canonical), node.id, env_ref)
    } else {
        if node.kind.any_text().is_some() && env_ref.has_text_engine() {
            budget.try_charge(20)?;
        }
        budget.lint_node("@agent", || LintConfig::from_preset(LintPreset::Agent), node.id, env_ref)
    };

    let mut rules: Vec<String> = findings.into_iter().map(|f| f.rule.to_string()).collect();
    rules.sort();
    rules.dedup();
    Ok(rules)
}

fn eval_single_predicate(
    doc: &Document,
    node: &Node,
    expr: &str,
    env: Option<&dyn LintEnv>,
    budget: &mut ScanBudget,
    cached_problems: &mut Option<Vec<String>>,
) -> Result<bool, StepError> {
    let s = expr.trim();
    if s.is_empty() {
        return Ok(true);
    }

    if let Ok(pos) = s.parse::<usize>() {
        if pos == 0 {
            return Ok(false);
        }
        let ty = xpath_type_name(&node.kind);
        if let Some(parent) = node.parent.and_then(|p| doc.get(p)) {
            let mut count = 0usize;
            for &sib_id in &parent.children {
                budget.try_charge(1)?;
                if let Some(sib) = doc.get(sib_id) {
                    if xpath_type_name(&sib.kind) == ty {
                        count += 1;
                        if sib_id == node.id {
                            return Ok(count == pos);
                        }
                    }
                }
            }
            return Ok(false);
        }
        return Ok(pos == 1);
    }

    if let Some(rest) = s.strip_prefix("not(").and_then(|r| r.strip_suffix(')')) {
        return Ok(!eval_single_predicate(doc, node, rest, env, budget, cached_problems)?);
    }

    if let Some(inner) = s.strip_prefix("has-problem(").and_then(|r| r.strip_suffix(')')) {
        let rule_name = unquote_arg(inner);
        let Some(canonical) = canonical_rule_id(&rule_name) else {
            return Err(StepError::Query(QueryError::UnknownRule { rule: rule_name }));
        };
        let problems = compute_node_problems(node, Some(canonical), env, budget)?;
        return Ok(problems.iter().any(|r| r == canonical));
    }

    if let Some(inner) = s.strip_prefix("is-bound(").and_then(|r| r.strip_suffix(')')) {
        let prop = unquote_arg(inner);
        return Ok(is_property_bound(node, &prop));
    }

    if let Some(inner) = s.strip_prefix("has-hardcoded(").and_then(|r| r.strip_suffix(')')) {
        let prop = unquote_arg(inner);
        return Ok(has_hardcoded_property(node, &prop, env, budget));
    }

    if s == "@inProgress" || s == "@in_progress" {
        return Ok(node_in_progress(doc, node));
    }

    if s.contains("@problems") {
        let probs = if let Some(ref p) = cached_problems {
            p.clone()
        } else {
            let p = compute_node_problems(node, None, env, budget)?;
            *cached_problems = Some(p.clone());
            p
        };
        let count = probs.len() as f32;
        if s == "@problems" {
            return Ok(count > 0.0);
        }
        for op in ["<=", ">=", "!=", "=", "<", ">"] {
            if let Some((lhs, rhs)) = s.split_once(op) {
                if lhs.trim() == "@problems" {
                    if let Ok(rhs_n) = rhs.trim().parse::<f32>() {
                        return Ok(match op {
                            "=" => (count - rhs_n).abs() < 1e-4,
                            "!=" => (count - rhs_n).abs() >= 1e-4,
                            "<" => count < rhs_n,
                            "<=" => count <= rhs_n,
                            ">" => count > rhs_n,
                            ">=" => count >= rhs_n,
                            _ => false,
                        });
                    }
                }
            }
        }
    }

    eval_predicate_expr(doc, node, s)
        .map_err(|message| StepError::Query(QueryError::InvalidSelector { message, corrected_example: "//FRAME[@name='Card']".to_string() }))
}

fn node_in_progress(doc: &Document, node: &Node) -> bool {
    if doc.get_in_progress(node.id).is_some() {
        return true;
    }
    if let Some(raw) = node.plugin_data.get("designos:in_progress") {
        let t = raw.trim();
        if !t.is_empty() && t != "false" && t != "null" {
            return true;
        }
    }
    false
}

fn step_matches_node(
    doc: &Document,
    node: &Node,
    step: &QueryStep,
    env: Option<&dyn LintEnv>,
    budget: &mut ScanBudget,
    cached_problems: &mut Option<Vec<String>>,
) -> Result<bool, StepError> {
    if !step.test_matches(node) {
        return Ok(false);
    }
    for pred in &step.predicates {
        if !eval_single_predicate(doc, node, pred, env, budget, cached_problems)? {
            return Ok(false);
        }
    }
    Ok(true)
}

fn round2(v: f32) -> f64 {
    ((v as f64) * 100.0).round() / 100.0
}

fn build_row(doc: &Document, node: &Node, format: QueryFormat, problems_val: &Value) -> Value {
    let page_bounds = crate::lint::rules::aabb_of_transformed_rect(doc.world_transform(node.id), node.size);
    let x = round2(page_bounds.x);
    let y = round2(page_bounds.y);
    let w = round2(page_bounds.w);
    let h = round2(page_bounds.h);
    let ty = xpath_type_name(&node.kind);
    let in_prog = node_in_progress(doc, node);

    match format {
        QueryFormat::Rows => json!([node.id.0, node.name, ty, x, y, w, h, problems_val, in_prog]),
        QueryFormat::Objects => json!({
            "id": node.id.0,
            "name": node.name,
            "type": ty,
            "x": x,
            "y": y,
            "w": w,
            "h": h,
            "bounds": [x, y, w, h],
            "visible": node.visible,
            "child_count": node.children.len(),
            "layout_mode": layout_mode_str(node),
            "problems": problems_val,
            "in_progress": in_prog,
        }),
    }
}

#[allow(clippy::too_many_arguments)]
fn evaluate_candidate_node(
    doc: &Document,
    node: &Node,
    steps: &[QueryStep],
    depth: u32,
    parent_matched: u64,
    ancestor_matched: u64,
    last_step_bit: u64,
    opts: &QueryOptions,
    env: Option<&dyn LintEnv>,
    budget: &mut ScanBudget,
) -> Result<(u64, Option<(Value, Value)>), StepError> {
    budget.try_charge(1)?;
    let mut cached_problems: Option<Vec<String>> = None;
    let mut my_mask = 0u64;

    for (s_idx, step) in steps.iter().enumerate() {
        let prev_satisfied = if s_idx == 0 {
            match step.axis {
                XPathAxis::Child => depth == 1,
                XPathAxis::Descendant => true,
            }
        } else {
            let prev_bit = 1u64 << (s_idx - 1);
            match step.axis {
                XPathAxis::Child => (parent_matched & prev_bit) != 0,
                XPathAxis::Descendant => (ancestor_matched & prev_bit) != 0,
            }
        };
        if prev_satisfied && step_matches_node(doc, node, step, env, budget, &mut cached_problems)? {
            my_mask |= 1u64 << s_idx;
        }
    }

    if (my_mask & last_step_bit) == 0 {
        return Ok((my_mask, None));
    }

    if let Some(want_hc) = opts.hardcoded {
        let is_hc = has_hardcoded_property(node, "*", env, budget);
        if is_hc != want_hc {
            return Ok((my_mask, None));
        }
    }

    let problems_val: Value = if let Some(probs) = cached_problems {
        json!(probs)
    } else if opts.with_problems {
        let probs = compute_node_problems(node, None, env, budget)?;
        json!(probs)
    } else {
        Value::Null
    };

    let row = build_row(doc, node, opts.format, &problems_val);
    let obj = build_row(doc, node, QueryFormat::Objects, &problems_val);
    Ok((my_mask, Some((row, obj))))
}

/// Nodes the pass can evaluate: everything below the scope roots, plus a root that is a page when
/// `page_roots_match` makes the root itself a candidate.
fn count_scope_nodes(doc: &Document, roots: &[NodeId], max_depth: Option<u32>, page_roots_match: bool) -> usize {
    let mut count = 0usize;
    let mut stack: Vec<(NodeId, u32)> = roots.iter().rev().map(|&r| (r, 0)).collect();
    let mut seen = HashSet::new();
    while let Some((nid, depth)) = stack.pop() {
        if !seen.insert(nid) {
            continue;
        }
        let Some(node) = doc.nodes.get(&nid) else {
            continue;
        };
        if depth >= 1 || (page_roots_match && matches!(node.kind, NodeKind::Page { .. })) {
            count += 1;
        }
        if max_depth.is_some_and(|max_d| depth >= max_d) {
            continue;
        }
        for &cid in node.children.iter().rev() {
            stack.push((cid, depth + 1));
        }
    }
    count
}

#[derive(Clone, Debug)]
struct StackFrame {
    node_id: NodeId,
    next_child_idx: usize,
    matched_mask: u64,
    ancestor_mask: u64,
    depth: u32,
}

fn default_columns() -> Vec<String> {
    ROW_COLUMNS.iter().map(|s| (*s).to_string()).collect()
}

/// Runs a streaming pre-order XPath query over `doc` with an NFA over steps,
/// enforcing `limit`, `max_scanned`, and cursor pagination (`E15-17`).
pub fn run(doc: &Document, opts: &QueryOptions) -> Result<QueryPage, QueryError> {
    let approx = crate::lint::ApproxLintEnv;
    run_with_env(doc, opts, Some(&approx))
}

/// Runs a streaming pre-order XPath query over `doc` with an explicit [`LintEnv`]
/// (pass `None` when no text-shaping engine is available so `has-problem` on text rules
/// returns `RULE_UNAVAILABLE` per `E15-17` §8.5).
pub fn run_with_env(doc: &Document, opts: &QueryOptions, env: Option<&dyn LintEnv>) -> Result<QueryPage, QueryError> {
    let start_instant = std::time::Instant::now();
    if opts.page.is_some() && opts.within.is_some() {
        return Err(QueryError::ExactlyOneScope);
    }

    let steps = parse_selector(&opts.selector)?;
    validate_steps(&steps, doc, env)?;
    let steps: Vec<QueryStep> = steps.into_iter().map(QueryStep::new).collect();
    // A page has no parent a step could reach it from, so a page at the scope root is a candidate only
    // for a selector with a step a page can answer: a `PAGE` test or a bare word (`//PAGE`, `//Page`,
    // `//Home`). Release walked the scope root with every bare step. Every other selector (`//*`, `//TEXT`,
    // `//Card[@width>90]`) keeps the root out of the rows and out of the scope count.
    let page_roots_match = steps.iter().any(QueryStep::can_match_a_page);

    let limit = opts.limit.unwrap_or(DEFAULT_LIMIT).clamp(1, MAX_LIMIT);
    let max_scanned = opts.max_scanned.or_else(|| opts.node_budget.map(|b| b as usize)).unwrap_or(DEFAULT_MAX_SCANNED).clamp(1, MAX_MAX_SCANNED);

    let doc_revision = doc.mutation_epoch();
    let expr_hash = fnv1a64(format!("{}:{:?}", opts.selector.trim(), opts.hardcoded).as_bytes());

    if let Some(page_id) = opts.page {
        let Some(pnode) = doc.nodes.get(&page_id) else {
            return Err(QueryError::NodeNotFound { id: page_id });
        };
        if !matches!(pnode.kind, NodeKind::Page { .. }) {
            return Err(QueryError::NodeNotFound { id: page_id });
        }
    }

    if let Some(within_id) = opts.within {
        if !doc.nodes.contains_key(&within_id) {
            return Err(QueryError::NodeNotFound { id: within_id });
        }
    }

    let scope_root: Option<NodeId> = opts.within.or(opts.page);
    let roots: Vec<NodeId> = if let Some(sr) = scope_root {
        vec![sr]
    } else if !doc.pages.is_empty() {
        doc.pages.clone()
    } else {
        doc.nodes.values().filter(|n| n.parent.is_none()).map(|n| n.id).collect()
    };

    let mut stack: Vec<StackFrame> = Vec::new();
    let (scope_size, prev_scanned_total) = if let Some(cursor_str) = &opts.cursor {
        let tok = decode_cursor(cursor_str)?;
        if tok.doc_revision != doc_revision {
            return Err(QueryError::CursorStale { cursor_epoch: tok.doc_revision, document_epoch: doc_revision });
        }
        if tok.expr_hash != expr_hash || tok.scope_root != scope_root.map(|id| id.0) || tok.stack.is_empty() {
            return Err(QueryError::InvalidCursor { message: "cursor parameters do not match query selector or scope".to_string() });
        }
        let mut running_anc = 0u64;
        for (idx, &(nid_raw, next_child_idx, matched_mask)) in tok.stack.iter().enumerate() {
            let nid = NodeId(nid_raw);
            if !doc.nodes.contains_key(&nid) {
                return Err(QueryError::InvalidCursor { message: format!("cursor references missing node {}", nid.0) });
            }
            stack.push(StackFrame { node_id: nid, next_child_idx, matched_mask, ancestor_mask: running_anc, depth: idx as u32 });
            running_anc |= matched_mask;
        }
        (tok.scope_size, tok.scanned_total)
    } else {
        let sz = count_scope_nodes(doc, &roots, opts.max_depth, page_roots_match);
        if let Some(&first_root) = roots.first() {
            stack.push(StackFrame { node_id: first_root, next_child_idx: 0, matched_mask: 0, ancestor_mask: 0, depth: 0 });
        }
        (sz, 0usize)
    };

    let mut rows = Vec::new();
    let mut matches_obj = Vec::new();
    let mut budget = ScanBudget { scanned: 0, max_scanned, doc, linters: HashMap::new() };
    let mut nodes_completed = 0usize;
    let mut seen_nodes: HashSet<NodeId> = HashSet::new();
    for frame in &stack {
        seen_nodes.insert(frame.node_id);
    }

    let advance_root = |stack: &mut Vec<StackFrame>, finished_root: NodeId| {
        if let Some(pos) = roots.iter().position(|&r| r == finished_root) {
            if let Some(&next_root) = roots.get(pos + 1) {
                stack.push(StackFrame { node_id: next_root, next_child_idx: 0, matched_mask: 0, ancestor_mask: 0, depth: 0 });
            }
        }
    };

    let settle_stack = |stack: &mut Vec<StackFrame>| {
        while let Some(top) = stack.last() {
            let Some(node) = doc.nodes.get(&top.node_id) else {
                let popped = stack.pop().unwrap();
                if stack.is_empty() {
                    advance_root(stack, popped.node_id);
                }
                continue;
            };
            let at_max_depth = opts.max_depth.is_some_and(|max_d| top.depth >= max_d);
            if top.next_child_idx == 0 {
                break;
            }
            if !at_max_depth && top.next_child_idx <= node.children.len() {
                break;
            }
            let popped = stack.pop().unwrap();
            if stack.is_empty() {
                advance_root(stack, popped.node_id);
            }
        }
    };

    let last_step_bit = 1u64 << (steps.len() - 1);

    loop {
        settle_stack(&mut stack);
        let Some(top_idx) = stack.len().checked_sub(1) else {
            let scanned_total = (prev_scanned_total + budget.scanned).min(scope_size.max(prev_scanned_total + budget.scanned));
            let remaining = scope_size.saturating_sub(scanned_total);
            let count = rows.len();
            return Ok(QueryPage {
                columns: default_columns(),
                rows,
                matches: matches_obj,
                count,
                returned: count,
                scanned: budget.scanned,
                scanned_total,
                scope_size,
                remaining,
                done: true,
                complete: true,
                truncated: false,
                cursor: None,
                next_cursor: None,
                document_epoch: doc_revision,
                elapsed_ms: start_instant.elapsed().as_millis() as u64,
            });
        };

        if stack[top_idx].next_child_idx == 0 {
            let nid = stack[top_idx].node_id;
            let depth = stack[top_idx].depth;
            let parent_matched = if top_idx > 0 { stack[top_idx - 1].matched_mask } else { 0 };
            let ancestor_matched = stack[top_idx].ancestor_mask;

            let Some(node) = doc.nodes.get(&nid) else {
                stack[top_idx].next_child_idx = 1;
                continue;
            };

            if depth == 0 && !(page_roots_match && matches!(node.kind, NodeKind::Page { .. })) {
                stack[top_idx].next_child_idx = 1;
                continue;
            }

            let node_start_scanned = budget.scanned;
            match evaluate_candidate_node(doc, node, &steps, depth, parent_matched, ancestor_matched, last_step_bit, opts, env, &mut budget) {
                Ok((my_mask, maybe_match)) => {
                    stack[top_idx].next_child_idx = 1;
                    stack[top_idx].matched_mask = my_mask;
                    nodes_completed += 1;

                    if let Some((row, obj)) = maybe_match {
                        rows.push(row);
                        matches_obj.push(obj);
                    }

                    if rows.len() >= limit || budget.scanned >= max_scanned {
                        settle_stack(&mut stack);
                        let has_more = !stack.is_empty();
                        let scanned_total = prev_scanned_total + budget.scanned;
                        let remaining = if has_more { scope_size.saturating_sub(scanned_total) } else { 0 };
                        let cur = if has_more {
                            let tok = CursorToken {
                                v: 1,
                                doc_revision,
                                scope_root: scope_root.map(|id| id.0),
                                expr_hash,
                                scope_size,
                                scanned_total,
                                stack: stack.iter().map(|f| (f.node_id.0, f.next_child_idx, f.matched_mask)).collect(),
                            };
                            Some(encode_cursor(&tok))
                        } else {
                            None
                        };
                        let count = rows.len();
                        return Ok(QueryPage {
                            columns: default_columns(),
                            rows,
                            matches: matches_obj,
                            count,
                            returned: count,
                            scanned: budget.scanned,
                            scanned_total,
                            scope_size,
                            remaining,
                            done: !has_more,
                            complete: !has_more,
                            truncated: has_more,
                            cursor: cur.clone(),
                            next_cursor: cur,
                            document_epoch: doc_revision,
                            elapsed_ms: start_instant.elapsed().as_millis() as u64,
                        });
                    }
                }
                Err(StepError::Query(e)) => return Err(e),
                Err(StepError::BudgetExceeded) => {
                    if nodes_completed == 0 {
                        stack[top_idx].next_child_idx = 1;
                        settle_stack(&mut stack);
                    } else {
                        budget.scanned = node_start_scanned;
                    }
                    let has_more = !stack.is_empty();
                    let scanned_total = prev_scanned_total + budget.scanned;
                    let remaining = if has_more { scope_size.saturating_sub(scanned_total) } else { 0 };
                    let cur = if has_more {
                        let tok = CursorToken {
                            v: 1,
                            doc_revision,
                            scope_root: scope_root.map(|id| id.0),
                            expr_hash,
                            scope_size,
                            scanned_total,
                            stack: stack.iter().map(|f| (f.node_id.0, f.next_child_idx, f.matched_mask)).collect(),
                        };
                        Some(encode_cursor(&tok))
                    } else {
                        None
                    };
                    let count = rows.len();
                    return Ok(QueryPage {
                        columns: default_columns(),
                        rows,
                        matches: matches_obj,
                        count,
                        returned: count,
                        scanned: budget.scanned,
                        scanned_total,
                        scope_size,
                        remaining,
                        done: !has_more,
                        complete: !has_more,
                        truncated: has_more,
                        cursor: cur.clone(),
                        next_cursor: cur,
                        document_epoch: doc_revision,
                        elapsed_ms: start_instant.elapsed().as_millis() as u64,
                    });
                }
            }
        } else {
            let nid = stack[top_idx].node_id;
            let depth = stack[top_idx].depth;
            let child_anc_mask = stack[top_idx].ancestor_mask | stack[top_idx].matched_mask;
            let child_idx = stack[top_idx].next_child_idx - 1;
            stack[top_idx].next_child_idx += 1;

            let Some(node) = doc.nodes.get(&nid) else {
                continue;
            };
            if let Some(&child_id) = node.children.get(child_idx) {
                if seen_nodes.insert(child_id) {
                    stack.push(StackFrame { node_id: child_id, next_child_idx: 0, matched_mask: 0, ancestor_mask: child_anc_mask, depth: depth + 1 });
                }
            }
        }
    }
}

#[cfg(test)]
mod release_parity_tests;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lint::{lint_subtree, Budget as LintBudget};
    use crate::node::{AutoLayout, LayoutDirection};
    use crate::paint::{TextAutoResize, TextStyle};
    use crate::Rect;

    fn build_synthetic_doc(frame_count: usize, children_per_frame: usize) -> (Document, NodeId) {
        let mut doc = Document::new("Large Query Test");
        let page = doc.add_page("Page 1");
        for f_idx in 0..frame_count {
            let fid = doc.alloc_id();
            let mut frame = Node::frame(fid, Rect::new((f_idx as f32) * 120.0, 0.0, 100.0, 100.0));
            frame.name = if f_idx % 3 == 0 { "Card".to_string() } else { format!("Card_{f_idx}") };
            if let NodeKind::Frame(ref mut fp) = frame.kind {
                let mut al = AutoLayout::default();
                al.direction = LayoutDirection::Vertical;
                fp.auto_layout = Some(al);
            }
            doc.insert(page, frame).unwrap();
            for c_idx in 0..children_per_frame {
                let cid = doc.alloc_id();
                let child = if c_idx % 2 == 0 {
                    let mut t = Node::text(cid, Rect::new(0.0, 0.0, 80.0, 20.0), format!("Label {f_idx}:{c_idx}"), TextStyle::default());
                    t.name = if c_idx == 0 { format!("btn_{f_idx}") } else { format!("Text_{f_idx}_{c_idx}") };
                    t
                } else {
                    let mut r = Node::rectangle(cid, Rect::new(0.0, 24.0, 80.0, 20.0));
                    r.name = format!("Rect_{f_idx}_{c_idx}");
                    r
                };
                doc.insert(fid, child).unwrap();
            }
        }
        (doc, page)
    }

    #[test]
    fn paging_returns_each_text_once() {
        let (doc, page_id) = build_synthetic_doc(120, 4);
        let expected_texts = doc.nodes.values().filter(|n| matches!(n.kind, NodeKind::Text(_))).count();
        assert_eq!(expected_texts, 240);

        let mut seen_ids = Vec::new();
        let mut cursor = None;
        loop {
            let res = run(
                &doc,
                &QueryOptions {
                    selector: "//TEXT".to_string(),
                    page: Some(page_id),
                    limit: Some(50),
                    max_scanned: Some(150),
                    cursor: cursor.clone(),
                    ..Default::default()
                },
            )
            .unwrap();
            for row in &res.rows {
                seen_ids.push(row[0].as_u64().unwrap());
            }
            if res.done {
                assert!(res.cursor.is_none());
                assert_eq!(res.remaining, 0);
                break;
            }
            cursor = res.cursor;
            assert!(cursor.is_some());
        }
        assert_eq!(seen_ids.len(), expected_texts);
        let unique: HashSet<u64> = seen_ids.iter().copied().collect();
        assert_eq!(unique.len(), expected_texts);
    }

    #[test]
    fn clipped_text_query_matches_linter() {
        let mut doc = Document::new("Clipped Text Test");
        let page = doc.add_page("Page 1");
        let root = doc.alloc_id();
        doc.insert(page, Node::frame(root, Rect::new(0.0, 0.0, 800.0, 600.0))).unwrap();

        for i in 0..20 {
            let tid = doc.alloc_id();
            let mut style = TextStyle::default();
            style.font_size = 16.0;
            let content = if i < 12 { "This is a very long sentence that definitely overflows a narrow 40x24 fixed text box" } else { "Hi" };
            let mut t = Node::text(tid, Rect::new(0.0, (i as f32) * 28.0, 40.0, 24.0), content, style);
            if let NodeKind::Text(ref mut tp) = t.kind {
                tp.auto_resize = TextAutoResize::None;
            }
            doc.insert(root, t).unwrap();
        }

        let cfg = LintConfig::from_rule_list(&["text-truncated".to_string()]).unwrap();
        let mut budget = LintBudget::unlimited();
        let lint_report = lint_subtree(&doc, &[page], &cfg, &crate::lint::ApproxLintEnv, &mut budget);
        let lint_ids: Vec<u64> = lint_report.findings.iter().map(|f| f.node.0).collect();
        assert_eq!(lint_ids.len(), 12);

        let q_page = run(
            &doc,
            &QueryOptions { selector: "//TEXT[has-problem('text-truncated')]".to_string(), page: Some(page), limit: Some(100), ..Default::default() },
        )
        .unwrap();
        let query_ids: Vec<u64> = q_page.rows.iter().map(|r| r[0].as_u64().unwrap()).collect();
        assert_eq!(query_ids, lint_ids);
    }

    #[test]
    fn no_page_exceeds_budget() {
        let (doc, page_id) = build_synthetic_doc(80, 4);
        for selector in ["//TEXT", "//TEXT[has-problem('text-truncated')]", "//TEXT[2]"] {
            let mut cursor = None;
            let mut pages = 0usize;
            loop {
                let res = run(
                    &doc,
                    &QueryOptions {
                        selector: selector.to_string(),
                        page: Some(page_id),
                        limit: Some(25),
                        max_scanned: Some(60),
                        cursor: cursor.clone(),
                        ..Default::default()
                    },
                )
                .unwrap();
                pages += 1;
                assert!(res.rows.len() <= 25);
                assert!(res.scanned <= 60, "selector {selector} exceeded budget: {}", res.scanned);
                if res.done {
                    break;
                }
                cursor = res.cursor;
            }
            assert!(pages >= 6);
        }
    }

    #[test]
    fn cursor_stale_after_edit() {
        let (mut doc, page_id) = build_synthetic_doc(40, 2);
        let p1 = run(&doc, &QueryOptions { selector: "//FRAME".to_string(), page: Some(page_id), limit: Some(10), ..Default::default() }).unwrap();
        let cursor = p1.cursor.expect("expected continuation cursor");

        let extra = doc.alloc_id();
        doc.insert(page_id, Node::rectangle(extra, Rect::new(0.0, 0.0, 10.0, 10.0))).unwrap();
        doc.bump_mutation_epoch();

        let err =
            run(&doc, &QueryOptions { selector: "//FRAME".to_string(), page: Some(page_id), limit: Some(10), cursor: Some(cursor), ..Default::default() })
                .unwrap_err();
        assert!(matches!(err, QueryError::CursorStale { .. }));
    }

    #[test]
    fn legacy_selectors_still_work() {
        let (doc, page_id) = build_synthetic_doc(18, 2);

        let mut expected_cards = Vec::new();
        let mut expected_btns = Vec::new();
        for &fid in &doc.get(page_id).unwrap().children {
            let fnode = doc.get(fid).unwrap();
            if matches!(fnode.kind, NodeKind::Frame(_)) && fnode.name == "Card" {
                expected_cards.push(fnode.id.0);
            }
            if fnode.name.contains("btn") {
                expected_btns.push(fnode.id.0);
            }
            for &cid in &fnode.children {
                let cnode = doc.get(cid).unwrap();
                if cnode.name.contains("btn") {
                    expected_btns.push(cnode.id.0);
                }
            }
        }

        for (sel, expected) in [("//FRAME[@name='Card']", expected_cards), ("//*[contains(@name,'btn')]", expected_btns)] {
            let q = run(&doc, &QueryOptions { selector: sel.to_string(), page: Some(page_id), limit: Some(500), ..Default::default() }).unwrap();
            let stream_ids: Vec<u64> = q.rows.iter().map(|r| r[0].as_u64().unwrap()).collect();
            let legacy = crate::openpencil::query_xpath(&doc, Some(page_id), sel, 500).unwrap();
            let legacy_ids: Vec<u64> = legacy.iter().map(|v| v["id"].as_u64().unwrap()).collect();
            assert!(!stream_ids.is_empty());
            assert_eq!(stream_ids, expected);
            assert_eq!(stream_ids, legacy_ids);
        }
    }

    #[test]
    fn component_and_fill_selectors_of_the_previous_query_still_match() {
        let mut doc = Document::new("Component And Fill");
        let page = doc.add_page("Page 1");

        let button = doc.alloc_id();
        let mut main = Node::frame(button, Rect::new(0.0, 0.0, 80.0, 24.0));
        main.name = "Button".to_string();
        main.kind = NodeKind::Component(crate::FrameProps::default());
        doc.insert(page, main).unwrap();

        let set = doc.alloc_id();
        let mut component_set = Node::frame(set, Rect::new(0.0, 100.0, 200.0, 40.0));
        component_set.name = "Toggle".to_string();
        component_set.kind = NodeKind::ComponentSet(crate::FrameProps::default());
        doc.insert(page, component_set).unwrap();
        let variant = doc.alloc_id();
        let mut variant_main = Node::frame(variant, Rect::new(0.0, 0.0, 40.0, 24.0));
        variant_main.name = "State=On".to_string();
        variant_main.kind = NodeKind::Component(crate::FrameProps::default());
        doc.insert(set, variant_main).unwrap();

        let instance_of = |doc: &mut Document, component: NodeId, name: &str, y: f32| {
            let id = doc.alloc_id();
            let mut node = Node::frame(id, Rect::new(0.0, y, 80.0, 24.0));
            node.name = name.to_string();
            node.kind = NodeKind::Instance { frame: crate::FrameProps::default(), component: Some(component), component_key: None };
            doc.insert(page, node).unwrap();
            id
        };
        let cta = instance_of(&mut doc, button, "CTA", 200.0);
        let toggle = instance_of(&mut doc, variant, "Dark mode", 240.0);

        let filled = doc.alloc_id();
        doc.insert(page, Node::rectangle(filled, Rect::new(0.0, 300.0, 10.0, 10.0))).unwrap();
        let bare = doc.alloc_id();
        let mut bare_rect = Node::rectangle(bare, Rect::new(0.0, 320.0, 10.0, 10.0));
        bare_rect.fills.clear();
        doc.insert(page, bare_rect).unwrap();

        assert_eq!(ids_of(&doc, page, "//INSTANCE[@component='Button']"), vec![cta.0]);
        assert_eq!(ids_of(&doc, page, "//INSTANCE[@component='Toggle']"), vec![toggle.0], "a variant answers to its component set's name");
        assert!(ids_of(&doc, page, "//INSTANCE[@component='Card']").is_empty());
        assert_eq!(ids_of(&doc, page, "//INSTANCE[@component!='Button']"), vec![toggle.0]);
        assert_eq!(ids_of(&doc, page, "//RECTANGLE[@fill]"), vec![filled.0]);
        assert_eq!(ids_of(&doc, page, "//RECTANGLE[not(@fill)]"), vec![bare.0]);
    }

    #[test]
    fn rule_unavailable_when_no_text_engine() {
        let (doc, page_id) = build_synthetic_doc(2, 2);
        let err = run_with_env(
            &doc,
            &QueryOptions { selector: "//TEXT[has-problem('text-truncated')]".to_string(), page: Some(page_id), ..Default::default() },
            None,
        )
        .unwrap_err();
        assert!(matches!(err, QueryError::RuleUnavailable { .. }));
    }

    #[test]
    fn hardcoded_query_matches_linter_and_ignores_styled_fill_and_other_props() {
        let mut doc = Document::new("Hardcoded Test");
        let page = doc.add_page("Page 1");
        add_color_variable(&mut doc, "brand", "Color/Brand", crate::Color::rgb(200, 50, 50));
        add_color_variable(&mut doc, "ink", "Color/Ink", crate::Color::BLACK);

        // 1. Raw solid fill that matches a colour variable -> flagged by hardcoded-value
        let raw_id = doc.alloc_id();
        let mut raw_frame = Node::frame(raw_id, Rect::new(0.0, 0.0, 100.0, 100.0));
        raw_frame.fills = vec![crate::Paint::solid(crate::Color::rgb(200, 50, 50))];
        doc.insert(page, raw_frame).unwrap();

        // 2. Solid fill linked to a shared fill style -> NOT hardcoded-value
        let styled_id = doc.alloc_id();
        let mut styled_frame = Node::frame(styled_id, Rect::new(120.0, 0.0, 100.0, 100.0));
        styled_frame.fills = vec![crate::Paint::solid(crate::Color::rgb(200, 50, 50))];
        styled_frame.style_refs.insert("fill".to_string(), "style_brand".to_string());
        doc.insert(page, styled_frame).unwrap();

        // 3. Solid fill bound to a variable -> NOT hardcoded-value
        let bound_id = doc.alloc_id();
        let mut bound_frame = Node::frame(bound_id, Rect::new(240.0, 0.0, 100.0, 100.0));
        bound_frame.fills = vec![crate::Paint::solid(crate::Color::rgb(200, 50, 50))];
        bound_frame.variables.bindings.insert("fills/0".to_string(), "brand".to_string());
        doc.insert(page, bound_frame).unwrap();

        // 4. Frame with no fill, but with a variable-bound stroke, positive itemSpacing, and positive corner_radii -> NOT hardcoded-value
        let other_id = doc.alloc_id();
        let mut other_frame = Node::frame(other_id, Rect::new(360.0, 0.0, 100.0, 100.0));
        other_frame.fills.clear();
        other_frame.stroke.paints = vec![crate::Paint::solid(crate::Color::BLACK)];
        other_frame.stroke.weight = 2.0;
        other_frame.variables.bindings.insert("strokes/0".to_string(), "ink".to_string());
        other_frame.corner_radii = [8.0; 4];
        if let NodeKind::Frame(ref mut fp) = other_frame.kind {
            let mut al = AutoLayout::default();
            al.item_spacing = 12.0;
            fp.auto_layout = Some(al);
        }
        doc.insert(page, other_frame).unwrap();

        let lint_ids = linter_ids(&doc, page, "hardcoded-value");
        assert_eq!(lint_ids, vec![raw_id.0]);

        let q_fills = run(&doc, &QueryOptions { selector: "//*[has-hardcoded('fills/0')]".to_string(), page: Some(page), ..Default::default() }).unwrap();
        let fills_ids: Vec<u64> = q_fills.rows.iter().map(|r| r[0].as_u64().unwrap()).collect();
        assert_eq!(fills_ids, lint_ids, "has-hardcoded('fills/0') must match linter hardcoded-value findings and skip style_refs");

        let q_star = run(&doc, &QueryOptions { selector: "//*[has-hardcoded('*')]".to_string(), page: Some(page), ..Default::default() }).unwrap();
        let star_ids: Vec<u64> = q_star.rows.iter().map(|r| r[0].as_u64().unwrap()).collect();
        assert_eq!(star_ids, lint_ids, "has-hardcoded('*') must match linter findings without flagging strokes/spacing/radius");

        let q_opt = run(&doc, &QueryOptions { selector: "//*".to_string(), page: Some(page), hardcoded: Some(true), ..Default::default() }).unwrap();
        let opt_ids: Vec<u64> = q_opt.rows.iter().map(|r| r[0].as_u64().unwrap()).collect();
        assert_eq!(opt_ids, lint_ids, "hardcoded: Some(true) must filter to linter hardcoded findings");

        // A node whose only unbound paint is a stroke is a linter finding, but not a `fills/0` one.
        let stroke_only_id = doc.alloc_id();
        let mut stroke_only = Node::frame(stroke_only_id, Rect::new(480.0, 0.0, 100.0, 100.0));
        stroke_only.fills.clear();
        stroke_only.stroke.paints = vec![crate::Paint::solid(crate::Color::BLACK)];
        stroke_only.stroke.weight = 2.0;
        doc.insert(page, stroke_only).unwrap();
        let q_stroke_as_fill =
            run(&doc, &QueryOptions { selector: "//*[has-hardcoded('fills/0')]".to_string(), page: Some(page), ..Default::default() }).unwrap();
        let ids: Vec<u64> = q_stroke_as_fill.rows.iter().map(|r| r[0].as_u64().unwrap()).collect();
        assert_eq!(ids, vec![raw_id.0], "stroke-only node must not match has-hardcoded('fills/0')");
        let q_stroke = run(&doc, &QueryOptions { selector: "//*[has-hardcoded('strokes/0')]".to_string(), page: Some(page), ..Default::default() }).unwrap();
        let ids: Vec<u64> = q_stroke.rows.iter().map(|r| r[0].as_u64().unwrap()).collect();
        assert_eq!(ids, vec![stroke_only_id.0], "stroke-only node must match has-hardcoded('strokes/0')");
    }

    /// E15-17 test row `hardcoded_query_matches_linter`: the predicate answers from the E15-11
    /// `hardcoded-value` finding and its `property`, so a raw colour with no matching variable is not
    /// hard-coded for the query (the legacy `hardcoded-color` rule flags it), and number properties
    /// that match a variable are (the legacy rule never looks at them).
    #[test]
    fn hardcoded_query_matches_linter() {
        let mut doc = Document::new("Hardcoded Value Test");
        let page = doc.add_page("Page 1");
        add_color_variable(&mut doc, "brand", "Color/Brand", crate::Color::rgb(200, 50, 50));
        add_number_variable(&mut doc, "space-12", "Space/12", 12.0, &["GAP"]);

        let frame = |doc: &mut Document, x: f32, fill: Option<crate::Color>| {
            let id = doc.alloc_id();
            let mut f = Node::frame(id, Rect::new(x, 0.0, 100.0, 100.0));
            f.fills = fill.map(|c| vec![crate::Paint::solid(c)]).unwrap_or_default();
            doc.insert(page, f).unwrap();
            id
        };
        // A: raw fill equal to a variable. B: raw fill that matches no variable. C: fill bound to the variable.
        let a = frame(&mut doc, 0.0, Some(crate::Color::rgb(200, 50, 50)));
        let b = frame(&mut doc, 120.0, Some(crate::Color::rgb(10, 20, 30)));
        let c = frame(&mut doc, 240.0, Some(crate::Color::rgb(200, 50, 50)));
        doc.nodes.get_mut(&c).unwrap().variables.bindings.insert("fills/0".to_string(), "brand".to_string());
        // D: no fill, gap equal to a number variable. E: matching fill that is linked to a shared style.
        let d = frame(&mut doc, 360.0, None);
        if let NodeKind::Frame(ref mut fp) = doc.nodes.get_mut(&d).unwrap().kind {
            let mut al = AutoLayout::default();
            al.item_spacing = 12.0;
            fp.auto_layout = Some(al);
        }
        let e = frame(&mut doc, 480.0, Some(crate::Color::rgb(200, 50, 50)));
        doc.nodes.get_mut(&e).unwrap().style_refs.insert("fill".to_string(), "style_brand".to_string());
        // F: no fill, raw stroke equal to the variable.
        let f = frame(&mut doc, 600.0, None);
        {
            let n = doc.nodes.get_mut(&f).unwrap();
            n.stroke.paints = vec![crate::Paint::solid(crate::Color::rgb(200, 50, 50))];
            n.stroke.weight = 2.0;
        }

        // The two rules disagree on this fixture, so the test cannot pass on the legacy rule.
        assert!(linter_ids(&doc, page, "hardcoded-color").contains(&b.0), "hardcoded-color flags the raw fill with no variable");
        assert!(!linter_ids(&doc, page, "hardcoded-value").contains(&b.0), "hardcoded-value does not");

        let on_fills = linter_property_ids(&doc, page, "hardcoded-value", "fills/0");
        assert_eq!(on_fills, vec![a.0], "fixture: only A is a hardcoded-value finding on fills/0");
        assert_eq!(ids_of(&doc, page, "//*[has-hardcoded('fills/0')]"), on_fills);
        assert_eq!(ids_of(&doc, page, "//*[has-hardcoded('strokes/0')]"), linter_property_ids(&doc, page, "hardcoded-value", "strokes/0"));
        assert_eq!(ids_of(&doc, page, "//*[has-hardcoded('strokes/0')]"), vec![f.0]);
        assert_eq!(ids_of(&doc, page, "//*[has-hardcoded('itemSpacing')]"), vec![d.0], "a number property that matches a variable is hard-coded too");
        assert_eq!(ids_of(&doc, page, "//*[has-hardcoded('fills')]"), vec![a.0], "`fills` names every fill paint");
        assert_eq!(ids_of(&doc, page, "//*[has-hardcoded('strokes')]"), vec![f.0], "`strokes` names every stroke paint");

        let any = linter_ids(&doc, page, "hardcoded-value");
        assert_eq!(any, vec![a.0, d.0, f.0]);
        assert_eq!(ids_of(&doc, page, "//*[has-hardcoded('*')]"), any);
        let flagged = run(&doc, &QueryOptions { selector: "//*".to_string(), page: Some(page), hardcoded: Some(true), ..Default::default() }).unwrap();
        assert_eq!(flagged.rows.iter().map(|r| r[0].as_u64().unwrap()).collect::<Vec<_>>(), any, "hardcoded: true");
        let rest = run(&doc, &QueryOptions { selector: "//*".to_string(), page: Some(page), hardcoded: Some(false), ..Default::default() }).unwrap();
        assert_eq!(rest.rows.iter().map(|r| r[0].as_u64().unwrap()).collect::<Vec<_>>(), vec![b.0, c.0, e.0], "hardcoded: false");
    }

    fn ids_of(doc: &Document, page: NodeId, selector: &str) -> Vec<u64> {
        let page = run(doc, &QueryOptions { selector: selector.to_string(), page: Some(page), ..Default::default() }).unwrap();
        page.rows.iter().map(|r| r[0].as_u64().unwrap()).collect()
    }

    fn linter_ids(doc: &Document, page: NodeId, rule: &str) -> Vec<u64> {
        let cfg = LintConfig::from_rule_list(&[rule.to_string()]).unwrap();
        let report = lint_subtree(doc, &[page], &cfg, &crate::lint::ApproxLintEnv, &mut LintBudget::unlimited());
        report.findings.iter().map(|f| f.node.0).collect()
    }

    /// Ids of the nodes the linter reports for `rule` on exactly `property`.
    fn linter_property_ids(doc: &Document, page: NodeId, rule: &str, property: &str) -> Vec<u64> {
        let cfg = LintConfig::from_rule_list(&[rule.to_string()]).unwrap();
        let report = lint_subtree(doc, &[page], &cfg, &crate::lint::ApproxLintEnv, &mut LintBudget::unlimited());
        report.findings.iter().filter(|f| f.property.as_deref() == Some(property)).map(|f| f.node.0).collect()
    }

    fn add_variable(doc: &mut Document, id: &str, name: &str, kind: crate::VariableKind, value: crate::VariableValue, scopes: &[&str]) {
        use crate::{Variable, VariableCollection, VariableMode};
        let collection = doc.variable_collections.entry("tokens".into()).or_insert_with(|| VariableCollection {
            id: "tokens".into(),
            name: "tokens".into(),
            modes: vec![VariableMode { id: "m".into(), name: "m".into() }],
            default_mode: "m".into(),
            variables: Vec::new(),
            key: None,
            source_library_key: None,
            extensions: Default::default(),
        });
        collection.variables.push(id.to_string());
        doc.variables.insert(
            id.to_string(),
            Variable {
                id: id.to_string(),
                name: name.to_string(),
                collection: "tokens".into(),
                kind,
                values: [("m".to_string(), value)].into_iter().collect(),
                description: String::new(),
                scopes: scopes.iter().map(|s| (*s).to_string()).collect(),
                hidden_from_publishing: false,
                key: None,
                code_syntax: Default::default(),
                extensions: Default::default(),
                mode_extensions: Default::default(),
            },
        );
    }

    fn add_color_variable(doc: &mut Document, id: &str, name: &str, colour: crate::Color) {
        add_variable(doc, id, name, crate::VariableKind::Color, crate::VariableValue::Color(colour), &[]);
    }

    fn add_number_variable(doc: &mut Document, id: &str, name: &str, value: f32, scopes: &[&str]) {
        add_variable(doc, id, name, crate::VariableKind::Number, crate::VariableValue::Number(value), scopes);
    }

    #[test]
    fn check_designs_ignored_hardcoded_fill_is_hidden_from_queries_like_from_the_linter() {
        let mut doc = Document::new("Ignored Hardcoded");
        let page = doc.add_page("Page 1");
        add_color_variable(&mut doc, "brand", "Color/Brand", crate::Color::rgb(200, 50, 50));
        let ignored_id = doc.alloc_id();
        let mut ignored_frame = Node::frame(ignored_id, Rect::new(0.0, 0.0, 100.0, 100.0));
        ignored_frame.fills = vec![crate::Paint::solid(crate::Color::rgb(200, 50, 50))];
        doc.insert(page, ignored_frame).unwrap();
        let kept_id = doc.alloc_id();
        let mut kept_frame = Node::frame(kept_id, Rect::new(120.0, 0.0, 100.0, 100.0));
        kept_frame.fills = vec![crate::Paint::solid(crate::Color::rgb(200, 50, 50))];
        doc.insert(page, kept_frame).unwrap();

        // The key Check designs writes when the user ignores one hard-coded fill.
        doc.check_designs.ignored.insert(crate::IgnoreKey { node: ignored_id, rule: "RECOMMEND_VARIABLES".to_string(), property: "fills/0".to_string() });

        assert_eq!(linter_ids(&doc, page, "hardcoded-value"), vec![kept_id.0], "the linter drops the ignored hard-coded fill");
        assert_eq!(ids_of(&doc, page, "//*[has-hardcoded('fills/0')]"), vec![kept_id.0]);
        assert_eq!(ids_of(&doc, page, "//*[has-hardcoded('*')]"), vec![kept_id.0]);
        assert_eq!(ids_of(&doc, page, "//*[has-problem('hardcoded-color')]"), vec![kept_id.0]);
        let filtered = run(&doc, &QueryOptions { selector: "//*".to_string(), page: Some(page), hardcoded: Some(true), ..Default::default() }).unwrap();
        let filtered_ids: Vec<u64> = filtered.rows.iter().map(|r| r[0].as_u64().unwrap()).collect();
        assert_eq!(filtered_ids, vec![kept_id.0], "hardcoded: true agrees with the linter");
    }

    #[test]
    fn per_property_ignore_hides_only_the_ignored_paint_from_has_hardcoded() {
        let mut doc = Document::new("Partly Ignored Hardcoded");
        let page = doc.add_page("Page 1");
        add_color_variable(&mut doc, "brand", "Color/Brand", crate::Color::rgb(200, 50, 50));
        add_color_variable(&mut doc, "ink", "Color/Ink", crate::Color::BLACK);
        let id = doc.alloc_id();
        let mut frame = Node::frame(id, Rect::new(0.0, 0.0, 100.0, 100.0));
        frame.fills = vec![crate::Paint::solid(crate::Color::rgb(200, 50, 50))];
        frame.stroke.paints = vec![crate::Paint::solid(crate::Color::BLACK)];
        frame.stroke.weight = 2.0;
        doc.insert(page, frame).unwrap();
        doc.check_designs.ignored.insert(crate::IgnoreKey { node: id, rule: "RECOMMEND_VARIABLES".to_string(), property: "fills/0".to_string() });

        assert_eq!(linter_ids(&doc, page, "hardcoded-value"), vec![id.0], "the unignored stroke keeps the linter finding");
        assert!(ids_of(&doc, page, "//*[has-hardcoded('fills/0')]").is_empty(), "the ignored fill is not hard-coded for the query");
        assert_eq!(ids_of(&doc, page, "//*[has-hardcoded('strokes/0')]"), vec![id.0]);
        assert_eq!(ids_of(&doc, page, "//*[has-hardcoded('*')]"), vec![id.0]);
    }

    /// Number variables with a GAP scope, enough for the spacing scale and the token index to be costly to rebuild.
    fn add_gap_variables(doc: &mut Document, count: usize) {
        use crate::{Variable, VariableCollection, VariableKind, VariableMode, VariableValue};
        doc.variable_collections.insert(
            "space".into(),
            VariableCollection {
                id: "space".into(),
                name: "space".into(),
                modes: vec![VariableMode { id: "m".into(), name: "m".into() }],
                default_mode: "m".into(),
                variables: Vec::new(),
                key: None,
                source_library_key: None,
                extensions: Default::default(),
            },
        );
        for i in 0..count {
            let id = format!("space-{i}");
            doc.variable_collections.get_mut("space").unwrap().variables.push(id.clone());
            doc.variables.insert(
                id.clone(),
                Variable {
                    id: id.clone(),
                    name: format!("space/{i}"),
                    collection: "space".into(),
                    kind: VariableKind::Number,
                    values: [("m".to_string(), VariableValue::Number(i as f32 + 1.0))].into_iter().collect(),
                    description: String::new(),
                    scopes: vec!["GAP".to_string()],
                    hidden_from_publishing: false,
                    key: None,
                    code_syntax: Default::default(),
                    extensions: Default::default(),
                    mode_extensions: Default::default(),
                },
            );
        }
    }

    #[test]
    fn token_and_scale_predicates_build_document_state_once_per_call() {
        // Rebuilding the token index and scales for every candidate node costs seconds on a
        // document of this size in a release build and minutes in a debug build; built once per
        // call, the whole query is milliseconds. The ceiling is far above the fixed cost so a loaded
        // machine does not trip it, and far below the cost of rebuilding per node.
        let (mut doc, page) = build_synthetic_doc(600, 4);
        add_gap_variables(&mut doc, 3_000);
        for selector in ["//*[has-problem('hardcoded-value')]", "//*[has-problem('off-scale')]", "//*[@problems]"] {
            let started = std::time::Instant::now();
            let res = run(&doc, &QueryOptions { selector: selector.to_string(), page: Some(page), max_scanned: Some(200_000), ..Default::default() }).unwrap();
            assert_eq!(res.scanned_total, res.scanned, "{selector}");
            assert!(started.elapsed() < std::time::Duration::from_secs(10), "{selector} took {:?}", started.elapsed());
        }
    }

    #[test]
    fn single_node_linter_matches_the_one_shot_call_node_after_node() {
        let (mut doc, page) = build_synthetic_doc(20, 4);
        add_gap_variables(&mut doc, 50);
        for node in doc.nodes.values_mut().filter(|n| matches!(n.kind, NodeKind::Rectangle)) {
            node.fills = vec![crate::Paint::solid(crate::Color::rgb(200, 50, 50))];
            node.corner_radii = [7.0; 4];
        }
        let cfg = LintConfig::from_preset(LintPreset::Agent);
        let linter = SingleNodeLinter::new(&doc, &cfg);
        let mut compared = 0;
        let mut stack = vec![page];
        while let Some(id) = stack.pop() {
            let node = doc.get(id).unwrap();
            stack.extend(node.children.iter().copied());
            let reused: Vec<(&str, Option<String>)> = linter.lint(id, &NoLintEnv).iter().map(|f| (f.rule, f.property.clone())).collect();
            let fresh: Vec<(&str, Option<String>)> =
                crate::lint::lint_single_node(&doc, id, &cfg, &NoLintEnv).iter().map(|f| (f.rule, f.property.clone())).collect();
            assert_eq!(reused, fresh, "node {}", id.0);
            compared += 1;
        }
        assert!(compared > 80);
    }

    #[test]
    fn problem_and_sibling_predicates_respect_max_scanned_and_lint_only_candidate_node() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        struct CountingEnv(AtomicUsize);
        impl LintEnv for CountingEnv {
            fn has_text_engine(&self) -> bool {
                true
            }
            fn text_metrics(&self, node: &Node, props: &crate::TextProps, size: crate::Vec2) -> Option<crate::lint::TextMetrics> {
                self.0.fetch_add(1, Ordering::SeqCst);
                crate::lint::ApproxLintEnv.text_metrics(node, props, size)
            }
        }

        let (doc, page_id) = build_synthetic_doc(80, 4);
        // 1. Single-row query with has-problem must NOT lint all 160 text nodes on the page.
        let env = CountingEnv(AtomicUsize::new(0));
        let single = run_with_env(
            &doc,
            &QueryOptions {
                selector: "//TEXT[has-problem('text-truncated')]".to_string(),
                page: Some(page_id),
                limit: Some(1),
                max_scanned: Some(30),
                ..Default::default()
            },
            Some(&env),
        )
        .unwrap();
        assert!(single.scanned <= 30, "scanned={} exceeded max_scanned=30", single.scanned);
        assert!(env.0.load(Ordering::SeqCst) <= 2, "expected single-node linting (<=2 text_metrics calls), got {}", env.0.load(Ordering::SeqCst));

        // 2. Positional sibling predicate on a parent with 100 children must stop at max_scanned=40.
        let mut wide_doc = Document::new("Wide Siblings");
        let wide_page = wide_doc.add_page("Page 1");
        let parent = wide_doc.alloc_id();
        wide_doc.insert(wide_page, Node::frame(parent, Rect::new(0.0, 0.0, 500.0, 500.0))).unwrap();
        for i in 0..100 {
            let cid = wide_doc.alloc_id();
            wide_doc.insert(parent, Node::rectangle(cid, Rect::new(i as f32 * 4.0, 0.0, 10.0, 10.0))).unwrap();
        }
        let pos_page =
            run(&wide_doc, &QueryOptions { selector: "//RECTANGLE[80]".to_string(), page: Some(wide_page), max_scanned: Some(40), ..Default::default() })
                .unwrap();
        assert!(pos_page.scanned <= 40, "positional predicate scanned={} exceeded max_scanned=40", pos_page.scanned);
        assert!(!pos_page.done);
    }

    /// Hero frame (with a headline and a "hero bg" rectangle), a Card frame (with a "Card title" text
    /// and a rectangle named "Text field") and a Card_2 frame, in that pre-order.
    struct NamedDoc {
        doc: Document,
        page: NodeId,
        hero: u64,
        headline: u64,
        hero_bg: u64,
        card: u64,
        card_title: u64,
        text_field: u64,
        card_two: u64,
    }

    fn named_doc() -> NamedDoc {
        let mut doc = Document::new("Bare Names");
        let page = doc.add_page("Page 1");
        let frame = |doc: &mut Document, parent: NodeId, name: &str| {
            let id = doc.alloc_id();
            let mut n = Node::frame(id, Rect::new(0.0, 0.0, 100.0, 100.0));
            n.name = name.to_string();
            doc.insert(parent, n).unwrap();
            id
        };
        let text = |doc: &mut Document, parent: NodeId, name: &str| {
            let id = doc.alloc_id();
            let mut n = Node::text(id, Rect::new(0.0, 0.0, 80.0, 20.0), "Welcome", TextStyle::default());
            n.name = name.to_string();
            doc.insert(parent, n).unwrap();
            id
        };
        let rectangle = |doc: &mut Document, parent: NodeId, name: &str| {
            let id = doc.alloc_id();
            let mut n = Node::rectangle(id, Rect::new(0.0, 24.0, 80.0, 20.0));
            n.name = name.to_string();
            doc.insert(parent, n).unwrap();
            id
        };
        let hero = frame(&mut doc, page, "Hero Banner");
        let headline = text(&mut doc, hero, "Headline");
        let hero_bg = rectangle(&mut doc, hero, "hero bg");
        let card = frame(&mut doc, page, "Card");
        let card_title = text(&mut doc, card, "Card title");
        let text_field = rectangle(&mut doc, card, "Text field");
        let card_two = frame(&mut doc, page, "Card_2");
        NamedDoc {
            doc,
            page,
            hero: hero.0,
            headline: headline.0,
            hero_bg: hero_bg.0,
            card: card.0,
            card_title: card_title.0,
            text_field: text_field.0,
            card_two: card_two.0,
        }
    }

    #[test]
    fn bare_word_selectors_match_node_names_by_substring_as_the_previous_query_did() {
        let d = named_doc();
        let ids = |selector: &str| ids_of(&d.doc, d.page, selector);
        assert_eq!(ids("//Card"), vec![d.card, d.card_title, d.card_two], "`//Card` matches every name that contains it");
        assert_eq!(ids("Hero"), vec![d.hero, d.hero_bg], "a bare word without slashes does the same, ignoring case");
        assert_eq!(ids("//hero"), vec![d.hero, d.hero_bg]);
        assert_eq!(ids("Hero Banner"), vec![d.hero], "a bare name may hold spaces");
        assert_eq!(ids("//Hero//TEXT"), vec![d.headline], "a name step can be an ancestor step");
        assert_eq!(ids("//Card[@width>90]"), vec![d.card, d.card_two], "a name step takes predicates");
        assert!(ids("//Zebra").is_empty(), "a name nothing carries is an honest empty answer");
        // Type words stay node tests (E15-17 3.1): the rectangle named "Text field" is not a TEXT.
        assert_eq!(ids("//TEXT"), vec![d.headline, d.card_title]);
        assert!(!ids("//TEXT").contains(&d.text_field));
    }

    #[test]
    fn a_name_with_spaces_inside_a_longer_selector_is_a_named_error_with_a_working_example() {
        let d = named_doc();
        let err = run(&d.doc, &QueryOptions { selector: "//Hero Banner//TEXT".to_string(), page: Some(d.page), ..Default::default() }).unwrap_err();
        let QueryError::InvalidSelector { corrected_example, .. } = err else { panic!("expected INVALID_SELECTOR, got {err:?}") };
        assert_eq!(corrected_example, "//*[contains(@name,'Hero Banner')]//TEXT");
        assert_eq!(ids_of(&d.doc, d.page, &corrected_example), vec![d.headline], "the example runs and answers the question");
    }

    /// A page holding, in pre-order: the frame "Button group" (with the text "Label"), the frames
    /// "Home page" and "Page header", a boolean layer "Combined", the rectangle "Text field" and a
    /// group "Cluster". No node other than "Cluster" is of kind GROUP, and the page node is the only PAGE.
    struct KindWordDoc {
        doc: Document,
        page: NodeId,
        button_group: u64,
        label: u64,
        home: u64,
        header: u64,
        combined: u64,
        text_field: u64,
        cluster: u64,
    }

    fn kind_word_doc() -> KindWordDoc {
        let mut doc = Document::new("Kind Words");
        let page = doc.add_page("Page 1");
        let frame = |doc: &mut Document, parent: NodeId, name: &str| {
            let id = doc.alloc_id();
            let mut n = Node::frame(id, Rect::new(0.0, 0.0, 100.0, 100.0));
            n.name = name.to_string();
            doc.insert(parent, n).unwrap();
            id
        };
        let layer = |doc: &mut Document, kind: NodeKind, name: &str| {
            let id = doc.alloc_id();
            let mut n = Node::new(id, kind);
            n.name = name.to_string();
            doc.insert(page, n).unwrap();
            id
        };
        let button_group = frame(&mut doc, page, "Button group");
        let label_id = doc.alloc_id();
        let mut label = Node::text(label_id, Rect::new(0.0, 0.0, 80.0, 20.0), "Pay", TextStyle::default());
        label.name = "Label".to_string();
        doc.insert(button_group, label).unwrap();
        let home = frame(&mut doc, page, "Home page");
        let header = frame(&mut doc, page, "Page header");
        let combined = layer(&mut doc, NodeKind::BooleanOp(crate::BooleanOp::Union), "Combined");
        let field_id = doc.alloc_id();
        let mut field = Node::rectangle(field_id, Rect::new(0.0, 24.0, 80.0, 20.0));
        field.name = "Text field".to_string();
        doc.insert(page, field).unwrap();
        let cluster = layer(&mut doc, NodeKind::Group, "Cluster");
        KindWordDoc {
            doc,
            page,
            button_group: button_group.0,
            label: label_id.0,
            home: home.0,
            header: header.0,
            combined: combined.0,
            text_field: field_id.0,
            cluster: cluster.0,
        }
    }

    /// What release's `matches_single_step` answered for a bare word: the scope root and everything
    /// below it, in pre-order, whose default kind name equals the word or whose name contains it.
    fn release_bare_word_ids(doc: &Document, root: NodeId, word: &str) -> Vec<u64> {
        let mut ids = Vec::new();
        doc.walk(root, |n, _| {
            if n.kind.default_name().eq_ignore_ascii_case(word) || n.name.to_lowercase().contains(&word.to_lowercase()) {
                ids.push(n.id.0);
            }
        });
        ids
    }

    #[test]
    fn a_type_word_in_any_other_casing_matches_the_kind_or_a_name_as_release_did() {
        let d = kind_word_doc();
        let ids = |selector: &str| ids_of(&d.doc, d.page, selector);
        assert_eq!(ids("//Group"), vec![d.button_group, d.cluster], "`//Group` finds the frame \"Button group\" and the group");
        assert_eq!(ids("Group"), vec![d.button_group, d.cluster], "a bare word without slashes does the same");
        assert_eq!(ids("//group"), vec![d.button_group, d.cluster]);
        assert_eq!(ids("//Text"), vec![d.label, d.text_field], "`//Text` finds the text and the rectangle \"Text field\"");
        for word in ["Group", "group", "Text", "Page", "Line", "Frame"] {
            assert_eq!(ids(&format!("//{word}")), release_bare_word_ids(&d.doc, d.page, word), "//{word} against release");
        }
        assert_eq!(ids("//Group//TEXT"), vec![d.label], "a kind-or-name step can be an ancestor step");
    }

    #[test]
    fn a_boolean_layer_answers_to_union_by_kind_as_release_did() {
        let d = kind_word_doc();
        let ids = |selector: &str| ids_of(&d.doc, d.page, selector);
        assert_eq!(ids("//Union"), vec![d.combined], "`//Union` finds boolean layers by kind, whatever they are named");
        assert_eq!(ids("Union"), vec![d.combined]);
        assert_eq!(ids("//UNION"), vec![d.combined], "`UNION` is not a kind token of this tool, so it keeps release's meaning");
        for word in ["Union", "UNION", "union"] {
            assert_eq!(ids(&format!("//{word}")), release_bare_word_ids(&d.doc, d.page, word), "//{word} against release");
        }
    }

    #[test]
    fn the_exact_upper_case_type_token_matches_by_kind_only() {
        let d = kind_word_doc();
        let ids = |selector: &str| ids_of(&d.doc, d.page, selector);
        assert_eq!(ids("//GROUP"), vec![d.cluster], "`//GROUP` does not find the frame \"Button group\"");
        assert_eq!(ids("//TEXT"), vec![d.label], "the rectangle \"Text field\" is not a TEXT");
        assert_eq!(ids("//FRAME"), vec![d.button_group, d.home, d.header]);
        assert!(ids("//LINE").is_empty(), "no LINE node and no name decides");
        assert_eq!(ids("//BOOLEAN_OPERATION"), vec![d.combined]);
        assert_eq!(ids("//PAGE"), vec![d.page.0], "the page is the only PAGE node, and `//PAGE` is not always empty");
    }

    #[test]
    fn a_bare_page_word_returns_the_page_and_the_names_release_returned() {
        let d = kind_word_doc();
        let release = release_bare_word_ids(&d.doc, d.page, "Page");
        assert_eq!(release, vec![d.page.0, d.home, d.header], "release walked the scope root, so the page came first");
        assert_eq!(ids_of(&d.doc, d.page, "//Page"), release);
        assert_eq!(ids_of(&d.doc, d.page, "Page"), release);
        assert_eq!(ids_of(&d.doc, d.page, "//page"), release);
        assert_eq!(ids_of(&d.doc, d.page, "//Page//TEXT"), vec![d.label], "the page is an ancestor of every text on it");
        assert_eq!(ids_of(&d.doc, d.page, "//*"), vec![d.button_group, d.label, d.home, d.header, d.combined, d.text_field, d.cluster], "`//*` is unchanged");

        let unscoped = run(&d.doc, &QueryOptions { selector: "//Page".to_string(), ..Default::default() }).unwrap();
        let unscoped_ids: Vec<u64> = unscoped.rows.iter().map(|r| r[0].as_u64().unwrap()).collect();
        assert_eq!(unscoped_ids, release, "without a scope the document's pages are searched");
        assert_eq!(unscoped.scanned_total, unscoped.scope_size, "every node the query visited is part of the scope");
    }

    #[test]
    fn a_bare_page_word_pages_through_the_page_and_the_names_once_each() {
        let d = kind_word_doc();
        let mut seen = Vec::new();
        let mut cursor = None;
        for _ in 0..10 {
            let res = run(
                &d.doc,
                &QueryOptions { selector: "//Page//*".to_string(), page: Some(d.page), limit: Some(2), cursor: cursor.clone(), ..Default::default() },
            )
            .unwrap();
            seen.extend(res.rows.iter().map(|r| r[0].as_u64().unwrap()));
            if res.done {
                assert_eq!(res.remaining, 0);
                break;
            }
            cursor = res.cursor;
        }
        let everything_below = vec![d.button_group, d.label, d.home, d.header, d.combined, d.text_field, d.cluster];
        assert_eq!(seen, everything_below, "the page is an ancestor of every layer on it");
        assert_eq!(ids_of(&d.doc, d.page, "//Page//*"), everything_below, "paging with a cursor returns the same rows as one call");

        let mut seen = Vec::new();
        let mut cursor = None;
        for _ in 0..10 {
            let res =
                run(&d.doc, &QueryOptions { selector: "//Page".to_string(), page: Some(d.page), limit: Some(1), cursor: cursor.clone(), ..Default::default() })
                    .unwrap();
            seen.extend(res.rows.iter().map(|r| r[0].as_u64().unwrap()));
            if res.done {
                break;
            }
            cursor = res.cursor;
        }
        assert_eq!(seen, vec![d.page.0, d.home, d.header]);
    }

    #[test]
    fn predicates_on_a_page_root_answer_like_they_do_on_any_node() {
        let d = kind_word_doc();
        let run_page = |selector: &str, hardcoded: Option<bool>, with_problems: bool| {
            run(&d.doc, &QueryOptions { selector: selector.to_string(), page: Some(d.page), hardcoded, with_problems, ..Default::default() }).unwrap()
        };
        assert!(run_page("//Page[has-problem('hardcoded-value')]", None, false).rows.is_empty());
        assert!(run_page("//PAGE[@problems]", None, false).rows.is_empty());
        assert!(run_page("//PAGE[is-bound('fills/0')]", None, false).rows.is_empty());
        assert!(run_page("//PAGE", Some(true), false).rows.is_empty(), "no finding on the page, so `hardcoded: true` drops it");
        let with_problems = run_page("//PAGE", None, true);
        assert_eq!(with_problems.rows.len(), 1);
        assert_eq!(with_problems.rows[0][2], json!("PAGE"));
        assert_eq!(with_problems.rows[0][7], json!([]), "the problems column is computed, and the page has none");
        assert_eq!(run_page("//PAGE[@name='Page 1']", None, false).rows.len(), 1);
        assert!(run_page("//PAGE[@name='Other']", None, false).rows.is_empty());
    }

    #[test]
    fn contains_on_the_name_ignores_case_like_the_previous_query() {
        let d = named_doc();
        let ids = |selector: &str| ids_of(&d.doc, d.page, selector);
        assert_eq!(ids("//*[contains(@name,'card')]"), vec![d.card, d.card_title, d.card_two], "a lower-case needle finds \"Card\"");
        assert_eq!(ids("//*[contains(@name,'CARD')]"), vec![d.card, d.card_title, d.card_two]);
        assert_eq!(ids("//FRAME[contains(@name,'hero b')]"), vec![d.hero]);
        assert_eq!(ids("//*[contains(name,'TEXT F')]"), vec![d.text_field], "the attribute may be written without the @");
        assert!(ids("//*[contains(@name,'zebra')]").is_empty());
        // Only the name is folded: other string attributes keep their case.
        assert!(ids("//TEXT[contains(@characters,'WELCOME')]").is_empty());
        assert_eq!(ids("//TEXT[contains(@characters,'Welcome')]"), vec![d.headline, d.card_title]);
    }

    #[test]
    fn a_rule_unavailable_error_carries_no_retry_hint_of_its_own() {
        let err = QueryError::RuleUnavailable { rule: "text-truncated".to_string(), reason: "no_text_engine".to_string() };
        let text = err.to_string();
        assert!(text.starts_with("RULE_UNAVAILABLE: lint rule 'text-truncated' cannot be evaluated (no_text_engine)"), "{text}");
        assert!(!text.contains("safe_to_retry"), "the retry flag is the tool layer's, derived from the state change: {text}");
    }

    #[test]
    fn row_bounds_are_in_page_space_rounded_to_hundredths() {
        let mut doc = Document::new("Page Bounds Test");
        let page = doc.add_page("Page 1");
        let fid = doc.alloc_id();
        doc.insert(page, Node::frame(fid, Rect::new(100.0, 200.0, 300.0, 200.0))).unwrap();
        let tid = doc.alloc_id();
        doc.insert(fid, Node::text(tid, Rect::new(24.126, 16.004, 80.555, 20.0), "Hello", TextStyle::default())).unwrap();

        let res = run(&doc, &QueryOptions { selector: "//TEXT".to_string(), page: Some(page), ..Default::default() }).unwrap();
        assert_eq!(res.rows.len(), 1);
        let row = &res.rows[0];
        assert_eq!(row[3].as_f64().unwrap(), 124.13);
        assert_eq!(row[4].as_f64().unwrap(), 216.0);
        assert_eq!(row[5].as_f64().unwrap(), 80.56);
        assert_eq!(row[6].as_f64().unwrap(), 20.0);
    }

    #[test]
    fn in_progress_predicate_and_column_reflect_document_map_and_missing_store_errors() {
        let mut doc = Document::new("InProgress Test");
        let page = doc.add_page("Page 1");
        let f1 = doc.alloc_id();
        doc.insert(page, Node::frame(f1, Rect::new(0.0, 0.0, 100.0, 100.0))).unwrap();
        let f2 = doc.alloc_id();
        doc.insert(page, Node::frame(f2, Rect::new(120.0, 0.0, 100.0, 100.0))).unwrap();

        // Missing flag store (env = None and empty store) returns RULE_UNAVAILABLE
        let err = run_with_env(&doc, &QueryOptions { selector: "//*[@inProgress]".to_string(), page: Some(page), ..Default::default() }, None).unwrap_err();
        assert!(matches!(err, QueryError::RuleUnavailable { ref rule, .. } if rule == "inProgress"), "got {err:?}");

        // Populate Document.in_progress on f1
        doc.set_in_progress(
            f1,
            Some(crate::InProgress {
                run_id: Some("run-1".to_string()),
                agent: Some("Claude Code".to_string()),
                since_ms: doc.now_ms(),
                stale_after_s: None,
                status: None,
            }),
        );
        // A flag past its staleness window is no longer work in progress.
        doc.set_in_progress(f2, Some(crate::InProgress { run_id: None, agent: None, since_ms: 1_000, stale_after_s: Some(1), status: None }));

        let flagged = run(&doc, &QueryOptions { selector: "//*[@inProgress]".to_string(), page: Some(page), ..Default::default() }).unwrap();
        let flagged_ids: Vec<u64> = flagged.rows.iter().map(|r| r[0].as_u64().unwrap()).collect();
        assert_eq!(flagged_ids, vec![f1.0]);
        assert_eq!(flagged.rows[0][8], json!(true));

        let all_frames = run(&doc, &QueryOptions { selector: "//FRAME".to_string(), page: Some(page), ..Default::default() }).unwrap();
        assert_eq!(all_frames.rows.len(), 2);
        assert_eq!(all_frames.rows[0][0], json!(f1.0));
        assert_eq!(all_frames.rows[0][8], json!(true));
        assert_eq!(all_frames.rows[1][0], json!(f2.0));
        assert_eq!(all_frames.rows[1][8], json!(false));
    }
}
