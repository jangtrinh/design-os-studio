//! Studio writes its own files after it has cleared the sources of the CLI whose tab is starting. An entry another agent tab puts there
//! in between would be loaded by the CLI, so Studio looks once more when it has written, sets that entry aside too, and stops the tab.

use super::test_support::{aside_path, is_there, plant, tab_state_in};
use crate::terminal::{AgentCliPreset, PermissionMode};

#[test]
fn a_hook_file_that_appears_while_studio_writes_stops_a_grok_tab_and_is_set_aside() {
    let mut st = tab_state_in(PermissionMode::Ask);
    let config = st.session_dir.join(".grok/config.toml");
    let late = st.session_dir.join(".grok/hooks/late.json");
    let (hooks, planted) = (st.session_dir.clone(), late.clone());
    plant(&hooks, ".grok/hooks/designos-status.json", "{}");
    super::session_io::test_hooks::set_before_read_back(&config, Box::new(move || std::fs::write(&planted, "{}").expect("another agent tab plants a hook")));

    let message = st.spawn_tab(AgentCliPreset::Grok, None).expect_err("an entry that came back while Studio wrote must stop the tab");

    assert_eq!(
        message,
        format!("Couldn't start Grok Build: {} changed while Studio wrote it. Close this file's other agent tabs, then try again.", late.display())
    );
    assert!(st.tabs.is_empty(), "no tab was added");
    assert!(!is_there(&late), "it is no longer where Grok would load it");
    assert!(is_there(&aside_path(&st.session_dir, ".grok__hooks__late.json")), "it is kept, not deleted");
}

#[test]
fn a_plugin_that_appears_while_studio_writes_stops_an_agy_tab_and_is_set_aside() {
    let mut st = tab_state_in(PermissionMode::Ask);
    let hooks = st.session_dir.join(".agents/hooks.json");
    let late = st.session_dir.join(".agents/plugins/late");
    let planted = st.session_dir.clone();
    super::session_io::test_hooks::set_before_read_back(&hooks, Box::new(move || plant(&planted, ".agents/plugins/late/plugin.json", "{}")));

    let message = st.spawn_tab(AgentCliPreset::Antigravity, None).expect_err("an entry that came back while Studio wrote must stop the tab");

    assert_eq!(
        message,
        format!("Couldn't start Antigravity (agy): {} changed while Studio wrote it. Close this file's other agent tabs, then try again.", late.display())
    );
    assert!(st.tabs.is_empty(), "no tab was added");
    assert!(!is_there(&late), "it is no longer where agy would load it");
    assert!(is_there(&aside_path(&st.session_dir, ".agents__plugins__late/plugin.json")), "it is kept whole, not deleted");
}
