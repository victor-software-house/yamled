# Tasks: edit YAML in place through a location index

## 1. Index

- [x] 1.1 `Path`, `Segment`, `Path::from_pointer`, and `Location` with line and
  column. Verify: unit tests for `~0`, `~1`, numeric keys, and the root pointer.
- [x] 1.2 `Document::parse` builds the index from `granit-parser` events: value
  range, key range, style, child count, owned comment lines. Verify: tests for
  every scenario in `specs/location-index/spec.md`.
- [x] 1.3 `locate` and `locate_nearest`. Verify: a pointer test on a ledger
  fixture names the right line and column.

## 2. Edits

- [x] 2.1 `replace` and `replace_text` with the scalar style rule. Verify:
  scenarios in `specs/edits/spec.md` under "Replace a value" and "Write text".
- [x] 2.2 `remove`. Verify: the neighbour-comment scenario.
- [x] 2.3 `insert`, `insert_text`, `push`, `push_text`, including an empty flow
  list and list-style detection. Verify: the note and empty-queue scenarios.
- [x] 2.4 `take` and `put` with blank-line separators. Verify: the
  queue-to-archive scenario.
- [x] 2.5 Each edit parses its result and leaves the source unchanged on
  failure. Verify: a test that forces an invalid result.

## 3. Proof and release

- [x] 3.1 Whole-file tests on ledger-shaped fixtures under `tests/fixtures/`:
  each edit's diff is exactly the lines it names. Verify: `mise run verify` on
  the build host.
- [x] 3.2 Drop `--no-tests=warn` from the nextest task. Verify: `mise.dev.toml`
  diff.
- [x] 3.3 A minor changeset for 0.1.0 and crate docs with a runnable example.
  Verify: `mise run test:doc` and `cargo package --locked`.

## Evidence

2026-09-28T23:38Z. `mise run verify` passed on the build host through mbx:
format, clippy with `-D warnings`, `cargo check --no-default-features`,
nextest (29 passed), doc-tests (1 passed), cargo-deny licenses, bans, and
sources, cargo-machete, and `openspec validate --all --strict`.
`cargo package --locked` built the package. The tests are in
[`tests/index.rs`](../../../tests/index.rs) and
[`tests/edits.rs`](../../../tests/edits.rs); the ledger fixture is a copy of
the public [ctl-core ledger][ctl-core-ledger].

Not verified: CI on GitHub, which runs on the pull request.

`deny.toml` now allows `BSD-3-Clause`, for `encoding_rs`, which
`serde-saphyr`'s `deserialize` feature pulls in through `encoding_rs_io`; the
same allowance is in [forkctl's policy][forkctl-deny].

[ctl-core-ledger]: https://github.com/victor-software-house/ctl-core/blob/65eecc4450a1b522c4db4cba47055214cf3b0296/tasks.yaml
[forkctl-deny]: https://github.com/victor-software-house/forkctl/blob/9f9ec4f767e174c83a6a582deac4577dfa511a67/deny.toml#L12
