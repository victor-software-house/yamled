# Proposal: edits a ledger formatter needs

Queue row: YMD-002.

## Why

A work-queue tool's 33 accepted mutation snapshots were mapped onto the edits
in yamled 0.0.1. Twenty-five map onto them. Four capabilities are missing, and
without them the tool keeps splicing text of its own:

1. Reordering sections of a mapping, or rows of a list, with each moving with
   its comments while the blank lines between them stay where they were.
   Three `fmt` snapshots reorder sections and one sorts rows.
2. Editing a flow list such as `blocked_by: [A-1, A-2]`: removing one blocker,
   or replacing the list, turns it into a block list today.
3. Re-indenting a list whose rows were written four columns deep to the
   file's two.
4. Inserting a new row at an index. `Fragment` has no public constructor, so
   the only way is push, take the last item, and put it.

## What Changes

- New `Document::reorder`: the children it names trade places among the slots
  they hold; blank lines, loose comments, and unnamed children stay in place.
- `Document::remove` removes an item from a one-line flow sequence.
- `Document::replace` keeps a flow sequence in flow style when every new item
  fits on one line, with the list's own separator and padding.
- New `Document::reindent`: a block collection's lines shift to a new column.
- New `Document::insert_item` and `Document::insert_item_text`: a value at an
  index of a sequence, spaced like `put`.
- New `Error::Repeated` for a reorder that names a child twice.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `edits`: reorder, flow-sequence remove and replace, reindent, and insert at
  an index.

## Impact

- New public API in a patch release. `reorder`, `reindent`, and the flow
  remove move text and need no feature; `insert_item` and the flow replace
  write values and sit behind the default `serde` feature.
- Whitespace tidying (trailing spaces, runs of blank lines) stays in the
  consumer: yamled does not change bytes nobody named.
