---
yamled: patch
---

New `Quotes`, set per document with `Document::with_quotes`, chooses the quotes every writer uses. `Quotes::Auto`, the default, writes plain text when it reads back, then single quotes for text without an apostrophe, then double quotes for text with an apostrophe and no double quote, then the form with fewer escapes. So an apostrophe is never written as a doubled single quote. `Quotes::Single` and `Quotes::Double` pick one style. A scalar already quoted keeps its quotes on replace.

`TextStyle` now chooses layout only. New `TextStyle::Line` writes a string on one line whatever the node's current style, so `replace_text` can turn a folded or literal item into one line.

One decision function writes block values and flow items alike. It chooses the style from one pass over the text by the reader's own rules, with no parse. A block scalar no longer holds a character only double quotes carry, such as U+2028. New `Document::batch` runs several edits and parses once at the end, and an edit that writes the bytes already there no longer reparses the document.

`Document::batch` accepts the caller's own error type. yamled now builds on serde-saphyr 2.0.0 and granit-parser 2.0.0.
