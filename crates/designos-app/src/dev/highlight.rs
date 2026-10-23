//! A small syntax highlighter for the Inspect code blocks (CSS, Tailwind/HTML/JSX, Swift, Kotlin).
//!
//! It is a one-pass tokenizer per line, not a parser: good enough to color properties, numbers,
//! strings, types and calls consistently in the generated code.

use super::Language;
use crate::theme;
use egui::Color32;
use std::borrow::Cow;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tok {
    Plain,
    Property,
    Keyword,
    Str,
    Number,
    Type,
    Function,
    Comment,
    Punct,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Span {
    pub tok: Tok,
    pub text: String,
}

fn span(tok: Tok, text: impl Into<String>) -> Span {
    Span { tok, text: text.into() }
}

pub fn color(t: Tok) -> Color32 {
    match t {
        Tok::Plain => theme::SYNTAX_PLAIN,
        Tok::Property => theme::SYNTAX_PROPERTY,
        Tok::Keyword => theme::SYNTAX_KEYWORD,
        Tok::Str => theme::SYNTAX_STRING,
        Tok::Number => theme::SYNTAX_NUMBER,
        Tok::Type => theme::SYNTAX_TYPE,
        Tok::Function => theme::SYNTAX_FUNCTION,
        Tok::Comment => theme::SYNTAX_COMMENT,
        Tok::Punct => theme::SYNTAX_PUNCT,
    }
}

fn keywords(lang: Language) -> &'static [&'static str] {
    match lang {
        Language::Css | Language::Tailwind | Language::Html => &["true", "false"],
        Language::SwiftUi | Language::UiKit => {
            &["let", "var", "func", "struct", "some", "return", "if", "else", "true", "false", "nil", "import", "self", "in"]
        }
        Language::AndroidXml => &["true", "false"],
        Language::Compose => &["val", "var", "fun", "return", "if", "else", "true", "false", "null", "import", "class", "object"],
        Language::React => &["export", "function", "return", "const", "true", "false", "null", "import"],
    }
}

fn is_ident_start(c: char) -> bool {
    c.is_ascii_alphabetic() || c == '_' || c == '@'
}

/// Columns a tab takes on screen. egui draws `\t` as four spaces, so the code view lays tabs out as four spaces too:
/// every character is one column, tokens never overlap and a line's width is its true width.
const TAB_COLUMNS: usize = 4;

/// The text of `src` as shown: each tab becomes `TAB_COLUMNS` spaces. The code that is copied keeps its tabs.
fn expand_tabs(src: &str) -> Cow<'_, str> {
    if src.contains('\t') {
        Cow::Owned(src.replace('\t', &" ".repeat(TAB_COLUMNS)))
    } else {
        Cow::Borrowed(src)
    }
}

/// Tokenizes `code` line by line, with tabs expanded to spaces for display.
pub fn highlight(lang: Language, code: &str) -> Vec<Vec<Span>> {
    code.lines().map(|l| line(lang, &expand_tabs(l))).collect()
}

fn line(lang: Language, src: &str) -> Vec<Span> {
    // XML comments (file headers, notes) are always one line in the generated layouts
    if lang == Language::AndroidXml && src.trim_start().starts_with("<!--") {
        return vec![span(Tok::Comment, src)];
    }
    let chars: Vec<char> = src.chars().collect();
    let mut out: Vec<Span> = Vec::new();
    let mut i = 0;
    let hyphen_idents = lang == Language::Css;
    while i < chars.len() {
        let c = chars[i];
        let prev = if i > 0 { Some(chars[i - 1]) } else { None };
        if c.is_whitespace() {
            let s = i;
            while i < chars.len() && chars[i].is_whitespace() {
                i += 1;
            }
            out.push(span(Tok::Plain, chars[s..i].iter().collect::<String>()));
        } else if c == '/' && chars.get(i + 1) == Some(&'/') {
            out.push(span(Tok::Comment, chars[i..].iter().collect::<String>()));
            i = chars.len();
        } else if c == '/' && chars.get(i + 1) == Some(&'*') {
            let s = i;
            i += 2;
            while i < chars.len() && !(chars[i] == '*' && chars.get(i + 1) == Some(&'/')) {
                i += 1;
            }
            i = (i + 2).min(chars.len());
            out.push(span(Tok::Comment, chars[s..i].iter().collect::<String>()));
        } else if c == '"' || c == '\'' {
            let s = i;
            i += 1;
            while i < chars.len() && chars[i] != c {
                if chars[i] == '\\' {
                    i += 1;
                }
                i += 1;
            }
            i = (i + 1).min(chars.len());
            out.push(span(Tok::Str, chars[s..i].iter().collect::<String>()));
        } else if c == '#' && chars.get(i + 1).is_some_and(|d| d.is_ascii_hexdigit()) {
            let s = i;
            i += 1;
            while i < chars.len() && chars[i].is_ascii_hexdigit() {
                i += 1;
            }
            out.push(span(Tok::Number, chars[s..i].iter().collect::<String>()));
        } else if c.is_ascii_digit()
            || (c == '-' && chars.get(i + 1).is_some_and(|d| d.is_ascii_digit()) && !prev.is_some_and(|p| p.is_alphanumeric() || p == ')'))
        {
            let s = i;
            i += 1;
            while i < chars.len() && (chars[i].is_ascii_digit() || chars[i] == '.' && chars.get(i + 1).is_some_and(|d| d.is_ascii_digit())) {
                i += 1;
            }
            // unit suffix: px, %, deg, f ...
            while i < chars.len() && (chars[i].is_ascii_alphabetic() || chars[i] == '%') {
                i += 1;
            }
            out.push(span(Tok::Number, chars[s..i].iter().collect::<String>()));
        } else if is_ident_start(c) {
            let s = i;
            i += 1;
            while i < chars.len() && (chars[i].is_ascii_alphanumeric() || chars[i] == '_' || (hyphen_idents && chars[i] == '-')) {
                i += 1;
            }
            let word: String = chars[s..i].iter().collect();
            let mut j = i;
            while j < chars.len() && chars[j] == ' ' {
                j += 1;
            }
            let next = chars.get(j).copied();
            let after = chars.get(j + 1).copied();
            let tok = if prev == Some('<') || (prev == Some('/') && s >= 2 && chars[s - 2] == '<') {
                Tok::Type
            } else if prev == Some('.') && s >= 1 {
                if next == Some('(') {
                    Tok::Function
                } else {
                    Tok::Plain
                }
            } else if next == Some('(') {
                if word.chars().next().is_some_and(char::is_uppercase) {
                    Tok::Type
                } else {
                    Tok::Function
                }
            } else if (next == Some(':') || (next == Some('=') && after != Some('=') && after != Some('>'))) && !keywords(lang).contains(&word.as_str()) {
                Tok::Property
            } else if keywords(lang).contains(&word.as_str()) {
                Tok::Keyword
            } else if word.chars().next().is_some_and(char::is_uppercase) {
                Tok::Type
            } else {
                Tok::Plain
            };
            out.push(span(tok, word));
        } else {
            let punct = "{}()[],;:.<>/=+*!?|&";
            out.push(span(if punct.contains(c) { Tok::Punct } else { Tok::Plain }, c.to_string()));
            i += 1;
        }
    }
    out
}

/// Width of a tokenized line in characters (the code font is monospace, so this is also its width in columns).
pub fn line_len(line: &[Span]) -> usize {
    line.iter().map(|s| s.text.chars().count()).sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kinds(l: &[Span]) -> Vec<(Tok, &str)> {
        l.iter().map(|s| (s.tok, s.text.as_str())).collect()
    }

    #[test]
    fn css_declaration() {
        let l = &highlight(Language::Css, "border-radius: 8px;")[0];
        assert_eq!(kinds(l), vec![(Tok::Property, "border-radius"), (Tok::Punct, ":"), (Tok::Plain, " "), (Tok::Number, "8px"), (Tok::Punct, ";")]);
        let l = &highlight(Language::Css, "background: var(--color-primary);")[0];
        assert!(kinds(l).contains(&(Tok::Function, "var")));
        let l = &highlight(Language::Css, "color: #FFF;")[0];
        assert!(kinds(l).contains(&(Tok::Number, "#FFF")));
    }

    #[test]
    fn swift_and_kotlin() {
        let l = &highlight(Language::SwiftUi, "    .padding(16)")[0];
        assert_eq!(kinds(l)[2], (Tok::Function, "padding"));
        assert!(kinds(l).contains(&(Tok::Number, "16")));
        let l = &highlight(Language::SwiftUi, "VStack(alignment: .leading, spacing: 8) {")[0];
        assert_eq!(kinds(l)[0], (Tok::Type, "VStack"));
        assert!(kinds(l).contains(&(Tok::Property, "alignment")));
        let l = &highlight(Language::Compose, "text = \"Hi\",")[0];
        assert_eq!(kinds(l)[0], (Tok::Property, "text"));
        assert!(kinds(l).contains(&(Tok::Str, "\"Hi\"")));
        let l = &highlight(Language::Compose, "color = Color(0xFF111111)")[0];
        assert!(kinds(l).contains(&(Tok::Type, "Color")));
    }

    #[test]
    fn uikit_and_android_xml() {
        let l = &highlight(Language::UiKit, "let card = UIStackView()")[0];
        assert_eq!(kinds(l)[0], (Tok::Keyword, "let"));
        assert!(kinds(l).contains(&(Tok::Type, "UIStackView")));
        let l = &highlight(Language::UiKit, "card.spacing = 8")[0];
        assert!(kinds(l).contains(&(Tok::Number, "8")));
        // an XML file header is one comment, even though it holds slashes and dots
        let l = &highlight(Language::AndroidXml, "<!-- res/layout/card.xml -->")[0];
        assert_eq!(kinds(l), vec![(Tok::Comment, "<!-- res/layout/card.xml -->")]);
        let l = &highlight(Language::AndroidXml, "    android:text=\"Hi\"")[0];
        assert!(kinds(l).contains(&(Tok::Property, "android")));
        assert!(kinds(l).contains(&(Tok::Property, "text")));
        assert!(kinds(l).contains(&(Tok::Str, "\"Hi\"")));
        let l = &highlight(Language::AndroidXml, "<LinearLayout")[0];
        assert_eq!(kinds(l)[1], (Tok::Type, "LinearLayout"));
    }

    #[test]
    fn markup() {
        let l = &highlight(Language::Tailwind, r#"<div class="flex p-4">"#)[0];
        assert_eq!(kinds(l)[1], (Tok::Type, "div"));
        assert!(kinds(l).contains(&(Tok::Property, "class")));
        assert!(kinds(l).contains(&(Tok::Str, "\"flex p-4\"")));
        let l = &highlight(Language::React, "export function Card() {")[0];
        assert_eq!(kinds(l)[0], (Tok::Keyword, "export"));
        assert!(kinds(l).contains(&(Tok::Type, "Card")));
    }

    #[test]
    fn comments_and_strings_do_not_leak() {
        let l = &highlight(Language::SwiftUi, "Text(\"a // b\") // note")[0];
        assert!(kinds(l).contains(&(Tok::Str, "\"a // b\"")));
        assert_eq!(kinds(l).last().unwrap().0, Tok::Comment);
    }

    #[test]
    fn a_line_is_as_wide_as_its_characters_and_keeps_its_indent() {
        let lines = highlight(Language::AndroidXml, "<A\n    xmlns:android=\"http://schemas.android.com/apk/res/android\"\n\n</A>");
        assert_eq!(lines.len(), 4, "a blank line stays a row");
        assert_eq!(line_len(&lines[0]), 2);
        assert_eq!(line_len(&lines[1]), 4 + "xmlns:android=\"http://schemas.android.com/apk/res/android\"".chars().count());
        assert_eq!(line_len(&lines[2]), 0);
        assert_eq!(lines[1][0].text, "    ");
    }

    #[test]
    fn tabs_become_four_columns_wherever_they_are() {
        let lines = highlight(Language::Css, "\tcontent: \"x\ty\"; /* a\tb */");
        let shown: String = lines[0].iter().map(|s| s.text.as_str()).collect();
        assert_eq!(shown, "    content: \"x    y\"; /* a    b */");
        assert!(!shown.contains('\t'));
        assert_eq!(line_len(&lines[0]), shown.chars().count());
    }
}
