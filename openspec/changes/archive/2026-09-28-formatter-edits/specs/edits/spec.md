## ADDED Requirements

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
