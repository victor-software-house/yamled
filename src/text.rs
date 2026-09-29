//! Line arithmetic on the source. Every edit is a splice at line boundaries
//! these helpers find.

/// The line break this crate reads and writes. Sources with `\r\n` are
/// refused at parse time, so this is the only one.
pub(crate) const LF: char = '\n';

/// [`LF`] as a string, for joining and inserting.
pub(crate) const NEWLINE: &str = "\n";

/// Lines joined into text, with no break after the last one.
pub(crate) fn join(lines: &[String]) -> String {
    lines.join(NEWLINE)
}

/// The `|` or `>` header of the block scalar whose text starts at `text`.
/// The parser's span for a block scalar begins at its text, so the header is
/// found on the nearest line above that is not blank: the last token there
/// that starts with `|` or `>`, before any comment.
pub(crate) fn block_header(source: &str, text: usize) -> Option<usize> {
    let mut line = line_start(source, text);
    let own = &source[line..text];
    if own.trim().is_empty() {
        while line > 0 {
            line = line_start(source, line - 1);
            if !source[line..line_end(source, line)].trim().is_empty() {
                break;
            }
        }
    }
    let end = if own.trim().is_empty() {
        line_end(source, line)
    } else {
        text
    };
    let mut header = None;
    let mut at = line;
    for token in source[line..end].split_inclusive(char::is_whitespace) {
        let word = token.trim_end();
        if word.starts_with('#') {
            break;
        }
        if word.starts_with(['|', '>']) {
            header = Some(at);
        }
        at += token.len();
    }
    header
}

/// The non-empty parts, joined by one space.
pub(crate) fn spaced(parts: &[&str]) -> String {
    parts
        .iter()
        .copied()
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

/// A line with its break.
pub(crate) fn terminated(line: &str) -> String {
    let mut text = line.to_owned();
    text.push(LF);
    text
}

/// The byte where the line holding `at` starts.
pub(crate) fn line_start(source: &str, at: usize) -> usize {
    source[..at].rfind(LF).map_or(0, |newline| newline + 1)
}

/// The byte after the line break that ends the line holding `at`, or the end
/// of the source when that line has no break.
pub(crate) fn line_end(source: &str, at: usize) -> usize {
    source[at..]
        .find(LF)
        .map_or(source.len(), |newline| at + newline + 1)
}

/// `end` moved back over trailing whitespace, down to `start`. A block
/// scalar's span reaches into the indentation of the next line.
pub(crate) fn trim_end(source: &str, start: usize, end: usize) -> usize {
    start + source[start..end].trim_end().len()
}

/// How many characters `at` sits from the start of its line.
pub(crate) fn column(source: &str, at: usize) -> usize {
    source[line_start(source, at)..at].chars().count()
}

/// The byte of the last `-` on the line before `at`, or `at` itself: the
/// dash of the sequence item whose value starts at `at`.
pub(crate) fn dash_before(source: &str, at: usize) -> usize {
    let line = line_start(source, at);
    source[line..at]
        .rfind('-')
        .map_or(at, |offset| line + offset)
}

/// Whether the text from the start of `at`'s line up to `at` is only spaces,
/// so a splice can take whole lines.
pub(crate) fn starts_line(source: &str, at: usize) -> bool {
    source[line_start(source, at)..at].trim().is_empty()
}

/// Whether the text between two offsets holds a line with nothing on it.
pub(crate) fn has_blank_line(source: &str, from: usize, to: usize) -> bool {
    source[from..to].lines().any(|line| line.trim().is_empty())
}

/// Prefix every non-empty line with `indent` spaces.
pub(crate) fn indent(lines: &[String], indent: usize) -> Vec<String> {
    let pad = " ".repeat(indent);
    lines
        .iter()
        .map(|line| {
            if line.is_empty() {
                String::new()
            } else {
                format!("{pad}{line}")
            }
        })
        .collect()
}

/// Add `by` leading spaces to each line that is not empty.
pub(crate) fn pad(text: &str, by: usize) -> String {
    let spaces = " ".repeat(by);
    text.split_inclusive(LF)
        .map(|line| {
            if line == NEWLINE {
                line.to_owned()
            } else {
                format!("{spaces}{line}")
            }
        })
        .collect()
}

/// Remove up to `by` leading spaces from each line.
pub(crate) fn dedent(text: &str, by: usize) -> String {
    text.split_inclusive(LF)
        .map(|line| {
            let spaces = line.len() - line.trim_start_matches(' ').len();
            &line[spaces.min(by)..]
        })
        .collect()
}
