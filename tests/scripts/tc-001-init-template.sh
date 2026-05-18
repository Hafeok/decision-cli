#!/usr/bin/env bash
# TC-001
# Spec: .product/tests/TC-001-*.md
set -euo pipefail

cat >&2 <<MSG
TC-001 not yet implemented.

dec init --template engineering-development must produce a .dec/store/ with ValueStream + ValueAction reachable via SPARQL and PROV-O-linked to dec:session/init-001.

The dec binary, oxi-events crate, and code-writer worker are not yet
built. This script is a placeholder so `product verify` finds a failing
runner and the implementation pipeline can pick the TC up.
MSG
exit 1
