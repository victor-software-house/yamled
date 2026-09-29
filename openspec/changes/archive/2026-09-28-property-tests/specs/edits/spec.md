## ADDED Requirements

### Requirement: A replace keeps what surrounds the value

`Document::replace` SHALL keep a value's anchors and tags, and a comment on
its key's line, whatever kind of value replaces it. A block scalar that keeps
its trailing lines (`|+`) SHALL be replaced together with the blank lines it
owns.

#### Scenario: An anchored value keeps its anchor

- **WHEN** the source is:

  ```yaml
  key: &shared old # note
  copy: *shared
  ```

- **AND** the caller replaces `key` with `new`
- **THEN** the source reads:

  ```yaml
  key: &shared new # note
  copy: *shared
  ```

### Requirement: An edit refuses rather than corrupts

An edit SHALL return `Error::Unsupported`, and leave the source unchanged,
for a value it cannot write in place: a key with no `:` after it, an empty
value inside a flow collection, and an empty document. No edit SHALL panic
on any input the parser accepts.

#### Scenario: A key without a value in a flow mapping is refused

- **WHEN** the source is `{http://foo.com, other: 1}`
- **AND** the caller replaces `http://foo.com` with `null`
- **THEN** the edit fails with `Error::Unsupported`
- **AND** the source is unchanged
