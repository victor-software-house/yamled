# Proposal

## Why

yamled decides quoting inside its writer, and a caller cannot change it.
`render::text` tries plain, then single-quoted, then double-quoted, so the note
`it's: here` is written `'it''s: here'`.
qctl writes every ledger list item through this writer, and three of its
consumer ledgers already carry nine doubled quotes; verctl holds 5 and qctl 1.
`flow_safe` already prefers double quotes for an apostrophe inside a flow
collection, so the two writers disagree today.
`TextStyle` also mixes layout and quoting: its `Auto` variant carries the
quoting order, so no caller can ask for one line without a block, or pick
quotes, as separate choices.

Queue row: YMD-011.

## What Changes

- New `Quotes` enum, set once per document with `Document::with_quotes`, the
  way `Document::with_spacing` sets spacing. Every writer follows it: the
  `*_text` forms, `replace`, `insert`, `push`, and flow sequence items.
- `Quotes::Auto`, the default, is an explicit order: plain when the text reads
  back unquoted, else single quotes when it holds no `'`, else double quotes
  when it holds no `"`, else the form with fewer escapes, ties to single.
  `Quotes::Single` and `Quotes::Double` pick one quote style. All three write
  double quotes for a character only they can carry.
- `TextStyle` chooses layout only. New `TextStyle::Line` writes one line and
  ignores the node's current style. qctl needs it to rewrite a block list item
  in place as one line: `replace_text` with `Auto` keeps a folded item folded,
  and a remove and insert would drop the comments the item owns.
- A replaced scalar that is already quoted keeps its quotes under
  `TextStyle::Auto`, as today.
- One decision function serves block values and flow items, with the context
  as a parameter; `flow_safe` and its separate rule go.
- The style is chosen from one pass over the text's bytes, with no parse and
  no serialization. The pass encodes the family reader's own rules:
  granit-parser's plain-scalar scanner, and serde-saphyr's resolution of a
  plain scalar with strict booleans. Measured on the largest consumer ledger
  (3089 strings), the round trip that proved plain text cost 3.1 µs per
  string; the operator asked for nanoseconds. The reader stays the oracle in
  differential tests and in debug assertions only.
- `Document::batch` runs several edits and parses once at the end; a splice
  that writes the bytes already there costs no parse. Measured on the largest
  consumer ledger (3089 strings), one parse of a scalar takes 3.3 µs, and a
  reparse of the whole document after each splice takes 2.9 ms, so a
  300-item rewrite spent about 0.9 s reparsing.
- serde-saphyr's own `is_plain_value_safe` is `pub(crate)` and stricter than
  the reader: it quotes `inf` and `nan`, which the reader keeps as strings.
  yamled learns from it, with attribution, and follows the reader instead.
- A block scalar no longer holds a character only double quotes carry, such
  as U+2028. Text holding one is double-quoted, as plain and single-quoted
  text already was.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `edits`: the new requirements "A document declares its quotes" and
  "Choosing a style parses nothing", the quoting
  in "Replace a value", and the layout-only `TextStyle` in "Write text in a
  chosen block style".

## Impact

- `src/render.rs`: `Quotes`, its order in `text`, and `TextStyle::Line`.
  Both enums are `#[non_exhaustive]`. `src/render/scan.rs`: the one-pass
  decision, with differential tests against the reader in
  `src/render/tests.rs`.
- `benches/render.rs`: criterion benchmarks of the decision per string and of
  a whole-ledger rewrite. criterion 0.8.2 is a new dev-dependency, without
  its default plotting features. The benchmarks reach the decision through a
  `#[doc(hidden)]` `yamled::bench` module that is not part of the API.
- `src/document.rs`: the `quotes` field and `Document::with_quotes`, behind
  the `serde` feature like the writers that read it; `Document::batch`, and
  the skip for a splice that changes nothing.
- Callers that write new text with an apostrophe and no double quote get
  double quotes. Bytes already in a file do not change.
- `Cargo.toml`: serde-saphyr 1.3.0 to 2.0.0 and granit-parser 1.3.0 to
  2.0.0, in one commit, so consumers take one yamled bump. yamled keeps
  granit-parser's default features, because the index reads its comment
  events. None of 2.0.0's breaking changes reaches yamled's code: it builds no
  `Error::AliasError`, constructs no `Span`, and reads the end span only of
  flow collections, not of the indentless sequence whose span changed.
- Released as a patch, 0.0.7; qctl takes it for its QCTL-039.
