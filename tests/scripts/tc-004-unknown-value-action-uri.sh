#!/usr/bin/env bash
# TC-004
# Spec: .product/tests/TC-004-*.md
set -euo pipefail

cat >&2 <<MSG
TC-004 not yet implemented.

dec init --from <ttl-with-unknown-URI> must fail naming the unresolvable URI and write NO state.

The dec binary, oxi-events crate, and code-writer worker are not yet
built. This script is a placeholder so `product verify` finds a failing
runner and the implementation pipeline can pick the TC up.
MSG
exit 1
