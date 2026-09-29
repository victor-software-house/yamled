# Tasks

- [x] 1. `take`, `put`, `remove`, `insert`, `reorder`, and `Node::owned` run
  an item's text through its kept lines. Proof: `mise run verify` on the
  build host.
- [x] 2. Each proposal case has a test that reads the values back and fails
  on `main`. Proof: the new tests in `tests/edits.rs` fail against 0.0.3 on
  the build host.

2026-09-29 (-03:00): the five new tests in `tests/edits.rs` fail against
yamled 0.0.3 on the build host and pass with this change; `mise run verify`
passes there.
