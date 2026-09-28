//! Turning a value into the text an edit splices in.

use std::collections::BTreeMap;
use std::fmt::{self, Write as _};

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

/// What a value looks like before it is placed: the part that sits on the
/// key's or dash's line, and the lines below it, relative to column 0.
#[derive(Clone, Debug)]
pub(crate) struct Rendered {
    pub(crate) head: Option<String>,
    pub(crate) body: Vec<String>,
    pub(crate) kind: Kind,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Kind {
    Scalar,
    Sequence,
    Mapping,
}

impl Rendered {
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
        if let (Kind::Scalar, Some(head)) = (self.kind, &self.head) {
            let mut lines = vec![format!(" {head}")];
            lines.extend(indent(&self.body, body_column));
            return join(&lines);
        }
        let step = if self.kind == Kind::Sequence && compact {
            0
        } else {
            2
        };
        let mut lines = vec![String::new()];
        lines.extend(indent(&self.all_lines(), key_column + step));
        join(&lines)
    }

    /// The text after `- `, for a dash at `dash_column`.
    pub(crate) fn after_dash(&self, dash_column: usize) -> String {
        let lines = self.all_lines();
        let Some((first, rest)) = lines.split_first() else {
            return String::new();
        };
        let mut placed = vec![first.clone()];
        placed.extend(indent(rest, dash_column + 2));
        join(&placed)
    }

    fn all_lines(&self) -> Vec<String> {
        self.head
            .iter()
            .cloned()
            .chain(self.body.iter().cloned())
            .collect()
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
    match root.style {
        Style::BlockSequence | Style::BlockMapping => Ok(Rendered {
            head: None,
            body: yaml.lines().map(str::to_owned).collect(),
            kind: if root.style == Style::BlockSequence {
                Kind::Sequence
            } else {
                Kind::Mapping
            },
        }),
        Style::FlowSequence | Style::FlowMapping | Style::Alias => Ok(inline(yaml.trim())),
        Style::Plain
        | Style::SingleQuoted
        | Style::DoubleQuoted
        | Style::Literal
        | Style::Folded => match read_string(&yaml) {
            Some(decoded) => text(&decoded, TextStyle::Auto, preferred),
            None => Ok(inline(yaml.trim())),
        },
    }
}

fn inline(head: &str) -> Rendered {
    Rendered {
        head: Some(head.to_owned()),
        body: Vec::new(),
        kind: Kind::Scalar,
    }
}

/// A scalar as it may appear inside `[...]` or `{...}`, where `,`, `[`, `]`,
/// `{`, and `}` end a plain scalar. A plain head holding one is quoted;
/// quoted heads and nested flow collections pass through.
pub(crate) fn flow_safe(head: &str) -> String {
    if head.starts_with(['\'', '"', '[', '{']) || !head.contains([',', '[', ']', '{', '}']) {
        return head.to_owned();
    }
    if head.contains('\'') {
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
    let mut order: Vec<Style> = Vec::new();
    match style {
        TextStyle::Folded => order.push(Style::Folded),
        TextStyle::Literal => order.push(Style::Literal),
        TextStyle::Auto => {}
    }
    order.extend(preferred.filter(|style| style.is_scalar()));
    if value.contains(LF) {
        order.push(Style::Literal);
    }
    order.extend([Style::Plain, Style::SingleQuoted, Style::DoubleQuoted]);
    for candidate in order {
        if let Some(rendered) = scalar(value, candidate)
            && reads_back(&rendered, value)
        {
            return Ok(rendered);
        }
    }
    Err(Error::Serialize {
        message: "no scalar style reads back as this string".to_owned(),
    })
}

fn scalar(value: &str, style: Style) -> Option<Rendered> {
    match style {
        Style::Plain if !value.is_empty() && !value.contains(LF) => Some(inline(value)),
        Style::SingleQuoted if !value.contains(LF) => {
            Some(inline(&format!("'{}'", value.replace('\'', "''"))))
        }
        Style::DoubleQuoted => Some(inline(&format!("\"{}\"", escape(value)))),
        Style::Folded if !value.is_empty() && !value.contains(LF) => Some(Rendered {
            head: Some(format!(">{}-", indicator(value))),
            body: vec![value.to_owned()],
            kind: Kind::Scalar,
        }),
        Style::Literal if !value.is_empty() => {
            let content = value.trim_end_matches(LF);
            let trailing = value.len() - content.len();
            let chomp = match trailing {
                0 => "-",
                1 => "",
                _ => "+",
            };
            let mut body: Vec<String> = content.split(LF).map(str::to_owned).collect();
            body.extend(std::iter::repeat_n(
                String::new(),
                trailing.saturating_sub(1),
            ));
            Some(Rendered {
                head: Some(format!("|{}{chomp}", indicator(value))),
                body,
                kind: Kind::Scalar,
            })
        }
        _ => None,
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
                let _ = write!(out, r"\u{:04X}", u32::from(c));
            }
            c => out.push(c),
        }
    }
    out
}

/// Whether the family's reader, `serde-saphyr` with strict booleans, reads the
/// rendered text back as this exact string. A plain `true`, `4`, or `null`
/// reads as another type and fails here.
fn reads_back(rendered: &Rendered, expected: &str) -> bool {
    let probe = terminated(&format!("v:{}", rendered.after_key(0, false)));
    let options = serde_saphyr::options! {
        strict_booleans: true,
    };
    serde_saphyr::from_str_with_options::<BTreeMap<String, Captured>>(&probe, options).is_ok_and(
        |map| {
            map.len() == 1
                && map
                    .get("v")
                    .is_some_and(|v| v.0.as_deref() == Some(expected))
        },
    )
}

/// The string a serialized scalar holds, or `None` when it reads as another
/// type such as a number or a boolean.
fn read_string(yaml: &str) -> Option<String> {
    let options = serde_saphyr::options! {
        strict_booleans: true,
    };
    serde_saphyr::from_str_with_options::<Captured>(yaml, options)
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
