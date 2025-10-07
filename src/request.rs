use crate::crate_specifier::CrateSpecifier;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct CrateRequest {
    /// Crate specifier (name@version)
    #[serde(flatten)]
    pub specifier: CrateSpecifier,
    /// Optional features to enable
    #[serde(default)]
    pub features: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use semver::Version;

    #[test]
    fn test_crate_request_flatten_string() {
        // Test CrateRequest with flattened string specifier
        let json = r#"{"name": "serde", "version": "1.0.0", "features": ["derive"]}"#;
        let req: CrateRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.specifier.name(), "serde");
        assert_eq!(req.specifier.version(), &Version::parse("1.0.0").unwrap());
        assert_eq!(req.features, vec!["derive"]);
    }

    #[test]
    fn test_crate_request_no_features() {
        // Test CrateRequest without features (should default to empty vec)
        let json = r#"{"name": "tokio", "version": "1.35.0"}"#;
        let req: CrateRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.specifier.name(), "tokio");
        assert!(req.features.is_empty());
    }
}
