//! Per-part tallies of the scoreboard: matched / measured per feature bucket, split into visible and hidden subtrees,
//! with the keys of every visible item so a later run can list the items that newly fail.

use serde_json::{json, Map, Value};
use std::collections::{BTreeMap, BTreeSet};

/// When set, every comparator of the scoreboard reports a match. The synthetic tests then fail, which is how their red
/// output is recorded: a comparison that cannot fail proves nothing.
pub fn broken_comparator() -> bool {
    std::env::var_os("SCOREBOARD_BROKEN_COMPARATOR").is_some()
}

#[derive(Clone, Copy, Default, Debug, PartialEq, Eq)]
pub struct Tally {
    pub matched: u64,
    pub measured: u64,
}

impl Tally {
    fn add(&mut self, ok: bool) {
        self.measured += 1;
        self.matched += u64::from(ok);
    }
}

pub struct Part {
    pub id: &'static str,
    pub title: &'static str,
    pub rule: String,
    /// `Some(reason)` when the part could not be measured honestly.
    pub not_measured: Option<String>,
    /// Bucket -> [visible, hidden].
    pub buckets: BTreeMap<String, [Tally; 2]>,
    /// Buckets that are reported but not scored, with their item count and the reason.
    pub unscored: BTreeMap<String, (u64, String)>,
    /// Per-bucket breakdown counts (for example equal / changed / lost of one record key).
    pub details: BTreeMap<String, BTreeMap<String, u64>>,
    pub matched: BTreeSet<String>,
    pub failed: BTreeSet<String>,
    pub notes: Vec<String>,
}

impl Part {
    pub fn new(id: &'static str, title: &'static str, rule: impl Into<String>) -> Self {
        Part {
            id,
            title,
            rule: rule.into(),
            not_measured: None,
            buckets: BTreeMap::new(),
            unscored: BTreeMap::new(),
            details: BTreeMap::new(),
            matched: BTreeSet::new(),
            failed: BTreeSet::new(),
            notes: Vec::new(),
        }
    }

    /// Scores one item in every bucket it belongs to. Only visible items are kept for the regression list.
    pub fn record(&mut self, item: String, hidden: bool, ok: bool, buckets: &[String]) {
        for b in buckets {
            self.tally(b, hidden, ok);
        }
        if !hidden {
            if ok {
                self.matched.insert(item);
            } else {
                self.failed.insert(item);
            }
        }
    }

    pub fn tally(&mut self, bucket: &str, hidden: bool, ok: bool) {
        self.buckets.entry(bucket.to_string()).or_default()[usize::from(hidden)].add(ok);
    }

    pub fn unscored(&mut self, bucket: &str, reason: &str) {
        let e = self.unscored.entry(bucket.to_string()).or_insert_with(|| (0, reason.to_string()));
        e.0 += 1;
    }

    pub fn note(&mut self, s: impl Into<String>) {
        self.notes.push(s.into());
    }

    pub fn bucket(&self, name: &str) -> [Tally; 2] {
        self.buckets.get(name).copied().unwrap_or_default()
    }
}

pub fn thousands(n: u64) -> String {
    let s = n.to_string();
    let mut out = String::new();
    for (i, ch) in s.chars().enumerate() {
        if i > 0 && (s.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(ch);
    }
    out
}

pub fn pct(t: Tally) -> String {
    if t.measured == 0 {
        "n/a".into()
    } else {
        format!("{:.1}%", t.matched as f64 * 100.0 / t.measured as f64)
    }
}

/// One printed line per bucket, e.g. `Stroke align OUTSIDE: 1,212 / 1,240 = 97.7% (visible)`.
pub fn lines(p: &Part) -> Vec<String> {
    if let Some(r) = &p.not_measured {
        return vec![format!("{}: NOT MEASURED ({r})", p.title)];
    }
    let mut out = Vec::new();
    for (b, [v, h]) in &p.buckets {
        out.push(format!(
            "{} / {b}: {} / {} = {} (visible); hidden {} / {} (not scored)",
            p.title,
            thousands(v.matched),
            thousands(v.measured),
            pct(*v),
            thousands(h.matched),
            thousands(h.measured)
        ));
    }
    for (b, (n, why)) in &p.unscored {
        out.push(format!("{} / {b}: NOT MEASURED, {} items ({why})", p.title, thousands(*n)));
    }
    out
}

fn tally_json(t: Tally) -> Value {
    json!({"matched": t.matched, "measured": t.measured})
}

pub fn part_json(p: &Part) -> Value {
    let buckets: Map<String, Value> = p.buckets.iter().map(|(b, [v, h])| (b.clone(), json!({"visible": tally_json(*v), "hidden": tally_json(*h)}))).collect();
    let unscored: Map<String, Value> = p.unscored.iter().map(|(b, (n, why))| (b.clone(), json!({"items": n, "reason": why}))).collect();
    json!({
        "id": p.id,
        "title": p.title,
        "rule": p.rule,
        "not_measured": p.not_measured,
        "buckets": buckets,
        "unscored": unscored,
        "details": p.details,
        "notes": p.notes,
        "matched": p.matched,
        "failed": p.failed,
    })
}

/// Items that matched in `prev` and fail now, per part.
pub fn regressions(parts: &[Part], prev: &Value) -> BTreeMap<String, Vec<String>> {
    let mut out = BTreeMap::new();
    for p in parts {
        let Some(pp) = prev.get("parts").and_then(Value::as_array).and_then(|a| a.iter().find(|x| x.get("id").and_then(Value::as_str) == Some(p.id))) else {
            continue;
        };
        let before: BTreeSet<&str> = pp.get("matched").and_then(Value::as_array).map(|a| a.iter().filter_map(Value::as_str).collect()).unwrap_or_default();
        let now: Vec<String> = p.failed.iter().filter(|k| before.contains(k.as_str())).cloned().collect();
        out.insert(p.id.to_string(), now);
    }
    out
}
