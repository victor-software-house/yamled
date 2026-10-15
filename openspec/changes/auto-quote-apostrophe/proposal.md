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

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `edits`: the new requirement "A document declares its quotes", the quoting
  in "Replace a value", and the layout-only `TextStyle` in "Write text in a
  chosen block style".

## Impact

- `src/render.rs`: `Quotes`, its order in `text` and `flow_safe`, and
  `TextStyle::Line`. Both enums are `#[non_exhaustive]`.
- `src/document.rs`: the `quotes` field and `Document::with_quotes`, behind
  the `serde` feature like the writers that read it.
- Callers that write new text with an apostrophe and no double quote get
  double quotes. Bytes already in a file do not change.
- Released as a patch, 0.0.7; qctl takes it for its QCTL-039.
