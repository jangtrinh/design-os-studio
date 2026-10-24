//! The items of a source file that the scan of shipped code tells apart: attributes, where the item an attribute stands before ends, and
//! the `mod name;` declarations that put another file in the build.

/// An attribute: where it starts and ends in the text, and what is between its brackets (`!` first for an inner attribute `#![…]`).
pub(super) struct Attribute {
    pub(super) start: usize,
    pub(super) end: usize,
    pub(super) text: String,
}

/// Whether an item with this attribute is only in a test build: `#[test]`, `#[cfg(test)]` and `#[cfg(all(test, ..))]`. Any other
/// mention of `test` (`any(test, ..)`, `not(..)`, a feature named after it) can ship, so the item is scanned.
pub(super) fn is_test_attribute(text: &str) -> bool {
    let text = text.trim();
    let Some(condition) = text.strip_prefix("cfg(").and_then(|rest| rest.strip_suffix(')')).map(str::trim) else {
        return text == "test";
    };
    condition == "test"
        || condition.strip_prefix("all(").and_then(|args| args.strip_suffix(')')).is_some_and(|args| args.split(',').any(|arg| arg.trim() == "test"))
}

pub(super) fn attributes(shape: &[char], code: &[char]) -> Vec<Attribute> {
    let mut found = Vec::new();
    let mut i = 0;
    while i < shape.len() {
        let open = match (shape[i], shape.get(i + 1), shape.get(i + 2)) {
            ('#', Some('['), _) => Some(i + 1),
            ('#', Some('!'), Some('[')) => Some(i + 2),
            _ => None,
        };
        let close = open.and_then(|open| {
            let mut depth = 0;
            (open..shape.len()).find(|j| {
                depth += i32::from(shape[*j] == '[') - i32::from(shape[*j] == ']');
                depth == 0
            })
        });
        if let (Some(open), Some(close)) = (open, close) {
            let inner: String = code[open + 1..close].iter().collect();
            found.push(Attribute { start: i, end: close + 1, text: if open == i + 2 { format!("!{inner}") } else { inner } });
            i = close + 1;
        } else {
            i += 1;
        }
    }
    found
}

/// The index just after the item that starts at `from`: its closing `}`, or its `;`.
pub(super) fn item_end(shape: &[char], from: usize) -> usize {
    let (mut braces, mut others, mut seen_brace) = (0i32, 0i32, false);
    for (j, c) in shape.iter().enumerate().skip(from) {
        match c {
            '{' => (braces, seen_brace) = (braces + 1, true),
            '}' => {
                braces -= 1;
                if braces <= 0 && seen_brace {
                    return j + 1;
                }
            }
            '(' | '[' => others += 1,
            ')' | ']' => others -= 1,
            ';' if braces == 0 && others <= 0 => return j + 1,
            _ => {}
        }
    }
    shape.len()
}

/// The name of the module `item` declares when it is `mod name;`.
pub(super) fn file_module_name(item: &str) -> Option<String> {
    let item = item.trim();
    let rest = item.strip_prefix("pub").map(|r| r.trim_start()).unwrap_or(item);
    let rest = if rest.starts_with('(') { rest.split_once(')')?.1.trim_start() } else { rest };
    let name = rest.strip_prefix("mod ")?.strip_suffix(';')?.trim();
    (!name.is_empty() && name.chars().all(|c| c.is_alphanumeric() || c == '_')).then(|| name.to_string())
}

#[test]
fn only_an_item_that_needs_a_test_build_is_taken_for_test_only() {
    for text in ["test", "cfg(test)", " cfg(test) ", "cfg(all(test, unix))", "cfg(all(unix, test))"] {
        assert!(is_test_attribute(text), "{text}");
    }
    for text in [
        "cfg(not(test))",
        "cfg(any(test, unix))",
        "cfg(not(any(test, windows)))",
        "cfg(feature = \"test-support\")",
        "cfg(all(not(test), unix))",
        "cfg_attr(test, allow(dead_code))",
        "derive(Debug)",
    ] {
        assert!(!is_test_attribute(text), "{text}");
    }
}
