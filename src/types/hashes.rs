//! ContentHash - BLAKE3-based content addressing for deduplication
//!
//! This module provides cryptographic content hashing using BLAKE3 for content-addressable
//! storage. Identical content always produces identical hashes, enabling efficient
//! deduplication of documentation chunks across multiple builds.
//!
//! # Design Rationale
//!
//! - **BLAKE3**: Faster than SHA-256, cryptographically secure, optimized for modern CPUs
//! - **Deterministic**: Same content always produces same hash (deduplication guarantee)
//! - **Fixed-size**: 256-bit output (64 hex characters) optimal for database indexing
//! - **Content-addressable**: Hash serves as unique identifier for content
//!
//! # Format
//!
//! ContentHash is represented as 64 lowercase hexadecimal characters.
//!
//! Example: `af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262`
//!
//! # Usage
//!
//! ```rust
//! use crately::types::ContentHash;
//!
//! // Hash documentation content
//! let content = "Documentation content to deduplicate";
//! let hash = ContentHash::from_content(content.as_bytes());
//!
//! // Verify format
//! assert_eq!(hash.as_str().len(), 64);
//!
//! // Same content produces same hash
//! let hash2 = ContentHash::from_content(content.as_bytes());
//! assert_eq!(hash, hash2);
//! ```

use blake3;
use serde::{Deserialize, Serialize};
use std::fmt;
use thiserror::Error;

/// Content hash for deduplication using BLAKE3
///
/// ContentHash provides cryptographic hashing of documentation chunks to enable
/// content-addressable storage and deduplication. Identical content always produces
/// identical hashes, allowing the system to store content once and reference it
/// from multiple builds.
///
/// # Properties
///
/// - **Deterministic**: Same input always produces same output
/// - **Collision-resistant**: 256-bit cryptographic hash
/// - **Fast**: BLAKE3 optimized for modern CPUs
/// - **Format**: 64 lowercase hexadecimal characters
///
/// # Examples
///
/// ```rust
/// use crately::types::ContentHash;
///
/// // Hash some content
/// let content = b"fn main() { println!(\"Hello, world!\"); }";
/// let hash = ContentHash::from_content(content);
///
/// // Verify deduplication
/// let same_content = b"fn main() { println!(\"Hello, world!\"); }";
/// let hash2 = ContentHash::from_content(same_content);
/// assert_eq!(hash, hash2, "Identical content produces identical hash");
///
/// // Parse from database
/// let stored = hash.as_str();
/// let parsed = ContentHash::try_from(stored.to_string())?;
/// assert_eq!(hash, parsed);
/// # Ok::<(), crately::types::ContentHashError>(())
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ContentHash(String);

impl ContentHash {
    /// Length of BLAKE3 hash in hexadecimal characters (256 bits = 64 hex chars)
    pub const HASH_LENGTH: usize = 64;

    /// Compute BLAKE3 hash from raw content bytes
    ///
    /// This is the primary method for creating content hashes. It produces a
    /// deterministic hash that enables deduplication.
    ///
    /// # Arguments
    ///
    /// * `content` - Raw bytes to hash (typically UTF-8 encoded text)
    ///
    /// # Examples
    ///
    /// ```rust
    /// use crately::types::ContentHash;
    ///
    /// let content = "Documentation content";
    /// let hash = ContentHash::from_content(content.as_bytes());
    /// assert_eq!(hash.as_str().len(), 64);
    ///
    /// // Deduplication: same content = same hash
    /// let hash2 = ContentHash::from_content(content.as_bytes());
    /// assert_eq!(hash, hash2);
    /// ```
    pub fn from_content(content: &[u8]) -> Self {
        let hash = blake3::hash(content);
        Self(hash.to_hex().to_string())
    }

    /// Get the hash as a string slice
    ///
    /// Returns the 64-character hexadecimal hash string.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use crately::types::ContentHash;
    ///
    /// let hash = ContentHash::from_content(b"test");
    /// let s: &str = hash.as_str();
    /// assert_eq!(s.len(), 64);
    /// ```
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Validate a hash string format
    ///
    /// Checks that the string is exactly 64 hexadecimal characters.
    ///
    /// # Arguments
    ///
    /// * `s` - String to validate
    ///
    /// # Returns
    ///
    /// `Ok(())` if valid, `Err(ContentHashError)` if invalid
    fn validate(s: &str) -> Result<(), ContentHashError> {
        if s.len() != Self::HASH_LENGTH {
            return Err(ContentHashError::InvalidLength {
                expected: Self::HASH_LENGTH,
                actual: s.len(),
            });
        }

        if !s.chars().all(|c| c.is_ascii_hexdigit()) {
            return Err(ContentHashError::InvalidFormat(s.to_string()));
        }

        Ok(())
    }
}

impl fmt::Display for ContentHash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl TryFrom<String> for ContentHash {
    type Error = ContentHashError;

    /// Parse a ContentHash from a hex string with validation
    ///
    /// Validates that:
    /// 1. String is exactly 64 characters
    /// 2. All characters are hexadecimal (0-9, a-f, A-F)
    ///
    /// # Errors
    ///
    /// - `ContentHashError::InvalidLength` - String is not 64 characters
    /// - `ContentHashError::InvalidFormat` - String contains non-hex characters
    ///
    /// # Examples
    ///
    /// ```rust
    /// use crately::types::ContentHash;
    ///
    /// // Valid hash
    /// let valid = "af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262";
    /// let hash = ContentHash::try_from(valid.to_string())?;
    /// assert_eq!(hash.as_str(), valid);
    ///
    /// // Invalid length
    /// assert!(ContentHash::try_from("too_short".to_string()).is_err());
    ///
    /// // Invalid characters
    /// assert!(ContentHash::try_from("g".repeat(64)).is_err());
    /// # Ok::<(), crately::types::ContentHashError>(())
    /// ```
    fn try_from(s: String) -> Result<Self, Self::Error> {
        // Normalize to lowercase for consistency
        let normalized = s.to_lowercase();
        Self::validate(&normalized)?;
        Ok(Self(normalized))
    }
}

impl std::str::FromStr for ContentHash {
    type Err = ContentHashError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        ContentHash::try_from(s.to_string())
    }
}

/// Errors that can occur when working with ContentHash
#[derive(Debug, Error)]
pub enum ContentHashError {
    /// Hash string has incorrect length
    #[error("Invalid hash length: expected {expected} characters, got {actual}")]
    InvalidLength { expected: usize, actual: usize },

    /// Hash string contains non-hexadecimal characters
    #[error("Invalid hash format (must be hexadecimal): {0}")]
    InvalidFormat(String),
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    #[test]
    fn test_content_hash_from_content() {
        let content = b"Documentation content for testing";
        let hash = ContentHash::from_content(content);
        assert_eq!(hash.as_str().len(), ContentHash::HASH_LENGTH);
    }

    #[test]
    fn test_content_hash_deterministic() {
        let content = b"Deterministic content";
        let hash1 = ContentHash::from_content(content);
        let hash2 = ContentHash::from_content(content);
        assert_eq!(hash1, hash2, "Same content should produce same hash");
    }

    #[test]
    fn test_content_hash_different_content() {
        let content1 = b"Content A";
        let content2 = b"Content B";
        let hash1 = ContentHash::from_content(content1);
        let hash2 = ContentHash::from_content(content2);
        assert_ne!(hash1, hash2, "Different content should produce different hash");
    }

    #[test]
    fn test_content_hash_empty_content() {
        let hash = ContentHash::from_content(b"");
        assert_eq!(hash.as_str().len(), ContentHash::HASH_LENGTH);
    }

    #[test]
    fn test_content_hash_try_from_valid() {
        let valid_hash = "af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262";
        let hash = ContentHash::try_from(valid_hash.to_string()).expect("Should parse valid hash");
        assert_eq!(hash.as_str(), valid_hash);
    }

    #[test]
    fn test_content_hash_try_from_uppercase() {
        let uppercase = "AF1349B9F5F9A1A6A0404DEA36DCC9499BCB25C9ADC112B7CC9A93CAE41F3262";
        let hash = ContentHash::try_from(uppercase.to_string()).expect("Should normalize to lowercase");
        assert!(hash.as_str().chars().all(|c| c.is_lowercase() || c.is_numeric()));
    }

    #[test]
    fn test_content_hash_try_from_invalid_length() {
        let too_short = "abc123";
        let result = ContentHash::try_from(too_short.to_string());
        assert!(matches!(result, Err(ContentHashError::InvalidLength { .. })));

        let too_long = "a".repeat(100);
        let result = ContentHash::try_from(too_long);
        assert!(matches!(result, Err(ContentHashError::InvalidLength { .. })));
    }

    #[test]
    fn test_content_hash_try_from_invalid_format() {
        let invalid = "g".repeat(64); // 'g' is not hexadecimal
        let result = ContentHash::try_from(invalid);
        assert!(matches!(result, Err(ContentHashError::InvalidFormat(_))));

        // Test with 64 characters but invalid hex (mix of valid hex and 'g')
        let mixed_invalid = format!("{}{}", "af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f326", "g");
        assert_eq!(mixed_invalid.len(), 64);
        let result = ContentHash::try_from(mixed_invalid);
        assert!(matches!(result, Err(ContentHashError::InvalidFormat(_))));
    }

    #[test]
    fn test_content_hash_from_str() {
        let valid = "af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262";
        let hash = ContentHash::from_str(valid).expect("Should parse from str");
        assert_eq!(hash.as_str(), valid);
    }

    #[test]
    fn test_content_hash_display() {
        let content = b"test content";
        let hash = ContentHash::from_content(content);
        let displayed = format!("{}", hash);
        assert_eq!(displayed, hash.as_str());
    }

    #[test]
    fn test_content_hash_clone_and_equality() {
        let content = b"clone test";
        let hash1 = ContentHash::from_content(content);
        let hash2 = hash1.clone();
        assert_eq!(hash1, hash2);
    }

    #[test]
    fn test_content_hash_serde_roundtrip() {
        let original = ContentHash::from_content(b"serde test");
        let json = serde_json::to_string(&original).expect("Should serialize");
        let deserialized: ContentHash = serde_json::from_str(&json).expect("Should deserialize");
        assert_eq!(original, deserialized);
    }

    #[test]
    fn test_content_hash_is_send_sync() {
        fn assert_send<T: Send>() {}
        fn assert_sync<T: Sync>() {}
        assert_send::<ContentHash>();
        assert_sync::<ContentHash>();
    }

    #[test]
    fn test_content_hash_hash_trait() {
        use std::collections::HashMap;
        let mut map = HashMap::new();
        let hash = ContentHash::from_content(b"hash test");
        map.insert(hash.clone(), "value");
        assert_eq!(map.get(&hash), Some(&"value"));
    }

    #[test]
    fn test_content_hash_error_display() {
        let err = ContentHashError::InvalidLength {
            expected: 64,
            actual: 10,
        };
        let msg = format!("{}", err);
        assert!(msg.contains("64"));
        assert!(msg.contains("10"));

        let err = ContentHashError::InvalidFormat("invalid".to_string());
        let msg = format!("{}", err);
        assert!(msg.contains("hexadecimal"));
    }

    #[test]
    fn test_content_hash_known_value() {
        // Test against known BLAKE3 hash for reproducibility
        let content = b"hello world";
        let hash = ContentHash::from_content(content);
        // Known BLAKE3 hash for "hello world"
        let expected = "d74981efa70a0c880b8d8c1985d075dbcbf679b99a5f9914e5aaf96b831a9e24";
        assert_eq!(hash.as_str(), expected);
    }

    #[test]
    fn test_content_hash_validate_function() {
        // Valid hashes
        assert!(ContentHash::validate(&"a".repeat(64)).is_ok());
        assert!(ContentHash::validate(&"f".repeat(64)).is_ok());
        assert!(ContentHash::validate(&"0".repeat(64)).is_ok());

        // Invalid length
        assert!(ContentHash::validate(&"a".repeat(63)).is_err());
        assert!(ContentHash::validate(&"a".repeat(65)).is_err());

        // Invalid characters
        assert!(ContentHash::validate(&("g".to_string() + &"a".repeat(63))).is_err());
    }
}
