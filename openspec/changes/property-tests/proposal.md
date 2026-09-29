# Proposal: property and corpus tests for every edit

Queue row: YMD-003.

## Why

The edits were tested only by cases someone thought to write. A first run of
random edit sequences and of the public [YAML test suite][yaml-test-suite]
found eleven defects the hand-written tests missed, among them a replace that
dropped a value's anchor and broke every alias to it, a block scalar whose
header stayed behind when its value was replaced, and a panic on an empty
value whose span starts before its key's colon.

## What Changes

- New `tests/properties.rs`: `proptest` generates ledgers and random edit
  sequences, and checks each edit against a value model and the bytes outside
  the edited collection.
- New `tests/corpus.rs`: every input of the YAML test suite, pinned as a git
  submodule, is parsed, and each scalar yamled can address is replaced with
  its own value.
- A block scalar's range starts at its `|` or `>` header and ends at its last
  content line.
- The eleven defects are fixed; each has a test in `tests/edits.rs`.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `location-index`: block scalar ranges.
- `edits`: replace keeps properties and comments, refuses what it cannot
  write, and never panics.

## Impact

- A patch release. No public signature changes.
- Contributors run `git submodule update --init`; CI checks out submodules.
  The suite is excluded from the published crate.

[yaml-test-suite]: https://github.com/yaml/yaml-test-suite
