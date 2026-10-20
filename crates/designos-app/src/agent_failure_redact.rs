//! Redaction for "Copy details" on mid-session agent failures (`#686`).
//!
//! Removes secret/key values before any diagnostic text is placed on the clipboard:
//! 1. Provider key prefixes (`sk-ant-…`, `sk-proj-…`, `sk-…`, `xai-…`, `AIza…`, `ghp_…`, `gho_…`,
//!    `github_pat_…`, `glpat-…`, `ant-…`), including masked keys in error messages (`sk-ant-***9f2a`).
//! 2. `Authorization: Bearer <token>` and `Bearer <token>` headers.
//! 3. `NAME=value` / `NAME: value` / `"NAME": "value"` assignments where `NAME` contains `KEY`,
//!    `TOKEN`, `SECRET`, or `PASSWORD` (case-insensitive).
//! 4. Long base64 or hex runs after such a secret label, or standalone long hex/base64 secret tokens.

use crate::terminal::strip_ansi_codes;

pub const REDACTED: &str = "[redacted]";
pub const COPY_DETAILS_MAX_LINES: usize = 40;

const PROVIDER_PREFIXES: &[&str] =
    &["sk-ant-", "sk-proj-", "sk-live-", "sk-test-", "sk-", "xai-", "AIza", "ghp_", "gho_", "ghu_", "ghs_", "github_pat_", "glpat-", "ant-api"];

const SECRET_NAME_PARTS: &[&str] = &["KEY", "TOKEN", "SECRET", "PASSWORD"];

fn is_token_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.' | '*' | '+' | '/' | '=')
}

fn is_word_boundary_prev(prev: Option<char>) -> bool {
    match prev {
        None => true,
        Some(c) => !c.is_ascii_alphanumeric() && c != '_' && c != '-',
    }
}

/// Redacts all key/token/secret shapes in `line`, replacing each secret value with `[redacted]`.
pub fn redact_line(line: &str) -> String {
    let step1 = redact_bearer_tokens(line);
    let step2 = redact_named_secret_assignments(&step1);
    let step3 = redact_provider_prefixed_keys(&step2);
    redact_standalone_hex_runs(&step3)
}

/// Redacts a multi-line string line by line.
pub fn redact_text(text: &str) -> String {
    text.lines().map(redact_line).collect::<Vec<_>>().join("\n")
}

/// Formats the 4-part "Copy details" clipboard payload (`#686` §3.6):
/// 1. CLI name and version
/// 2. Failure class (or `"unknown"`)
/// 3. Last 40 non-empty lines of output (ANSI escapes stripped, each line redacted, never elided)
/// 4. Session ID
pub fn format_copy_details(cli_name_and_version: &str, class_slug: &str, raw_output: &str, session_id: &str) -> String {
    let cleaned_lines: Vec<String> = raw_output
        .split('\n')
        .map(|l| {
            let after_cr = l.rsplit('\r').find(|seg| !strip_ansi_codes(seg).trim().is_empty()).unwrap_or("");
            strip_ansi_codes(after_cr).trim_end().to_string()
        })
        .filter(|l| !l.trim().is_empty())
        .collect();
    let start = cleaned_lines.len().saturating_sub(COPY_DETAILS_MAX_LINES);
    let redacted_tail = redact_text(&cleaned_lines[start..].join("\n"));

    format!(
        "cli: {}\nclass: {}\nsession_id: {}\nlast_output:\n{}",
        redact_line(cli_name_and_version.trim()),
        class_slug.trim(),
        redact_line(session_id.trim()),
        redacted_tail
    )
}

fn redact_bearer_tokens(input: &str) -> String {
    let lower = input.to_ascii_lowercase();
    let mut out = String::with_capacity(input.len());
    let mut i = 0;
    while i < input.len() {
        if let Some(rel) = lower[i..].find("bearer ") {
            let start = i + rel;
            let prev = input[..start].chars().last();
            if is_word_boundary_prev(prev) {
                let bearer_end = start + "bearer ".len();
                out.push_str(&input[i..bearer_end]);
                let mut rest = bearer_end;
                while rest < input.len() && input.as_bytes()[rest] == b' ' {
                    rest += 1;
                }
                let mut tok_end = rest;
                for ch in input[rest..].chars() {
                    if is_token_char(ch) {
                        tok_end += ch.len_utf8();
                    } else {
                        break;
                    }
                }
                if tok_end > rest {
                    out.push_str(REDACTED);
                    i = tok_end;
                    continue;
                }
            }
            out.push_str(&input[i..start + "bearer ".len()]);
            i = start + "bearer ".len();
        } else {
            out.push_str(&input[i..]);
            break;
        }
    }
    out
}

fn name_has_secret_keyword(name: &str) -> bool {
    let upper = name.to_ascii_uppercase();
    SECRET_NAME_PARTS.iter().any(|kw| upper.contains(kw))
}

fn redact_named_secret_assignments(input: &str) -> String {
    let chars: Vec<(usize, char)> = input.char_indices().collect();
    let mut out = String::with_capacity(input.len());
    let mut idx = 0;
    while idx < chars.len() {
        let (byte_pos, ch) = chars[idx];
        if ch.is_ascii_alphabetic() || ch == '_' {
            let start_idx = idx;
            while idx < chars.len() && (chars[idx].1.is_ascii_alphanumeric() || chars[idx].1 == '_' || chars[idx].1 == '-') {
                idx += 1;
            }
            let name_end_byte = if idx < chars.len() { chars[idx].0 } else { input.len() };
            let name = &input[byte_pos..name_end_byte];
            if name_has_secret_keyword(name) {
                let mut sep_idx = idx;
                if sep_idx < chars.len() && (chars[sep_idx].1 == '"' || chars[sep_idx].1 == '\'') {
                    sep_idx += 1;
                }
                while sep_idx < chars.len() && chars[sep_idx].1 == ' ' {
                    sep_idx += 1;
                }
                if sep_idx < chars.len() && (chars[sep_idx].1 == '=' || chars[sep_idx].1 == ':') {
                    sep_idx += 1;
                    while sep_idx < chars.len() && chars[sep_idx].1 == ' ' {
                        sep_idx += 1;
                    }
                    let quote = if sep_idx < chars.len() && (chars[sep_idx].1 == '"' || chars[sep_idx].1 == '\'') {
                        let q = chars[sep_idx].1;
                        sep_idx += 1;
                        Some(q)
                    } else {
                        None
                    };
                    let val_start_idx = sep_idx;
                    if let Some(q) = quote {
                        while sep_idx < chars.len() && chars[sep_idx].1 != q {
                            sep_idx += 1;
                        }
                    } else {
                        while sep_idx < chars.len() && is_token_char(chars[sep_idx].1) {
                            sep_idx += 1;
                        }
                    }
                    if sep_idx > val_start_idx {
                        let prefix_end_byte = chars[val_start_idx].0;
                        out.push_str(&input[chars[start_idx].0..prefix_end_byte]);
                        out.push_str(REDACTED);
                        if quote.is_some() && sep_idx < chars.len() {
                            out.push(chars[sep_idx].1);
                            sep_idx += 1;
                        }
                        idx = sep_idx;
                        continue;
                    }
                }
            }
            out.push_str(&input[byte_pos..name_end_byte]);
        } else {
            out.push(ch);
            idx += 1;
        }
    }
    out
}

fn redact_provider_prefixed_keys(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut i = 0;
    while i < input.len() {
        let rest = &input[i..];
        let prev = input[..i].chars().last();
        if is_word_boundary_prev(prev) {
            if let Some(prefix) = PROVIDER_PREFIXES.iter().copied().find(|p| rest.starts_with(p)) {
                let mut end = i + prefix.len();
                for ch in input[end..].chars() {
                    if is_token_char(ch) || ch == '…' {
                        end += ch.len_utf8();
                    } else {
                        break;
                    }
                }
                if end - i >= prefix.len() + 3 {
                    out.push_str(REDACTED);
                    i = end;
                    continue;
                }
            }
        }
        let ch = rest.chars().next().unwrap();
        out.push(ch);
        i += ch.len_utf8();
    }
    out
}

fn redact_standalone_hex_runs(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut i = 0;
    while i < input.len() {
        let rest = &input[i..];
        let prev = input[..i].chars().last();
        if is_word_boundary_prev(prev) {
            let mut end = i;
            let mut hex_count = 0usize;
            for ch in rest.chars() {
                if ch.is_ascii_hexdigit() {
                    end += ch.len_utf8();
                    hex_count += 1;
                } else {
                    break;
                }
            }
            let next_ch = input[end..].chars().next();
            if hex_count >= 32 && is_word_boundary_prev(next_ch) {
                out.push_str(REDACTED);
                i = end;
                continue;
            }
        }
        let ch = rest.chars().next().unwrap();
        out.push(ch);
        i += ch.len_utf8();
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn copied_details_pass_a_redaction_test_with_planted_fake_keys() {
        let raw = concat!(
            "line 1: provider key sk-ant-api03-99887766554433221100aabbccddeeff\n",
            "line 2: Authorization: Bearer eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.fakePayload.fakeSig\n",
            "line 3: ANTHROPIC_API_KEY=super_secret_value_1234567890 and ACCESS_TOKEN=\"tok_abc123xyz987\"\n",
            "line 4: rotated SECRET_HEX=0123456789abcdef0123456789abcdef0123456789abcdef\n",
            "line 5: masked error key sk-proj-****abcd9876 and xai-1234****5678\n",
        );
        let details = format_copy_details("claude 1.0.0", "login_expired", raw, "p100-tab-1");
        for planted in [
            "sk-ant-api03-99887766554433221100aabbccddeeff",
            "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9",
            "super_secret_value_1234567890",
            "tok_abc123xyz987",
            "0123456789abcdef0123456789abcdef0123456789abcdef",
            "sk-proj-****abcd9876",
            "xai-1234****5678",
        ] {
            assert!(!details.contains(planted), "planted key {planted:?} leaked into details:\n{details}");
        }
        assert!(details.contains(REDACTED), "expected [redacted] marker in:\n{details}");
    }

    #[test]
    fn copied_details_have_the_four_parts() {
        let mut raw = String::new();
        for i in 1..=55 {
            raw.push_str(&format!("\x1b[31moutput line {i}\x1b[0m\n"));
        }
        let details = format_copy_details("codex 0.42.0", "rate_limited", &raw, "p999-tab-3");
        assert!(details.contains("cli: codex 0.42.0"), "missing cli name and version:\n{details}");
        assert!(details.contains("class: rate_limited"), "missing class:\n{details}");
        assert!(details.contains("session_id: p999-tab-3"), "missing session_id:\n{details}");
        let output_section = details.split("last_output:\n").nth(1).expect("last_output section");
        let lines: Vec<&str> = output_section.lines().collect();
        assert_eq!(lines.len(), COPY_DETAILS_MAX_LINES, "expected exactly 40 output lines");
        assert_eq!(lines.first().copied(), Some("output line 16"));
        assert_eq!(lines.last().copied(), Some("output line 55"));
    }
}
