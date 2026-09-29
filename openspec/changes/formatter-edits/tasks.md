# Tasks: edits a ledger formatter needs

## 1. Text moves

- [x] 1.1 `Document::reorder` and `Error::Repeated`. Verify: tests for both
  reorder scenarios, a repeated child, a missing child, and a dash-line
  mapping, run by `mise run verify` on the build host.
- [x] 1.2 Remove from a one-line flow sequence. Verify: first, middle, last,
  and only item, and a multi-line flow sequence refused.
- [x] 1.3 `Document::reindent` with the structure check. Verify: the
  re-indent scenario and a shift that would leave its key refused.

## 2. Value writes

- [x] 2.1 Flow-preserving `replace`. Verify: the padded-list scenario, a
  separator without a space, and a list of mappings falling back to block.
- [x] 2.2 `insert_item` and `insert_item_text`. Verify: the insert scenario.

## 3. Proof and release

- [x] 3.1 Workflow tests for the four formatter snapshots this closes, with
  value checks and marked-diff snapshots. Verify: `mise run verify`.
- [x] 3.2 A patch changeset. Verify: `.changeset/*.md` says `patch`.

## Evidence

2026-09-29. `cargo insta test --accept --all-features` on the build host: 42
edit tests, 10 index tests, 27 workflow tests, and the doc-test pass. Each
workflow edit reads its value back through `serde-saphyr`, and its inline
snapshot was reviewed line by line: section reorder, a section moved above
the comment the next key owns, a loose comment kept in its slot, a started row
moved to the front with its blank line, an archive sorted newest first, rows
re-indented from four columns to two with a literal block, blockers removed in
flow style, flow lists replaced in their own layout, and a row inserted at an
index. The tests are in [`tests/workflows.rs`](../../../tests/workflows.rs)
and [`tests/edits.rs`](../../../tests/edits.rs).
