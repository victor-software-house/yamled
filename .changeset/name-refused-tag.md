---
yamled: patch
---

A replace refused by a tag, and a fill of an empty value tagged as something other than a sequence, now fail with `Error::TagMismatch` naming the tag, such as `!!timestamp`, instead of `Error::Unsupported`.
