use std::collections::BTreeSet;
use std::ops::Range;

use granit_parser::{Event, Parser, ScalarStyle, StructureStyle};

use crate::text::{LF, line_start, trim_end};
use crate::{Error, Path, Segment};

/// How a node is written in the source.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Style {
    /// A plain scalar: `value`.
    Plain,
    /// A single-quoted scalar: `'value'`.
    SingleQuoted,
    /// A double-quoted scalar: `"value"`.
    DoubleQuoted,
    /// A literal block scalar: `|`.
    Literal,
    /// A folded block scalar: `>`.
    Folded,
    /// An alias to an anchored node: `*name`.
    Alias,
    /// A sequence written one `- ` item per line.
    BlockSequence,
    /// A sequence written as `[a, b]`.
    FlowSequence,
    /// A mapping written one `key: value` per line.
    BlockMapping,
    /// A mapping written as `{k: v}`.
    FlowMapping,
}

impl Style {
    pub(crate) const fn is_block_collection(self) -> bool {
        matches!(self, Self::BlockSequence | Self::BlockMapping)
    }

    #[cfg(feature = "serde")]
    pub(crate) const fn is_scalar(self) -> bool {
        matches!(
            self,
            Self::Plain | Self::SingleQuoted | Self::DoubleQuoted | Self::Literal | Self::Folded
        )
    }
}

/// A byte range in the source, with the line and column where it starts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Location {
    /// First byte.
    pub start: usize,
    /// One past the last byte.
    pub end: usize,
    /// 1-based line of `start`.
    pub line: usize,
    /// 1-based column of `start`, counted in characters.
    pub column: usize,
}

impl Location {
    pub(crate) fn of(source: &str, range: Range<usize>) -> Self {
        let line = source[..range.start].matches(LF).count() + 1;
        let column = source[line_start(source, range.start)..range.start]
            .chars()
            .count()
            + 1;
        Self {
            start: range.start,
            end: range.end,
            line,
            column,
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) struct Key {
    pub(crate) text: String,
    pub(crate) range: Range<usize>,
}

#[derive(Clone, Debug)]
pub(crate) struct Node {
    pub(crate) value: Range<usize>,
    pub(crate) key: Option<Key>,
    pub(crate) style: Style,
    pub(crate) parent: Option<usize>,
    pub(crate) children: Vec<usize>,
}

/// Every node of one document, in an arena whose first entry is the root.
#[derive(Clone, Debug)]
pub(crate) struct Index {
    pub(crate) nodes: Vec<Node>,
    comment_lines: BTreeSet<usize>,
}

struct Frame {
    node: usize,
    pending_key: Option<Key>,
    expect_key: bool,
    flow: bool,
}

/// The state of one pass over the parser's events.
#[derive(Default)]
struct Builder {
    nodes: Vec<Node>,
    stack: Vec<Frame>,
    comment_lines: BTreeSet<usize>,
    documents: usize,
}

impl Builder {
    fn event(
        &mut self,
        source: &str,
        event: Event<'_>,
        start: usize,
        end: usize,
    ) -> Result<(), Error> {
        match event {
            Event::DocumentStart(..) => {
                self.documents += 1;
                if self.documents > 1 {
                    return Err(Error::Unsupported {
                        path: Path::root(),
                        what: "edit a stream of several documents",
                    });
                }
            }
            Event::Comment(..) => {
                let line = line_start(source, start);
                if source[line..start].trim().is_empty() {
                    self.comment_lines.insert(line);
                }
            }
            Event::Scalar(text, style, _, _) => {
                self.scalar(source, text.into_owned(), style, start, end);
            }
            Event::Alias(_) => {
                self.attach(Node::leaf(start..end, Style::Alias));
            }
            Event::SequenceStart(structure, ..) => self.open(false, structure, start),
            Event::MappingStart(structure, ..) => self.open(true, structure, start),
            Event::SequenceEnd | Event::MappingEnd => {
                if let Some(frame) = self.stack.pop()
                    && frame.flow
                {
                    self.nodes[frame.node].value.end = end;
                }
            }
            _ => {}
        }
        Ok(())
    }

    fn scalar(&mut self, source: &str, text: String, style: ScalarStyle, start: usize, end: usize) {
        if let Some(top) = self.stack.last_mut()
            && top.expect_key
        {
            top.pending_key = Some(Key {
                text,
                range: start..end,
            });
            top.expect_key = false;
            return;
        }
        let end = trim_end(source, start, end);
        self.attach(Node::leaf(start..end, scalar_style(style)));
    }

    fn open(&mut self, mapping: bool, structure: StructureStyle, start: usize) {
        let flow = matches!(structure, StructureStyle::Flow);
        let style = match (mapping, flow) {
            (true, true) => Style::FlowMapping,
            (true, false) => Style::BlockMapping,
            (false, true) => Style::FlowSequence,
            (false, false) => Style::BlockSequence,
        };
        let node = self.attach(Node::leaf(start..start, style));
        self.stack.push(Frame {
            node,
            pending_key: None,
            expect_key: mapping,
            flow,
        });
    }

    fn attach(&mut self, mut node: Node) -> usize {
        let id = self.nodes.len();
        if let Some(top) = self.stack.last_mut() {
            node.parent = Some(top.node);
            node.key = top.pending_key.take();
            top.expect_key = matches!(
                self.nodes[top.node].style,
                Style::BlockMapping | Style::FlowMapping
            );
            self.nodes[top.node].children.push(id);
        }
        self.nodes.push(node);
        id
    }

    /// A block collection ends at the end of its last value, not where the
    /// parser reports, which is the start of the next token after any blank
    /// lines and comments.
    fn finish(mut self) -> Index {
        for node in (0..self.nodes.len()).rev() {
            if self.nodes[node].style.is_block_collection()
                && let Some(end) = self.nodes[node]
                    .children
                    .iter()
                    .map(|&child| self.nodes[child].value.end)
                    .max()
            {
                self.nodes[node].value.end = end;
            }
        }
        Index {
            nodes: self.nodes,
            comment_lines: self.comment_lines,
        }
    }
}

impl Node {
    fn leaf(value: Range<usize>, style: Style) -> Self {
        Self {
            value,
            key: None,
            style,
            parent: None,
            children: Vec::new(),
        }
    }
}

impl Index {
    /// Index one document. The parse is also the validity check every edit
    /// relies on, which is why an edit rebuilds the whole index.
    pub(crate) fn build(source: &str) -> Result<Self, Error> {
        let mut builder = Builder::default();
        for next in Parser::new_from_str(source) {
            let (event, span) = next.map_err(|error| Error::Parse {
                line: error.marker().line(),
                column: error.marker().col() + 1,
                message: error.info(),
            })?;
            let start = span.start.byte_offset().unwrap_or(0);
            let end = span.end.byte_offset().unwrap_or(start);
            builder.event(source, event, start, end)?;
        }
        Ok(builder.finish())
    }

    /// The node a path leads to. A key segment that is a number addresses an
    /// index on a sequence, and an index segment addresses its spelling on a
    /// mapping, so a JSON pointer resolves without knowing the shape first.
    pub(crate) fn find(&self, path: &Path) -> Option<usize> {
        self.walk(path).ok()
    }

    /// How far a path resolves: the node reached and how many segments it
    /// consumed.
    pub(crate) fn walk(&self, path: &Path) -> Result<usize, (usize, usize)> {
        if self.nodes.is_empty() {
            return Err((0, 0));
        }
        let mut at = 0;
        for (depth, segment) in path.segments().iter().enumerate() {
            let node = &self.nodes[at];
            let child = match (node.style, segment) {
                (Style::BlockMapping | Style::FlowMapping, Segment::Key(key)) => {
                    self.child_by_key(at, key)
                }
                (Style::BlockMapping | Style::FlowMapping, Segment::Index(index)) => {
                    self.child_by_key(at, &index.to_string())
                }
                (Style::BlockSequence | Style::FlowSequence, Segment::Index(index)) => {
                    node.children.get(*index).copied()
                }
                (Style::BlockSequence | Style::FlowSequence, Segment::Key(key)) => key
                    .parse::<usize>()
                    .ok()
                    .and_then(|index| node.children.get(index).copied()),
                _ => None,
            };
            match child {
                Some(child) => at = child,
                None => return Err((at, depth)),
            }
        }
        Ok(at)
    }

    fn child_by_key(&self, node: usize, key: &str) -> Option<usize> {
        self.nodes[node].children.iter().copied().find(|&child| {
            self.nodes[child]
                .key
                .as_ref()
                .is_some_and(|k| k.text == key)
        })
    }

    /// Where a node's entry begins: its key for a mapping value, else its
    /// value. A sequence item's dash sits on the same line.
    pub(crate) fn entry_start(&self, node: usize) -> usize {
        let node = &self.nodes[node];
        node.key
            .as_ref()
            .map_or(node.value.start, |key| key.range.start)
    }

    /// The first byte a node owns: the start of its entry's line, moved up
    /// over comment lines that sit directly above it.
    pub(crate) fn owned_start(&self, source: &str, node: usize) -> usize {
        let mut start = line_start(source, self.entry_start(node));
        while start > 0 {
            let previous = line_start(source, start - 1);
            if self.comment_lines.contains(&previous) {
                start = previous;
            } else {
                break;
            }
        }
        start
    }
}

fn scalar_style(style: ScalarStyle) -> Style {
    match style {
        ScalarStyle::SingleQuoted => Style::SingleQuoted,
        ScalarStyle::DoubleQuoted => Style::DoubleQuoted,
        ScalarStyle::Literal => Style::Literal,
        ScalarStyle::Folded => Style::Folded,
        ScalarStyle::Plain => Style::Plain,
    }
}
