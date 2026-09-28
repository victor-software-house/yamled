# Design: edit YAML in place through a location index

## Context

The edits qctl performs on a ledger are few and known: replace a scalar,
replace or extend a list, remove a key, add a key, and move a row between
lists. qctl uses only `query_exact`, `query_key_only`, and `query_exists` from
[`yamlpath`][yamlpath] and only `Op::Replace` and `Op::Remove` from
[`yamlpatch`][yamlpatch], and splices bytes itself for everything else. A spike
confirmed that [`granit-parser`][granit-parser] 1.3.0 gives what an index
needs: byte offsets for `&str` input, block or flow style for collections,
scalar style including folded and literal, and `Comment` events with an
`Above` or `Right` placement, where an `Above` comment precedes the node it
belongs to.

## Goals and non-goals

1. Goal: the edits in the specs, each changing only the bytes it names.
2. Goal: a location for any path, including the nearest ancestor of a missing
   one, so a JSON Schema error can be put on its line.
3. Non-goal: a full editable tree like `toml_edit`. The gaps are listed in the
   proposal and ship later.

## Decisions

### 1. Rebuild the index after each edit

Each edit splices the source and parses it again. A ledger of 60 KB parses in
under 1 ms, and the parse doubles as the validity check the spec requires.
Considered: updating offsets in place. It lost because a wrong offset corrupts
a file silently, and the speed is not needed yet.

### 2. Paths, not handles

A node is addressed by `Path` (keys and indexes), resolved against the current
index on every call. Considered: handles that point into the index. They lost
because every edit invalidates them; the full document model will add them with
incremental offsets.

### 3. Block collections end at their last value

`granit-parser` ends a block collection where the next token starts, after any
blank lines and comments. The index ends a node at the end of its last value,
so trailing blank lines and a following node's comment are never inside it. A
folded or literal scalar's span includes trailing whitespace, which the index
trims.

### 4. Comment ownership comes from the parser

A comment line directly above a node with no blank line between belongs to the
node; a comment on the node's last line belongs to it. The parser identifies
which lines are comments, so text inside a block scalar that starts with `#` is
never mistaken for one. Considered: tree-sitter's attribution, which gives a
comment to the node above it. It lost because it strands a comment on a move,
which is the workaround qctl carries today.

### 5. Values go through `serde-saphyr`

A compound value is serialized with `serde-saphyr` (indent step 2; its
`yaml_12` option is left off because it emits a `%YAML` directive, and off it
quotes YAML 1.1 words such as `no`) and re-indented to the target column. The list style (items under
their key, or at the key's column) is detected from the first block list that
is a mapping value in the file, and passed as `compact_list_indent`. A string
scalar is written by yamled's own rule (plain, then single-quoted, then
double-quoted, each checked by reading it back), because the style of one
scalar is the part a person notices in a diff. Considered: `yaml_serde` for
rendering, which lost because the family is leaving it.

### 6. Errors

`Error` is a closed, non-exhaustive enum: `Parse` (with line and column),
`Pointer` (a malformed JSON pointer), `NoNode(Path)`, `WrongKind` (for example
`push` on a mapping), `KeyExists` (an `insert` of a key the mapping has),
`Unsupported` (valid YAML this version cannot write in place, such as a block
value inside a flow collection), `Invalid` (the edited text did not parse,
with the parser's message), and `Serialize`. No `anyhow` in a library.

### 7. Flow context and key-line comments

A plain scalar written inside `[...]` or `{...}` is quoted when it holds `,`,
`[`, `]`, `{`, or `}`, because those end a plain scalar there even though the
same text reads back unchanged in block context. Emptying a keyed list writes
`[]` after the colon and keeps the rest of the key line, such as a comment;
filling an empty list keeps that rest on the key line and writes the items
below it. Removing the first key of a sequence item moves the comment lines
owned by the next key above the item, so no neighbour's comment is lost.

## Risks

1. Flow collections other than an empty `[]` are refused by the edits in this
   change. A ledger that writes `blocked_by: [A-1]` can still replace the whole
   list, which rewrites that one line.
2. The spike wrote `queue: ` with a trailing space when filling `queue: []`.
   The put and push paths strip the space; a test covers it.

[yamlpath]: https://crates.io/crates/yamlpath
[yamlpatch]: https://crates.io/crates/yamlpatch
[granit-parser]: https://crates.io/crates/granit-parser
