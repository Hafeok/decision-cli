//! Resolve a feature's TC list from its product-cli markdown artifact.
//!
//! The coverage primitive needs `Vec<TcId>` for a given `FT-NNN`.
//! product-cli stores that list in the feature's YAML frontmatter under
//! the `tests:` key (see `.product/features/FT-001-…md` for the shape).
//! Per FT-045 §Behaviour, resolution follows the same pattern
//! `features/implement/bundle.rs` uses — walk to the surrounding
//! `.product/` root, then read the matching file.
//!
//! No external YAML crate is pulled in for one tiny use-case. A
//! line-based parser handles both inline (`tests: [TC-A, TC-B]`) and
//! block (`tests:\n- TC-A\n- TC-B`) forms.

use std::fs;
use std::path::{Path, PathBuf};

use super::report::TcId;
use super::CoverageError;

/// IRI prefix for feature artifacts referenced via `dec:verifies`
/// (mirrors `features/verify_graph_new/resolve.rs`).
pub const IRI_FEATURE_PREFIX: &str = "https://decision-cli.dev/ns/feature/";
/// IRI prefix for test-criterion artifacts referenced via `dec:verifies`.
pub const IRI_TC_PREFIX: &str = "https://decision-cli.dev/ns/tc/";
/// IRI prefix for verification graphs.
pub const IRI_GRAPH_PREFIX: &str = "https://decision-cli.dev/ns/graph/";

/// Translate a short feature id (`FT-NNN`) into its IRI form.
#[must_use]
pub fn feature_iri_for(short_id: &str) -> String {
    if short_id.starts_with("https://") {
        short_id.to_string()
    } else {
        format!("{IRI_FEATURE_PREFIX}{short_id}")
    }
}

/// Translate a short TC id (`TC-NNN`) into its IRI form.
#[must_use]
pub fn tc_iri_for(short_id: &str) -> String {
    if short_id.starts_with("https://") {
        short_id.to_string()
    } else {
        format!("{IRI_TC_PREFIX}{short_id}")
    }
}

/// Translate a short graph id (`VG-NNN`) into its IRI form.
#[must_use]
pub fn graph_iri_for(short_id: &str) -> String {
    if short_id.starts_with("https://") {
        short_id.to_string()
    } else {
        format!("{IRI_GRAPH_PREFIX}{short_id}")
    }
}

/// Resolve the feature's TC list as full IRIs, in feature-spec
/// declaration order.
///
/// # Errors
/// Returns [`CoverageError::ArtifactNotFound`] if no
/// `.product/features/{feature}*.md` is found under `product_root`.
pub fn resolve_feature_tc_iris(
    product_root: &Path,
    feature_short_id: &str,
) -> Result<Vec<TcId>, CoverageError> {
    let shorts = resolve_feature_tcs_short(product_root, feature_short_id)?;
    Ok(shorts.iter().map(|s| tc_iri_for(s)).collect())
}

/// Resolve the feature's TC list as short ids (e.g. `["TC-068"]`).
///
/// Returns `Ok(Vec::new())` when the feature exists but lists no TCs.
///
/// # Errors
/// Returns [`CoverageError::ArtifactNotFound`] for unknown feature ids.
pub fn resolve_feature_tcs_short(
    product_root: &Path,
    feature_short_id: &str,
) -> Result<Vec<String>, CoverageError> {
    let path = find_feature_file(product_root, feature_short_id)?;
    let body = fs::read_to_string(&path).map_err(|e| CoverageError::StoreUnreachable {
        detail: format!("reading {p}: {e}", p = path.display()),
    })?;
    Ok(parse_tests_list(&body))
}

fn find_feature_file(product_root: &Path, feature_id: &str) -> Result<PathBuf, CoverageError> {
    let features_dir = product_root.join(".product").join("features");
    if !features_dir.exists() {
        return Err(artifact_not_found(feature_id));
    }
    let exact = features_dir.join(format!("{feature_id}.md"));
    if exact.is_file() {
        return Ok(exact);
    }
    if let Some(path) = scan_features_dir_for_match(&features_dir, feature_id)? {
        return Ok(path);
    }
    Err(artifact_not_found(feature_id))
}

fn artifact_not_found(feature_id: &str) -> CoverageError {
    CoverageError::ArtifactNotFound {
        kind: "Feature".to_string(),
        id: feature_id.to_string(),
    }
}

fn scan_features_dir_for_match(
    features_dir: &Path,
    feature_id: &str,
) -> Result<Option<PathBuf>, CoverageError> {
    let prefix = format!("{feature_id}-");
    let entries = fs::read_dir(features_dir).map_err(|e| CoverageError::StoreUnreachable {
        detail: format!("reading {d}: {e}", d = features_dir.display()),
    })?;
    for entry in entries {
        let entry = entry.map_err(|e| CoverageError::StoreUnreachable {
            detail: format!("walking {d}: {e}", d = features_dir.display()),
        })?;
        let name = entry.file_name();
        let Some(name) = name.to_str() else { continue };
        let Some(stem) = name.strip_suffix(".md") else {
            continue;
        };
        if stem == feature_id || stem.starts_with(&prefix) {
            return Ok(Some(entry.path()));
        }
    }
    Ok(None)
}

/// Extract the `tests:` list from a markdown body's YAML frontmatter.
/// Returns an empty vec if the frontmatter is absent or the key is
/// missing/empty.
fn parse_tests_list(body: &str) -> Vec<String> {
    let Some(fm) = extract_frontmatter(body) else {
        return Vec::new();
    };
    extract_yaml_list(fm, "tests")
}

fn extract_frontmatter(body: &str) -> Option<&str> {
    let rest = body
        .strip_prefix("---\n")
        .or_else(|| body.strip_prefix("---\r\n"))?;
    let end = rest.find("\n---")?;
    Some(&rest[..end])
}

fn extract_yaml_list(fm: &str, key: &str) -> Vec<String> {
    let mut lines = fm.lines().enumerate().peekable();
    let key_prefix = format!("{key}:");
    while let Some((_, line)) = lines.next() {
        let trimmed = line.trim_end();
        if !trimmed.starts_with(&key_prefix) {
            continue;
        }
        // skip past the key (also allow `tests :` etc.)
        let after = trimmed[key_prefix.len()..].trim();
        if let Some(inline) = after.strip_prefix('[').and_then(|s| s.strip_suffix(']')) {
            return parse_inline_list(inline);
        }
        if !after.is_empty() {
            return single_item_list(after);
        }
        return collect_block_list(&mut lines);
    }
    Vec::new()
}

fn parse_inline_list(inline: &str) -> Vec<String> {
    let mut out = Vec::new();
    for raw in inline.split(',') {
        let v = clean_item(raw);
        if !v.is_empty() {
            out.push(v);
        }
    }
    out
}

fn single_item_list(after: &str) -> Vec<String> {
    let v = clean_item(after);
    if v.is_empty() {
        Vec::new()
    } else {
        vec![v]
    }
}

fn collect_block_list<'a, I>(lines: &mut std::iter::Peekable<I>) -> Vec<String>
where
    I: Iterator<Item = (usize, &'a str)>,
{
    let mut out = Vec::new();
    while let Some((_, peek)) = lines.peek() {
        let t = peek.trim_end();
        if t.is_empty() {
            lines.next();
            continue;
        }
        let Some(item) = t.trim_start().strip_prefix("- ") else {
            // Reached a non-list, non-blank line — list block ends here.
            break;
        };
        if !(t.starts_with(char::is_whitespace) || t.starts_with("- ")) {
            break;
        }
        let v = clean_item(item);
        if !v.is_empty() {
            out.push(v);
        }
        lines.next();
    }
    out
}

fn clean_item(raw: &str) -> String {
    raw.trim()
        .trim_matches(|c: char| c == '"' || c == '\'')
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn block_list_form_yields_ordered_tcs() {
        let body = "---\nid: FT-X\ntitle: t\ntests:\n- TC-068\n- TC-069\nphase: 2\n---\nbody\n";
        let list = parse_tests_list(body);
        assert_eq!(list, vec!["TC-068".to_string(), "TC-069".to_string()]);
    }

    #[test]
    fn inline_list_form_yields_ordered_tcs() {
        let body = "---\nid: FT-X\ntests: [TC-001, TC-002]\n---\nbody\n";
        let list = parse_tests_list(body);
        assert_eq!(list, vec!["TC-001".to_string(), "TC-002".to_string()]);
    }

    #[test]
    fn empty_block_list_returns_empty_vec() {
        let body = "---\nid: FT-X\ntests:\nphase: 2\n---\nbody\n";
        let list = parse_tests_list(body);
        assert!(list.is_empty());
    }

    #[test]
    fn missing_key_returns_empty_vec() {
        let body = "---\nid: FT-X\nphase: 2\n---\nbody\n";
        let list = parse_tests_list(body);
        assert!(list.is_empty());
    }

    #[test]
    fn iri_prefixes_are_correct() {
        assert_eq!(
            feature_iri_for("FT-001"),
            "https://decision-cli.dev/ns/feature/FT-001"
        );
        assert_eq!(
            tc_iri_for("TC-068"),
            "https://decision-cli.dev/ns/tc/TC-068"
        );
        assert_eq!(
            graph_iri_for("VG-001"),
            "https://decision-cli.dev/ns/graph/VG-001"
        );
    }

    #[test]
    fn iri_passthrough_when_already_iri() {
        let f = "https://decision-cli.dev/ns/feature/FT-100";
        assert_eq!(feature_iri_for(f), f);
    }
}
