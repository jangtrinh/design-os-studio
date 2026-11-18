//! Writes `scoreboard.json` and `scoreboard.md` and prints the per-bucket lines.

use super::board::{self, Part};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::fmt::Write as _;

pub fn write(out: &std::path::Path, parts: &[Part], meta: Value, regressions: Option<&BTreeMap<String, Vec<String>>>, deltas: &[String]) {
    std::fs::create_dir_all(out).expect("create SCOREBOARD_OUT");
    for p in parts {
        for l in board::lines(p) {
            println!("{l}");
        }
    }
    if let Some(reg) = regressions {
        let total: usize = reg.values().map(Vec::len).sum();
        println!("REGRESSIONS vs SCOREBOARD_PREV: {total}");
        for d in deltas {
            println!("{d}");
        }
        for (part, ids) in reg {
            println!("REGRESSIONS {part}: {}", ids.len());
            for id in ids.iter().take(200) {
                println!("REGRESSED {part} {id}");
            }
            if ids.len() > 200 {
                println!("REGRESSED {part}: {} more in scoreboard.json", ids.len() - 200);
            }
        }
    }
    let doc = json!({
        "meta": meta,
        "parts": parts.iter().map(board::part_json).collect::<Vec<_>>(),
        "regressions": regressions,
        "count_deltas": deltas,
    });
    std::fs::write(out.join("scoreboard.json"), serde_json::to_vec(&doc).expect("serialise")).expect("write scoreboard.json");
    std::fs::write(out.join("scoreboard.md"), markdown(parts, &meta, regressions)).expect("write scoreboard.md");
}

fn markdown(parts: &[Part], meta: &Value, regressions: Option<&BTreeMap<String, Vec<String>>>) -> String {
    let mut s = String::from("# Kit fidelity scoreboard\n\n");
    let _ = writeln!(s, "Wall time: {:.0} s. Kit bytes: {}.\n", meta["wall_s"].as_f64().unwrap_or(0.0), meta["kit_bytes"]);
    if let Some(pr) = meta["presence"].as_array() {
        s.push_str("| Stored field | Records |\n|---|---|\n");
        for e in pr {
            let _ = writeln!(s, "| {} | {} |", e["key"].as_str().unwrap_or(""), board::thousands(e["records"].as_u64().unwrap_or(0)));
        }
        s.push('\n');
    }
    for p in parts {
        let _ = writeln!(s, "## {}\n", p.title);
        if let Some(r) = &p.not_measured {
            let _ = writeln!(s, "NOT MEASURED: {r}\n");
            continue;
        }
        let _ = writeln!(s, "Pass rule: {}.\n", p.rule);
        for n in &p.notes {
            let _ = writeln!(s, "- {n}");
        }
        s.push_str("\n| Bucket | Visible matched / measured | % | Hidden (not scored) |\n|---|---|---|---|\n");
        for (b, [v, h]) in &p.buckets {
            let _ = writeln!(
                s,
                "| {b} | {} / {} | {} | {} / {} |",
                board::thousands(v.matched),
                board::thousands(v.measured),
                board::pct(*v),
                board::thousands(h.matched),
                board::thousands(h.measured)
            );
        }
        if !p.unscored.is_empty() {
            s.push_str("\n| Not measured | Items | Why |\n|---|---|---|\n");
            for (b, (n, why)) in &p.unscored {
                let _ = writeln!(s, "| {b} | {} | {why} |", board::thousands(*n));
            }
        }
        if !p.details.is_empty() {
            s.push_str("\n| Bucket | Breakdown |\n|---|---|\n");
            for (b, m) in &p.details {
                let mut v: Vec<(&String, &u64)> = m.iter().collect();
                v.sort_by(|a, b| b.1.cmp(a.1).then(a.0.cmp(b.0)));
                let txt: Vec<String> = v.iter().take(12).map(|(k, n)| format!("{k} {}", board::thousands(**n))).collect();
                let _ = writeln!(s, "| {b} | {} |", txt.join("; "));
            }
        }
        let failed: Vec<&String> = p.failed.iter().take(20).collect();
        let _ = writeln!(
            s,
            "\nFailing visible items: {} (first {}: {})\n",
            board::thousands(p.failed.len() as u64),
            failed.len(),
            failed.iter().map(|x| x.as_str()).collect::<Vec<_>>().join(", ")
        );
        if let Some(ids) = regressions.and_then(|r| r.get(p.id)) {
            let _ = writeln!(s, "Regressions vs the previous run: {} ({})\n", ids.len(), ids.iter().take(50).cloned().collect::<Vec<_>>().join(", "));
        }
        if let Some(ids) = regressions.and_then(|r| r.get(&format!("{}: left measurement", p.id))) {
            let _ = writeln!(s, "Matched before, no longer measured: {} ({})\n", ids.len(), ids.iter().take(50).cloned().collect::<Vec<_>>().join(", "));
        }
    }
    s
}
