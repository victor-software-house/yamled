# yamled

Format-preserving YAML edits for Rust. **A library, not a command.** Import
as `yamled`.

This repo's queue is [`tasks.yaml`](tasks.yaml) (`YMD-###`).

## What this crate owns

- A location index over a YAML source: each node's byte range, its key's
  range, the comments it owns, its indentation, and its style (block or flow,
  plain, quoted, folded, or literal).
- Edits applied as byte splices: replace a value, remove a key or item with
  its own comments, insert a key or item at the file's indentation, and move a
  byte range.
- A value writer that follows the file's style, so a caller never decides
  quoting or indentation by hand.

Parsing is `granit-parser`; values are read and written through
`serde-saphyr`. Domain rules stay in the caller: which row moves where, what a
field means, and what a ledger forbids.

## Rules

1. **Every byte an edit does not name stays as it was.** A test compares whole
   files, not values alone.
2. **No edit leaves invalid YAML.** Each edit is proven by parsing the result
   back to the expected value.
3. **A comment belongs to the node below it** when no blank line separates
   them, and a comment on the same line belongs to that line's node.
4. **Spacing is kept, not guessed.** A new sequence item follows the blank
   lines the sequence already has. Only a sequence that does not settle it
   (fewer than two items, or both kinds of gap) uses the caller's `Spacing`.
5. **Pure Rust.** `unsafe_code` is forbidden here, and a dependency that
   carries C or unsafe transpiled code needs a recorded reason.

## Strings and comments

Multiline strings use `indoc!` and `formatdoc!`. No `concat!`, and no escaped
`\n` inside a document. Doc comments carry the why, on the item; no inline
`//` prose in function bodies.

## Release

A human writes `.changeset/*.md` on the same PR that ships the behaviour. Every
changeset is a `patch` bump; do not write `minor` or `major` until the operator
decides otherwise. Never hand-edit a version or `CHANGELOG.md`. Declarations
live in [`.ctl/ver.yaml`](.ctl/ver.yaml); verctl opens the Version PR, and
merging it publishes to crates.io.

## Checks

```sh
mise run verify
```

`verify` runs, in parallel:

1. Rust: rustfmt, clippy (pedantic, denied) with all features and again with
   none, nextest, doc-tests, cargo-deny licenses, bans, and sources, and
   cargo-machete.
2. Everything else: taplo for TOML (`.taplo.toml`), rumdl for Markdown
   (`.rumdl.toml`), typos for spelling, actionlint for workflows, and
   `openspec validate --all --strict`.

mise pins every tool, OpenSpec included (`npm:@fission-ai/openspec`, installed
through Bun); do not use a global copy. Advisories run on CI only
(`mise run deny:advisories`). Locally, `.miserc.toml` adds the `mbx` env, which
routes Cargo through mr-boxington.

Tests put YAML in `indoc!` blocks. A unit test binds the expected text before
the assertion (`let expected = indoc! {...}; assert_eq!(document.as_str(),
expected);`), so rustfmt keeps it readable. A test of an edit sequence goes in
[`tests/workflows.rs`](tests/workflows.rs): its `Ledger` helper reads each
edited value back through `serde-saphyr`, and an inline `insta` snapshot
records the whole file with each line marked `-`, `+`, or kept. Review a
changed snapshot with `mise run snapshots`.

## Features

`serde` (default) adds the edits that write a value (`replace`, `insert`,
`push`, `insert_item`, and their `_text` forms) and pulls `serde` and
`serde-saphyr`. Without it the crate needs `granit-parser` alone: the location
index, `remove`, `take`, `put`, `reorder`, and `reindent`. Anything new that
needs a serializer goes under `serde`; `check:features` lints the crate
without it. Dependencies are declared with the features the crate uses,
default features off.

## Git

Conventional commits. lefthook. No `--no-verify`. Branch `type/number-desc`.
Always open a PR; never push to `main`.

## Changes

Plan a behaviour or contract change as an OpenSpec change in
`openspec/changes/<name>/` before writing code. `openspec/config.yaml` holds
this repository's context and rules, and `openspec validate <name>` checks the
change. One change maps to one `tasks.yaml` row.
