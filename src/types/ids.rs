//! BuildId - Unique identifier for crate documentation builds
//!
//! This module provides a strongly-typed identifier for documentation builds using
//! ULIDs (Universally Unique Lexicographically Sortable Identifiers) from the mti crate.
//!
//! # Design Rationale
//!
//! - **Time-sortable**: ULIDs encode timestamp in first 48 bits, enabling natural chronological ordering
//! - **Collision-resistant**: 128-bit entropy provides practical uniqueness guarantees
//! - **Human-readable**: Base32 encoding (e.g., `build_01ARZ3NDEKTSV4RRFFQ69G5FAV`)
//! - **Database-friendly**: String format suitable for primary keys and foreign keys
//!
//! # Format
//!
//! BuildIds follow the format: `build_<ULID>`
//!
//! Example: `build_01HZQKR9VF8P6QXWM7YJDG2K4N`
//!
//! # Usage
//!
//! ```rust
//! use crately::types::BuildId;
//!
//! // Generate a new build ID
//! let build_id = BuildId::new();
//! assert!(build_id.as_str().starts_with("build_"));
//!
//! // Parse from string (validation included)
//! let parsed = BuildId::try_from("build_01HZQKR9VF8P6QXWM7YJDG2K4N".to_string())?;
//! assert_eq!(parsed.as_str(), "build_01HZQKR9VF8P6QXWM7YJDG2K4N");
//! # Ok::<(), crately::types::BuildIdError>(())
//! ```

use serde::{Deserialize, Serialize};
use ulid::Ulid;
use std::fmt;
use thiserror::Error;

/// Unique identifier for a crate documentation build
///
/// BuildId uses ULIDs to provide time-sortable, globally unique identifiers for builds.
/// Each build of a crate (potentially with different features) gets a unique BuildId.
///
/// # Properties
///
/// - **Uniqueness**: 128-bit ULID provides collision resistance
/// - **Sortability**: Lexicographically sorts by creation time
/// - **Format**: `build_<ULID>` (e.g., `build_01HZQKR9VF8P6QXWM7YJDG2K4N`)
/// - **Validation**: Enforces prefix and ULID structure
///
/// # Examples
///
/// ```rust
/// use crately::types::BuildId;
///
/// // Generate new ID
/// let id = BuildId::new();
/// println!("Created build: {}", id);
///
/// // Parse from database
/// let stored = "build_01HZQKR9VF8P6QXWM7YJDG2K4N";
/// let id = BuildId::try_from(stored.to_string())?;
/// assert_eq!(id.as_str(), stored);
/// # Ok::<(), crately::types::BuildIdError>(())
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct BuildId(String);

impl BuildId {
    /// Prefix for all BuildId strings
    const PREFIX: &'static str = "build_";

    /// Generate a new BuildId with current timestamp
    ///
    /// Creates a fresh ULID and formats it with the `build_` prefix.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use crately::types::BuildId;
    ///
    /// let id = BuildId::new();
    /// assert!(id.as_str().starts_with("build_"));
    /// assert_eq!(id.as_str().len(), 32); // "build_" (6) + ULID (26)
    /// ```
    pub fn new() -> Self {
        let ulid = Ulid::new();
        Self(format!("{}{}", Self::PREFIX, ulid))
    }

    /// Generate a content-addressable BuildId for a crate + features combination
    ///
    /// This method creates a deterministic BuildId based on the crate specifier
    /// and features. Identical inputs (same crate name, version, and features)
    /// will always produce the same BuildId, enabling content deduplication and
    /// idempotent build operations.
    ///
    /// # Arguments
    ///
    /// * `specifier` - The crate name and version
    /// * `features` - The feature flags enabled for this build
    ///
    /// # Returns
    ///
    /// A deterministic BuildId that uniquely identifies this build configuration.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use crately::types::BuildId;
    /// use crately::crate_specifier::CrateSpecifier;
    /// use std::str::FromStr;
    ///
    /// let specifier = CrateSpecifier::from_str("serde@1.0.0").unwrap();
    /// let features = vec!["derive".to_string()];
    ///
    /// // Same inputs produce same BuildId
    /// let id1 = BuildId::generate(&specifier, &features);
    /// let id2 = BuildId::generate(&specifier, &features);
    /// assert_eq!(id1, id2);
    ///
    /// // Different features produce different BuildIds
    /// let features2 = vec!["alloc".to_string()];
    /// let id3 = BuildId::generate(&specifier, &features2);
    /// assert_ne!(id1, id3);
    /// ```
    pub fn generate(specifier: &crate::crate_specifier::CrateSpecifier, features: &[String]) -> Self {
        use blake3;

        // Create canonical representation for deterministic hashing
        let mut canonical = String::new();
        canonical.push_str(specifier.name());
        canonical.push('@');
        canonical.push_str(&specifier.version().to_string());

        // Sort features for deterministic ordering
        let mut sorted_features = features.to_vec();
        sorted_features.sort();

        if !sorted_features.is_empty() {
            canonical.push('[');
            canonical.push_str(&sorted_features.join(","));
            canonical.push(']');
        }

        // Generate content-addressable hash using BLAKE3
        let hash = blake3::hash(canonical.as_bytes());
        // Use first 128 bits (16 bytes) = 32 hex characters for reasonable length
        let hash_hex = hash.to_hex();
        let short_hash = &hash_hex[..32];

        Self(format!("{}{}", Self::PREFIX, short_hash))
    }

    /// Get the BuildId as a string slice
    ///
    /// Returns the full formatted ID including prefix.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use crately::types::BuildId;
    ///
    /// let id = BuildId::new();
    /// let s: &str = id.as_str();
    /// assert!(s.starts_with("build_"));
    /// ```
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Extract the ULID portion (without prefix)
    ///
    /// Returns the raw ULID string without the `build_` prefix.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use crately::types::BuildId;
    ///
    /// let id = BuildId::try_from("build_01HZQKR9VF8P6QXWM7YJDG2K4N".to_string())?;
    /// assert_eq!(id.ulid_str(), "01HZQKR9VF8P6QXWM7YJDG2K4N");
    /// # Ok::<(), crately::types::BuildIdError>(())
    /// ```
    pub fn ulid_str(&self) -> &str {
        &self.0[Self::PREFIX.len()..]
    }

    /// Parse and validate a ULID from the ID string
    ///
    /// Extracts the ULID portion and parses it into a `Ulid` instance.
    ///
    /// # Errors
    ///
    /// Returns `BuildIdError::InvalidUlid` if the ULID portion cannot be parsed.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use crately::types::BuildId;
    ///
    /// let id = BuildId::new();
    /// let ulid = id.parse_ulid()?;
    /// assert!(ulid.timestamp_ms() > 0);
    /// # Ok::<(), crately::types::BuildIdError>(())
    /// ```
    pub fn parse_ulid(&self) -> Result<Ulid, BuildIdError> {
        Ulid::from_string(self.ulid_str()).map_err(|e| BuildIdError::InvalidUlid(e.to_string()))
    }
}

impl Default for BuildId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for BuildId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl TryFrom<String> for BuildId {
    type Error = BuildIdError;

    /// Parse a BuildId from a string with validation
    ///
    /// Validates that:
    /// 1. String starts with `build_` prefix
    /// 2. Remaining characters form a valid ULID (26 chars, base32)
    ///
    /// # Errors
    ///
    /// - `BuildIdError::MissingPrefix` - String doesn't start with `build_`
    /// - `BuildIdError::InvalidUlid` - ULID portion is malformed
    ///
    /// # Examples
    ///
    /// ```rust
    /// use crately::types::BuildId;
    ///
    /// // Valid BuildId
    /// let id = BuildId::try_from("build_01HZQKR9VF8P6QXWM7YJDG2K4N".to_string())?;
    /// assert_eq!(id.as_str(), "build_01HZQKR9VF8P6QXWM7YJDG2K4N");
    ///
    /// // Missing prefix
    /// assert!(BuildId::try_from("01HZQKR9VF8P6QXWM7YJDG2K4N".to_string()).is_err());
    ///
    /// // Invalid ULID
    /// assert!(BuildId::try_from("build_INVALID".to_string()).is_err());
    /// # Ok::<(), crately::types::BuildIdError>(())
    /// ```
    fn try_from(s: String) -> Result<Self, Self::Error> {
        if !s.starts_with(Self::PREFIX) {
            return Err(BuildIdError::MissingPrefix(s));
        }

        let id_portion = &s[Self::PREFIX.len()..];

        // Validate ID portion: must be either ULID (26 chars) or content-hash (32 hex chars)
        if id_portion.len() == 26 {
            // ULID format - validate it
            Ulid::from_string(id_portion).map_err(|e| BuildIdError::InvalidUlid(e.to_string()))?;
        } else if id_portion.len() == 32 {
            // Content-addressable hash format - validate hex
            if !id_portion.chars().all(|c| c.is_ascii_hexdigit()) {
                return Err(BuildIdError::InvalidUlid(
                    "Content-addressable hash must be 32 hexadecimal characters".to_string()
                ));
            }
        } else {
            return Err(BuildIdError::InvalidUlid(format!(
                "BuildId must be either 26-char ULID or 32-char content hash, got {} chars",
                id_portion.len()
            )));
        }

        Ok(Self(s))
    }
}

impl std::str::FromStr for BuildId {
    type Err = BuildIdError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        BuildId::try_from(s.to_string())
    }
}

/// Errors that can occur when working with BuildId
#[derive(Debug, Error)]
pub enum BuildIdError {
    /// BuildId string missing required `build_` prefix
    #[error("BuildId missing 'build_' prefix: {0}")]
    MissingPrefix(String),

    /// ULID portion of BuildId is invalid
    #[error("Invalid ULID in BuildId: {0}")]
    InvalidUlid(String),
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    #[test]
    fn test_build_id_new() {
        let id = BuildId::new();
        assert!(id.as_str().starts_with("build_"));
        assert_eq!(id.as_str().len(), 32); // "build_" (6) + ULID (26)
    }

    #[test]
    fn test_build_id_uniqueness() {
        let id1 = BuildId::new();
        let id2 = BuildId::new();
        assert_ne!(id1, id2, "BuildIds should be unique");
    }

    #[test]
    fn test_build_id_ulid_str() {
        let id = BuildId::new();
        let ulid_part = id.ulid_str();
        assert_eq!(ulid_part.len(), 26);
        assert!(!ulid_part.contains("build_"));
    }

    #[test]
    fn test_build_id_parse_ulid() {
        let id = BuildId::new();
        let ulid = id.parse_ulid().expect("Should parse ULID");
        assert!(ulid.timestamp_ms() > 0);
    }

    #[test]
    fn test_build_id_try_from_valid() {
        let valid_id = "build_01HZQKR9VF8P6QXWM7YJDG2K4N";
        let id = BuildId::try_from(valid_id.to_string()).expect("Should parse valid BuildId");
        assert_eq!(id.as_str(), valid_id);
        assert_eq!(id.ulid_str(), "01HZQKR9VF8P6QXWM7YJDG2K4N");
    }

    #[test]
    fn test_build_id_try_from_missing_prefix() {
        let result = BuildId::try_from("01HZQKR9VF8P6QXWM7YJDG2K4N".to_string());
        assert!(matches!(result, Err(BuildIdError::MissingPrefix(_))));
    }

    #[test]
    fn test_build_id_try_from_invalid_ulid() {
        let result = BuildId::try_from("build_INVALID123".to_string());
        assert!(matches!(result, Err(BuildIdError::InvalidUlid(_))));

        let result = BuildId::try_from("build_".to_string());
        assert!(matches!(result, Err(BuildIdError::InvalidUlid(_))));
    }

    #[test]
    fn test_build_id_from_str() {
        let valid_id = "build_01HZQKR9VF8P6QXWM7YJDG2K4N";
        let id = BuildId::from_str(valid_id).expect("Should parse from str");
        assert_eq!(id.as_str(), valid_id);
    }

    #[test]
    fn test_build_id_display() {
        let id = BuildId::new();
        let displayed = format!("{}", id);
        assert!(displayed.starts_with("build_"));
        assert_eq!(displayed, id.as_str());
    }

    #[test]
    fn test_build_id_clone_and_equality() {
        let id1 = BuildId::new();
        let id2 = id1.clone();
        assert_eq!(id1, id2);

        let id3 = BuildId::new();
        assert_ne!(id1, id3);
    }

    #[test]
    fn test_build_id_serde_roundtrip() {
        let original = BuildId::new();
        let json = serde_json::to_string(&original).expect("Should serialize");
        let deserialized: BuildId = serde_json::from_str(&json).expect("Should deserialize");
        assert_eq!(original, deserialized);
    }

    #[test]
    fn test_build_id_is_send_sync() {
        fn assert_send<T: Send>() {}
        fn assert_sync<T: Sync>() {}
        assert_send::<BuildId>();
        assert_sync::<BuildId>();
    }

    #[test]
    fn test_build_id_hash() {
        use std::collections::HashMap;
        let mut map = HashMap::new();
        let id = BuildId::new();
        map.insert(id.clone(), "test");
        assert_eq!(map.get(&id), Some(&"test"));
    }

    #[test]
    fn test_build_id_default() {
        let id = BuildId::default();
        assert!(id.as_str().starts_with("build_"));
    }

    #[test]
    fn test_build_id_error_display() {
        let err = BuildIdError::MissingPrefix("test".to_string());
        let msg = format!("{}", err);
        assert!(msg.contains("build_"));
        assert!(msg.contains("test"));

        let err = BuildIdError::InvalidUlid("bad_ulid".to_string());
        let msg = format!("{}", err);
        assert!(msg.contains("Invalid ULID"));
    }

    #[test]
    fn test_build_id_lexicographic_sorting() {
        // Sleep briefly to ensure different timestamps
        use std::thread;
        use std::time::Duration;

        let id1 = BuildId::new();
        thread::sleep(Duration::from_millis(10));
        let id2 = BuildId::new();

        // ULIDs encode timestamp, so newer IDs should be lexicographically greater
        assert!(id1.as_str() < id2.as_str(), "ULIDs should sort chronologically");
    }

    #[test]
    fn test_build_id_generate_deterministic() {
        use crate::crate_specifier::CrateSpecifier;

        let spec = CrateSpecifier::from_str("serde@1.0.0").unwrap();
        let features = vec!["derive".to_string()];

        let id1 = BuildId::generate(&spec, &features);
        let id2 = BuildId::generate(&spec, &features);

        assert_eq!(id1, id2, "Same inputs must produce same BuildId");
    }

    #[test]
    fn test_build_id_generate_feature_order_normalized() {
        use crate::crate_specifier::CrateSpecifier;

        let spec = CrateSpecifier::from_str("tokio@1.35.0").unwrap();

        let id1 = BuildId::generate(&spec, &["full".to_string(), "rt".to_string()]);
        let id2 = BuildId::generate(&spec, &["rt".to_string(), "full".to_string()]);

        assert_eq!(id1, id2, "Feature order should not affect BuildId");
    }

    #[test]
    fn test_build_id_generate_different_features() {
        use crate::crate_specifier::CrateSpecifier;

        let spec = CrateSpecifier::from_str("serde@1.0.0").unwrap();

        let id1 = BuildId::generate(&spec, &["derive".to_string()]);
        let id2 = BuildId::generate(&spec, &["alloc".to_string()]);

        assert_ne!(id1, id2, "Different features must produce different BuildIds");
    }

    #[test]
    fn test_build_id_generate_no_features() {
        use crate::crate_specifier::CrateSpecifier;

        let spec = CrateSpecifier::from_str("anyhow@1.0.75").unwrap();
        let id = BuildId::generate(&spec, &[]);

        assert!(id.as_str().starts_with("build_"));
        assert!(!id.as_str().is_empty());
    }

    #[test]
    fn test_build_id_generate_format() {
        use crate::crate_specifier::CrateSpecifier;

        let spec = CrateSpecifier::from_str("test@1.0.0").unwrap();
        let id = BuildId::generate(&spec, &[]);

        // Should be: "build_" (6) + 32 hex characters = 38 total
        assert_eq!(id.as_str().len(), 38);
        assert!(id.as_str().starts_with("build_"));

        // Verify hex characters after prefix
        let hex_part = &id.as_str()[6..];
        assert!(hex_part.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn test_build_id_generate_different_versions() {
        use crate::crate_specifier::CrateSpecifier;

        let spec1 = CrateSpecifier::from_str("serde@1.0.0").unwrap();
        let spec2 = CrateSpecifier::from_str("serde@1.0.1").unwrap();
        let features = vec!["derive".to_string()];

        let id1 = BuildId::generate(&spec1, &features);
        let id2 = BuildId::generate(&spec2, &features);

        assert_ne!(id1, id2, "Different versions must produce different BuildIds");
    }

    #[test]
    fn test_build_id_generate_different_crates() {
        use crate::crate_specifier::CrateSpecifier;

        let spec1 = CrateSpecifier::from_str("serde@1.0.0").unwrap();
        let spec2 = CrateSpecifier::from_str("tokio@1.0.0").unwrap();
        let features = vec!["full".to_string()];

        let id1 = BuildId::generate(&spec1, &features);
        let id2 = BuildId::generate(&spec2, &features);

        assert_ne!(id1, id2, "Different crates must produce different BuildIds");
    }

    #[test]
    fn test_build_id_generate_empty_vs_no_features() {
        use crate::crate_specifier::CrateSpecifier;

        let spec = CrateSpecifier::from_str("test@1.0.0").unwrap();

        let id_empty = BuildId::generate(&spec, &[]);
        let id_no_features: Vec<String> = vec![];
        let id_none = BuildId::generate(&spec, &id_no_features);

        assert_eq!(id_empty, id_none, "Empty vec and no features should be identical");
    }

    #[test]
    fn test_build_id_generate_case_sensitivity() {
        use crate::crate_specifier::CrateSpecifier;

        // Crate names are case-sensitive in Cargo
        let spec1 = CrateSpecifier::from_str("MyCreate@1.0.0").unwrap();
        let spec2 = CrateSpecifier::from_str("mycrate@1.0.0").unwrap();

        let id1 = BuildId::generate(&spec1, &[]);
        let id2 = BuildId::generate(&spec2, &[]);

        // Should produce different IDs (case matters)
        assert_ne!(id1, id2, "Crate name case should affect BuildId");
    }
}
