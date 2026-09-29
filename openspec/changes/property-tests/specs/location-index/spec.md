## ADDED Requirements

### Requirement: A block scalar's range covers its header and its content

The index SHALL start a literal or folded scalar's range at its `|` or `>`
header and end it at its last content line: a line with text, or with only
spaces that reach past the text's indentation. A block scalar with no content
SHALL end at its header.

#### Scenario: An empty block scalar ends at its header

- **WHEN** the source is:

  ```yaml
  clip: >

  next: 1
  ```

- **AND** the caller replaces `clip` with `text`
- **THEN** the source reads:

  ```yaml
  clip: >-
    text

  next: 1
  ```
