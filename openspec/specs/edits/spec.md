# edits Specification

## Purpose

How yamled changes a YAML source: each edit is a byte splice that changes only
the bytes it names, keeps comments and style, and leaves the source unchanged
when the result would not parse.

## Requirements

### Requirement: An edit changes only the bytes it names

Every edit SHALL be applied as a byte splice of the source. Bytes outside the
spliced range SHALL be identical before and after the edit. After each edit the
document SHALL parse the result; when the result is not valid YAML, the edit
SHALL fail and the source SHALL stay unchanged.

#### Scenario: Replacing one value leaves the rest of the file byte-identical

- **WHEN** a ledger has `active: CTC-006` on line 8
- **AND** the caller replaces `active` with `CTC-008`
- **THEN** exactly one line differs: `active: CTC-008`

### Requirement: Replace a value

`Document::replace` SHALL write a serializable value in place of the node at a
path. A replaced scalar SHALL keep its style when that style can represent the
new value: plain stays plain, single- and double-quoted stay quoted, folded and
literal stay block scalars. Otherwise the writer SHALL write it plain or quoted
as "A document declares its quotes" says.

#### Scenario: A plain title that now needs quoting is quoted once

- **WHEN** the row has `title: Old`
- **AND** the caller replaces it with `A new title: with a colon`
- **THEN** the line reads `title: 'A new title: with a colon'`

#### Scenario: A title with an apostrophe is double-quoted

- **WHEN** the row has `title: Old`
- **AND** the caller replaces it with `It's done: mostly`
- **THEN** the line reads `title: "It's done: mostly"`

#### Scenario: A single-quoted title keeps its quotes

- **WHEN** the row has `title: 'Old: one'`
- **AND** the caller replaces it with `It's done: mostly`
- **THEN** the line reads `title: 'It''s done: mostly'`

#### Scenario: A folded outcome stays folded

- **WHEN** the row has `outcome: >-` with its text on the next lines
- **AND** the caller replaces it with a new sentence
- **THEN** the key line still reads `outcome: >-`
- **AND** the new text sits on the next line at the old text's indentation

### Requirement: Remove a key or an item

`Document::remove` SHALL delete the node at a path together with its key, its
owned comments, and its line break. It SHALL NOT remove any byte of a
neighbouring node, including a comment the neighbour owns.

#### Scenario: Removing an item keeps the next row's comment

- **WHEN** a list item is followed by a blank line and a comment that owns the
  next item
- **AND** the caller removes the first item
- **THEN** the comment and the next item are byte-identical

### Requirement: Insert a key and push an item

`Document::insert` SHALL add a key and value to a block mapping at its end,
before a named key, or after a named key, at the mapping's indentation.
`Document::push` SHALL append an item to a block sequence at the sequence's
indentation. Pushing into an empty flow sequence `[]` SHALL turn it into a
block sequence under its key. A new nested value SHALL follow the file's list
style: items indented under their key, or at the key's column.

#### Scenario: A note is appended to a block list

- **WHEN** the row has `notes:` with two items indented six spaces
- **AND** the caller pushes a third note
- **THEN** the note is written as a third item, indented six spaces like the
  others

#### Scenario: The first row goes into an empty queue

- **WHEN** the source has `queue: []`
- **AND** the caller pushes a mapping `{id: T-001, acceptance: [It holds.]}`
- **THEN** the source reads:

  ```yaml
  queue:
    - id: T-001
      acceptance:
        - It holds.
  ```

### Requirement: Write text in a chosen block style

`Document::replace_text`, `Document::insert_text`, and `Document::push_text`
SHALL write a string in a caller-chosen layout, `TextStyle`: automatic, folded
(`>-`), literal (`|-`), or one line. `TextStyle` SHALL choose layout only; the
quotes of a one-line string SHALL come from the document's `Quotes`.
Automatic layout SHALL keep the node's current style when it can hold the
text, then try a literal block for text with a line break, then one line.

#### Scenario: A quoted scalar takes the document's quotes in one-line style

- **WHEN** the row has `title: 'It''s here: now'` and `scope: "demo"` under
  `Quotes::Auto`
- **AND** the caller replaces each with its own text in one-line style
- **THEN** the lines read `title: "It's here: now"` and `scope: demo`

### Requirement: One-line layout writes no block scalar

One-line layout SHALL ignore the node's current style and never write a block
scalar, so a block scalar it replaces becomes one line, and text with a line
break is double-quoted with `\n` escapes.

#### Scenario: A folded item becomes one line

- **WHEN** a list item reads `- >2-` followed by `2026-09-28: a folded note`
- **AND** the caller replaces it with the same text in one-line style
- **THEN** the item reads `- '2026-09-28: a folded note'`

#### Scenario: A literal item becomes one escaped line

- **WHEN** a list item reads `- |-` followed by the lines `first` and `second`
- **AND** the caller replaces it with the same text in one-line style
- **THEN** the item reads `- "first\nsecond"`

### Requirement: A block scalar holds only what it can carry

An indentation indicator SHALL be written only when the text starts with a
space. A block scalar SHALL not hold a character only double quotes carry,
such as U+2028; such text SHALL be double-quoted.

#### Scenario: A dated note is folded without an indentation indicator

- **WHEN** the caller pushes the text `2026-09-28: a dated note` with folded
  style
- **THEN** the item reads `- >-` followed by the text on the next line

#### Scenario: A line separator is never written raw in a block

- **WHEN** the caller pushes `a`, U+2028, and `b` with literal style
- **THEN** the item reads `- "a\u2028b"`

### Requirement: Take an item and put it elsewhere

`Document::take` SHALL remove a sequence item with its owned comments and
return it as a fragment. `Document::put` SHALL insert a fragment into a block
sequence at an index, re-indented to that sequence. A take SHALL leave exactly
one blank line between remaining neighbours that were separated.

#### Scenario: A row moves from the queue to the front of the archive

- **WHEN** the queue's first row is taken and put at index 0 of `archive`
- **THEN** the row's text, including its owned comment, is byte-identical in
  its new place apart from indentation
- **AND** the queue and the archive keep one blank line between rows

### Requirement: Spacing is kept, not guessed

A new item from `Document::put` or `Document::push` SHALL be separated from its
neighbours by a blank line when every pair of neighbours in the sequence is
separated, and SHALL NOT be when no pair is. A sequence with fewer than two
items, or with both kinds of gap, SHALL use the document's `Spacing`, set with
`Document::with_spacing`; the default is `Spacing::Tight`.

#### Scenario: A compact list stays compact

- **WHEN** a list has three items with no blank lines between them
- **AND** the document's spacing is `Spacing::Blank`
- **AND** the caller moves the last item to index 1
- **THEN** no blank line is written

#### Scenario: A row moved in a list of two uses the configured spacing

- **WHEN** a list has two items separated by a blank line
- **AND** the caller takes the second item and puts it at index 0
- **THEN** a blank line separates the two items under `Spacing::Blank`
- **AND** none does under `Spacing::Tight`

### Requirement: Reorder the children of a block collection

`Document::reorder` SHALL take a block sequence or block mapping and a list of
its children, by index or key. The named children SHALL trade places among the
slots they hold, in the order given, each with its owned comments and its text
byte-identical. Children it does not name, and the blank lines and loose
comments between children, SHALL stay where they are. A child named twice
SHALL fail with `Error::Repeated`, and a child that does not exist with
`Error::NoNode`.

#### Scenario: A section moves above the comment the next key owns

- **WHEN** the source is:

  ```yaml
  queue: []

  # History, not work.
  archive:
    - id: A-1
  horizon: []
  ```

- **AND** the caller reorders the root as `queue`, `horizon`, `archive`
- **THEN** the source reads:

  ```yaml
  queue: []

  horizon: []
  # History, not work.
  archive:
    - id: A-1
  ```

#### Scenario: Two rows swap and the blank line between them stays

- **WHEN** a list reads `- id: A-1`, a blank line, then `- id: A-2`
- **AND** the caller reorders it as index 1, then index 0
- **THEN** it reads `- id: A-2`, a blank line, then `- id: A-1`

### Requirement: Edit a flow sequence in flow style

`Document::remove` SHALL remove an item from a flow sequence written on one
line, together with one separator, and SHALL leave `[]` when it removes the
last item. `Document::replace` on a flow sequence SHALL write a sequence of
single-line scalars in flow style, with the separator and bracket padding the
list already uses, and SHALL write a block sequence otherwise.

#### Scenario: One of two blockers is removed

- **WHEN** a row has `blocked_by: [A-1, A-2]`
- **AND** the caller removes `blocked_by/0`
- **THEN** the line reads `blocked_by: [A-2]`

#### Scenario: A padded flow list keeps its padding

- **WHEN** a row has `tags: [ a, b ]`
- **AND** the caller replaces `tags` with `["c", "d", "e"]`
- **THEN** the line reads `tags: [ c, d, e ]`

### Requirement: Re-indent a block collection

`Document::reindent` SHALL shift every line of a block collection's children,
with their comments, nested collections, and block scalars, so the children
start at a given column. Empty lines SHALL stay empty. When the shifted text
would parse as a different structure, the edit SHALL fail with
`Error::Invalid` and the source SHALL stay unchanged.

#### Scenario: Rows written four columns deep move to two

- **WHEN** the source is:

  ```yaml
  queue:
      # owns A-1
      - id: A-1
        outcome: >-
          Folded text.
  ```

- **AND** the caller re-indents `queue` to column 2
- **THEN** the source reads:

  ```yaml
  queue:
    # owns A-1
    - id: A-1
      outcome: >-
        Folded text.
  ```

### Requirement: Insert a value at an index

`Document::insert_item` and `Document::insert_item_text` SHALL write a value
into a sequence at an index, re-indented and spaced as `Document::put` places a
fragment. An index equal to the length SHALL append.

#### Scenario: A new row goes between two rows

- **WHEN** a list has rows `A-1` and `A-2` separated by a blank line
- **AND** the caller inserts `{id: A-3}` at index 1
- **THEN** the list reads `A-1`, a blank line, `A-3`, a blank line, `A-2`

### Requirement: A replace keeps what surrounds the value

`Document::replace` SHALL keep a value's anchors and tags, and a comment on
its key's line, whatever kind of value replaces it. A block scalar that keeps
its trailing lines (`|+`) SHALL be replaced together with the blank lines it
owns.

#### Scenario: An anchored value keeps its anchor

- **WHEN** the source is:

  ```yaml
  key: &shared old # note
  copy: *shared
  ```

- **AND** the caller replaces `key` with `new`
- **THEN** the source reads:

  ```yaml
  key: &shared new # note
  copy: *shared
  ```

### Requirement: An edit refuses rather than corrupts

An edit SHALL return `Error::Unsupported`, and leave the source unchanged,
for a value it cannot write in place: a key with no `:` after it, an empty
value inside a flow collection, and an empty document. No edit SHALL panic
on any input the parser accepts.

#### Scenario: A key without a value in a flow mapping is refused

- **WHEN** the source is `{http://foo.com, other: 1}`
- **AND** the caller replaces `http://foo.com` with `null`
- **THEN** the edit fails with `Error::Unsupported`
- **AND** the source is unchanged

### Requirement: A kept block scalar keeps its value through every edit

An item's text SHALL run through the blank lines its last block scalar keeps
(`|+` or `>+`). `take`, `put`, `remove`, `insert`, and `reorder` SHALL move
those lines with the item, and SHALL drop a blank line that would otherwise
land directly after it, so every value reads back unchanged.

#### Scenario: An item that keeps its lines moves to the front

- **WHEN** the item at index 1 of this list is taken and put at index 0:

  ```yaml
  notes:
    - a
    - |+
      kept

    - c
  ```

- **THEN** the notes read back as `"kept\n\n"`, `"a"`, and `"c"`

#### Scenario: Removing the entry after a kept scalar leaves it unchanged

- **WHEN** `b` is removed from:

  ```yaml
  a: |+
    x
  b: 1

  c: 2
  ```

- **THEN** the document reads:

  ```yaml
  a: |+
    x
  c: 2
  ```

#### Scenario: A key inserted after a kept scalar goes after its lines

- **WHEN** `new: v` is inserted after `a` in:

  ```yaml
  a: |+
    x

  b: 1
  ```

- **THEN** `a` is still `"x\n\n"` and `new` follows its blank line

### Requirement: A refusal caused by a tag names the tag

When a kept tag does not fit the new value, or yamled cannot check a value
against it, the edit SHALL fail with `Error::TagMismatch` carrying the tag as
written, and SHALL leave the source unchanged.

#### Scenario: A timestamp tag refuses and is named

- **WHEN** `created` is replaced with `"2002-01-01"` in:

  ```yaml
  created: !!timestamp 2001-12-14
  ```

- **THEN** the replace fails with `Error::TagMismatch` whose `tag` is
  `!!timestamp`, displayed as
  `at /created: the tag !!timestamp does not fit the new value`, and the
  source is unchanged

### Requirement: A document declares its quotes

A document SHALL carry one quote preference, `Quotes`, set with
`Document::with_quotes` and `Quotes::Auto` by default, and every writer,
flow sequence items included, SHALL follow it. Each preference SHALL write a
string plain when it reads back unquoted; otherwise `Single` SHALL write
single quotes and `Double` double quotes. A character only double quotes can
carry, such as a control character, SHALL get double quotes.

#### Scenario: Plain text stays plain

- **WHEN** the caller pushes `plain text` in one-line style
- **THEN** the item reads `- plain text` under every preference

#### Scenario: A control character is double-quoted

- **WHEN** the caller pushes `a`, the character U+0001, and `: b` in one-line
  style under `Quotes::Single`
- **THEN** the item reads `- "a\u0001: b"`

#### Scenario: Single and double preferences

- **WHEN** the caller pushes `it's: here` under `Quotes::Single`
- **THEN** the item reads `- 'it''s: here'`
- **AND** `a: b` under `Quotes::Double` reads `- "a: b"`

#### Scenario: A flow item follows the document's quotes

- **WHEN** a row has `tags: [a]`
- **AND** the caller replaces `tags` with `["it's, flow", "a, b"]` under
  `Quotes::Auto`
- **THEN** the line reads `tags: ["it's, flow", 'a, b']`

### Requirement: Automatic quotes follow a fixed order

`Quotes::Auto` SHALL quote a string that cannot be plain with the first that
applies: single quotes when the text holds no `'`; double quotes when it holds
`'` and no `"`; holding both, the form with fewer escapes, a tie going to
single quotes.

#### Scenario: Text without an apostrophe is single-quoted

- **WHEN** the caller pushes `a: b` in one-line style under `Quotes::Auto`
- **THEN** the item reads `- 'a: b'`

#### Scenario: An apostrophe without a double quote is double-quoted

- **WHEN** the caller pushes `it's: C:\dir` in one-line style under
  `Quotes::Auto`
- **THEN** the item reads `- "it's: C:\\dir"`

#### Scenario: Both quote characters take the form with fewer escapes

- **WHEN** the caller pushes `it's "x": y` in one-line style under
  `Quotes::Auto`
- **THEN** the item reads `- 'it''s "x": y'`, one escape against two
- **AND** `it's 'odd': "x"` reads `- "it's 'odd': \"x\""`, two against three

### Requirement: Choosing a style parses nothing

The writer SHALL choose a string's style from one pass over its bytes, with
no parse and no serialization: whether it can be plain in a block or flow
context, which quotes it takes, and whether a block scalar holds it. The
family reader, serde-saphyr with strict booleans, SHALL read every written
form back as the same string, and plain SHALL be chosen exactly when that
reader reads the unquoted text back as the string.

#### Scenario: Words the reader keeps as strings stay plain

- **WHEN** the caller pushes `yes`, `on`, `inf`, `2026-10-15`, and `<<` in
  one-line style
- **THEN** the items read `- yes`, `- on`, `- inf`, `- 2026-10-15`, and
  `- <<`

#### Scenario: Text the reader resolves to another type is quoted

- **WHEN** the caller pushes `True`, `NULL`, `~`, `0x1F`, `1_000`, `0755`,
  `1e5`, and `-.INF` in one-line style
- **THEN** each item is single-quoted, as `- 'True'` and `- '0755'`

#### Scenario: The decision agrees with the reader

- **WHEN** every string of the YAML test suite, of the consumer ledgers, and
  of a generator biased toward indicators, colons, hashes, spaces, numerals,
  and non-ASCII text is decided in block and flow context
- **THEN** plain, literal, and folded are each chosen exactly when the reader
  reads that form back as the string

### Requirement: A flow item is written by the flow rule

A scalar written inside a flow collection SHALL be decided by the rules the
reader applies inside `[...]`, so `,`, `[`, `]`, `{`, and `}` force quotes
there, and SHALL never be written as a block scalar, whatever layout the
caller asked for.

#### Scenario: Flow punctuation forces quotes

- **WHEN** a row has `tags: [a, b, c]`
- **AND** the caller replaces the items with `x, y` (automatic layout),
  `it's [z]` (folded layout), and `a: b` (one line)
- **THEN** the line reads `tags: ['x, y', "it's [z]", 'a: b']`

### Requirement: A batch of edits parses once

`Document::batch` SHALL run several edits and parse the result once. Inside a
batch every path SHALL name the text as it was when the batch began, and two
inserts at one place SHALL land in call order. When an edit fails, including
with the caller's own error type, or the result does not parse, the document
SHALL keep its old text.

#### Scenario: Edits name the text the batch began with

- **WHEN** a list holds a folded item and a two-line literal item
- **AND** one batch replaces both on one line and inserts `two` and `three`
  at index 2
- **THEN** the list reads the first item, `one`, `two`, `three`, in that order

#### Scenario: A caller's error ends the batch

- **WHEN** a batch replaces an item and then returns the caller's own error
- **THEN** the batch returns that error
- **AND** the document's text is what it was before the batch

### Requirement: A batch refuses edits its starting text cannot prove

A batch SHALL refuse edits that touch the same bytes, a key inserted twice
into one mapping, and a re-indent, which needs a parse to prove itself.

#### Scenario: A key inserted twice in one batch is refused

- **WHEN** one batch inserts the key `scope` into the same mapping twice
- **THEN** the batch fails with a key-exists error
- **AND** the document's text is what it was before the batch

#### Scenario: Overlapping edits leave the text unchanged

- **WHEN** one batch replaces the same item twice
- **THEN** the batch fails with an unsupported-edit error
- **AND** the document's text is what it was before the batch

### Requirement: An edit that changes nothing costs nothing

An edit whose rendering equals the bytes already there SHALL leave the text
as it is without parsing it again, inside a batch or outside one.

#### Scenario: Rewriting a value as it is written

- **WHEN** a row has `tail: kept`
- **AND** the caller replaces `tail` with `kept` in one-line style
- **THEN** the text is unchanged and the document was not parsed again
