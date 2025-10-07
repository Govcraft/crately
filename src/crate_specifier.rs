use cargo_util_schemas::manifest::PackageName;
use semver::Version;
use serde::{Deserialize, Deserializer, Serialize};
use std::{fmt, str::FromStr};

/// Strongly-typed crate specifier combining name and version
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CrateSpecifier {
    /// Crate name (e.g., "serde")
    name: String,
    /// Semantic version
    version: Version,
}

impl CrateSpecifier {
    /// Create a new CrateSpecifier with validation
    pub fn new(name: String, version: Version) -> Result<Self, CrateSpecifierError> {
        // Validate using cargo-util-schemas
        PackageName::new(&name)
            .map_err(|e| CrateSpecifierError::InvalidCrateName(e.to_string()))?;
        Ok(Self { name, version })
    }

    /// Get the crate name
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Get the semantic version
    pub fn version(&self) -> &Version {
        &self.version
    }
}

impl FromStr for CrateSpecifier {
    type Err = CrateSpecifierError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let parts: Vec<&str> = s.splitn(2, '@').collect();

        if parts.len() != 2 {
            return Err(CrateSpecifierError::MissingVersion(s.to_string()));
        }

        let name = parts[0].to_string();
        let version = Version::parse(parts[1]).map_err(CrateSpecifierError::InvalidVersion)?;

        Self::new(name, version)
    }
}

impl fmt::Display for CrateSpecifier {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}@{}", self.name, self.version)
    }
}

impl<'de> Deserialize<'de> for CrateSpecifier {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum CrateSpecifierInput {
            /// Accept "name@version" string format
            String(String),
            /// Accept object with separate name and version fields
            Object { name: String, version: String },
        }

        let input = CrateSpecifierInput::deserialize(deserializer)?;

        match input {
            CrateSpecifierInput::String(s) => {
                CrateSpecifier::from_str(&s).map_err(serde::de::Error::custom)
            }
            CrateSpecifierInput::Object { name, version } => {
                let v = Version::parse(&version).map_err(serde::de::Error::custom)?;
                CrateSpecifier::new(name, v).map_err(serde::de::Error::custom)
            }
        }
    }
}

#[derive(Debug)]
pub enum CrateSpecifierError {
    InvalidCrateName(String),
    InvalidVersion(semver::Error),
    MissingVersion(String),
}

impl fmt::Display for CrateSpecifierError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CrateSpecifierError::InvalidCrateName(name) => {
                write!(f, "Invalid crate name: '{}'", name)
            }
            CrateSpecifierError::InvalidVersion(e) => {
                write!(f, "Invalid semantic version: {}", e)
            }
            CrateSpecifierError::MissingVersion(s) => {
                write!(
                    f,
                    "Missing version in specifier: '{}'. Expected format: 'name@version'",
                    s
                )
            }
        }
    }
}

impl std::error::Error for CrateSpecifierError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_crate_specifier_from_str_valid() {
        let spec = CrateSpecifier::from_str("serde@1.0.0").unwrap();
        assert_eq!(spec.name(), "serde");
        assert_eq!(spec.version(), &Version::parse("1.0.0").unwrap());
        assert_eq!(spec.to_string(), "serde@1.0.0");

        let spec = CrateSpecifier::from_str("tokio-util@0.7.10").unwrap();
        assert_eq!(spec.name(), "tokio-util");
        assert_eq!(spec.version(), &Version::parse("0.7.10").unwrap());
    }

    #[test]
    fn test_crate_specifier_from_str_invalid() {
        // Missing version
        assert!(CrateSpecifier::from_str("serde").is_err());

        // Invalid version
        assert!(CrateSpecifier::from_str("serde@invalid").is_err());

        // Invalid crate name
        assert!(CrateSpecifier::from_str("123invalid@1.0.0").is_err());
        assert!(CrateSpecifier::from_str("@1.0.0").is_err());

        // Multiple @ symbols (uses first as delimiter)
        let result = CrateSpecifier::from_str("crate@1.0.0@extra");
        // This should fail to parse version "1.0.0@extra"
        assert!(result.is_err());
    }

    #[test]
    fn test_crate_specifier_serde_string() {
        // Test deserializing from string format
        let json = r#""serde@1.0.0""#;
        let spec: CrateSpecifier = serde_json::from_str(json).unwrap();
        assert_eq!(spec.name(), "serde");
        assert_eq!(spec.version(), &Version::parse("1.0.0").unwrap());
    }

    #[test]
    fn test_crate_specifier_serde_object() {
        // Test deserializing from object format
        let json = r#"{"name": "tokio", "version": "1.35.0"}"#;
        let spec: CrateSpecifier = serde_json::from_str(json).unwrap();
        assert_eq!(spec.name(), "tokio");
        assert_eq!(spec.version(), &Version::parse("1.35.0").unwrap());
    }

    #[test]
    fn test_crate_specifier_serialize() {
        let spec = CrateSpecifier::from_str("axum@0.7.0").unwrap();
        let json = serde_json::to_string(&spec).unwrap();
        assert!(json.contains("axum"));
        assert!(json.contains("0.7.0"));
    }

    #[test]
    fn test_crate_specifier_clone_and_equality() {
        let spec1 = CrateSpecifier::from_str("serde@1.0.0").unwrap();
        let spec2 = spec1.clone();
        assert_eq!(spec1, spec2);

        let spec3 = CrateSpecifier::from_str("serde@1.0.1").unwrap();
        assert_ne!(spec1, spec3);
    }

    #[test]
    fn test_crate_specifier_display() {
        let spec = CrateSpecifier::from_str("my-crate@2.1.3").unwrap();
        assert_eq!(format!("{}", spec), "my-crate@2.1.3");
    }

    #[test]
    fn test_crate_specifier_error_messages() {
        let err = CrateSpecifier::from_str("serde").unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("Missing version"));
        assert!(msg.contains("name@version"));

        let err = CrateSpecifier::from_str("@1.0.0").unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("Invalid crate name"));
    }

    #[test]
    fn test_valid_crate_names() {
        // Test via CrateSpecifier validation
        assert!(CrateSpecifier::from_str("serde@1.0.0").is_ok());
        assert!(CrateSpecifier::from_str("serde_json@1.0.0").is_ok());
        assert!(CrateSpecifier::from_str("tokio-util@0.7.0").is_ok());
        assert!(CrateSpecifier::from_str("my_crate123@1.0.0").is_ok());
    }

    #[test]
    fn test_invalid_crate_names() {
        // Empty name
        assert!(CrateSpecifier::from_str("@1.0.0").is_err());
        // Starting with number
        assert!(CrateSpecifier::from_str("123crate@1.0.0").is_err());
        // Starting with hyphen
        assert!(CrateSpecifier::from_str("-crate@1.0.0").is_err());
        // Invalid characters
        assert!(CrateSpecifier::from_str("crate name@1.0.0").is_err());
    }
}
