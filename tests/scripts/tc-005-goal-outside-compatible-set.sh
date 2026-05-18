#!/usr/bin/env bash
# TC-005
# Spec: .product/tests/TC-005-*.md
set -euo pipefail

cat >&2 <<MSG
TC-005 not yet implemented.

dec init --from <ttl-with-unauthorized-goal> must fail naming the goal and the ValueAction's compatible set, writing NO state.

The dec binary, oxi-events crate, and code-writer worker are not yet
built. This script is a placeholder so `product verify` finds a failing
runner and the implementation pipeline can pick the TC up.
MSG
exit 1
