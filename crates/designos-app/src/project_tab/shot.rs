//! A screenshot run can start with a Project tab open: `DESIGNOS_PROJECT_TAB_SHOT` names the project and what to
//! show, as `key=value` pairs joined by `;` (`project=<id>;page=spec/cart-rules.md;menu=spec/cart-rules.md;delete=...;search=ca`).
//! It is a development aid, read only by a `--screenshot` run, a debug build, or a run with `DESIGNOS_DEV` set (the idle-CPU measurement of a release build).

pub const ENV: &str = "DESIGNOS_PROJECT_TAB_SHOT";

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ShotSpec {
    pub project: String,
    pub page: Option<String>,
    /// Open the page's context menu.
    pub menu: Option<String>,
    /// Open the delete dialog for this page.
    pub delete: Option<String>,
    pub search: Option<String>,
    /// Show Home's Trash section (with `--home`), for the evidence of a deleted page.
    pub home_trash: bool,
}

fn enabled(screenshot: bool, debug_build: bool, dev_env: bool) -> bool {
    screenshot || debug_build || dev_env
}

impl ShotSpec {
    pub fn parse(text: &str) -> Result<Self, String> {
        let mut spec = ShotSpec::default();
        for pair in text.split(';').map(str::trim).filter(|p| !p.is_empty()) {
            let (key, value) = pair.split_once('=').ok_or_else(|| format!("{ENV}: `{pair}` is not key=value"))?;
            let value = value.trim().to_string();
            match key.trim() {
                "project" => spec.project = value,
                "page" => spec.page = Some(value),
                "menu" => spec.menu = Some(value),
                "delete" => spec.delete = Some(value),
                "search" => spec.search = Some(value),
                "home" if value == "trash" => spec.home_trash = true,
                other => return Err(format!("{ENV}: unknown key `{other}` (expected project, page, menu, delete, search or home=trash)")),
            }
        }
        if spec.project.is_empty() && !spec.home_trash {
            return Err(format!("{ENV}: `project=<project id>` is required"));
        }
        Ok(spec)
    }

    /// The spec in the environment, when this run may use it: a `--screenshot` run, a debug build, or `DESIGNOS_DEV` set
    /// (the idle-CPU measurement of a release build). A release window a user starts never reads it.
    pub fn from_env(screenshot: bool) -> Option<Result<Self, String>> {
        if !enabled(screenshot, cfg!(debug_assertions), std::env::var_os("DESIGNOS_DEV").is_some()) {
            return None;
        }
        std::env::var(ENV).ok().filter(|v| !v.trim().is_empty()).map(|v| Self::parse(&v))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_a_screenshot_run_a_debug_build_or_the_dev_switch_may_use_it() {
        assert!(enabled(true, false, false) && enabled(false, true, false) && enabled(false, false, true));
        assert!(!enabled(false, false, false), "a release window that a user starts");
    }

    #[test]
    fn home_trash_needs_no_project() {
        let s = ShotSpec::parse("home=trash").unwrap();
        assert!(s.home_trash && s.project.is_empty());
        assert!(ShotSpec::parse("home=recents").is_err());
    }

    #[test]
    fn a_spec_names_the_project_and_what_to_show() {
        let s = ShotSpec::parse("project=p1; page=spec/a.md ;menu=spec/a.md;search=ca").unwrap();
        assert_eq!(
            s,
            ShotSpec {
                project: "p1".into(),
                page: Some("spec/a.md".into()),
                menu: Some("spec/a.md".into()),
                delete: None,
                search: Some("ca".into()),
                home_trash: false
            }
        );
        assert!(ShotSpec::parse("page=a.md").is_err(), "a project is required");
        assert!(ShotSpec::parse("project=p;color=red").unwrap_err().contains("unknown key"));
        assert!(ShotSpec::parse("project").unwrap_err().contains("key=value"));
    }
}
