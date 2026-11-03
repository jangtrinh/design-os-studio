//! A screenshot run can start with a Project tab open: `DESIGNOS_PROJECT_TAB_SHOT` names the project and what to
//! show, as `key=value` pairs joined by `;` (`project=<id>;page=spec/cart-rules.md;menu=spec/cart-rules.md;delete=...;search=ca`).
//! It is a development aid: a `--screenshot` run captures that state, a normal run (idle-CPU measurement) starts on it.

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
                other => return Err(format!("{ENV}: unknown key `{other}` (expected project, page, menu, delete or search)")),
            }
        }
        if spec.project.is_empty() {
            return Err(format!("{ENV}: `project=<project id>` is required"));
        }
        Ok(spec)
    }

    pub fn from_env() -> Option<Result<Self, String>> {
        std::env::var(ENV).ok().filter(|v| !v.trim().is_empty()).map(|v| Self::parse(&v))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_spec_names_the_project_and_what_to_show() {
        let s = ShotSpec::parse("project=p1; page=spec/a.md ;menu=spec/a.md;search=ca").unwrap();
        assert_eq!(
            s,
            ShotSpec { project: "p1".into(), page: Some("spec/a.md".into()), menu: Some("spec/a.md".into()), delete: None, search: Some("ca".into()) }
        );
        assert!(ShotSpec::parse("page=a.md").is_err(), "a project is required");
        assert!(ShotSpec::parse("project=p;color=red").unwrap_err().contains("unknown key"));
        assert!(ShotSpec::parse("project").unwrap_err().contains("key=value"));
    }
}
