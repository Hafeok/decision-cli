#!/usr/bin/env bash
# TC-002
# Spec: .product/tests/TC-002-*.md
set -euo pipefail

cat >&2 <<MSG
TC-002 not yet implemented.

dec init --from <path>.ttl must record the source path and SHA-256 content hash on the bootstrap session.

The dec binary, oxi-events crate, and code-writer worker are not yet
built. This script is a placeholder so `product verify` finds a failing
runner and the implementation pipeline can pick the TC up.
MSG
exit 1
