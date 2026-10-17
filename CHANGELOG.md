# Changelog

## yamled 0.0.8

- `TextStyle::Line` now ignores the quotes a replaced scalar already has, as its documentation says: `'It''s here: now'` replaced in one-line style under `Quotes::Auto` reads `"It's here: now"`, and a needlessly quoted `"demo"` reads `demo`.
`TextStyle::Auto` still keeps a quoted scalar's quotes ([#37](https://github.com/victor-software-house/yamled/pull/37)).

## yamled 0.0.7

- New `Quotes`, set per document with `Document::with_quotes`, chooses the quotes every writer uses. `Quotes::Auto`, the default, writes plain text when it reads back, then single quotes for text without an apostrophe, then double quotes for text with an apostrophe and no double quote, then the form with fewer escapes. So an apostrophe is never written as a doubled single quote. `Quotes::Single` and `Quotes::Double` pick one style. A scalar already quoted keeps its quotes on replace.

`TextStyle` now chooses layout only. New `TextStyle::Line` writes a string on one line whatever the node's current style, so `replace_text` can turn a folded or literal item into one line.

One decision function writes block values and flow items alike. It chooses the style from one pass over the text by the reader's own rules, with no parse. A block scalar no longer holds a character only double quotes carry, such as U+2028. New `Document::batch` runs several edits and parses once at the end, and an edit that writes the bytes already there no longer reparses the document.

`Document::batch` accepts the caller's own error type. yamled now builds on serde-saphyr 2.0.0 and granit-parser 2.0.0 ([#33](https://github.com/victor-software-house/yamled/pull/33)).

## yamled 0.0.6

- yamled now requires Rust 1.99.0 (`rust-version`), the pinned stable it builds, lints, and tests on.

## yamled 0.0.5

- A replace refused by a tag, and a fill of an empty value tagged as something other than a sequence, now fail with `Error::TagMismatch` naming the tag, such as `!!timestamp`, instead of `Error::Unsupported`.

## yamled 0.0.4

- An item whose last block scalar keeps its trailing lines (`|+`) can be taken, put, and reordered with those lines, and removing or inserting a key next to it no longer changes its value.

## yamled 0.0.3

- An item added after one whose block scalar keeps its trailing lines (`|+`) goes after those lines, so the earlier item keeps its value.

## yamled 0.0.2

- Reorder a block collection's children through fixed slots, re-indent a block collection, remove from and replace a one-line flow sequence in flow style, and insert a value at an index of a sequence.
- Fix eleven edit defects found by new property and YAML test suite tests: a replace keeps anchors, tags, and key-line comments; block scalars are replaced with their header and the trailing lines they own; empty values, explicit keys, and lists written at their key's column are handled or refused instead of corrupted; no edit panics.

## yamled 0.0.1

- Edit a YAML file in place: address a node by path or JSON pointer, then replace, remove, insert, push, take, or put values while every byte you did not name, comments included, stays as it was.

