use std::fmt;
use std::ops::Range;

use serde::Serialize;

use crate::index::{Index, Location, Style};
use crate::render::{self, Rendered, TextStyle};
use crate::text::{
    LF, NEWLINE, column, dedent, has_blank_line, indent, join, line_end, line_start, starts_line,
    terminated,
};
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

/// A sequence item taken out of a document by [`Document::take`], with the
/// comments it owns, ready for [`Document::put`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Fragment {
    text: String,
}

impl Fragment {
    /// The item as text, its dash at column 0.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.text
    }
}

/// A splice that removes a sequence item, and the item's own text.
struct Cut {
    from: usize,
    to: usize,
    with: String,
    text: String,
}

/// A read-only view of one node.
#[derive(Clone, Copy, Debug)]
pub struct Node<'a> {
    document: &'a Document,
    id: usize,
}

impl Node<'_> {
    /// Where the value is written.
    #[must_use]
    pub fn value(&self) -> Location {
        let node = &self.document.index.nodes[self.id];
        Location::of(&self.document.source, node.value.clone())
    }

    /// Where the key is written, for a mapping value.
    #[must_use]
    pub fn key(&self) -> Option<Location> {
        let node = &self.document.index.nodes[self.id];
        let key = node.key.as_ref()?;
        Some(Location::of(&self.document.source, key.range.clone()))
    }

    /// The lines this node owns: its comments above, its entry, and its value.
    #[must_use]
    pub fn owned(&self) -> Location {
        let source = &self.document.source;
        let index = &self.document.index;
        let start = index.owned_start(source, self.id);
        let end = line_end(source, index.nodes[self.id].value.end);
        Location::of(source, start..end)
    }

    /// How the value is written.
    #[must_use]
    pub fn style(&self) -> Style {
        self.document.index.nodes[self.id].style
    }

    /// How many items or entries a collection holds; 0 for a scalar.
    #[must_use]
    pub fn len(&self) -> usize {
        self.document.index.nodes[self.id].children.len()
    }

    /// Whether a collection is empty, or the node is a scalar.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// The value as written in the source.
    #[must_use]
    pub fn text(&self) -> &str {
        let node = &self.document.index.nodes[self.id];
        &self.document.source[node.value.clone()]
    }
}

/// A YAML document that edits its own text in place.
///
/// Each edit splices the source and parses it again. When the result does not
/// parse, the edit returns an error and the document keeps its old text.
#[derive(Clone, Debug)]
pub struct Document {
    source: String,
    index: Index,
    compact: bool,
}

impl Document {
    /// Index one YAML document.
    ///
    /// # Errors
    ///
    /// [`Error::Parse`] for text that is not YAML, and [`Error::Unsupported`]
    /// for `\r\n` line breaks or a stream of several documents.
    pub fn parse(source: impl Into<String>) -> Result<Self, Error> {
        let source = source.into();
        if source.contains('\r') {
            return Err(Error::Unsupported {
                path: Path::root(),
                what: "edit text with carriage returns",
            });
        }
        let index = Index::build(&source)?;
        let compact = compact_lists(&source, &index);
        Ok(Self {
            source,
            index,
            compact,
        })
    }

    /// The current text.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.source
    }

    /// The current text, owned.
    #[must_use]
    pub fn into_string(self) -> String {
        self.source
    }

    /// The node at a path.
    #[must_use]
    pub fn node(&self, path: &Path) -> Option<Node<'_>> {
        self.index.find(path).map(|id| Node { document: self, id })
    }

    /// Where the value at a path is written.
    #[must_use]
    pub fn locate(&self, path: &Path) -> Option<Location> {
        self.node(path).map(|node| node.value())
    }

    /// Where the value at a path is written, or where its nearest existing
    /// ancestor is, for a path that names something missing. A schema error
    /// about a missing key lands on the mapping that lacks it.
    #[must_use]
    pub fn locate_nearest(&self, path: &Path) -> Option<Location> {
        let id = match self.index.walk(path) {
            Ok(id) => id,
            Err((id, _)) if !self.index.nodes.is_empty() => id,
            Err(_) => return None,
        };
        Some(Node { document: self, id }.value())
    }

    /// Write a value in place of the node at a path. A scalar keeps its style
    /// when that style can hold the new value.
    ///
    /// # Errors
    ///
    /// [`Error::NoNode`], [`Error::Serialize`], [`Error::Unsupported`] for a
    /// block value inside a flow collection, and [`Error::Invalid`].
    pub fn replace<T: Serialize + ?Sized>(&mut self, path: &Path, value: &T) -> Result<(), Error> {
        let id = self.id(path)?;
        let rendered = render::value(value, Some(self.index.nodes[id].style), self.compact)?;
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

    /// Remove the node at a path with its key, the comments it owns, and its
    /// line break. Neighbours keep every byte, including their comments.
    ///
    /// # Errors
    ///
    /// [`Error::NoNode`], [`Error::Unsupported`] for a node inside a flow
    /// collection or the only entry of a mapping, and [`Error::Invalid`].
    pub fn remove(&mut self, path: &Path) -> Result<(), Error> {
        let id = self.id(path)?;
        let parent = self.parent(path, id)?;
        match self.index.nodes[parent].style {
            Style::BlockSequence => {
                let cut = self.cut(path, id, parent)?;
                self.commit_splice(cut.from, cut.to, &cut.with)
            }
            Style::BlockMapping => self.remove_entry(path, id, parent),
            _ => Err(Error::Unsupported {
                path: path.clone(),
                what: "remove from a flow collection",
            }),
        }
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

    /// Append a value to a sequence. An empty `[]` or an empty value becomes
    /// a block sequence under its key.
    ///
    /// # Errors
    ///
    /// As [`Document::put`], and [`Error::Serialize`].
    pub fn push<T: Serialize + ?Sized>(&mut self, path: &Path, value: &T) -> Result<(), Error> {
        let rendered = render::value(value, None, self.compact)?;
        self.push_rendered(path, &rendered)
    }

    /// Append a string to a sequence, in a chosen style.
    ///
    /// # Errors
    ///
    /// As [`Document::push`].
    pub fn push_text(&mut self, path: &Path, text: &str, style: TextStyle) -> Result<(), Error> {
        let rendered = render::text(text, style, None)?;
        self.push_rendered(path, &rendered)
    }

    /// Take a sequence item out, with the comments it owns.
    ///
    /// # Errors
    ///
    /// [`Error::NoNode`], [`Error::WrongKind`] when the parent is not a block
    /// sequence, [`Error::Unsupported`] for the only item of a sequence with
    /// no key, and [`Error::Invalid`].
    pub fn take(&mut self, path: &Path) -> Result<Fragment, Error> {
        let id = self.id(path)?;
        let parent = self.parent(path, id)?;
        if self.index.nodes[parent].style != Style::BlockSequence {
            return Err(Error::WrongKind {
                path: path.clone(),
                expected: "an item of a block sequence",
            });
        }
        let cut = self.cut(path, id, parent)?;
        self.commit_splice(cut.from, cut.to, &cut.with)?;
        Ok(Fragment { text: cut.text })
    }

    /// Insert a fragment into a sequence at an index, re-indented to it. Items
    /// separated by blank lines stay separated.
    ///
    /// # Errors
    ///
    /// [`Error::NoNode`] for a missing sequence or an index past its end,
    /// [`Error::WrongKind`] when the path is not a sequence or an empty value,
    /// and [`Error::Invalid`].
    pub fn put(&mut self, path: &Path, index: usize, fragment: &Fragment) -> Result<(), Error> {
        let id = self.id(path)?;
        let node = &self.index.nodes[id];
        let lines: Vec<String> = fragment.text.lines().map(str::to_owned).collect();
        match node.style {
            Style::BlockSequence => {
                let dash = column(&self.source, node.value.start);
                let block = join(&indent(&lines, dash));
                let separated = match node.children[..] {
                    [first, second, ..] => has_blank_line(
                        &self.source,
                        line_end(&self.source, self.index.nodes[first].value.end),
                        self.index.owned_start(&self.source, second),
                    ),
                    _ => false,
                };
                let gap = if separated { NEWLINE } else { "" };
                if let Some(&child) = node.children.get(index) {
                    let at = self.index.owned_start(&self.source, child);
                    let text = format!("{block}{LF}{gap}");
                    self.commit_splice(at, at, &text)
                } else if let Some(&last) = node.children.last()
                    && index == node.children.len()
                {
                    let at = line_end(&self.source, self.index.nodes[last].value.end);
                    let lead = if self.source[..at].ends_with(LF) {
                        ""
                    } else {
                        NEWLINE
                    };
                    let text = format!("{lead}{gap}{block}{LF}");
                    self.commit_splice(at, at, &text)
                } else {
                    Err(Error::NoNode {
                        path: path.clone().index(index),
                    })
                }
            }
            Style::FlowSequence | Style::Plain
                if node.children.is_empty() && self.is_empty_value(id) =>
            {
                if index != 0 {
                    return Err(Error::NoNode {
                        path: path.clone().index(index),
                    });
                }
                self.fill_empty(path, id, &lines)
            }
            _ => Err(Error::WrongKind {
                path: path.clone(),
                expected: "a block sequence or an empty value",
            }),
        }
    }

    fn id(&self, path: &Path) -> Result<usize, Error> {
        self.index
            .find(path)
            .ok_or_else(|| Error::NoNode { path: path.clone() })
    }

    fn parent(&self, path: &Path, id: usize) -> Result<usize, Error> {
        self.index.nodes[id]
            .parent
            .ok_or_else(|| Error::Unsupported {
                path: path.clone(),
                what: "remove or take the root",
            })
    }

    /// Whether a node is `[]`, `~`, `null`, or nothing at all.
    fn is_empty_value(&self, id: usize) -> bool {
        let node = &self.index.nodes[id];
        match node.style {
            Style::FlowSequence => node.children.is_empty(),
            Style::Plain => matches!(self.source[node.value.clone()].trim(), "" | "~" | "null"),
            _ => false,
        }
    }

    /// The byte after a mapping value's `:`, and the key's column.
    fn colon(&self, path: &Path, id: usize) -> Result<(usize, usize), Error> {
        let key = self.index.nodes[id]
            .key
            .as_ref()
            .ok_or_else(|| Error::Unsupported {
                path: path.clone(),
                what: "fill an empty value that has no key",
            })?;
        let after = &self.source[key.range.end..];
        let offset = after.find(':').ok_or_else(|| Error::Invalid {
            message: format!("no ':' after the key at {path}"),
        })?;
        Ok((
            key.range.end + offset + 1,
            column(&self.source, key.range.start),
        ))
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
            None => self.commit_splice(value.start, value.end, &rendered.at_root()),
        }
    }

    /// Write `lines` as the items of an empty value (`[]`, `~`, `null`, or
    /// nothing) under its key. The key line keeps its node properties and
    /// comment; an empty value on a later line is replaced with its line, and
    /// its own properties and comment move up to the key line.
    fn fill_empty(&mut self, path: &Path, id: usize, lines: &[String]) -> Result<(), Error> {
        let (colon, key_column) = self.colon(path, id)?;
        let value = self.index.nodes[id].value.clone();
        let key_line_end = line_end(&self.source, colon);
        let key_line_content =
            key_line_end - usize::from(self.source[..key_line_end].ends_with(LF));
        let (mut properties, mut comment) = self.key_line_parts(colon, Some(&value));
        let end = if value.is_empty() || value.end <= key_line_content {
            key_line_end
        } else {
            if value.start > key_line_content {
                let value_line = line_start(&self.source, value.start);
                if !self.source[key_line_end..value_line].trim().is_empty() {
                    return Err(Error::Unsupported {
                        path: path.clone(),
                        what: "fill an empty value with comment lines above it",
                    });
                }
                properties = spaced(&[&properties, self.source[value_line..value.start].trim()]);
            }
            let end = line_end(&self.source, value.end);
            comment = spaced(&[&comment, self.source[value.end..end].trim()]);
            end
        };
        let head = spaced(&[&properties, &comment]);
        let step = if self.compact { 0 } else { 2 };
        let placed: Vec<String> = std::iter::once(if head.is_empty() {
            head
        } else {
            format!(" {head}")
        })
        .chain(indent(lines, key_column + step))
        .collect();
        self.commit_splice(colon, end, &terminated(&join(&placed)))
    }

    /// What follows a key's `:` on its line, apart from the part of `value`
    /// written there: node properties such as `&anchor` or `!!tag`, and a
    /// trailing comment.
    fn key_line_parts(&self, colon: usize, value: Option<&Range<usize>>) -> (String, String) {
        let end = line_end(&self.source, colon);
        let mut rest = self.source[colon..end].trim_end_matches(LF).to_owned();
        let line_content = colon + rest.len();
        if let Some(value) = value.filter(|value| (colon..line_content).contains(&value.start)) {
            rest.replace_range(value.start - colon..value.end.min(line_content) - colon, "");
        }
        let comment_at = rest
            .char_indices()
            .find(|&(at, c)| c == '#' && (at == 0 || rest[..at].ends_with([' ', '\t'])))
            .map(|(at, _)| at);
        let (properties, comment) = rest.split_at(comment_at.unwrap_or(rest.len()));
        (properties.trim().to_owned(), comment.trim().to_owned())
    }

    /// The column a block scalar's text starts at; the parser's span for a
    /// block scalar begins at its text, not at the `>` or `|` header. `None`
    /// for any other node.
    fn block_body_column(&self, id: usize) -> Option<usize> {
        let node = &self.index.nodes[id];
        matches!(node.style, Style::Folded | Style::Literal)
            .then(|| column(&self.source, node.value.start))
    }

    fn remove_entry(&mut self, path: &Path, id: usize, parent: usize) -> Result<(), Error> {
        let siblings = &self.index.nodes[parent].children;
        if siblings.len() == 1 {
            return Err(Error::Unsupported {
                path: path.clone(),
                what: "remove the only entry of a mapping",
            });
        }
        let entry = self.index.entry_start(id);
        if starts_line(&self.source, entry) {
            let from = self.index.owned_start(&self.source, id);
            let to = line_end(&self.source, self.index.nodes[id].value.end);
            return self.commit_splice(from, to, "");
        }
        let Some(&[_, next]) = siblings.windows(2).find(|pair| pair[0] == id) else {
            return Err(Error::Unsupported {
                path: path.clone(),
                what: "remove the last key of a mapping from its dash line",
            });
        };
        let next_entry = self.index.entry_start(next);
        let comments_start = self.index.owned_start(&self.source, next);
        let comments_end = line_start(&self.source, next_entry);
        let line = line_start(&self.source, entry);
        let dash_column = column(&self.source, self.dash_before(entry));
        let comments: Vec<String> = self.source[comments_start..comments_end]
            .lines()
            .map(|comment| comment.trim_start().to_owned())
            .collect();
        let mut with: String = indent(&comments, dash_column)
            .iter()
            .map(|comment| terminated(comment))
            .collect();
        with.push_str(&self.source[line..entry]);
        self.commit_splice(line, next_entry, &with)
    }

    /// The splice that takes a sequence item out, and the item's text
    /// dedented to its dash. Between blank-separated items the blank line
    /// goes with the item below it, except for the last item, which takes the
    /// one above. Taking the only item leaves `key: []`.
    fn cut(&self, path: &Path, id: usize, parent: usize) -> Result<Cut, Error> {
        let source = &self.source;
        let start = self.index.owned_start(source, id);
        let end = line_end(source, self.index.nodes[id].value.end);
        let text = dedent(&source[start..end], column(source, self.dash(id)));
        let siblings = &self.index.nodes[parent].children;
        if siblings.len() == 1 {
            let (colon, _) = self.colon(path, parent).map_err(|_| Error::Unsupported {
                path: path.clone(),
                what: "take the only item of a sequence with no key",
            })?;
            let (properties, comment) = self.key_line_parts(colon, None);
            let with = format!(" {}{LF}", spaced(&[&properties, "[]", &comment]));
            return Ok(Cut {
                from: colon,
                to: end,
                with,
                text,
            });
        }
        let (mut from, mut to) = (start, end);
        if siblings.last() == Some(&id) {
            while from > 0 {
                let previous = line_start(source, from - 1);
                if !source[previous..from].trim().is_empty() {
                    break;
                }
                from = previous;
            }
        } else {
            while to < source.len() && source[to..line_end(source, to)].trim().is_empty() {
                to = line_end(source, to);
            }
        }
        Ok(Cut {
            from,
            to,
            with: String::new(),
            text,
        })
    }

    /// The byte of the `-` that starts a sequence item.
    fn dash(&self, id: usize) -> usize {
        self.dash_before(self.index.nodes[id].value.start)
    }

    /// The byte of the last `-` on the line before `at`, or `at` itself.
    fn dash_before(&self, at: usize) -> usize {
        let line = line_start(&self.source, at);
        self.source[line..at]
            .rfind('-')
            .map_or(at, |offset| line + offset)
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

    fn push_rendered(&mut self, path: &Path, rendered: &Rendered) -> Result<(), Error> {
        let id = self.id(path)?;
        let length = self.index.nodes[id].children.len();
        let fragment = Fragment {
            text: format!("- {}{LF}", rendered.after_dash(0)),
        };
        self.put(path, length, &fragment)
    }

    fn commit_splice(&mut self, from: usize, to: usize, with: &str) -> Result<(), Error> {
        let edited = [&self.source[..from], with, &self.source[to..]].concat();
        let index = Index::build(&edited).map_err(|error| Error::Invalid {
            message: error.to_string(),
        })?;
        self.compact = compact_lists(&edited, &index);
        self.source = edited;
        self.index = index;
        Ok(())
    }
}

/// A document prints as its current text.
impl fmt::Display for Document {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.source)
    }
}

/// The non-empty parts, joined by one space.
fn spaced(parts: &[&str]) -> String {
    parts
        .iter()
        .copied()
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

/// Whether the file writes list items at their key's column rather than
/// under it, judged from the first block list that is a mapping value.
fn compact_lists(source: &str, index: &Index) -> bool {
    index
        .nodes
        .iter()
        .find(|node| node.style == Style::BlockSequence && node.key.is_some())
        .and_then(|node| {
            let key = node.key.as_ref()?;
            Some(column(source, node.value.start) == column(source, key.range.start))
        })
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use indoc::indoc;

    use super::Document;
    use crate::Error;

    #[test]
    fn an_edit_that_would_not_parse_leaves_the_text_unchanged() {
        let source = indoc! {"
            a: 1
            b: 2
        "};
        let mut document = Document::parse(source).unwrap();
        let broken = document.commit_splice(0, 4, "a: [1");
        assert!(matches!(broken, Err(Error::Invalid { .. })));
        assert_eq!(document.as_str(), source);
    }
}
