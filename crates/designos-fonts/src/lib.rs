//! Open-source font support for designos.
//!
//! * [`bundled`] returns fonts embedded in the binary (Inter and Roboto Mono), always available offline.
//! * [`requests_for`] lists every family/weight/italic a document uses.
//! * [`substitute`] maps well-known proprietary families to open look-alikes.
//! * [`FontCache`] resolves a [`FontRequest`] from the on-disk cache, fetching
//!   from the Fontsource API (cargo feature `net`, on by default) on a miss.
//! * [`ensure_fonts`] does all of that for a whole document; run it on a
//!   background thread after opening a file, then register the result with
//!   `designos_render::register_font`.
//!
//! Nothing here is called from the renderer: rendering stays offline and
//! deterministic, and only fonts you explicitly register affect it.

mod assets;
mod cache;
mod ensure;
mod error;
pub mod fontsource;
mod request;
mod scripts;
mod subst;

pub use assets::{bundled, bundled_for, FontAsset};
pub use cache::{write_atomic, Fetch, FontCache, Sidecar};
pub use ensure::{ensure_font_requests_with, ensure_fonts, ensure_fonts_with, FontReport, LoadedFont};
pub use error::FontError;
pub use request::{requests_for, FontRequest};
pub use scripts::{script_of, script_requests_for, scripts_in, Script};
pub use subst::{substitute, SUBSTITUTIONS};
