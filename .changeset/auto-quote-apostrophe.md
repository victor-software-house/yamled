---
yamled: patch
---

Automatic text style writes text that holds an apostrophe and no double quote inside double quotes, so it never carries a doubled single quote. Text with both quote characters keeps single quotes, and a scalar already single-quoted keeps its style on replace.

New `TextStyle::Line` writes a string on one line whatever the node's current style, so `replace_text` can turn a folded or literal item into one line.
