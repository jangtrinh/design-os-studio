//! The five doc groups, in the fixed order the tree shows them (P4).

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Group {
    Overview,
    Spec,
    Screens,
    States,
    Discovery,
}

impl Group {
    pub const ALL: [Group; 5] = [Group::Overview, Group::Spec, Group::Screens, Group::States, Group::Discovery];

    /// The id used in the manifest and in page front matter.
    pub fn as_str(self) -> &'static str {
        match self {
            Group::Overview => "overview",
            Group::Spec => "spec",
            Group::Screens => "screens",
            Group::States => "states",
            Group::Discovery => "discovery",
        }
    }

    /// Position in `Group::ALL`.
    pub fn index(self) -> usize {
        self as usize
    }

    pub fn parse(s: &str) -> Option<Group> {
        Group::ALL.into_iter().find(|g| g.as_str() == s)
    }

    /// `overview | spec | screens | states | discovery`, for "use one of" fixes.
    pub fn list() -> &'static str {
        "overview | spec | screens | states | discovery"
    }
}
