# Proposal: edit YAML in place through a location index

Queue row: YMD-001.

## Why

Rust has no mature format-preserving YAML editor, the role `toml_edit` plays
for TOML. [qctl][qctl] needed one to stop rewriting whole ledgers: one verb
changed 641 lines of a 479-line file and deleted its schema comment. It settled
on [`yamlpath`][yamlpath] and [`yamlpatch`][yamlpatch] in
[qctl#15][qctl#15], after [`yaml-edit`][yaml-edit] 0.2.3 miscounted a seven-row
list as three and dropped the header comment ([decision note][qctl-note]).

That pair works, but qctl carries six workarounds on top of it, and the pair
pulls in about 65,000 lines of C (tree-sitter and its YAML grammar) plus
`libyaml-rs`, a C-to-Rust transpile with 236 `unsafe` sites. `yaml-edit` 0.3.2,
retested on 2026-09-28, still counts a 26-row list as 4 and writes invalid YAML
when a row moves. The ctl family is moving every YAML read to
[`serde-saphyr`][serde-saphyr]; its parser, [`granit-parser`][granit-parser],
reports byte spans for every node and comment, which is enough to edit in
place. A spike on that parser handled every qctl edit on four real ledgers
with correct values, and changed one line where qctl changes up to seven.

## What Changes

- New `Document` type: parse a YAML source into a location index of nodes,
  keys, owned comments, indentation, and style.
- New `Path` type: address a node by keys and indexes, or by a JSON pointer
  such as `/queue/3/title`, the form a JSON Schema error reports.
- New edits: replace a value, remove a key or item, insert a key, push an
  item, and take an item out and put it back elsewhere. Each is a byte splice
  that leaves every other byte as it was.
- New value writer: values are serialized through `serde-saphyr` and written
  at the file's indentation; a replaced scalar keeps its style when it can.
- Dependencies: `granit-parser`, `serde-saphyr`, `serde`. Pure Rust.

## Capabilities

### New Capabilities

- `location-index`: parsing a source into addressable nodes with locations,
  owned comments, and style.
- `edits`: format-preserving replace, remove, insert, push, take, and put.

### Modified Capabilities

None.

## Impact

- New public API in `yamled` 0.1.0.
- First consumers: qctl, to replace `yamlpath`, `yamlpatch`, and `yaml_serde`
  in its ledger edits; ctl-core, to place schema errors on their line.
- Out of scope, recorded for a later full document model: node handles that
  survive edits, incremental re-indexing, flow-collection edits beyond an empty
  list, anchors and aliases, multi-document streams, and CRLF line endings.

[qctl]: https://github.com/victor-software-house/qctl
[qctl#15]: https://github.com/victor-software-house/qctl/pull/15
[qctl-note]: https://github.com/victor-software-house/qctl/blob/8d69a9e5065038cedd6a4cca228c5ff62ef41500/tasks.yaml#L678-L691
[yamlpath]: https://crates.io/crates/yamlpath
[yamlpatch]: https://crates.io/crates/yamlpatch
[yaml-edit]: https://crates.io/crates/yaml-edit
[serde-saphyr]: https://crates.io/crates/serde-saphyr
[granit-parser]: https://crates.io/crates/granit-parser
