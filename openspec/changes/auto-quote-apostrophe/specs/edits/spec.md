# Spec Delta

## ADDED Requirements

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

### Requirement: Quoting costs at most one parse

The writer SHALL pick the quoted style from one pass over the text, with no
parse. It SHALL parse a candidate back only to prove plain text or a block
scalar, and SHALL build quoted text by construction. Plain SHALL not be tried
for a character only double quotes can carry.

#### Scenario: A note that needs quotes is written without a second parse

- **WHEN** the caller pushes `a: b` in one-line style
- **THEN** the writer parses the plain candidate once, rejects it, and writes
  `- 'a: b'` without parsing it

### Requirement: A flow item is written by the flow rule

A scalar written inside a flow collection SHALL be proven plain by parsing it
inside `[...]`, so `,`, `[`, `]`, `{`, and `}` force quotes there, and SHALL
never be written as a block scalar, whatever layout the caller asked for.

#### Scenario: Flow punctuation forces quotes

- **WHEN** a row has `tags: [a, b, c]`
- **AND** the caller replaces the items with `x, y` (automatic layout),
  `it's [z]` (folded layout), and `a: b` (one line)
- **THEN** the line reads `tags: ['x, y', "it's [z]", 'a: b']`

### Requirement: A batch of edits parses once

`Document::batch` SHALL run several edits and parse the result once. Inside a
batch every path SHALL name the text as it was when the batch began, two
inserts at one place SHALL land in call order, and edits that touch the same
bytes SHALL be refused. When an edit fails or the result does not parse, the
document SHALL keep its old text.

#### Scenario: Edits name the text the batch began with

- **WHEN** a list holds a folded item and a two-line literal item
- **AND** one batch replaces both on one line and inserts `two` and `three`
  at index 2
- **THEN** the list reads the first item, `one`, `two`, `three`, in that order

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

## MODIFIED Requirements

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

### Requirement: Write text in a chosen block style

`Document::replace_text`, `Document::insert_text`, and `Document::push_text`
SHALL write a string in a caller-chosen layout, `TextStyle`: automatic, folded
(`>-`), literal (`|-`), or one line. `TextStyle` SHALL choose layout only; the
quotes of a one-line string SHALL come from the document's `Quotes`.
Automatic layout SHALL keep the node's current style when it can hold the
text, then try a literal block for text with a line break, then one line.
One-line layout SHALL ignore the node's current style and never write a block
scalar, so a block scalar it replaces becomes one line, and text with a line
break is double-quoted with `\n` escapes. An indentation indicator SHALL be
written only when the text starts with a space.

#### Scenario: A dated note is folded without an indentation indicator

- **WHEN** the caller pushes the text `2026-09-28: a dated note` with folded
  style
- **THEN** the item reads `- >-` followed by the text on the next line

#### Scenario: A folded item becomes one line

- **WHEN** a list item reads `- >2-` followed by `2026-09-28: a folded note`
- **AND** the caller replaces it with the same text in one-line style
- **THEN** the item reads `- '2026-09-28: a folded note'`

#### Scenario: A literal item becomes one escaped line

- **WHEN** a list item reads `- |-` followed by the lines `first` and `second`
- **AND** the caller replaces it with the same text in one-line style
- **THEN** the item reads `- "first\nsecond"`
