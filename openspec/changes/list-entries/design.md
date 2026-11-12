# Design

## Context

The index is an arena of nodes built from granit-parser events.
A mapping frame alternates between a key and a value, and `Builder::scalar` took
the next scalar of a mapping as a key.
A key that is not a scalar broke the alternation, since `attach` always set the
mapping to expect a key next.

## Decisions

1. **The method sits on `Node`.**
   `Document::node(&Path::root())` reaches the root, and `Node::entries`
   lists a mapping from any node, so a caller walks a document without a path
   for each entry.
   It yields the key as `&str`, the scalar the parser read, and the value as
   a `Node`, which already holds `value()`, `key()` and `style()`.
   No new type is needed.
2. **A key that is not a scalar is in no entry.**
   `detach_key` puts it in the arena with its parent and no place among the
   children, and the value after it becomes an entry with no key text.
   A lookup by key never finds that entry, and `entries` leaves it out.
   `len` still counts it, since it is a child of the mapping.
3. **No span is changed.**
   The probe below ran on yamled 0.0.12.

| Source | Span of the value | Style |
|:--|:--|:--|
| `a: &x text` | `text` | plain |
| `b: *x` | `*x` | alias |
| `c: !!str 12` | `12` | plain |
| `d: &y !!str t` | `t` | plain |
| `"aA": 1` as a key | the key reads `aA` | not applicable |

      An anchor and a tag stay outside the span, so the span of a scalar is the
   scalar.
      A caller that needs the type of a plain scalar takes it from a parser that
   resolves tags, as serde-saphyr does.

## Risks

A mapping whose first key is not a scalar had its entries misread before this
change, and now lists the right ones.
No documented behaviour changes.
