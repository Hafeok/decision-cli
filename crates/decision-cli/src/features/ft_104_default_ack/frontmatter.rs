//! `adrs-rejected:` frontmatter parser.
//!
//! Per-feature opt-out for a default-acknowledged cross-cutting ADR.
//! Each entry MUST carry a non-empty `reason` string. Empty reasons are
//! a malformed-feature error so the gap surfaces in `product feature
//! show` rather than silently slipping past preflight.
//!
//! The shape is a YAML list of `{ id, reason }` records:
//!
//! ```yaml
//! adrs-rejected:
//!   - id: ADR-013
//!     reason: "This feature deliberately bypasses graph-as-state because <reason>."
//! ```

use std::fmt;

/// One `adrs-rejected:` entry parsed from a feature's frontmatter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RejectedAdr {
    /// ADR id, e.g. `"ADR-013"`.
    pub id: String,
    /// Operator-supplied rationale. Always non-empty after parsing —
    /// empty `reason` strings surface as
    /// [`AdrsRejectedError::EmptyReason`] from [`parse_adrs_rejected`].
    pub reason: String,
}

/// Errors raised while parsing the `adrs-rejected:` frontmatter block.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AdrsRejectedError {
    /// An entry was missing its `id:` field.
    MissingId {
        /// 1-based line number where the malformed entry began.
        line: usize,
    },
    /// An entry had an empty or whitespace-only `reason:` field.
    EmptyReason {
        /// ADR id whose reason was empty.
        adr_id: String,
        /// 1-based line number of the entry.
        line: usize,
    },
    /// An entry was missing its `reason:` field entirely. Carrying a
    /// rationale is invariant 2 of FT-104.
    MissingReason {
        /// ADR id missing the reason.
        adr_id: String,
        /// 1-based line number of the entry.
        line: usize,
    },
}

impl fmt::Display for AdrsRejectedError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingId { line } => write!(
                f,
                "adrs-rejected entry at line {line} is missing its `id:` field"
            ),
            Self::EmptyReason { adr_id, line } => write!(
                f,
                "adrs-rejected[{adr_id}] at line {line}: `reason` must be non-empty \
                 (FT-104 invariant: per-feature rejection requires a rationale)"
            ),
            Self::MissingReason { adr_id, line } => write!(
                f,
                "adrs-rejected[{adr_id}] at line {line}: missing `reason:` field"
            ),
        }
    }
}

impl std::error::Error for AdrsRejectedError {}

/// Parse the `adrs-rejected:` list out of a YAML frontmatter document.
///
/// `body` is the *frontmatter* block (the text between the leading
/// `---` and trailing `---` of a `.product/features/FT-XXX-*.md`). The
/// parser scans for an `adrs-rejected:` key at column 0 and reads
/// indented list entries until the next column-0 key or the end of the
/// frontmatter.
///
/// Returns an empty vector when the key is absent — that is the
/// pre-FT-104 default. Returns an error for malformed entries; callers
/// should surface the error verbatim so operators can repair the
/// feature file.
pub fn parse_adrs_rejected(body: &str) -> Result<Vec<RejectedAdr>, AdrsRejectedError> {
    let lines: Vec<&str> = body.lines().collect();
    let Some(start) = find_block_start(&lines) else {
        return Ok(Vec::new());
    };
    let mut out = Vec::new();
    let mut cursor = start + 1;
    let mut entry: Option<PartialEntry> = None;
    while cursor < lines.len() {
        let raw = lines[cursor];
        if is_top_level_key(raw) && !raw.trim_start().starts_with('-') {
            break;
        }
        let trimmed = raw.trim_start();
        if trimmed.is_empty() {
            cursor += 1;
            continue;
        }
        let indent = raw.len() - trimmed.len();
        if trimmed.starts_with('-') {
            if let Some(prev) = entry.take() {
                out.push(prev.finalize()?);
            }
            let after_dash = trimmed.trim_start_matches('-').trim_start();
            let mut new_entry = PartialEntry::new(cursor + 1);
            apply_field(&mut new_entry, after_dash);
            entry = Some(new_entry);
            cursor += 1;
            continue;
        }
        // Continuation line for the current entry. Must be indented
        // further than the `-` row above.
        if indent == 0 {
            break;
        }
        if let Some(current) = entry.as_mut() {
            apply_field(current, trimmed);
        }
        cursor += 1;
    }
    if let Some(prev) = entry {
        out.push(prev.finalize()?);
    }
    Ok(out)
}

struct PartialEntry {
    id: Option<String>,
    reason: Option<String>,
    line: usize,
}

impl PartialEntry {
    fn new(line: usize) -> Self {
        Self {
            id: None,
            reason: None,
            line,
        }
    }
    fn finalize(self) -> Result<RejectedAdr, AdrsRejectedError> {
        let id = self
            .id
            .ok_or(AdrsRejectedError::MissingId { line: self.line })?;
        let reason = self
            .reason
            .ok_or_else(|| AdrsRejectedError::MissingReason {
                adr_id: id.clone(),
                line: self.line,
            })?;
        if reason.trim().is_empty() {
            return Err(AdrsRejectedError::EmptyReason {
                adr_id: id,
                line: self.line,
            });
        }
        Ok(RejectedAdr { id, reason })
    }
}

fn apply_field(entry: &mut PartialEntry, fragment: &str) {
    let Some((key, value)) = fragment.split_once(':') else {
        return;
    };
    let key = key.trim();
    let value = strip_yaml_string(value.trim());
    match key {
        "id" => entry.id = Some(value),
        "reason" => entry.reason = Some(value),
        _ => {}
    }
}

fn strip_yaml_string(s: &str) -> String {
    let s = s.trim();
    if (s.starts_with('"') && s.ends_with('"') && s.len() >= 2)
        || (s.starts_with('\'') && s.ends_with('\'') && s.len() >= 2)
    {
        s[1..s.len() - 1].to_string()
    } else {
        s.to_string()
    }
}

fn find_block_start(lines: &[&str]) -> Option<usize> {
    for (idx, line) in lines.iter().enumerate() {
        if line.starts_with("adrs-rejected:") {
            return Some(idx);
        }
    }
    None
}

fn is_top_level_key(line: &str) -> bool {
    if line.is_empty() {
        return false;
    }
    let first = line.chars().next().unwrap_or(' ');
    first != ' ' && first != '\t' && line.contains(':')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn absent_block_returns_empty() {
        let body = "id: FT-001\ntitle: x\n";
        assert!(parse_adrs_rejected(body).expect("parse").is_empty());
    }

    #[test]
    fn single_entry_parses() {
        let body = "id: FT-001\n\
                    adrs-rejected:\n  \
                    - id: ADR-013\n    \
                    reason: \"because reasons\"\n";
        let got = parse_adrs_rejected(body).expect("parse");
        assert_eq!(
            got,
            vec![RejectedAdr {
                id: "ADR-013".into(),
                reason: "because reasons".into(),
            }]
        );
    }

    #[test]
    fn multiple_entries_parse() {
        let body = "adrs-rejected:\n  \
                    - id: ADR-013\n    \
                    reason: \"first reason\"\n  \
                    - id: ADR-020\n    \
                    reason: 'second reason'\n\
                    tests: []\n";
        let got = parse_adrs_rejected(body).expect("parse");
        assert_eq!(got.len(), 2);
        assert_eq!(got[0].id, "ADR-013");
        assert_eq!(got[1].id, "ADR-020");
        assert_eq!(got[1].reason, "second reason");
    }

    #[test]
    fn empty_reason_is_an_error() {
        let body = "adrs-rejected:\n  - id: ADR-013\n    reason: \"\"\n";
        let err = parse_adrs_rejected(body).expect_err("empty reason should fail");
        assert!(matches!(err, AdrsRejectedError::EmptyReason { .. }));
        assert!(err.to_string().contains("ADR-013"));
        assert!(err.to_string().contains("reason"));
    }

    #[test]
    fn whitespace_only_reason_is_an_error() {
        let body = "adrs-rejected:\n  - id: ADR-013\n    reason: \"   \"\n";
        let err = parse_adrs_rejected(body).expect_err("whitespace-only");
        assert!(matches!(err, AdrsRejectedError::EmptyReason { .. }));
    }

    #[test]
    fn missing_id_is_an_error() {
        let body = "adrs-rejected:\n  - reason: \"orphan reason\"\n";
        let err = parse_adrs_rejected(body).expect_err("missing id");
        assert!(matches!(err, AdrsRejectedError::MissingId { .. }));
    }
}
