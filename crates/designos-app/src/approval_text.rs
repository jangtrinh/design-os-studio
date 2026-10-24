//! Text shown on an approval card: one sanitizer for every CLI's titles and summaries.
//!
//! What a card shows must be what the user is approving. Control characters and invisible format characters (bidi
//! controls, zero-width marks, line and paragraph separators) can reorder text or pad it toward the length limit while
//! looking short, so they become spaces; then the text is cut at the shared limit and the cut is reported.
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

/// `raw` reads the same after [`clean`]: it changes nothing but tabs, which show as spaces. A command that does not may
/// have a line break or an invisible character that decides what runs.
pub fn is_shown_exactly(raw: &str) -> bool {
    raw.chars().zip(clean(raw).chars()).all(|(before, after)| before == after || before == '\t')
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
    fn only_tabs_may_change_for_the_text_to_count_as_shown_exactly() {
        assert!(is_shown_exactly("echo a\tb  c") && is_shown_exactly("") && is_shown_exactly("日本語 é"));
        for changed in ["a\nb", "a\rb", "a\u{7}b", "a\u{2028}b", "a\u{202E}b", "a\u{200B}b", "a\u{FEFF}"] {
            assert!(!is_shown_exactly(changed), "{changed:?}");
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
