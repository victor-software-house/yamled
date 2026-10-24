# Tasks

## 1. Writer

- [x] 1.1 Add `Quotes` with `Auto`, `Single`, and `Double`, and
  `Document::with_quotes`. Thread the document's quotes through `text`,
  `value`, `flow_items`, and `flow_safe`. Verify with the tests in 1.3 on the
  build host.
  - 2026-10-15 UTC: the 1.3 tests pass at [61e4e6b] on the build host.
- [x] 1.2 Add `TextStyle::Line`, which ignores the node's current style and
  writes no block scalar. Verify with the tests in 1.3.
  - 2026-10-15 UTC: the `Line` tests in `tests/edits.rs` pass at [61e4e6b].
- [x] 1.3 Add tests in `tests/edits.rs`: one case per step of `Quotes::Auto`,
  `Single` and `Double`, every writer under each, and `Line` on block items.
  Verify that `cargo test --test edits` passes on the build host.
  - 2026-10-15 UTC: `cargo test --test edits` passes, 77 tests, at [61e4e6b].
- [x] 1.4 One decision function with a block or flow context and one pass
  over the bytes that encodes the reader's rules, with no parse; remove
  `flow_safe`. Verify the flow test in `tests/edits.rs` passes on the build
  host.
  - 2026-10-15 UTC: the flow test passes and `flow_safe` is gone at
    [61e4e6b]; the pass is `src/render/scan.rs`.
- [ ] 1.5 Add `Document::batch` and the skip for unchanged splices. Verify
  the batch tests in `tests/edits.rs` pass, and time a 300-item rewrite and a
  whole-ledger `qctl fmt` before and after on the build host.
  - 2026-10-15 UTC: the batch tests pass at [61e4e6b]; one batch rewrites
    the 270 block items of the largest consumer ledger in 5.0 ms against
    830.7 ms one by one, at [75fe8c9]. Not yet done: the `qctl fmt` timing,
    which comes with the qctl change that uses the batch.
  - 2026-10-24 UTC: the batch shipped in [PR 33][yamled#33], and qctl
    calls it since [qctl PR 118][qctl#118]. The `qctl fmt` timing was
    never recorded, so this box stays open; the change is archived
    because YMD-011's acceptance is met.
- [x] 1.6 Move serde-saphyr and granit-parser to 2.0.0 in their own
  commit. Verify `mise run verify` passes on the build host and the cost of
  one `reads_back` on the largest consumer ledger has not regressed.
  - 2026-10-15 UTC: `mise run verify` passed at [c816e0e]. On the largest
    consumer ledger, 0.99 parses per string before and after, and the
    decision over all strings took 9.7 ms on 1.3.0 and 9.6 ms on 2.0.0.
- [x] 1.7 Differential tests: a proptest generator biased toward
  indicators, colons, hashes, spaces, numerals, and non-ASCII text, every
  scalar of the YAML test suite, and every string of the ledgers in
  `YAMLED_CORPUS`, each compared with the reader for plain, literal, and
  folded, in block and flow context. Verify `cargo test --lib render` passes
  on the build host with the consumer ledgers in `YAMLED_CORPUS`, and that a
  debug build asserts the same agreement on every decision.
  - 2026-10-15 UTC: 0 disagreements at [61e4e6b] over 20,000 biased and
    5,000 arbitrary generated strings, 813 suite strings, 28,598 strings of
    18 consumer ledgers, and 82 hand-picked cases. `written` asserts the
    agreement in debug builds.
- [x] 1.8 Criterion benchmarks in `benches/render.rs`: the decision per
  string, and a whole-ledger rewrite and no-op rewrite in one batch. Verify
  `cargo bench` on the build host, on the largest consumer ledger, before
  and after 1.4, and record the numbers in the pull request.
  - 2026-10-15 UTC: criterion, [ec6e8f9] against [61e4e6b], largest
    consumer ledger: decision per string 3.9 µs to 0.37 µs (block) and
    4.5 µs to 0.40 µs (flow), rewrite 16.0 ms to 8.5 ms, no-op rewrite
    12.3 ms to 2.8 ms. Recorded in [PR 33][yamled#33].

## 2. Release

- [x] 2.1 Add a `patch` changeset under `.changeset/`. Verify that
  `mise run verify` passes on the build host and the pull request's CI is
  green.
  - 2026-10-15 UTC: `.changeset/auto-quote-apostrophe.md` is a patch;
    `mise run verify` passed at [61e4e6b] and the [CI run][ci] is green.

[61e4e6b]: https://github.com/victor-software-house/yamled/commit/61e4e6ba7ef5cf4e0f9debaf2e68543a92c58b87
[ec6e8f9]: https://github.com/victor-software-house/yamled/commit/ec6e8f9f30c5f75730a36318d6429b7355af53ed
[c816e0e]: https://github.com/victor-software-house/yamled/commit/c816e0e17e33027234459fbf87ce3927acab57e7
[75fe8c9]: https://github.com/victor-software-house/yamled/commit/75fe8c9686a85abc47247a54f429cd5d89260c59
[yamled#33]: https://github.com/victor-software-house/yamled/pull/33
[ci]: https://github.com/victor-software-house/yamled/actions/runs/38763501482
[qctl#118]: https://github.com/victor-software-house/qctl/pull/118
