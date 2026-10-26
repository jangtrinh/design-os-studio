//! Text shown on an approval card: one sanitizer for every CLI's titles and summaries.
//!
//! What a card shows must be what the user is approving. Control characters and invisible format characters (bidi
//! controls, zero-width marks, line and paragraph separators) can reorder text or pad it toward the length limit while
//! looking short, so they become spaces; then the text is cut at the shared limit and the cut is reported.
//!
//! Some text cannot be shown at all: a space look-alike draws as a space but is part of a word, an invisible character
//! draws as nothing, a combining mark can vanish, a look-alike letter reads as another. [`reads_as_ascii_exactly`] (a
//! command) and [`reads_as_it_is`] (a file path) say whether a card can show the text as it is; when it cannot, the
//! request goes to the terminal, which shows it in full.
// The Claude poll path (`agent_permissions.rs`) adopts this in the terminal integration; until then only the Codex
// module (macOS) calls it. Remove this allow when the Claude path does.
#![cfg_attr(not(target_os = "macos"), allow(dead_code))]

use crate::agent_permissions::MAX_INPUT_SUMMARY_CHARS;

/// A control character (category Cc) or an invisible format character that can reorder or hide text.
fn is_hidden(c: char) -> bool {
    c.is_control() || matches!(c, '\u{061C}' | '\u{200B}'..='\u{200F}' | '\u{2028}'..='\u{202E}' | '\u{2060}'..='\u{2069}' | '\u{FEFF}')
}

/// `raw` with every hidden character replaced by a space (one for one, so the length is unchanged).
pub fn clean(raw: &str) -> String {
    raw.chars().map(|c| if is_hidden(c) { ' ' } else { c }).collect()
}

/// `raw` is printable ASCII only (U+0020 to U+007E). That is the one kind of text that reads on a card exactly as the
/// shell parses it: a no-break space looks like a space and is part of a word, so `--exclude=.git<NBSP>--dry-run` is one
/// argument that looks like two; soft hyphens and variation selectors draw as nothing; combining marks can vanish; Cyrillic
/// or Vietnamese letters can pass for Latin ones; a tab or a line break is not one space. Empty text passes (blank text is
/// the caller's own case).
pub fn reads_as_ascii_exactly(raw: &str) -> bool {
    raw.chars().all(|c| matches!(c, ' '..='~'))
}

/// An invisible character: default-ignorable (they draw as nothing or reorder text) or a blank-drawing symbol.
fn is_invisible(c: char) -> bool {
    matches!(
        c,
        '\u{00AD}'
            | '\u{034F}'
            | '\u{061C}'
            | '\u{115F}'..='\u{1160}'
            | '\u{17B4}'..='\u{17B5}'
            | '\u{180B}'..='\u{180F}'
            | '\u{200B}'..='\u{200F}'
            | '\u{202A}'..='\u{202E}'
            | '\u{2060}'..='\u{206F}'
            | '\u{2800}' // Braille blank: draws as a space
            | '\u{3164}'
            | '\u{FE00}'..='\u{FE0F}'
            | '\u{FEFF}'
            | '\u{FFA0}'
            | '\u{FFF0}'..='\u{FFF8}'
            | '\u{1BCA0}'..='\u{1BCA3}'
            | '\u{1D173}'..='\u{1D17A}'
            | '\u{E0000}'..='\u{E0FFF}'
    )
}

/// The other format characters (category Cf), the ones that are not default-ignorable: Arabic and Syriac number and
/// verse signs, interlinear annotation marks, Kaithi number signs, Egyptian hieroglyph format controls. Some draw as
/// nothing and some draw a sign over the next characters, so a path that has one does not read as it is. (A sweep of
/// every Cc, Cf, Zs, Zl and Zp code point of Unicode 16.0 against this file's sets leaves only U+0020.)
fn is_other_format(c: char) -> bool {
    matches!(
        c,
        '\u{0600}'..='\u{0605}'
            | '\u{06DD}'
            | '\u{070F}'
            | '\u{0890}'..='\u{0891}'
            | '\u{08E2}'
            | '\u{FFF9}'..='\u{FFFB}'
            | '\u{110BD}'
            | '\u{110CD}'
            | '\u{13430}'..='\u{1343F}'
    )
}

/// A combining mark: some fonts draw it as nothing, so the letter it belongs to shows without it.
fn is_combining_mark(c: char) -> bool {
    matches!(c, '\u{0300}'..='\u{036F}' | '\u{1AB0}'..='\u{1AFF}' | '\u{1DC0}'..='\u{1DFF}' | '\u{20D0}'..='\u{20FF}' | '\u{FE20}'..='\u{FE2F}')
}

/// `raw` (a file path) reads on a card as it is: no control character, no whitespace but U+0020, no invisible character
/// and no combining mark. Letters of any script pass when they are precomposed (NFC, as Vietnamese file names usually are);
/// the same name decomposed (NFD) does not, because its accents can vanish.
pub fn reads_as_it_is(raw: &str) -> bool {
    raw.chars().all(|c| !(is_hidden(c) || is_invisible(c) || is_other_format(c) || is_combining_mark(c) || (c.is_whitespace() && c != ' ')))
}

/// [`clean`], then cut at [`MAX_INPUT_SUMMARY_CHARS`] characters. The flag says the text was cut.
pub fn finish(raw: &str) -> (String, bool) {
    let text = clean(raw);
    let cut = text.chars().count() > MAX_INPUT_SUMMARY_CHARS;
    (if cut { text.chars().take(MAX_INPUT_SUMMARY_CHARS).collect() } else { text }, cut)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bidi_zero_width_and_separator_characters_become_spaces() {
        let tricky = "echo \u{202E}txt.exe\u{202C} \u{2066}x\u{2069} \u{200B}\u{200F}\u{2028}\u{FEFF}end\u{61C}\u{2060}\u{7}\t";
        let (out, cut) = finish(tricky);
        assert!(!cut && !out.chars().any(is_hidden));
        assert_eq!(out.chars().count(), tricky.chars().count(), "one space for each, nothing dropped");
        assert_eq!(out.split_whitespace().collect::<Vec<_>>(), ["echo", "txt.exe", "x", "end"]);
        assert_eq!(clean("plain text, é and 日本語"), "plain text, é and 日本語", "ordinary text is untouched");
    }

    #[test]
    fn only_printable_ascii_reads_as_ascii_exactly() {
        assert!(reads_as_ascii_exactly("") && reads_as_ascii_exactly(&(' '..='~').collect::<String>()));
        for other in ["a\tb", "a\nb", "a\rb", "a\u{1f}b", "a\u{7f}b", "a\u{a0}b", "\u{e9}", "\u{65e5}\u{672c}", "a\u{200b}b", "\u{1f680}"] {
            assert!(!reads_as_ascii_exactly(other), "{other:?}");
        }
    }

    #[test]
    fn a_path_reads_as_it_is_unless_a_character_hides_or_looks_like_a_space_or_stacks_on_another() {
        for good in ["", "/a b/c-d_1.md", "/T\u{e0}i li\u{1ec7}u/\u{111}\u{1a1}n h\u{e0}ng.md", "/\u{65e5}\u{672c}\u{8a9e}/\u{e9}.txt"] {
            assert!(reads_as_it_is(good), "{good:?}");
        }
        let bad = [
            "a\nb",
            "a\tb",
            "a\u{7}b",
            "a\u{85}b",
            "a\u{a0}b",
            "a\u{1680}b",
            "a\u{2003}b",
            "a\u{202f}b",
            "a\u{3000}b",
            "a\u{2028}b",
            "a\u{2029}b",
            "a\u{ad}b",
            "a\u{34f}b",
            "a\u{61c}b",
            "a\u{115f}b",
            "a\u{17b4}b",
            "a\u{180e}b",
            "a\u{200b}b",
            "a\u{202e}b",
            "a\u{2066}b",
            "a\u{2800}b",
            "a\u{3164}b",
            "a\u{fe0f}b",
            "a\u{feff}",
            "a\u{ffa0}b",
            "a\u{1d173}b",
            "a\u{e0041}b",
            "e\u{301}",
            "e\u{323}\u{302}",
            "a\u{1ab0}",
            "a\u{1dc0}",
            "a\u{20d0}",
            "a\u{fe20}",
            // format characters (category Cf) that are not default-ignorable: Arabic and Syriac marks, interlinear
            // annotation, Kaithi number signs, Egyptian hieroglyph format controls
            "a\u{600}b",
            "a\u{605}b",
            "a\u{6dd}b",
            "a\u{70f}b",
            "a\u{890}b",
            "a\u{891}b",
            "a\u{8e2}b",
            "a\u{fff9}b",
            "a\u{fffb}b",
            "a\u{110bd}b",
            "a\u{110cd}b",
            "a\u{13430}b",
            "a\u{1343f}b",
        ];
        for text in bad {
            assert!(!reads_as_it_is(text), "{text:?}");
        }
    }

    #[test]
    fn the_cut_counts_characters_after_cleaning_so_padding_cannot_hide_a_tail() {
        assert_eq!(finish(&"é".repeat(MAX_INPUT_SUMMARY_CHARS)), ("é".repeat(MAX_INPUT_SUMMARY_CHARS), false));
        let (cut_text, cut) = finish(&"é".repeat(MAX_INPUT_SUMMARY_CHARS + 1));
        assert_eq!((cut_text.chars().count(), cut), (MAX_INPUT_SUMMARY_CHARS, true));
        let padded = format!("{}{}", "a".repeat(150), "\u{200B}".repeat(60));
        assert!(finish(&padded).1, "60 zero-width characters count toward the limit");
    }
}
