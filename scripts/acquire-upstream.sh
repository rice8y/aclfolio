#!/usr/bin/env bash
# Optional developer-only acquisition. Never called by the library.
set -euo pipefail
cd "$(dirname "$0")/.."
revision=$(cat scripts/upstream-revision)
if [[ ! -d artifacts/upstream/.git ]]; then
  git clone --depth 1 --filter=blob:none --no-checkout https://github.com/acl-org/acl-anthology.git artifacts/upstream
  git -C artifacts/upstream sparse-checkout set data python
  git -C artifacts/upstream fetch --depth 1 --filter=blob:none origin "$revision"
  git -C artifacts/upstream checkout --detach "$revision"
fi
actual=$(git -C artifacts/upstream rev-parse HEAD)
if [[ "$actual" != "$revision" ]]; then
  echo "Existing artifacts/upstream is at $actual; expected $revision. No checkout was changed." >&2
  exit 1
fi
