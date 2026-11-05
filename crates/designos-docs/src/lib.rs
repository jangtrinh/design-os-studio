//! Project docs for DESIGN:OS Studio: a Markdown block model that round-trips byte for byte, the
//! `docs-manifest.json` manifest, conflict-checked atomic storage under `<library>/projects/<id>/docs/`, and a
//! disk watcher. No UI; `designos-app` and `designos-mcp` both build on it. See the README for the Markdown
//! library choice and the layout.

mod error;
mod escape;
mod front_matter;
mod group;
mod hash;
mod inline;
mod manifest;
mod manifest_parse;
mod markdown;
mod markdown_structures;
mod model;
mod outline;
mod page_edit;
mod paths;
mod render;
mod store;
mod store_files;
mod watch;
mod watch_scan;

pub use error::{DocsError, ManifestError, Result};
pub use escape::{escape_plain, paragraphs_from_plain, plain_inlines, TextContext};
pub use front_matter::{FrontMatter, FrontMatterProblem};
pub use group::Group;
pub use hash::ContentHash;
pub use manifest::{GroupEntry, Manifest, Pin, MANIFEST_FILE, SCHEMA_VERSION};
pub use model::{Align, Block, BlockKind, ChecklistItem, Inline, ListItem, Page};
pub use outline::{plain_text, Heading};
pub use store::{DocsStore, Expect, Versioned};
pub use watch::{DocChange, DocsWatcher, WatchMode};
