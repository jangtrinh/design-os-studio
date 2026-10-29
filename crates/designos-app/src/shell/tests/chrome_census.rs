//! Chrome-colour census. An exact port of `scripts/rust_literal_scan.py` `clean`
//! (`strip_code` then `drop_cfg_test`). The helper applies no allowlist.

/// Blank comments and string or char literal bodies. Every newline stays.
pub fn strip_code(src: &str) -> String {
    let bytes = src.as_bytes();
    let mut out = String::with_capacity(src.len());
    let mut i = 0;
    let n = bytes.len();
    while i < n {
        if i + 1 < n && bytes[i] == b'/' && bytes[i + 1] == b'/' {
            while i < n && bytes[i] != b'\n' {
                i += 1;
            }
            continue;
        }
        if i + 1 < n && bytes[i] == b'/' && bytes[i + 1] == b'*' {
            let mut depth = 1;
            i += 2;
            while i < n && depth > 0 {
                if i + 1 < n && bytes[i] == b'/' && bytes[i + 1] == b'*' {
                    depth += 1;
                    i += 2;
                } else if i + 1 < n && bytes[i] == b'*' && bytes[i + 1] == b'/' {
                    depth -= 1;
                    i += 2;
                } else {
                    if bytes[i] == b'\n' {
                        out.push('\n');
                    }
                    i += 1;
                }
            }
            continue;
        }
        if let Some((hashes, open_end)) = raw_string_at(src, i) {
            let end_mark = format!("\"{}", "#".repeat(hashes));
            let found = src[open_end..].find(&end_mark).map(|p| open_end + p);
            let end = found.map(|j| j + end_mark.len()).unwrap_or(n);
            let newlines = src[i..end].matches('\n').count();
            out.push_str("\"\"");
            for _ in 0..newlines {
                out.push('\n');
            }
            i = end;
            continue;
        }
        if bytes[i] == b'"' {
            let mut j = i + 1;
            while j < n && bytes[j] != b'"' {
                j += if bytes[j] == b'\\' { 2 } else { 1 };
            }
            let end = if j < n { j + 1 } else { n };
            let newlines = src[i..end].matches('\n').count();
            out.push_str("\"\"");
            for _ in 0..newlines {
                out.push('\n');
            }
            i = end;
            continue;
        }
        if bytes[i] == b'\'' {
            if let Some(len) = char_lit_len(src, i) {
                out.push_str("' '");
                i += len;
                continue;
            }
        }
        let ch = src[i..].chars().next().unwrap();
        out.push(ch);
        i += ch.len_utf8();
    }
    out
}

/// `b?r#*"…"` at a byte index, looking at the same 12-character window as the scanner.
fn raw_string_at(src: &str, i: usize) -> Option<(usize, usize)> {
    if i > 0 {
        let prev = src.as_bytes()[i - 1];
        if prev.is_ascii_alphanumeric() || prev == b'_' {
            return None;
        }
    }
    let mut end = (i + 12).min(src.len());
    while end > i && !src.is_char_boundary(end) {
        end -= 1;
    }
    let window = &src[i..end];
    let b = window.as_bytes();
    let mut k = 0;
    if b.first() == Some(&b'b') && b.get(1) == Some(&b'r') {
        k = 1;
    }
    if b.get(k) != Some(&b'r') {
        return None;
    }
    k += 1;
    let hash_start = k;
    while b.get(k) == Some(&b'#') {
        k += 1;
    }
    if b.get(k) == Some(&b'"') {
        Some((k - hash_start, i + k + 1))
    } else {
        None
    }
}

/// `'x'` or `'\n'` / `'\u{..}'`. Lifetimes stay.
fn char_lit_len(src: &str, i: usize) -> Option<usize> {
    let rest = &src[i..];
    let mut chars = rest.chars();
    if chars.next() != Some('\'') {
        return None;
    }
    let second = chars.next()?;
    if second == '\\' {
        let escaped = chars.next()?;
        let mut extra = 0;
        for ch in chars {
            if ch == '\'' {
                return Some(1 + second.len_utf8() + escaped.len_utf8() + extra + 1);
            }
            extra += ch.len_utf8();
            if extra > 16 {
                return None;
            }
        }
        None
    } else if second != '\'' && chars.next() == Some('\'') {
        Some(1 + second.len_utf8() + 1)
    } else {
        None
    }
}

/// Blank every item annotated `#[cfg(test)]`, keeping line numbers.
pub fn drop_cfg_test(code: &str) -> String {
    let needle = "#[cfg(test)]";
    let mut code = code.to_string();
    loop {
        let Some(start) = code.find(needle) else {
            return code;
        };
        let bytes = code.as_bytes();
        let mut j = start + needle.len();
        let n = bytes.len();
        let mut depth = 0i32;
        let mut started = false;
        while j < n {
            match bytes[j] {
                b'{' => {
                    depth += 1;
                    started = true;
                    j += 1;
                }
                b'}' => {
                    depth -= 1;
                    j += 1;
                    if started && depth == 0 {
                        break;
                    }
                }
                b';' if !started => {
                    j += 1;
                    break;
                }
                _ => j += 1,
            }
        }
        let newlines = code[start..j].matches('\n').count();
        let mut next = String::with_capacity(code.len());
        next.push_str(&code[..start]);
        for _ in 0..newlines {
            next.push('\n');
        }
        next.push_str(&code[j..]);
        code = next;
    }
}

pub fn clean_rust(src: &str) -> String {
    drop_cfg_test(&strip_code(src))
}

/// 1-based line of every chrome colour site. Duplicates are kept. No allowlist.
pub fn chrome_colour_sites(src: &str) -> Vec<usize> {
    let clean = clean_rust(src);
    let mut sites = Vec::new();
    let mut line = 1usize;
    let bytes = clean.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'\n' {
            line += 1;
            i += 1;
            continue;
        }
        if let Some(consumed) = site_at(&clean[i..]) {
            sites.push(line);
            let mut k = 0;
            while k < consumed {
                if bytes[i + k] == b'\n' {
                    line += 1;
                }
                k += 1;
            }
            i += consumed;
            continue;
        }
        i += 1;
    }
    sites
}

fn site_at(rest: &str) -> Option<usize> {
    const PREFIX: &str = "Color32::";
    if !rest.starts_with(PREFIX) {
        return None;
    }
    let after = &rest[PREFIX.len()..];
    const CTORS: [&str; 6] =
        ["from_rgba_premultiplied(", "from_rgb_premultiplied(", "from_rgba_unmultiplied(", "from_rgb_unmultiplied(", "from_rgba(", "from_rgb("];
    for ctor in CTORS {
        if let Some(tail) = after.strip_prefix(ctor) {
            return integer_then(tail, b',').map(|n| PREFIX.len() + ctor.len() + n);
        }
    }
    if let Some(tail) = after.strip_prefix("from_gray(") {
        return integer_then(tail, b')').map(|n| PREFIX.len() + "from_gray(".len() + n);
    }
    for name in ["from_black_alpha(", "from_white_alpha("] {
        if after.starts_with(name) {
            return Some(PREFIX.len() + name.len());
        }
    }
    const NAMES: [&str; 9] = ["LIGHT_GRAY", "DARK_GRAY", "WHITE", "BLACK", "GRAY", "YELLOW", "GREEN", "BLUE", "RED"];
    for name in NAMES {
        if let Some(tail) = after.strip_prefix(name) {
            let next = tail.chars().next();
            if next.is_none_or(|c| !(c.is_ascii_alphanumeric() || c == '_')) {
                return Some(PREFIX.len() + name.len());
            }
        }
    }
    None
}

/// Optional whitespace, an integer literal, optional whitespace, then `end`.
fn integer_then(s: &str, end: u8) -> Option<usize> {
    let b = s.as_bytes();
    let mut i = 0;
    while i < b.len() && b[i].is_ascii_whitespace() {
        i += 1;
    }
    let start = i;
    if i + 1 < b.len() && b[i] == b'0' && (b[i + 1] == b'x' || b[i + 1] == b'X') {
        i += 2;
        let hex = i;
        while i < b.len() && b[i].is_ascii_hexdigit() {
            i += 1;
        }
        if i == hex {
            return None;
        }
    } else {
        while i < b.len() && b[i].is_ascii_digit() {
            i += 1;
        }
        if i == start {
            return None;
        }
    }
    while i < b.len() && b[i].is_ascii_whitespace() {
        i += 1;
    }
    if i < b.len() && b[i] == end {
        Some(i + 1)
    } else {
        None
    }
}

/// A cleaned line whose white is a texture tint, not chrome.
pub fn texture_tint_line(clean_line: &str) -> bool {
    clean_line.contains("Color32::WHITE") && (clean_line.contains(".image(") || clean_line.contains("with_texture"))
}

#[test]
fn helper_blanks_only_the_cfg_test_item() {
    let a = "#[cfg(test)]\nmod x;\nlet c = Color32::from_rgb(1, 2, 3);\n";
    assert_eq!(chrome_colour_sites(a), vec![3]);
    let b = "#[cfg(test)]\nmod t {\n    fn f() { let _ = Color32::RED; }\n}\nlet g = Color32::from_gray(10);\n";
    assert_eq!(chrome_colour_sites(b), vec![5]);
}

#[test]
fn helper_skips_comments_and_strings() {
    let a = "// Color32::WHITE\nlet s = \"Color32::WHITE\";\n/* Color32::BLACK\n */ let c = 'x';\n";
    assert_eq!(chrome_colour_sites(a), Vec::<usize>::new());
    let b = "a(Color32::WHITE, Color32::BLACK);\n";
    assert_eq!(chrome_colour_sites(b), vec![1, 1]);
    assert!(chrome_colour_sites("let c = Color32::TRANSPARENT;\n").is_empty());
    assert!(chrome_colour_sites("let c = Color32::from_rgb(r, g, b);\n").is_empty());
}

#[test]
#[ignore]
fn chrome_census_print() {
    let files = std::env::var("CHROME_CENSUS_FILES").unwrap_or_default();
    for file in files.split_whitespace() {
        let path = format!("crates/designos-app/src/{file}");
        let src = std::fs::read_to_string(&path).unwrap_or_else(|err| panic!("{path}: {err}"));
        let sites = chrome_colour_sites(&src);
        let mut uniq = sites.clone();
        uniq.sort();
        uniq.dedup();
        let list = uniq.iter().map(|n| n.to_string()).collect::<Vec<_>>().join(", ");
        println!("{file} chrome_sites={} [{list}]", sites.len());
    }
}
