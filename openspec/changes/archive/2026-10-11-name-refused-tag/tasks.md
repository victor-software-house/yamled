# Tasks

- [x] 1. Tag refusals return `Error::TagMismatch` with the tag. Proof:
  `a_tag_this_version_cannot_check_is_named_in_the_refusal` and the updated
  tag tests pass in `mise run verify` on the build host.

  2026-10-11 UTC. `mise run verify` on a build runner at
  [`8269d08`][main], with the yaml-test-suite submodule initialised: 107
  tests pass, including
  `a_tag_this_version_cannot_check_is_named_in_the_refusal` and the seven
  other tag tests in `tests/edits.rs`. The change shipped in
  [yamled 0.0.5][0.0.5] from [PR 13][yamled#13].
  The runner image lacked `node`, which the `specs` task needs for
  `openspec` and the mise config does not declare, so it was installed on
  the runner for that run.

[main]: https://github.com/victor-software-house/yamled/commit/8269d08
[0.0.5]: https://crates.io/crates/yamled/0.0.5
[yamled#13]: https://github.com/victor-software-house/yamled/pull/13
