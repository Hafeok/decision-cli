//! Parsed components of an OCI referrer descriptor URI (FT-091 / ADR-059).

/// SHA-256 digests are 64 lowercase hex digits per RFC 6234 / the OCI image spec.
pub const SHA256_HEX_DIGITS: usize = 64;

/// Distilled components of a digest-pinned OCI reference suitable for
/// the SBOM referrer position.
///
/// Constructed by [`super::validate_oci_referrer_uri`]; callers obtain a
/// parsed [`OciReferrerUri`] only after the syntactic checks pass, so a
/// value of this type is "known well-formed" by construction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OciReferrerUri {
    /// Registry host (and optional port). E.g. `"ghcr.io"`,
    /// `"registry.example.com:5000"`.
    pub registry: String,
    /// Repository path. E.g. `"example/worker"`. Always at least one path
    /// segment; the validator refuses an empty repository.
    pub repository: String,
    /// Hex-encoded SHA-256 digest. Exactly [`SHA256_HEX_DIGITS`] lowercase
    /// hex chars; the validator strips the `sha256:` prefix before storing.
    pub digest_hex: String,
}

impl OciReferrerUri {
    /// Reassemble the canonical descriptor URI form.
    #[must_use]
    pub fn as_uri(&self) -> String {
        format!(
            "{registry}/{repository}@sha256:{digest_hex}",
            registry = self.registry,
            repository = self.repository,
            digest_hex = self.digest_hex,
        )
    }
}
