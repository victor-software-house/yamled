//! Format-preserving YAML edits.
//!
//! `yamled` changes the values you name in a YAML file and leaves every other
//! byte where it was: comments, blank lines, quoting, block and flow style,
//! and indentation. It indexes the source through `granit-parser` spans and
//! reads and writes values through `serde-saphyr`.
//!
//! This release reserves the name. The location index and the edit
//! operations land in the next version.
