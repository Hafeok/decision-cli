#!/usr/bin/env bash
# TC-010
# Spec: .product/tests/TC-010-*.md
set -euo pipefail

cat >&2 <<MSG
TC-010 not yet implemented.

SIGKILL of dec mid-dispatch then restart must cause the outbox publisher to resume in-flight events on startup.

The dec binary, oxi-events crate, and code-writer worker are not yet
built. This script is a placeholder so `product verify` finds a failing
runner and the implementation pipeline can pick the TC up.
MSG
exit 1
