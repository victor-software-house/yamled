# Design

## Decisions

1. **Drop a blank line that would follow a kept scalar.** The alternative
   was to keep refusing such edits. It lost because the refusal made whole
   ledgers uneditable over one note, and because a blank line after a kept
   scalar can only ever change that scalar's value: dropping it is the one
   way to keep every value and still move the item.
2. **One end for every edit.** `item_end` walks to the deepest last child and
   extends over its kept lines. The alternative, extending only where an edit
   was reported broken, lost because each edit that measured an item by its
   value end had the same defect.
