//! Which library project a docs tool reads: the one named by the caller, else the one the open file lives in,
//! else the only one there is.

use crate::tools::errors::{ErrorCode, ToolError};
use designos_library::{Library, Project};
use serde_json::json;
use std::path::Path;

/// The projects of the library at `root`. A missing library has none; nothing is created.
fn projects_of(root: &Path) -> Vec<Project> {
    if !root.is_dir() {
        return Vec::new();
    }
    Library::open(root).map(|lib| lib.list_projects()).unwrap_or_default()
}

/// The id of the project folder `file` sits in (`<root>/projects/<id>/...`), if it sits in one.
fn project_id_of_file(root: &Path, file: &str) -> Option<String> {
    let rel = Path::new(file).strip_prefix(root.join("projects")).ok()?;
    Some(rel.components().next()?.as_os_str().to_str()?.to_string())
}

/// Resolves the project. `requested` matches a project id, or else a project name that exactly one project has.
pub fn resolve_project(root: &Path, requested: Option<&str>, open_file: Option<&str>) -> Result<Project, ToolError> {
    let projects = projects_of(root);
    let ids = || projects.iter().map(|p| p.id.clone());
    if projects.is_empty() {
        return Err(ToolError::new(ErrorCode::InvalidArguments, "/project", "the library has no projects, so there are no docs to read")
            .with_next_step("Stop and tell the user: create a project in DESIGN:OS Studio (Home, New project) first."));
    }
    if let Some(wanted) = requested {
        if let Some(p) = projects.iter().find(|p| p.id == wanted) {
            return Ok(p.clone());
        }
        let by_name: Vec<&Project> = projects.iter().filter(|p| p.name == wanted).collect();
        return match by_name.as_slice() {
            [one] => Ok((*one).clone()),
            [] => Err(unknown_project(wanted, "is not a project id or name in this library", ids())),
            _ => Err(unknown_project(wanted, "is the name of more than one project", by_name.iter().map(|p| p.id.clone()))),
        };
    }
    if let Some(p) = open_file.and_then(|f| project_id_of_file(root, f)).and_then(|id| projects.iter().find(|p| p.id == id)) {
        return Ok(p.clone());
    }
    match projects.as_slice() {
        [only] => Ok(only.clone()),
        _ => Err(ToolError::new(ErrorCode::MissingField, "/project", "the library has several projects and the open file is not in one; pass `project`")
            .with_expected("a project id from valid_options")
            .with_valid_options(ids())),
    }
}

fn unknown_project(wanted: &str, why: &str, options: impl Iterator<Item = String>) -> ToolError {
    ToolError::new(ErrorCode::InvalidArguments, "/project", format!("`{wanted}` {why}; fix: pass a project id from valid_options"))
        .with_received(json!(wanted))
        .with_expected("a project id from valid_options")
        .with_valid_options(options)
}
