use std::fmt;

use crate::Path;

/// Why a document could not be read or an edit could not be made. A failed
/// edit leaves the document as it was.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Error {
    /// The source is not valid YAML.
    Parse {
        /// 1-based line of the problem.
        line: usize,
        /// 1-based column of the problem.
        column: usize,
        /// The parser's description.
        message: String,
    },
    /// A JSON pointer that RFC 6901 does not allow.
    Pointer {
        /// The pointer as given.
        pointer: String,
    },
    /// No node sits at this path.
    NoNode {
        /// The path that was asked for.
        path: Path,
    },
    /// The node at this path is not the kind the edit needs.
    WrongKind {
        /// The path of the node.
        path: Path,
        /// What the edit needed, such as "a block sequence".
        expected: &'static str,
    },
    /// The mapping already has this key.
    KeyExists {
        /// The path of the key that exists.
        path: Path,
    },
    /// The edit is valid YAML but outside what this version can write in
    /// place.
    Unsupported {
        /// The path of the node.
        path: Path,
        /// What could not be done.
        what: &'static str,
    },
    /// The edit would have left text that does not parse.
    Invalid {
        /// The parser's description of the edited text.
        message: String,
    },
    /// The value could not be serialized.
    Serialize {
        /// The serializer's description.
        message: String,
    },
}

impl fmt::Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Parse {
                line,
                column,
                message,
            } => write!(formatter, "line {line}, column {column}: {message}"),
            Self::Pointer { pointer } => write!(formatter, "{pointer:?} is not a JSON pointer"),
            Self::NoNode { path } => write!(formatter, "no node at {path}"),
            Self::WrongKind { path, expected } => {
                write!(formatter, "the node at {path} is not {expected}")
            }
            Self::KeyExists { path } => write!(formatter, "{path} already exists"),
            Self::Unsupported { path, what } => write!(formatter, "at {path}: cannot {what} yet"),
            Self::Invalid { message } => write!(formatter, "the edit would not parse: {message}"),
            Self::Serialize { message } => {
                write!(formatter, "cannot serialize the value: {message}")
            }
        }
    }
}

impl std::error::Error for Error {}
