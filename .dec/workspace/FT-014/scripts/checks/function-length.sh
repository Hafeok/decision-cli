#!/usr/bin/env bash
# scripts/checks/function-length.sh
#
# Enforces ADR-013 Rule 2 — Function Length Limit for Rust sources.
# Walks every *.rs file under crates/*/src/ and counts statement lines
# inside each `fn` body. Statement lines are non-blank lines that are
# not comment-only after stripping a trailing `//` comment.
#
# Tracks brace depth from the opening `{` of each function body. The
# function ends when depth returns to zero. Functions whose bodies
# exceed the hard limit produce ERROR diagnostics; those in the warning
# band produce WARN diagnostics. The script propagates the worst
# severity it sees as its exit code.
#
# Exit 0: no function body exceeds the hard limit. Warn-band offenders
#         (default 30..=40 statement lines), if any, are listed on stdout
#         as advisory `WARNING:` diagnostics but do not gate CI.
# Exit 1: at least one function body exceeds the hard limit (default 40).
# Exit 127: `awk` is not on PATH (precondition failure).
#
# Thresholds may be overridden via environment variables:
#   FN_LENGTH_HARD (default: 40)
#   FN_LENGTH_WARN (default: 30)
#
# Diagnostic messages go to stdout. Script-self errors go to stderr.
#
# The brace-depth heuristic strips single-line `//` comments and ignores
# braces inside string literals at the level of "no double-quoted runs".
# Raw strings (`r#"..."#`) and macro invocations using braces may produce
# minor miscounts; the script is intentionally simple over precise — the
# remedy for a borderline false-positive is to split the function, which
# is what the rule prescribes anyway.
set -euo pipefail

HARD_LIMIT=${FN_LENGTH_HARD:-40}
WARN_LIMIT=${FN_LENGTH_WARN:-30}

if ! command -v awk >/dev/null 2>&1; then
  echo "ERROR: awk not found on PATH (required for function-length scan)" >&2
  exit 127
fi

if REPO_ROOT="$(git rev-parse --show-toplevel 2>/dev/null)"; then
  cd "$REPO_ROOT"
fi

FILES="$( {
    find crates -path '*/src/*.rs' \
      -not -name 'tests.rs' \
      -not -path '*/tests/*' \
      -not -path '*/benches/*' 2>/dev/null || true
  } | sort -u )"

if [ -z "$FILES" ]; then
  echo "OK: no first-party Rust source files found"
  exit 0
fi

# Per-file awk program. Emits one line per offending function:
#   <severity>\t<file>\t<line>\t<name>\t<len>
# where severity is ERROR or WARN. The shell driver below aggregates
# the worst severity into the exit code.
AWK_PROG='
function strip_inline_comment(line,    pos, in_str, ch, i, out) {
  # Strip everything from a "//" that is not inside a double-quoted
  # string. Approximate: we ignore raw strings and char literals.
  in_str = 0
  out = ""
  i = 1
  while (i <= length(line)) {
    ch = substr(line, i, 1)
    if (ch == "\\" && in_str) {
      out = out substr(line, i, 2)
      i += 2
      continue
    }
    if (ch == "\"") {
      in_str = !in_str
      out = out ch
      i++
      continue
    }
    if (!in_str && ch == "/" && substr(line, i + 1, 1) == "/") {
      break
    }
    out = out ch
    i++
  }
  return out
}

function count_char(s, c,    n, i) {
  n = 0
  for (i = 1; i <= length(s); i++) if (substr(s, i, 1) == c) n++
  return n
}

function is_statement(line,    s) {
  s = line
  sub(/^[[:space:]]+/, "", s)
  sub(/[[:space:]]+$/, "", s)
  if (length(s) == 0) return 0
  # Treat block-comment delimiters and pure // lines as non-statement.
  if (substr(s, 1, 2) == "//") return 0
  if (substr(s, 1, 3) == "///") return 0
  if (substr(s, 1, 2) == "/*") return 0
  if (substr(s, 1, 1) == "*") return 0
  return 1
}

BEGIN {
  hard = HARD + 0
  warn = WARN + 0
  in_fn = 0
  depth = 0
  stmt = 0
  fn_name = ""
  fn_line = 0
  seeking_brace = 0
  cfg_test_depth = 0      # >0 while inside a #[cfg(test)] module body
  cfg_test_pending = 0    # 1 after seeing #[cfg(test)] before its mod brace
}

# Reset per-file state. FNR == 1 marks the start of every input file.
FNR == 1 {
  in_fn = 0
  depth = 0
  stmt = 0
  fn_name = ""
  fn_line = 0
  seeking_brace = 0
  cfg_test_depth = 0
  cfg_test_pending = 0
}

{
  raw = $0
  line = strip_inline_comment(raw)
  opens = count_char(line, "{")
  closes = count_char(line, "}")

  # Detect entry into a #[cfg(test)] module. We skip any function bodies
  # defined inside such a block — they are unit tests with the same
  # "scenarios are necessarily verbose" exemption ADR-013 names for the
  # `crates/*/tests/` integration directory.
  if (cfg_test_pending) {
    if (opens > 0) {
      cfg_test_depth = 1
      cfg_test_pending = 0
    }
    next
  }
  if (cfg_test_depth > 0) {
    cfg_test_depth += opens - closes
    if (cfg_test_depth <= 0) {
      cfg_test_depth = 0
    }
    next
  }
  if (match(line, /^[[:space:]]*#\[cfg\(test\)\]/)) {
    cfg_test_pending = 1
    next
  }

  if (!in_fn && !seeking_brace) {
    # Detect a fn header. Match the keyword `fn` with a leading word
    # boundary (start of line or whitespace) and an identifier after.
    if (match(line, /(^|[[:space:]])fn[[:space:]]+[A-Za-z_][A-Za-z0-9_]*/)) {
      hdr = substr(line, RSTART, RLENGTH)
      sub(/^.*fn[[:space:]]+/, "", hdr)
      fn_name = hdr
      fn_line = FNR
      if (opens > 0) {
        in_fn = 1
        depth = opens - closes
        stmt = 0
      } else {
        seeking_brace = 1
      }
    }
    next
  }

  if (seeking_brace) {
    # Between the fn header and its opening brace (e.g. multi-line
    # parameter list or where-clause). No statement lines counted yet.
    if (opens > 0) {
      seeking_brace = 0
      in_fn = 1
      depth = opens - closes
      stmt = 0
    }
    next
  }

  # Inside a function body.
  if (is_statement(line)) stmt++
  depth += opens - closes

  if (depth <= 0) {
    if (stmt > hard) {
      printf "ERROR\t%s\t%d\t%s\t%d\n", FILENAME, fn_line, fn_name, stmt
    } else if (stmt > warn) {
      printf "WARN\t%s\t%d\t%s\t%d\n", FILENAME, fn_line, fn_name, stmt
    }
    in_fn = 0
    depth = 0
    stmt = 0
    fn_name = ""
    fn_line = 0
  }
}
'

RESULTS="$(echo "$FILES" | xargs -r awk \
  -v HARD="$HARD_LIMIT" -v WARN="$WARN_LIMIT" "$AWK_PROG" || true)"

ERRORS="$(echo "$RESULTS" | awk -F'\t' '$1=="ERROR"' | sort -t $'\t' -k5 -rn || true)"
WARNS="$(echo "$RESULTS" | awk -F'\t' '$1=="WARN"' | sort -t $'\t' -k5 -rn || true)"

if [ -n "${ERRORS//[$' \t\n']/}" ]; then
  echo "ERROR: functions exceeding hard limit ($HARD_LIMIT statement lines):"
  echo "$ERRORS" | while IFS=$'\t' read -r _sev file lineno name len; do
    [ -z "$file" ] && continue
    echo "  $file:$lineno fn $name — $len statement lines (limit: $HARD_LIMIT)"
  done
  exit 1
fi

if [ -n "${WARNS//[$' \t\n']/}" ]; then
  echo "WARNING: functions approaching limit ($WARN_LIMIT-$HARD_LIMIT statement lines):"
  echo "$WARNS" | while IFS=$'\t' read -r _sev file lineno name len; do
    [ -z "$file" ] && continue
    echo "  $file:$lineno fn $name — $len statement lines (warn at: $WARN_LIMIT)"
  done
fi

echo "OK: all Rust functions within hard limit ($HARD_LIMIT)"
exit 0
