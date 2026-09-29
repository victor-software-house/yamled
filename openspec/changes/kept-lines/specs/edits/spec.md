# edits

## ADDED Requirements

### Requirement: A kept block scalar keeps its value through every edit

An item's text SHALL run through the blank lines its last block scalar keeps
(`|+` or `>+`). `take`, `put`, `remove`, `insert`, and `reorder` SHALL move
those lines with the item, and SHALL drop a blank line that would otherwise
land directly after it, so every value reads back unchanged.

#### Scenario: An item that keeps its lines moves to the front

- **WHEN** the item at index 1 of this list is taken and put at index 0:

  ```yaml
  notes:
    - a
    - |+
      kept

    - c
  ```

- **THEN** the notes read back as `"kept\n\n"`, `"a"`, and `"c"`

#### Scenario: Removing the entry after a kept scalar leaves it unchanged

- **WHEN** `b` is removed from:

  ```yaml
  a: |+
    x
  b: 1

  c: 2
  ```

- **THEN** the document reads:

  ```yaml
  a: |+
    x
  c: 2
  ```

#### Scenario: A key inserted after a kept scalar goes after its lines

- **WHEN** `new: v` is inserted after `a` in:

  ```yaml
  a: |+
    x

  b: 1
  ```

- **THEN** `a` is still `"x\n\n"` and `new` follows its blank line
