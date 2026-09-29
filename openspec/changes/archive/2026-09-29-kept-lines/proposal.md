# Move and edit around a block scalar that keeps its trailing lines

## Why

A block scalar with keep chomping (`|+` or `>+`) owns the blank lines after
its text: they are part of its value. yamled 0.0.3 handled that only for a
replace and for a push after such an item. Everywhere else it either refused
or corrupted:

1. `take` and `reorder` refused an item whose last block scalar keeps its
   lines, so a ledger row whose last note ends in two line breaks could no
   longer be started, archived, parked, promoted, or reordered by qctl
   ([qctl#82] review).
2. `remove` of a mapping entry after such a scalar left the removed entry's
   trailing blank lines behind, and YAML read them into the scalar above:
   `a: |+` with value `"x\n"` became `"x\n\n"`.
3. `insert` after such a scalar put the new key between its text and its
   kept lines, so the value lost them.
4. `Node::owned` left the kept lines out for an item whose last descendant
   keeps them.

This change is queue row YMD-004.

## What Changes

1. `take`, `put`, `remove`, `insert`, `reorder`, and `Node::owned` treat an
   item's text as running through the blank lines its last block scalar
   keeps.
2. A blank line that would land directly after such an item is dropped,
   since YAML would read it into the value. This is the one case where an
   edit touches bytes it does not name, and only blank lines.
3. `take` and `reorder` no longer refuse these items.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `edits`: take, reorder, remove, and insert keep a kept block scalar's value.

## Impact

1. Code: `src/document.rs`, `src/document/arrange.rs`, `src/document/values.rs`.
2. Tests: `tests/edits.rs`, with each value read back through serde-saphyr.
3. Consumers: qctl moves rows whose last note ends in line breaks.

[qctl#82]: https://github.com/victor-software-house/qctl/pull/82
