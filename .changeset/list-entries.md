---
"yamled": patch
---

`Node::entries` lists the entries of a mapping with a scalar key, each as the key's text and the node of its value, in the order written.
An entry whose key is a sequence, a mapping or an alias no longer hides the entries after it from a lookup by path.
