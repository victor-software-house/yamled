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

/// Remove up to `by` leading spaces from each line.
pub(crate) fn dedent(text: &str, by: usize) -> String {
    text.split_inclusive(LF)
        .map(|line| {
            let spaces = line.len() - line.trim_start_matches(' ').len();
            &line[spaces.min(by)..]
        })
        .collect()
}
