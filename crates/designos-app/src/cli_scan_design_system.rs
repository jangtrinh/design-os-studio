//! `designos FILE --scan-design-system OUT.json`: writes the design-system scan file the DESIGN:OS kernel reads
//! (`ui ingest-figma-ds`), from the document alone. It reads the document and writes one file: the path follows
//! the same rules as `--export-fig` (see `cli_export_write`), so an existing file is replaced only with `--overwrite`.

use crate::cli_export_write;
use std::path::Path;

/// Stops a run whose output path is unusable before anything is loaded.
pub fn check_target(out_path: &Path, overwrite: bool) -> Result<(), String> {
    cli_export_write::check_target(out_path, "json", overwrite)
}

/// Scans `doc`, writes the file, and returns the short report printed on stdout.
pub fn run(doc: &designos_doc::Document, out_path: &Path, overwrite: bool) -> Result<serde_json::Value, String> {
    let scan = designos_fig::scan_design_system(doc).map_err(|e| format!("Cannot scan the design system: {e}"))?;
    let text = scan.to_json_text();
    cli_export_write::write_export(out_path, "json", text.as_bytes(), overwrite)?;
    Ok(serde_json::json!({
        "scanned_design_system": true,
        "path": out_path,
        "bytes": text.len(),
        "counts": scan.counts,
        "frames": scan.frames.as_ref().map(Vec::len),
        "sensing": scan.sensing,
    }))
}
