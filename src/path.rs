use std::fmt;

use crate::Error;

/// One step from a node to a child: a mapping key or a sequence index.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Segment {
    /// A mapping key. On a sequence, a key that is a decimal number addresses
    /// that index, which is how a JSON pointer names an item.
    Key(String),
    /// A sequence index. On a mapping, it addresses the key spelled as that
    /// number.
    Index(usize),
}

/// Where a node sits in a document, as the keys and indexes that lead to it.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct Path(Vec<Segment>);

impl Path {
    /// The document's root node.
    #[must_use]
    pub fn root() -> Self {
        Self::default()
    }

    /// This path extended by a mapping key.
    #[must_use]
    pub fn key(mut self, key: impl Into<String>) -> Self {
        self.0.push(Segment::Key(key.into()));
        self
    }

    /// This path extended by a sequence index.
    #[must_use]
    pub fn index(mut self, index: usize) -> Self {
        self.0.push(Segment::Index(index));
        self
    }

    /// An RFC 6901 JSON pointer such as `/queue/3/title`, the form a JSON
    /// Schema validator reports an error at. The empty pointer is the root.
    ///
    /// # Errors
    ///
    /// [`Error::Pointer`] when a non-empty pointer does not start with `/`, or
    /// when `~` is followed by anything but `0` or `1`.
    pub fn from_pointer(pointer: &str) -> Result<Self, Error> {
        if pointer.is_empty() {
            return Ok(Self::root());
        }
        let invalid = || Error::Pointer {
            pointer: pointer.to_owned(),
        };
        let rest = pointer.strip_prefix('/').ok_or_else(invalid)?;
        let mut path = Self::root();
        for raw in rest.split('/') {
            let mut segment = String::with_capacity(raw.len());
            let mut chars = raw.chars();
            while let Some(c) = chars.next() {
                if c == '~' {
                    match chars.next() {
                        Some('0') => segment.push('~'),
                        Some('1') => segment.push('/'),
                        _ => return Err(invalid()),
                    }
                } else {
                    segment.push(c);
                }
            }
            path.0.push(Segment::Key(segment));
        }
        Ok(path)
    }

    /// The steps from the root, in order.
    #[must_use]
    pub fn segments(&self) -> &[Segment] {
        &self.0
    }

    /// The path of the node that holds this one; `None` for the root.
    #[must_use]
    pub fn parent(&self) -> Option<Self> {
        let (_, parent) = self.0.split_last()?;
        Some(Self(parent.to_vec()))
    }
}

/// A path prints as its JSON pointer.
impl fmt::Display for Path {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        for segment in &self.0 {
            match segment {
                Segment::Key(key) => {
                    write!(formatter, "/{}", key.replace('~', "~0").replace('/', "~1"))?;
                }
                Segment::Index(index) => write!(formatter, "/{index}")?,
            }
        }
        Ok(())
    }
}
