//! The items of a source file that the scan of shipped code tells apart: attributes, where the item an attribute stands before ends, and
//! the `mod name;` declarations that put another file in the build.

/// An attribute: where it starts and ends in the text, and what is between its brackets (`!` first for an inner attribute `#![…]`).
pub(super) struct Attribute {
    pub(super) start: usize,
    pub(super) end: usize,
    pub(super) text: String,
}

/// Whether an item with this attribute is only in a test build: `#[test]`, `#[cfg(test)]` and `#[cfg(all(test, ..))]`, where `test` is
/// one of the arguments of the `all` itself. Any other mention of `test` (`any(test, ..)`, `all(any(.., test, ..))`, `not(..)`, a
/// feature named after it) can ship, so the item is scanned.
pub(super) fn is_test_attribute(text: &str) -> bool {
    let text = text.trim();
    let Some(condition) = text.strip_prefix("cfg(").and_then(|rest| rest.strip_suffix(')')).map(str::trim) else {
        return text == "test";
    };
    condition == "test"
        || condition
            .strip_prefix("all(")
            .and_then(|args| args.strip_suffix(')'))
            .is_some_and(|args| top_level_args(args).iter().any(|arg| arg.trim() == "test"))
}

/// The arguments of a call at its own level: `all(any(unix, test), unix)` has two, `any(unix, test)` and `unix`.
fn top_level_args(args: &str) -> Vec<&str> {
    let (mut depth, mut start, mut parts) = (0i32, 0, Vec::new());
    for (at, c) in args.char_indices() {
        match c {
            '(' => depth += 1,
            ')' => depth -= 1,
            ',' if depth == 0 => {
                parts.push(&args[start..at]);
                start = at + 1;
            }
            _ => {}
        }
    }
    parts.push(&args[start..]);
    parts
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

/// The words an item starts with. An attribute before anything else stands on a field, a variant, a match arm or an argument, which
/// ends at its `,`. (`pub` counts as an item word: a `pub` field then runs to the `}` of its struct, which holds no code.)
const ITEM_WORDS: &[&str] =
    &["fn", "mod", "impl", "use", "struct", "enum", "union", "const", "static", "type", "trait", "macro_rules", "pub", "unsafe", "async", "extern", "let"];

/// Whether what follows an attribute at `from` is a field, a variant, a match arm or an argument, not an item.
fn starts_like_field(shape: &[char], from: usize) -> bool {
    let head: String = shape[from..].iter().take(32).collect();
    let word: String = head.trim_start().chars().take_while(|c| c.is_alphanumeric() || *c == '_').collect();
    !ITEM_WORDS.contains(&word.as_str())
}

/// The index just after the thing that starts at `from`: an item ends at its closing `}` or its `;`, a field, variant, arm or argument
/// at its `,`. Nothing runs past the `}`, `)` or `]` that closes the block it sits in, which is not its own.
pub(super) fn item_end(shape: &[char], from: usize) -> usize {
    let field_like = starts_like_field(shape, from);
    let (mut braces, mut others, mut seen_brace) = (0i32, 0i32, false);
    for (j, c) in shape.iter().enumerate().skip(from) {
        match c {
            '{' => (braces, seen_brace) = (braces + 1, true),
            '}' => {
                if braces == 0 {
                    return j;
                }
                braces -= 1;
                if braces == 0 && seen_brace {
                    return j + 1;
                }
            }
            '(' | '[' => others += 1,
            ')' | ']' => {
                if others == 0 && braces == 0 {
                    return j;
                }
                others -= 1;
            }
            ';' if braces == 0 && others <= 0 => return j + 1,
            ',' if field_like && braces == 0 && others == 0 => return j + 1,
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
