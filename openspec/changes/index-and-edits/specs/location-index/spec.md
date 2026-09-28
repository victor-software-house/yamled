## ADDED Requirements

### Requirement: Parse a source into an index of addressable nodes

`Document::parse` SHALL index one YAML document so that every scalar,
sequence, and mapping node can be found by a `Path` of mapping keys and
sequence indexes. The index SHALL record each node's value byte range, its
key's byte range when it is a mapping value, its style, and the number of
children of a collection. A source that is not valid YAML SHALL be refused with
the parser's line and column.

#### Scenario: A nested value is found by its path

- **WHEN** the source is:

  ```yaml
  queue:
    - id: A-1
      title: "First: row"
  ```

- **THEN** the node at path `queue` / `0` / `title` has value text
  `"First: row"`, style double-quoted, and a key range covering `title`
- **AND** the node at path `queue` has style block sequence and 1 child

#### Scenario: Invalid YAML is refused with its location

- **WHEN** the source is `a: 1\nb: [1, 2\nc: 3\n`
- **THEN** parsing fails with an error naming line 3

### Requirement: Address a node by JSON pointer

`Path::from_pointer` SHALL turn an RFC 6901 JSON pointer into a path, decoding
`~1` as `/` and `~0` as `~`. A segment that is a decimal number SHALL address a
sequence index when the parent is a sequence and a key when the parent is a
mapping.

#### Scenario: A schema error path finds its line

- **WHEN** the source has `title: x` on line 7 at `queue` / `2`
- **AND** the pointer is `/queue/2/title`
- **THEN** `Document::locate` returns line 7, column of `x`

#### Scenario: A missing node reports its nearest ancestor

- **WHEN** the pointer names a key that the mapping at `/queue/2` lacks
- **THEN** `Document::locate_nearest` returns the location of `/queue/2`

### Requirement: Comments belong to the node they sit above

A comment line directly above a node, with no blank line between them, SHALL
belong to that node. A comment after a node's value on the same line SHALL
belong to that node. A node's owned range SHALL start at its first owned
comment line and end at the end of its last line.

#### Scenario: A row owns the comment above it

- **WHEN** the source is:

  ```yaml
  queue:
    # owns A-1
    - id: A-1

    - id: A-2 # right
  ```

- **THEN** the owned range of `queue` / `0` starts at the `# owns A-1` line
- **AND** the owned range of `queue` / `1` includes `# right`
