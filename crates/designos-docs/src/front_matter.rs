//! Optional `---` front matter with `title`, `description` and `group`. The raw text is the source of truth and is
//! written back verbatim, so keys Studio does not know survive; the three known keys are read from it.

use crate::error::{DocsError, Result};
use crate::group::Group;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FrontMatter {
    /// From the opening `---` through the closing `---` line, including its newline.
    pub(crate) raw: String,
    title: Option<String>,
    description: Option<String>,
    group: Option<Group>,
}

impl FrontMatter {
    pub(crate) fn empty() -> Self {
        Self { raw: "---\n---\n".into(), title: None, description: None, group: None }
    }

    pub fn title(&self) -> Option<&str> {
        self.title.as_deref()
    }

    pub fn description(&self) -> Option<&str> {
        self.description.as_deref()
    }

    pub fn group(&self) -> Option<Group> {
        self.group
    }

    pub fn raw(&self) -> &str {
        &self.raw
    }

    /// Splits `text` into its front matter (if it starts with a `---` block that is closed) and the rest.
    pub(crate) fn split(text: &str) -> Result<(Option<FrontMatter>, &str)> {
        let mut lines = text.split_inclusive('\n');
        let Some(opening) = lines.next().filter(|l| line_content(l) == "---") else { return Ok((None, text)) };
        let mut end = opening.len();
        for line in lines {
            end += line.len();
            if line_content(line) == "---" {
                let raw = &text[..end];
                return Ok((Some(Self::from_raw(raw)?), &text[end..]));
            }
        }
        Ok((None, text))
    }

    fn from_raw(raw: &str) -> Result<Self> {
        let (mut title, mut description, mut group) = (None, None, None);
        for line in top_level_lines(raw) {
            let Some((key, value)) = line_content(line).split_once(':') else { continue };
            match key.trim() {
                "title" => title = scalar("title", value)?,
                "description" => description = scalar("description", value)?,
                "group" => {
                    if let Some(g) = scalar("group", value)? {
                        group = Some(Group::parse(&g).ok_or_else(|| DocsError::FrontMatter {
                            field: "group".into(),
                            problem: format!("is {g:?}, which is not a doc group"),
                            fix: format!("use one of {}", Group::list()),
                        })?);
                    }
                }
                _ => {}
            }
        }
        Ok(Self { raw: raw.to_string(), title, description, group })
    }

    /// Sets, replaces or (with `None`) removes one of the known keys, keeping every other line as it was.
    pub(crate) fn set(&mut self, key: &str, value: Option<&str>) {
        let eol = if self.raw.contains("\r\n") { "\r\n" } else { "\n" };
        let mut out = String::new();
        let mut replaced = false;
        let mut first = true;
        for line in self.raw.split_inclusive('\n') {
            let is_closing = !first && line_content(line) == "---";
            first = false;
            let is_key = !line.starts_with([' ', '\t', '#']) && line_content(line).split_once(':').is_some_and(|(k, _)| k.trim() == key);
            if is_key && !replaced {
                replaced = true;
                if let Some(v) = value {
                    out.push_str(&format!("{key}: {}{eol}", quote(v)));
                }
                continue;
            }
            if is_closing && !replaced {
                replaced = true;
                if let Some(v) = value {
                    out.push_str(&format!("{key}: {}{eol}", quote(v)));
                }
            }
            out.push_str(line);
        }
        // Values written by `quote` always read back; keep the old fields if a hand-written line cannot.
        if let Ok(updated) = Self::from_raw(&out) {
            *self = updated;
        }
    }
}

/// A line without its `\n` / `\r\n`.
fn line_content(line: &str) -> &str {
    line.trim_end_matches(['\n', '\r'])
}

/// The key lines between the fences (not indented, not comments).
fn top_level_lines(raw: &str) -> impl Iterator<Item = &str> {
    let mut all: Vec<&str> = raw.split_inclusive('\n').collect();
    all.pop();
    all.into_iter().skip(1).filter(|l| !l.starts_with([' ', '\t', '#']))
}

/// One-line YAML scalar: plain, `"double"` or `'single'`. Anything that needs multiple lines is refused so it is
/// never silently read as something else.
fn scalar(field: &str, value: &str) -> Result<Option<String>> {
    let bad = |problem: &str| DocsError::FrontMatter {
        field: field.into(),
        problem: problem.into(),
        fix: format!("write `{field}: Some text` on one line, in double quotes if it contains `: ` or `#`"),
    };
    let v = value.trim();
    if v.is_empty() {
        return Ok(None);
    }
    if let Some(rest) = v.strip_prefix('"') {
        let mut out = String::new();
        let mut chars = rest.chars();
        while let Some(c) = chars.next() {
            match c {
                '\\' => match chars.next() {
                    Some('n') => out.push('\n'),
                    Some(other @ ('"' | '\\')) => out.push(other),
                    _ => return Err(bad("has an unsupported escape in its double-quoted value")),
                },
                '"' => {
                    let tail = chars.as_str().trim();
                    return if tail.is_empty() || tail.starts_with('#') { Ok(Some(out)) } else { Err(bad("has text after the closing quote")) };
                }
                c => out.push(c),
            }
        }
        return Err(bad("has a double-quoted value that is never closed"));
    }
    if let Some(rest) = v.strip_prefix('\'') {
        let mut out = String::new();
        let mut chars = rest.chars().peekable();
        while let Some(c) = chars.next() {
            if c == '\'' {
                if chars.peek() == Some(&'\'') {
                    chars.next();
                    out.push('\'');
                    continue;
                }
                let tail = chars.collect::<String>();
                return if tail.trim().is_empty() || tail.trim().starts_with('#') { Ok(Some(out)) } else { Err(bad("has text after the closing quote")) };
            }
            out.push(c);
        }
        return Err(bad("has a single-quoted value that is never closed"));
    }
    if v.starts_with(['|', '>', '[', '{', '&', '*', '!', '%', '@', '`']) {
        return Err(bad("uses YAML syntax Studio does not read here (block, list, anchor or tag)"));
    }
    let plain = v.split_once(" #").map_or(v, |(before, _)| before).trim_end();
    Ok(Some(plain.to_string()))
}

/// Quotes `v` only when a plain scalar would not read back as the same text.
fn quote(v: &str) -> String {
    let lower = v.to_ascii_lowercase();
    let reserved = matches!(lower.as_str(), "true" | "false" | "null" | "yes" | "no" | "on" | "off" | "~") || v.parse::<f64>().is_ok();
    let needs = v.is_empty()
        || reserved
        || v != v.trim()
        || v.contains(": ")
        || v.contains(" #")
        || v.contains(['\n', '\r', '"'])
        || v.ends_with(':')
        || v.starts_with(['-', '?', ':', ',', '[', ']', '{', '}', '#', '&', '*', '!', '|', '>', '\'', '%', '@', '`']);
    if !needs {
        return v.to_string();
    }
    let escaped = v.replace('\\', "\\\\").replace('"', "\\\"").replace('\n', "\\n").replace('\r', "");
    format!("\"{escaped}\"")
}
