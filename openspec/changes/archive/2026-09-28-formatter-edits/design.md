# Design: edits a ledger formatter needs

## Context

yamled 0.0.1 moves one sequence item at a time with `take` and `put`, which
respace the item from the list around it. A formatter reorders many children
at once and wants the file's spacing kept exactly, including a comment that
sits between sections with blank lines around it and belongs to none of them.

## Decisions

### 1. Reorder permutes children through fixed slots

`reorder` cuts the collection into children and the gaps between them. Each
child is its owned comments, its entry, and its value. The named children
trade slots; the gaps and the unnamed children do not move. Considered: a
`take` and `put` for mapping entries. It lost because each put respaces the
entry from its new neighbours, so a sequence of moves drifts from the spacing
the file had, and a loose comment would travel with whichever entry it ended
up beside.

### 2. A partial order names only what moves

A caller names the children it wants in order, and they fill the slots those
same children held. A formatter that orders `queue`, `horizon`, and `archive`
leaves `style` and `active` alone without listing them. Considered: a full
permutation of every child. It lost because the caller would need every key
in the file, including ones it does not know.

### 3. Flow edits stay on one line

A flow sequence is edited only when it sits on one line, which is how ledgers
write blockers. The separator and bracket padding are read from the list: the
text between its first two items, and a space after `[`. A list with fewer
than two items shows no separator, so a comma and one space are used.
Considered: re-flowing a multi-line flow sequence. It lost because comments
may sit between its lines and nothing in the known ledgers writes one.

### 4. Reindent checks the structure it leaves

Shifting lines left can move a list out from under its key, which parses as a
different document without any error. `reindent` compares the index before
and after: every node's style, child count, and key must match, or the edit
fails with `Error::Invalid`. Considered: trusting the re-parse alone. It lost
because a re-parse only proves the text is YAML, not that it is the same YAML.

### 5. Insert at an index reuses put

`insert_item` renders the value as `push` does and places it with `put`, so an
inserted row is spaced by the same rule as a moved one. Considered: a public
`Fragment` constructor. It lost because the fragment would be rendered without
the document's list style.

## Risks

1. A reorder of a mapping whose first entry shares its line with a sequence
   dash (`- id: A-1`) is refused: that entry cannot trade places with the next
   one without moving the dash.
