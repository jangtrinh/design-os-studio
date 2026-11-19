//! The kit fidelity scoreboard: Studio measured, feature by feature, against the data Figma stored in a `.fig`.

pub mod board;
pub mod census;
pub mod feature;
pub mod geometry;
pub mod import_cov;
pub mod instance_hydration;
pub mod instances;
pub mod keys;
pub mod layout;
pub mod raster;
pub mod raw;
pub mod report;
pub mod run;
pub mod save_class;
pub mod save_loss;
pub mod scratch;
pub mod stored_use;
pub mod text;
pub mod text_fonts;
pub mod text_glyphs;

#[cfg(test)]
mod fixtures;
#[cfg(test)]
#[path = "../common/missing_font_fig.rs"]
mod missing_font_fig;
#[cfg(test)]
mod synthetic;
#[cfg(test)]
mod synthetic_stored_data;
