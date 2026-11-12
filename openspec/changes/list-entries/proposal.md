# List the entries of a mapping

## Why

A caller that reads a YAML mapping it knows nothing about, such as the front
matter of a Markdown file, needs each entry's key, the byte range of its value
and how the value is written.
The index holds all three, and `Document::node` reaches a value only by a key
the caller already knows.
hakkiri lints the string values of front matter and read the keys with a line
scanner of its own, which skipped anchors, aliases and complex keys.
The index has a defect of its own there: a key that is a sequence, a mapping or
an alias made the parser's events read as the wrong entries, and the entries
after it could not be found by path.
This change is queue row YMD-013.

## What Changes

1. `Node::entries` lists the entries of a mapping that have a scalar key, in the
   order written, each as the key's text and the node of its value.
2. A key that is not a scalar is held by the index and by no entry, so the
   entries after it keep their keys and stay addressable by path.
3. The tests pin the spans that the probe measured: an anchor and a tag stay
   outside the span of a value, and an alias is its own style with the span of
   `*name`.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `location-index`: a mapping lists its entries, and a key that is not a scalar
  leaves the entries after it addressable.

## Impact

1. Code: `src/index.rs` and `src/document.rs`.
2. Tests: `tests/index.rs`.
3. Consumers: ctl-core reads the top-level entries of a front matter block
   through the new method, with serde-saphyr's typing.
   No existing call changes.
