//! Optional `---` front matter with `title`, `description` and `group`. The raw text is the source of truth and is
//! written back verbatim, so keys Studio does not know survive; the three known keys are read from it. A value
//! Studio cannot use is left unset and reported in [`FrontMatter::problems`]; it never hides the page body.

use crate::group::Group;

/// A front matter value that was not used, with the field and how to repair it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FrontMatterProblem {
    pub field: String,
    pub problem: String,
    pub fix: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FrontMatter {
    /// From the opening `---` through the closing `---` line, including its newline (after any byte order mark).
    pub(crate) raw: String,
    title: Option<String>,
    description: Option<String>,
    group: Option<Group>,
    problems: Vec<FrontMatterProblem>,
}

impl FrontMatter {
    pub(crate) fn empty() -> Self {
        Self { raw: "---\n---\n".into(), title: None, description: None, group: None, problems: Vec::new() }
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

    /// Known keys whose value could not be used. Show them next to the page; the page itself is fine.
    pub fn problems(&self) -> &[FrontMatterProblem] {
        &self.problems
    }

    pub fn raw(&self) -> &str {
        &self.raw
    }

    /// Splits `text` (without a byte order mark) into its front matter and the rest. Front matter is a closed `---`
    /// block whose lines are all `key: value` lines, comments or indented continuations, with at least one key; a
    /// page that opens with a horizontal rule is not front matter.
    pub(crate) fn split(text: &str) -> (Option<FrontMatter>, &str) {
        let mut lines = text.split_inclusive('\n');
        let Some(opening) = lines.next().filter(|l| line_content(l) == "---") else { return (None, text) };
        let mut end = opening.len();
        let mut inner: Vec<&str> = Vec::new();
        for line in lines {
            end += line.len();
            if line_content(line) == "---" {
                if !looks_like_front_matter(&inner) {
                    return (None, text);
                }
                return (Some(Self::from_raw(&text[..end])), &text[end..]);
            }
            inner.push(line_content(line));
        }
        (None, text)
    }

    fn from_raw(raw: &str) -> Self {
        let mut fm = Self { raw: raw.to_string(), title: None, description: None, group: None, problems: Vec::new() };
        for line in top_level_lines(raw) {
            let Some((key, value)) = line_content(line).split_once(':') else { continue };
            let key = key.trim();
            if !matches!(key, "title" | "description" | "group") {
                continue;
            }
            let parsed = match scalar(value) {
                Ok(v) => v,
                Err(problem) => {
                    fm.problems.push(FrontMatterProblem {
                        field: key.to_string(),
                        problem: problem.to_string(),
                        fix: format!("write `{key}: Some text` on one line, in double quotes if it contains `: ` or `#`"),
                    });
                    continue;
                }
            };
            match (key, parsed) {
                ("title", v) => fm.title = v,
                ("description", v) => fm.description = v,
                (_, None) => {}
                (_, Some(g)) => match Group::parse(&g.trim().to_ascii_lowercase()) {
                    Some(group) => fm.group = Some(group),
                    None => fm.problems.push(FrontMatterProblem {
                        field: "group".into(),
                        problem: format!("is {g:?}, which is not a doc group"),
                        fix: format!("use one of {}", Group::list()),
                    }),
                },
            }
        }
        fm
    }

    /// Sets, replaces or (with `None`) removes one of the known keys, keeping every other line as it was.
    pub(crate) fn set(&mut self, key: &str, value: Option<&str>) {
        let eol = if self.raw.contains("\r\n") { "\r\n" } else { "\n" };
        let mut out = String::new();
        let mut replaced = false;
        let mut first = true;
        // Set while the lines of a replaced key's old value (`title: |` and its indented text) are being dropped.
        let mut dropping_value = false;
        for line in self.raw.split_inclusive('\n') {
            if dropping_value && line.starts_with([' ', '\t']) {
                continue;
            }
            dropping_value = false;
            let is_closing = !first && line_content(line) == "---";
            first = false;
            let is_key = !line.starts_with([' ', '\t', '#']) && line_content(line).split_once(':').is_some_and(|(k, _)| k.trim() == key);
            if is_key && !replaced {
                replaced = true;
                dropping_value = true;
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
        *self = Self::from_raw(&out);
    }
}

fn looks_like_front_matter(inner: &[&str]) -> bool {
    let mut keys = 0;
    for line in inner {
        if line.trim().is_empty() || line.starts_with('#') || line.starts_with([' ', '\t']) || line.starts_with("- ") {
            continue;
        }
        if is_key_line(line) {
            keys += 1;
        } else {
            return false;
        }
    }
    keys > 0 || inner.is_empty()
}

fn is_key_line(line: &str) -> bool {
    let Some((key, rest)) = line.split_once(':') else { return false };
    let key = key.trim_end();
    key.starts_with(|c: char| c.is_ascii_alphabetic() || c == '_')
        && key.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.'))
        && (rest.is_empty() || rest.starts_with([' ', '\t']))
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

/// One-line YAML scalar: plain, `"double"` (with the standard escapes) or `'single'`. Anything that needs several
/// lines is refused so it is never silently read as something else.
fn scalar(value: &str) -> Result<Option<String>, &'static str> {
    let v = value.trim();
    if v.is_empty() {
        return Ok(None);
    }
    if let Some(rest) = v.strip_prefix('"') {
        return double_quoted(rest).map(Some);
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
                return if tail.trim().is_empty() || tail.trim().starts_with('#') { Ok(Some(out)) } else { Err("has text after the closing quote") };
            }
            out.push(c);
        }
        return Err("has a single-quoted value that is never closed");
    }
    if v.starts_with(['|', '>', '[', '{', '&', '*', '!', '%', '@', '`']) {
        return Err("uses YAML syntax Studio does not read here (block, list, anchor or tag)");
    }
    let plain = v.split_once(" #").map_or(v, |(before, _)| before).trim_end();
    Ok(Some(plain.to_string()))
}

fn double_quoted(rest: &str) -> Result<String, &'static str> {
    let mut out = String::new();
    let mut chars = rest.chars();
    while let Some(c) = chars.next() {
        match c {
            '\\' => {
                let escaped = match chars.next() {
                    Some('n') => '\n',
                    Some('t') => '\t',
                    Some('r') => '\r',
                    Some('0') => '\0',
                    Some(c @ ('"' | '\\' | '/' | ' ')) => c,
                    Some('x') => hex_char(&mut chars, 2)?,
                    Some('u') => hex_char(&mut chars, 4)?,
                    _ => return Err("has an unsupported escape in its double-quoted value"),
                };
                out.push(escaped);
            }
            '"' => {
                let tail = chars.as_str().trim();
                return if tail.is_empty() || tail.starts_with('#') { Ok(out) } else { Err("has text after the closing quote") };
            }
            c => out.push(c),
        }
    }
    Err("has a double-quoted value that is never closed")
}

fn hex_char(chars: &mut std::str::Chars, digits: usize) -> Result<char, &'static str> {
    let hex: String = chars.take(digits).collect();
    u32::from_str_radix(&hex, 16).ok().and_then(char::from_u32).ok_or("has an invalid \\x or \\u escape in its double-quoted value")
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
        || v.contains(['\n', '\r', '"', '\t'])
        || v.ends_with(':')
        || v.starts_with(['-', '?', ':', ',', '[', ']', '{', '}', '#', '&', '*', '!', '|', '>', '\'', '%', '@', '`']);
    if !needs {
        return v.to_string();
    }
    let escaped = v.replace('\\', "\\\\").replace('"', "\\\"").replace('\n', "\\n").replace('\r', "").replace('\t', "\\t");
    format!("\"{escaped}\"")
}
