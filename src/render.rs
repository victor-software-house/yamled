//! Turning a value into the text an edit splices in.

use std::collections::BTreeMap;
use std::fmt::{self, Write as _};
use std::iter;

use serde::de::{self, Deserializer, Visitor};
use serde::{Deserialize, Serialize};

use crate::Error;
use crate::index::{Index, Style};
use crate::text::{LF, indent, join, spaced, terminated};

/// How a string should be written when the caller has a preference.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[non_exhaustive]
pub enum TextStyle {
    /// Plain when it reads back unchanged, else single-quoted, else
    /// double-quoted; a string with line breaks tries a literal block first.
    #[default]
    Auto,
    /// A folded block, `>-`, for one long line. Falls back to [`Self::Auto`]
    /// for text with line breaks.
    Folded,
    /// A literal block, `|-`, which keeps every line break.
    Literal,
}

/// What a value looks like before it is placed, relative to column 0.
#[derive(Clone, Debug)]
pub(crate) enum Rendered {
    /// The text on the key's or dash's line, and a block scalar's lines below.
    Scalar { head: String, body: Vec<String> },
    /// A block sequence, one entry per line of text.
    Sequence(Vec<String>),
    /// A block mapping, one entry per line of text.
    Mapping(Vec<String>),
}

impl Rendered {
    fn inline(head: impl Into<String>) -> Self {
        Self::Scalar {
            head: head.into(),
            body: Vec::new(),
        }
    }

    /// The text after `key:`, for a key at `key_column`. `compact` puts list
    /// items at the key's column instead of under it.
    pub(crate) fn after_key(&self, key_column: usize, compact: bool) -> String {
        self.after_key_at(key_column, key_column + 2, compact)
    }

    /// As [`Self::after_key`], with a block scalar's text at `body_column`, so
    /// a replaced block keeps the indentation it had.
    pub(crate) fn after_key_at(
        &self,
        key_column: usize,
        body_column: usize,
        compact: bool,
    ) -> String {
        self.after_key_with(key_column, body_column, compact, "", "")
    }

    /// As [`Self::after_key_at`], with node properties such as `&anchor`
    /// before the value and a comment at the end of the key line.
    pub(crate) fn after_key_with(
        &self,
        key_column: usize,
        body_column: usize,
        compact: bool,
        properties: &str,
        comment: &str,
    ) -> String {
        let (first, rest, column) = match self {
            Self::Scalar { head, body } => {
                (spaced(&[properties, head, comment]), body, body_column)
            }
            Self::Sequence(lines) if compact => (spaced(&[properties, comment]), lines, key_column),
            Self::Sequence(lines) | Self::Mapping(lines) => {
                (spaced(&[properties, comment]), lines, key_column + 2)
            }
        };
        let first = if first.is_empty() {
            first
        } else {
            format!(" {first}")
        };
        let lines: Vec<String> = iter::once(first).chain(indent(rest, column)).collect();
        join(&lines)
    }

    /// The text after `- `, for a dash at `dash_column`: the first line stays
    /// on the dash's line and the rest sit under it.
    pub(crate) fn after_dash(&self, dash_column: usize) -> String {
        let (first, rest) = match self {
            Self::Scalar { head, body } => (head.as_str(), body.as_slice()),
            Self::Sequence(lines) | Self::Mapping(lines) => match lines.split_first() {
                Some((first, rest)) => (first.as_str(), rest),
                None => return String::new(),
            },
        };
        let lines: Vec<String> = iter::once(first.to_owned())
            .chain(indent(rest, dash_column + 2))
            .collect();
        join(&lines)
    }

    /// The text of a root value whose first line starts at `column`: a
    /// collection's other lines line up under it, and a scalar is placed as
    /// after a dash there.
    pub(crate) fn at_root(&self, column: usize) -> String {
        match self {
            Self::Scalar { .. } => self.after_dash(column),
            Self::Sequence(lines) | Self::Mapping(lines) => match lines.split_first() {
                Some((first, rest)) => {
                    let lines: Vec<String> = iter::once(first.clone())
                        .chain(indent(rest, column))
                        .collect();
                    join(&lines)
                }
                None => String::new(),
            },
        }
    }

    /// The single line of a scalar that fits on one line, such as a flow
    /// value or a key.
    pub(crate) fn inline_head(&self) -> Option<&str> {
        match self {
            Self::Scalar { head, body } if body.is_empty() => Some(head),
            _ => None,
        }
    }

    /// Whether a core tag names this value's kind, so the tag can stay in
    /// front of it: `seq`, `map`, and `str` by shape, and `int`, `float`,
    /// `bool`, and `null` by what the text reads as.
    pub(crate) fn fits_core_tag(&self, name: &str) -> bool {
        match (name, self) {
            ("seq", Self::Sequence(_)) | ("map", Self::Mapping(_)) => true,
            ("seq", Self::Scalar { head, body }) => body.is_empty() && head.starts_with('['),
            ("map", Self::Scalar { head, body }) => body.is_empty() && head.starts_with('{'),
            ("str", Self::Scalar { head, body }) => !body.is_empty() || read_string(head).is_some(),
            (scalar, Self::Scalar { head, body }) if body.is_empty() => matches!(
                (scalar, read_scalar(head)),
                ("int", Some(Captured::Int))
                    | ("float", Some(Captured::Float | Captured::Int))
                    | ("bool", Some(Captured::Bool))
                    | ("null", Some(Captured::Null))
            ),
            _ => false,
        }
    }

    /// Whether the value is a block scalar with an explicit indentation
    /// indicator, which YAML measures from the key's indentation.
    pub(crate) fn has_indicator(&self) -> bool {
        matches!(self, Self::Scalar { head, .. }
            if head.starts_with(['>', '|']) && head.contains(|c: char| c.is_ascii_digit()))
    }
}

/// Serialize a value. A string is written by [`text`] with `preferred` tried
/// first; anything else is laid out by `serde-saphyr`.
pub(crate) fn value<T: Serialize + ?Sized>(
    value: &T,
    preferred: Option<Style>,
    compact: bool,
) -> Result<Rendered, Error> {
    let yaml = serialize(value, compact)?;
    let index = Index::build(&yaml)?;
    let root = index.nodes.first().ok_or_else(|| Error::Serialize {
        message: "the value serialized to nothing".to_owned(),
    })?;
    let lines = || yaml.lines().map(str::to_owned).collect();
    match root.style {
        Style::BlockSequence => Ok(Rendered::Sequence(lines())),
        Style::BlockMapping => Ok(Rendered::Mapping(lines())),
        Style::FlowSequence | Style::FlowMapping | Style::Alias => {
            Ok(Rendered::inline(yaml.trim()))
        }
        Style::Plain
        | Style::SingleQuoted
        | Style::DoubleQuoted
        | Style::Literal
        | Style::Folded => match read_string(&yaml) {
            Some(decoded) => text(&decoded, TextStyle::Auto, preferred),
            None => Ok(Rendered::inline(yaml.trim())),
        },
    }
}

/// The items of a value that serializes to a sequence of scalars that each
/// fit on one line, written for a flow sequence; `None` for any other value.
pub(crate) fn flow_items<T: Serialize + ?Sized>(value: &T) -> Result<Option<Vec<String>>, Error> {
    let yaml = serialize(value, false)?;
    let index = Index::build(&yaml)?;
    let Some(root) = index.nodes.first() else {
        return Ok(None);
    };
    match root.style {
        Style::FlowSequence if root.children.is_empty() => Ok(Some(Vec::new())),
        Style::BlockSequence => {
            let mut items = Vec::with_capacity(root.children.len());
            for &child in &root.children {
                let node = &index.nodes[child];
                let written = yaml[node.value.clone()].trim();
                let item = match node.style {
                    Style::Plain | Style::SingleQuoted | Style::DoubleQuoted => {
                        match read_string(written) {
                            Some(decoded) => text(&decoded, TextStyle::Auto, None)?
                                .inline_head()
                                .map(flow_safe),
                            None => Some(written.to_owned()),
                        }
                    }
                    _ => None,
                };
                match item {
                    Some(item) => items.push(item),
                    None => return Ok(None),
                }
            }
            Ok(Some(items))
        }
        _ => Ok(None),
    }
}

fn serialize<T: Serialize + ?Sized>(value: &T, compact: bool) -> Result<String, Error> {
    let options = serde_saphyr::ser_options! {
        indent_step: 2,
        compact_list_indent: compact,
    };
    serde_saphyr::to_string_with_options(&value, options).map_err(|error| Error::Serialize {
        message: error.to_string(),
    })
}

/// A scalar as it may appear inside `[...]` or `{...}`, where `,`, `[`, `]`,
/// `{`, and `}` end a plain scalar. A plain head holding one is quoted;
/// quoted heads and nested flow collections pass through.
pub(crate) fn flow_safe(head: &str) -> String {
    if head.starts_with(['\'', '"', '[', '{']) || !head.contains([',', '[', ']', '{', '}']) {
        head.to_owned()
    } else if head.contains('\'') {
        format!("\"{}\"", escape(head))
    } else {
        format!("'{head}'")
    }
}

/// Write a string in the first style that reads back as the same string:
/// the caller's `style`, then the node's current style, then the automatic
/// order.
pub(crate) fn text(
    value: &str,
    style: TextStyle,
    preferred: Option<Style>,
) -> Result<Rendered, Error> {
    let requested = match style {
        TextStyle::Auto => None,
        TextStyle::Folded => Some(Style::Folded),
        TextStyle::Literal => Some(Style::Literal),
    };
    let multiline = value.contains(LF).then_some(Style::Literal);
    requested
        .into_iter()
        .chain(preferred.filter(|style| style.is_scalar()))
        .chain(multiline)
        .chain([Style::Plain, Style::SingleQuoted, Style::DoubleQuoted])
        .filter_map(|candidate| scalar(value, candidate))
        .find(|rendered| reads_back(rendered, value))
        .ok_or_else(|| Error::Serialize {
            message: "no scalar style reads back as this string".to_owned(),
        })
}

fn scalar(value: &str, style: Style) -> Option<Rendered> {
    let one_line = !value.contains(LF);
    match style {
        Style::Plain if one_line && !value.is_empty() => Some(Rendered::inline(value)),
        Style::SingleQuoted if one_line => {
            let quoted = format!("'{}'", value.replace('\'', "''"));
            Some(Rendered::inline(quoted))
        }
        Style::DoubleQuoted => Some(Rendered::inline(format!("\"{}\"", escape(value)))),
        Style::Folded if one_line && !value.is_empty() => Some(Rendered::Scalar {
            head: format!(">{}-", indicator(value)),
            body: vec![value.to_owned()],
        }),
        Style::Literal if !value.is_empty() => Some(literal(value)),
        _ => None,
    }
}

/// A literal block whose chomping keeps exactly the trailing line breaks the
/// text has: `|-` for none, `|` for one, `|+` for more.
fn literal(value: &str) -> Rendered {
    let content = value.trim_end_matches(LF);
    let trailing = value.len() - content.len();
    let chomp = match trailing {
        0 => "-",
        1 => "",
        _ => "+",
    };
    let body = content
        .split(LF)
        .map(str::to_owned)
        .chain(iter::repeat_n(String::new(), trailing.saturating_sub(1)))
        .collect();
    Rendered::Scalar {
        head: format!("|{}{chomp}", indicator(value)),
        body,
    }
}

/// A block scalar needs an explicit indentation only when its text starts
/// with a space; otherwise the parser finds it.
fn indicator(value: &str) -> &'static str {
    if value.starts_with(' ') { "2" } else { "" }
}

fn escape(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for c in value.chars() {
        match c {
            '\\' => out.push_str(r"\\"),
            '"' => out.push_str(r#"\""#),
            LF => out.push_str(r"\n"),
            '\t' => out.push_str(r"\t"),
            '\r' => out.push_str(r"\r"),
            '\0' => out.push_str(r"\0"),
            c if c.is_control() || matches!(c, '\u{85}' | '\u{2028}' | '\u{2029}' | '\u{feff}') => {
                // Writing to a String cannot fail.
                let _ = write!(out, r"\u{:04X}", u32::from(c));
            }
            c => out.push(c),
        }
    }
    out
}

/// The family's reader settings: only `true` and `false` are booleans.
fn reader() -> serde_saphyr::Options {
    serde_saphyr::options! {
        strict_booleans: true,
    }
}

/// Whether the family's reader reads the rendered text back as this exact
/// string. A plain `true`, `4`, or `null` reads as another type and fails.
fn reads_back(rendered: &Rendered, expected: &str) -> bool {
    let probe = terminated(&format!("v:{}", rendered.after_key(0, false)));
    serde_saphyr::from_str_with_options::<BTreeMap<String, Captured>>(&probe, reader()).is_ok_and(
        |map| {
            map.len() == 1
                && map
                    .get("v")
                    .is_some_and(|value| matches!(value, Captured::Text(text) if text == expected))
        },
    )
}

/// The string a serialized scalar holds, or `None` when it reads as another
/// type such as a number or a boolean.
fn read_string(yaml: &str) -> Option<String> {
    match read_scalar(yaml)? {
        Captured::Text(text) => Some(text),
        Captured::Int | Captured::Float | Captured::Bool | Captured::Null => None,
    }
}

/// What a scalar reads as under the family's reader settings.
fn read_scalar(yaml: &str) -> Option<Captured> {
    serde_saphyr::from_str_with_options::<Captured>(yaml, reader()).ok()
}

/// What a scalar was read as, keeping the text of a string.
enum Captured {
    Text(String),
    Int,
    Float,
    Bool,
    Null,
}

impl<'de> Deserialize<'de> for Captured {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct Probe;

        impl Visitor<'_> for Probe {
            type Value = Captured;

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("any scalar")
            }

            fn visit_str<E: de::Error>(self, value: &str) -> Result<Captured, E> {
                Ok(Captured::Text(value.to_owned()))
            }

            fn visit_bool<E: de::Error>(self, _: bool) -> Result<Captured, E> {
                Ok(Captured::Bool)
            }

            fn visit_i64<E: de::Error>(self, _: i64) -> Result<Captured, E> {
                Ok(Captured::Int)
            }

            fn visit_u64<E: de::Error>(self, _: u64) -> Result<Captured, E> {
                Ok(Captured::Int)
            }

            fn visit_f64<E: de::Error>(self, _: f64) -> Result<Captured, E> {
                Ok(Captured::Float)
            }

            fn visit_unit<E: de::Error>(self) -> Result<Captured, E> {
                Ok(Captured::Null)
            }

            fn visit_none<E: de::Error>(self) -> Result<Captured, E> {
                Ok(Captured::Null)
            }
        }

        deserializer.deserialize_any(Probe)
    }
}
