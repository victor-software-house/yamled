#!/usr/bin/env bash
#MISE description="GitHub Actions workflow lint"
# actionlint fails in a repository with no workflow, such as a fresh Bun
# render, so that case passes with a notice.
shopt -s nullglob
workflows=(.github/workflows/*.yml .github/workflows/*.yaml)
if [ "${#workflows[@]}" -eq 0 ]; then
  echo "no workflow in .github/workflows, nothing to lint"
  exit 0
fi
actionlint
