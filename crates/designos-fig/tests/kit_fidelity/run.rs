//! The kit run: one raw decode and one import of the kit clone, every part in turn, then `scoreboard.json` and
//! `scoreboard.md` in `SCOREBOARD_OUT`. Prints node ids, keys and numbers only; never layer names or text.

use super::board::{self, Part};
use super::keys::NodeKeys;
use super::{census, geometry, import_cov, instance_hydration, instances, layout, raw, report, save_class, save_loss, stored_use, text};
use serde_json::json;
use std::collections::HashSet;
use std::time::Instant;

pub const PARTS: [&str; 9] =
    ["auto_layout", "auto_layout_measurer", "import_coverage", "save_loss", "save_class", "geometry", "instances", "text_layout", "stored_use"];

/// Whether part `id` is in the `SCOREBOARD_PARTS` list `only` (`None` selects every part). `layout` selects both
/// auto layout rows and `text` the text leg, and `auto_layout` also selects the row with the text measurer installed.
pub fn selected(only: Option<&HashSet<String>>, id: &str) -> bool {
    let Some(only) = only else { return true };
    let alias = match id {
        "auto_layout" | "auto_layout_measurer" => "layout",
        "text_layout" => "text",
        _ => "",
    };
    only.contains(id) || only.contains(alias) || (id == "auto_layout_measurer" && only.contains("auto_layout"))
}

pub fn run_kit() {
    // The test is ignored by default, so a run without a kit is a misconfigured kit run: fail it loudly.
    let kit =
        std::env::var_os("DESIGNOS_KIT_PATH").or_else(|| std::env::var_os("KIT_CLONE")).expect("DESIGNOS_KIT_PATH (or KIT_CLONE) must name the kit clone");
    let out = std::path::PathBuf::from(std::env::var_os("SCOREBOARD_OUT").expect("SCOREBOARD_OUT"));
    let only: Option<HashSet<String>> = std::env::var("SCOREBOARD_PARTS").ok().map(|s| s.split(',').map(|x| x.trim().to_string()).collect());
    run_bytes(&std::fs::read(&kit).expect("read the kit clone"), &out, only.as_ref());
}

/// Every selected part on one `.fig`, written to `out`.
pub fn run_bytes(bytes: &[u8], out: &std::path::Path, only: Option<&HashSet<String>>) {
    let want = |id: &str| selected(only, id);
    let t0 = Instant::now();
    let mut timings: Vec<(String, f64)> = Vec::new();
    let lap = |what: &str, timings: &mut Vec<(String, f64)>| {
        timings.push((what.to_string(), t0.elapsed().as_secs_f64()));
        eprintln!("[{:>7.1}s] {what}", t0.elapsed().as_secs_f64());
    };
    let r = raw::decode(bytes);
    let presence = raw::presence(&r);
    for (k, n) in &presence {
        eprintln!("PRESENCE {k}: {}", board::thousands(*n));
    }
    let rec_keys = want("import_coverage").then(|| import_cov::truth(&r));
    let geo = (want("geometry") || want("stored_use")).then(|| geometry::truth(&r));
    let inst = (want("instances") || want("stored_use")).then(|| instances::truth(&r));
    let txt = (want("text_layout") || want("stored_use")).then(|| text::truth(&r));
    let empty_derived = want("stored_use").then(|| stored_use::empty_derived(&r));
    let derived: HashSet<String> = r.live().filter(|x| x.get("derivedSymbolData").is_some()).filter_map(raw::record_guid).collect();
    let blobs = r.blobs;
    drop(r.records);
    lap("raw decode and truth extraction", &mut timings);

    let mut doc = designos_fig::import_fig(bytes).expect("import the kit");
    lap("import", &mut timings);
    let (saved, kept_notes) = if want("save_loss") || want("save_class") {
        let (bytes, report) = designos_fig::export_fig_with_kept_report(&doc).expect("no-edit save");
        (Some(bytes), Some(kept_write_notes(&doc, &report)))
    } else {
        (None, None)
    };
    if let Some(s) = &saved {
        eprintln!("NO-EDIT SAVE bytes: {}", s.len());
        lap("no-edit save", &mut timings);
    }
    let census_before = (want("auto_layout") || want("auto_layout_measurer")).then(|| census::census(&mut doc));
    let deferred = inst.as_ref().map(|t| instance_hydration::deferred(t, &doc)).unwrap_or_default();
    doc.hydrate_all_pages();
    let keys = NodeKeys::build(&doc);
    lap("census before hydration, hydrate all pages", &mut timings);

    let mut parts: Vec<Part> = Vec::new();
    if let (true, Some(t), Some(g), Some((tx, _)), Some(empty)) = (want("stored_use"), &inst, &geo, &txt, empty_derived) {
        let text_guids: Vec<String> = tx.iter().map(|x| x.guid.clone()).collect();
        parts.push(stored_use::part(t, g, &text_guids, empty, &doc, &keys));
        lap("stored-data use", &mut timings);
    }
    if let Some(c0) = census_before {
        let c1 = census::census(&mut doc);
        let refused_holders = layout::refused_holders(&doc, &keys, &derived);
        if want("auto_layout_measurer") {
            parts.push(layout::measurer_part(&doc, &keys, &refused_holders));
            lap("auto layout, text measurer installed", &mut timings);
        }
        if want("auto_layout") {
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
            parts.push(p);
        }
        lap("auto layout", &mut timings);
    }
    if let Some(rk) = &rec_keys {
        parts.push(import_cov::part(rk, &doc, &keys));
        lap("import coverage", &mut timings);
    }
    let hidden_guids: HashSet<String> = keys.by_source.iter().filter(|(_, id)| keys.is_hidden(**id)).map(|(g, _)| g.clone()).collect();
    let mut late: Vec<Part> = Vec::new();
    if let Some(g) = geo.as_ref().filter(|_| want("geometry")) {
        late.push(geometry::part(g, &blobs, &doc, &keys));
        lap("shape and stroke geometry", &mut timings);
    }
    let mut text_part = None;
    if let Some((t, without)) = txt.as_ref().filter(|_| want("text_layout")) {
        text_part = Some(text::part(t, *without, &doc, &keys));
        lap("text layout", &mut timings);
    }
    if let Some(t) = inst.as_ref().filter(|_| want("instances")) {
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
        let original = raw::decode(bytes);
        let saved_raw = raw::decode(&saved);
        let raw_loss = want("save_loss").then(|| save_loss::part(&original, &saved_raw, &hidden_guids));
        if want("save_class") {
            let mut p = save_class::part(&original, &saved_raw, &hidden_guids);
            let rate = raw_loss.as_ref().map(|l| l.bucket("total occurrences")[0]);
            p.note(match rate {
                Some(t) => format!(
                    "raw rate beside it (every compared key counted as the save loss part counts it): {} / {} = {}",
                    board::thousands(t.matched),
                    board::thousands(t.measured),
                    board::pct(t)
                ),
                None => "raw rate beside it: not run (add save_loss to SCOREBOARD_PARTS)".to_string(),
            });
            for n in kept_notes.iter().flatten() {
                p.note(n.clone());
            }
            parts.push(p);
            lap("save loss by class", &mut timings);
        }
        if let Some(l) = raw_loss {
            parts.push(l);
            lap("save loss", &mut timings);
        }
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
    report::write(out, &parts, meta, regressions.as_ref(), &deltas);
}

/// What the no-edit save wrote from the records the open kept, as scoreboard notes. A guid at a place no rule lists, or a field
/// the save could not decode, fails the run: a field Figma added must be classified before it is written.
fn kept_write_notes(doc: &designos_doc::Document, report: &designos_fig::KeptWriteReport) -> Vec<String> {
    for loud in ["id_at_an_unlisted_path", "hash_at_an_unlisted_path", "decode_refused", "field_unknown_to_the_schema"] {
        assert!(!report.fields_dropped_by_reason.contains_key(loud), "the save met a kept field it cannot classify ({loud}): {report:?}");
    }
    let mut notes = vec![format!(
        "no-edit save wrote from the store: {} layers ({} records), {} fields; layers edited (snapshot mismatch, Studio's record written): {}; layers under another guid: {}; fields dropped: {} {:?}",
        board::thousands(report.nodes_written as u64),
        board::thousands(report.records_written as u64),
        board::thousands(report.fields_written as u64),
        report.nodes_edited,
        report.nodes_guid_mismatch,
        report.fields_dropped(),
        report.fields_dropped_by_reason,
    )];
    for id in &report.edited_sample {
        if let Some(n) = doc.get(*id) {
            notes.push(format!("false edit: {:?} {:?} {:?}", n.source_id, n.name, std::mem::discriminant(&n.kind)));
        }
    }
    notes
}
