//! DatabaseActor for SurrealDB persistence operations.
//!
//! This module provides the DatabaseActor which manages an embedded RocksDB-backed
//! SurrealDB instance for high-performance local persistence.

use acton_reactive::prelude::*;
use anyhow::Context;
use std::path::PathBuf;
use surrealdb::engine::local::{Db, RocksDb};
use surrealdb::Surreal;
use tracing::*;

use std::str::FromStr;

use crate::crate_specifier::CrateSpecifier;
use crate::messages::{
    CrateListResponse, CrateQueryResponse, CrateSummary, DatabaseError, DatabaseReady,
    ListCrates, PersistCrate, QueryCrate,
};

/// Information about the database connection returned at spawn time.
///
/// This struct is returned alongside the actor handle when spawning the DatabaseActor,
/// providing immediate access to connection details without requiring message passing.
#[derive(Debug, Clone)]
pub struct DatabaseInfo {
    /// The filesystem path to the database directory
    pub db_path: PathBuf,
    /// The SurrealDB namespace in use
    pub namespace: String,
    /// The SurrealDB database name in use
    pub database: String,
}

/// DatabaseActor manages the embedded SurrealDB lifecycle and persistence operations.
///
/// This actor handles:
/// - Connection management for embedded RocksDB backend
/// - Schema initialization and migrations
/// - Crate metadata persistence
/// - Query operations
/// - Error broadcasting to subscribers
///
/// The actor uses the Service Actor Pattern, returning both a handle and database
/// connection information at spawn time.
///
/// Note: This actor does not use `#[acton_actor]` because `Surreal<Db>` does not
/// implement `Default`. Instead, the actor model is constructed manually in the
/// `spawn` method after establishing the database connection.
#[derive(Clone, Debug)]
pub struct DatabaseActor {
    db: Surreal<Db>,
    namespace: String,
    database: String,
    db_path: PathBuf,
}

impl Default for DatabaseActor {
    /// Provides a placeholder Default implementation required by acton-reactive.
    ///
    /// This Default creates a temporary instance that is immediately replaced by
    /// the spawn method. The placeholder uses an in-memory database connection
    /// that is never actually used since spawn() always overwrites builder.model
    /// with a properly configured instance before starting the actor.
    ///
    /// Note: This implementation exists solely to satisfy acton-reactive's
    /// requirement that actor types implement Default. The actual actor
    /// initialization happens in spawn() via builder.model assignment.
    fn default() -> Self {
        // Create a placeholder instance with an in-memory connection
        // This will be immediately replaced by spawn() before the actor starts
        Self {
            db: Surreal::init(),
            namespace: String::new(),
            database: String::new(),
            db_path: PathBuf::new(),
        }
    }
}

impl DatabaseActor {
    /// Spawns, configures, and starts a new DatabaseActor with embedded RocksDB.
    ///
    /// This factory method creates an embedded SurrealDB instance using RocksDB
    /// for high-performance local persistence. The connection string format is
    /// `rocksdb://<path>` where the path points to the database directory.
    ///
    /// # Connection Details
    ///
    /// - **Protocol**: `rocksdb:` (embedded, high-performance)
    /// - **Path**: User-provided via `db_path` parameter
    /// - **Namespace**: `crately`
    /// - **Database**: `production`
    ///
    /// # Lifecycle
    ///
    /// 1. **Initialization**: Connects to embedded RocksDB
    /// 2. **Namespace/DB Selection**: Sets active namespace and database
    /// 3. **Schema Setup**: Executes DEFINE statements in `after_start` hook
    /// 4. **Ready Broadcast**: Emits `DatabaseReady` event when fully initialized
    /// 5. **Shutdown**: Graceful connection close in `before_stop` hook
    ///
    /// # Arguments
    ///
    /// * `runtime` - Mutable reference to the acton-reactive runtime
    /// * `db_path` - Filesystem path for the RocksDB database directory
    ///
    /// # Returns
    ///
    /// Returns a tuple containing:
    /// - `AgentHandle` - Handle to the started DatabaseActor for message passing
    /// - `DatabaseInfo` - Connection details for immediate use
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Database connection fails
    /// - Namespace or database selection fails
    /// - Actor creation or initialization fails
    ///
    /// # Example
    ///
    /// ```no_run
    /// use acton_reactive::prelude::*;
    /// use crately::actors::database::DatabaseActor;
    /// use std::path::PathBuf;
    ///
    /// #[tokio::main]
    /// async fn main() -> anyhow::Result<()> {
    ///     let mut runtime = ActonApp::launch();
    ///     let db_path = PathBuf::from("/home/user/.local/share/crately/db");
    ///
    ///     let (db_handle, db_info) = DatabaseActor::spawn(&mut runtime, db_path).await?;
    ///
    ///     println!("Database ready at: {}", db_info.db_path.display());
    ///
    ///     runtime.shutdown_all().await?;
    ///     Ok(())
    /// }
    /// ```
    #[instrument(skip(runtime))]
    pub async fn spawn(
        runtime: &mut AgentRuntime,
        db_path: PathBuf,
    ) -> anyhow::Result<(AgentHandle, DatabaseInfo)> {
        // Connect to embedded RocksDB
        info!("Connecting to database: {}", db_path.display());

        let db = Surreal::new::<RocksDb>(db_path.clone())
            .await
            .with_context(|| {
                format!(
                    "Failed to connect to RocksDB database at {}\n\
                     \n\
                     Possible causes:\n\
                     - Database directory does not exist or is not accessible\n\
                     - Insufficient permissions to create or access database files\n\
                     - Another process has locked the database\n\
                     - RocksDB feature not enabled in dependencies",
                    db_path.display()
                )
            })?;

        // Set namespace and database
        let namespace = "crately".to_string();
        let database = "production".to_string();

        db.use_ns(&namespace)
            .use_db(&database)
            .await
            .with_context(|| {
                format!(
                    "Failed to select namespace '{}' and database '{}'\n\
                     \n\
                     This may indicate a permissions issue or invalid namespace/database names.",
                    namespace, database
                )
            })?;

        info!(
            "Selected namespace '{}' and database '{}'",
            namespace, database
        );

        // Create agent configuration
        let agent_config =
            AgentConfig::new(Ern::with_root("database").unwrap(), None, None)
                .context("Failed to create DatabaseActor agent configuration")?;

        // Create the DatabaseActor model with the established connection
        let database_actor = DatabaseActor {
            db,
            namespace: namespace.clone(),
            database: database.clone(),
            db_path: db_path.clone(),
        };

        // Create agent builder
        let mut builder = runtime
            .new_agent_with_config::<DatabaseActor>(agent_config)
            .await;

        // Set the custom model before starting
        builder.model = database_actor;

        // Handle PersistCrate requests
        builder.mutate_on::<PersistCrate>(|agent, envelope| {
            let msg = envelope.message().clone();
            let db = agent.model.db.clone();
            let broker = agent.broker().clone();

            Box::pin(async move {
                // Create the record data
                let record_data = serde_json::json!({
                    "name": msg.specifier.name(),
                    "version": msg.specifier.version().to_string(),
                    "features": msg.features,
                    "status": "pending",
                });

                // Execute the create statement
                match db
                    .query("CREATE crate CONTENT $data")
                    .bind(("data", record_data))
                    .await
                {
                    Ok(_) => {
                        debug!(
                            "Persisted crate: {}@{}",
                            msg.specifier.name(),
                            msg.specifier.version()
                        );
                    }
                    Err(e) => {
                        error!("Failed to persist crate: {}", e);
                        broker
                            .broadcast(DatabaseError {
                                operation: format!(
                                    "persist crate {}@{}",
                                    msg.specifier.name(),
                                    msg.specifier.version()
                                ),
                                error: e.to_string(),
                            })
                            .await;
                    }
                }
            })
        });

        // Handle QueryCrate requests
        builder.mutate_on::<QueryCrate>(|agent, envelope| {
            let msg = envelope.message().clone();
            let db = agent.model.db.clone();
            let broker = agent.broker().clone();

            Box::pin(async move {
                // Clone fields to avoid lifetime issues with query bindings
                let name = msg.name.clone();
                let version_opt = msg.version.clone();

                // Build query based on whether version is specified
                let query_result = if let Some(version) = version_opt.clone() {
                    // Query for specific version
                    db.query("SELECT * FROM crate WHERE name = $name AND version = $version")
                        .bind(("name", name.clone()))
                        .bind(("version", version))
                        .await
                } else {
                    // Query for latest version (highest version number)
                    db.query("SELECT * FROM crate WHERE name = $name ORDER BY version DESC LIMIT 1")
                        .bind(("name", name.clone()))
                        .await
                };

                match query_result {
                    Ok(mut response) => {
                        // Take the first result
                        let result: Result<Option<serde_json::Value>, _> = response.take(0);
                        match result {
                            Ok(Some(record)) => {
                                let version_display = msg.version.as_deref().unwrap_or("latest");
                                debug!("Query successful for {}@{}: {:?}", msg.name, version_display, record);

                                // Extract fields from the record
                                let name_field = record
                                    .get("name")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or_default()
                                    .to_string();
                                let version_field = record
                                    .get("version")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or_default()
                                    .to_string();
                                let features: Vec<String> = record
                                    .get("features")
                                    .and_then(|v| v.as_array())
                                    .map(|arr| {
                                        arr.iter()
                                            .filter_map(|v| v.as_str().map(|s| s.to_string()))
                                            .collect()
                                    })
                                    .unwrap_or_default();
                                let status = record
                                    .get("status")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("unknown")
                                    .to_string();
                                let created_at = record
                                    .get("created_at")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("")
                                    .to_string();

                                // Create CrateSpecifier using FromStr for type safety
                                let specifier_str = format!("{}@{}", name_field, version_field);
                                match CrateSpecifier::from_str(&specifier_str) {
                                    Ok(specifier) => {
                                        // Broadcast successful query response
                                        broker
                                            .broadcast(CrateQueryResponse {
                                                specifier: Some(specifier),
                                                features,
                                                status,
                                                created_at,
                                            })
                                            .await;
                                    }
                                    Err(e) => {
                                        error!("Failed to create CrateSpecifier from database record: {}", e);
                                        broker
                                            .broadcast(DatabaseError {
                                                operation: format!("query crate {}@{}", msg.name, version_display),
                                                error: format!("Invalid crate data in database: {}", e),
                                            })
                                            .await;
                                    }
                                }
                            }
                            Ok(None) => {
                                let version_display = msg.version.as_deref().unwrap_or("latest");
                                debug!("No record found for {}@{}", msg.name, version_display);

                                // Broadcast not-found response with None specifier
                                broker
                                    .broadcast(CrateQueryResponse {
                                        specifier: None,
                                        features: vec![],
                                        status: "not_found".to_string(),
                                        created_at: String::new(),
                                    })
                                    .await;
                            }
                            Err(e) => {
                                error!("Failed to parse query result: {}", e);
                                let version_display = msg.version.as_deref().unwrap_or("latest");
                                broker
                                    .broadcast(DatabaseError {
                                        operation: format!("query crate {}@{}", msg.name, version_display),
                                        error: e.to_string(),
                                    })
                                    .await;
                            }
                        }
                    }
                    Err(e) => {
                        error!("Failed to query crate: {}", e);
                        let version_display = msg.version.as_deref().unwrap_or("latest");
                        broker
                            .broadcast(DatabaseError {
                                operation: format!("query crate {}@{}", msg.name, version_display),
                                error: e.to_string(),
                            })
                            .await;
                    }
                }
            })
        });

        // Handle ListCrates requests
        builder.mutate_on::<ListCrates>(|agent, envelope| {
            let msg = envelope.message().clone();
            let db = agent.model.db.clone();
            let broker = agent.broker().clone();

            Box::pin(async move {
                // Apply default values for pagination
                let limit = msg.limit.unwrap_or(100);
                let offset = msg.offset.unwrap_or(0);

                debug!("Listing crates with limit={}, offset={}", limit, offset);

                // Execute paginated query
                let query_result = db
                    .query("SELECT * FROM crate LIMIT $limit START $offset")
                    .bind(("limit", limit))
                    .bind(("offset", offset))
                    .await;

                match query_result {
                    Ok(mut response) => {
                        // Parse the results into CrateSummary structs
                        let results: Result<Vec<serde_json::Value>, _> = response.take(0);

                        match results {
                            Ok(records) => {
                                // Map records to CrateSummary structs
                                let crates: Vec<CrateSummary> = records
                                    .iter()
                                    .map(|record| CrateSummary {
                                        name: record
                                            .get("name")
                                            .and_then(|v| v.as_str())
                                            .unwrap_or_default()
                                            .to_string(),
                                        version: record
                                            .get("version")
                                            .and_then(|v| v.as_str())
                                            .unwrap_or_default()
                                            .to_string(),
                                        status: record
                                            .get("status")
                                            .and_then(|v| v.as_str())
                                            .unwrap_or("unknown")
                                            .to_string(),
                                    })
                                    .collect();

                                debug!("Retrieved {} crates from database", crates.len());

                                // Get total count for pagination metadata
                                let count_result = db
                                    .query("SELECT count() FROM crate GROUP ALL")
                                    .await;

                                match count_result {
                                    Ok(mut count_response) => {
                                        // Extract count from response
                                        let count_value: Result<Option<serde_json::Value>, _> = count_response.take(0);
                                        let total_count = match count_value {
                                            Ok(Some(val)) => {
                                                val.get("count")
                                                    .and_then(|v| v.as_u64())
                                                    .unwrap_or(0) as u32
                                            }
                                            Ok(None) => 0,
                                            Err(e) => {
                                                error!("Failed to parse count result: {}", e);
                                                0
                                            }
                                        };

                                        debug!("Total crate count: {}", total_count);

                                        // Broadcast successful list response
                                        broker
                                            .broadcast(CrateListResponse {
                                                crates,
                                                total_count,
                                            })
                                            .await;
                                    }
                                    Err(e) => {
                                        error!("Failed to get total crate count: {}", e);
                                        broker
                                            .broadcast(DatabaseError {
                                                operation: "list crates (count)".to_string(),
                                                error: e.to_string(),
                                            })
                                            .await;
                                    }
                                }
                            }
                            Err(e) => {
                                error!("Failed to parse list results: {}", e);
                                broker
                                    .broadcast(DatabaseError {
                                        operation: "list crates".to_string(),
                                        error: e.to_string(),
                                    })
                                    .await;
                            }
                        }
                    }
                    Err(e) => {
                        error!("Failed to list crates: {}", e);
                        broker
                            .broadcast(DatabaseError {
                                operation: "list crates".to_string(),
                                error: e.to_string(),
                            })
                            .await;
                    }
                }
            })
        });

        // Add after_start hook to create schema and broadcast ready event
        builder.after_start(|agent| {
            let db = agent.model.db.clone();
            let broker = agent.broker().clone();

            AgentReply::from_async(async move {
                info!("Initializing database schema...");

                // Execute schema definition
                let schema_sql = r#"
                    DEFINE TABLE crate SCHEMAFULL;
                    DEFINE FIELD name ON TABLE crate TYPE string;
                    DEFINE FIELD version ON TABLE crate TYPE string;
                    DEFINE FIELD features ON TABLE crate TYPE array;
                    DEFINE FIELD created_at ON TABLE crate TYPE datetime DEFAULT time::now();
                    DEFINE FIELD status ON TABLE crate TYPE string DEFAULT 'pending';
                    DEFINE INDEX idx_name_version ON TABLE crate COLUMNS name, version UNIQUE;
                "#;

                match db.query(schema_sql).await {
                    Ok(_) => {
                        info!("Database schema initialized successfully");

                        // Broadcast DatabaseReady event
                        broker
                            .broadcast(DatabaseReady)
                            .await;
                    }
                    Err(e) => {
                        error!("Failed to initialize database schema: {}", e);
                        broker
                            .broadcast(DatabaseError {
                                operation: "schema initialization".to_string(),
                                error: e.to_string(),
                            })
                            .await;
                    }
                }
            })
        });

        // Add before_stop hook for graceful shutdown
        builder.before_stop(|_agent| {
            info!("DatabaseActor shutting down");
            AgentReply::immediate()
        });

        let handle = builder.start().await;
        let db_info = DatabaseInfo {
            db_path,
            namespace,
            database,
        };

        Ok((handle, db_info))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    #[tokio::test(flavor = "multi_thread")]
    async fn test_database_actor_spawns_successfully() {
        let mut runtime = ActonApp::launch();
        let test_db_path = env::temp_dir().join("crately_test_db_spawn");

        // Clean up any existing test database
        let _ = std::fs::remove_dir_all(&test_db_path);

        let result = DatabaseActor::spawn(&mut runtime, test_db_path.clone()).await;

        assert!(result.is_ok(), "DatabaseActor should spawn successfully");

        let (handle, db_info) = result.unwrap();
        assert_eq!(db_info.namespace, "crately");
        assert_eq!(db_info.database, "production");
        assert_eq!(db_info.db_path, test_db_path);

        // Clean shutdown
        handle.stop().await.unwrap();
        runtime.shutdown_all().await.unwrap();

        // Cleanup
        let _ = std::fs::remove_dir_all(&test_db_path);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_database_actor_lifecycle() {
        let mut runtime = ActonApp::launch();
        let test_db_path = env::temp_dir().join("crately_test_db_lifecycle");

        // Clean up any existing test database
        let _ = std::fs::remove_dir_all(&test_db_path);

        // Spawn actor
        let (handle, _db_info) = DatabaseActor::spawn(&mut runtime, test_db_path.clone())
            .await
            .unwrap();

        // Give the after_start hook time to execute
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        // Stop actor gracefully
        let stop_result = handle.stop().await;
        assert!(
            stop_result.is_ok(),
            "DatabaseActor should stop gracefully"
        );

        runtime.shutdown_all().await.unwrap();

        // Cleanup
        let _ = std::fs::remove_dir_all(&test_db_path);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_multiple_database_actors_use_isolated_connections() {
        let mut runtime = ActonApp::launch();
        let test_db_path_1 = env::temp_dir().join("crately_test_db_isolated_1");
        let test_db_path_2 = env::temp_dir().join("crately_test_db_isolated_2");

        // Clean up any existing test databases
        let _ = std::fs::remove_dir_all(&test_db_path_1);
        let _ = std::fs::remove_dir_all(&test_db_path_2);

        // Spawn first actor
        let result1 = DatabaseActor::spawn(&mut runtime, test_db_path_1.clone()).await;
        assert!(result1.is_ok(), "First DatabaseActor should spawn");

        // Spawn second actor with different path
        let result2 = DatabaseActor::spawn(&mut runtime, test_db_path_2.clone()).await;
        assert!(result2.is_ok(), "Second DatabaseActor should spawn");

        let (handle1, db_info1) = result1.unwrap();
        let (handle2, db_info2) = result2.unwrap();

        // Verify they use different paths
        assert_ne!(
            db_info1.db_path, db_info2.db_path,
            "Database actors should use isolated paths"
        );

        // Clean shutdown
        handle1.stop().await.unwrap();
        handle2.stop().await.unwrap();
        runtime.shutdown_all().await.unwrap();

        // Cleanup
        let _ = std::fs::remove_dir_all(&test_db_path_1);
        let _ = std::fs::remove_dir_all(&test_db_path_2);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_database_actor_query_crate_specific_version() {
        use std::str::FromStr;

        let mut runtime = ActonApp::launch();
        let test_db_path = env::temp_dir().join("crately_test_db_query_specific");

        // Clean up any existing test database
        let _ = std::fs::remove_dir_all(&test_db_path);

        let (handle, _db_info) = DatabaseActor::spawn(&mut runtime, test_db_path.clone())
            .await
            .unwrap();

        // Wait for schema initialization
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        // Persist a test crate
        let specifier = CrateSpecifier::from_str("serde@1.0.0").unwrap();
        handle
            .send(PersistCrate {
                specifier,
                features: vec!["derive".to_string()],
            })
            .await;

        // Wait for persistence
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        // Query for the specific version
        handle
            .send(QueryCrate {
                name: "serde".to_string(),
                version: Some("1.0.0".to_string()),
            })
            .await;

        // Wait for query to complete
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        // Clean shutdown
        handle.stop().await.unwrap();
        runtime.shutdown_all().await.unwrap();

        // Cleanup
        let _ = std::fs::remove_dir_all(&test_db_path);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_database_actor_query_crate_latest_version() {
        use std::str::FromStr;

        let mut runtime = ActonApp::launch();
        let test_db_path = env::temp_dir().join("crately_test_db_query_latest");

        // Clean up any existing test database
        let _ = std::fs::remove_dir_all(&test_db_path);

        let (handle, _db_info) = DatabaseActor::spawn(&mut runtime, test_db_path.clone())
            .await
            .unwrap();

        // Wait for schema initialization
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        // Persist multiple versions of the same crate
        let specifier1 = CrateSpecifier::from_str("tokio@1.35.0").unwrap();
        handle
            .send(PersistCrate {
                specifier: specifier1,
                features: vec![],
            })
            .await;

        let specifier2 = CrateSpecifier::from_str("tokio@1.36.0").unwrap();
        handle
            .send(PersistCrate {
                specifier: specifier2,
                features: vec!["full".to_string()],
            })
            .await;

        // Wait for persistence
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        // Query for latest version (version = None)
        handle
            .send(QueryCrate {
                name: "tokio".to_string(),
                version: None,
            })
            .await;

        // Wait for query to complete
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        // Clean shutdown
        handle.stop().await.unwrap();
        runtime.shutdown_all().await.unwrap();

        // Cleanup
        let _ = std::fs::remove_dir_all(&test_db_path);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_database_actor_query_crate_not_found() {
        let mut runtime = ActonApp::launch();
        let test_db_path = env::temp_dir().join("crately_test_db_query_not_found");

        // Clean up any existing test database
        let _ = std::fs::remove_dir_all(&test_db_path);

        let (handle, _db_info) = DatabaseActor::spawn(&mut runtime, test_db_path.clone())
            .await
            .unwrap();

        // Wait for schema initialization
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        // Query for a non-existent crate
        handle
            .send(QueryCrate {
                name: "nonexistent-crate".to_string(),
                version: Some("1.0.0".to_string()),
            })
            .await;

        // Wait for query to complete
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        // Clean shutdown
        handle.stop().await.unwrap();
        runtime.shutdown_all().await.unwrap();

        // Cleanup
        let _ = std::fs::remove_dir_all(&test_db_path);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_database_actor_list_crates_default_pagination() {
        use std::str::FromStr;

        let mut runtime = ActonApp::launch();
        let test_db_path = env::temp_dir().join("crately_test_db_list_default");

        // Clean up any existing test database
        let _ = std::fs::remove_dir_all(&test_db_path);

        let (handle, _db_info) = DatabaseActor::spawn(&mut runtime, test_db_path.clone())
            .await
            .unwrap();

        // Wait for schema initialization
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        // Persist some test crates
        let specifier1 = CrateSpecifier::from_str("serde@1.0.0").unwrap();
        handle
            .send(PersistCrate {
                specifier: specifier1,
                features: vec![],
            })
            .await;

        let specifier2 = CrateSpecifier::from_str("tokio@1.35.0").unwrap();
        handle
            .send(PersistCrate {
                specifier: specifier2,
                features: vec![],
            })
            .await;

        // Wait for persistence
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        // List crates with default pagination
        handle
            .send(ListCrates {
                limit: None,
                offset: None,
            })
            .await;

        // Wait for list to complete
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        // Clean shutdown
        handle.stop().await.unwrap();
        runtime.shutdown_all().await.unwrap();

        // Cleanup
        let _ = std::fs::remove_dir_all(&test_db_path);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_database_actor_list_crates_with_pagination() {
        use std::str::FromStr;

        let mut runtime = ActonApp::launch();
        let test_db_path = env::temp_dir().join("crately_test_db_list_paginated");

        // Clean up any existing test database
        let _ = std::fs::remove_dir_all(&test_db_path);

        let (handle, _db_info) = DatabaseActor::spawn(&mut runtime, test_db_path.clone())
            .await
            .unwrap();

        // Wait for schema initialization
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        // Persist multiple test crates
        for i in 0..5 {
            let specifier =
                CrateSpecifier::from_str(&format!("test-crate{}@1.0.0", i)).unwrap();
            handle
                .send(PersistCrate {
                    specifier,
                    features: vec![],
                })
                .await;
        }

        // Wait for persistence
        tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;

        // List first page with limit=2
        handle
            .send(ListCrates {
                limit: Some(2),
                offset: Some(0),
            })
            .await;

        // Wait for list to complete
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        // List second page with limit=2, offset=2
        handle
            .send(ListCrates {
                limit: Some(2),
                offset: Some(2),
            })
            .await;

        // Wait for list to complete
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        // Clean shutdown
        handle.stop().await.unwrap();
        runtime.shutdown_all().await.unwrap();

        // Cleanup
        let _ = std::fs::remove_dir_all(&test_db_path);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_database_actor_list_crates_empty_database() {
        let mut runtime = ActonApp::launch();
        let test_db_path = env::temp_dir().join("crately_test_db_list_empty");

        // Clean up any existing test database
        let _ = std::fs::remove_dir_all(&test_db_path);

        let (handle, _db_info) = DatabaseActor::spawn(&mut runtime, test_db_path.clone())
            .await
            .unwrap();

        // Wait for schema initialization
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        // List crates from empty database
        handle
            .send(ListCrates {
                limit: Some(10),
                offset: Some(0),
            })
            .await;

        // Wait for list to complete
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        // Clean shutdown
        handle.stop().await.unwrap();
        runtime.shutdown_all().await.unwrap();

        // Cleanup
        let _ = std::fs::remove_dir_all(&test_db_path);
    }
}
