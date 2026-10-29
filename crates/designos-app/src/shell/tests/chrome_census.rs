//! Chrome-colour census. Ports the Python cleaner so line numbers match it.

/// Blank comments and string or char literal bodies. Every newline stays.
fn strip_code(src: &str) -> String {
    let chars: Vec<char> = src.chars().collect();
    let n = chars.len();
    let mut out = String::new();
    let mut i = 0;
    while i < n {
        if i + 1 < n && chars[i] == '/' && chars[i + 1] == '/' {
            while i < n && chars[i] != '\n' {
                i += 1;
            }
            continue;
        }
        if i + 1 < n && chars[i] == '/' && chars[i + 1] == '*' {
            let mut depth = 1;
            i += 2;
            while i < n && depth > 0 {
                if i + 1 < n && chars[i] == '/' && chars[i + 1] == '*' {
                    depth += 1;
                    i += 2;
                } else if i + 1 < n && chars[i] == '*' && chars[i + 1] == '/' {
                    depth -= 1;
                    i += 2;
                } else {
                    if chars[i] == '\n' {
                        out.push('\n');
                    }
                    i += 1;
                }
            }
            continue;
        }
        let window: String = chars[i..].iter().take(12).collect();
        if let Some((match_len, end)) = raw_open(&window) {
            let prev_ok = i == 0 || !(chars[i - 1].is_alphanumeric() || chars[i - 1] == '_');
            if prev_ok {
                let search_from = i + match_len;
                // Search in chars like the Python cleaner: a byte offset into the tail would skip real code after a
                // raw string that holds multi-byte text.
                let end_chars: Vec<char> = end.chars().collect();
                let found = chars[search_from..].windows(end_chars.len()).position(|w| w == end_chars.as_slice());
                let end_at = found.map(|p| search_from + p).unwrap_or(n);
                let consumed_end = (end_at + end_chars.len()).min(n);
                let newlines = chars[i..consumed_end].iter().filter(|c| **c == '\n').count();
                out.push_str("\"\"");
                for _ in 0..newlines {
                    out.push('\n');
                }
                i = end_at + end_chars.len();
                continue;
            }
        }
        if chars[i] == '"' {
            let mut j = i + 1;
            while j < n && chars[j] != '"' {
                j += if chars[j] == '\\' { 2 } else { 1 };
            }
            let consumed_end = (j + 1).min(n);
            let newlines = chars[i..consumed_end].iter().filter(|c| **c == '\n').count();
            out.push_str("\"\"");
            for _ in 0..newlines {
                out.push('\n');
            }
            i = j + 1;
            continue;
        }
        if chars[i] == '\'' {
            let window: String = chars[i..].iter().take(12).collect();
            if let Some(len) = char_lit_len(&window) {
                out.push_str("' '");
                i += len;
                continue;
            }
        }
        out.push(chars[i]);
        i += 1;
    }
    out
}

/// `b?r#*"`, and the closing marker. `match_len` is in chars.
fn raw_open(slice: &str) -> Option<(usize, String)> {
    let chars: Vec<char> = slice.chars().collect();
    let mut i = 0;
    if chars.first() == Some(&'b') {
        i = 1;
    }
    if chars.get(i) != Some(&'r') {
        return None;
    }
    i += 1;
    let hash_at = i;
    while chars.get(i) == Some(&'#') {
        i += 1;
    }
    if chars.get(i) != Some(&'"') {
        return None;
    }
    let hashes: String = chars[hash_at..i].iter().collect();
    i += 1;
    Some((i, format!("\"{hashes}")))
}

/// Length in chars of a `'x'` or `'\n'` literal inside a 12-char window.
fn char_lit_len(slice: &str) -> Option<usize> {
    let chars: Vec<char> = slice.chars().collect();
    if chars.first() != Some(&'\'') {
        return None;
    }
    if chars.get(1) == Some(&'\\') && chars.len() > 2 {
        let mut k = 3;
        while k < chars.len() && chars[k] != '\'' {
            k += 1;
        }
        if k < chars.len() && chars[k] == '\'' {
            return Some(k + 1);
        }
    }
    if let Some(&c) = chars.get(1) {
        if c != '\\' && c != '\'' && chars.get(2) == Some(&'\'') {
            return Some(3);
        }
    }
    None
}

/// Blank every item annotated `#[cfg(test)]`. Line numbers stay put.
fn drop_cfg_test(code: &str) -> String {
    let mut code = code.to_string();
    loop {
        let Some(start) = code.find("#[cfg(test)]") else { return code };
        let bytes = code.as_bytes();
        let mut j = start + "#[cfg(test)]".len();
        let mut depth = 0i32;
        let mut started = false;
        while j < bytes.len() {
            match bytes[j] {
                b'{' => {
                    depth += 1;
                    started = true;
                    j += 1;
                }
                b'}' => {
                    depth -= 1;
                    if started && depth == 0 {
                        j += 1;
                        break;
                    }
                    j += 1;
                }
                b';' if !started => {
                    j += 1;
                    break;
                }
                _ => j += 1,
            }
        }
        let newlines = code[start..j].bytes().filter(|b| *b == b'\n').count();
        let mut next = String::new();
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

fn skip_ws(s: &str) -> &str {
    s.trim_start_matches(|c: char| c.is_whitespace())
}

fn take_int(s: &str) -> Option<&str> {
    let b = s.as_bytes();
    if b.len() >= 3 && b[0] == b'0' && (b[1] == b'x' || b[1] == b'X') {
        let mut k = 2;
        while k < b.len() && b[k].is_ascii_hexdigit() {
            k += 1;
        }
        if k > 2 {
            Some(&s[k..])
        } else {
            None
        }
    } else {
        let mut k = 0;
        while k < b.len() && b[k].is_ascii_digit() {
            k += 1;
        }
        if k > 0 {
            Some(&s[k..])
        } else {
            None
        }
    }
}

fn match_call(rest: &str) -> Option<usize> {
    const LONG: &[&str] =
        &["from_rgba_unmultiplied(", "from_rgb_unmultiplied(", "from_rgba_premultiplied(", "from_rgb_premultiplied(", "from_rgba(", "from_rgb("];
    for name in LONG {
        if let Some(after) = rest.strip_prefix(name) {
            let after_ws = skip_ws(after);
            let after_int = take_int(after_ws)?;
            let after_ws = skip_ws(after_int);
            if after_ws.starts_with(',') {
                let used = rest.len() - after_ws.len() + 1;
                return Some(used);
            }
            return None;
        }
    }
    if let Some(after) = rest.strip_prefix("from_gray(") {
        let after_ws = skip_ws(after);
        let after_int = take_int(after_ws)?;
        let after_ws = skip_ws(after_int);
        if after_ws.starts_with(')') {
            let used = rest.len() - after_ws.len() + 1;
            return Some(used);
        }
        return None;
    }
    for name in ["from_black_alpha(", "from_white_alpha("] {
        if rest.starts_with(name) {
            return Some(name.len());
        }
    }
    const NAMES: &[&str] = &["LIGHT_GRAY", "DARK_GRAY", "WHITE", "BLACK", "GRAY", "GREEN", "BLUE", "YELLOW", "RED"];
    for name in NAMES {
        if let Some(after) = rest.strip_prefix(name) {
            let ident = after.chars().next().is_some_and(|c| c.is_ascii_alphanumeric() || c == '_');
            if !ident {
                return Some(name.len());
            }
        }
    }
    None
}

fn sites_on_line(line: &str) -> usize {
    let mut count = 0;
    let mut rest = line;
    while let Some(idx) = rest.find("Color32::") {
        let after = &rest[idx + "Color32::".len()..];
        if let Some(used) = match_call(after) {
            count += 1;
            rest = &after[used..];
        } else {
            rest = after;
        }
    }
    count
}

/// 1-based line of every chrome colour site. Duplicates on one line are kept. No allowlist.
pub fn chrome_colour_sites(src: &str) -> Vec<usize> {
    let clean = clean_rust(src);
    let mut out = Vec::new();
    for (i, line) in clean.split('\n').enumerate() {
        for _ in 0..sites_on_line(line) {
            out.push(i + 1);
        }
    }
    out
}

/// A cleaned line whose white is a texture tint, not chrome.
pub fn texture_tint_line(clean_line: &str) -> bool {
    clean_line.contains("Color32::WHITE") && (clean_line.contains(".image(") || clean_line.contains("with_texture"))
}

#[test]
fn helper_blanks_only_the_cfg_test_item() {
    let one = "#[cfg(test)]\nmod x;\nlet c = Color32::from_rgb(1, 2, 3);\n";
    assert_eq!(chrome_colour_sites(one), vec![3]);
    let two = "#[cfg(test)]\nmod t {\n    fn f() { let _ = Color32::RED; }\n}\nlet g = Color32::from_gray(10);\n";
    assert_eq!(chrome_colour_sites(two), vec![5]);
}

#[test]
fn helper_skips_comments_and_strings() {
    let commented = "// Color32::WHITE\nlet s = \"Color32::WHITE\";\n/* Color32::BLACK\n */ let c = 'x';\n";
    assert_eq!(chrome_colour_sites(commented), Vec::<usize>::new());
    assert_eq!(chrome_colour_sites("a(Color32::WHITE, Color32::BLACK);\n"), vec![1, 1]);
    assert_eq!(chrome_colour_sites("let a = Color32::TRANSPARENT;\nlet b = Color32::from_rgb(r, g, b);\n"), Vec::<usize>::new());
}

#[test]
fn helper_keeps_code_after_a_raw_string_with_wide_characters() {
    // The raw string holds 20 chars of 3 bytes each; the scan must land on the char after its closing quote.
    let src = format!("let s = r\"{}\";\nlet c = Color32::WHITE;\nfn f() {{}}\n", "\u{2026}".repeat(20));
    assert_eq!(chrome_colour_sites(&src), vec![2]);
    let cleaned = clean_rust(&src);
    assert!(cleaned.contains("fn f() {}"), "code after the raw string survives: {cleaned:?}");
    let hashed = format!("let s = r#\"{}\"#;\nlet c = Color32::BLACK;\n", "\u{00b7}".repeat(40));
    assert_eq!(chrome_colour_sites(&hashed), vec![2]);
}

#[test]
#[ignore]
fn chrome_census_print() {
    let files = std::env::var("CHROME_CENSUS_FILES").unwrap_or_default();
    for rel in files.split_whitespace() {
        let path = format!("crates/designos-app/src/{rel}");
        let src = std::fs::read_to_string(&path).unwrap_or_else(|err| panic!("{path}: {err}"));
        let sites = chrome_colour_sites(&src);
        let mut uniq = sites.clone();
        uniq.sort_unstable();
        uniq.dedup();
        println!("{rel} chrome_sites={} {uniq:?}", sites.len());
    }
}
