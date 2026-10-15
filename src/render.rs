//! Turning a value into the text an edit splices in.

use std::collections::BTreeMap;
use std::fmt::{self, Write as _};
use std::iter;

use serde::de::{self, Deserializer, Visitor};
use serde::{Deserialize, Serialize};

use crate::Error;
use crate::index::{Index, Style};
use crate::text::{LF, indent, join, spaced, terminated};

mod scan;
#[cfg(test)]
mod tests;

use scan::Scan;

/// The layout a string is written in: block or one line. Which quotes a
/// one-line string gets is the document's [`Quotes`], not this.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[non_exhaustive]
pub enum TextStyle {
    /// The node's current style when it can hold the text, then a literal
    /// block for text with line breaks, then one line as [`Self::Line`]
    /// writes it.
    #[default]
    Auto,
    /// A folded block, `>-`, for one long line. Falls back to [`Self::Auto`]
    /// for text with line breaks.
    Folded,
    /// A literal block, `|-`, which keeps every line break.
    Literal,
    /// One line: plain when it reads back unchanged, else quoted as the
    /// document's [`Quotes`] say, with `\n` escapes for line breaks. It
    /// ignores the node's current style, so what it writes depends only on
    /// the text and the document, and replacing a block scalar leaves one
    /// line.
    Line,
}

/// Which quotes a one-line string gets when it cannot be plain. A document
/// sets it once, with [`Document::with_quotes`](crate::Document::with_quotes),
/// and every writer follows it. Every variant writes plain text when it reads
/// back unchanged, and double quotes for a character only they can carry,
/// such as a control character or a line break. A replaced node that is
/// already quoted keeps its quotes under [`TextStyle::Auto`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[non_exhaustive]
pub enum Quotes {
    /// yamled chooses, in this order:
    ///
    /// 1. Plain, when the text reads back as itself unquoted.
    /// 2. Single quotes, when the text holds no `'`.
    /// 3. Double quotes, when it holds `'` and no `"`, so it is never written
    ///    with a doubled `''`.
    /// 4. Holding both, the form with fewer escapes; a tie goes to single.
    /// 5. Double quotes for a character only they can carry, whatever the steps
    ///    above chose.
    #[default]
    Auto,
    /// Single quotes when the text cannot be plain.
    Single,
    /// Double quotes when the text cannot be plain.
    Double,
}

impl Quotes {
    /// The quotes a string with these counts gets when it cannot be plain.
    fn style(self, scan: Scan) -> Style {
        let prefers_single = match self {
            Self::Auto => {
                scan.apostrophes == 0
                    || (scan.quotes > 0 && scan.apostrophes <= scan.quotes + scan.backslashes)
            }
            Self::Single => true,
            Self::Double => false,
        };
        if prefers_single && !scan.double_only() {
            Style::SingleQuoted
        } else {
            Style::DoubleQuoted
        }
    }
}

/// Where a scalar is written: as a block value, or inside `[...]` or
/// `{...}`, where `,`, `[`, `]`, `{`, and `}` also end a plain scalar and no
/// block scalar fits.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Context {
    Block,
    Flow,
}

/// A character a single-quoted or plain scalar cannot hold on one line: a
/// line break or another non-printable character. A tab is printable to
/// YAML.
fn double_only(c: char) -> bool {
    (c != '\t' && c.is_control())
        || matches!(
            c,
            '\u{2028}' | '\u{2029}' | '\u{feff}' | '\u{fffe}' | '\u{ffff}'
        )
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
            Self::Scalar { .. } | Self::Sequence(_) | Self::Mapping(_) => None,
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
    quotes: Quotes,
    context: Context,
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
            Some(decoded) => text(&decoded, TextStyle::Auto, quotes, preferred, context),
            None => Ok(Rendered::inline(yaml.trim())),
        },
    }
}

/// The items of a value that serializes to a sequence of scalars that each
/// fit on one line, written for a flow sequence; `None` for any other value.
pub(crate) fn flow_items<T: Serialize + ?Sized>(
    value: &T,
    quotes: Quotes,
) -> Result<Option<Vec<String>>, Error> {
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
                            Some(decoded) => {
                                text(&decoded, TextStyle::Line, quotes, None, Context::Flow)?
                                    .inline_head()
                                    .map(str::to_owned)
                            }
                            None => Some(written.to_owned()),
                        }
                    }
                    Style::Literal
                    | Style::Folded
                    | Style::Alias
                    | Style::BlockSequence
                    | Style::FlowSequence
                    | Style::BlockMapping
                    | Style::FlowMapping => None,
                };
                match item {
                    Some(item) => items.push(item),
                    None => return Ok(None),
                }
            }
            Ok(Some(items))
        }
        Style::Plain
        | Style::SingleQuoted
        | Style::DoubleQuoted
        | Style::Literal
        | Style::Folded
        | Style::Alias
        | Style::FlowSequence
        | Style::BlockMapping
        | Style::FlowMapping => Ok(None),
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

/// Write a string in the first style that holds it: the caller's `style`,
/// then the node's current style, then a literal block for text with line
/// breaks, then plain, then the quotes `quotes` picks, then double quotes,
/// which hold any string. [`TextStyle::Line`] and a flow `context` skip the
/// block styles, so only one-line styles remain.
///
/// One pass over the bytes decides which styles hold the string, by the
/// reader's own rules, so nothing is parsed. A debug build parses every
/// candidate it weighs and asserts the reader agrees.
pub(crate) fn text(
    value: &str,
    style: TextStyle,
    quotes: Quotes,
    preferred: Option<Style>,
    context: Context,
) -> Result<Rendered, Error> {
    let blocks = style != TextStyle::Line && context == Context::Block;
    let requested = match style {
        TextStyle::Auto | TextStyle::Line => None,
        TextStyle::Folded => Some(Style::Folded),
        TextStyle::Literal => Some(Style::Literal),
    };
    let block = |style: &Style| blocks || !matches!(style, Style::Literal | Style::Folded);
    let preferred = preferred.filter(|style| style.is_scalar() && block(style));
    let multiline = (blocks && value.contains(LF)).then_some(Style::Literal);
    let scan = Scan::of(value, context);
    requested
        .filter(block)
        .into_iter()
        .chain(preferred)
        .chain(multiline)
        .chain([Style::Plain, quotes.style(scan), Style::DoubleQuoted])
        .find_map(|candidate| written(value, candidate, scan, context))
        .ok_or_else(|| Error::Serialize {
            message: "no scalar style reads back as this string".to_owned(),
        })
}

/// The one line every writer writes for `value` in a block or flow context.
#[must_use]
pub fn line(value: &str, quotes: Quotes, flow: bool) -> String {
    let context = if flow { Context::Flow } else { Context::Block };
    text(value, TextStyle::Line, quotes, None, context)
        .ok()
        .and_then(|rendered| rendered.inline_head().map(str::to_owned))
        .unwrap_or_default()
}

/// `value` in `style` when the scan says that style holds it.
fn written(value: &str, style: Style, scan: Scan, context: Context) -> Option<Rendered> {
    let holds = match style {
        Style::Plain => scan.plain,
        Style::SingleQuoted => !scan.double_only(),
        Style::DoubleQuoted => true,
        Style::Literal => scan.literal,
        Style::Folded => scan.folded,
        Style::Alias
        | Style::BlockSequence
        | Style::FlowSequence
        | Style::BlockMapping
        | Style::FlowMapping => false,
    };
    debug_assert_eq!(
        holds,
        proven(value, style, context),
        "{style:?} in {context:?} for {value:?}"
    );
    if !holds {
        return None;
    }
    laid_out(value, style)
}

/// Whether the reader reads `value` written in `style` back as itself. Only
/// double quotes may carry a character [`double_only`] names, and a block
/// scalar may carry a line break. The oracle for the debug assertion in
/// [`written`] and for the tests.
fn proven(value: &str, style: Style, context: Context) -> bool {
    let escaped = |c: char| double_only(c) && !(style == Style::Literal && c == LF);
    let carried = style == Style::DoubleQuoted || !value.chars().any(escaped);
    let block = matches!(style, Style::Literal | Style::Folded);
    carried
        && !(block && context == Context::Flow)
        && laid_out(value, style).is_some_and(|rendered| reads_back(&rendered, value, context))
}

/// `value` laid out in `style`, when that layout can carry it at all: plain
/// and folded text on one line, and no block scalar for empty text.
fn laid_out(value: &str, style: Style) -> Option<Rendered> {
    let one_line = !value.contains(LF);
    match style {
        Style::Plain if one_line && !value.is_empty() => Some(Rendered::inline(value)),
        Style::SingleQuoted => Some(Rendered::inline(single(value))),
        Style::DoubleQuoted => Some(Rendered::inline(double(value))),
        Style::Folded if one_line && !value.is_empty() => Some(Rendered::Scalar {
            head: format!(">{}-", indicator(value)),
            body: vec![value.to_owned()],
        }),
        Style::Literal if !value.is_empty() => Some(literal(value)),
        Style::Plain
        | Style::Literal
        | Style::Folded
        | Style::Alias
        | Style::BlockSequence
        | Style::FlowSequence
        | Style::BlockMapping
        | Style::FlowMapping => None,
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

/// `value` single-quoted, each apostrophe doubled.
fn single(value: &str) -> String {
    format!("'{}'", value.replace('\'', "''"))
}

/// `value` double-quoted, with backslash escapes.
fn double(value: &str) -> String {
    format!("\"{}\"", escape(value))
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
            c if double_only(c) => {
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
fn reads_back(rendered: &Rendered, expected: &str, context: Context) -> bool {
    match context {
        Context::Block => {
            let probe = terminated(&format!("v:{}", rendered.after_key(0, false)));
            serde_saphyr::from_str_with_options::<BTreeMap<String, Captured>>(&probe, reader())
                .is_ok_and(|map| {
                    map.len() == 1
                        && map.get("v").is_some_and(
                            |value| matches!(value, Captured::Text(text) if text == expected),
                        )
                })
        }
        Context::Flow => rendered.inline_head().is_some_and(|head| {
            let probe = terminated(&format!("v: [{head}]"));
            serde_saphyr::from_str_with_options::<BTreeMap<String, Vec<Captured>>>(&probe, reader())
                .is_ok_and(|map| {
                    map.len() == 1
                        && matches!(
                            map.get("v").map(Vec::as_slice),
                            Some([Captured::Text(text)]) if text == expected
                        )
                })
        }),
    }
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
