//! One pass over a string's bytes that decides how it can be written, with
//! no parse.
//!
//! The rules are the family reader's: granit-parser 2.0.0's plain-scalar
//! scanner, and serde-saphyr 2.0.0's resolution of a plain scalar with
//! `strict_booleans`. serde-saphyr's own `is_plain_value_safe` (MIT OR
//! Apache-2.0) showed the shape. These rules depart from it where it quotes
//! text this reader keeps as a string, such as `inf`. The differential tests
//! in `render::tests` hold every rule to the reader.

use super::Context;

/// What one pass over a string finds: the escapes each quoted form needs,
/// and which unquoted forms read back as the string.
#[derive(Clone, Copy, Debug, Default)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "each flag is a separate finding of the one pass"
)]
pub(super) struct Scan {
    /// Each one is doubled inside single quotes.
    pub(super) apostrophes: usize,
    /// Each one is escaped inside double quotes.
    pub(super) quotes: usize,
    /// Backslashes and tabs, each escaped inside double quotes.
    pub(super) backslashes: usize,
    /// A line break.
    pub(super) breaks: bool,
    /// A character other than a line break that only double quotes carry:
    /// a control character, U+2028, U+2029, U+FEFF, U+FFFE, or U+FFFF.
    pub(super) escapes: bool,
    /// The reader reads the text back unquoted as this string.
    pub(super) plain: bool,
    /// A literal block, laid out as `render::literal` does, reads back as
    /// this string.
    pub(super) literal: bool,
    /// A one-line folded block reads back as this string.
    pub(super) folded: bool,
}

impl Scan {
    /// Scan `value` as it would be written in `context`.
    pub(super) fn of(value: &str, context: Context) -> Self {
        let bytes = value.as_bytes();
        let flow = context == Context::Flow;
        let mut scan = Self::default();
        let mut syntax = starts_plain(bytes, flow) && !matches!(bytes.last(), Some(b' ' | b'\t'));
        for (at, &byte) in bytes.iter().enumerate() {
            if !NOTABLE[usize::from(byte)] {
                continue;
            }
            match byte {
                b'\'' => scan.apostrophes += 1,
                b'"' => scan.quotes += 1,
                b'\\' | b'\t' => scan.backslashes += 1,
                b'\n' => scan.breaks = true,
                b':' if bytes.get(at + 1).is_none_or(|&next| ends_plain(next, flow)) => {
                    syntax = false;
                }
                b'#' if at > 0 && matches!(bytes[at - 1], b' ' | b'\t') => syntax = false,
                _ if flow && is_flow(byte) => syntax = false,
                0x00..=0x1f | 0x7f => scan.escapes = true,
                0x80.. => scan.escapes |= escaped(&bytes[at..]),
                _ => {}
            }
        }
        let carried = !scan.escapes;
        scan.plain = syntax && carried && !scan.breaks && reads_as_string(value);
        scan.folded = carried && !scan.breaks && !bytes.is_empty();
        scan.literal = carried && literal_reads_back(bytes);
        scan
    }

    /// Whether only double quotes can hold the text on one line.
    pub(super) fn double_only(self) -> bool {
        self.breaks || self.escapes
    }
}

/// The bytes the pass acts on. Every other byte is ordinary text, skipped
/// with one lookup.
const NOTABLE: [bool; 256] = {
    let mut table = [false; 256];
    let mut byte = 0;
    while byte < table.len() {
        table[byte] = byte < 0x20 || byte >= 0x7f;
        byte += 1;
    }
    let marks = b"'\"\\:#,[]{}";
    let mut at = 0;
    while at < marks.len() {
        table[marks[at] as usize] = true;
        at += 1;
    }
    table
};

/// The indicators that cannot start a plain scalar.
const INDICATORS: &[u8] = b",[]{}#&*!|>'\"%@`";

/// Whether the text can start a plain scalar: not with a space, a tab, or an
/// indicator, and with `-`, `?`, or `:` only before a character that is not
/// a space or tab. In flow context a lone `?` is plain too, as in `[?]`.
fn starts_plain(bytes: &[u8], flow: bool) -> bool {
    match bytes {
        [] => false,
        [b'?'] => flow,
        [b'-' | b'?' | b':', rest @ ..] => !matches!(rest.first(), None | Some(b' ' | b'\t')),
        [first, ..] => !matches!(first, b' ' | b'\t') && !INDICATORS.contains(first),
    }
}

/// Whether a `:` before `next` ends a plain scalar: before a space or tab,
/// and in flow context before `,`, `[`, `]`, `{`, or `}`.
fn ends_plain(next: u8, flow: bool) -> bool {
    matches!(next, b' ' | b'\t') || (flow && is_flow(next))
}

/// The characters that end a plain scalar inside `[...]` or `{...}`.
fn is_flow(byte: u8) -> bool {
    matches!(byte, b',' | b'[' | b']' | b'{' | b'}')
}

/// Whether the UTF-8 character starting `bytes` is one only double quotes
/// carry: a C1 control (U+0080 to U+009F), U+2028, U+2029, U+FEFF, U+FFFE,
/// or U+FFFF. ASCII controls are matched before this.
fn escaped(bytes: &[u8]) -> bool {
    matches!(
        bytes,
        [0xc2, 0x80..=0x9f, ..]
            | [0xe2, 0x80, 0xa8 | 0xa9, ..]
            | [0xef, 0xbb, 0xbf, ..]
            | [0xef, 0xbf, 0xbe | 0xbf, ..]
    )
}

/// Whether a literal block reads back as text whose characters it can hold.
/// The scanner measures the indentation from the first line that is not
/// empty, so that line may start with a space only when an indentation
/// indicator is written, which happens when the text itself starts with a
/// space. Text of line breaks alone keeps them only with `|+`, at two or
/// more.
fn literal_reads_back(bytes: &[u8]) -> bool {
    match bytes.iter().position(|&byte| byte != b'\n') {
        Some(first) => first == 0 || bytes[first] != b' ',
        None => bytes.len() >= 2,
    }
}

/// Whether the reader resolves plain text to a string: not `~`, `null`,
/// `true`, or `false` in any case, and not a number.
fn reads_as_string(text: &str) -> bool {
    let bytes = text.as_bytes();
    match bytes {
        b"~" => false,
        [b'n' | b'N', ..] => !bytes.eq_ignore_ascii_case(b"null"),
        [b't' | b'T', ..] => !bytes.eq_ignore_ascii_case(b"true"),
        [b'f' | b'F', ..] => !bytes.eq_ignore_ascii_case(b"false"),
        [b'0'..=b'9' | b'+' | b'-' | b'.', ..] => !number(text),
        _ => true,
    }
}

/// Whether the reader resolves plain text to an integer or a float.
///
/// An integer has an optional sign, then decimal digits with no leading
/// zero, or `0x`, `0o`, or `0b` and digits of that radix, with single
/// underscores between digits, and fits `u64`, or `i64` with a `-`. A
/// float is `.inf` or `.nan` with an optional sign, in any case, or a
/// numeral Rust's `f64` parser accepts. A numeral that overflows `f64` is
/// still a float when a digit leads it; one that starts at the dot is a
/// string, so only that rare shape is parsed to find out.
fn number(text: &str) -> bool {
    let bytes = text.as_bytes();
    let (negative, unsigned) = match bytes {
        [b'-', rest @ ..] => (true, rest),
        [b'+', rest @ ..] => (false, rest),
        _ => (false, bytes),
    };
    if unsigned.eq_ignore_ascii_case(b".inf") || unsigned.eq_ignore_ascii_case(b".nan") {
        return true;
    }
    if let [b'0', radix, digits @ ..] = unsigned {
        let radix = match radix {
            b'x' | b'X' => Some(16),
            b'o' | b'O' => Some(8),
            b'b' | b'B' => Some(2),
            _ => None,
        };
        if let Some(radix) = radix {
            return integer(digits, radix, negative);
        }
    }
    let leading_zero = unsigned.len() > 1 && unsigned[0] == b'0';
    (!leading_zero && integer(unsigned, 10, negative)) || float(text, unsigned)
}

/// Whether `digits` in `radix` is an integer that fits: at least one digit,
/// and each underscore between two digits.
fn integer(digits: &[u8], radix: u32, negative: bool) -> bool {
    let limit = if negative { 1 << 63 } else { u64::MAX };
    let mut value: u64 = 0;
    let mut any = false;
    for (at, &byte) in digits.iter().enumerate() {
        if byte == b'_' {
            let between = at > 0
                && digits[at - 1] != b'_'
                && digits.get(at + 1).is_some_and(|&next| next != b'_');
            if !between {
                return false;
            }
            continue;
        }
        let Some(digit) = char::from(byte).to_digit(radix) else {
            return false;
        };
        let Some(next) = value
            .checked_mul(u64::from(radix))
            .and_then(|value| value.checked_add(u64::from(digit)))
            .filter(|&value| value <= limit)
        else {
            return false;
        };
        value = next;
        any = true;
    }
    any
}

/// Whether `text` is a numeral Rust's `f64` parser accepts and the reader
/// keeps as a float: `digits[.digits]` or `.digits`, then an optional
/// exponent, after an optional sign. `unsigned` is `text` without its sign.
fn float(text: &str, unsigned: &[u8]) -> bool {
    let digits = |bytes: &[u8]| {
        bytes
            .iter()
            .take_while(|byte| byte.is_ascii_digit())
            .count()
    };
    let whole = digits(unsigned);
    let mut rest = &unsigned[whole..];
    let mut fraction = 0;
    if let [b'.', tail @ ..] = rest {
        fraction = digits(tail);
        rest = &tail[fraction..];
    }
    if whole + fraction == 0 {
        return false;
    }
    let [b'e' | b'E', tail @ ..] = rest else {
        return rest.is_empty();
    };
    let tail = match tail {
        [b'+' | b'-', rest @ ..] => rest,
        _ => tail,
    };
    let exponent = digits(tail);
    exponent > 0
        && exponent == tail.len()
        && (whole > 0 || text.parse::<f64>().is_ok_and(f64::is_finite))
}
