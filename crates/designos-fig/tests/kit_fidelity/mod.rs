//! The kit fidelity scoreboard: Studio measured, feature by feature, against the data Figma stored in a `.fig`.

pub mod board;
pub mod feature;
pub mod import_cov;
pub mod keys;
pub mod layout;
pub mod raw;
pub mod report;
pub mod run;
pub mod save_loss;

#[cfg(test)]
mod fixtures;
#[cfg(test)]
mod synthetic;
