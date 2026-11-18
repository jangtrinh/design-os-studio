//! Kit fidelity scoreboard (ignored by default): Studio measured against the data Figma stored inside a `.fig`, one
//! feature bucket at a time, visible and hidden subtrees apart.
//!
//! - `DESIGNOS_KIT_PATH` (or `KIT_CLONE`): the kit clone to read.
//! - `SCOREBOARD_OUT`: folder for `scoreboard.json` and `scoreboard.md` (outside the repository: kit data).
//! - `SCOREBOARD_PREV`: an earlier `scoreboard.json`; every item that matched there and fails now is listed.
//! - `SCOREBOARD_PARTS`: comma list of parts to run (default all).
//!
//! The synthetic tests in `kit_fidelity/synthetic.rs` run without a kit and show each part can fail.

mod kit_fidelity;

#[test]
#[ignore = "reads DESIGNOS_KIT_PATH, writes SCOREBOARD_OUT"]
fn kit_fidelity_scoreboard() {
    kit_fidelity::run::run_kit();
}
