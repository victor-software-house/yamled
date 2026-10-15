# Tasks

## 1. Writer

- [ ] 1.1 Add `Quotes` with `Auto`, `Single`, and `Double`, and
  `Document::with_quotes`. Thread the document's quotes through `text`,
  `value`, `flow_items`, and `flow_safe`. Verify with the tests in 1.3 on the
  build host.
- [ ] 1.2 Add `TextStyle::Line`, which ignores the node's current style and
  writes no block scalar. Verify with the tests in 1.3.
- [ ] 1.3 Add tests in `tests/edits.rs`: one case per step of `Quotes::Auto`,
  `Single` and `Double`, every writer under each, and `Line` on block items.
  Verify that `cargo test --test edits` passes on the build host.

## 2. Release

- [ ] 2.1 Add a `patch` changeset under `.changeset/`. Verify that
  `mise run verify` passes on the build host and the pull request's CI is
  green.
