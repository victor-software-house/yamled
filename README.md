# yamled

Format-preserving YAML edits: change what you meant, keep every other byte.

`yamled` edits a YAML file the way a person would. It replaces, removes, and
inserts the values you name, and leaves comments, blank lines, quoting, block
and flow style, and indentation as they were. It is pure Rust, built on
[`granit-parser`](https://crates.io/crates/granit-parser) spans and
[`serde-saphyr`](https://crates.io/crates/serde-saphyr) values.

Status: `0.0.0` reserves the name. The location index and edit operations
come next.

## License

MIT
