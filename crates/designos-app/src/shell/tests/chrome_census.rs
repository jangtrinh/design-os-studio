//! Chrome-colour census. Ports the comment/string/test blanking in
//! `scripts/rust_literal_scan.py` and counts `Color32` constructors by hand.

pub(super) fn clean_rust(src: &str) -> String {
    drop_cfg_test(&strip_code(src))
}

pub(super) fn chrome_colour_sites(src: &str) -> Vec<usize> {
    let mut out = Vec::new();
    for (idx, line) in clean_rust(src).split('\n').enumerate() {
        let mut rest = line;
        while let Some(at) = find_site(rest) {
            out.push(idx + 1);
            rest = &rest[at..];
        }
    }
    out
}

pub(super) fn texture_tint_line(clean_line: &str) -> bool {
    clean_line.contains("Color32::WHITE") && (clean_line.contains(".image(") || clean_line.contains("with_texture"))
}

fn strip_code(src: &str) -> String {
    let b = src.as_bytes();
    let n = b.len();
    let mut out = String::with_capacity(n);
    let mut i = 0;
    while i < n {
        if b[i] == b'/' && i + 1 < n && b[i + 1] == b'/' {
            while i < n && b[i] != b'\n' {
                i += 1;
            }
            continue;
        }
        if b[i] == b'/' && i + 1 < n && b[i + 1] == b'*' {
            let mut depth = 1;
            i += 2;
            while i < n && depth > 0 {
                if b[i] == b'/' && i + 1 < n && b[i + 1] == b'*' {
                    depth += 1;
                    i += 2;
                } else if b[i] == b'*' && i + 1 < n && b[i + 1] == b'/' {
                    depth -= 1;
                    i += 2;
                } else {
                    if b[i] == b'\n' {
                        out.push('\n');
                    }
                    i += 1;
                }
            }
            continue;
        }
        if let Some(raw_end) = raw_string_end(b, i) {
            let body = &src[i..raw_end];
            out.push_str("\"\"");
            out.extend(std::iter::repeat_n('\n', body.matches('\n').count()));
            i = raw_end;
            continue;
        }
        if b[i] == b'"' {
            let mut j = i + 1;
            while j < n && b[j] != b'"' {
                j += if b[j] == b'\\' { 2 } else { 1 };
            }
            let end = if j < n { j + 1 } else { n };
            out.push_str("\"\"");
            out.extend(std::iter::repeat_n('\n', src[i..end].matches('\n').count()));
            i = end;
            continue;
        }
        if b[i] == b'\'' {
            if let Some(end) = char_literal_end(b, i) {
                out.push_str("' '");
                i = end;
                continue;
            }
        }
        out.push(b[i] as char);
        i += 1;
    }
    out
}

fn raw_string_end(b: &[u8], i: usize) -> Option<usize> {
    if i > 0 && is_ident(b[i - 1]) {
        return None;
    }
    let window = &b[i..(i + 12).min(b.len())];
    let mut k = 0;
    if window.first() == Some(&b'b') {
        k = 1;
    }
    if k >= window.len() || window[k] != b'r' {
        return None;
    }
    k += 1;
    let hash_from = k;
    while k < window.len() && window[k] == b'#' {
        k += 1;
    }
    if k >= window.len() || window[k] != b'"' {
        return None;
    }
    let hashes = k - hash_from;
    let mut j = i + k + 1;
    while j < b.len() {
        if b[j] == b'"' {
            let mut h = 0;
            while h < hashes && j + 1 + h < b.len() && b[j + 1 + h] == b'#' {
                h += 1;
            }
            if h == hashes {
                return Some(j + 1 + hashes);
            }
        }
        j += 1;
    }
    Some(b.len())
}

fn char_literal_end(b: &[u8], i: usize) -> Option<usize> {
    if i + 2 >= b.len() || b[i] != b'\'' {
        return None;
    }
    if b[i + 1] == b'\\' {
        let mut j = i + 2;
        while j < b.len() && b[j] != b'\'' {
            j += 1;
        }
        if j < b.len() && j - i <= 12 {
            return Some(j + 1);
        }
        return None;
    }
    if b[i + 1] != b'\\' && b[i + 1] != b'\'' && i + 2 < b.len() && b[i + 2] == b'\'' {
        return Some(i + 3);
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
        let newlines = code[start..j].matches('\n').count();
        code.replace_range(start..j, &"\n".repeat(newlines));
    }
}

fn find_site(line: &str) -> Option<usize> {
    let b = line.as_bytes();
    let key = b"Color32::";
    let mut from = 0;
    while let Some(rel) = line[from..].find("Color32::") {
        let at = from + rel;
        let rest = &b[at + key.len()..];
        if let Some(consumed) = site_after(rest) {
            return Some(at + key.len() + consumed);
        }
        from = at + key.len();
    }
    None
}

fn site_after(rest: &[u8]) -> Option<usize> {
    const NAMED: [&[u8]; 10] = [b"LIGHT_GRAY", b"DARK_GRAY", b"WHITE", b"BLACK", b"GRAY", b"RED", b"GREEN", b"BLUE", b"YELLOW", b"TRANSPARENT"];
    for name in NAMED {
        if rest.starts_with(name) {
            let after = name.len();
            let ident = rest.get(after).is_some_and(|c| is_ident(*c));
            if name == b"TRANSPARENT" || ident {
                return None;
            }
            return Some(after);
        }
    }
    const RGB: [&[u8]; 6] =
        [b"from_rgba_premultiplied(", b"from_rgba_unmultiplied(", b"from_rgb_premultiplied(", b"from_rgb_unmultiplied(", b"from_rgba(", b"from_rgb("];
    for prefix in RGB {
        if let Some(tail) = rest.strip_prefix(prefix) {
            let i = skip_ws(tail, 0);
            let j = take_int(tail, i)?;
            let k = skip_ws(tail, j);
            if tail.get(k) == Some(&b',') {
                return Some(prefix.len() + k + 1);
            }
            return None;
        }
    }
    if let Some(tail) = rest.strip_prefix(b"from_gray(") {
        let i = skip_ws(tail, 0);
        let j = take_int(tail, i)?;
        let k = skip_ws(tail, j);
        if tail.get(k) == Some(&b')') {
            return Some("from_gray(".len() + k + 1);
        }
        return None;
    }
    if rest.starts_with(b"from_black_alpha(") {
        return Some("from_black_alpha(".len());
    }
    if rest.starts_with(b"from_white_alpha(") {
        return Some("from_white_alpha(".len());
    }
    None
}

fn skip_ws(s: &[u8], mut i: usize) -> usize {
    while i < s.len() && s[i].is_ascii_whitespace() {
        i += 1;
    }
    i
}

fn take_int(s: &[u8], i: usize) -> Option<usize> {
    if i >= s.len() {
        return None;
    }
    if s[i..].starts_with(b"0x") || s[i..].starts_with(b"0X") {
        let mut j = i + 2;
        if j >= s.len() || !s[j].is_ascii_hexdigit() {
            return None;
        }
        while j < s.len() && s[j].is_ascii_hexdigit() {
            j += 1;
        }
        return Some(j);
    }
    if s[i].is_ascii_digit() {
        let mut j = i + 1;
        while j < s.len() && s[j].is_ascii_digit() {
            j += 1;
        }
        return Some(j);
    }
    None
}

fn is_ident(c: u8) -> bool {
    c.is_ascii_alphanumeric() || c == b'_'
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
    assert_eq!(chrome_colour_sites("let t = Color32::TRANSPARENT;\n"), Vec::<usize>::new());
    assert_eq!(chrome_colour_sites("let c = Color32::from_rgb(r, g, b);\n"), Vec::<usize>::new());
}

#[test]
#[ignore]
fn chrome_census_print() {
    let files = std::env::var("CHROME_CENSUS_FILES").unwrap_or_default();
    for rel in files.split_whitespace() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src").join(rel);
        let src = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {rel}: {e}"));
        let sites = chrome_colour_sites(&src);
        let mut uniq = sites.clone();
        uniq.sort_unstable();
        uniq.dedup();
        println!("{rel} chrome_sites={} {uniq:?}", sites.len());
    }
}
