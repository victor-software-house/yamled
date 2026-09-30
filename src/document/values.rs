//! Edits that write a value: through `serde-saphyr` for a serializable value,
//! or through yamled's scalar rule for a string.

use std::ops::Range;

use serde::Serialize;

use super::{Document, Fragment};
use crate::index::Style;
use crate::render::{self, Rendered, TextStyle};
use crate::text::{LF, NEWLINE, column, comment_start, line_end, line_start, spaced, starts_line};
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
        let value = node.value.start..self.kept_end(id);
        let parent_style = node.parent.map(|parent| self.index.nodes[parent].style);
        match parent_style {
            Some(Style::BlockMapping) => {
                let (colon, key_column) = self.colon(path, id)?;
                let value = value.start.max(colon)..value.end.max(colon);
                let body_column = if rendered.has_indicator() {
                    key_column + 2
                } else {
                    self.block_body_column(id).unwrap_or(key_column + 2)
                };
                let key_line_end = line_end(&self.source, colon);
                let key_line_content =
                    key_line_end - usize::from(self.source[..key_line_end].ends_with(LF));
                let on_key_line = value.start <= key_line_content;
                let block = matches!(self.index.nodes[id].style, Style::Literal | Style::Folded);
                if on_key_line && !block && rendered.inline_head().is_some() {
                    self.check_tags(path, &self.source[colon..value.start], rendered)?;
                    let from = colon + self.source[colon..value.start].trim_end().len();
                    let text = rendered.after_key_at(key_column, body_column, self.compact);
                    return self.commit_splice(from, value.end, &text);
                }
                let (properties, comment) = if on_key_line {
                    self.key_line_parts(colon, Some(&self.head(id)))
                } else {
                    let (properties, comment) = self.key_line_parts(colon, None);
                    let value_line = line_start(&self.source, value.start);
                    let own_line = self.source[value_line..value.start].trim();
                    (spaced(&[&properties, own_line]), comment)
                };
                let properties = node_properties(&properties);
                self.check_tags(path, &properties, rendered)?;
                let to = if on_key_line {
                    value.end.max(key_line_content)
                } else {
                    value.end
                };
                let text = rendered.after_key_with(
                    key_column,
                    body_column,
                    self.compact,
                    &properties,
                    &comment,
                );
                self.commit_splice(colon, to, &text)
            }
            Some(Style::BlockSequence) => {
                let before = self
                    .source
                    .get(self.dash(id) + 1..value.start)
                    .unwrap_or_default();
                let code: Vec<&str> = before
                    .lines()
                    .map(|line| comment_start(line).map_or(line, |at| &line[..at]))
                    .collect();
                self.check_tags(path, &code.join(" "), rendered)?;
                let text = rendered.after_dash(column(&self.source, self.dash(id)));
                let lead = if self.source[..value.start].ends_with([' ', '\t']) {
                    ""
                } else {
                    " "
                };
                self.commit_splice(value.start, value.end, &format!("{lead}{text}"))
            }
            Some(_) if value.is_empty() => Err(Error::Unsupported {
                path: path.clone(),
                what: "write a value where a flow collection has none",
            }),
            Some(_) if let Some(head) = rendered.inline_head() => {
                self.check_tags(
                    path,
                    &properties_before(&self.source, value.start),
                    rendered,
                )?;
                let head = render::flow_safe(head);
                self.commit_splice(value.start, value.end, &head)
            }
            Some(_) => Err(Error::Unsupported {
                path: path.clone(),
                what: "write a block value inside a flow collection",
            }),
            None if value.is_empty() => Err(Error::Unsupported {
                path: path.clone(),
                what: "write a value into an empty document",
            }),
            None => {
                self.check_tags(
                    path,
                    &properties_before(&self.source, value.start),
                    rendered,
                )?;
                let text = rendered.at_root(column(&self.source, value.start));
                self.commit_splice(value.start, value.end, &text)
            }
        }
    }

    /// Refuse a replace whose kept tags would retype the new value. An anchor
    /// and a local tag always stay; a core tag stays only when it names the
    /// new value's kind, so `!!int` stays in front of `8081` and refuses the
    /// edit in front of `abc`. The non-specific tag `!` makes a scalar a string
    /// and leaves a collection as it is, so it fits a string, a list, or a map.
    /// A core tag this version cannot check, such as `!!timestamp`, never fits.
    fn check_tags(&self, path: &Path, properties: &str, rendered: &Rendered) -> Result<(), Error> {
        let handles = self.tag_handles();
        let misfit = properties
            .split_whitespace()
            .filter(|token| token.starts_with('!'))
            .find(|tag| !tag_fits(tag, &handles, rendered));
        match misfit {
            Some(tag) => Err(Error::TagMismatch {
                path: path.clone(),
                tag: tag.to_owned(),
            }),
            None => Ok(()),
        }
    }

    /// The tag handles in force: YAML's defaults, then each `%TAG` directive
    /// written before the document's `---`. Directives start at column 0 and
    /// only comments and blank lines sit between them, so any other line ends
    /// the search and leaves the defaults.
    fn tag_handles(&self) -> Vec<(String, String)> {
        let mut handles = vec![
            ("!".to_owned(), "!".to_owned()),
            ("!!".to_owned(), CORE_TAGS.to_owned()),
        ];
        let mut declared = Vec::new();
        for line in self.source.lines() {
            if line.starts_with("---") {
                handles.extend(declared);
                return handles;
            }
            let directive = line.starts_with('%');
            let mut words = line.split_whitespace();
            match words.next() {
                Some("%TAG") if directive => {
                    if let (Some(handle), Some(prefix)) = (words.next(), words.next()) {
                        declared.push((handle.to_owned(), prefix.to_owned()));
                    }
                }
                None => {}
                Some(word) if directive || word.starts_with('#') => {}
                Some(_) => break,
            }
        }
        handles
    }

    /// The column a block scalar's text starts at: the first line under its
    /// header that is not blank. `None` for any other node, or a block scalar
    /// with no text.
    fn block_body_column(&self, id: usize) -> Option<usize> {
        let node = &self.index.nodes[id];
        if !matches!(node.style, Style::Folded | Style::Literal) {
            return None;
        }
        let body = line_end(&self.source, node.value.start);
        self.source
            .get(body..node.value.end)?
            .lines()
            .find(|line| !line.trim().is_empty())
            .map(|line| line.len() - line.trim_start().len())
    }

    /// The part of a value written on its first line that belongs to it: a
    /// block scalar's header, or the whole value for any other node.
    fn head(&self, id: usize) -> Range<usize> {
        let node = &self.index.nodes[id];
        match node.style {
            Style::Folded | Style::Literal => {
                let header = self.source[node.value.start..]
                    .split_whitespace()
                    .next()
                    .map_or(0, str::len);
                node.value.start..node.value.start + header
            }
            Style::Plain
            | Style::SingleQuoted
            | Style::DoubleQuoted
            | Style::Alias
            | Style::BlockSequence
            | Style::FlowSequence
            | Style::BlockMapping
            | Style::FlowMapping => node.value.clone(),
        }
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
        let at = line_end(&self.source, self.item_end(after));
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

/// The anchors and tags in the text before a value, without a block scalar
/// header such as `|-`, which the new value writes for itself.
fn node_properties(text: &str) -> String {
    let properties: Vec<&str> = text
        .split_whitespace()
        .filter(|token| token.starts_with(['&', '!']))
        .collect();
    properties.join(" ")
}

/// The anchors and tags written just before `start` in a flow collection or
/// at the root, read backwards until a token that is not a property. A token
/// that opens with a flow indicator (`[!!int`) is the last one read; one that
/// ends with an indicator (`!t,`, `a:`) belongs to an earlier node.
fn properties_before(source: &str, start: usize) -> String {
    let mut found = Vec::new();
    for line in source[..start].lines().rev() {
        let code = comment_start(line).map_or(line, |at| &line[..at]);
        for token in code.split_whitespace().rev() {
            let property = token.trim_start_matches(FLOW_INDICATORS);
            if token.ends_with(FLOW_INDICATORS) || !property.starts_with(['&', '!']) {
                return found.join(" ");
            }
            found.push(property);
            if property.len() < token.len() {
                return found.join(" ");
            }
        }
    }
    found.join(" ")
}

/// Whether a kept tag still describes the new value; see
/// [`Document::check_tags`].
fn tag_fits(tag: &str, handles: &[(String, String)], rendered: &Rendered) -> bool {
    if tag == "!" {
        return ["str", "seq", "map"]
            .iter()
            .any(|name| rendered.fits_core_tag(name));
    }
    resolve_tag(tag, handles)
        .as_deref()
        .and_then(|full| full.strip_prefix(CORE_TAGS))
        .is_none_or(|name| rendered.fits_core_tag(name))
}

/// The prefix YAML's core schema tags share.
const CORE_TAGS: &str = "tag:yaml.org,2002:";

/// A tag's full name: a verbatim tag as written, or its handle's prefix
/// joined to its suffix, the handle declared last winning. `None` for a
/// handle nothing declares.
fn resolve_tag(tag: &str, handles: &[(String, String)]) -> Option<String> {
    if let Some(verbatim) = tag.strip_prefix("!<") {
        return verbatim.strip_suffix('>').map(str::to_owned);
    }
    let handle = match tag[1..].find('!') {
        Some(at) => &tag[..at + 2],
        None => "!",
    };
    let (_, prefix) = handles.iter().rev().find(|(name, _)| name == handle)?;
    Some(format!("{prefix}{}", &tag[handle.len()..]))
}

const FLOW_INDICATORS: [char; 4] = ['[', '{', ',', ':'];
