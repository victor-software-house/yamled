//! Edits that rearrange a block collection's lines without writing values:
//! reordering its children and moving it to another column.

use super::{Document, blank_run_end};
use crate::index::{Index, Style};
use crate::path::Segment;
use crate::text::{LF, column, dedent, line_end, pad, starts_line, terminated};
use crate::{Error, Path};

impl Document {
    /// Reorder some children of a block sequence or mapping. The children
    /// `order` names, by index or key, trade places among the slots they
    /// hold, in that order, each with its comments. Children it does not
    /// name, and the blank lines and loose comments between children, stay
    /// where they are, except blank lines that would land after a child
    /// whose last block scalar keeps its trailing lines (`|+`), which are
    /// dropped so that value does not change.
    ///
    /// # Errors
    ///
    /// [`Error::NoNode`] for a missing collection or child,
    /// [`Error::WrongKind`] when the path is not a block collection,
    /// [`Error::Repeated`] for a child named twice, [`Error::Unsupported`]
    /// for a child that shares its line with its parent's dash, and
    /// [`Error::Invalid`].
    pub fn reorder(&mut self, path: &Path, order: &[Segment]) -> Result<(), Error> {
        let id = self.block_collection(path)?;
        let children = self.index.nodes[id].children.clone();
        let mut named: Vec<usize> = Vec::with_capacity(order.len());
        for segment in order {
            let child_path = match segment {
                Segment::Key(key) => path.clone().key(key.clone()),
                Segment::Index(index) => path.clone().index(*index),
            };
            let slot = self
                .index
                .find(&child_path)
                .and_then(|child| children.iter().position(|&c| c == child))
                .ok_or_else(|| Error::NoNode {
                    path: child_path.clone(),
                })?;
            if named.contains(&slot) {
                return Err(Error::Repeated { path: child_path });
            }
            named.push(slot);
        }
        if let Some(&child) = children.iter().find(|&&child| !self.starts_own_line(child)) {
            return Err(Error::Unsupported {
                path: path.clone(),
                what: match self.index.nodes[child].key {
                    Some(_) => "reorder a mapping whose first key shares its dash's line",
                    None => "reorder a sequence whose first item shares its parent's line",
                },
            });
        }
        let mut slots = named.clone();
        slots.sort_unstable();
        let mut placed: Vec<usize> = (0..children.len()).collect();
        for (&slot, &child) in slots.iter().zip(&named) {
            placed[slot] = child;
        }
        let spans: Vec<(usize, usize)> = children
            .iter()
            .map(|&child| {
                let start = self.index.owned_start(&self.source, child);
                (start, line_end(&self.source, self.item_end(child)))
            })
            .collect();
        let (from, mut to) = (spans[0].0, spans[spans.len() - 1].1);
        let mut text = String::new();
        for (slot, &child) in placed.iter().enumerate() {
            let (start, end) = spans[child];
            let written = &self.source[start..end];
            text.push_str(written);
            if !written.ends_with(LF) {
                text.push(LF);
            }
            let keeps = self.ends_in_kept_lines(children[child]);
            match spans.get(slot + 1) {
                Some(&(next, _)) => {
                    let gap = &self.source[spans[slot].1..next];
                    if keeps {
                        gap.lines()
                            .filter(|line| !line.trim().is_empty())
                            .for_each(|line| text.push_str(&terminated(line)));
                    } else {
                        text.push_str(gap);
                    }
                }
                None if keeps => to = blank_run_end(&self.source, to),
                None => {}
            }
        }
        if !self.source[..to].ends_with(LF) {
            text.pop();
        }
        self.commit_splice(from, to, &text)
    }

    /// Move a block sequence or mapping so its children start at `column`.
    /// Every line of the children, with their comments, nested collections,
    /// and block scalars, shifts by the same number of columns; empty lines
    /// stay empty.
    ///
    /// # Errors
    ///
    /// [`Error::NoNode`], [`Error::WrongKind`] when the path is not a block
    /// collection, [`Error::Unsupported`] for a collection that starts on its
    /// parent's line, and [`Error::Invalid`] when the shifted text does not
    /// parse or parses as a different structure.
    pub fn reindent(&mut self, path: &Path, column: usize) -> Result<(), Error> {
        let id = self.block_collection(path)?;
        let children = &self.index.nodes[id].children;
        let (Some(&first), Some(&last)) = (children.first(), children.last()) else {
            return Err(Error::WrongKind {
                path: path.clone(),
                expected: "a collection with children",
            });
        };
        if !self.starts_own_line(first) {
            return Err(Error::Unsupported {
                path: path.clone(),
                what: "reindent a collection that starts on its parent's line",
            });
        }
        let current = self.child_column(first);
        let from = self.index.owned_start(&self.source, first);
        let to = line_end(&self.source, self.index.nodes[last].value.end);
        let block = &self.source[from..to];
        let shifted = if column >= current {
            pad(block, column - current)
        } else {
            dedent(block, current - column)
        };
        let before = shape(&self.index);
        let edited = [&self.source[..from], &shifted, &self.source[to..]].concat();
        let index = Index::build(&edited).map_err(|error| Error::Invalid {
            message: error.to_string(),
        })?;
        if shape(&index) != before {
            return Err(Error::Invalid {
                message: format!("re-indenting {path} would change the document's structure"),
            });
        }
        self.commit(edited, index);
        Ok(())
    }

    fn block_collection(&self, path: &Path) -> Result<usize, Error> {
        let id = self.id(path)?;
        match self.index.nodes[id].style {
            Style::BlockSequence | Style::BlockMapping => Ok(id),
            _ => Err(Error::WrongKind {
                path: path.clone(),
                expected: "a block sequence or mapping",
            }),
        }
    }

    /// Whether a child's entry, or its dash for a sequence item, is the
    /// first thing on its line.
    fn starts_own_line(&self, child: usize) -> bool {
        let at = match self.index.nodes[child].key {
            Some(_) => self.index.entry_start(child),
            None => self.dash(child),
        };
        starts_line(&self.source, at)
    }

    /// The column of a child's key, or of its dash for a sequence item.
    fn child_column(&self, child: usize) -> usize {
        match self.index.nodes[child].key {
            Some(_) => column(&self.source, self.index.entry_start(child)),
            None => column(&self.source, self.dash(child)),
        }
    }
}

/// Each node's style, child count, and key: what must survive a re-indent.
fn shape(index: &Index) -> Vec<(Style, usize, Option<&str>)> {
    index
        .nodes
        .iter()
        .map(|node| {
            let key = node.key.as_ref().map(|key| key.text.as_str());
            (node.style, node.children.len(), key)
        })
        .collect()
}
