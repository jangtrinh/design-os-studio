//! Document data (besides nodes) that `apply_spec` can write: variables, collections, styles,
//! style metadata and library bookkeeping. Used to roll a failed apply back and to commit a
//! successful one as undoable commands.

use crate::{Command, Document, LibraryState, NodeId, Style, StyleMeta, Variable, VariableCollection};
use std::collections::BTreeMap;

/// Copy of the non-node data a spec apply may change.
#[derive(Clone, Debug)]
pub struct SpecDataSnapshot {
    variables: BTreeMap<String, Variable>,
    variable_collections: BTreeMap<String, VariableCollection>,
    styles: BTreeMap<String, Style>,
    style_meta: BTreeMap<String, StyleMeta>,
    library: LibraryState,
    pub(crate) library_page: Option<NodeId>,
}

impl SpecDataSnapshot {
    pub fn of(doc: &Document) -> Self {
        Self {
            variables: doc.variables.clone(),
            variable_collections: doc.variable_collections.clone(),
            styles: doc.styles.clone(),
            style_meta: doc.style_meta.clone(),
            library: doc.library.clone(),
            library_page: doc.library_page,
        }
    }

    pub fn restore(self, doc: &mut Document) {
        doc.variables = self.variables;
        doc.variable_collections = self.variable_collections;
        doc.styles = self.styles;
        doc.style_meta = self.style_meta;
        doc.library = self.library;
        doc.library_page = self.library_page;
    }

    pub(crate) fn apply_cmd_data(&mut self, cmd: &Command) {
        match cmd {
            Command::SetVariables { variables, collections } => {
                self.variables = variables.clone();
                self.variable_collections = collections.clone();
            }
            Command::SetStyles { styles, style_meta } => {
                self.styles = styles.clone();
                self.style_meta = style_meta.clone();
            }
            Command::SetLibraryState { state, library_page } => {
                self.library = state.clone();
                self.library_page = *library_page;
            }
            Command::Batch(cmds) => {
                for c in cmds {
                    self.apply_cmd_data(c);
                }
            }
            _ => {}
        }
    }

    pub fn commands_to(&self, applied: &Self) -> Vec<Command> {
        let mut cmds = Vec::new();
        if self.variables != applied.variables || self.variable_collections != applied.variable_collections {
            cmds.push(Command::SetVariables { variables: applied.variables.clone(), collections: applied.variable_collections.clone() });
        }
        if self.styles != applied.styles || self.style_meta != applied.style_meta {
            cmds.push(Command::SetStyles { styles: applied.styles.clone(), style_meta: applied.style_meta.clone() });
        }
        if self.library != applied.library || self.library_page != applied.library_page {
            cmds.push(Command::SetLibraryState { state: applied.library.clone(), library_page: applied.library_page });
        }
        cmds
    }
}

impl Document {
    /// Commands that bring `self`'s variables, styles and library state to those of `applied`
    /// (a copy of `self` that `apply_spec` ran on), one per group that differs; empty when the
    /// apply wrote no such data. Run them before inserting the nodes so one undo step reverts both.
    pub fn spec_data_commands(&self, applied: &Document) -> Vec<Command> {
        let mut cmds = Vec::new();
        if self.variables != applied.variables || self.variable_collections != applied.variable_collections {
            cmds.push(Command::SetVariables { variables: applied.variables.clone(), collections: applied.variable_collections.clone() });
        }
        if self.styles != applied.styles || self.style_meta != applied.style_meta {
            cmds.push(Command::SetStyles { styles: applied.styles.clone(), style_meta: applied.style_meta.clone() });
        }
        if self.library != applied.library || self.library_page != applied.library_page {
            cmds.push(Command::SetLibraryState { state: applied.library.clone(), library_page: applied.library_page });
        }
        cmds
    }
}
