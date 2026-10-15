# Design

## Context

Every writer asks one function how to write a string: plain, single-quoted,
double-quoted, literal, or folded.
Until this change, plain text and block scalars were proven by parsing the
candidate back with the family reader: one parse of 3.1 µs per string on the
largest consumer ledger.
The operator asked for a decision measured in nanoseconds, with no parse and
no serialization per string.

## Decisions

### 1. yamled encodes the reader's rules

One pass over the bytes decides everything: the plain-scalar syntax in block
or flow context, the type the reader resolves plain text to, the escape
counts the quote order needs, and whether a block scalar holds the text.
The rules are the ones granit-parser 2.0.0 scans with and serde-saphyr 2.0.0
resolves with when `strict_booleans` is set:

1. Plain text is not empty, does not start or end with a space or tab, and
   does not start with an indicator (`,[]{}#&*!|>'"%@` and the backtick).
   `-`, `?`, and `:` start it only when a non-space follows.
2. `:` before a space, a tab, or the end, and `#` after a space or tab, end
   it. In flow context, so do `,`, `[`, `]`, `{`, and `}`.
3. The reader resolves `~`, `null`, `true`, and `false` in any case, and
   integers and floats, to other types. An integer is decimal without a
   leading zero, or `0x`, `0o`, or `0b` digits, with single underscores
   between digits, and must fit 64 bits; a float is `.inf` or `.nan` with an
   optional sign, in any case, or a numeral Rust's `f64` parser accepts.
   `yes`, `on`, `inf`, `nan`, and dates stay strings.

Alternative: keep the round trip. It lost on cost: one parse per string.

Alternative: serde-saphyr's `is_plain_value_safe`. It is `pub(crate)`, and it
quotes text the reader keeps as a string, such as `inf`. Its shape informed
this pass, with attribution in the source.

### 2. The reader stays the oracle in tests

Differential tests compare the pass with a parse of each candidate: a
generator biased toward the hard cases, every scalar of the YAML test suite,
and every string of the ledgers a caller points `YAMLED_CORPUS` at. Any
disagreement fails. Debug builds assert the same agreement on every
decision, so every other test checks it too.

Alternative: commit the consumer ledgers as fixtures. It lost because some
are private; the environment variable lets the build host run them.

### 3. A block scalar holds no character only double quotes carry

The pass already refuses plain and single quotes for a control character,
U+2028, U+2029, U+FEFF, U+FFFE, or U+FFFF. Block scalars now follow the same
set, so one flag serves every unquoted form.

Alternative: allow what the scanner accepts in a block, such as U+2028. It
lost because the same character would be escaped in one style and raw in
another.

### 4. Benchmarks reach the decision through a hidden module

`yamled::bench` exposes the decision for the criterion benchmarks and is
`#[doc(hidden)]`, outside the API and semver checks.

Alternative: a documented public function. It lost because it would add API
for a benchmark.
