//! A command card shows what Approve will run exactly, or is not a card: only printable ASCII reads on the card the way
//! the shell parses it.

use super::mapping_tests::{card, map, open_tab};
use super::support::{fixture, with};
use crate::codex_remote::card_mapping::OpenTabReason;
use crate::codex_remote::item_index::ItemIndex;
use serde_json::{json, Value};

fn command(text: &str) -> Value {
    with(fixture("command-approval-request"), "/params/command", json!(text))
}

fn open_tab_for(text: &str) -> OpenTabReason {
    open_tab(map(&command(text), &ItemIndex::default()))
}

#[test]
fn a_command_with_a_line_break_or_a_control_character_gets_no_buttons() {
    let probes = [
        "/bin/zsh -lc 'echo cleaning up\nrm -rf ..'",
        "/bin/zsh -lc 'echo cleaning up\rrm -rf ..'",
        "echo a\u{2028}rm -rf ..",
        "echo \u{202E}txt.exe",
        "echo \u{7}bell",
        "echo a\u{200B}b",
        "echo a\u{1f}b",
        "echo a\u{7f}b",
    ];
    for text in probes {
        assert_eq!(open_tab_for(text), OpenTabReason::CommandNotShownExactly, "{text:?}");
    }
}

/// A no-break space draws exactly like a space, and zsh reads it as part of a word: the flag after it is folded into the
/// option's value, so the dry run the card shows is a real run.
#[test]
fn a_no_break_space_cannot_turn_a_dry_run_flag_into_part_of_another_word() {
    let real = "rsync -a --delete --exclude=.git --dry-run src/ dst/";
    assert_eq!(card(map(&command(real), &ItemIndex::default())).summary, real, "with an ASCII space it is a card");
    let disguised = real.replace(".git --dry", ".git\u{A0}--dry");
    assert_ne!(disguised, real);
    assert_eq!(open_tab_for(&disguised), OpenTabReason::CommandNotShownExactly);
}

#[test]
fn space_look_alikes_invisible_and_combining_characters_and_look_alike_letters_stay_in_the_terminal() {
    let probes = [
        ("no-break space", "echo a\u{A0}b"),
        ("figure space", "echo a\u{2007}b"),
        ("narrow no-break space", "echo a\u{202F}b"),
        ("ideographic space", "echo a\u{3000}b"),
        ("soft hyphen", "echo a\u{AD}b"),
        ("variation selector 16", "echo a\u{FE0F}b"),
        ("combining acute accent", "echo e\u{301}"),
        ("Cyrillic a in a host name", "curl https://ex\u{430}mple.com"),
        ("Vietnamese letter", "echo l\u{1EC7}nh"),
        ("emoji", "echo \u{1F680}"),
        ("CJK", "echo \u{65E5}\u{672C}"),
    ];
    for (what, text) in probes {
        assert_eq!(open_tab_for(text), OpenTabReason::CommandNotShownExactly, "{what}");
    }
}

#[test]
fn a_tab_is_not_printable_ascii_so_it_gets_no_buttons_either() {
    assert_eq!(open_tab_for("echo\ta\t\tb"), OpenTabReason::CommandNotShownExactly);
}

#[test]
fn a_plain_ascii_command_is_a_card_exactly_as_sent() {
    let every_printable_character: String = (' '..='~').collect();
    for text in [r#"/bin/zsh -lc 'ls | xargs rm -rf "$HOME/x"'"#, "echo $PATH && cat <<'EOF' ; ! # ~ `date` ${HOME}", every_printable_character.as_str()] {
        let shown = card(map(&command(text), &ItemIndex::default()));
        assert_eq!((shown.summary.as_str(), shown.truncated), (text, false));
    }
}

#[test]
fn a_command_that_cleans_to_nothing_is_incomplete() {
    for text in ["\u{200B}\u{200B}\u{2060}", "   ", "\n\n", "\t \u{FEFF}", "\u{A0}\u{3000}"] {
        assert_eq!(open_tab_for(text), OpenTabReason::Incomplete, "{text:?}");
    }
}
