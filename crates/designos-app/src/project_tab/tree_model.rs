//! The doc tree as rows: five groups in fixed order, their pages and pinned frames, with search and collapse applied.

use crate::icons::Icon;
use crate::tabs::project_tab::ops::{PageMeta, TreeSnapshot};
use designos_docs::{Group, Pin};
use std::collections::HashSet;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum RowKey {
    Group(Group),
    Page(String),
    /// A pinned frame under Screens or States.
    Pin(String),
    /// A screen that has states, shown in the States group.
    StateParent(String),
    Placeholder(Group),
}

#[derive(Clone, Debug, PartialEq)]
pub enum RowKind {
    Group(Group),
    Page { rel: String, group: Group },
    Pin,
    Placeholder(Group),
}

#[derive(Clone, Debug, PartialEq)]
pub struct Row {
    pub key: RowKey,
    pub kind: RowKind,
    pub depth: u8,
    pub label: String,
    /// The group's count pill.
    pub count: Option<usize>,
    /// `Some(expanded)` when the row has a chevron.
    pub chevron: Option<bool>,
    pub icon: Icon,
}

pub fn group_label(group: Group) -> &'static str {
    match group {
        Group::Overview => "Overview",
        Group::Spec => "Spec",
        Group::Screens => "Screens",
        Group::States => "States",
        Group::Discovery => "Discovery",
    }
}

pub fn placeholder_text(group: Group) -> &'static str {
    match group {
        Group::Overview => "No overview pages",
        Group::Spec => "No spec pages",
        Group::Screens => "No pinned screens",
        Group::States => "No pinned states",
        Group::Discovery => "No discovery pages",
    }
}

/// Whether the group can take a new page from the tree.
pub fn group_takes_pages(group: Group) -> bool {
    matches!(group, Group::Overview | Group::Spec | Group::Discovery)
}

fn matches_query(meta: Option<&PageMeta>, label: &str, query: &str) -> bool {
    if query.is_empty() {
        return true;
    }
    let q = query.to_lowercase();
    label.to_lowercase().contains(&q) || meta.is_some_and(|m| m.headings.iter().any(|h| h.to_lowercase().contains(&q)))
}

fn pin_matches(pin: &Pin, query: &str) -> bool {
    query.is_empty() || pin.name.to_lowercase().contains(&query.to_lowercase())
}

/// Rows for `snapshot`. A non-empty `query` filters pages by title and heading text and pins by name, hides groups
/// with nothing left, and opens every group that has a match.
pub fn build_rows(snapshot: &TreeSnapshot, collapsed: &HashSet<RowKey>, query: &str) -> Vec<Row> {
    let query = query.trim();
    let searching = !query.is_empty();
    let mut rows = Vec::new();
    for group in Group::ALL {
        let pages: Vec<String> =
            snapshot.group_pages(group).into_iter().filter(|rel| matches_query(snapshot.pages.get(rel), &snapshot.title_of(rel), query)).collect();
        let pins: Vec<&Pin> = snapshot.manifest.pins.iter().filter(|p| p.group == group && pin_matches(p, query)).collect();
        // A screen with states shows in States as a parent even when only a state matches.
        let state_parents: Vec<&Pin> = if group == Group::States {
            snapshot
                .manifest
                .pins
                .iter()
                .filter(|p| p.group == Group::Screens)
                .filter(|screen| {
                    snapshot
                        .manifest
                        .pins
                        .iter()
                        .any(|s| s.group == Group::States && s.parent_screen.as_deref() == Some(screen.id.as_str()) && pin_matches(s, query))
                })
                .collect()
        } else {
            Vec::new()
        };
        let count = pages.len() + pins.len();
        if searching && count == 0 {
            continue;
        }
        let collapsible = group != Group::Overview;
        let expanded = !collapsible || searching || !collapsed.contains(&RowKey::Group(group));
        rows.push(Row {
            key: RowKey::Group(group),
            kind: RowKind::Group(group),
            depth: 0,
            label: group_label(group).to_string(),
            // An empty group has no pill (its placeholder row says so).
            count: (count > 0).then_some(count),
            chevron: collapsible.then_some(expanded),
            icon: Icon::ChevronDown,
        });
        if !expanded {
            continue;
        }
        for rel in &pages {
            rows.push(Row {
                key: RowKey::Page(rel.clone()),
                kind: RowKind::Page { rel: rel.clone(), group },
                depth: 0,
                label: snapshot.title_of(rel),
                count: None,
                chevron: None,
                icon: Icon::Page,
            });
        }
        match group {
            Group::Screens => {
                for pin in &pins {
                    rows.push(Row {
                        key: RowKey::Pin(pin.id.clone()),
                        kind: RowKind::Pin,
                        depth: 0,
                        label: pin.name.clone(),
                        count: None,
                        chevron: None,
                        icon: Icon::Frame,
                    });
                }
            }
            Group::States => {
                for screen in &state_parents {
                    let open = searching || !collapsed.contains(&RowKey::StateParent(screen.id.clone()));
                    rows.push(Row {
                        key: RowKey::StateParent(screen.id.clone()),
                        kind: RowKind::Pin,
                        depth: 0,
                        label: screen.name.clone(),
                        count: None,
                        chevron: Some(open),
                        icon: Icon::Frame,
                    });
                    if open {
                        for state in pins.iter().filter(|s| s.parent_screen.as_deref() == Some(screen.id.as_str())) {
                            rows.push(Row {
                                key: RowKey::Pin(state.id.clone()),
                                kind: RowKind::Pin,
                                depth: 1,
                                label: state.name.clone(),
                                count: None,
                                chevron: None,
                                icon: Icon::Frame,
                            });
                        }
                    }
                }
            }
            _ => {}
        }
        if count == 0 {
            rows.push(Row {
                key: RowKey::Placeholder(group),
                kind: RowKind::Placeholder(group),
                depth: 0,
                label: placeholder_text(group).to_string(),
                count: None,
                chevron: None,
                icon: Icon::Page,
            });
        }
    }
    rows
}

/// The page paths of `group` in the order the tree shows them (the order a drag or Move up / Move down changes).
pub fn page_order(rows: &[Row], group: Group) -> Vec<String> {
    rows.iter()
        .filter_map(|r| match &r.kind {
            RowKind::Page { rel, group: g } if *g == group => Some(rel.clone()),
            _ => None,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tabs::project_tab::ops::{PageMeta, TreeSnapshot};
    use designos_docs::{Manifest, Pin};

    fn meta(rel: &str, title: &str, headings: &[&str]) -> PageMeta {
        PageMeta {
            rel: rel.into(),
            title: title.into(),
            description: None,
            headings: headings.iter().map(|h| h.to_string()).collect(),
            problems: vec![],
            error: None,
            front_group: None,
        }
    }

    fn sample() -> TreeSnapshot {
        let mut manifest = Manifest::new();
        manifest.group_mut(Group::Overview).pages = vec!["overview.md".into()];
        manifest.group_mut(Group::Spec).pages = vec!["spec/payment-methods.md".into(), "spec/cart-rules.md".into()];
        manifest.group_mut(Group::Discovery).pages = vec!["discovery/research.md".into()];
        let pin = |id: &str, name: &str, group, parent: Option<&str>| Pin {
            id: id.into(),
            file_id: "f1".into(),
            node_id: format!("n-{id}"),
            name: name.into(),
            group,
            parent_screen: parent.map(str::to_string),
        };
        manifest.pins = vec![
            pin("cart", "Cart", Group::Screens, None),
            pin("pay", "Payment", Group::Screens, None),
            pin("declined", "Card declined", Group::States, Some("pay")),
        ];
        let mut s = TreeSnapshot::empty();
        s.manifest = manifest;
        for (rel, title, headings) in [
            ("overview.md", "Checkout", &["Goals"][..]),
            ("spec/payment-methods.md", "Payment methods", &["Accepted methods", "Fees and limits"][..]),
            ("spec/cart-rules.md", "Cart rules", &["Limits"][..]),
            ("discovery/research.md", "Checkout research", &[][..]),
        ] {
            s.pages.insert(rel.into(), meta(rel, title, headings));
        }
        s
    }

    #[test]
    fn the_five_groups_come_in_fixed_order_with_their_counts() {
        let rows = build_rows(&sample(), &HashSet::new(), "");
        let groups: Vec<(String, usize)> = rows.iter().filter(|r| matches!(r.kind, RowKind::Group(_))).map(|r| (r.label.clone(), r.count.unwrap())).collect();
        assert_eq!(groups, vec![("Overview".into(), 1), ("Spec".into(), 2), ("Screens".into(), 2), ("States".into(), 1), ("Discovery".into(), 1)]);
        assert_eq!(rows[0].chevron, None, "Overview is never collapsed");
        assert_eq!(rows[1].label, "Checkout");
    }

    #[test]
    fn states_show_under_the_screen_they_belong_to() {
        let rows = build_rows(&sample(), &HashSet::new(), "");
        let i = rows.iter().position(|r| r.key == RowKey::StateParent("pay".into())).expect("parent row");
        assert_eq!((rows[i].label.as_str(), rows[i].depth, rows[i].chevron), ("Payment", 0, Some(true)));
        assert_eq!((rows[i + 1].label.as_str(), rows[i + 1].depth), ("Card declined", 1));
        assert_eq!(rows[i + 1].icon, Icon::Frame, "a state is a frame; the diamond means a component instance");
        let mut closed = HashSet::new();
        closed.insert(RowKey::StateParent("pay".into()));
        assert!(!build_rows(&sample(), &closed, "").iter().any(|r| r.label == "Card declined"));
    }

    #[test]
    fn an_empty_group_keeps_a_placeholder_row_and_a_collapsed_group_hides_its_rows() {
        let mut s = sample();
        s.manifest.group_mut(Group::Spec).pages.clear();
        s.manifest.group_mut(Group::Discovery).pages.clear();
        // Pages the manifest does not list are adopted into a group, so an empty group has no files either.
        s.pages.retain(|rel, _| rel == "overview.md");
        let rows = build_rows(&s, &HashSet::new(), "");
        assert!(rows.iter().any(|r| r.label == "No spec pages" && matches!(r.kind, RowKind::Placeholder(Group::Spec))));
        assert!(rows.iter().any(|r| r.label == "No discovery pages"));
        assert_eq!(rows.iter().find(|r| r.label == "Spec").unwrap().count, None, "an empty group shows no count pill");
        let mut closed = HashSet::new();
        closed.insert(RowKey::Group(Group::Screens));
        let collapsed = build_rows(&sample(), &closed, "");
        assert!(!collapsed.iter().any(|r| r.label == "Cart"), "collapsed Screens hides its pins");
        assert_eq!(collapsed.iter().find(|r| r.label == "Screens").unwrap().chevron, Some(false));
    }

    #[test]
    fn search_matches_titles_and_heading_text_and_hides_groups_without_a_match() {
        let rows = build_rows(&sample(), &HashSet::new(), "limits");
        let labels: Vec<&str> = rows.iter().map(|r| r.label.as_str()).collect();
        assert_eq!(labels, vec!["Spec", "Payment methods", "Cart rules"], "a heading match shows the page: {labels:?}");
        let rows = build_rows(&sample(), &HashSet::new(), "CA");
        let labels: Vec<&str> = rows.iter().map(|r| r.label.as_str()).collect();
        assert_eq!(labels, vec!["Spec", "Cart rules", "Screens", "Cart", "States", "Payment", "Card declined"], "{labels:?}");
        assert!(build_rows(&sample(), &HashSet::new(), "zzz").is_empty());
    }

    #[test]
    fn page_order_lists_the_pages_of_one_group_as_shown() {
        let rows = build_rows(&sample(), &HashSet::new(), "");
        assert_eq!(page_order(&rows, Group::Spec), vec!["spec/payment-methods.md", "spec/cart-rules.md"]);
    }
}
