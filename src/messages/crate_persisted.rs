//! Event broadcast when crate metadata is successfully persisted to the database.

use acton_reactive::prelude::*;

/// Event broadcast when crate metadata is successfully persisted.
///
/// This message is broadcast via the broker by DatabaseActor after successfully
/// storing crate metadata in the database. Subscribers such as Console (for user
/// feedback) and ServerActor (for HTTP response) can listen for this event.
///
/// # Fields
///
/// * `name` - The name of the crate that was persisted
/// * `version` - The version of the crate that was persisted
/// * `record_id` - The SurrealDB record ID assigned to the persisted crate
///
/// # Example
///
/// ```no_run
/// use crately::messages::CratePersisted;
///
/// let event = CratePersisted {
///     name: "serde".to_string(),
///     version: "1.0.0".to_string(),
///     record_id: "crate:serde_1_0_0".to_string(),
/// };
/// // broker.broadcast(event).await;
/// ```
///
/// # Message Flow
///
/// 1. DatabaseActor receives PersistCrate message
/// 2. DatabaseActor stores metadata in SurrealDB
/// 3. DatabaseActor broadcasts CratePersisted with record details
/// 4. Console actor displays success message to user
/// 5. ServerActor includes record_id in HTTP response
///
/// # Subscribers
///
/// - **Console**: Displays user-facing success message
/// - **ServerActor**: Constructs HTTP response with record_id
#[acton_message]
pub struct CratePersisted {
    /// Name of the crate that was persisted
    pub name: String,
    /// Version of the crate that was persisted
    pub version: String,
    /// SurrealDB record ID for the persisted crate
    pub record_id: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Verify that CratePersisted can be created and cloned.
    #[test]
    fn test_crate_persisted_creation() {
        let event = CratePersisted {
            name: "serde".to_string(),
            version: "1.0.0".to_string(),
            record_id: "crate:serde_1_0_0".to_string(),
        };
        let cloned = event.clone();
        assert_eq!(event.name, cloned.name);
        assert_eq!(event.version, cloned.version);
        assert_eq!(event.record_id, cloned.record_id);
    }

    /// Verify that CratePersisted implements Debug.
    #[test]
    fn test_crate_persisted_debug() {
        let event = CratePersisted {
            name: "tokio".to_string(),
            version: "1.35.0".to_string(),
            record_id: "crate:tokio_1_35_0".to_string(),
        };
        let debug_str = format!("{:?}", event);
        assert!(!debug_str.is_empty());
    }

    /// Verify that CratePersisted is Send + Sync (required for actor message passing).
    #[test]
    fn test_crate_persisted_is_send_sync() {
        fn assert_send<T: Send>() {}
        fn assert_sync<T: Sync>() {}

        assert_send::<CratePersisted>();
        assert_sync::<CratePersisted>();
    }

    /// Verify that CratePersisted fields contain expected data.
    #[test]
    fn test_crate_persisted_fields() {
        let event = CratePersisted {
            name: "anyhow".to_string(),
            version: "1.0.75".to_string(),
            record_id: "crate:anyhow_1_0_75".to_string(),
        };
        assert_eq!(event.name, "anyhow");
        assert_eq!(event.version, "1.0.75");
        assert_eq!(event.record_id, "crate:anyhow_1_0_75");
    }
}
