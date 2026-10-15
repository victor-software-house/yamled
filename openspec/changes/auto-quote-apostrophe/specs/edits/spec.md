# Spec Delta

## MODIFIED Requirements

### Requirement: Replace a value

`Document::replace` SHALL write a serializable value in place of the node at a
path. A replaced scalar SHALL keep its style when that style can represent the
new value: plain stays plain, single- and double-quoted stay quoted, folded and
literal stay block scalars. Otherwise the writer SHALL choose plain, then
single-quoted, then double-quoted, in that order, taking the first that reads
back as the same value. Single quotes SHALL be skipped in that order when the
text holds an apostrophe and no double quote.

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

### Requirement: Write text in a chosen block style

`Document::replace_text`, `Document::insert_text`, and `Document::push_text`
SHALL write a string in a caller-chosen style: automatic, folded (`>-`),
literal (`|-`), or one line. Automatic style SHALL follow the quoting order of
"Replace a value", so text with an apostrophe and no double quote is never
written with a doubled single quote. One-line style SHALL follow the same order
without a block scalar and without the node's current style, so a block scalar
it replaces becomes one line, and text with a line break is double-quoted with
`\n` escapes. An indentation indicator SHALL be written only when the text
starts with a space.

#### Scenario: A dated note is folded without an indentation indicator

- **WHEN** the caller pushes the text `2026-09-28: a dated note` with folded
  style
- **THEN** the item reads `- >-` followed by the text on the next line

#### Scenario: An apostrophe is written inside double quotes

- **WHEN** the caller pushes the text `2026-10-15: it's here` with automatic
  style
- **THEN** the item reads `- "2026-10-15: it's here"`

#### Scenario: Both quote characters keep single quotes

- **WHEN** the caller pushes the text `it's "quoted": here` with automatic
  style
- **THEN** the item reads `- 'it''s "quoted": here'`

#### Scenario: A folded item becomes one line

- **WHEN** a list item reads `- >2-` followed by `2026-09-28: a folded note`
- **AND** the caller replaces it with the same text in one-line style
- **THEN** the item reads `- '2026-09-28: a folded note'`

#### Scenario: A literal item becomes one escaped line

- **WHEN** a list item reads `- |-` followed by the lines `first` and `second`
- **AND** the caller replaces it with the same text in one-line style
- **THEN** the item reads `- "first\nsecond"`
