//! Message requesting persistence of crate metadata to the database.

use acton_reactive::prelude::*;
use crate::crate_specifier::CrateSpecifier;

/// Request to persist crate metadata to database.
///
/// This message is sent from ServerActor to DatabaseActor when
/// a new crate request is received via the HTTP API. The DatabaseActor
/// will store the crate metadata in the `crate` table with status set to 'pending',
/// then broadcast a `CratePersisted` event on success or `DatabaseError` on failure.
///
/// # Fields
///
/// * `specifier` - The validated crate name and version to persist
/// * `features` - List of feature flags to enable for this crate
///
/// # Example
///
/// ```no_run
/// use crately::messages::PersistCrate;
/// use crately::crate_specifier::CrateSpecifier;
///
/// let specifier = CrateSpecifier::try_from("serde@1.0.0").unwrap();
/// let persist_msg = PersistCrate {
///     specifier,
///     features: vec!["derive".to_string()],
/// };
/// // database_handle.send(persist_msg).await;
/// ```
///
/// # Message Flow
///
/// 1. ServerActor receives HTTP request with crate details
/// 2. ServerActor validates input and creates CrateSpecifier
/// 3. ServerActor sends PersistCrate to DatabaseActor
/// 4. DatabaseActor persists to database
/// 5. DatabaseActor broadcasts CratePersisted or DatabaseError
#[acton_message]
pub struct PersistCrate {
    /// The crate name and version to persist
    pub specifier: CrateSpecifier,
    /// Feature flags for this crate
    pub features: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    /// Verify that PersistCrate can be created and cloned.
    #[test]
    fn test_persist_crate_creation() {
        let specifier = CrateSpecifier::from_str("serde@1.0.0").unwrap();
        let message = PersistCrate {
            specifier: specifier.clone(),
            features: vec!["derive".to_string()],
        };
        let cloned = message.clone();
        assert_eq!(message.specifier, cloned.specifier);
        assert_eq!(message.features, cloned.features);
    }

    /// Verify that PersistCrate implements Debug.
    #[test]
    fn test_persist_crate_debug() {
        let specifier = CrateSpecifier::from_str("tokio@1.35.0").unwrap();
        let message = PersistCrate {
            specifier,
            features: vec!["full".to_string()],
        };
        let debug_str = format!("{:?}", message);
        assert!(!debug_str.is_empty());
    }

    /// Verify that PersistCrate is Send + Sync (required for actor message passing).
    #[test]
    fn test_persist_crate_is_send_sync() {
        fn assert_send<T: Send>() {}
        fn assert_sync<T: Sync>() {}

        assert_send::<PersistCrate>();
        assert_sync::<PersistCrate>();
    }

    /// Verify that PersistCrate works with empty features list.
    #[test]
    fn test_persist_crate_empty_features() {
        let specifier = CrateSpecifier::from_str("anyhow@1.0.75").unwrap();
        let message = PersistCrate {
            specifier,
            features: vec![],
        };
        assert!(message.features.is_empty());
    }
}
