//! The context menu of a cell, row or column selection: nine verb + object entries. An entry the
//! selection cannot use is disabled and says why, so no entry is live without effect.

use super::{plan, TableAction};
use crate::app::App;
use crate::widgets::menu::MenuItem;

/// The nine entries, each enabled by running its command as a dry run on the current selection.
pub fn entries(app: &App) -> Vec<(MenuItem, TableAction)> {
    TableAction::ALL
        .iter()
        .map(|&a| {
            let reason = plan(app, a).err();
            let mut item = MenuItem::new(a.label());
            if let Some(why) = reason {
                item = item.disabled(true).tooltip(why.message());
            }
            if matches!(a, TableAction::InsertColRight | TableAction::DeleteCol | TableAction::UnmergeCells) {
                item = item.sep();
            }
            (item, a)
        })
        .collect()
}
