use std::fmt;
use std::ops::Range;

use crate::index::{Index, Location, Style};
use crate::text::{
    LF, NEWLINE, column, comment_start, dash_before, dedent, has_blank_line, indent, join,
    line_end, line_start, spaced, starts_line, terminated,
};
use crate::{Error, Path};

mod arrange;
#[cfg(feature = "serde")]
mod values;

#[cfg(feature = "serde")]
pub use values::Position;

/// Whether a new sequence item is set off from its neighbours by a blank line,
/// for a sequence whose own items do not settle it. A sequence whose
/// neighbours are all separated, or all adjacent, keeps its spacing whatever
/// this says; one with fewer than two items, or with both kinds of gap, gets
/// this spacing. Set it with [`Document::with_spacing`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[non_exhaustive]
pub enum Spacing {
    /// Items follow each other directly.
    #[default]
    Tight,
    /// One blank line between items.
    Blank,
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

    /// The lines this node owns: its comments above, its entry, and its
    /// value, with the blank lines a block scalar keeps (`|+`).
    #[must_use]
    pub fn owned(&self) -> Location {
        let source = &self.document.source;
        let index = &self.document.index;
        let start = index.owned_start(source, self.id);
        let end = line_end(source, self.document.kept_end(self.id));
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
    spacing: Spacing,
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
            spacing: Spacing::default(),
        })
    }

    /// Use `spacing` for new items where a sequence does not settle it.
    #[must_use]
    pub fn with_spacing(mut self, spacing: Spacing) -> Self {
        self.spacing = spacing;
        self
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

    /// Remove the node at a path with its key, the comments it owns, and its
    /// line break. Neighbours keep every byte, including their comments.
    /// An item of a flow sequence written on one line goes with one
    /// separator, and the last one leaves `[]`.
    ///
    /// # Errors
    ///
    /// [`Error::NoNode`], [`Error::Unsupported`] for a node inside a flow
    /// mapping or a flow sequence that spans lines, or the only entry of a
    /// mapping, and [`Error::Invalid`].
    pub fn remove(&mut self, path: &Path) -> Result<(), Error> {
        let id = self.id(path)?;
        let parent = self.parent(path, id)?;
        match self.index.nodes[parent].style {
            Style::BlockSequence => {
                let cut = self.cut(path, id, parent)?;
                self.commit_splice(cut.from, cut.to, &cut.with)
            }
            Style::BlockMapping => self.remove_entry(path, id, parent),
            Style::FlowSequence => self.remove_flow_item(path, id, parent),
            _ => Err(Error::Unsupported {
                path: path.clone(),
                what: "remove from a flow mapping",
            }),
        }
    }

    /// Take a sequence item out, with the comments it owns.
    ///
    /// # Errors
    ///
    /// [`Error::NoNode`], [`Error::WrongKind`] when the parent is not a block
    /// sequence, [`Error::Unsupported`] for the only item of a sequence with
    /// no key or an item that ends in a block scalar keeping its trailing
    /// lines (`|+`), and [`Error::Invalid`].
    pub fn take(&mut self, path: &Path) -> Result<Fragment, Error> {
        let id = self.id(path)?;
        let parent = self.parent(path, id)?;
        if self.index.nodes[parent].style != Style::BlockSequence {
            return Err(Error::WrongKind {
                path: path.clone(),
                expected: "an item of a block sequence",
            });
        }
        if self.ends_in_kept_lines(id) {
            return Err(Error::Unsupported {
                path: path.clone(),
                what: "take an item that ends in a block scalar keeping its trailing lines",
            });
        }
        let cut = self.cut(path, id, parent)?;
        self.commit_splice(cut.from, cut.to, &cut.with)?;
        Ok(Fragment { text: cut.text })
    }

    /// Insert a fragment into a sequence at an index, re-indented to it, and
    /// spaced as [`Spacing`] describes.
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
                let dash = node.children.first().map_or_else(
                    || column(&self.source, node.value.start),
                    |&first| column(&self.source, self.dash(first)),
                );
                let block = join(&indent(&lines, dash));
                let gap = match self.item_spacing(id) {
                    Spacing::Tight => "",
                    Spacing::Blank => NEWLINE,
                };
                if let Some(&child) = node.children.get(index) {
                    let at = self.index.owned_start(&self.source, child);
                    let kept = lines.last().is_some_and(|line| line.trim().is_empty());
                    let gap = if kept { "" } else { gap };
                    let text = format!("{block}{LF}{gap}");
                    self.commit_splice(at, at, &text)
                } else if let Some(&last) = node.children.last()
                    && index == node.children.len()
                {
                    let at = line_end(&self.source, self.item_end(last));
                    let lead = if self.source[..at].ends_with(LF) {
                        ""
                    } else {
                        NEWLINE
                    };
                    let gap = if self.ends_in_kept_lines(last) {
                        ""
                    } else {
                        gap
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

    /// The spacing a sequence's items already share, or the document's.
    fn item_spacing(&self, sequence: usize) -> Spacing {
        let mut gaps = self.index.nodes[sequence].children.windows(2).map(|pair| {
            let after = line_end(&self.source, self.item_end(pair[0]));
            has_blank_line(
                &self.source,
                after,
                self.index.owned_start(&self.source, pair[1]),
            )
        });
        match gaps.next() {
            Some(first) if gaps.all(|gap| gap == first) => {
                if first {
                    Spacing::Blank
                } else {
                    Spacing::Tight
                }
            }
            _ => self.spacing,
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

    /// The byte after a mapping value's `:`, and the key's column. The `:`
    /// is looked for between the key and its value, or on the key's line
    /// for an empty value, and never inside a comment.
    fn colon(&self, path: &Path, id: usize) -> Result<(usize, usize), Error> {
        let key = self.index.nodes[id]
            .key
            .as_ref()
            .ok_or_else(|| Error::Unsupported {
                path: path.clone(),
                what: "fill an empty value that has no key",
            })?;
        let value = &self.index.nodes[id].value;
        let limit = if value.start > key.range.end {
            value.start
        } else {
            line_end(&self.source, key.range.end)
        };
        let after = &self.source[key.range.end..limit];
        let mut line_offset = 0;
        let mut found = None;
        for line in after.split_inclusive(LF) {
            let code = comment_start(line).map_or(line, |comment| &line[..comment]);
            if let Some(colon) = code.find(':') {
                found = Some(line_offset + colon);
                break;
            }
            line_offset += line.len();
        }
        let offset = found.ok_or_else(|| Error::Unsupported {
            path: path.clone(),
            what: "write the value of a key with no ':' after it",
        })?;
        Ok((
            key.range.end + offset + 1,
            column(&self.source, key.range.start),
        ))
    }

    /// Write `lines` as the items of an empty value (`[]`, `~`, `null`, or
    /// nothing) under its key. The key line keeps its node properties and
    /// comment; an empty value on a later line is replaced with its line, and
    /// its own properties and comment move up to the key line. A tag other
    /// than `!!seq`, short or verbatim, would contradict the items, so it is
    /// refused.
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
                        what: "fill an empty value with lines between it and its key",
                    });
                }
                properties = spaced(&[&properties, self.source[value_line..value.start].trim()]);
            }
            let end = line_end(&self.source, value.end);
            comment = spaced(&[&comment, self.source[value.end..end].trim()]);
            end
        };
        if properties.split_whitespace().any(|property| {
            property.starts_with('!') && !matches!(property, "!!seq" | "!<tag:yaml.org,2002:seq>")
        }) {
            return Err(Error::Unsupported {
                path: path.clone(),
                what: "fill an empty value whose tag is not !!seq",
            });
        }
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
        let (properties, comment) = rest.split_at(comment_start(&rest).unwrap_or(rest.len()));
        (properties.trim().to_owned(), comment.trim().to_owned())
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
        let dash_column = column(&self.source, dash_before(&self.source, entry));
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
        let previous_keeps = siblings
            .iter()
            .position(|&sibling| sibling == id)
            .and_then(|at| at.checked_sub(1))
            .is_some_and(|at| self.ends_in_kept_lines(siblings[at]));
        if siblings.last() == Some(&id) && !previous_keeps {
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

    fn remove_flow_item(&mut self, path: &Path, id: usize, parent: usize) -> Result<(), Error> {
        let sequence = self.index.nodes[parent].value.clone();
        if self.source[sequence.clone()].contains(LF) {
            return Err(Error::Unsupported {
                path: path.clone(),
                what: "remove from a flow sequence that spans lines",
            });
        }
        let children = &self.index.nodes[parent].children;
        let at = children
            .iter()
            .position(|&child| child == id)
            .unwrap_or_default();
        let (from, to) = match (at.checked_sub(1), children.get(at + 1)) {
            (_, Some(_)) => (
                self.flow_item_start(parent, at),
                self.flow_item_start(parent, at + 1),
            ),
            (Some(previous), None) => (
                self.index.nodes[children[previous]].value.end,
                self.index.nodes[id].value.end,
            ),
            (None, None) => (sequence.start + 1, sequence.end - 1),
        };
        self.commit_splice(from, to, "")
    }

    /// Where item `k` of a one-line flow sequence begins: after the `[` or
    /// the comma before it, and the spaces after that.
    fn flow_item_start(&self, sequence: usize, k: usize) -> usize {
        let node = &self.index.nodes[sequence];
        let after = match k.checked_sub(1) {
            None => node.value.start + 1,
            Some(previous) => {
                let end = self.index.nodes[node.children[previous]].value.end;
                self.source[end..]
                    .find(',')
                    .map_or(end, |comma| end + comma + 1)
            }
        };
        let rest = &self.source[after..];
        after + rest.len() - rest.trim_start_matches(' ').len()
    }

    /// Whether a node's text ends in a block scalar that keeps its trailing
    /// line breaks (`|+` or `>+`). The blank lines after such a node are part
    /// of its value, so they cannot stay behind when it moves.
    fn ends_in_kept_lines(&self, id: usize) -> bool {
        let mut last = id;
        while let Some(&child) = self.index.nodes[last].children.last() {
            last = child;
        }
        self.keeps_trailing_lines(last)
    }

    /// Where a sequence item's text ends: the end of its value, or of the
    /// blank lines its last block scalar keeps (`|+`), which belong to it.
    fn item_end(&self, id: usize) -> usize {
        let mut last = id;
        while let Some(&child) = self.index.nodes[last].children.last() {
            last = child;
        }
        self.kept_end(last).max(self.index.nodes[id].value.end)
    }

    /// Whether a node is a block scalar with keep chomping (`|+` or `>+`).
    fn keeps_trailing_lines(&self, id: usize) -> bool {
        let node = &self.index.nodes[id];
        matches!(node.style, Style::Literal | Style::Folded)
            && self.source[node.value.start..]
                .split_whitespace()
                .next()
                .is_some_and(|header| header.contains('+'))
    }

    /// Where a node's value ends, including the blank lines after a block
    /// scalar that keeps them: the index trims those, but they are part of
    /// its value, so a replace must take them.
    fn kept_end(&self, id: usize) -> usize {
        let mut end = self.index.nodes[id].value.end;
        if !self.keeps_trailing_lines(id) {
            return end;
        }
        loop {
            let next = line_end(&self.source, end);
            if next >= self.source.len() {
                break;
            }
            let line = &self.source[next..line_end(&self.source, next)];
            if !line.trim().is_empty() {
                break;
            }
            end = next + line.trim_end_matches(LF).len();
        }
        end
    }

    /// The byte of the `-` that starts a sequence item.
    fn dash(&self, id: usize) -> usize {
        dash_before(&self.source, self.index.nodes[id].value.start)
    }

    fn commit_splice(&mut self, from: usize, to: usize, with: &str) -> Result<(), Error> {
        let edited = [&self.source[..from], with, &self.source[to..]].concat();
        let index = Index::build(&edited).map_err(|error| Error::Invalid {
            message: error.to_string(),
        })?;
        self.commit(edited, index);
        Ok(())
    }

    fn commit(&mut self, source: String, index: Index) {
        self.compact = compact_lists(&source, &index);
        self.source = source;
        self.index = index;
    }
}

/// A document prints as its current text.
impl fmt::Display for Document {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.source)
    }
}

/// Whether the file writes list items at their key's column rather than
/// under it, judged from the first block list that is a mapping value. The
/// parser starts such a list's span at its first item's value, so the column
/// is read from that item's dash.
fn compact_lists(source: &str, index: &Index) -> bool {
    index
        .nodes
        .iter()
        .find(|node| node.style == Style::BlockSequence && node.key.is_some())
        .and_then(|node| {
            let key = node.key.as_ref()?;
            let first = index.nodes.get(*node.children.first()?)?;
            let dash = dash_before(source, first.value.start);
            Some(column(source, dash) == column(source, key.range.start))
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
