//! Studio-owned front door: the engines must NOT decide what "supported"
//! means (mermaid-rs-renderer renders 23 types, merman more). v1 supports
//! flowchart + sequence; everything else shows source + a notice.

#[derive(Debug, PartialEq, Eq)]
pub enum Kind {
    Flowchart,
    Sequence,
    Unsupported(String),
}

/// A source with `accTitle` / `accDescr` pulled out. The engines never see
/// those lines (keeps the card title / aria label under Studio's control).
pub struct Prepared {
    pub kind: Kind,
    pub body: String,
    pub acc_title: Option<String>,
    pub acc_descr: Option<String>,
}

pub fn prepare(src: &str) -> Prepared {
    let (mut title, mut descr) = (None, None);
    let mut body = String::new();
    for line in src.lines() {
        let t = line.trim();
        if let Some(v) = t.strip_prefix("accTitle:") {
            title = Some(v.trim().to_string());
            body.push('\n'); // keep line numbers stable for error reporting
        } else if let Some(v) = t.strip_prefix("accDescr:") {
            descr = Some(v.trim().to_string());
            body.push('\n');
        } else {
            body.push_str(line);
            body.push('\n');
        }
    }
    let first = body
        .lines()
        .map(str::trim)
        .find(|l| !l.is_empty() && !l.starts_with("%%"))
        .unwrap_or("");
    let word = first.split(|c: char| c.is_whitespace() || c == ';').next().unwrap_or("");
    let kind = match word {
        "flowchart" | "graph" => Kind::Flowchart,
        "sequenceDiagram" => Kind::Sequence,
        other => Kind::Unsupported(other.to_string()),
    };
    // Experiment switch: hand the engine the untouched source (accTitle/accDescr
    // lines included) to see whether it accepts them natively.
    let body = if std::env::var_os("RAW_ACC").is_some() { src.to_string() } else { body };
    Prepared { kind, body, acc_title: title, acc_descr: descr }
}
