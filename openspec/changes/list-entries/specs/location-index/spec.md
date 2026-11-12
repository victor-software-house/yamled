## ADDED Requirements

### Requirement: List the entries of a mapping

`Node::entries` SHALL list the entries of a block or flow mapping that have a
scalar key, in the order written, each as the key's text and the node of the
value, and SHALL list none for any other node.
The key's text SHALL be the scalar the parser read, with its quotes and escapes
resolved.
The span of a value SHALL exclude an anchor and a tag before it, and an alias
SHALL have the style alias and the span of `*name`.

#### Scenario: Entries in order

- **WHEN** a mapping holds `id` and `title` entries
- **THEN** `entries` yields `id`, then `title`, each with its value node

#### Scenario: Quoted keys

- **WHEN** a flow mapping has the keys `z`, `"b c"` and `'d'`
- **THEN** the keys are `z`, `b c` and `d`

#### Scenario: An anchor, an alias and a tag

- **WHEN** the entries are `a: &x text`, `b: *x`, `c: !!str 12` and `d: &y !!str
  t`
- **THEN** the value spans are `text`, `*x`, `12` and `t`, and the styles are
  plain, alias, plain and plain

#### Scenario: Not a mapping

- **WHEN** the node is a sequence or a scalar
- **THEN** `entries` yields nothing

### Requirement: A key that is not a scalar leaves later entries addressable

An entry whose key is a sequence, a mapping or an alias SHALL be left out of
`Node::entries`, and the entries after it SHALL keep their keys and be found by
`Path`.

#### Scenario: A sequence and a mapping as keys

- **WHEN** a mapping is `? [a, b]`, `: v`, `z: w`, `? {c: d}`, `: u` and `y: x`
- **THEN** `entries` yields `z` and `y`, and the nodes at `z` and `y` have the
  text `w` and `x`

#### Scenario: An alias as a key

- **WHEN** a mapping is `k: &k key`, `*k : v` and `z: w`
- **THEN** `entries` yields `k` and `z`
