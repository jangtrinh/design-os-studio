//! Reads Rust source into two texts of the same length: `code`, with the comments blanked and the strings kept, and `shape`, with the
//! contents of the strings and characters blanked too, so the braces and calls it holds are real ones. Newlines stay where they are.

fn is_ident(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

fn blank(c: char) -> char {
    if c == '\n' {
        '\n'
    } else {
        ' '
    }
}

/// Appends one character to `code` and one to `shape`, so the two texts keep the same length.
fn emit(code: &mut Vec<char>, shape: &mut Vec<char>, in_code: char, in_shape: char) {
    code.push(in_code);
    shape.push(in_shape);
}

/// Where a raw string (`r"…"`, `r#"…"#`, `br"…"`) that starts at `i` begins its contents: `(length of the opening, number of #)`.
fn raw_string_start(c: &[char], i: usize) -> Option<(usize, usize)> {
    if i > 0 && is_ident(c[i - 1]) {
        return None;
    }
    let mut j = i + usize::from(c[i] == 'b');
    if c.get(j) != Some(&'r') {
        return None;
    }
    j += 1;
    let hashes = c[j..].iter().take_while(|ch| **ch == '#').count();
    (c.get(j + hashes) == Some(&'"')).then_some((j + hashes + 1 - i, hashes))
}

/// The index of the closing quote of the character literal that starts at `i`, or `None` when `'` starts a lifetime.
fn char_literal_end(c: &[char], i: usize) -> Option<usize> {
    if c.get(i + 1) == Some(&'\\') {
        return (i + 3..c.len()).find(|j| c[*j] == '\'');
    }
    (c.get(i + 2) == Some(&'\'')).then_some(i + 2)
}

pub(super) fn lex(src: &str) -> (Vec<char>, Vec<char>) {
    let c: Vec<char> = src.chars().collect();
    let (mut code, mut shape) = (Vec::with_capacity(c.len()), Vec::with_capacity(c.len()));
    let mut i = 0;
    while i < c.len() {
        let next = c.get(i + 1).copied();
        if c[i] == '/' && next == Some('/') {
            while i < c.len() && c[i] != '\n' {
                emit(&mut code, &mut shape, ' ', ' ');
                i += 1;
            }
        } else if c[i] == '/' && next == Some('*') {
            let mut depth = 0;
            while i < c.len() {
                let (open, close) = (c[i] == '/' && c.get(i + 1) == Some(&'*'), c[i] == '*' && c.get(i + 1) == Some(&'/'));
                let width = if open || close { 2 } else { 1 };
                depth += i32::from(open) - i32::from(close);
                for ch in &c[i..i + width] {
                    emit(&mut code, &mut shape, blank(*ch), blank(*ch));
                }
                i += width;
                if depth == 0 {
                    break;
                }
            }
        } else if let Some((opening, hashes)) = raw_string_start(&c, i) {
            for ch in &c[i..i + opening] {
                emit(&mut code, &mut shape, *ch, *ch);
            }
            i += opening;
            while i < c.len() && !(c[i] == '"' && c[i + 1..].iter().take_while(|h| **h == '#').count() >= hashes) {
                emit(&mut code, &mut shape, c[i], blank(c[i]));
                i += 1;
            }
            let closing = (1 + hashes).min(c.len() - i);
            for ch in &c[i..i + closing] {
                emit(&mut code, &mut shape, *ch, *ch);
            }
            i += closing;
        } else if c[i] == '"' {
            emit(&mut code, &mut shape, '"', '"');
            i += 1;
            while i < c.len() && c[i] != '"' {
                let width = if c[i] == '\\' { 2.min(c.len() - i) } else { 1 };
                for ch in &c[i..i + width] {
                    emit(&mut code, &mut shape, *ch, blank(*ch));
                }
                i += width;
            }
            if i < c.len() {
                emit(&mut code, &mut shape, '"', '"');
                i += 1;
            }
        } else if let Some(end) = (c[i] == '\'').then(|| char_literal_end(&c, i)).flatten() {
            for ch in &c[i..=end] {
                emit(&mut code, &mut shape, *ch, if *ch == '\'' { '\'' } else { ' ' });
            }
            i = end + 1;
        } else {
            emit(&mut code, &mut shape, c[i], c[i]);
            i += 1;
        }
    }
    (code, shape)
}
