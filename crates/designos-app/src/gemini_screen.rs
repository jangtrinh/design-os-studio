//! What Gemini CLI's confirmation dialog looks like on a tab's screen, and how Studio finds it there (`gemini_dialog` decides what to
//! do with it). Gemini's text, not Studio's: it was read from the source of Gemini CLI 0.35.3 and 0.65.0, never from a real dialog.

/// The last option of every Gemini tool-confirmation dialog that has options. Tests pin it; the screen is searched for its start.
#[cfg_attr(not(test), allow(dead_code))]
pub const DIALOG_MARKER: &str = "No, suggest changes (esc)";

/// The part of [`DIALOG_MARKER`] the screen is searched for. Gemini cuts an option label that does not fit, with an ellipsis, and
/// never wraps it (`RadioButtonSelect.tsx:103`, `ToolConfirmationMessage.tsx:1009-1013` in 0.65.0). The label gets `columns - 9` cells
/// (`columns - 10` with the alternate screen): the row is a 2-cell indicator, a 2-cell number and a 1-cell gap
/// (`BaseSelectionList.tsx:122-144`), inside the dialog's border and padding, which leave `columns - 4`
/// (`ToolConfirmationQueue.tsx:79,129,210`, `ui-sizing.ts:14-17`). At the narrowest panel Studio allows, 260 px or 30 columns
/// (`terminal.rs`, `shell.rs:1471`), that is 20 cells: the first 19 characters and an ellipsis, "No, suggest changes…". Those 19
/// characters, three whole words, are what is searched for, and the ellipsis counts as a break between words. The line is found
/// down to 29 columns (30 with the alternate screen).
pub const DIALOG_PREFIX: &str = "No, suggest changes";

/// What Gemini draws in place of the options while an edit is open in the external editor (`ToolConfirmationMessage.tsx:708-710`
/// in 0.35.3, `1031-1034` in 0.65.0).
pub const MODIFY_MARKER: &str = "Save and close external editor to continue";

/// What a Gemini tab's screen shows of a confirmation dialog.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum DialogView {
    #[default]
    None,
    /// The options are on screen.
    Options,
    /// An edit is open in the external editor.
    Modifying,
}

/// Finds a phrase of space-separated words in a stream of words, one word at a time and without collecting them.
struct PhraseFinder {
    phrase: &'static str,
    words: usize,
    at: usize,
}

impl PhraseFinder {
    fn new(phrase: &'static str) -> Self {
        Self { phrase, words: phrase.split(' ').count(), at: 0 }
    }

    /// Takes the next word; true once the phrase's words have come one after another. The words of a phrase differ, so a miss
    /// can only restart at the first one.
    fn feed(&mut self, word: &str) -> bool {
        let phrase = self.phrase;
        let wanted = |index: usize| phrase.split(' ').nth(index);
        self.at = if Some(word) == wanted(self.at) {
            self.at + 1
        } else if Some(word) == wanted(0) {
            1
        } else {
            0
        };
        self.at == self.words
    }
}

/// Where words break on a screen: whitespace, the box-drawing characters of the dialog's border, and the ellipsis of a cut label.
fn breaks_words(c: char) -> bool {
    c.is_whitespace() || ('\u{2500}'..='\u{257F}').contains(&c) || c == '\u{2026}'
}

/// What `parser`'s screen shows. Read at scrollback 0 whatever the user scrolled to, and put back under the same lock.
pub fn dialog_view(parser: &mut vt100::Parser) -> DialogView {
    let scrolled = parser.screen().scrollback();
    parser.set_scrollback(0);
    let text = parser.screen().contents();
    parser.set_scrollback(scrolled);
    let (mut options, mut modifying, mut modify_seen) = (PhraseFinder::new(DIALOG_PREFIX), PhraseFinder::new(MODIFY_MARKER), false);
    for word in text.split(breaks_words).filter(|word| !word.is_empty()) {
        if options.feed(word) {
            return DialogView::Options;
        }
        modify_seen |= modifying.feed(word);
    }
    if modify_seen {
        DialogView::Modifying
    } else {
        DialogView::None
    }
}
