# Tasks: property and corpus tests for every edit

## 1. Tests

- [x] 1.1 `tests/properties.rs` with `proptest` (default features off, `std`
  on), 256 cases of up to seven edits. Verify: `mise run verify` on the build
  host.
- [x] 1.2 `tests/corpus.rs` over the pinned suite, with the coverage floor.
  Verify: the run prints its tally and passes.

## 2. Fixes the tests found

- [x] 2.1 Each fix has a test in `tests/edits.rs`. Verify: `mise run verify`.

## 3. Proof and release

- [x] 3.1 CI checks out submodules; the suite is excluded from lints and the
  package. Verify: CI Verify on the pull request.
- [x] 3.2 A patch changeset. Verify: `.changeset/*.md` says `patch`.

## Evidence

2026-09-29. On the build host, 90 tests pass. The property test ran 256 cases.
The corpus run: 402 inputs, 112 refused at parse, 27 not readable as JSON,
534 scalars replaced with their value and outside bytes kept, 23 refused as
unsupported. Defects found and fixed, each with a test in
[`tests/edits.rs`](../../../tests/edits.rs):

1. A replace dropped the value's anchor or tag.
2. A replace dropped the key-line comment, or carried it into a block scalar.
3. Rows put into a list written at its key's column were indented wrong.
4. A key with no `:` was reported as `Invalid` instead of refused.
5. A key with no value in a flow mapping got its new value glued to the key.
6. A block scalar's header stayed behind when its value was replaced.
7. An item's dash was looked for on a block scalar's text line.
8. A value whose span starts before the colon panicked.
9. An explicit key found a `:` belonging to another entry.
10. An empty document took a value before its `---`.
11. A `|+` scalar, an empty block scalar, or a line of deeper spaces changed
    the value's trailing lines when replaced.

The corpus tally is from [`tests/corpus.rs`](../../../tests/corpus.rs) at
suite commit [`6ad3d2c`][suite-pin].

[suite-pin]: https://github.com/yaml/yaml-test-suite/tree/6ad3d2c62885d82fc349026c136ef560838fdf3d
