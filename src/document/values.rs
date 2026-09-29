//! Edits that write a value: through `serde-saphyr` for a serializable value,
//! or through yamled's scalar rule for a string.

use serde::Serialize;

use super::{Document, Fragment};
use crate::index::Style;
use crate::render::{self, Rendered, TextStyle};
use crate::text::{LF, NEWLINE, column, line_end, starts_line};
use crate::{Error, Path};

/// Where [`Document::insert`] puts a new key.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Position<'a> {
    /// After the mapping's last entry.
    End,
    /// Directly before this key, above the comments it owns.
    Before(&'a str),
    /// Directly after this key's value.
    After(&'a str),
}

impl Document {
    /// Write a value in place of the node at a path. A scalar keeps its style
    /// when that style can hold the new value, and a flow sequence stays in
    /// flow style, with its own separator and padding, when every new item
    /// fits on one line.
    ///
    /// # Errors
    ///
    /// [`Error::NoNode`], [`Error::Serialize`], [`Error::Unsupported`] for a
    /// block value inside a flow collection, and [`Error::Invalid`].
    pub fn replace<T: Serialize + ?Sized>(&mut self, path: &Path, value: &T) -> Result<(), Error> {
        let id = self.id(path)?;
        let node = &self.index.nodes[id];
        if node.style == Style::FlowSequence
            && let Some(items) = render::flow_items(value)?
        {
            let range = node.value.clone();
            let text = self.flow_text(id, &items);
            return self.commit_splice(range.start, range.end, &text);
        }
        let rendered = render::value(value, Some(node.style), self.compact)?;
        self.splice_value(path, id, &rendered)
    }

    /// Write a string in place of the node at a path, in a chosen style.
    ///
    /// # Errors
    ///
    /// As [`Document::replace`].
    pub fn replace_text(&mut self, path: &Path, text: &str, style: TextStyle) -> Result<(), Error> {
        let id = self.id(path)?;
        let rendered = render::text(text, style, Some(self.index.nodes[id].style))?;
        self.splice_value(path, id, &rendered)
    }

    /// Add a key and value to a block mapping.
    ///
    /// # Errors
    ///
    /// [`Error::NoNode`], [`Error::WrongKind`] when the path is not a block
    /// mapping, [`Error::KeyExists`], [`Error::Serialize`], and
    /// [`Error::Invalid`].
    pub fn insert<T: Serialize + ?Sized>(
        &mut self,
        path: &Path,
        key: &str,
        value: &T,
        position: Position<'_>,
    ) -> Result<(), Error> {
        let rendered = render::value(value, None, self.compact)?;
        self.insert_rendered(path, key, &rendered, position)
    }

    /// Add a key and a string to a block mapping, in a chosen style.
    ///
    /// # Errors
    ///
    /// As [`Document::insert`].
    pub fn insert_text(
        &mut self,
        path: &Path,
        key: &str,
        text: &str,
        style: TextStyle,
        position: Position<'_>,
    ) -> Result<(), Error> {
        let rendered = render::text(text, style, None)?;
        self.insert_rendered(path, key, &rendered, position)
    }

    /// Insert a value into a sequence at an index, placed and spaced as
    /// [`Document::put`] places a fragment. An index equal to the length
    /// appends.
    ///
    /// # Errors
    ///
    /// As [`Document::put`], and [`Error::Serialize`].
    pub fn insert_item<T: Serialize + ?Sized>(
        &mut self,
        path: &Path,
        index: usize,
        value: &T,
    ) -> Result<(), Error> {
        let rendered = render::value(value, None, self.compact)?;
        self.put_rendered(path, index, &rendered)
    }

    /// Insert a string into a sequence at an index, in a chosen style.
    ///
    /// # Errors
    ///
    /// As [`Document::insert_item`].
    pub fn insert_item_text(
        &mut self,
        path: &Path,
        index: usize,
        text: &str,
        style: TextStyle,
    ) -> Result<(), Error> {
        let rendered = render::text(text, style, None)?;
        self.put_rendered(path, index, &rendered)
    }

    /// Append a value to a sequence. An empty `[]` or an empty value becomes
    /// a block sequence under its key.
    ///
    /// # Errors
    ///
    /// As [`Document::put`], and [`Error::Serialize`].
    pub fn push<T: Serialize + ?Sized>(&mut self, path: &Path, value: &T) -> Result<(), Error> {
        let rendered = render::value(value, None, self.compact)?;
        self.put_rendered(path, self.length(path), &rendered)
    }

    /// Append a string to a sequence, in a chosen style.
    ///
    /// # Errors
    ///
    /// As [`Document::push`].
    pub fn push_text(&mut self, path: &Path, text: &str, style: TextStyle) -> Result<(), Error> {
        let rendered = render::text(text, style, None)?;
        self.put_rendered(path, self.length(path), &rendered)
    }

    fn splice_value(&mut self, path: &Path, id: usize, rendered: &Rendered) -> Result<(), Error> {
        let node = &self.index.nodes[id];
        let value = node.value.clone();
        let parent_style = node.parent.map(|parent| self.index.nodes[parent].style);
        match parent_style {
            Some(Style::BlockMapping) => {
                let (colon, key_column) = self.colon(path, id)?;
                let body_column = if rendered.has_indicator() {
                    key_column + 2
                } else {
                    self.block_body_column(id).unwrap_or(key_column + 2)
                };
                let text = rendered.after_key_at(key_column, body_column, self.compact);
                self.commit_splice(colon, value.end, &text)
            }
            Some(Style::BlockSequence) => {
                let text = rendered.after_dash(column(&self.source, self.dash(id)));
                self.commit_splice(value.start, value.end, &text)
            }
            Some(_) if let Some(head) = rendered.inline_head() => {
                let head = render::flow_safe(head);
                self.commit_splice(value.start, value.end, &head)
            }
            Some(_) => Err(Error::Unsupported {
                path: path.clone(),
                what: "write a block value inside a flow collection",
            }),
            None => self.commit_splice(
                value.start,
                value.end,
                &rendered.at_root(column(&self.source, value.start)),
            ),
        }
    }

    /// The column a block scalar's text starts at; the parser's span for a
    /// block scalar begins at its text, not at the `>` or `|` header. `None`
    /// for any other node.
    fn block_body_column(&self, id: usize) -> Option<usize> {
        let node = &self.index.nodes[id];
        matches!(node.style, Style::Folded | Style::Literal)
            .then(|| column(&self.source, node.value.start))
    }

    fn insert_rendered(
        &mut self,
        path: &Path,
        key: &str,
        rendered: &Rendered,
        position: Position<'_>,
    ) -> Result<(), Error> {
        let id = self.id(path)?;
        let node = &self.index.nodes[id];
        if node.style != Style::BlockMapping {
            return Err(Error::WrongKind {
                path: path.clone(),
                expected: "a block mapping",
            });
        }
        if self.index.find(&path.clone().key(key)).is_some() {
            return Err(Error::KeyExists {
                path: path.clone().key(key),
            });
        }
        let key_text = render::text(key, TextStyle::Auto, None)?;
        let key_text = key_text.inline_head().ok_or_else(|| Error::Serialize {
            message: format!("the key {key:?} needs more than one line"),
        })?;
        let key_column = column(&self.source, self.index.entry_start(node.children[0]));
        let entry = format!(
            "{key_text}:{}",
            rendered.after_key(key_column, self.compact)
        );
        let pad = " ".repeat(key_column);
        let anchor = |name: &str| {
            self.index
                .find(&path.clone().key(name))
                .ok_or_else(|| Error::NoNode {
                    path: path.clone().key(name),
                })
        };
        let after = match position {
            Position::End => *node.children.last().ok_or_else(|| Error::WrongKind {
                path: path.clone(),
                expected: "a mapping with at least one entry",
            })?,
            Position::After(name) => anchor(name)?,
            Position::Before(name) => {
                let before = anchor(name)?;
                let start = self.index.entry_start(before);
                return if starts_line(&self.source, start) {
                    let at = self.index.owned_start(&self.source, before);
                    self.commit_splice(at, at, &format!("{pad}{entry}{LF}"))
                } else {
                    self.commit_splice(start, start, &format!("{entry}{LF}{pad}"))
                };
            }
        };
        let at = line_end(&self.source, self.index.nodes[after].value.end);
        let lead = if self.source[..at].ends_with(LF) {
            ""
        } else {
            NEWLINE
        };
        self.commit_splice(at, at, &format!("{lead}{pad}{entry}{LF}"))
    }

    fn put_rendered(
        &mut self,
        path: &Path,
        index: usize,
        rendered: &Rendered,
    ) -> Result<(), Error> {
        let fragment = Fragment {
            text: format!("- {}{LF}", rendered.after_dash(0)),
        };
        self.put(path, index, &fragment)
    }

    /// How many items the node at a path holds, or 0 when there is none; a
    /// missing path fails in [`Document::put`].
    fn length(&self, path: &Path) -> usize {
        self.node(path).map_or(0, |node| node.len())
    }

    /// Flow items laid out as this flow sequence already writes them: its
    /// separator, and a space inside the brackets when it has one. A list
    /// that spans lines, or shows no separator, gets `[a, b]`.
    fn flow_text(&self, id: usize, items: &[String]) -> String {
        if items.is_empty() {
            return "[]".to_owned();
        }
        let node = &self.index.nodes[id];
        let written = &self.source[node.value.clone()];
        let one_line = !written.contains(LF);
        let separator = match node.children[..] {
            [first, _, ..] if one_line => {
                let end = self.index.nodes[first].value.end;
                &self.source[end..self.flow_item_start(id, 1)]
            }
            _ => ", ",
        };
        let padding = if one_line && written.starts_with("[ ") {
            " "
        } else {
            ""
        };
        format!("[{padding}{}{padding}]", items.join(separator))
    }
}
