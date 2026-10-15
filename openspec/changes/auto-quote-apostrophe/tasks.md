# Tasks

## 1. Writer

- [ ] 1.1 Skip the single-quoted candidate in the automatic order of
  `render::text` when the text holds an apostrophe and no double quote.
  Verify with the tests in 1.3 on the build host.
- [ ] 1.2 Add `TextStyle::Line`, which tries plain, quoted, and double-quoted
  only, ignoring the node's current style. Verify with the tests in 1.3.
- [ ] 1.3 Add tests in `tests/edits.rs` for each new scenario. Verify that
  `cargo test --test edits` passes on the build host.

## 2. Release

- [ ] 2.1 Add a `patch` changeset under `.changeset/`. Verify that
  `mise run verify` passes on the build host and the pull request's CI is
  green.
