//! Shared chrome-colour census. Ports `scripts/rust_literal_scan.py` `clean`
//! (`strip_code`, then `drop_cfg_test`) and reports raw sites. No allowlist.

pub fn clean_rust(src: &str) -> String {
    drop_cfg_test(&strip_code(src))
}

pub fn chrome_colour_sites(src: &str) -> Vec<usize> {
    let cleaned = clean_rust(src);
    let mut sites = Vec::new();
    for (idx, line) in cleaned.split_inclusive('\n').enumerate() {
        sites.extend(std::iter::repeat_n(idx + 1, hex_matches(line)));
    }
    sites
}

#[allow(dead_code)]
pub fn texture_tint_line(clean_line: &str) -> bool {
    clean_line.contains("Color32::WHITE") && (clean_line.contains(".image(") || clean_line.contains("with_texture"))
}

fn strip_code(src: &str) -> String {
    let b = src.as_bytes();
    let n = b.len();
    let mut out: Vec<u8> = Vec::with_capacity(n);
    let mut i = 0;
    while i < n {
        if i + 1 < n && b[i] == b'/' && b[i + 1] == b'/' {
            while i < n && b[i] != b'\n' {
                i += 1;
            }
            continue;
        }
        if i + 1 < n && b[i] == b'/' && b[i + 1] == b'*' {
            let mut depth = 1;
            i += 2;
            while i < n && depth > 0 {
                if i + 1 < n && b[i] == b'/' && b[i + 1] == b'*' {
                    depth += 1;
                    i += 2;
                } else if i + 1 < n && b[i] == b'*' && b[i + 1] == b'/' {
                    depth -= 1;
                    i += 2;
                } else {
                    if b[i] == b'\n' {
                        out.push(b'\n');
                    }
                    i += 1;
                }
            }
            continue;
        }
        if let Some((opener_end, hashes)) = raw_string_at(b, i) {
            let delim_quote_at = find_raw_close(b, opener_end, hashes);
            let end = delim_quote_at.unwrap_or(n);
            let consumed_to = if delim_quote_at.is_some() { end + 1 + hashes } else { n };
            let consumed_to = consumed_to.min(n);
            out.extend_from_slice(b"\"\"");
            out.extend(b[i..consumed_to].iter().copied().filter(|c| *c == b'\n'));
            i = consumed_to;
            continue;
        }
        if b[i] == b'"' {
            let mut j = i + 1;
            while j < n && b[j] != b'"' {
                j += if b[j] == b'\\' { 2 } else { 1 };
            }
            let end = if j < n { j + 1 } else { n };
            out.extend_from_slice(b"\"\"");
            out.extend(b[i..end].iter().copied().filter(|c| *c == b'\n'));
            i = end;
            continue;
        }
        if b[i] == b'\'' {
            if let Some(len) = char_lit_len(b, i) {
                out.extend_from_slice(b"' '");
                i += len;
                continue;
            }
        }
        out.push(b[i]);
        i += 1;
    }
    String::from_utf8(out).unwrap_or_default()
}

fn raw_string_at(b: &[u8], i: usize) -> Option<(usize, usize)> {
    if i > 0 {
        let prev = b[i - 1];
        if prev.is_ascii_alphanumeric() || prev == b'_' {
            return None;
        }
    }
    let window_end = (i + 12).min(b.len());
    let w = &b[i..window_end];
    let mut k = 0;
    if k < w.len() && w[k] == b'b' {
        k += 1;
    }
    if k >= w.len() || w[k] != b'r' {
        return None;
    }
    k += 1;
    let mut hashes = 0;
    while k < w.len() && w[k] == b'#' {
        hashes += 1;
        k += 1;
    }
    if k >= w.len() || w[k] != b'"' {
        return None;
    }
    Some((i + k + 1, hashes))
}

fn find_raw_close(b: &[u8], from: usize, hashes: usize) -> Option<usize> {
    let mut i = from;
    while i < b.len() {
        if b[i] == b'"' && b[i + 1..].iter().take(hashes).all(|c| *c == b'#') && i + 1 + hashes <= b.len() {
            return Some(i);
        }
        i += 1;
    }
    None
}

fn char_lit_len(b: &[u8], i: usize) -> Option<usize> {
    let window_end = (i + 12).min(b.len());
    let w = &b[i..window_end];
    if w.first() != Some(&b'\'') || w.len() < 3 {
        return None;
    }
    if w[1] == b'\\' {
        let mut j = 3;
        while j < w.len() && w[j] != b'\'' {
            j += 1;
        }
        if j < w.len() && w[j] == b'\'' {
            return Some(j + 1);
        }
        return None;
    }
    if w[1] != b'\'' && w.get(2) == Some(&b'\'') {
        return Some(3);
    }
    None
}

fn drop_cfg_test(code: &str) -> String {
    let mut code = code.to_string();
    loop {
        let Some(start) = code.find("#[cfg(test)]") else {
            return code;
        };
        let bytes = code.as_bytes();
        let mut j = start + "#[cfg(test)]".len();
        let mut depth = 0i32;
        let mut started = false;
        while j < bytes.len() {
            match bytes[j] {
                b'{' => {
                    depth += 1;
                    started = true;
                }
                b'}' => {
                    depth -= 1;
                    if started && depth == 0 {
                        j += 1;
                        break;
                    }
                }
                b';' if !started => {
                    j += 1;
                    break;
                }
                _ => {}
            }
            j += 1;
        }
        let newlines = code[start..j].bytes().filter(|c| *c == b'\n').count();
        code = format!("{}{}{}", &code[..start], "\n".repeat(newlines), &code[j..]);
    }
}

const NAMED: &[&str] = &["LIGHT_GRAY", "DARK_GRAY", "WHITE", "BLACK", "GRAY", "RED", "GREEN", "BLUE", "YELLOW"];

fn hex_matches(line: &str) -> usize {
    let b = line.as_bytes();
    let mut n = 0;
    let mut i = 0;
    while i < b.len() {
        if let Some(rest) = line[i..].strip_prefix("Color32::") {
            if let Some(consumed) = constructor_site(rest) {
                n += 1;
                i += "Color32::".len() + consumed;
                continue;
            }
        }
        i += 1;
    }
    n
}

fn constructor_site(rest: &str) -> Option<usize> {
    const CTORS: &[&str] =
        &["from_rgba_unmultiplied(", "from_rgba_premultiplied(", "from_rgb_unmultiplied(", "from_rgb_premultiplied(", "from_rgba(", "from_rgb("];
    for ctor in CTORS {
        if let Some(after) = rest.strip_prefix(ctor) {
            return literal_then_comma(after).map(|used| ctor.len() + used);
        }
    }
    if let Some(after) = rest.strip_prefix("from_gray(") {
        return literal_then_close(after).map(|used| "from_gray(".len() + used);
    }
    if rest.starts_with("from_black_alpha(") {
        return Some("from_black_alpha(".len());
    }
    if rest.starts_with("from_white_alpha(") {
        return Some("from_white_alpha(".len());
    }
    for name in NAMED {
        if let Some(after) = rest.strip_prefix(name) {
            let next = after.chars().next();
            if next.is_none_or(|c| !(c.is_ascii_alphanumeric() || c == '_')) {
                return Some(name.len());
            }
        }
    }
    None
}

fn literal_then_comma(s: &str) -> Option<usize> {
    let (used, rest) = skip_ws_and_int(s)?;
    let rest = rest.trim_start();
    let ws = s.len() - used - rest.len();
    if rest.starts_with(',') {
        Some(used + ws + 1)
    } else {
        None
    }
}

fn literal_then_close(s: &str) -> Option<usize> {
    let (used, rest) = skip_ws_and_int(s)?;
    let rest = rest.trim_start();
    let ws = s.len() - used - rest.len();
    if rest.starts_with(')') {
        Some(used + ws + 1)
    } else {
        None
    }
}

fn skip_ws_and_int(s: &str) -> Option<(usize, &str)> {
    let trimmed = s.trim_start();
    let ws = s.len() - trimmed.len();
    let b = trimmed.as_bytes();
    if b.len() >= 2 && b[0] == b'0' && (b[1] == b'x' || b[1] == b'X') {
        let mut k = 2;
        if k >= b.len() || !b[k].is_ascii_hexdigit() {
            return None;
        }
        while k < b.len() && b[k].is_ascii_hexdigit() {
            k += 1;
        }
        return Some((ws + k, &trimmed[k..]));
    }
    if b.first().is_some_and(|c| c.is_ascii_digit()) {
        let mut k = 1;
        while k < b.len() && b[k].is_ascii_digit() {
            k += 1;
        }
        return Some((ws + k, &trimmed[k..]));
    }
    None
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
    assert_eq!(chrome_colour_sites("let t = Color32::TRANSPARENT;\nlet c = Color32::from_rgb(r, g, b);\n"), Vec::<usize>::new());
}

#[test]
#[ignore]
fn chrome_census_print() {
    let files = std::env::var("CHROME_CENSUS_FILES").unwrap_or_default();
    for rel in files.split_whitespace() {
        let path = format!("crates/designos-app/src/{rel}");
        let src = std::fs::read_to_string(&path).unwrap_or_else(|err| panic!("read {path}: {err}"));
        let sites = chrome_colour_sites(&src);
        let mut unique: Vec<usize> = sites.clone();
        unique.sort_unstable();
        unique.dedup();
        println!("{rel} chrome_sites={} {unique:?}", sites.len());
    }
}
