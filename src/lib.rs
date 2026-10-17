//! Format-preserving YAML edits: change what you name, keep every other byte.
//!
//! `yamled` replaces, removes, and inserts the values you name in a YAML file.
//! Comments, blank lines, quoting, block and flow style, and indentation stay
//! as they were. It indexes the source through [`granit-parser`] spans and
//! writes values through [`serde-saphyr`].
//!
//! # Quick start
//!
//! ```
//! use indoc::indoc;
//! use yamled::{Document, Path};
//!
//! let mut ledger = Document::parse(indoc! {"
//!     active: A-1 # the row in progress
//!     queue:
//!       - id: A-1
//!         title: Write the index
//! "})?;
//!
//! ledger.replace(&Path::root().key("active"), "A-2")?;
//! ledger.replace(
//!     &Path::root().key("queue").index(0).key("title"),
//!     "Write the index: v2",
//! )?;
//!
//! assert_eq!(
//!     ledger.as_str(),
//!     indoc! {"
//!     active: A-2 # the row in progress
//!     queue:
//!       - id: A-1
//!         title: 'Write the index: v2'
//! "}
//! );
//! # Ok::<(), yamled::Error>(())
//! ```
//!
//! # Failed edits
//!
//! Every edit parses its result. When the result would not parse, the edit
//! returns an [`Error`] and the document keeps its old text.
//!
//! # Cargo features
//!
//! | Feature | Adds | Depends on |
//! |:--|:--|:--|
//! | (none) | The location index, [`remove`], [`take`], [`put`], [`reorder`], [`reindent`] | `granit-parser` |
//! | `serde` (default) | `replace`, `insert`, `push`, `insert_item`, their `_text` forms, `TextStyle`, `Quotes` | `serde`, `serde-saphyr` |
//!
//! A caller that needs only the line and column of a path, such as a schema
//! error reporter, depends on `yamled` with `default-features = false`.
//!
//! [`granit-parser`]: https://docs.rs/granit-parser
//! [`serde-saphyr`]: https://docs.rs/serde-saphyr
//! [`remove`]: Document::remove
//! [`take`]: Document::take
//! [`put`]: Document::put
//! [`reorder`]: Document::reorder
//! [`reindent`]: Document::reindent

#![cfg_attr(docsrs, feature(doc_cfg))]

mod document;
mod error;
mod index;
mod path;
#[cfg(feature = "serde")]
mod render;
mod text;

#[cfg(feature = "serde")]
pub use crate::document::Position;
pub use crate::document::{Document, Fragment, Node, Spacing};
pub use crate::error::Error;
pub use crate::index::{Location, Style};
pub use crate::path::{Path, Segment};
#[cfg(feature = "serde")]
pub use crate::render::{Quotes, TextStyle};

/// The writer's style decision, for the benchmarks in `benches/`. Not part
/// of the API: it may change in any release.
#[doc(hidden)]
#[cfg(feature = "serde")]
pub mod bench {
    pub use crate::render::line;
}
