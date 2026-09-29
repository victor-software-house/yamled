# Name the tag that refuses a replace

## Why

Since 0.0.2 a replace keeps the tags in front of a value only when they fit
the new value, and refuses otherwise with `Error::Unsupported` and a fixed
sentence. The error does not say which tag refused. Tags from the YAML 1.1
type repository (`!!timestamp`, `!!binary`, `!!set`, `!!omap`, `!!pairs`)
are not in the YAML 1.2 core schema, and yamled cannot check a new value
against them, so every replace under them refuses. The operator decided to
keep refusing them and to name the tag. This change is queue row YMD-005.

## What Changes

1. A new error variant, `Error::TagMismatch { path, tag }`, carries the tag
   as written. `Error` is `#[non_exhaustive]`, so callers keep compiling.
2. A replace whose kept tag does not fit, and a push or put into an empty
   value tagged as something other than a sequence, return it instead of
   `Error::Unsupported`.
3. The 1.1 type repository tags keep refusing.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `edits`: a refusal caused by a tag names the tag.

## Impact

1. Code: `src/error.rs`, `src/document.rs`, `src/document/values.rs`.
2. Tests: the tag refusals in `tests/edits.rs` match the new variant; the
   corpus and property tests count it as a refusal.
3. Consumers matching `Error::Unsupported` for a tag refusal match
   `Error::TagMismatch`. No consumer does today.
