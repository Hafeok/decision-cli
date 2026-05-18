#!/usr/bin/env bash
# TC-010 — outbox crash recovery (FT-003)
# Spec: .product/tests/TC-010-outbox-resumes-in-flight-dispatch-after-crash.md
#
# Slice 1 stand-in: the chaos scenario (SIGKILL of `dec` then restart) is
# realised inside the oxi-events crate by dropping a writer mid-flight,
# re-opening over the same store, and asserting the outbox publisher's
# initial sweep delivers every stranded event. The runner config will
# graduate to a full process-supervised SIGKILL once the FT-009 / FT-011
# binary path lands and `dec implement` can drive a real dispatch.
set -euo pipefail

cd "$(dirname "$0")/../.."

exec cargo test \
    --quiet \
    -p oxi-events \
    --test tc_010_outbox_recovery \
    -- --nocapture --test-threads=1
