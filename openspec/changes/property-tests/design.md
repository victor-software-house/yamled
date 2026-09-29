# Design: property and corpus tests for every edit

## Context

yamled's tests compared whole files for cases chosen by hand. A byte-splicing
editor fails at the edges nobody writes down: empty values, anchors, block
scalar chomping, explicit keys.

## Decisions

### 1. The value model is the oracle

Each generated edit is applied to the document and to a `serde_json::Value`
model. After the edit the document is read back through `serde-saphyr` and
compared with the model, and every byte outside the edited collection's owned
lines must be unchanged. Considered: comparing against a second YAML writer.
It lost because it would test two formatters against each other, not the
edit.

### 2. The corpus replaces each scalar with itself

Replacing a scalar with its own value must keep the document's value and
every byte outside that node. This needs no expected output per case, so all
402 inputs of the suite run without hand-written fixtures. Considered:
requiring the replaced text to be byte-identical. It lost because a folded
scalar legitimately comes back refolded.

### 3. The suite is a pinned submodule

The suite is pinned to a commit of its `data` branch as a git submodule, so
the repository stores one commit hash, and the published crate excludes it.
Considered: vendoring the inputs. It lost because 400 upstream files would
sit in every diff and need their own lint exclusions forever.

### 4. Coverage is a floor

The corpus test counts the scalars it replaced and fails below the count at
the pinned commit (534). A change that starts refusing inputs it handled shows
up as a failure rather than a quieter run.

### 5. Block scalar ranges start at the header

The parser's span for a block scalar starts at its text, so the header and
everything keyed to it (take, remove, owned lines, replace) missed the header
line. The index now moves the start back to the header and ends the range at
the last content line. Considered: fixing each edit to look up the header. It
lost because every reader of the range would need the same fix.
