//! Turning a value into the text an edit splices in.

use std::collections::BTreeMap;
use std::fmt::{self, Write as _};
use std::iter;

use serde::de::{self, Deserializer, Visitor};
use serde::{Deserialize, Serialize};

use crate::Error;
use crate::index::{Index, Style};
use crate::text::{LF, indent, join, terminated};

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
        let (first, rest, column) = match self {
            Self::Scalar { head, body } => (format!(" {head}"), body, body_column),
            Self::Sequence(lines) if compact => (String::new(), lines, key_column),
            Self::Sequence(lines) | Self::Mapping(lines) => (String::new(), lines, key_column + 2),
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

    /// The text of a root value: a collection at column 0, a scalar as after a
    /// dash at column 0.
    pub(crate) fn at_root(&self) -> String {
        match self {
            Self::Scalar { .. } => self.after_dash(0),
            Self::Sequence(lines) | Self::Mapping(lines) => join(lines),
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
    let options = serde_saphyr::ser_options! {
        indent_step: 2,
        compact_list_indent: compact,
    };
    let yaml = serde_saphyr::to_string_with_options(&value, options).map_err(|error| {
        Error::Serialize {
            message: error.to_string(),
        }
    })?;
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
                    .is_some_and(|value| value.0.as_deref() == Some(expected))
        },
    )
}

/// The string a serialized scalar holds, or `None` when it reads as another
/// type such as a number or a boolean.
fn read_string(yaml: &str) -> Option<String> {
    serde_saphyr::from_str_with_options::<Captured>(yaml, reader())
        .ok()?
        .0
}

/// A value that remembers the string it was read from, and nothing when it
/// was read as any other type.
struct Captured(Option<String>);

impl<'de> Deserialize<'de> for Captured {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct Probe;

        impl Visitor<'_> for Probe {
            type Value = Captured;

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("any scalar")
            }

            fn visit_str<E: de::Error>(self, value: &str) -> Result<Captured, E> {
                Ok(Captured(Some(value.to_owned())))
            }

            fn visit_bool<E: de::Error>(self, _: bool) -> Result<Captured, E> {
                Ok(Captured(None))
            }

            fn visit_i64<E: de::Error>(self, _: i64) -> Result<Captured, E> {
                Ok(Captured(None))
            }

            fn visit_u64<E: de::Error>(self, _: u64) -> Result<Captured, E> {
                Ok(Captured(None))
            }

            fn visit_f64<E: de::Error>(self, _: f64) -> Result<Captured, E> {
                Ok(Captured(None))
            }

            fn visit_unit<E: de::Error>(self) -> Result<Captured, E> {
                Ok(Captured(None))
            }

            fn visit_none<E: de::Error>(self) -> Result<Captured, E> {
                Ok(Captured(None))
            }
        }

        deserializer.deserialize_any(Probe)
    }
}
