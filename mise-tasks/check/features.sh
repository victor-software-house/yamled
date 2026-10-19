#!/usr/bin/env bash
#MISE description="Lint the crate and its tests without the serde feature"
#MISE env={CARGO_TARGET_DIR = "target/features"}
# verify depends on check:features. A starter: the template writes this file
# once and never rewrites it. lint already covers every feature on, and
# without serde the crate needs granit-parser alone.
set -euo pipefail
cargo clippy --locked --no-default-features --all-targets -- -D warnings
