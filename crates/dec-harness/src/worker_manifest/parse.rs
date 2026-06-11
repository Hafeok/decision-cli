//! Minimal TOML reader for `worker.toml` (FT-093).
//!
//! decision-cli intentionally pulls no general-purpose TOML dependency
//! for this slice — the manifest shape is small, fixed, and easy to
//! parse with a hand-rolled scanner. Anything outside the declared shape
//! is refused at parse time so a typo surfaces with a structured
//! violation rather than silently loading defaults.
//!
//! Subset supported:
//! - `# comments`
//! - `[table]` headers (one level deep; no dotted tables).
//! - `key = "string"`
//! - `key = ["string", "string"]`
//! - Blank lines.
//!
//! Anything else (numbers, booleans, multi-line strings, nested arrays,
//! inline tables, dotted keys) is refused. The fixed manifest shape only
//! uses string scalars and string arrays — every other TOML feature is
//! out of scope for FT-093.

use std::collections::BTreeMap;

use thiserror::Error;

use super::types::{
    Capabilities, RuntimeKind, RuntimeSpec, WorkerManifest, WorkerSection,
    DEFAULT_WIRE_PROTOCOL_VERSION,
};

/// Failure modes when parsing `worker.toml`.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum ManifestParseError {
    /// A syntactic defect in the raw TOML stream (unexpected token,
    /// unterminated string, malformed array, etc.).
    #[error("worker.toml syntax error at line {line}: {detail}")]
    Syntax {
        /// 1-based line number where the defect was detected.
        line: usize,
        /// Operator-facing explanation.
        detail: String,
    },
    /// A required table is missing from the manifest.
    #[error("worker.toml missing required table [{table}]")]
    MissingTable {
        /// Table name (e.g. `"worker"`).
        table: String,
    },
    /// A required key is missing from a present table.
    #[error("worker.toml missing required key `{key}` in table [{table}]")]
    MissingKey {
        /// Table name (e.g. `"worker"`).
        table: String,
        /// Key name (e.g. `"name"`).
        key: String,
    },
    /// A key declared in the manifest is not part of the FT-093 shape.
    #[error("worker.toml unknown key `{key}` in table [{table}]")]
    UnknownKey {
        /// Table name.
        table: String,
        /// Offending key.
        key: String,
    },
    /// A key's declared value has the wrong shape (e.g. array where
    /// scalar expected).
    #[error("worker.toml key `{key}` in table [{table}] has wrong shape: {detail}")]
    WrongShape {
        /// Table name.
        table: String,
        /// Key name.
        key: String,
        /// Explanation.
        detail: String,
    },
    /// A value taken from the manifest is not in the controlled
    /// vocabulary (currently only `runtime.kind`).
    #[error(
        "worker.toml key `{key}` in table [{table}] carries unsupported value {value:?}: {detail}"
    )]
    UnsupportedValue {
        /// Table name.
        table: String,
        /// Key name.
        key: String,
        /// The offending raw value.
        value: String,
        /// Explanation.
        detail: String,
    },
}

/// Parse a `worker.toml` body into a [`WorkerManifest`].
pub fn parse_worker_manifest(raw: &str) -> Result<WorkerManifest, ManifestParseError> {
    let doc = scan(raw)?;
    lift(doc)
}

/// Intermediate representation: table → key → value (string or string-array).
type Document = BTreeMap<String, BTreeMap<String, RawValue>>;

#[derive(Debug, Clone, PartialEq, Eq)]
enum RawValue {
    String(String),
    Array(Vec<String>),
}

fn scan(raw: &str) -> Result<Document, ManifestParseError> {
    let mut doc: Document = BTreeMap::new();
    let mut current_table: Option<String> = None;

    for (idx, raw_line) in raw.lines().enumerate() {
        let line_no = idx + 1;
        let trimmed = strip_comment(raw_line).trim();
        if trimmed.is_empty() {
            continue;
        }

        if let Some(table) = parse_table_header(trimmed, line_no)? {
            doc.entry(table.clone()).or_default();
            current_table = Some(table);
            continue;
        }

        let table = current_table
            .clone()
            .ok_or_else(|| ManifestParseError::Syntax {
                line: line_no,
                detail: "key/value declared before any [table] header".to_string(),
            })?;
        let (key, value) = parse_kv(trimmed, line_no)?;
        let entry = doc.entry(table.clone()).or_default();
        if entry.contains_key(&key) {
            return Err(ManifestParseError::Syntax {
                line: line_no,
                detail: format!("duplicate key `{key}` in table [{table}]"),
            });
        }
        entry.insert(key, value);
    }
    Ok(doc)
}

fn strip_comment(line: &str) -> &str {
    // Toml comments outside of strings start with `#`. The manifest
    // never embeds `#` inside a string, so a substring split is safe
    // for this subset.
    line.split('#').next().unwrap_or("")
}

fn parse_table_header(s: &str, line: usize) -> Result<Option<String>, ManifestParseError> {
    if !s.starts_with('[') {
        return Ok(None);
    }
    let inner = s
        .strip_prefix('[')
        .and_then(|t| t.strip_suffix(']'))
        .ok_or_else(|| ManifestParseError::Syntax {
            line,
            detail: format!("malformed table header {s:?}; expected `[name]`"),
        })?;
    let name = inner.trim();
    if name.is_empty() || name.contains('.') || name.contains('[') || name.contains(']') {
        return Err(ManifestParseError::Syntax {
            line,
            detail: format!(
                "table header {s:?} must be a single bare name (dotted / nested tables are not supported by FT-093)"
            ),
        });
    }
    Ok(Some(name.to_string()))
}

fn parse_kv(s: &str, line: usize) -> Result<(String, RawValue), ManifestParseError> {
    let (raw_key, raw_value) = s
        .split_once('=')
        .ok_or_else(|| ManifestParseError::Syntax {
            line,
            detail: format!("missing `=` in key/value line {s:?}"),
        })?;
    let key = raw_key.trim();
    if key.is_empty() || key.contains('.') || key.contains('[') {
        return Err(ManifestParseError::Syntax {
            line,
            detail: format!("malformed key {raw_key:?}"),
        });
    }
    let value = parse_value(raw_value.trim(), line)?;
    Ok((key.to_string(), value))
}

fn parse_value(s: &str, line: usize) -> Result<RawValue, ManifestParseError> {
    if let Some(rest) = s.strip_prefix('[') {
        let body = rest
            .strip_suffix(']')
            .ok_or_else(|| ManifestParseError::Syntax {
                line,
                detail: format!("array value {s:?} missing closing `]`"),
            })?;
        return Ok(RawValue::Array(parse_string_array(body, line)?));
    }
    if let Some(rest) = s.strip_prefix('"') {
        let value = rest
            .strip_suffix('"')
            .ok_or_else(|| ManifestParseError::Syntax {
                line,
                detail: format!("string value {s:?} missing closing `\"`"),
            })?;
        if value.contains('"') {
            return Err(ManifestParseError::Syntax {
                line,
                detail: format!(
                    "embedded `\"` in string {value:?} is not supported by FT-093's minimal parser"
                ),
            });
        }
        return Ok(RawValue::String(value.to_string()));
    }
    Err(ManifestParseError::Syntax {
        line,
        detail: format!(
            "unsupported value {s:?}; only `\"string\"` and `[\"string\", \"string\"]` are accepted"
        ),
    })
}

fn parse_string_array(body: &str, line: usize) -> Result<Vec<String>, ManifestParseError> {
    let trimmed = body.trim();
    if trimmed.is_empty() {
        return Ok(Vec::new());
    }
    let mut out = Vec::new();
    for piece in trimmed.split(',') {
        let item = piece.trim();
        if item.is_empty() {
            return Err(ManifestParseError::Syntax {
                line,
                detail: format!("trailing or empty entry in string array {body:?}"),
            });
        }
        let stripped = item
            .strip_prefix('"')
            .and_then(|t| t.strip_suffix('"'))
            .ok_or_else(|| ManifestParseError::Syntax {
                line,
                detail: format!("array entry {item:?} is not a quoted string"),
            })?;
        if stripped.contains('"') {
            return Err(ManifestParseError::Syntax {
                line,
                detail: format!(
                    "embedded `\"` in array entry {stripped:?} is not supported by FT-093's minimal parser"
                ),
            });
        }
        out.push(stripped.to_string());
    }
    Ok(out)
}

fn lift(mut doc: Document) -> Result<WorkerManifest, ManifestParseError> {
    let worker =
        lift_worker(
            doc.remove("worker")
                .ok_or_else(|| ManifestParseError::MissingTable {
                    table: "worker".to_string(),
                })?,
        )?;
    let capabilities = lift_capabilities(doc.remove("capabilities").ok_or_else(|| {
        ManifestParseError::MissingTable {
            table: "capabilities".to_string(),
        }
    })?)?;
    let runtime =
        lift_runtime(
            doc.remove("runtime")
                .ok_or_else(|| ManifestParseError::MissingTable {
                    table: "runtime".to_string(),
                })?,
        )?;
    if let Some((extra, _)) = doc.into_iter().next() {
        return Err(ManifestParseError::Syntax {
            line: 0,
            detail: format!(
                "unknown table [{extra}] declared in worker.toml; FT-093 manifests have exactly three tables: [worker], [capabilities], [runtime]"
            ),
        });
    }
    Ok(WorkerManifest {
        worker,
        capabilities,
        runtime,
    })
}

fn lift_worker(mut table: BTreeMap<String, RawValue>) -> Result<WorkerSection, ManifestParseError> {
    let name = take_string(&mut table, "worker", "name")?;
    let sdk_version = take_string(&mut table, "worker", "sdk_version")?;
    let wire_protocol = take_optional_string(&mut table, "worker", "wire_protocol")?
        .unwrap_or_else(|| DEFAULT_WIRE_PROTOCOL_VERSION.to_string());
    reject_extras(&table, "worker")?;
    Ok(WorkerSection {
        name,
        sdk_version,
        wire_protocol,
    })
}

fn lift_capabilities(
    mut table: BTreeMap<String, RawValue>,
) -> Result<Capabilities, ManifestParseError> {
    let tags = take_array(&mut table, "capabilities", "tags")?;
    let compatible_roles =
        take_optional_array(&mut table, "capabilities", "compatible_roles")?.unwrap_or_default();
    reject_extras(&table, "capabilities")?;
    Ok(Capabilities {
        tags,
        compatible_roles,
    })
}

fn lift_runtime(mut table: BTreeMap<String, RawValue>) -> Result<RuntimeSpec, ManifestParseError> {
    let raw_kind = take_string(&mut table, "runtime", "kind")?;
    let entrypoint = take_string(&mut table, "runtime", "entrypoint")?;
    reject_extras(&table, "runtime")?;
    let kind = RuntimeKind::try_from_str(&raw_kind).ok_or_else(|| {
        ManifestParseError::UnsupportedValue {
            table: "runtime".to_string(),
            key: "kind".to_string(),
            value: raw_kind.clone(),
            detail: "FT-093 accepts `subscribed` (slice 1) or `invoked` (reserved for Dagger per ADR-065)".to_string(),
        }
    })?;
    Ok(RuntimeSpec { kind, entrypoint })
}

fn take_string(
    table: &mut BTreeMap<String, RawValue>,
    table_name: &str,
    key: &str,
) -> Result<String, ManifestParseError> {
    take_optional_string(table, table_name, key)?.ok_or_else(|| ManifestParseError::MissingKey {
        table: table_name.to_string(),
        key: key.to_string(),
    })
}

fn take_optional_string(
    table: &mut BTreeMap<String, RawValue>,
    table_name: &str,
    key: &str,
) -> Result<Option<String>, ManifestParseError> {
    match table.remove(key) {
        None => Ok(None),
        Some(RawValue::String(s)) => Ok(Some(s)),
        Some(RawValue::Array(_)) => Err(ManifestParseError::WrongShape {
            table: table_name.to_string(),
            key: key.to_string(),
            detail: "expected a quoted string, got an array".to_string(),
        }),
    }
}

fn take_array(
    table: &mut BTreeMap<String, RawValue>,
    table_name: &str,
    key: &str,
) -> Result<Vec<String>, ManifestParseError> {
    take_optional_array(table, table_name, key)?.ok_or_else(|| ManifestParseError::MissingKey {
        table: table_name.to_string(),
        key: key.to_string(),
    })
}

fn take_optional_array(
    table: &mut BTreeMap<String, RawValue>,
    table_name: &str,
    key: &str,
) -> Result<Option<Vec<String>>, ManifestParseError> {
    match table.remove(key) {
        None => Ok(None),
        Some(RawValue::Array(v)) => Ok(Some(v)),
        Some(RawValue::String(_)) => Err(ManifestParseError::WrongShape {
            table: table_name.to_string(),
            key: key.to_string(),
            detail: "expected an array of quoted strings, got a string scalar".to_string(),
        }),
    }
}

fn reject_extras(
    table: &BTreeMap<String, RawValue>,
    table_name: &str,
) -> Result<(), ManifestParseError> {
    if let Some(key) = table.keys().next() {
        return Err(ManifestParseError::UnknownKey {
            table: table_name.to_string(),
            key: key.clone(),
        });
    }
    Ok(())
}
