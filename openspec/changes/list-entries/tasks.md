# Tasks

Every compile, lint and test step runs in the pull request's CI.

## 1. The entries

- [ ] 1.1 Hold a key that is not a scalar outside the entries, and list the
  entries of a mapping with `Node::entries`.
  Verify the new tests in `tests/index.rs` fail on the source before the
  change and pass after it.

## 2. The surfaces

- [ ] 2.1 Add the patch changeset and the requirements of this change to
  `location-index`.
  Verify `openspec validate --all --strict` passes and the changeset names
  `yamled: patch`.

## 3. Integration

- [ ] 3.1 Verify the pull request's CI passes.
