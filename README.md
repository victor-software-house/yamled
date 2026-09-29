# yamled

Format-preserving YAML edits: change what you meant, keep every other byte.

`yamled` edits a YAML file the way a person would. It replaces, removes, and
inserts the values you name, and leaves comments, blank lines, quoting, block
and flow style, and indentation as they were. It is pure Rust, built on
[`granit-parser`](https://crates.io/crates/granit-parser) spans and
[`serde-saphyr`](https://crates.io/crates/serde-saphyr) values.

## Features

| Feature | Adds | Pulls |
|:--|:--|:--|
| (none) | Location index; `remove`, `take`, `put`, `reorder`, `reindent` | `granit-parser` |
| `serde` (default) | `replace`, `insert`, `push`, `insert_item`, and their `_text` forms | `serde`, `serde-saphyr` |

A caller that only needs the line and column of a path, such as a schema
error reporter, can depend on `yamled` with `default-features = false`.

## License

MIT
