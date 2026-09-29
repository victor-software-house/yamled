# Design

## Decisions

1. **Keep refusing the YAML 1.1 type repository tags.** The alternatives were
   to check each by shape, to keep the tag unchecked, or to drop it. Checking
   lost because no consumer writes these tags; keeping it unchecked can write
   a value its tag contradicts; dropping it retypes the value silently.
2. **A new variant, not a longer `Unsupported` sentence.** `Unsupported`
   carries a `&'static str`, so it cannot hold the tag. Changing that field
   would break every caller's pattern; a new variant does not, because
   `Error` is `#[non_exhaustive]`.
