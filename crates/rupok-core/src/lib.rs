//! The rupok document model.
//!
//! A lesson is a [Loro](https://loro.dev) CRDT document. That single choice
//! gives us undo/redo, timestamped history (replay), multi-device merge (sync)
//! and, later, live broadcast — all from the same op log.
//!
//! Layout inside the Loro doc (see `spec/rupok-format.md`):
//! ```text
//! meta   : Map   { title, format, created }
//! pages  : MovableList<Map { id, title, frame?, bg_pattern, bg_color, items: Map<id, Map> }>
//! ```

pub mod doc;
pub mod model;
pub mod scene;

pub use doc::{CoreError, Lesson, media_from_value};
pub use model::*;
pub use scene::Scene;

/// Current document schema version written into `meta.format`.
pub const DOC_FORMAT: i64 = 1;
