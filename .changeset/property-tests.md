---
yamled: patch
---

Fix eleven edit defects found by new property and YAML test suite tests: a replace keeps anchors, tags, and key-line comments; block scalars are replaced with their header and the trailing lines they own; empty values, explicit keys, and lists written at their key's column are handled or refused instead of corrupted; no edit panics.
