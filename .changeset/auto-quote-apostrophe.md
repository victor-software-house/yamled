---
yamled: patch
---

New `Quotes`, set per document with `Document::with_quotes`, chooses the quotes every writer uses. `Quotes::Auto`, the default, writes plain text when it reads back, then single quotes for text without an apostrophe, then double quotes for text with an apostrophe and no double quote, then the form with fewer escapes. So an apostrophe is never written as a doubled single quote. `Quotes::Single` and `Quotes::Double` pick one style. A scalar already quoted keeps its quotes on replace.

`TextStyle` now chooses layout only. New `TextStyle::Line` writes a string on one line whatever the node's current style, so `replace_text` can turn a folded or literal item into one line.
