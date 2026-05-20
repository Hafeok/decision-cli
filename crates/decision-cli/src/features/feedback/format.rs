//! Shared rendering helpers for `dec feedback {…}` output (FT-033).
//!
//! Two helpers live here:
//!
//!   * [`truncate_evidence`] — char-boundary-safe truncation used by
//!     `list` and `show` to keep long evidence citations readable.
//!   * [`json_escape`] — minimal JSON-string escaper used by every
//!     subcommand's `--format json` path (we deliberately avoid pulling
//!     in `serde_json` here so the JSON output stays a thin shim over
//!     the existing key-value rendering).

/// Truncate `s` to `max` characters with an ellipsis suffix, honouring
/// UTF-8 char boundaries so the slice never panics on multi-byte text.
#[must_use]
pub(super) fn truncate_evidence(s: &str, max: usize) -> String {
    if s.len() <= max {
        return s.to_string();
    }
    let mut cut = max.saturating_sub(1);
    while cut > 0 && !s.is_char_boundary(cut) {
        cut -= 1;
    }
    format!("{}…", &s[..cut])
}

/// Escape `s` for inclusion in a JSON string literal. Supports the
/// minimum set of escapes required by RFC 8259 so the subcommand JSON
/// emitters can build records without a JSON library dependency.
#[must_use]
pub(super) fn json_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\u{08}' => out.push_str("\\b"),
            '\u{0c}' => out.push_str("\\f"),
            c if (c as u32) < 0x20 => {
                out.push_str(&format!("\\u{:04x}", c as u32));
            }
            c => out.push(c),
        }
    }
    out
}

/// Render an `Option<&str>` as a JSON string or `null`.
#[must_use]
pub(super) fn json_opt(s: Option<&str>) -> String {
    match s {
        Some(v) => format!("\"{}\"", json_escape(v)),
        None => "null".to_string(),
    }
}

/// CLI `--format` switch. Slice-3 ships `text` (default) and `json`;
/// later slices may add `ndjson` / `tsv`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputFormat {
    /// Human-readable aligned text. Default.
    Text,
    /// One-shot JSON object per command.
    Json,
}

impl Default for OutputFormat {
    fn default() -> Self {
        Self::Text
    }
}

impl OutputFormat {
    /// Parse the wire string. Unknown values resolve to `None` so callers
    /// can produce a "usage" error and exit 2 per FT-033 §Error handling.
    #[must_use]
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "text" => Some(Self::Text),
            "json" => Some(Self::Json),
            _ => None,
        }
    }

    /// True if this output should be JSON.
    #[must_use]
    pub const fn is_json(self) -> bool {
        matches!(self, Self::Json)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn truncate_preserves_short_strings() {
        assert_eq!(truncate_evidence("short", 80), "short");
    }

    #[test]
    fn truncate_keeps_utf8_boundaries() {
        let s = "héllo world this is a long string with utf8 héllo é";
        let out = truncate_evidence(s, 10);
        assert!(out.ends_with('…'));
        assert!(out.chars().count() <= 11);
    }

    #[test]
    fn json_escape_handles_specials() {
        assert_eq!(json_escape("a\"b\\c"), "a\\\"b\\\\c");
        assert_eq!(json_escape("line\nbreak"), "line\\nbreak");
        assert_eq!(json_escape("tab\there"), "tab\\there");
    }

    #[test]
    fn json_opt_emits_null_for_none() {
        assert_eq!(json_opt(None), "null");
        assert_eq!(json_opt(Some("x")), "\"x\"");
    }

    #[test]
    fn output_format_parse_handles_known_values() {
        assert_eq!(OutputFormat::parse("text"), Some(OutputFormat::Text));
        assert_eq!(OutputFormat::parse("json"), Some(OutputFormat::Json));
        assert_eq!(OutputFormat::parse("yaml"), None);
    }
}
