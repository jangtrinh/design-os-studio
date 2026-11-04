//! Which library project a docs tool reads: the one named by the caller, else the one the open file lives in,
//! else the only one there is.

use crate::tools::errors::{ErrorCode, ToolError};
use designos_library::{Library, Project};
use std::path::{Path, PathBuf};

/// An id the library makes (`new_id`): lowercase hex.
fn is_library_id(name: &str) -> bool {
    !name.is_empty() && name.len() <= 64 && name.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
}

/// The projects of the library at `root`, oldest first. A missing library has none; nothing is created. A project is
/// listed only when its folder is named with a library id and its `project.json` says that same id: the id is a path
/// segment and is echoed in results, so one that disagrees with its folder, or is free text, is not trusted.
fn projects_of(root: &Path) -> Vec<Project> {
    let Ok(entries) = std::fs::read_dir(root.join("projects")) else { return Vec::new() };
    let Ok(lib) = Library::open(root) else { return Vec::new() };
    let mut out: Vec<Project> = entries
        .flatten()
        .filter_map(|e| {
            let name = e.file_name().to_str()?.to_string();
            let project = lib.project(&name).ok().filter(|_| is_library_id(&name))?;
            (project.id == name).then_some(project)
        })
        .collect();
    out.sort_by(|a, b| a.created.cmp(&b.created).then_with(|| a.name.cmp(&b.name)));
    out
}

/// The id of the project folder `file` sits in (`<root>/projects/<id>/...`), if it sits in one. Both sides are compared
/// with their symlinks resolved (a library reached through `/tmp` or a linked folder), and as given.
fn project_id_of_file(root: &Path, file: &str) -> Option<String> {
    let real = |p: &Path| p.canonicalize().unwrap_or_else(|_| p.to_path_buf());
    let projects = root.join("projects");
    [(real(&projects), real(Path::new(file))), (projects, PathBuf::from(file))].into_iter().find_map(|(base, file)| {
        let rel = file.strip_prefix(&base).ok()?;
        Some(rel.components().next()?.as_os_str().to_str()?.to_string())
    })
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
            [] => Err(unknown_project(wanted, "no project in this library has that id or name", ids())),
            _ => Err(unknown_project(wanted, "more than one project has that name", by_name.iter().map(|p| p.id.clone()))),
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
    // The name can come from a project name, i.e. from doc-folder text: it travels fenced in `received`, not in `message`.
    ToolError::new(ErrorCode::InvalidArguments, "/project", format!("{why}; fix: pass a project id from valid_options"))
        .with_received(crate::tools::docs_fence::fence_capped(wanted, 500))
        .with_expected("a project id from valid_options")
        .with_valid_options(options)
}
