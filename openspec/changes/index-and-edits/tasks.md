# Tasks: edit YAML in place through a location index

## 1. Index

- [ ] 1.1 `Path`, `Segment`, `Path::from_pointer`, and `Location` with line and
  column. Verify: unit tests for `~0`, `~1`, numeric keys, and the root pointer.
- [ ] 1.2 `Document::parse` builds the index from `granit-parser` events: value
  range, key range, style, child count, owned comment lines. Verify: tests for
  every scenario in `specs/location-index/spec.md`.
- [ ] 1.3 `locate` and `locate_nearest`. Verify: a pointer test on a ledger
  fixture names the right line and column.

## 2. Edits

- [ ] 2.1 `replace` and `replace_text` with the scalar style rule. Verify:
  scenarios in `specs/edits/spec.md` under "Replace a value" and "Write text".
- [ ] 2.2 `remove`. Verify: the neighbour-comment scenario.
- [ ] 2.3 `insert`, `insert_text`, `push`, `push_text`, including an empty flow
  list and list-style detection. Verify: the note and empty-queue scenarios.
- [ ] 2.4 `take` and `put` with blank-line separators. Verify: the
  queue-to-archive scenario.
- [ ] 2.5 Each edit parses its result and leaves the source unchanged on
  failure. Verify: a test that forces an invalid result.

## 3. Proof and release

- [ ] 3.1 Whole-file tests on ledger-shaped fixtures under `tests/fixtures/`:
  each edit's diff is exactly the lines it names. Verify: `mise run verify` on
  the build host.
- [ ] 3.2 Drop `--no-tests=warn` from the nextest task. Verify: `mise.dev.toml`
  diff.
- [ ] 3.3 A minor changeset for 0.1.0 and crate docs with a runnable example.
  Verify: `mise run test:doc` and `cargo package --locked`.
