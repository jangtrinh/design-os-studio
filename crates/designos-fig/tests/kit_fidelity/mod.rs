//! The kit fidelity scoreboard: Studio measured, feature by feature, against the data Figma stored in a `.fig`.

pub mod board;
pub mod feature;
pub mod geometry;
pub mod import_cov;
pub mod instances;
pub mod keys;
pub mod layout;
pub mod raster;
pub mod raw;
pub mod report;
pub mod run;
pub mod save_loss;
pub mod scratch;
pub mod text;

#[cfg(test)]
mod fixtures;
#[cfg(test)]
mod synthetic;
