# Proposal

## Why

Automatic text style writes an apostrophe as a doubled single quote.
`render::text` tries plain, then single-quoted, then double-quoted, so the note
`it's: here` is written `'it''s: here'`.
qctl writes every ledger list item through this style, and three of its
consumer ledgers already carry nine doubled quotes; verctl holds 5 and qctl 1.
`flow_safe` already prefers double quotes for an apostrophe inside a flow
collection, so the two writers disagree today.

Queue row: YMD-011.

## What Changes

- `TextStyle::Auto` skips single quotes when the text holds an apostrophe and no
  double quote, and writes it double-quoted instead.
- Text that holds both an apostrophe and a double quote keeps today's
  single-quoted form, which needs no backslash escape.
- A replaced scalar that is already single-quoted keeps its style, as today.
- New `TextStyle::Line` writes a string on one line, in the automatic order
  without a block scalar, and ignores the node's current style. qctl needs it
  to rewrite a block list item in place as one line: `replace_text` with
  `Auto` keeps a folded item folded, and a remove and insert would drop the
  comments the item owns.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `edits`: the automatic quoting order in "Replace a value", and the one-line
  style in "Write text in a chosen block style".

## Impact

- `src/render.rs`: the automatic candidate order in `text`, and the new
  `TextStyle::Line` variant. `TextStyle` is `#[non_exhaustive]`, so the variant
  is additive.
- Callers that write through `TextStyle::Auto`, `Document::push`,
  `Document::insert`, or `Document::replace` of a plain scalar: new text with an
  apostrophe is written double-quoted. Bytes already in a file do not change.
- Released as a patch; qctl takes it for its QCTL-039.
