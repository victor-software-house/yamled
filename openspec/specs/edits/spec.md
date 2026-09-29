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
literal stay block scalars. Otherwise the writer SHALL choose plain, then
single-quoted, then double-quoted, in that order, taking the first that reads
back as the same value.

#### Scenario: A plain title that now needs quoting is quoted once

- **WHEN** the row has `title: Old`
- **AND** the caller replaces it with `A new title: with a colon`
- **THEN** the line reads `title: 'A new title: with a colon'`

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
SHALL write a string in a caller-chosen style: automatic, folded (`>-`), or
literal (`|-`). An indentation indicator SHALL be written only when the text
starts with a space.

#### Scenario: A dated note is folded without an indentation indicator

- **WHEN** the caller pushes the text `2026-09-28: a dated note` with folded
  style
- **THEN** the item reads `- >-` followed by the text on the next line

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
