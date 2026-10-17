---
"yamled": patch
---

`TextStyle::Line` now ignores the quotes a replaced scalar already has, as its documentation says: `'It''s here: now'` replaced in one-line style under `Quotes::Auto` reads `"It's here: now"`, and a needlessly quoted `"demo"` reads `demo`.
`TextStyle::Auto` still keeps a quoted scalar's quotes.
