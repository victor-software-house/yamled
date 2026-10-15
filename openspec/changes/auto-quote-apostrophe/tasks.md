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
- [ ] 1.4 One decision function with a block or flow context and one pass
  over the bytes that encodes the reader's rules, with no parse; remove
  `flow_safe`. Verify the flow test in `tests/edits.rs` passes on the build
  host.
- [ ] 1.5 Add `Document::batch` and the skip for unchanged splices. Verify
  the batch tests in `tests/edits.rs` pass, and time a 300-item rewrite and a
  whole-ledger `qctl fmt` before and after on the build host.
- [ ] 1.6 Move serde-saphyr and granit-parser to 2.0.0 in their own
  commit. Verify `mise run verify` passes on the build host and the cost of
  one `reads_back` on the largest consumer ledger has not regressed.
- [ ] 1.7 Differential tests: a proptest generator biased toward
  indicators, colons, hashes, spaces, numerals, and non-ASCII text, every
  scalar of the YAML test suite, and every string of the ledgers in
  `YAMLED_CORPUS`, each compared with the reader for plain, literal, and
  folded, in block and flow context. Verify `cargo test --lib render` passes
  on the build host with the consumer ledgers in `YAMLED_CORPUS`, and that a
  debug build asserts the same agreement on every decision.
- [ ] 1.8 Criterion benchmarks in `benches/render.rs`: the decision per
  string, and a whole-ledger rewrite and no-op rewrite in one batch. Verify
  `cargo bench` on the build host, on the largest consumer ledger, before
  and after 1.4, and record the numbers in the pull request.

## 2. Release

- [ ] 2.1 Add a `patch` changeset under `.changeset/`. Verify that
  `mise run verify` passes on the build host and the pull request's CI is
  green.
