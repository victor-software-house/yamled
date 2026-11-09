---
yamled: patch
---

The `serde` feature takes serde-saphyr as vsh-serde-saphyr 0.0.2, under the same library name, `serde_saphyr`.
That release is serde-saphyr 2.1.1 with two patches.

Upstream serde-saphyr formats a whole error report while it deserializes, snippet renderer included.
So every binary that read a value through yamled carried annotate-snippets.
The patched release reads the error's kind instead, and builds an alias error's message from the plain text.
A program that also depends on ctl-core now links one serde-saphyr instead of two.
