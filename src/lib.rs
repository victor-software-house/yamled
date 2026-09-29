//! Format-preserving YAML edits.
//!
//! `yamled` changes the values you name in a YAML file and leaves every other
//! byte where it was: comments, blank lines, quoting, block and flow style,
//! and indentation. It indexes the source through `granit-parser` spans and
//! writes values through `serde-saphyr`.
//!
//! ```
//! use indoc::indoc;
//! use yamled::{Document, Path};
//!
//! let mut ledger = Document::parse(indoc! {"
//!     ## A work queue.
//!     active: A-1
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
//!     ## A work queue.
//!     active: A-2
//!     queue:
//!       - id: A-1
//!         title: 'Write the index: v2'
//! "}
//! );
//! # Ok::<(), yamled::Error>(())
//! ```
//!
//! Every edit parses its result. When the result would not parse, the edit
//! returns an [`Error`] and the document keeps its old text.

mod document;
mod error;
mod index;
mod path;
mod render;
mod text;

pub use crate::document::{Document, Fragment, Node, Position, Spacing};
pub use crate::error::Error;
pub use crate::index::{Location, Style};
pub use crate::path::{Path, Segment};
pub use crate::render::TextStyle;
