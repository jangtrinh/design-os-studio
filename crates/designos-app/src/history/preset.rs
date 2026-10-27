//! `--panel version-history`: a library file with a believable history, the panel
//! open and one version on the canvas. For screenshots.

use crate::app::App;
use std::path::PathBuf;

const MIN: u64 = 60_000;
const HOUR: u64 = 60 * MIN;
const DAY: u64 = 24 * HOUR;

pub fn apply(app: &mut App, library: Option<PathBuf>) {
    let root = library.unwrap_or_else(|| std::env::temp_dir().join(format!("designos_version_history_shot_{}", std::process::id())));
    let _ = std::fs::remove_dir_all(&root);
    app.attach_library(root);
    let Some(lib) = app.lib.clone() else { return };
    let Some(project) = lib.list_projects().first().map(|p| p.id.clone()) else { return };
    let Ok(mut doc) = crate::home::load_starter_document() else { return };
    doc.name = "DESIGN-OS Studio Starter".into();
    let Ok(entry) = lib.create_file(&project, None, "DESIGN-OS Studio Starter", &doc) else { return };
    // older states have fewer layers, so versions differ
    let mut older = doc.clone();
    let drop: Vec<_> = older.nodes.values().filter(|n| matches!(n.name.as_str(), "Progress" | "Avatar")).map(|n| n.id).collect();
    for id in drop {
        let _ = older.remove(id);
    }
    let now = crate::comments::time::now_ms();
    let at = |ago: u64| now.saturating_sub(ago);
    let save = |d: &designos_doc::Document, label: Option<&str>, desc: Option<&str>, auto: bool, ago: u64| {
        let _ = lib.save_version_at(&entry.id, d, label.map(str::to_string), desc.map(str::to_string), auto, at(ago));
    };
    save(&older, Some("Initial layout"), Some("First pass of the welcome screen"), false, 5 * DAY + 3 * HOUR);
    save(&older, None, None, true, 2 * DAY + 5 * HOUR);
    save(&older, None, None, true, DAY + 2 * HOUR);
    save(&doc, Some("Ready for review"), Some("Card and dashboard added"), false, 26 * HOUR);
    save(&doc, None, None, true, 3 * HOUR);
    save(&doc, None, None, true, 2 * HOUR + 10 * MIN);
    save(&doc, Some("Before the colour pass"), None, false, 55 * MIN);
    save(&doc, None, None, true, 20 * MIN);
    save(&doc, None, None, true, 6 * MIN);
    app.open_library_file(&entry.id);
    app.history.open = true;
    app.history.dirty = true;
    // view the named version from yesterday
    if let Some(n) = lib.list_versions(&entry.id).iter().find(|v| v.label.as_deref() == Some("Initial layout")).map(|v| v.n) {
        app.view_version(n);
    }
}
