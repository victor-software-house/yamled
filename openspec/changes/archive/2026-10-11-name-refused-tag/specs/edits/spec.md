# edits

## ADDED Requirements

### Requirement: A refusal caused by a tag names the tag

When a kept tag does not fit the new value, or yamled cannot check a value
against it, the edit SHALL fail with `Error::TagMismatch` carrying the tag as
written, and SHALL leave the source unchanged.

#### Scenario: A timestamp tag refuses and is named

- **WHEN** `created` is replaced with `"2002-01-01"` in:

  ```yaml
  created: !!timestamp 2001-12-14
  ```

- **THEN** the replace fails with `Error::TagMismatch` whose `tag` is
  `!!timestamp`, displayed as
  `at /created: the tag !!timestamp does not fit the new value`, and the
  source is unchanged
