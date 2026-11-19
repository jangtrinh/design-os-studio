//! The kit run: one raw decode and one import of the kit clone, every part in turn, then `scoreboard.json` and
//! `scoreboard.md` in `SCOREBOARD_OUT`. Prints node ids, keys and numbers only; never layer names or text.

use super::board::{self, Part};
use super::keys::NodeKeys;
use super::{census, geometry, import_cov, instance_hydration, instances, layout, raw, report, save_loss, text};
use serde_json::json;
use std::collections::HashSet;
use std::time::Instant;

pub const PARTS: [&str; 6] = ["auto_layout", "import_coverage", "save_loss", "geometry", "instances", "text_layout"];

pub fn run_kit() {
    // The test is ignored by default, so a run without a kit is a misconfigured kit run: fail it loudly.
    let kit =
        std::env::var_os("DESIGNOS_KIT_PATH").or_else(|| std::env::var_os("KIT_CLONE")).expect("DESIGNOS_KIT_PATH (or KIT_CLONE) must name the kit clone");
    let out = std::path::PathBuf::from(std::env::var_os("SCOREBOARD_OUT").expect("SCOREBOARD_OUT"));
    let only: Option<HashSet<String>> = std::env::var("SCOREBOARD_PARTS").ok().map(|s| s.split(',').map(|x| x.trim().to_string()).collect());
    let want = |id: &str| only.as_ref().is_none_or(|o| o.contains(id));
    let t0 = Instant::now();
    let mut timings: Vec<(String, f64)> = Vec::new();
    let lap = |what: &str, timings: &mut Vec<(String, f64)>| {
        timings.push((what.to_string(), t0.elapsed().as_secs_f64()));
        eprintln!("[{:>7.1}s] {what}", t0.elapsed().as_secs_f64());
    };
    let bytes = std::fs::read(&kit).expect("read the kit clone");

    let r = raw::decode(&bytes);
    let presence = raw::presence(&r);
    for (k, n) in &presence {
        eprintln!("PRESENCE {k}: {}", board::thousands(*n));
    }
    let rec_keys = want("import_coverage").then(|| import_cov::truth(&r));
    let geo = want("geometry").then(|| geometry::truth(&r));
    let inst = want("instances").then(|| instances::truth(&r));
    let txt = want("text_layout").then(|| text::truth(&r));
    let derived: HashSet<String> = r.live().filter(|x| x.get("derivedSymbolData").is_some()).filter_map(raw::record_guid).collect();
    let blobs = r.blobs;
    drop(r.records);
    lap("raw decode and truth extraction", &mut timings);

    let mut doc = designos_fig::import_fig(&bytes).expect("import the kit");
    lap("import", &mut timings);
    let saved = want("save_loss").then(|| designos_fig::export_fig(&doc).expect("no-edit save"));
    if let Some(s) = &saved {
        eprintln!("NO-EDIT SAVE bytes: {}", s.len());
        lap("no-edit save", &mut timings);
    }
    let census_before = want("auto_layout").then(|| census::census(&mut doc));
    let deferred = inst.as_ref().map(|t| instance_hydration::deferred(t, &doc)).unwrap_or_default();
    doc.hydrate_all_pages();
    let keys = NodeKeys::build(&doc);
    lap("census before hydration, hydrate all pages", &mut timings);

    let mut parts: Vec<Part> = Vec::new();
    if let Some(c0) = census_before {
        let c1 = census::census(&mut doc);
        let refused_holders = layout::refused_holders(&doc, &keys, &derived);
        let mut p = layout::part(&doc, &keys, &refused_holders);
        p.note(format!("instances left out because hydration refused their stored layout: {}", refused_holders.len()));
        p.note(format!(
            "census as kit_auto_layout_census.rs runs it (no page hydrated): {} of {} page layers move > 0.5 px ({} in hidden subtrees, {} visible); {} auto-layout frames on pages",
            c0.moved, c0.page_layers, c0.moved_hidden, c0.moved - c0.moved_hidden, c0.auto_frames_on_pages
        ));
        p.note(format!(
            "same census with every page hydrated: {} of {} page layers move ({} hidden, {} visible); {} auto-layout frames on pages",
            c1.moved,
            c1.page_layers,
            c1.moved_hidden,
            c1.moved - c1.moved_hidden,
            c1.auto_frames_on_pages
        ));
        p.note(format!("text measurer installed: {}", designos_doc::text_measure::layout_measurer().is_some()));
        parts.push(p);
        lap("auto layout", &mut timings);
    }
    if let Some(rk) = &rec_keys {
        parts.push(import_cov::part(rk, &doc, &keys));
        lap("import coverage", &mut timings);
    }
    let hidden_guids: HashSet<String> = keys.by_source.iter().filter(|(_, id)| keys.is_hidden(**id)).map(|(g, _)| g.clone()).collect();
    let mut late: Vec<Part> = Vec::new();
    if let Some(g) = &geo {
        late.push(geometry::part(g, &blobs, &doc, &keys));
        lap("shape and stroke geometry", &mut timings);
    }
    let mut text_part = None;
    if let Some((t, without)) = &txt {
        text_part = Some(text::part(t, *without, &doc, &keys));
        lap("text layout", &mut timings);
    }
    if let Some(t) = &inst {
        let refused = instance_hydration::refused(t, &doc, &keys);
        let synced = instance_hydration::sync_all(&mut doc);
        lap("no-change sync of every main", &mut timings);
        let keys_after = NodeKeys::build(&doc);
        let mut p = instances::part(t, &doc, &keys_after, &refused);
        p.note(format!(
            "instances whose stored derived data hydration refused (derived data in the file, no imported-layout record after hydration): {}, of which {} on pages the import deferred and {} on pages loaded at import",
            refused.len(),
            refused.intersection(&deferred).count(),
            refused.difference(&deferred).count()
        ));
        p.note(format!("instance records with derived data: {}; mains synced: {synced}", t.instances.len()));
        let (holders, no_edits) = instance_hydration::refused_split(t, &doc, &keys_after, &refused);
        p.note(format!("of the refused instances, top-level instances (no other instance holds them): {holders}; of those, with no override: {no_edits}"));
        late.push(p);
        lap("instances", &mut timings);
    }
    late.extend(text_part);
    drop(doc);
    drop(keys);
    if let Some(saved) = saved {
        let original = raw::decode(&bytes);
        let saved_raw = raw::decode(&saved);
        parts.push(save_loss::part(&original, &saved_raw, &hidden_guids));
        lap("save loss", &mut timings);
    }
    parts.extend(late);
    for &id in PARTS.iter().filter(|id| !want(id)) {
        let mut p = Part::new(id, id, "");
        p.not_measured = Some("left out of this run by SCOREBOARD_PARTS".into());
        parts.push(p);
    }

    let prev = std::env::var_os("SCOREBOARD_PREV")
        .map(|p| serde_json::from_slice::<serde_json::Value>(&std::fs::read(p).expect("read SCOREBOARD_PREV")).expect("parse SCOREBOARD_PREV"));
    let regressions = prev.as_ref().map(|pv| board::regressions(&parts, pv));
    let deltas = prev.as_ref().map(|pv| board::count_deltas(&parts, pv)).unwrap_or_default();
    let meta = json!({
        "kit_bytes": bytes.len(),
        "presence": presence.iter().map(|(k, n)| json!({"key": k, "records": n})).collect::<Vec<_>>(),
        "timings_s": timings.iter().map(|(k, s)| json!({"step": k, "at": s})).collect::<Vec<_>>(),
        "wall_s": t0.elapsed().as_secs_f64(),
        "parts_run": PARTS.iter().filter(|id| want(id)).collect::<Vec<_>>(),
    });
    report::write(&out, &parts, meta, regressions.as_ref(), &deltas);
}
