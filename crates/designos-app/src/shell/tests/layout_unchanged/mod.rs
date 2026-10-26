use super::*;
use serde::Deserialize;
use std::path::{Path, PathBuf};

mod d;
mod e;
mod f;
mod g;
mod h;
mod i;
mod j;
mod k;

#[derive(Debug, Clone, Deserialize)]
pub struct CasesFile {
    #[serde(default)]
    pub bless_sha: Option<String>,
    pub cases: Vec<CaseEntry>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CaseEntry {
    pub id: String,
    pub spec: String,
    pub args: Vec<String>,
    #[serde(default)]
    pub kit: bool,
    #[serde(default)]
    pub virtualized: Vec<String>,
    #[serde(default)]
    pub spawns: bool,
}

pub fn baseline_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/layout-baseline")
}

pub fn load_cases_file() -> CasesFile {
    let path = baseline_dir().join("cases.json");
    let raw = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("failed to read {}: {e}", path.display()));
    serde_json::from_str(&raw).unwrap_or_else(|e| panic!("failed to parse {}: {e}", path.display()))
}

pub fn load_allow_entries() -> crate::layout_snapshot::AllowList {
    let path = baseline_dir().join("allow.json");
    let raw = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("failed to read {}: {e}", path.display()));
    crate::layout_snapshot::parse_allow_list(&raw).unwrap_or_else(|e| panic!("failed to parse {}: {e}", path.display()))
}

pub fn run_spec_cases(spec_key: &str) {
    if option_env!("CARGO_BIN_NAME") != Some("designos") {
        return;
    }
    assert!(cfg!(debug_assertions), "Tier A layout gate requires debug_assertions");

    let bdir = baseline_dir();
    let cases_file = load_cases_file();
    let allow_entries = load_allow_entries();
    let bless = std::env::var_os("DESIGNOS_LAYOUT_BLESS").is_some();

    let spec_cases: Vec<&CaseEntry> = cases_file.cases.iter().filter(|c| !c.kit && c.spec == spec_key).collect();
    assert!(!spec_cases.is_empty(), "no headless cases found in cases.json for spec {spec_key}");

    let stub_dir = crate::cli_resolve::stub_dir().to_path_buf();
    let pid = std::process::id();

    for (idx, case) in spec_cases.into_iter().enumerate() {
        let _ = case.spawns;
        let case_tmp = std::env::temp_dir().join(format!("designos-tier-a-{}-{}-{pid}-{idx}", spec_key.to_ascii_lowercase(), case.id));
        let _ = std::fs::remove_dir_all(&case_tmp);
        std::fs::create_dir_all(&case_tmp).expect("create case temp dir");

        let mut expanded_args = Vec::with_capacity(case.args.len());
        let mut seeded_lib: Option<PathBuf> = None;
        for arg in &case.args {
            if arg.contains("{LIB}") {
                let lib_dir = seeded_lib.get_or_insert_with(|| {
                    let d = case_tmp.join("seed-lib");
                    crate::showcase::seed_library_at(&d, 1_700_000_000_000).expect("seed deterministic library");
                    d
                });
                expanded_args.push(arg.replace("{LIB}", &lib_dir.to_string_lossy()));
            } else {
                expanded_args.push(arg.clone());
            }
        }

        let mut opts = crate::cli::parse(expanded_args).unwrap_or_else(|e| panic!("case {} failed cli::parse: {e}", case.id));
        opts.size = Some((1440, 900));
        opts.scale = Some(1.0);
        opts.no_toast = true;
        opts.agent_cli_dir = Some(stub_dir.clone());
        opts.fixed_now = Some(1_700_000_060_000);

        {
            let _scoped = crate::cli_resolve::ScopedCliDirs::enter(stub_dir.clone());
            let mut sim = Sim::with(opts);
            sim.shell.apply_launch_defaults(case.id != "d-window-chrome");
            crate::theme::apply(&sim.ctx);
            sim.ctx.all_styles_mut(|s| s.debug.show_interactive_widgets = true);

            for _ in 0..8 {
                sim.frame(vec![]);
            }
            if case.id == "i-agents-start-failure" {
                let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
                while std::time::Instant::now() < deadline {
                    let alive = sim.shell.app.terminal.tabs.first_mut().is_some_and(|t| t.is_alive());
                    if !alive {
                        break;
                    }
                    std::thread::sleep(std::time::Duration::from_millis(10));
                }
                for _ in 0..2 {
                    sim.frame(vec![]);
                }
            }

            let mut snap = crate::layout_snapshot::capture(&sim.ctx, &sim.shell.app.ui.regions);
            snap.case = case.id.clone();
            assert!(!snap.named.is_empty() || !snap.unnamed.is_empty(), "case {} produced an empty widget snapshot", case.id);

            let baseline_path = bdir.join(format!("{}.layout.json", case.id));
            if bless {
                let json = serde_json::to_string_pretty(&snap).expect("serialize layout snapshot");
                std::fs::write(&baseline_path, format!("{json}\n")).expect("write blessed layout baseline");
            } else if cases_file.bless_sha.as_deref().is_some_and(|s| !s.is_empty()) || baseline_path.exists() {
                if let Err(err) = crate::layout_snapshot::load_and_compare_baseline(&baseline_path, &snap, &allow_entries, &case.virtualized) {
                    panic!("case {} failed layout gate:\n{err}", case.id);
                }
            }
        }

        let _ = std::fs::remove_dir_all(&case_tmp);
    }
}

#[test]
fn cases_json_and_allow_json_match_spec_l_contract() {
    let cases_file = load_cases_file();
    let _allow = load_allow_entries();

    let headless: Vec<&CaseEntry> = cases_file.cases.iter().filter(|c| !c.kit).collect();
    let kit: Vec<&CaseEntry> = cases_file.cases.iter().filter(|c| c.kit).collect();
    assert_eq!(headless.len(), 74, "expected 74 headless cases in cases.json, got {}", headless.len());
    assert_eq!(kit.len(), 2, "expected 2 kit cases in cases.json, got {}", kit.len());

    for spec_key in ["D", "E", "F", "G", "H", "I", "J", "K"] {
        assert!(headless.iter().any(|c| c.spec == spec_key), "expected at least one headless case for spec {spec_key}");
    }

    for c in &headless {
        let args: Vec<String> = c.args.iter().map(|a| a.replace("{LIB}", "/tmp/designos-lib")).collect();
        crate::cli::parse(args).unwrap_or_else(|e| panic!("headless case {} failed cli::parse: {e}", c.id));
    }
}
