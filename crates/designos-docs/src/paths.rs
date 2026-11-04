//! Page paths come from the manifest, the tree UI and MCP clients, so they are validated before any disk access.

/// Why `rel` is not an acceptable page path, or `None` when it is: a relative `/`-separated path of plain names
/// ending in `.md`, with no `..`, no hidden (dot) names and no characters that mean something to the filesystem.
pub(crate) fn page_path_problem(rel: &str) -> Option<String> {
    if rel.is_empty() {
        return Some("is empty".into());
    }
    if rel.len() > 512 {
        return Some("is longer than 512 bytes".into());
    }
    if rel.starts_with('/') {
        return Some("is absolute".into());
    }
    if let Some(c) = rel.chars().find(|c| matches!(c, '\\' | ':' | '\0') || c.is_control()) {
        return Some(format!("contains the character {c:?}"));
    }
    for part in rel.split('/') {
        match part {
            "" => return Some("has an empty path segment".into()),
            "." | ".." => return Some("contains `.` or `..`".into()),
            p if p.starts_with('.') => return Some(format!("has the hidden segment `{p}`")),
            p if p.len() > 255 => return Some("has a segment longer than 255 bytes".into()),
            _ => {}
        }
    }
    if !rel.ends_with(".md") || rel.ends_with("/.md") {
        return Some("is not a Markdown file (`.md`)".into());
    }
    None
}

/// A project id is one plain folder name under `<library>/projects/`.
pub(crate) fn project_id_problem(id: &str) -> Option<String> {
    if id.is_empty() || id == "." || id == ".." || id.contains(['/', '\\', '\0']) {
        return Some("must be a single folder name".into());
    }
    None
}
