//! Event broadcast when a crate processing request is received via HTTP API.

use acton_reactive::prelude::*;
use crate::crate_specifier::CrateSpecifier;

/// Event broadcast when a crate processing request is received via HTTP API.
///
/// This message is broadcast by ServerActor after validating the incoming HTTP
/// request and extracting a valid CrateSpecifier. It signals the beginning of
/// the crate processing pipeline.
///
/// # Fields
///
/// * `specifier` - The validated crate name and version to process
/// * `features` - List of feature flags to enable for this crate
///
/// # Message Flow
///
/// 1. HTTP POST /crate request arrives at ServerActor
/// 2. ServerActor validates request and creates CrateSpecifier
/// 3. ServerActor broadcasts CrateReceived event
/// 4. Multiple subscribers react:
///    - Console: Displays "Processing crate: {specifier}"
///    - DatabaseActor: May persist initial pending state
///    - CrateDownloader: Begins download process
///
/// # Subscribers
///
/// - **Console**: User feedback - "Processing crate: serde@1.0.0"
/// - **CrateDownloader**: Triggers download workflow (primary pipeline progression)
/// - **CrateCoordinatorActor** (future): Tracks processing state machine
///
/// # Example
///
/// ```no_run
/// use crately::messages::CrateReceived;
/// use crately::crate_specifier::CrateSpecifier;
/// use std::str::FromStr;
///
/// let specifier = CrateSpecifier::from_str("serde@1.0.0").unwrap();
/// let event = CrateReceived {
///     specifier,
///     features: vec!["derive".to_string()],
/// };
/// // broker.broadcast(event).await;
/// ```
#[acton_message]
pub struct CrateReceived {
    /// The crate name and version to process
    pub specifier: CrateSpecifier,
    /// Feature flags for this crate
    pub features: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    #[test]
    fn test_message_creation() {
        let specifier = CrateSpecifier::from_str("serde@1.0.0").unwrap();
        let message = CrateReceived {
            specifier: specifier.clone(),
            features: vec!["derive".to_string()],
        };
        assert_eq!(message.specifier, specifier);
        assert_eq!(message.features, vec!["derive"]);
    }

    #[test]
    fn test_message_clone() {
        let specifier = CrateSpecifier::from_str("tokio@1.0.0").unwrap();
        let message = CrateReceived {
            specifier,
            features: vec![],
        };
        let cloned = message.clone();
        assert_eq!(message.specifier, cloned.specifier);
        assert_eq!(message.features, cloned.features);
    }

    #[test]
    fn test_message_debug() {
        let specifier = CrateSpecifier::from_str("axum@0.7.0").unwrap();
        let message = CrateReceived {
            specifier,
            features: vec!["macros".to_string()],
        };
        let debug_str = format!("{:?}", message);
        assert!(debug_str.contains("CrateReceived"));
    }

    #[test]
    fn test_message_is_send_sync() {
        fn assert_send<T: Send>() {}
        fn assert_sync<T: Sync>() {}
        assert_send::<CrateReceived>();
        assert_sync::<CrateReceived>();
    }

    #[test]
    fn test_empty_features() {
        let specifier = CrateSpecifier::from_str("anyhow@1.0.0").unwrap();
        let message = CrateReceived {
            specifier,
            features: vec![],
        };
        assert!(message.features.is_empty());
    }

    #[test]
    fn test_round_trip_with_specifier() {
        let specifier = CrateSpecifier::from_str("thiserror@1.0.0").unwrap();
        let message = CrateReceived {
            specifier: specifier.clone(),
            features: vec!["std".to_string()],
        };
        assert_eq!(message.specifier, specifier);
    }
}
