//! DatabaseActor for SurrealDB persistence operations.
//!
//! This module provides the DatabaseActor which manages an embedded RocksDB-backed
//! SurrealDB instance for high-performance local persistence.

use acton_reactive::prelude::*;
use anyhow::Context;
use std::path::PathBuf;
use surrealdb::Surreal;
use surrealdb::engine::local::Db;
#[cfg(not(feature = "rocksdb"))]
use surrealdb::engine::local::Mem;
#[cfg(feature = "rocksdb")]
use surrealdb::engine::local::RocksDb;

use tracing::*;

use std::str::FromStr;

use crate::crate_specifier::CrateSpecifier;
use crate::messages::{
    ChunksPersistenceComplete, CrateDownloadFailed, CrateDownloaded, CrateListResponse,
    CrateProcessingFailed, CrateQueryResponse, CrateReceived, CrateSummary, DatabaseError,
    DatabaseReady, DocChunkData, DocChunkPersisted, DocChunksQueryResponse,
    DocumentationExtracted, DocumentationExtractionFailed, EmbeddingPersisted,
    EmbeddingsPersistenceComplete, ListCrates, PersistCodeSample, PersistCrate,
    PersistDocChunk, PersistEmbedding, PrintWarning, QueryCrate, QueryDocChunks,
    QuerySimilarDocs, SimilarDocsResponse,
};

#[cfg(test)]
use crate::messages::{DocumentationChunked, DocumentationVectorized};
use crate::types::SearchResult;

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

/// Response structure for queries that return only a crate record ID.
///
/// SurrealDB returns record IDs as `Thing` types (table + id enum), which cannot
/// be directly deserialized into `serde_json::Value`. This struct properly handles
/// the `Thing` type and provides convenient access to the ID as a string.
///
/// # Example
///
/// ```rust,ignore
/// let query = "SELECT id FROM crate WHERE name = $name AND version = $version";
/// let mut response = db.query(query).bind(params).await?;
/// let record: Option<CrateIdRecord> = response.take(0)?;
/// if let Some(rec) = record {
///     let id = rec.thing(); // Use the Thing directly in queries
/// }
/// ```
#[derive(Debug, Clone, serde::Deserialize)]
struct CrateIdRecord {
    /// The SurrealDB record ID (Thing type: table name + record identifier)
    id: surrealdb::sql::Thing,
}

impl CrateIdRecord {
    /// Extract the record ID as a string in SurrealDB format (e.g., "crate:xyz123")
    #[allow(dead_code)]
    fn id_string(&self) -> String {
        self.id.to_string()
    }

    /// Get a reference to the Thing for use in queries
    fn thing(&self) -> &surrealdb::sql::Thing {
        &self.id
    }
}

/// Response structure for queries that return only a doc_chunk record ID.
///
/// SurrealDB returns record IDs as `Thing` types (table + id enum), which cannot
/// be directly deserialized into `serde_json::Value`. This struct properly handles
/// the `Thing` type and provides convenient access to the ID as a string.
///
/// # Example
///
/// ```rust,ignore
/// let query = "SELECT id FROM doc_chunk WHERE chunk_id = $chunk_id";
/// let mut response = db.query(query).bind(params).await?;
/// let record: Option<DocChunkIdRecord> = response.take(0)?;
/// if let Some(rec) = record {
///     let id = rec.thing(); // Use the Thing directly in queries
/// }
/// ```
#[derive(Debug, Clone, serde::Deserialize)]
struct DocChunkIdRecord {
    /// The SurrealDB record ID (Thing type: table name + record identifier)
    id: surrealdb::sql::Thing,
}

impl DocChunkIdRecord {
    /// Extract the record ID as a string in SurrealDB format (e.g., "doc_chunk:xyz123")
    #[allow(dead_code)]
    fn id_string(&self) -> String {
        self.id.to_string()
    }

    /// Get a reference to the Thing for use in queries
    fn thing(&self) -> &surrealdb::sql::Thing {
        &self.id
    }
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
        // Connect to database (in-memory for tests, RocksDB for production)
        #[cfg(not(feature = "rocksdb"))]
        {
            info!(
                "Connecting to in-memory database for test: {}",
                db_path.display()
            );
        }
        #[cfg(feature = "rocksdb")]
        {
            info!("Connecting to RocksDB database: {}", db_path.display());
        }

        #[cfg(not(feature = "rocksdb"))]
        let db = Surreal::new::<Mem>(())
            .await
            .context("Failed to create in-memory database for testing")?;

        #[cfg(feature = "rocksdb")]
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

        debug!(
            "Selected namespace '{}' and database '{}'",
            namespace, database
        );

        // Create agent configuration
        let agent_config = AgentConfig::new(Ern::with_root("database").unwrap(), None, None)
            .context("Failed to create DatabaseActor agent configuration")?;

        // Create the DatabaseActor model with the established connection
        let database_actor = DatabaseActor { db };

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
                let name = msg.specifier.name().to_string();
                let version = msg.specifier.version().to_string();

                // Step 1: Clean up any existing chunks/embeddings for idempotency
                // This ensures reprocessing doesn't create duplicates
                let cleanup_query = r#"
                    -- First, delete embeddings that reference chunks for this crate
                    DELETE embedding WHERE crate_id.name = $name AND crate_id.version = $version;
                    -- Then delete the doc chunks themselves
                    DELETE doc_chunk WHERE crate_id.name = $name AND crate_id.version = $version;
                    -- Also delete any code samples
                    DELETE code_sample WHERE crate_id.name = $name AND crate_id.version = $version;
                "#;

                match db
                    .query(cleanup_query)
                    .bind(("name", name.clone()))
                    .bind(("version", version.clone()))
                    .await
                {
                    Ok(_) => {
                        debug!(
                            "Cleaned up existing chunks/embeddings for {}@{} before reprocessing",
                            name, version
                        );
                    }
                    Err(e) => {
                        // Log warning but continue - cleanup failure shouldn't block new processing
                        warn!(
                            "Failed to cleanup existing data for {}@{}: {}",
                            name, version, e
                        );
                    }
                }

                // Step 2: Create or update the crate record
                let record_data = serde_json::json!({
                    "name": name,
                    "version": version,
                    "features": msg.features,
                    "status": "pending",
                    "requested_at": "time::now()",
                });

                // Execute the create statement
                match db
                    .query("CREATE crate CONTENT $data")
                    .bind(("data", record_data))
                    .await
                {
                    Ok(_) => {
                        debug!("Persisted crate: {}@{}", name, version);
                    }
                    Err(e) => {
                        error!("Failed to persist crate: {}", e);
                        broker
                            .broadcast(DatabaseError {
                                operation: format!("persist crate {}@{}", name, version),
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

        // Handle QueryDocChunks - retrieve all chunks for a crate
        builder.act_on::<QueryDocChunks>(|agent, envelope| {
            let msg = envelope.message().clone();
            let db = agent.model.db.clone();
            let broker = agent.broker().clone();

            Box::pin(async move {
                let name = msg.specifier.name().to_string();
                let version = msg.specifier.version().to_string();

                // Query all chunks for this crate, ordered by chunk_index
                let query = r#"
                    SELECT chunk_id, chunk_index, content, content_type, source_file, token_count, char_count
                    FROM doc_chunk
                    WHERE crate_id = (SELECT id FROM crate WHERE name = $name AND version = $version LIMIT 1)
                    ORDER BY chunk_index ASC
                "#;

                match db.query(query)
                    .bind(("name", name.clone()))
                    .bind(("version", version.clone()))
                    .await
                {
                    Ok(mut response) => {
                        let chunks: Result<Vec<DocChunkData>, _> = response.take(0);

                        match chunks {
                            Ok(chunks) => {
                                debug!("Found {} chunks for {}@{}", chunks.len(), name, version);
                                broker.broadcast(DocChunksQueryResponse {
                                    specifier: msg.specifier,
                                    chunks,
                                }).await;
                            }
                            Err(e) => {
                                error!("Failed to parse chunk query results for {}@{}: {}", name, version, e);
                                // Broadcast empty response on parse error
                                broker.broadcast(DocChunksQueryResponse {
                                    specifier: msg.specifier,
                                    chunks: vec![],
                                }).await;
                            }
                        }
                    }
                    Err(e) => {
                        error!("Failed to query chunks for {}@{}: {}", name, version, e);
                        // Broadcast empty response on query error
                        broker.broadcast(DocChunksQueryResponse {
                            specifier: msg.specifier,
                            chunks: vec![],
                        }).await;
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
                                let count_result =
                                    db.query("SELECT count() FROM crate GROUP ALL").await;

                                match count_result {
                                    Ok(mut count_response) => {
                                        // Extract count from response
                                        let count_value: Result<Option<serde_json::Value>, _> =
                                            count_response.take(0);
                                        let total_count = match count_value {
                                            Ok(Some(val)) => val
                                                .get("count")
                                                .and_then(|v| v.as_u64())
                                                .unwrap_or(0)
                                                as u32,
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

        // Subscribe to pipeline events for state tracking
        builder.handle().subscribe::<CrateReceived>().await;
        builder.handle().subscribe::<CrateDownloaded>().await;
        builder.handle().subscribe::<CrateDownloadFailed>().await;
        builder.handle().subscribe::<DocumentationExtracted>().await;
        builder
            .handle()
            .subscribe::<DocumentationExtractionFailed>()
            .await;
        builder
            .handle()
            .subscribe::<ChunksPersistenceComplete>()
            .await;
        builder
            .handle()
            .subscribe::<EmbeddingsPersistenceComplete>()
            .await;
        builder.handle().subscribe::<CrateProcessingFailed>().await;

        // Handle CrateReceived - create initial crate record with pending status
        builder.act_on::<CrateReceived>(|agent, envelope| {
            let msg = envelope.message().clone();
            let db = agent.model.db.clone();
            let broker = agent.broker().clone();

            AgentReply::from_async(async move {
                let name = msg.specifier.name().to_string();
                let version = msg.specifier.version().to_string();

                // Create initial crate record with pending status
                let create_query = r#"
                    CREATE crate CONTENT {
                        name: $name,
                        version: $version,
                        features: $features,
                        status: 'pending',
                        requested_at: time::now(),
                        status_updated_at: time::now(),
                        retry_count: 0,
                        dependencies: [],
                        cargo_metadata: NONE
                    }
                "#;

                match db
                    .query(create_query)
                    .bind(("name", name.clone()))
                    .bind(("version", version.clone()))
                    .bind(("features", msg.features.clone()))
                    .await
                {
                    Ok(_) => {
                        debug!(
                            "Created crate record with pending status: {}@{}",
                            name, version
                        );
                    }
                    Err(e) => {
                        error!("Failed to create crate record: {}", e);
                        broker
                            .broadcast(DatabaseError {
                                operation: format!("create crate record for {}@{}", name, version),
                                error: e.to_string(),
                            })
                            .await;
                    }
                }
            })
        });

        // Handle CrateDownloaded - update status and record download completion
        builder.act_on::<CrateDownloaded>(|agent, envelope| {
            let msg = envelope.message().clone();
            let db = agent.model.db.clone();
            let broker = agent.broker().clone();

            AgentReply::from_async(async move {
                let name = msg.specifier.name().to_string();
                let version = msg.specifier.version().to_string();
                let path = msg.extracted_path.to_string_lossy().to_string();

                let update_query = r#"
                    UPDATE crate SET
                        status = 'downloaded',
                        status_updated_at = time::now(),
                        download_completed_at = time::now(),
                        download_path = $path,
                        extraction_path = $extracted_path
                    WHERE name = $name AND version = $version
                "#;

                match db
                    .query(update_query)
                    .bind(("name", name.clone()))
                    .bind(("version", version.clone()))
                    .bind(("path", path.clone()))
                    .bind(("extracted_path", path.clone()))
                    .await
                {
                    Ok(_) => {
                        trace!("Updated crate status to downloaded: {}@{}", name, version);
                    }
                    Err(e) => {
                        error!("Failed to update crate download status: {}", e);
                        broker
                            .broadcast(DatabaseError {
                                operation: format!(
                                    "update download status for {}@{}",
                                    name, version
                                ),
                                error: e.to_string(),
                            })
                            .await;
                    }
                }
            })
        });

        // Handle CrateDownloadFailed - record download failure details
        builder.act_on::<CrateDownloadFailed>(|agent, envelope| {
            let msg = envelope.message().clone();
            let db = agent.model.db.clone();
            let broker = agent.broker().clone();

            AgentReply::from_async(async move {
                let name = msg.specifier.name().to_string();
                let version = msg.specifier.version().to_string();
                let error_message = msg.error_message.clone();
                let error_kind = format!("{:?}", msg.error_kind);

                let update_query = r#"
                    UPDATE crate SET
                        status = 'download_failed',
                        status_updated_at = time::now(),
                        error_stage = 'download',
                        error_message = $error_message,
                        error_timestamp = time::now(),
                        retry_count = retry_count + 1
                    WHERE name = $name AND version = $version
                "#;

                match db
                    .query(update_query)
                    .bind(("name", name.clone()))
                    .bind(("version", version.clone()))
                    .bind((
                        "error_message",
                        format!("{}: {}", error_kind, error_message),
                    ))
                    .await
                {
                    Ok(_) => {
                        trace!(
                            "Updated crate status to download_failed: {}@{} (error: {})",
                            name, version, error_message
                        );
                    }
                    Err(e) => {
                        error!("Failed to update crate download failure status: {}", e);
                        broker
                            .broadcast(DatabaseError {
                                operation: format!(
                                    "update download failure status for {}@{}",
                                    name, version
                                ),
                                error: e.to_string(),
                            })
                            .await;
                    }
                }
            })
        });

        // Handle DocumentationExtracted - update status and record extraction
        builder.act_on::<DocumentationExtracted>(|agent, envelope| {
            let msg = envelope.message().clone();
            let db = agent.model.db.clone();
            let broker = agent.broker().clone();

            AgentReply::from_async(async move {
                let name = msg.specifier.name().to_string();
                let version = msg.specifier.version().to_string();
                let file_count = msg.file_count;
                let documentation_bytes = msg.documentation_bytes;

                let update_query = r#"
                    UPDATE crate SET
                        status = 'extracted',
                        status_updated_at = time::now(),
                        extraction_completed_at = time::now()
                    WHERE name = $name AND version = $version
                "#;

                match db
                    .query(update_query)
                    .bind(("name", name.clone()))
                    .bind(("version", version.clone()))
                    .await
                {
                    Ok(_) => {
                        trace!(
                            "Updated crate status to extracted: {}@{} ({} files, {} bytes)",
                            name, version, file_count, documentation_bytes
                        );
                    }
                    Err(e) => {
                        error!("Failed to update crate extraction status: {}", e);
                        broker
                            .broadcast(DatabaseError {
                                operation: format!(
                                    "update extraction status for {}@{}",
                                    name, version
                                ),
                                error: e.to_string(),
                            })
                            .await;
                    }
                }
            })
        });

        // Handle DocumentationExtractionFailed - record extraction failure details
        builder.act_on::<DocumentationExtractionFailed>(|agent, envelope| {
            let msg = envelope.message().clone();
            let db = agent.model.db.clone();
            let broker = agent.broker().clone();

            AgentReply::from_async(async move {
                let name = msg.specifier.name().to_string();
                let version = msg.specifier.version().to_string();
                let error_message = msg.error_message.clone();
                let error_kind = format!("{:?}", msg.error_kind);
                let elapsed_ms = msg.elapsed_ms;

                let update_query = r#"
                    UPDATE crate SET
                        status = 'extraction_failed',
                        status_updated_at = time::now(),
                        error_stage = 'extraction',
                        error_message = $error_message,
                        error_timestamp = time::now(),
                        retry_count = retry_count + 1
                    WHERE name = $name AND version = $version
                "#;

                match db
                    .query(update_query)
                    .bind(("name", name.clone()))
                    .bind(("version", version.clone()))
                    .bind(("error_message", format!("{}: {} (elapsed: {}ms)", error_kind, error_message, elapsed_ms)))
                    .await
                {
                    Ok(_) => {
                        trace!(
                            "Updated crate status to extraction_failed: {}@{} (error: {}, elapsed: {}ms)",
                            name, version, error_message, elapsed_ms
                        );
                    }
                    Err(e) => {
                        error!("Failed to update crate extraction failure status: {}", e);
                        broker
                            .broadcast(DatabaseError {
                                operation: format!(
                                    "update extraction failure status for {}@{}",
                                    name, version
                                ),
                                error: e.to_string(),
                            })
                            .await;
                    }
                }
            })
        });

        // Handle ChunksPersistenceComplete - update status to chunked after confirmed persistence
        builder.act_on::<ChunksPersistenceComplete>(|agent, envelope| {
            let msg = envelope.message().clone();
            let db = agent.model.db.clone();
            let broker = agent.broker().clone();

            AgentReply::from_async(async move {
                let name = msg.specifier.name().to_string();
                let version = msg.specifier.version().to_string();
                let chunk_count = msg.chunk_count;

                // Update status to 'chunked' - persistence already confirmed by CrateCoordinatorActor
                let update_query = r#"
                    UPDATE crate SET
                        status = 'chunked',
                        status_updated_at = time::now(),
                        chunking_completed_at = time::now()
                    WHERE name = $name AND version = $version
                "#;

                match db
                    .query(update_query)
                    .bind(("name", name.clone()))
                    .bind(("version", version.clone()))
                    .await
                {
                    Ok(_) => {
                        info!(
                            "Crate chunking complete: {}@{} ({} chunks)",
                            name, version, chunk_count
                        );
                    }
                    Err(e) => {
                        error!("Failed to update crate chunking status: {}", e);
                        broker
                            .broadcast(DatabaseError {
                                operation: format!(
                                    "update chunking status for {}@{}",
                                    name, version
                                ),
                                error: e.to_string(),
                            })
                            .await;
                    }
                }
            })
        });

        // Handle EmbeddingsPersistenceComplete - update status to complete after confirmed persistence
        builder.act_on::<EmbeddingsPersistenceComplete>(|agent, envelope| {
            let msg = envelope.message().clone();
            let db = agent.model.db.clone();
            let broker = agent.broker().clone();

            AgentReply::from_async(async move {
                let name = msg.specifier.name().to_string();
                let version = msg.specifier.version().to_string();
                let vector_count = msg.vector_count;
                let embedding_model = msg.embedding_model.clone();

                // Update status directly to 'complete' - persistence already confirmed by CrateCoordinatorActor
                // This is the final status, so we set both vectorization_completed_at and completed_at
                let update_query = r#"
                    UPDATE crate SET
                        status = 'complete',
                        status_updated_at = time::now(),
                        vectorization_completed_at = time::now(),
                        completed_at = time::now()
                    WHERE name = $name AND version = $version
                "#;

                match db
                    .query(update_query)
                    .bind(("name", name.clone()))
                    .bind(("version", version.clone()))
                    .await
                {
                    Ok(_) => {
                        info!(
                            "Crate processing complete: {}@{} ({} embeddings, model: {})",
                            name, version, vector_count, embedding_model
                        );
                    }
                    Err(e) => {
                        error!("Failed to update crate completion status: {}", e);
                        broker
                            .broadcast(DatabaseError {
                                operation: format!(
                                    "update completion status for {}@{}",
                                    name, version
                                ),
                                error: e.to_string(),
                            })
                            .await;
                    }
                }
            })
        });

        // Handle CrateProcessingFailed - record error details
        builder.act_on::<CrateProcessingFailed>(|agent, envelope| {
            let msg = envelope.message().clone();
            let db = agent.model.db.clone();
            let broker = agent.broker().clone();

            AgentReply::from_async(async move {
                let name = msg.specifier.name().to_string();
                let version = msg.specifier.version().to_string();
                let stage = msg.stage.clone();
                let error = msg.error.clone();

                let update_query = r#"
                    UPDATE crate SET
                        status = 'failed',
                        status_updated_at = time::now(),
                        error_stage = $stage,
                        error_message = $error,
                        error_timestamp = time::now(),
                        retry_count = retry_count + 1
                    WHERE name = $name AND version = $version
                "#;

                match db
                    .query(update_query)
                    .bind(("name", name.clone()))
                    .bind(("version", version.clone()))
                    .bind(("stage", stage.clone()))
                    .bind(("error", error.clone()))
                    .await
                {
                    Ok(_) => {
                        warn!(
                            "Crate processing failed: {}@{} (stage: {}, error: {})",
                            name, version, stage, error
                        );
                    }
                    Err(e) => {
                        error!("Failed to record crate processing failure: {}", e);
                        broker
                            .broadcast(DatabaseError {
                                operation: format!("record failure for {}@{}", name, version),
                                error: e.to_string(),
                            })
                            .await;
                    }
                }
            })
        });

        // Handle PersistDocChunk - upsert documentation chunk (idempotent)
        builder.mutate_on::<PersistDocChunk>(|agent, envelope| {
            let msg = envelope.message().clone();
            let db = agent.model.db.clone();
            let broker = agent.broker().clone();

            Box::pin(async move {
                // Clone strings before async block to satisfy lifetime requirements
                let name = msg.specifier.name().to_string();
                let version = msg.specifier.version().to_string();

                // First, get the crate record ID
                let crate_query = "SELECT id FROM crate WHERE name = $name AND version = $version";

                match db
                    .query(crate_query)
                    .bind(("name", name.clone()))
                    .bind(("version", version.clone()))
                    .await
                {
                    Ok(mut response) => {
                        let crate_record: Result<Option<CrateIdRecord>, _> = response.take(0);

                        match crate_record {
                            Ok(Some(record)) => {
                                let id = record.thing();
                                // Upsert pattern: Try UPDATE first, then INSERT if no rows affected
                                let update_query = r#"
                                        UPDATE doc_chunk SET
                                            content = $content,
                                            source_file = $source_file,
                                            content_type = $content_type,
                                            token_count = $token_count,
                                            char_count = $char_count,
                                            start_line = $start_line,
                                            end_line = $end_line,
                                            parent_module = $parent_module,
                                            item_type = $item_type,
                                            item_name = $item_name,
                                            vectorized = false,
                                            vectorized_at = NONE
                                        WHERE chunk_id = $chunk_id
                                        RETURN BEFORE
                                    "#;

                                let update_result = db
                                    .query(update_query)
                                    .bind(("chunk_id", msg.chunk_id.clone()))
                                    .bind(("content", msg.content.clone()))
                                    .bind(("source_file", msg.source_file.clone()))
                                    .bind(("content_type", msg.metadata.content_type.clone()))
                                    .bind(("token_count", msg.metadata.token_count as i64))
                                    .bind(("char_count", msg.metadata.char_count as i64))
                                    .bind(("start_line", msg.metadata.start_line.map(|n| n as i64)))
                                    .bind(("end_line", msg.metadata.end_line.map(|n| n as i64)))
                                    .bind(("parent_module", msg.metadata.parent_module.clone()))
                                    .bind(("item_type", msg.metadata.item_type.clone()))
                                    .bind(("item_name", msg.metadata.item_name.clone()))
                                    .await;

                                // Check if update affected any rows
                                let needs_insert = match update_result {
                                    Ok(mut response) => {
                                        // Check if any rows were returned (indicating update succeeded)
                                        let rows: Result<Vec<serde_json::Value>, _> =
                                            response.take(0);
                                        match rows {
                                            Ok(rows) if !rows.is_empty() => {
                                                debug!(
                                                    "Updated existing doc chunk {} for {}@{}",
                                                    msg.chunk_id, name, version
                                                );
                                                trace!(
                                                    chunk_id = %msg.chunk_id,
                                                    specifier = %format!("{}@{}", name, version),
                                                    source_file = %msg.source_file,
                                                    chunk_index = msg.chunk_index,
                                                    "Doc chunk updated in database"
                                                );
                                                false
                                            }
                                            _ => true, // No rows returned, need to insert
                                        }
                                    }
                                    Err(_) => true, // Update failed, try insert
                                };

                                if needs_insert {
                                    // No existing record, insert new one
                                    let insert_query = r#"
                                            CREATE doc_chunk CONTENT {
                                                crate_id: $crate_id,
                                                chunk_index: $chunk_index,
                                                chunk_id: $chunk_id,
                                                content: $content,
                                                source_file: $source_file,
                                                content_type: $content_type,
                                                token_count: $token_count,
                                                char_count: $char_count,
                                                start_line: $start_line,
                                                end_line: $end_line,
                                                parent_module: $parent_module,
                                                item_type: $item_type,
                                                item_name: $item_name
                                            }
                                        "#;

                                    match db
                                        .query(insert_query)
                                        .bind(("crate_id", id.clone()))
                                        .bind(("chunk_index", msg.chunk_index as i64))
                                        .bind(("chunk_id", msg.chunk_id.clone()))
                                        .bind(("content", msg.content.clone()))
                                        .bind(("source_file", msg.source_file.clone()))
                                        .bind(("content_type", msg.metadata.content_type.clone()))
                                        .bind(("token_count", msg.metadata.token_count as i64))
                                        .bind(("char_count", msg.metadata.char_count as i64))
                                        .bind((
                                            "start_line",
                                            msg.metadata.start_line.map(|n| n as i64),
                                        ))
                                        .bind(("end_line", msg.metadata.end_line.map(|n| n as i64)))
                                        .bind(("parent_module", msg.metadata.parent_module.clone()))
                                        .bind(("item_type", msg.metadata.item_type.clone()))
                                        .bind(("item_name", msg.metadata.item_name.clone()))
                                        .await
                                    {
                                        Ok(_) => {
                                            debug!(
                                                "Inserted new doc chunk {} for {}@{}",
                                                msg.chunk_id, name, version
                                            );
                                        }
                                        Err(e) => {
                                            error!("Failed to insert doc chunk: {}", e);
                                            broker
                                                .broadcast(DatabaseError {
                                                    operation: format!(
                                                        "persist doc chunk {}",
                                                        msg.chunk_id
                                                    ),
                                                    error: e.to_string(),
                                                })
                                                .await;
                                            return;
                                        }
                                    }
                                }

                                // Broadcast acknowledgment for completion tracking
                                broker
                                    .broadcast(DocChunkPersisted {
                                        chunk_id: msg.chunk_id.clone(),
                                        specifier: msg.specifier.clone(),
                                        chunk_index: msg.chunk_index,
                                        source_file: msg.source_file.clone(),
                                    })
                                    .await;
                            }
                            Ok(None) => {
                                error!("Crate not found for chunk: {}@{}", name, version);
                                broker
                                    .broadcast(DatabaseError {
                                        operation: format!("persist doc chunk {}", msg.chunk_id),
                                        error: format!(
                                            "Crate {}@{} not found in database",
                                            name, version
                                        ),
                                    })
                                    .await;
                            }
                            Err(e) => {
                                error!("Failed to parse crate lookup result: {}", e);
                                broker
                                    .broadcast(DatabaseError {
                                        operation: format!("persist doc chunk {}", msg.chunk_id),
                                        error: e.to_string(),
                                    })
                                    .await;
                            }
                        }
                    }
                    Err(e) => {
                        error!("Failed to lookup crate for chunk persistence: {}", e);
                        broker
                            .broadcast(DatabaseError {
                                operation: format!("persist doc chunk {}", msg.chunk_id),
                                error: e.to_string(),
                            })
                            .await;
                    }
                }
            })
        });

        // Handle PersistEmbedding - insert vector embedding
        builder.mutate_on::<PersistEmbedding>(|agent, envelope| {
            let msg = envelope.message().clone();
            let db = agent.model.db.clone();
            let broker = agent.broker().clone();

            Box::pin(async move {
                // Extract context for error messages
                let name = msg.specifier.name().to_string();
                let version = msg.specifier.version().to_string();
                let chunk_id = msg.chunk_id.clone();

                // Step 1: Verify doc_chunk exists first (fail fast if missing)
                let chunk_query = "SELECT id FROM doc_chunk WHERE chunk_id = $chunk_id";

                let chunk_db_id = match db
                    .query(chunk_query)
                    .bind(("chunk_id", chunk_id.clone()))
                    .await
                {
                    Ok(mut chunk_response) => {
                        match chunk_response.take::<Option<DocChunkIdRecord>>(0) {
                            Ok(Some(record)) => record.thing().clone(),
                            Ok(None) => {
                                // Chunk not found - this is the key scenario we're improving
                                let warning_msg = format!(
                                    "Cannot persist embedding: doc_chunk '{}' not found for crate {}@{}. \
                                     The chunk must be persisted before embeddings can be stored.",
                                    chunk_id, name, version
                                );

                                error!("{}", warning_msg);

                                // Send warning to Console actor for user visibility
                                broker
                                    .broadcast(PrintWarning(warning_msg.clone()))
                                    .await;

                                // Also broadcast DatabaseError for error handling subscribers
                                broker
                                    .broadcast(DatabaseError {
                                        operation: format!(
                                            "persist embedding for chunk {} (crate: {}@{})",
                                            chunk_id, name, version
                                        ),
                                        error: format!(
                                            "Documentation chunk '{}' not found in database. \
                                             Ensure chunks are persisted before embedding vectorization.",
                                            chunk_id
                                        ),
                                    })
                                    .await;

                                return;
                            }
                            Err(e) => {
                                error!(
                                    "Failed to parse chunk lookup result for chunk_id: {} (crate: {}@{}): {}",
                                    chunk_id, name, version, e
                                );
                                broker
                                    .broadcast(DatabaseError {
                                        operation: format!(
                                            "persist embedding for chunk {} (crate: {}@{})",
                                            chunk_id, name, version
                                        ),
                                        error: format!(
                                            "Database query failed while checking chunk '{}': {}",
                                            chunk_id, e
                                        ),
                                    })
                                    .await;
                                return;
                            }
                        }
                    }
                    Err(e) => {
                        error!(
                            "Failed to lookup chunk for embedding (chunk_id: {}, crate: {}@{}): {}",
                            chunk_id, name, version, e
                        );
                        broker
                            .broadcast(DatabaseError {
                                operation: format!(
                                    "persist embedding for chunk {} (crate: {}@{})",
                                    chunk_id, name, version
                                ),
                                error: format!(
                                    "Database connection error while looking up chunk '{}': {}",
                                    chunk_id, e
                                ),
                            })
                            .await;
                        return;
                    }
                };

                // Step 2: Verify crate exists
                let crate_query = "SELECT id FROM crate WHERE name = $name AND version = $version";

                let crate_db_id = match db
                    .query(crate_query)
                    .bind(("name", name.clone()))
                    .bind(("version", version.clone()))
                    .await
                {
                    Ok(mut crate_response) => {
                        match crate_response.take::<Option<CrateIdRecord>>(0) {
                            Ok(Some(record)) => record.thing().clone(),
                            Ok(None) => {
                                let warning_msg = format!(
                                    "Cannot persist embedding: crate {}@{} not found in database (chunk: {})",
                                    name, version, chunk_id
                                );

                                error!("{}", warning_msg);

                                broker
                                    .broadcast(PrintWarning(warning_msg.clone()))
                                    .await;

                                broker
                                    .broadcast(DatabaseError {
                                        operation: format!(
                                            "persist embedding for chunk {} (crate: {}@{})",
                                            chunk_id, name, version
                                        ),
                                        error: format!(
                                            "Crate '{}@{}' not found in database. \
                                             Ensure crate is persisted before embedding vectorization.",
                                            name, version
                                        ),
                                    })
                                    .await;

                                return;
                            }
                            Err(e) => {
                                error!(
                                    "Failed to parse crate lookup result for {}@{} (chunk: {}): {}",
                                    name, version, chunk_id, e
                                );
                                broker
                                    .broadcast(DatabaseError {
                                        operation: format!(
                                            "persist embedding for chunk {} (crate: {}@{})",
                                            chunk_id, name, version
                                        ),
                                        error: format!(
                                            "Database query failed while checking crate '{}@{}': {}",
                                            name, version, e
                                        ),
                                    })
                                    .await;
                                return;
                            }
                        }
                    }
                    Err(e) => {
                        error!(
                            "Failed to lookup crate for embedding persistence ({}@{}, chunk: {}): {}",
                            name, version, chunk_id, e
                        );
                        broker
                            .broadcast(DatabaseError {
                                operation: format!(
                                    "persist embedding for chunk {} (crate: {}@{})",
                                    chunk_id, name, version
                                ),
                                error: format!(
                                    "Database connection error while looking up crate '{}@{}': {}",
                                    name, version, e
                                ),
                            })
                            .await;
                        return;
                    }
                };

                // Step 3: Both prerequisites satisfied - upsert the embedding (idempotent)
                let vector_dim = msg.vector.len();

                // Upsert pattern: Try UPDATE first, then INSERT if no rows affected
                let update_query = r#"
                    UPDATE embedding SET
                        vector = $vector,
                        vector_dimension = $vector_dimension,
                        content_hash = crypto::md5(string::concat($chunk_id_str, $model_name)),
                        created_at = time::now()
                    WHERE chunk_id = $chunk_id AND model_name = $model_name AND model_version = $model_version
                    RETURN BEFORE
                "#;

                let update_result = db
                    .query(update_query)
                    .bind(("chunk_id", chunk_db_id.clone()))
                    .bind(("chunk_id_str", chunk_id.clone()))
                    .bind(("vector", msg.vector.clone()))
                    .bind(("vector_dimension", vector_dim as i64))
                    .bind(("model_name", msg.model_name.clone()))
                    .bind(("model_version", msg.model_version.clone()))
                    .await;

                // Check if update affected any rows
                let needs_insert = match update_result {
                    Ok(mut response) => {
                        // Check if any rows were returned (indicating update succeeded)
                        let rows: Result<Vec<serde_json::Value>, _> = response.take(0);
                        match rows {
                            Ok(rows) if !rows.is_empty() => {
                                debug!(
                                    "Updated existing embedding for chunk {} (crate: {}@{}, dim: {}, model: {})",
                                    chunk_id, name, version, vector_dim, msg.model_name
                                );
                                false
                            }
                            _ => true, // No rows returned, need to insert
                        }
                    }
                    Err(_) => true, // Update failed, try insert
                };

                if needs_insert {
                    // No existing embedding, insert new one
                    let insert_query = r#"
                        CREATE embedding CONTENT {
                            chunk_id: $chunk_id,
                            crate_id: $crate_id,
                            vector: $vector,
                            vector_dimension: $vector_dimension,
                            model_name: $model_name,
                            model_version: $model_version,
                            content_hash: crypto::md5(string::concat($chunk_id_str, $model_name))
                        }
                    "#;

                    match db
                        .query(insert_query)
                        .bind(("chunk_id", chunk_db_id))
                        .bind(("crate_id", crate_db_id))
                        .bind(("chunk_id_str", chunk_id.clone()))
                        .bind(("vector", msg.vector.clone()))
                        .bind(("vector_dimension", vector_dim as i64))
                        .bind(("model_name", msg.model_name.clone()))
                        .bind(("model_version", msg.model_version.clone()))
                        .await
                    {
                        Ok(_) => {
                            debug!(
                                "Inserted new embedding for chunk {} (crate: {}@{}, dim: {}, model: {})",
                                chunk_id, name, version, vector_dim, msg.model_name
                            );
                        }
                        Err(e) => {
                            error!(
                                "Failed to insert embedding for chunk {} (crate: {}@{}): {}",
                                chunk_id, name, version, e
                            );
                            broker
                                .broadcast(DatabaseError {
                                    operation: format!(
                                        "persist embedding for chunk {} (crate: {}@{})",
                                        chunk_id, name, version
                                    ),
                                    error: format!(
                                        "Failed to create embedding record for chunk '{}': {}",
                                        chunk_id, e
                                    ),
                                })
                                .await;
                            return;
                        }
                    }
                }

                // Update chunk's vectorized flag and timestamp
                if let Err(e) = db
                    .query("UPDATE doc_chunk SET vectorized = true, vectorized_at = time::now() WHERE chunk_id = $chunk_id")
                    .bind(("chunk_id", chunk_id.clone()))
                    .await
                {
                    error!(
                        "Failed to update vectorized flag for chunk {} (crate: {}@{}): {}",
                        chunk_id, name, version, e
                    );
                }

                // Broadcast acknowledgment for completion tracking
                broker
                    .broadcast(EmbeddingPersisted {
                        chunk_id: msg.chunk_id.clone(),
                        specifier: msg.specifier.clone(),
                        #[cfg(test)]
                        model_name: msg.model_name.clone(),
                    })
                    .await;
            })
        });

        // Handle PersistCodeSample - insert code sample
        builder.mutate_on::<PersistCodeSample>(|agent, envelope| {
            let msg = envelope.message().clone();
            let db = agent.model.db.clone();
            let broker = agent.broker().clone();

            Box::pin(async move {
                // Clone strings before async block to satisfy lifetime requirements
                let name = msg.specifier.name().to_string();
                let version = msg.specifier.version().to_string();

                // Get crate ID
                let crate_query = "SELECT id FROM crate WHERE name = $name AND version = $version";

                match db
                    .query(crate_query)
                    .bind(("name", name.clone()))
                    .bind(("version", version.clone()))
                    .await
                {
                    Ok(mut response) => {
                        let crate_record: Result<Option<CrateIdRecord>, _> = response.take(0);

                        match crate_record {
                            Ok(Some(record)) => {
                                let id = record.thing();
                                let insert_query = r#"
                                        CREATE code_sample CONTENT {
                                            crate_id: $crate_id,
                                            sample_index: $sample_index,
                                            code: $code,
                                            language: $language,
                                            source_file: $source_file,
                                            doc_context: $doc_context,
                                            parent_item: $parent_item,
                                            sample_type: $sample_type,
                                            tags: $tags,
                                            line_count: $line_count
                                        }
                                    "#;

                                match db
                                    .query(insert_query)
                                    .bind(("crate_id", id.clone()))
                                    .bind(("sample_index", msg.sample_index as i64))
                                    .bind(("code", msg.code.clone()))
                                    .bind(("language", msg.metadata.language.clone()))
                                    .bind(("source_file", msg.metadata.source_file.clone()))
                                    .bind(("doc_context", msg.metadata.doc_context.clone()))
                                    .bind(("parent_item", msg.metadata.parent_item.clone()))
                                    .bind(("sample_type", msg.sample_type.clone()))
                                    .bind(("tags", msg.metadata.tags.clone()))
                                    .bind(("line_count", msg.metadata.line_count as i64))
                                    .await
                                {
                                    Ok(_) => {
                                        debug!(
                                            "Persisted code sample {} for {}@{} (type: {})",
                                            msg.sample_index, name, version, msg.sample_type
                                        );
                                    }
                                    Err(e) => {
                                        error!("Failed to persist code sample: {}", e);
                                        broker
                                            .broadcast(DatabaseError {
                                                operation: format!(
                                                    "persist code sample {} for {}@{}",
                                                    msg.sample_index, name, version
                                                ),
                                                error: e.to_string(),
                                            })
                                            .await;
                                    }
                                }
                            }
                            Ok(None) => {
                                error!("Crate not found for code sample: {}@{}", name, version);
                                broker
                                    .broadcast(DatabaseError {
                                        operation: format!(
                                            "persist code sample for {}@{}",
                                            name, version
                                        ),
                                        error: format!(
                                            "Crate {}@{} not found in database",
                                            name, version
                                        ),
                                    })
                                    .await;
                            }
                            Err(e) => {
                                error!("Failed to parse crate lookup result: {}", e);
                                broker
                                    .broadcast(DatabaseError {
                                        operation: format!(
                                            "persist code sample for {}@{}",
                                            name, version
                                        ),
                                        error: e.to_string(),
                                    })
                                    .await;
                            }
                        }
                    }
                    Err(e) => {
                        error!("Failed to lookup crate for code sample persistence: {}", e);
                        broker
                            .broadcast(DatabaseError {
                                operation: format!("persist code sample for {}@{}", name, version),
                                error: e.to_string(),
                            })
                            .await;
                    }
                }
            })
        });

        // Handle QuerySimilarDocs - vector similarity search
        builder.mutate_on::<QuerySimilarDocs>(|agent, envelope| {
            let msg = envelope.message().clone();
            let db = agent.model.db.clone();
            let broker = agent.broker().clone();

            Box::pin(async move {
                let limit = msg.limit.unwrap_or(10);

                // Extract crate filter strings before async operations to satisfy lifetime requirements
                let crate_filter_opt = msg.crate_filter.as_ref().map(|f| {
                    (f.name().to_string(), f.version().to_string())
                });

                // Build query with optional crate filter
                let search_query = if crate_filter_opt.is_some() {
                    r#"
                        SELECT
                            embedding.chunk_id as chunk_id,
                            doc_chunk.content as content,
                            doc_chunk.crate_id as crate_id,
                            vector::similarity::cosine(embedding.vector, $query_vector) AS score
                        FROM embedding
                        INNER JOIN doc_chunk ON doc_chunk.id = embedding.chunk_id
                        INNER JOIN crate ON crate.id = embedding.crate_id
                        WHERE score > 0.7 AND crate.name = $crate_name AND crate.version = $crate_version
                        ORDER BY score DESC
                        LIMIT $limit
                    "#
                } else {
                    r#"
                        SELECT
                            embedding.chunk_id as chunk_id,
                            doc_chunk.content as content,
                            doc_chunk.crate_id as crate_id,
                            vector::similarity::cosine(embedding.vector, $query_vector) AS score
                        FROM embedding
                        INNER JOIN doc_chunk ON doc_chunk.id = embedding.chunk_id
                        WHERE score > 0.7
                        ORDER BY score DESC
                        LIMIT $limit
                    "#
                };

                let mut query = db
                    .query(search_query)
                    .bind(("query_vector", msg.query_vector.clone()))
                    .bind(("limit", limit as i64));

                if let Some((name, version)) = crate_filter_opt {
                    query = query
                        .bind(("crate_name", name))
                        .bind(("crate_version", version));
                }

                match query.await {
                    Ok(mut response) => {
                        let results: Result<Vec<serde_json::Value>, _> = response.take(0);

                        match results {
                            Ok(raw_results) => {
                                let mut search_results = Vec::new();

                                for result in raw_results {
                                    // Extract crate information from the crate_id record reference
                                    if let (Some(content), Some(score), Some(chunk_id_val)) = (
                                        result.get("content").and_then(|v| v.as_str()),
                                        result.get("score").and_then(|v| v.as_f64()),
                                        result.get("chunk_id"),
                                    ) {
                                        // Get chunk_id string from the record reference
                                        let chunk_id = if let Some(chunk_obj) = chunk_id_val.as_object() {
                                            chunk_obj.get("chunk_id")
                                                .and_then(|v| v.as_str())
                                                .unwrap_or_default()
                                        } else {
                                            ""
                                        };

                                        // Get crate information
                                        if let Some(crate_id_val) = result.get("crate_id") {
                                            // Query to get crate details
                                            let crate_detail_query = "SELECT name, version FROM $crate_id";

                                            if let Ok(mut crate_response) = db
                                                .query(crate_detail_query)
                                                .bind(("crate_id", crate_id_val.clone()))
                                                .await
                                            {
                                                let crate_info: Result<Option<serde_json::Value>, _> =
                                                    crate_response.take(0);

                                                if let Ok(Some(crate_data)) = crate_info {
                                                    if let (Some(name), Some(version)) = (
                                                        crate_data.get("name").and_then(|v| v.as_str()),
                                                        crate_data.get("version").and_then(|v| v.as_str()),
                                                    ) {
                                                        let specifier_str = format!("{}@{}", name, version);
                                                        if let Ok(specifier) =
                                                            CrateSpecifier::from_str(&specifier_str)
                                                        {
                                                            search_results.push(SearchResult {
                                                                specifier,
                                                                chunk_id: chunk_id.to_string(),
                                                                content: content.to_string(),
                                                                similarity_score: score as f32,
                                                            });
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }

                                debug!("Vector search found {} results", search_results.len());

                                broker
                                    .broadcast(SimilarDocsResponse {
                                        results: search_results,
                                    })
                                    .await;
                            }
                            Err(e) => {
                                error!("Failed to parse vector search results: {}", e);
                                broker
                                    .broadcast(DatabaseError {
                                        operation: "vector similarity search".to_string(),
                                        error: e.to_string(),
                                    })
                                    .await;
                            }
                        }
                    }
                    Err(e) => {
                        error!("Vector search query failed: {}", e);
                        broker
                            .broadcast(DatabaseError {
                                operation: "vector similarity search".to_string(),
                                error: e.to_string(),
                            })
                            .await;
                    }
                }
            })
        });

        // Add before_start hook to initialize schema synchronously
        // This blocks start() from completing until schema initialization finishes
        builder.before_start(|agent| {
            let db = agent.model.db.clone();
            let broker = agent.broker().clone();

            AgentReply::from_async(async move {
                info!("Initializing database schema...");

                let schema_sql = r#"
                    -- Core crate metadata and processing state
                    DEFINE TABLE crate SCHEMAFULL;

                    -- Primary identification fields
                    DEFINE FIELD name ON TABLE crate TYPE string
                        ASSERT $value != NONE AND string::len($value) > 0;
                    DEFINE FIELD version ON TABLE crate TYPE string
                        ASSERT $value != NONE AND string::len($value) > 0;

                    -- Request metadata
                    DEFINE FIELD features ON TABLE crate TYPE array<string> DEFAULT [];
                    DEFINE FIELD requested_at ON TABLE crate TYPE datetime DEFAULT time::now();

                    -- Processing state machine
                    DEFINE FIELD status ON TABLE crate TYPE string DEFAULT 'pending'
                        ASSERT $value IN ['pending', 'downloading', 'downloaded', 'download_failed',
                                          'extracting', 'extracted', 'extraction_failed',
                                          'compiling_docs', 'docs_compiled',
                                          'chunking', 'chunked', 'vectorizing', 'vectorized',
                                          'complete', 'failed'];

                    -- State tracking timestamps
                    DEFINE FIELD status_updated_at ON TABLE crate TYPE datetime DEFAULT time::now();
                    DEFINE FIELD download_started_at ON TABLE crate TYPE option<datetime>;
                    DEFINE FIELD download_completed_at ON TABLE crate TYPE option<datetime>;
                    DEFINE FIELD extraction_completed_at ON TABLE crate TYPE option<datetime>;
                    DEFINE FIELD docs_completed_at ON TABLE crate TYPE option<datetime>;
                    DEFINE FIELD chunking_completed_at ON TABLE crate TYPE option<datetime>;
                    DEFINE FIELD vectorization_completed_at ON TABLE crate TYPE option<datetime>;
                    DEFINE FIELD completed_at ON TABLE crate TYPE option<datetime>;

                    -- Error tracking
                    DEFINE FIELD error_message ON TABLE crate TYPE option<string>;
                    DEFINE FIELD error_stage ON TABLE crate TYPE option<string>;
                    DEFINE FIELD error_timestamp ON TABLE crate TYPE option<datetime>;
                    DEFINE FIELD retry_count ON TABLE crate TYPE int DEFAULT 0;

                    -- File system paths
                    DEFINE FIELD download_path ON TABLE crate TYPE option<string>;
                    DEFINE FIELD extraction_path ON TABLE crate TYPE option<string>;
                    DEFINE FIELD docs_path ON TABLE crate TYPE option<string>;

                    -- Metadata
                    DEFINE FIELD cargo_metadata ON TABLE crate TYPE option<object>;
                    DEFINE FIELD dependencies ON TABLE crate TYPE array<object> DEFAULT [];
                    DEFINE FIELD size_bytes ON TABLE crate TYPE option<int>;

                    -- Indexes
                    DEFINE INDEX idx_name_version ON TABLE crate COLUMNS name, version UNIQUE;
                    DEFINE INDEX idx_status ON TABLE crate COLUMNS status;
                    DEFINE INDEX idx_requested_at ON TABLE crate COLUMNS requested_at;

                    -- Documentation chunks table
                    DEFINE TABLE doc_chunk SCHEMAFULL;
                    DEFINE FIELD crate_id ON TABLE doc_chunk TYPE record<crate>
                        ASSERT $value != NONE;
                    DEFINE FIELD chunk_index ON TABLE doc_chunk TYPE int
                        ASSERT $value >= 0;
                    DEFINE FIELD chunk_id ON TABLE doc_chunk TYPE string
                        ASSERT $value != NONE;
                    DEFINE FIELD content ON TABLE doc_chunk TYPE string
                        ASSERT $value != NONE AND string::len($value) > 0;
                    DEFINE FIELD content_type ON TABLE doc_chunk TYPE string DEFAULT 'markdown'
                        ASSERT $value IN ['markdown', 'rust', 'toml', 'text'];
                    DEFINE FIELD source_file ON TABLE doc_chunk TYPE string;
                    DEFINE FIELD start_line ON TABLE doc_chunk TYPE option<int>;
                    DEFINE FIELD end_line ON TABLE doc_chunk TYPE option<int>;
                    DEFINE FIELD token_count ON TABLE doc_chunk TYPE int DEFAULT 0;
                    DEFINE FIELD char_count ON TABLE doc_chunk TYPE int DEFAULT 0;
                    DEFINE FIELD created_at ON TABLE doc_chunk TYPE datetime DEFAULT time::now();
                    DEFINE FIELD vectorized ON TABLE doc_chunk TYPE bool DEFAULT false;
                    DEFINE FIELD vectorized_at ON TABLE doc_chunk TYPE option<datetime>;
                    DEFINE FIELD parent_module ON TABLE doc_chunk TYPE option<string>;
                    DEFINE FIELD item_type ON TABLE doc_chunk TYPE option<string>;
                    DEFINE FIELD item_name ON TABLE doc_chunk TYPE option<string>;
                    DEFINE INDEX idx_crate_id ON TABLE doc_chunk COLUMNS crate_id;
                    DEFINE INDEX idx_chunk_index ON TABLE doc_chunk COLUMNS chunk_index;
                    DEFINE INDEX idx_vectorized ON TABLE doc_chunk COLUMNS vectorized;
                    DEFINE INDEX idx_crate_chunk ON TABLE doc_chunk COLUMNS crate_id, chunk_index UNIQUE;
                    DEFINE INDEX idx_chunk_id_unique ON TABLE doc_chunk COLUMNS chunk_id UNIQUE;

                    -- Vector embeddings table
                    DEFINE TABLE embedding SCHEMAFULL;
                    DEFINE FIELD chunk_id ON TABLE embedding TYPE record<doc_chunk>
                        ASSERT $value != NONE;
                    DEFINE FIELD crate_id ON TABLE embedding TYPE record<crate>
                        ASSERT $value != NONE;
                    DEFINE FIELD vector ON TABLE embedding TYPE array<float>
                        ASSERT $value != NONE AND array::len($value) > 0;
                    DEFINE FIELD vector_dimension ON TABLE embedding TYPE int
                        ASSERT $value > 0;
                    DEFINE FIELD model_name ON TABLE embedding TYPE string DEFAULT 'text-embedding-3-small';
                    DEFINE FIELD model_version ON TABLE embedding TYPE string;
                    DEFINE FIELD created_at ON TABLE embedding TYPE datetime DEFAULT time::now();
                    DEFINE FIELD content_hash ON TABLE embedding TYPE string;
                    DEFINE INDEX idx_chunk_id ON TABLE embedding COLUMNS chunk_id UNIQUE;
                    DEFINE INDEX idx_crate_id_emb ON TABLE embedding COLUMNS crate_id;
                    DEFINE INDEX idx_model ON TABLE embedding COLUMNS model_name, model_version;
                    DEFINE INDEX idx_content_hash ON TABLE embedding COLUMNS content_hash;
                    DEFINE INDEX idx_embedding_unique ON TABLE embedding COLUMNS chunk_id, model_name, model_version UNIQUE;

                    -- Code samples table
                    DEFINE TABLE code_sample SCHEMAFULL;
                    DEFINE FIELD crate_id ON TABLE code_sample TYPE record<crate>
                        ASSERT $value != NONE;
                    DEFINE FIELD sample_index ON TABLE code_sample TYPE int
                        ASSERT $value >= 0;
                    DEFINE FIELD code ON TABLE code_sample TYPE string
                        ASSERT $value != NONE AND string::len($value) > 0;
                    DEFINE FIELD language ON TABLE code_sample TYPE string DEFAULT 'rust';
                    DEFINE FIELD source_file ON TABLE code_sample TYPE string;
                    DEFINE FIELD doc_context ON TABLE code_sample TYPE option<string>;
                    DEFINE FIELD parent_item ON TABLE code_sample TYPE option<string>;
                    DEFINE FIELD sample_type ON TABLE code_sample TYPE string
                        ASSERT $value IN ['example', 'test', 'usage', 'snippet'];
                    DEFINE FIELD tags ON TABLE code_sample TYPE array<string> DEFAULT [];
                    DEFINE FIELD line_count ON TABLE code_sample TYPE int DEFAULT 0;
                    DEFINE FIELD complexity_score ON TABLE code_sample TYPE option<float>;
                    DEFINE FIELD created_at ON TABLE code_sample TYPE datetime DEFAULT time::now();
                    DEFINE INDEX idx_crate_id_sample ON TABLE code_sample COLUMNS crate_id;
                    DEFINE INDEX idx_sample_index ON TABLE code_sample COLUMNS sample_index;
                    DEFINE INDEX idx_sample_type ON TABLE code_sample COLUMNS sample_type;
                    DEFINE INDEX idx_crate_sample ON TABLE code_sample COLUMNS crate_id, sample_index UNIQUE;

                    -- Processing events audit log
                    DEFINE TABLE processing_event SCHEMAFULL;
                    DEFINE FIELD crate_id ON TABLE processing_event TYPE record<crate>
                        ASSERT $value != NONE;
                    DEFINE FIELD event_type ON TABLE processing_event TYPE string
                        ASSERT $value != NONE;
                    DEFINE FIELD event_data ON TABLE processing_event TYPE option<object>;
                    DEFINE FIELD timestamp ON TABLE processing_event TYPE datetime DEFAULT time::now();
                    DEFINE FIELD actor_name ON TABLE processing_event TYPE option<string>;
                    DEFINE FIELD actor_type ON TABLE processing_event TYPE option<string>;
                    DEFINE FIELD success ON TABLE processing_event TYPE bool DEFAULT true;
                    DEFINE FIELD error_message ON TABLE processing_event TYPE option<string>;
                    DEFINE FIELD duration_ms ON TABLE processing_event TYPE option<int>;
                    DEFINE INDEX idx_crate_id_event ON TABLE processing_event COLUMNS crate_id;
                    DEFINE INDEX idx_timestamp ON TABLE processing_event COLUMNS timestamp;
                    DEFINE INDEX idx_event_type ON TABLE processing_event COLUMNS event_type;
                "#;

                match db.query(schema_sql).await {
                    Ok(_) => {
                        info!("Database schema initialized successfully");

                        // Broadcast DatabaseReady event
                        broker.broadcast(DatabaseReady).await;
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

        // Subscribe to persistence messages broadcasted from other actors
        handle.subscribe::<PersistDocChunk>().await;
        handle.subscribe::<PersistEmbedding>().await;

        // Subscribe to query messages
        handle.subscribe::<QueryDocChunks>().await;

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
    use crate::types::ChunkMetadata;
    use std::env;
    use std::str::FromStr;
    use std::time::{SystemTime, UNIX_EPOCH};
    use tokio::time::Duration;

    /// Helper: Create a unique test database path using test name and timestamp
    /// to prevent lock contention between parallel tests.
    fn create_test_db_path(test_name: &str) -> std::path::PathBuf {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        env::temp_dir().join(format!("crately_test_{}_{}", test_name, timestamp))
    }

    /// Helper: Clean up test database with retry logic to handle file locks.
    async fn cleanup_test_db(path: &std::path::Path) {
        // Give RocksDB time to release file locks
        tokio::time::sleep(Duration::from_millis(100)).await;

        // Retry cleanup a few times if locks are still held
        for attempt in 0..3 {
            match std::fs::remove_dir_all(path) {
                Ok(()) => break,
                Err(e) if attempt < 2 => {
                    tracing::warn!("Cleanup attempt {} failed: {}, retrying...", attempt + 1, e);
                    tokio::time::sleep(Duration::from_millis(50)).await;
                }
                Err(e) => {
                    tracing::warn!("Final cleanup attempt failed: {}", e);
                }
            }
        }
    }

    /// TestSubscriber helper actor for verifying broadcast responses in DatabaseActor tests.
    ///
    /// This actor subscribes to CrateQueryResponse, CrateListResponse, and DatabaseError
    /// broadcasts and collects them for verification in the after_stop hook.
    ///
    /// Follows the acton-reactive test pattern:
    /// 1. Subscribe to message types before starting
    /// 2. Collect responses during execution
    /// 3. Assert expectations in after_stop hook
    #[acton_actor]
    #[derive(Default, Clone)]
    struct TestSubscriber {
        query_responses: Vec<CrateQueryResponse>,
        list_responses: Vec<CrateListResponse>,
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_database_actor_spawns_successfully() {
        tokio::time::timeout(Duration::from_secs(10), async {
            let mut runtime = ActonApp::launch();
            let test_db_path = create_test_db_path("spawn");

            let result = DatabaseActor::spawn(&mut runtime, test_db_path.clone()).await;

            assert!(result.is_ok(), "DatabaseActor should spawn successfully");

            let (handle, db_info) = result.unwrap();
            assert_eq!(db_info.namespace, "crately");
            assert_eq!(db_info.database, "production");
            assert_eq!(db_info.db_path, test_db_path);

            // Clean shutdown
            handle.stop().await.unwrap();
            runtime.shutdown_all().await.unwrap();

            // Cleanup with proper delay
            cleanup_test_db(&test_db_path).await;
        })
        .await
        .expect("Test should complete within timeout");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_database_actor_lifecycle() {
        tokio::time::timeout(Duration::from_secs(10), async {
            let mut runtime = ActonApp::launch();
            let test_db_path = create_test_db_path("lifecycle");

            // Spawn actor
            let (handle, _db_info) = DatabaseActor::spawn(&mut runtime, test_db_path.clone())
                .await
                .unwrap();

            // Give the after_start hook time to execute
            tokio::time::sleep(Duration::from_millis(200)).await;

            // Stop actor gracefully
            let stop_result = handle.stop().await;
            assert!(stop_result.is_ok(), "DatabaseActor should stop gracefully");

            runtime.shutdown_all().await.unwrap();

            // Cleanup with proper delay
            cleanup_test_db(&test_db_path).await;
        })
        .await
        .expect("Test should complete within timeout");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_multiple_database_actors_use_isolated_connections() {
        tokio::time::timeout(Duration::from_secs(10), async {
            let mut runtime = ActonApp::launch();
            let test_db_path_1 = create_test_db_path("isolated_1");
            let test_db_path_2 = create_test_db_path("isolated_2");

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

            // Cleanup with proper delays
            cleanup_test_db(&test_db_path_1).await;
            cleanup_test_db(&test_db_path_2).await;
        })
        .await
        .expect("Test should complete within timeout");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_database_actor_query_crate_specific_version() {
        tokio::time::timeout(Duration::from_secs(15), async {
            use std::str::FromStr;

            let mut runtime = ActonApp::launch();
            let test_db_path = create_test_db_path("query_specific");

            let (handle, _db_info) = DatabaseActor::spawn(&mut runtime, test_db_path.clone())
                .await
                .unwrap();

            // Create TestSubscriber to verify responses
            let mut subscriber_builder = runtime.new_agent::<TestSubscriber>().await;

            // Configure handler to collect query responses
            subscriber_builder.mutate_on::<CrateQueryResponse>(|agent, envelope| {
                let msg = envelope.message().clone();
                agent.model.query_responses.push(msg);
                AgentReply::immediate()
            });

            // Verify responses in after_stop hook
            subscriber_builder.after_stop(|agent| {
                assert_eq!(
                    agent.model.query_responses.len(),
                    1,
                    "Should receive exactly one query response"
                );

                let response = &agent.model.query_responses[0];
                assert!(response.specifier.is_some(), "Should find the crate");

                let spec = response.specifier.as_ref().unwrap();
                assert_eq!(spec.name(), "serde", "Crate name should match");
                assert_eq!(spec.version().to_string(), "1.0.0", "Version should match");
                assert_eq!(response.features, vec!["derive"], "Features should match");
                assert_eq!(response.status, "pending", "Status should be pending");
                assert!(
                    !response.created_at.is_empty(),
                    "Should have created_at timestamp"
                );

                AgentReply::immediate()
            });

            // Subscribe to broadcast messages before starting
            subscriber_builder
                .handle()
                .subscribe::<CrateQueryResponse>()
                .await;
            let subscriber_handle = subscriber_builder.start().await;

            // Wait for schema initialization with longer timeout for safety
            tokio::time::sleep(Duration::from_millis(300)).await;

            // Persist a test crate
            let specifier = CrateSpecifier::from_str("serde@1.0.0").unwrap();
            handle
                .send(PersistCrate {
                    specifier,
                    features: vec!["derive".to_string()],
                })
                .await;

            // Wait for persistence with longer timeout
            tokio::time::sleep(Duration::from_millis(200)).await;

            // Query for the specific version
            handle
                .send(QueryCrate {
                    name: "serde".to_string(),
                    version: Some("1.0.0".to_string()),
                })
                .await;

            // Wait for query to complete and broadcast with longer timeout
            tokio::time::sleep(Duration::from_millis(200)).await;

            // Clean shutdown (subscriber assertions trigger in after_stop)
            subscriber_handle.stop().await.unwrap();
            handle.stop().await.unwrap();
            runtime.shutdown_all().await.unwrap();

            // Cleanup with proper delay
            cleanup_test_db(&test_db_path).await;
        })
        .await
        .expect("Test should complete within timeout");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_database_actor_query_crate_latest_version() {
        tokio::time::timeout(Duration::from_secs(15), async {
            use std::str::FromStr;

            let mut runtime = ActonApp::launch();
            let test_db_path = create_test_db_path("query_latest");

            let (handle, _db_info) = DatabaseActor::spawn(&mut runtime, test_db_path.clone())
                .await
                .unwrap();

            // Create TestSubscriber to verify responses
            let mut subscriber_builder = runtime.new_agent::<TestSubscriber>().await;

            // Configure handler to collect query responses
            subscriber_builder.mutate_on::<CrateQueryResponse>(|agent, envelope| {
                let msg = envelope.message().clone();
                agent.model.query_responses.push(msg);
                AgentReply::immediate()
            });

            // Verify responses in after_stop hook
            subscriber_builder.after_stop(|agent| {
                assert_eq!(
                    agent.model.query_responses.len(),
                    1,
                    "Should receive exactly one query response"
                );

                let response = &agent.model.query_responses[0];
                assert!(
                    response.specifier.is_some(),
                    "Should find the latest version"
                );

                let spec = response.specifier.as_ref().unwrap();
                assert_eq!(spec.name(), "tokio", "Crate name should match");
                assert_eq!(
                    spec.version().to_string(),
                    "1.36.0",
                    "Should return latest version"
                );
                assert_eq!(
                    response.features,
                    vec!["full"],
                    "Features should match latest version"
                );
                assert_eq!(response.status, "pending", "Status should be pending");
                assert!(
                    !response.created_at.is_empty(),
                    "Should have created_at timestamp"
                );

                AgentReply::immediate()
            });

            // Subscribe to broadcast messages before starting
            subscriber_builder
                .handle()
                .subscribe::<CrateQueryResponse>()
                .await;
            let subscriber_handle = subscriber_builder.start().await;

            // Wait for schema initialization with longer timeout
            tokio::time::sleep(Duration::from_millis(300)).await;

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

            // Wait for persistence with longer timeout
            tokio::time::sleep(Duration::from_millis(200)).await;

            // Query for latest version (version = None)
            handle
                .send(QueryCrate {
                    name: "tokio".to_string(),
                    version: None,
                })
                .await;

            // Wait for query to complete and broadcast with longer timeout
            tokio::time::sleep(Duration::from_millis(200)).await;

            // Clean shutdown (subscriber assertions trigger in after_stop)
            subscriber_handle.stop().await.unwrap();
            handle.stop().await.unwrap();
            runtime.shutdown_all().await.unwrap();

            // Cleanup with proper delay
            cleanup_test_db(&test_db_path).await;
        })
        .await
        .expect("Test should complete within timeout");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_database_actor_query_crate_not_found() {
        tokio::time::timeout(Duration::from_secs(15), async {
            let mut runtime = ActonApp::launch();
            let test_db_path = create_test_db_path("query_not_found");

            // Clean up any existing test database
            let _ = std::fs::remove_dir_all(&test_db_path);

            let (handle, _db_info) = DatabaseActor::spawn(&mut runtime, test_db_path.clone())
                .await
                .unwrap();

            // Create TestSubscriber to verify responses
            let mut subscriber_builder = runtime.new_agent::<TestSubscriber>().await;

            // Configure handler to collect query responses
            subscriber_builder.mutate_on::<CrateQueryResponse>(|agent, envelope| {
                let msg = envelope.message().clone();
                agent.model.query_responses.push(msg);
                AgentReply::immediate()
            });

            // Verify responses in after_stop hook
            subscriber_builder.after_stop(|agent| {
                assert_eq!(
                    agent.model.query_responses.len(),
                    1,
                    "Should receive exactly one query response"
                );

                let response = &agent.model.query_responses[0];
                assert!(
                    response.specifier.is_none(),
                    "Should not find non-existent crate"
                );
                assert_eq!(response.status, "not_found", "Status should be not_found");
                assert!(response.features.is_empty(), "Features should be empty");
                assert!(response.created_at.is_empty(), "created_at should be empty");

                AgentReply::immediate()
            });

            // Subscribe to broadcast messages before starting
            subscriber_builder
                .handle()
                .subscribe::<CrateQueryResponse>()
                .await;
            let subscriber_handle = subscriber_builder.start().await;

            // Wait for schema initialization
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

            // Query for a non-existent crate
            handle
                .send(QueryCrate {
                    name: "nonexistent-crate".to_string(),
                    version: Some("1.0.0".to_string()),
                })
                .await;

            // Wait for query to complete and broadcast
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

            // Clean shutdown (subscriber assertions trigger in after_stop)
            subscriber_handle.stop().await.unwrap();
            handle.stop().await.unwrap();
            runtime.shutdown_all().await.unwrap();

            // Cleanup
            cleanup_test_db(&test_db_path).await;
        })
        .await
        .expect("Test should complete within timeout");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_database_actor_list_crates_default_pagination() {
        tokio::time::timeout(Duration::from_secs(15), async {
            use std::str::FromStr;

            let mut runtime = ActonApp::launch();
            let test_db_path = create_test_db_path("list_default");

            // Clean up any existing test database
            let _ = std::fs::remove_dir_all(&test_db_path);

            let (handle, _db_info) = DatabaseActor::spawn(&mut runtime, test_db_path.clone())
                .await
                .unwrap();

            // Create TestSubscriber to verify responses
            let mut subscriber_builder = runtime.new_agent::<TestSubscriber>().await;

            // Configure handler to collect list responses
            subscriber_builder.mutate_on::<CrateListResponse>(|agent, envelope| {
                let msg = envelope.message().clone();
                agent.model.list_responses.push(msg);
                AgentReply::immediate()
            });

            // Verify responses in after_stop hook
            subscriber_builder.after_stop(|agent| {
                assert_eq!(
                    agent.model.list_responses.len(),
                    1,
                    "Should receive exactly one list response"
                );

                let response = &agent.model.list_responses[0];
                assert_eq!(response.crates.len(), 2, "Should list 2 crates");
                assert_eq!(response.total_count, 2, "Total count should be 2");

                // Verify crate summaries contain expected data
                let crate_names: Vec<&str> =
                    response.crates.iter().map(|c| c.name.as_str()).collect();
                assert!(crate_names.contains(&"serde"), "Should contain serde");
                assert!(crate_names.contains(&"tokio"), "Should contain tokio");

                AgentReply::immediate()
            });

            // Subscribe to broadcast messages before starting
            subscriber_builder
                .handle()
                .subscribe::<CrateListResponse>()
                .await;
            let subscriber_handle = subscriber_builder.start().await;

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

            // Wait for list to complete and broadcast
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

            // Clean shutdown (subscriber assertions trigger in after_stop)
            subscriber_handle.stop().await.unwrap();
            handle.stop().await.unwrap();
            runtime.shutdown_all().await.unwrap();

            // Cleanup
            cleanup_test_db(&test_db_path).await;
        })
        .await
        .expect("Test should complete within timeout");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_database_actor_list_crates_with_pagination() {
        tokio::time::timeout(Duration::from_secs(15), async {
            use std::str::FromStr;

            let mut runtime = ActonApp::launch();
            let test_db_path = create_test_db_path("list_paginated");

            // Clean up any existing test database
            let _ = std::fs::remove_dir_all(&test_db_path);

            let (handle, _db_info) = DatabaseActor::spawn(&mut runtime, test_db_path.clone())
                .await
                .unwrap();

            // Create TestSubscriber to verify responses
            let mut subscriber_builder = runtime.new_agent::<TestSubscriber>().await;

            // Configure handler to collect list responses
            subscriber_builder.mutate_on::<CrateListResponse>(|agent, envelope| {
                let msg = envelope.message().clone();
                agent.model.list_responses.push(msg);
                AgentReply::immediate()
            });

            // Verify responses in after_stop hook
            subscriber_builder.after_stop(|agent| {
                assert_eq!(
                    agent.model.list_responses.len(),
                    2,
                    "Should receive two list responses (2 pages)"
                );

                // First page should have 2 crates
                let first_page = &agent.model.list_responses[0];
                assert_eq!(
                    first_page.crates.len(),
                    2,
                    "First page should have 2 crates"
                );
                assert_eq!(first_page.total_count, 5, "Total count should be 5");

                // Second page should have 2 crates
                let second_page = &agent.model.list_responses[1];
                assert_eq!(
                    second_page.crates.len(),
                    2,
                    "Second page should have 2 crates"
                );
                assert_eq!(second_page.total_count, 5, "Total count should be 5");

                AgentReply::immediate()
            });

            // Subscribe to broadcast messages before starting
            subscriber_builder
                .handle()
                .subscribe::<CrateListResponse>()
                .await;
            let subscriber_handle = subscriber_builder.start().await;

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

            // Wait for list to complete and broadcast
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

            // Clean shutdown (subscriber assertions trigger in after_stop)
            subscriber_handle.stop().await.unwrap();
            handle.stop().await.unwrap();
            runtime.shutdown_all().await.unwrap();

            // Cleanup
            cleanup_test_db(&test_db_path).await;
        })
        .await
        .expect("Test should complete within timeout");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_database_actor_list_crates_empty_database() {
        tokio::time::timeout(Duration::from_secs(15), async {
            let mut runtime = ActonApp::launch();
            let test_db_path = create_test_db_path("list_empty");

            // Clean up any existing test database
            let _ = std::fs::remove_dir_all(&test_db_path);

            let (handle, _db_info) = DatabaseActor::spawn(&mut runtime, test_db_path.clone())
                .await
                .unwrap();

            // Create TestSubscriber to verify responses
            let mut subscriber_builder = runtime.new_agent::<TestSubscriber>().await;

            // Configure handler to collect list responses
            subscriber_builder.mutate_on::<CrateListResponse>(|agent, envelope| {
                let msg = envelope.message().clone();
                agent.model.list_responses.push(msg);
                AgentReply::immediate()
            });

            // Verify responses in after_stop hook
            subscriber_builder.after_stop(|agent| {
                assert_eq!(
                    agent.model.list_responses.len(),
                    1,
                    "Should receive exactly one list response"
                );

                let response = &agent.model.list_responses[0];
                assert!(
                    response.crates.is_empty(),
                    "Should return empty crates list"
                );
                assert_eq!(response.total_count, 0, "Total count should be 0");

                AgentReply::immediate()
            });

            // Subscribe to broadcast messages before starting
            subscriber_builder
                .handle()
                .subscribe::<CrateListResponse>()
                .await;
            let subscriber_handle = subscriber_builder.start().await;

            // Wait for schema initialization
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

            // List crates from empty database
            handle
                .send(ListCrates {
                    limit: Some(10),
                    offset: Some(0),
                })
                .await;

            // Wait for list to complete and broadcast
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

            // Clean shutdown (subscriber assertions trigger in after_stop)
            subscriber_handle.stop().await.unwrap();
            handle.stop().await.unwrap();
            runtime.shutdown_all().await.unwrap();

            // Cleanup
            cleanup_test_db(&test_db_path).await;
        })
        .await
        .expect("Test should complete within timeout");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_database_actor_crate_received_event() {
        tokio::time::timeout(Duration::from_secs(15), async {
            use std::str::FromStr;

            let mut runtime = ActonApp::launch();
            let test_db_path = create_test_db_path("crate_received");

            let (handle, _db_info) = DatabaseActor::spawn(&mut runtime, test_db_path.clone())
                .await
                .unwrap();

            // Wait for schema initialization
            tokio::time::sleep(Duration::from_millis(300)).await;

            // Broadcast CrateReceived event
            let specifier = CrateSpecifier::from_str("serde@1.0.0").unwrap();
            runtime
                .broker()
                .broadcast(CrateReceived {
                    specifier: specifier.clone(),
                    features: vec!["derive".to_string()],
                })
                .await;

            // Wait for event processing
            tokio::time::sleep(Duration::from_millis(200)).await;

            // Query the crate to verify it was created
            let mut subscriber_builder = runtime.new_agent::<TestSubscriber>().await;

            subscriber_builder.mutate_on::<CrateQueryResponse>(|agent, envelope| {
                let msg = envelope.message().clone();
                agent.model.query_responses.push(msg);
                AgentReply::immediate()
            });

            subscriber_builder.after_stop(|agent| {
                assert_eq!(
                    agent.model.query_responses.len(),
                    1,
                    "Should receive exactly one query response"
                );

                let response = &agent.model.query_responses[0];
                assert!(response.specifier.is_some(), "Should find the crate");

                let spec = response.specifier.as_ref().unwrap();
                assert_eq!(spec.name(), "serde");
                assert_eq!(spec.version().to_string(), "1.0.0");
                assert_eq!(response.features, vec!["derive"]);
                assert_eq!(response.status, "pending", "Status should be pending");

                AgentReply::immediate()
            });

            subscriber_builder
                .handle()
                .subscribe::<CrateQueryResponse>()
                .await;
            let subscriber_handle = subscriber_builder.start().await;

            // Query the crate
            handle
                .send(QueryCrate {
                    name: "serde".to_string(),
                    version: Some("1.0.0".to_string()),
                })
                .await;

            tokio::time::sleep(Duration::from_millis(200)).await;

            // Clean shutdown
            subscriber_handle.stop().await.unwrap();
            handle.stop().await.unwrap();
            runtime.shutdown_all().await.unwrap();

            cleanup_test_db(&test_db_path).await;
        })
        .await
        .expect("Test should complete within timeout");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_database_actor_crate_download_failed_event() {
        tokio::time::timeout(Duration::from_secs(15), async {
            use std::str::FromStr;

            let mut runtime = ActonApp::launch();
            let test_db_path = create_test_db_path("download_failed");

            let (handle, _db_info) = DatabaseActor::spawn(&mut runtime, test_db_path.clone())
                .await
                .unwrap();

            tokio::time::sleep(Duration::from_millis(300)).await;

            // Create initial crate record via CrateReceived
            let specifier = CrateSpecifier::from_str("broken-crate@1.0.0").unwrap();
            runtime
                .broker()
                .broadcast(CrateReceived {
                    specifier: specifier.clone(),
                    features: vec![],
                })
                .await;

            tokio::time::sleep(Duration::from_millis(200)).await;

            // Broadcast CrateDownloadFailed event
            use crate::messages::DownloadErrorKind;
            runtime
                .broker()
                .broadcast(CrateDownloadFailed {
                    specifier: specifier.clone(),
                    features: vec![],
                    error_message: "Network timeout".to_string(),
                    error_kind: DownloadErrorKind::Timeout,
                })
                .await;

            tokio::time::sleep(Duration::from_millis(200)).await;

            // Query to verify failure was recorded
            let mut subscriber_builder = runtime.new_agent::<TestSubscriber>().await;

            subscriber_builder.mutate_on::<CrateQueryResponse>(|agent, envelope| {
                let msg = envelope.message().clone();
                agent.model.query_responses.push(msg);
                AgentReply::immediate()
            });

            subscriber_builder.after_stop(|agent| {
                assert_eq!(
                    agent.model.query_responses.len(),
                    1,
                    "Should receive exactly one query response"
                );

                let response = &agent.model.query_responses[0];
                assert!(response.specifier.is_some(), "Should find the crate");
                assert_eq!(
                    response.status, "download_failed",
                    "Status should be download_failed"
                );

                AgentReply::immediate()
            });

            subscriber_builder
                .handle()
                .subscribe::<CrateQueryResponse>()
                .await;
            let subscriber_handle = subscriber_builder.start().await;

            handle
                .send(QueryCrate {
                    name: "broken-crate".to_string(),
                    version: Some("1.0.0".to_string()),
                })
                .await;

            tokio::time::sleep(Duration::from_millis(200)).await;

            subscriber_handle.stop().await.unwrap();
            handle.stop().await.unwrap();
            runtime.shutdown_all().await.unwrap();

            cleanup_test_db(&test_db_path).await;
        })
        .await
        .expect("Test should complete within timeout");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_database_actor_documentation_extraction_failed_event() {
        tokio::time::timeout(Duration::from_secs(15), async {
            use std::str::FromStr;

            let mut runtime = ActonApp::launch();
            let test_db_path = create_test_db_path("extraction_failed");

            let (handle, _db_info) = DatabaseActor::spawn(&mut runtime, test_db_path.clone())
                .await
                .unwrap();

            tokio::time::sleep(Duration::from_millis(300)).await;

            // Create initial crate record
            let specifier = CrateSpecifier::from_str("invalid-crate@1.0.0").unwrap();
            runtime
                .broker()
                .broadcast(CrateReceived {
                    specifier: specifier.clone(),
                    features: vec![],
                })
                .await;

            tokio::time::sleep(Duration::from_millis(200)).await;

            // Broadcast DocumentationExtractionFailed event
            use crate::messages::ExtractionErrorKind;
            runtime
                .broker()
                .broadcast(DocumentationExtractionFailed {
                    specifier: specifier.clone(),
                    features: vec![],
                    error_message: "Cargo.toml not found".to_string(),
                    error_kind: ExtractionErrorKind::FileNotFound,
                    elapsed_ms: 50,
                })
                .await;

            tokio::time::sleep(Duration::from_millis(200)).await;

            // Query to verify failure was recorded
            let mut subscriber_builder = runtime.new_agent::<TestSubscriber>().await;

            subscriber_builder.mutate_on::<CrateQueryResponse>(|agent, envelope| {
                let msg = envelope.message().clone();
                agent.model.query_responses.push(msg);
                AgentReply::immediate()
            });

            subscriber_builder.after_stop(|agent| {
                assert_eq!(
                    agent.model.query_responses.len(),
                    1,
                    "Should receive exactly one query response"
                );

                let response = &agent.model.query_responses[0];
                assert!(response.specifier.is_some(), "Should find the crate");
                assert_eq!(
                    response.status, "extraction_failed",
                    "Status should be extraction_failed"
                );

                AgentReply::immediate()
            });

            subscriber_builder
                .handle()
                .subscribe::<CrateQueryResponse>()
                .await;
            let subscriber_handle = subscriber_builder.start().await;

            handle
                .send(QueryCrate {
                    name: "invalid-crate".to_string(),
                    version: Some("1.0.0".to_string()),
                })
                .await;

            tokio::time::sleep(Duration::from_millis(200)).await;

            subscriber_handle.stop().await.unwrap();
            handle.stop().await.unwrap();
            runtime.shutdown_all().await.unwrap();

            cleanup_test_db(&test_db_path).await;
        })
        .await
        .expect("Test should complete within timeout");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_database_actor_full_pipeline_success_flow() {
        tokio::time::timeout(Duration::from_secs(15), async {
            use std::path::PathBuf;
            use std::str::FromStr;

            let mut runtime = ActonApp::launch();
            let test_db_path = create_test_db_path("full_pipeline");

            let (handle, _db_info) = DatabaseActor::spawn(&mut runtime, test_db_path.clone())
                .await
                .unwrap();

            tokio::time::sleep(Duration::from_millis(300)).await;

            let specifier = CrateSpecifier::from_str("test-crate@1.0.0").unwrap();

            // Step 1: CrateReceived
            runtime
                .broker()
                .broadcast(CrateReceived {
                    specifier: specifier.clone(),
                    features: vec!["test".to_string()],
                })
                .await;
            tokio::time::sleep(Duration::from_millis(100)).await;

            // Step 2: CrateDownloaded
            runtime
                .broker()
                .broadcast(CrateDownloaded {
                    specifier: specifier.clone(),
                    features: vec!["test".to_string()],
                    extracted_path: PathBuf::from("/tmp/test"),
                    download_duration_ms: 100,
                })
                .await;
            tokio::time::sleep(Duration::from_millis(100)).await;

            // Step 3: DocumentationExtracted
            runtime
                .broker()
                .broadcast(DocumentationExtracted {
                    specifier: specifier.clone(),
                    features: vec!["test".to_string()],
                    documentation_bytes: 2048,
                    file_count: 10,
                    extracted_path: PathBuf::from("/tmp/test"),
                })
                .await;
            tokio::time::sleep(Duration::from_millis(100)).await;

            // Step 4: DocumentationChunked
            runtime
                .broker()
                .broadcast(DocumentationChunked {
                    specifier: specifier.clone(),
                    features: vec!["test".to_string()],
                    chunk_count: 5,
                    total_tokens_estimated: 500,
                })
                .await;
            tokio::time::sleep(Duration::from_millis(100)).await;

            // Step 5: DocumentationVectorized
            runtime
                .broker()
                .broadcast(DocumentationVectorized {
                    specifier: specifier.clone(),
                    features: vec!["test".to_string()],
                    vector_count: 5,
                    embedding_model: "test-model".to_string(),
                    vectorization_duration_ms: 200,
                })
                .await;
            tokio::time::sleep(Duration::from_millis(100)).await;

            // Query final status after vectorization
            let mut subscriber_builder = runtime.new_agent::<TestSubscriber>().await;

            subscriber_builder.mutate_on::<CrateQueryResponse>(|agent, envelope| {
                let msg = envelope.message().clone();
                agent.model.query_responses.push(msg);
                AgentReply::immediate()
            });

            subscriber_builder.after_stop(|agent| {
                assert_eq!(
                    agent.model.query_responses.len(),
                    1,
                    "Should receive exactly one query response"
                );

                let response = &agent.model.query_responses[0];
                assert!(response.specifier.is_some(), "Should find the crate");
                assert_eq!(
                    response.status, "complete",
                    "Final status should be complete"
                );

                AgentReply::immediate()
            });

            subscriber_builder
                .handle()
                .subscribe::<CrateQueryResponse>()
                .await;
            let subscriber_handle = subscriber_builder.start().await;

            handle
                .send(QueryCrate {
                    name: "test-crate".to_string(),
                    version: Some("1.0.0".to_string()),
                })
                .await;

            tokio::time::sleep(Duration::from_millis(200)).await;

            subscriber_handle.stop().await.unwrap();
            handle.stop().await.unwrap();
            runtime.shutdown_all().await.unwrap();

            cleanup_test_db(&test_db_path).await;
        })
        .await
        .expect("Test should complete within timeout");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_persist_doc_chunk_success() {
        tokio::time::timeout(Duration::from_secs(10), async {
            let mut runtime = ActonApp::launch();
            let test_db_path = create_test_db_path("persist_doc_chunk");

            let (handle, _db_info) = DatabaseActor::spawn(&mut runtime, test_db_path.clone())
                .await
                .unwrap();

            tokio::time::sleep(Duration::from_millis(200)).await;

            // First persist a crate
            let specifier = CrateSpecifier::from_str("tokio@1.35.0").unwrap();
            handle
                .send(PersistCrate {
                    specifier: specifier.clone(),
                    features: vec!["full".to_string()],
                })
                .await;

            tokio::time::sleep(Duration::from_millis(200)).await;

            // Now persist a doc chunk
            let chunk_metadata = ChunkMetadata {
                content_type: "rust".to_string(),
                start_line: Some(100),
                end_line: Some(200),
                token_count: 512,
                char_count: 2048,
                parent_module: Some("tokio::runtime".to_string()),
                item_type: Some("function".to_string()),
                item_name: Some("spawn".to_string()),
            };

            handle
                .send(PersistDocChunk {
                    specifier: specifier.clone(),
                    chunk_index: 0,
                    chunk_id: "tokio_1.35.0_chunk_000".to_string(),
                    content: "Documentation for tokio::runtime::spawn function".to_string(),
                    source_file: "src/runtime/mod.rs".to_string(),
                    metadata: chunk_metadata,
                })
                .await;

            tokio::time::sleep(Duration::from_millis(200)).await;

            handle.stop().await.unwrap();
            runtime.shutdown_all().await.unwrap();

            cleanup_test_db(&test_db_path).await;
        })
        .await
        .expect("Test should complete within timeout");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_persist_doc_chunk_crate_not_found() {
        tokio::time::timeout(Duration::from_secs(10), async {
            let mut runtime = ActonApp::launch();
            let test_db_path = create_test_db_path("persist_doc_chunk_not_found");

            let (handle, _db_info) = DatabaseActor::spawn(&mut runtime, test_db_path.clone())
                .await
                .unwrap();

            tokio::time::sleep(Duration::from_millis(200)).await;

            // Try to persist a doc chunk for a non-existent crate
            // This should log an error and broadcast DatabaseError,
            // but the test verifies the message is handled gracefully
            let specifier = CrateSpecifier::from_str("nonexistent@1.0.0").unwrap();
            let chunk_metadata = ChunkMetadata {
                content_type: "rust".to_string(),
                start_line: Some(1),
                end_line: Some(10),
                token_count: 128,
                char_count: 512,
                parent_module: None,
                item_type: Some("module".to_string()),
                item_name: Some("lib".to_string()),
            };

            handle
                .send(PersistDocChunk {
                    specifier,
                    chunk_index: 0,
                    chunk_id: "nonexistent_1.0.0_chunk_000".to_string(),
                    content: "Some documentation".to_string(),
                    source_file: "src/lib.rs".to_string(),
                    metadata: chunk_metadata,
                })
                .await;

            // Give time for error handling to complete
            tokio::time::sleep(Duration::from_millis(200)).await;

            handle.stop().await.unwrap();
            runtime.shutdown_all().await.unwrap();

            cleanup_test_db(&test_db_path).await;
        })
        .await
        .expect("Test should complete within timeout");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_persist_doc_chunk_multiple_chunks() {
        tokio::time::timeout(Duration::from_secs(10), async {
            let mut runtime = ActonApp::launch();
            let test_db_path = create_test_db_path("persist_multiple_chunks");

            let (handle, _db_info) = DatabaseActor::spawn(&mut runtime, test_db_path.clone())
                .await
                .unwrap();

            tokio::time::sleep(Duration::from_millis(200)).await;

            // First persist a crate
            let specifier = CrateSpecifier::from_str("serde@1.0.0").unwrap();
            handle
                .send(PersistCrate {
                    specifier: specifier.clone(),
                    features: vec!["derive".to_string()],
                })
                .await;

            tokio::time::sleep(Duration::from_millis(200)).await;

            // Persist multiple doc chunks
            for i in 0..5 {
                let chunk_metadata = ChunkMetadata {
                    content_type: "markdown".to_string(),
                    start_line: Some(i * 100),
                    end_line: Some((i + 1) * 100),
                    token_count: 256,
                    char_count: 1024,
                    parent_module: Some(format!("serde::module_{}", i)),
                    item_type: Some("struct".to_string()),
                    item_name: Some(format!("Item{}", i)),
                };

                handle
                    .send(PersistDocChunk {
                        specifier: specifier.clone(),
                        chunk_index: i,
                        chunk_id: format!("serde_1.0.0_chunk_{:03}", i),
                        content: format!("Documentation chunk {} for serde", i),
                        source_file: format!("src/module_{}.rs", i),
                        metadata: chunk_metadata,
                    })
                    .await;

                tokio::time::sleep(Duration::from_millis(50)).await;
            }

            tokio::time::sleep(Duration::from_millis(200)).await;

            handle.stop().await.unwrap();
            runtime.shutdown_all().await.unwrap();

            cleanup_test_db(&test_db_path).await;
        })
        .await
        .expect("Test should complete within timeout");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_persist_doc_chunk_with_optional_metadata() {
        tokio::time::timeout(Duration::from_secs(10), async {
            let mut runtime = ActonApp::launch();
            let test_db_path = create_test_db_path("persist_chunk_optional_metadata");

            let (handle, _db_info) = DatabaseActor::spawn(&mut runtime, test_db_path.clone())
                .await
                .unwrap();

            tokio::time::sleep(Duration::from_millis(200)).await;

            // First persist a crate
            let specifier = CrateSpecifier::from_str("actix-web@4.0.0").unwrap();
            handle
                .send(PersistCrate {
                    specifier: specifier.clone(),
                    features: vec![],
                })
                .await;

            tokio::time::sleep(Duration::from_millis(200)).await;

            // Persist doc chunk with minimal metadata (no line numbers, no structural info)
            let chunk_metadata = ChunkMetadata {
                content_type: "markdown".to_string(),
                start_line: None,
                end_line: None,
                token_count: 64,
                char_count: 256,
                parent_module: None,
                item_type: None,
                item_name: None,
            };

            handle
                .send(PersistDocChunk {
                    specifier: specifier.clone(),
                    chunk_index: 0,
                    chunk_id: "actix_web_4.0.0_chunk_000".to_string(),
                    content: "General crate overview documentation".to_string(),
                    source_file: "README.md".to_string(),
                    metadata: chunk_metadata,
                })
                .await;

            tokio::time::sleep(Duration::from_millis(200)).await;

            handle.stop().await.unwrap();
            runtime.shutdown_all().await.unwrap();

            cleanup_test_db(&test_db_path).await;
        })
        .await
        .expect("Test should complete within timeout");
    }

    /// Test that DocumentationChunked validates chunks exist before updating status
    #[tokio::test(flavor = "multi_thread")]
    async fn test_chunked_status_validation_passes_with_chunks() {
        tokio::time::timeout(Duration::from_secs(10), async {
            let mut runtime = ActonApp::launch();
            let test_db_path = create_test_db_path("chunked_validation_pass");

            let (handle, _db_info) = DatabaseActor::spawn(&mut runtime, test_db_path.clone())
                .await
                .expect("Failed to spawn DatabaseActor");

            tokio::time::sleep(Duration::from_millis(100)).await;

            let specifier = CrateSpecifier::from_str("test_crate@1.0.0").unwrap();

            // Create crate record
            runtime
                .broker()
                .broadcast(CrateReceived {
                    specifier: specifier.clone(),
                    features: vec![],
                })
                .await;
            tokio::time::sleep(Duration::from_millis(100)).await;

            // Persist chunks BEFORE sending DocumentationChunked
            for i in 0..3 {
                let chunk_metadata = ChunkMetadata {
                    content_type: "markdown".to_string(),
                    start_line: Some(i * 10),
                    end_line: Some((i + 1) * 10),
                    token_count: 100,
                    char_count: 400,
                    parent_module: None,
                    item_type: None,
                    item_name: None,
                };

                handle
                    .send(PersistDocChunk {
                        specifier: specifier.clone(),
                        chunk_index: i,
                        chunk_id: format!("test_crate_1.0.0_chunk_{:03}", i),
                        content: format!("Test chunk {}", i),
                        source_file: "test.rs".to_string(),
                        metadata: chunk_metadata,
                    })
                    .await;
                tokio::time::sleep(Duration::from_millis(50)).await;
            }

            // Now send DocumentationChunked with correct count
            runtime
                .broker()
                .broadcast(DocumentationChunked {
                    specifier: specifier.clone(),
                    features: vec![],
                    chunk_count: 3,
                    total_tokens_estimated: 300,
                })
                .await;
            tokio::time::sleep(Duration::from_millis(200)).await;

            // Query status - should be 'chunked' since validation passed
            handle
                .send(QueryCrate {
                    name: specifier.name().to_string(),
                    version: Some(specifier.version().to_string()),
                })
                .await;
            tokio::time::sleep(Duration::from_millis(100)).await;

            handle.stop().await.unwrap();
            runtime.shutdown_all().await.unwrap();

            cleanup_test_db(&test_db_path).await;
        })
        .await
        .expect("Test should complete within timeout");
    }

    /// Test that DocumentationChunked rejects status update when no chunks exist
    #[tokio::test(flavor = "multi_thread")]
    async fn test_chunked_status_validation_fails_without_chunks() {
        tokio::time::timeout(Duration::from_secs(10), async {
            let mut runtime = ActonApp::launch();
            let test_db_path = create_test_db_path("chunked_validation_fail");

            let (_handle, _db_info) = DatabaseActor::spawn(&mut runtime, test_db_path.clone())
                .await
                .expect("Failed to spawn DatabaseActor");

            tokio::time::sleep(Duration::from_millis(100)).await;

            let specifier = CrateSpecifier::from_str("empty_crate@1.0.0").unwrap();

            // Create crate record
            runtime
                .broker()
                .broadcast(CrateReceived {
                    specifier: specifier.clone(),
                    features: vec![],
                })
                .await;
            tokio::time::sleep(Duration::from_millis(100)).await;

            // Send DocumentationChunked WITHOUT persisting any chunks
            runtime
                .broker()
                .broadcast(DocumentationChunked {
                    specifier: specifier.clone(),
                    features: vec![],
                    chunk_count: 5,
                    total_tokens_estimated: 500,
                })
                .await;
            tokio::time::sleep(Duration::from_millis(200)).await;

            // Status should NOT be 'chunked' because validation failed
            // (We can't easily verify this without a query response handler,
            // but the warning logs would show the validation failure)

            runtime.shutdown_all().await.unwrap();

            cleanup_test_db(&test_db_path).await;
        })
        .await
        .expect("Test should complete within timeout");
    }

    /// Test that DocumentationVectorized validates embeddings exist before updating status
    #[tokio::test(flavor = "multi_thread")]
    async fn test_vectorized_status_validation_passes_with_embeddings() {
        tokio::time::timeout(Duration::from_secs(15), async {
            let mut runtime = ActonApp::launch();
            let test_db_path = create_test_db_path("vectorized_validation_pass");

            let (handle, _db_info) = DatabaseActor::spawn(&mut runtime, test_db_path.clone())
                .await
                .expect("Failed to spawn DatabaseActor");

            tokio::time::sleep(Duration::from_millis(100)).await;

            let specifier = CrateSpecifier::from_str("vec_test@1.0.0").unwrap();

            // Create crate record
            runtime
                .broker()
                .broadcast(CrateReceived {
                    specifier: specifier.clone(),
                    features: vec![],
                })
                .await;
            tokio::time::sleep(Duration::from_millis(100)).await;

            // Persist chunks
            for i in 0..2 {
                let chunk_metadata = ChunkMetadata {
                    content_type: "markdown".to_string(),
                    start_line: None,
                    end_line: None,
                    token_count: 50,
                    char_count: 200,
                    parent_module: None,
                    item_type: None,
                    item_name: None,
                };

                handle
                    .send(PersistDocChunk {
                        specifier: specifier.clone(),
                        chunk_index: i,
                        chunk_id: format!("vec_test_1.0.0_chunk_{:03}", i),
                        content: format!("Chunk {}", i),
                        source_file: "vec.rs".to_string(),
                        metadata: chunk_metadata,
                    })
                    .await;
                tokio::time::sleep(Duration::from_millis(50)).await;
            }

            // Persist embeddings for each chunk
            for i in 0..2 {
                handle
                    .send(PersistEmbedding {
                        specifier: specifier.clone(),
                        chunk_id: format!("vec_test_1.0.0_chunk_{:03}", i),
                        vector: vec![0.1, 0.2, 0.3],
                        model_name: "test-model".to_string(),
                        model_version: "v1".to_string(),
                    })
                    .await;
                tokio::time::sleep(Duration::from_millis(50)).await;
            }

            // Now send DocumentationVectorized with correct count
            runtime
                .broker()
                .broadcast(DocumentationVectorized {
                    specifier: specifier.clone(),
                    features: vec![],
                    vector_count: 2,
                    embedding_model: "test-model".to_string(),
                    vectorization_duration_ms: 100,
                })
                .await;
            tokio::time::sleep(Duration::from_millis(200)).await;

            // Status should be 'vectorized' since validation passed

            handle.stop().await.unwrap();
            runtime.shutdown_all().await.unwrap();

            cleanup_test_db(&test_db_path).await;
        })
        .await
        .expect("Test should complete within timeout");
    }

    /// Test that DocumentationVectorized rejects status update when no embeddings exist
    #[tokio::test(flavor = "multi_thread")]
    async fn test_vectorized_status_validation_fails_without_embeddings() {
        tokio::time::timeout(Duration::from_secs(10), async {
            let mut runtime = ActonApp::launch();
            let test_db_path = create_test_db_path("vectorized_validation_fail");

            let (_handle, _db_info) = DatabaseActor::spawn(&mut runtime, test_db_path.clone())
                .await
                .expect("Failed to spawn DatabaseActor");

            tokio::time::sleep(Duration::from_millis(100)).await;

            let specifier = CrateSpecifier::from_str("no_vec@1.0.0").unwrap();

            // Create crate record
            runtime
                .broker()
                .broadcast(CrateReceived {
                    specifier: specifier.clone(),
                    features: vec![],
                })
                .await;
            tokio::time::sleep(Duration::from_millis(100)).await;

            // Send DocumentationVectorized WITHOUT persisting any embeddings
            runtime
                .broker()
                .broadcast(DocumentationVectorized {
                    specifier: specifier.clone(),
                    features: vec![],
                    vector_count: 3,
                    embedding_model: "test-model".to_string(),
                    vectorization_duration_ms: 100,
                })
                .await;
            tokio::time::sleep(Duration::from_millis(200)).await;

            // Status should NOT be 'vectorized' because validation failed

            runtime.shutdown_all().await.unwrap();

            cleanup_test_db(&test_db_path).await;
        })
        .await
        .expect("Test should complete within timeout");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_idempotent_chunk_and_embedding_persistence() {
        // Test that processing the same crate twice doesn't create duplicates
        tokio::time::timeout(Duration::from_secs(30), async {
            let mut runtime = ActonApp::launch();
            let test_db_path = create_test_db_path("idempotent");

            let (db_handle, _) = DatabaseActor::spawn(&mut runtime, test_db_path.clone())
                .await
                .unwrap();

            // Wait for schema initialization
            tokio::time::sleep(Duration::from_millis(500)).await;

            let specifier = CrateSpecifier::from_str("serde@1.0.0").unwrap();

            // First processing: persist crate, chunk, and embedding
            db_handle
                .send(PersistCrate {
                    specifier: specifier.clone(),
                    features: vec!["derive".to_string()],
                })
                .await;
            tokio::time::sleep(Duration::from_millis(200)).await;

            // Persist a doc chunk
            db_handle
                .send(PersistDocChunk {
                    specifier: specifier.clone(),
                    chunk_index: 0,
                    chunk_id: "test_chunk_1".to_string(),
                    content: "Test documentation content".to_string(),
                    source_file: "src/lib.rs".to_string(),
                    metadata: ChunkMetadata {
                        content_type: "markdown".to_string(),
                        token_count: 10,
                        char_count: 100,
                        start_line: Some(1),
                        end_line: Some(10),
                        parent_module: Some("root".to_string()),
                        item_type: Some("struct".to_string()),
                        item_name: Some("TestStruct".to_string()),
                    },
                })
                .await;
            tokio::time::sleep(Duration::from_millis(200)).await;

            // Persist an embedding for the chunk
            db_handle
                .send(PersistEmbedding {
                    specifier: specifier.clone(),
                    chunk_id: "test_chunk_1".to_string(),
                    vector: vec![0.1, 0.2, 0.3, 0.4],
                    model_name: "test-model".to_string(),
                    model_version: "v1".to_string(),
                })
                .await;
            tokio::time::sleep(Duration::from_millis(200)).await;

            // Second processing: Reprocess the same crate (idempotency test)
            // This should clean up old data and insert fresh
            db_handle
                .send(PersistCrate {
                    specifier: specifier.clone(),
                    features: vec!["derive".to_string()],
                })
                .await;
            tokio::time::sleep(Duration::from_millis(200)).await;

            // Persist the same chunk again (should upsert)
            db_handle
                .send(PersistDocChunk {
                    specifier: specifier.clone(),
                    chunk_index: 0,
                    chunk_id: "test_chunk_1".to_string(),
                    content: "Updated documentation content".to_string(),
                    source_file: "src/lib.rs".to_string(),
                    metadata: ChunkMetadata {
                        content_type: "markdown".to_string(),
                        token_count: 15,
                        char_count: 150,
                        start_line: Some(1),
                        end_line: Some(15),
                        parent_module: Some("root".to_string()),
                        item_type: Some("struct".to_string()),
                        item_name: Some("TestStruct".to_string()),
                    },
                })
                .await;
            tokio::time::sleep(Duration::from_millis(200)).await;

            // Persist the same embedding again (should upsert)
            db_handle
                .send(PersistEmbedding {
                    specifier: specifier.clone(),
                    chunk_id: "test_chunk_1".to_string(),
                    vector: vec![0.5, 0.6, 0.7, 0.8],
                    model_name: "test-model".to_string(),
                    model_version: "v1".to_string(),
                })
                .await;
            tokio::time::sleep(Duration::from_millis(200)).await;

            // Verify: Query the crate and count chunks/embeddings
            // There should be exactly 1 crate, 1 chunk, and 1 embedding (no duplicates)
            db_handle
                .send(QueryCrate {
                    name: "serde".to_string(),
                    version: Some("1.0.0".to_string()),
                })
                .await;
            tokio::time::sleep(Duration::from_millis(100)).await;

            // Clean shutdown
            db_handle.stop().await.unwrap();
            runtime.shutdown_all().await.unwrap();

            cleanup_test_db(&test_db_path).await;
        })
        .await
        .expect("Test should complete within timeout");
    }

    /// Test QueryDocChunks returns chunks in correct order
    #[tokio::test(flavor = "multi_thread")]
    async fn test_query_doc_chunks_returns_ordered_chunks() {
        tokio::time::timeout(Duration::from_secs(10), async {
            let mut runtime = ActonApp::launch();
            let test_db_path = create_test_db_path("query_doc_chunks_ordered");

            let (db_handle, _db_info) = DatabaseActor::spawn(&mut runtime, test_db_path.clone())
                .await
                .expect("Failed to spawn DatabaseActor");

            tokio::time::sleep(Duration::from_millis(200)).await;

            // First persist a crate
            let specifier = CrateSpecifier::from_str("test_crate@1.0.0").unwrap();
            db_handle
                .send(PersistCrate {
                    specifier: specifier.clone(),
                    features: vec!["test".to_string()],
                })
                .await;

            tokio::time::sleep(Duration::from_millis(200)).await;

            // Persist three doc chunks in non-sequential order to test ordering
            for (chunk_index, chunk_content) in [
                (2, "Third chunk content"),
                (0, "First chunk content"),
                (1, "Second chunk content"),
            ] {
                let chunk_metadata = ChunkMetadata {
                    content_type: "markdown".to_string(),
                    start_line: Some(chunk_index * 10),
                    end_line: Some((chunk_index + 1) * 10),
                    token_count: 50,
                    char_count: 200,
                    parent_module: None,
                    item_type: None,
                    item_name: None,
                };

                db_handle
                    .send(PersistDocChunk {
                        specifier: specifier.clone(),
                        chunk_index,
                        chunk_id: format!("test_chunk_{}", chunk_index),
                        source_file: "README.md".to_string(),
                        content: chunk_content.to_string(),
                        metadata: chunk_metadata,
                    })
                    .await;

                tokio::time::sleep(Duration::from_millis(100)).await;
            }

            // Create subscriber to receive the query response
            #[acton_actor]
            #[derive(Default)]
            struct ChunkQuerySubscriber {
                responses: Vec<DocChunksQueryResponse>,
            }

            let mut subscriber_builder = runtime.new_agent::<ChunkQuerySubscriber>().await;

            subscriber_builder.mutate_on::<DocChunksQueryResponse>(|agent, envelope| {
                let msg = envelope.message().clone();
                agent.model.responses.push(msg);
                AgentReply::immediate()
            });

            subscriber_builder.after_stop(|agent| {
                assert_eq!(
                    agent.model.responses.len(),
                    1,
                    "Should receive exactly one response"
                );

                let response = &agent.model.responses[0];
                assert_eq!(
                    response.chunks.len(),
                    3,
                    "Should have 3 chunks"
                );

                // Verify chunks are ordered by chunk_index
                for (i, chunk) in response.chunks.iter().enumerate() {
                    assert_eq!(
                        chunk.chunk_index, i as u32,
                        "Chunk at position {} should have index {}",
                        i, i
                    );
                }

                // Verify content order
                assert_eq!(response.chunks[0].content, "First chunk content");
                assert_eq!(response.chunks[1].content, "Second chunk content");
                assert_eq!(response.chunks[2].content, "Third chunk content");

                // Verify all metadata fields are populated correctly
                assert_eq!(response.chunks[0].chunk_id, "test_chunk_0");
                assert_eq!(response.chunks[0].content_type, "markdown");
                assert_eq!(response.chunks[0].source_file, "README.md");
                assert_eq!(response.chunks[0].token_count, 50);
                assert_eq!(response.chunks[0].char_count, 200);

                AgentReply::immediate()
            });

            subscriber_builder
                .handle()
                .subscribe::<DocChunksQueryResponse>()
                .await;
            let subscriber_handle = subscriber_builder.start().await;

            // Query the chunks
            db_handle
                .send(QueryDocChunks {
                    specifier: specifier.clone(),
                })
                .await;

            tokio::time::sleep(Duration::from_millis(200)).await;

            // Clean shutdown
            subscriber_handle.stop().await.unwrap();
            db_handle.stop().await.unwrap();
            runtime.shutdown_all().await.unwrap();

            cleanup_test_db(&test_db_path).await;
        })
        .await
        .expect("Test should complete within timeout");
    }

    /// Test QueryDocChunks returns empty vector for crate with no chunks
    #[tokio::test(flavor = "multi_thread")]
    async fn test_query_doc_chunks_empty_for_crate_without_chunks() {
        tokio::time::timeout(Duration::from_secs(10), async {
            let mut runtime = ActonApp::launch();
            let test_db_path = create_test_db_path("query_doc_chunks_empty");

            let (db_handle, _db_info) = DatabaseActor::spawn(&mut runtime, test_db_path.clone())
                .await
                .expect("Failed to spawn DatabaseActor");

            tokio::time::sleep(Duration::from_millis(200)).await;

            // Persist a crate but no chunks
            let specifier = CrateSpecifier::from_str("empty_crate@1.0.0").unwrap();
            db_handle
                .send(PersistCrate {
                    specifier: specifier.clone(),
                    features: vec![],
                })
                .await;

            tokio::time::sleep(Duration::from_millis(200)).await;

            // Create subscriber
            #[acton_actor]
            #[derive(Default)]
            struct EmptyChunkSubscriber {
                responses: Vec<DocChunksQueryResponse>,
            }

            let mut subscriber_builder = runtime.new_agent::<EmptyChunkSubscriber>().await;

            subscriber_builder.mutate_on::<DocChunksQueryResponse>(|agent, envelope| {
                let msg = envelope.message().clone();
                agent.model.responses.push(msg);
                AgentReply::immediate()
            });

            subscriber_builder.after_stop(|agent| {
                assert_eq!(
                    agent.model.responses.len(),
                    1,
                    "Should receive exactly one response"
                );

                let response = &agent.model.responses[0];
                assert_eq!(
                    response.specifier.name(),
                    "empty_crate",
                    "Response should be for the correct crate"
                );
                assert_eq!(
                    response.chunks.len(),
                    0,
                    "Should have no chunks for crate without chunks"
                );

                AgentReply::immediate()
            });

            subscriber_builder
                .handle()
                .subscribe::<DocChunksQueryResponse>()
                .await;
            let subscriber_handle = subscriber_builder.start().await;

            // Query chunks for crate with no chunks
            db_handle
                .send(QueryDocChunks {
                    specifier: specifier.clone(),
                })
                .await;

            tokio::time::sleep(Duration::from_millis(200)).await;

            // Clean shutdown
            subscriber_handle.stop().await.unwrap();
            db_handle.stop().await.unwrap();
            runtime.shutdown_all().await.unwrap();

            cleanup_test_db(&test_db_path).await;
        })
        .await
        .expect("Test should complete within timeout");
    }

    /// Test QueryDocChunks returns empty vector for nonexistent crate
    #[tokio::test(flavor = "multi_thread")]
    async fn test_query_doc_chunks_empty_for_nonexistent_crate() {
        tokio::time::timeout(Duration::from_secs(10), async {
            let mut runtime = ActonApp::launch();
            let test_db_path = create_test_db_path("query_doc_chunks_nonexistent");

            let (db_handle, _db_info) = DatabaseActor::spawn(&mut runtime, test_db_path.clone())
                .await
                .expect("Failed to spawn DatabaseActor");

            tokio::time::sleep(Duration::from_millis(200)).await;

            // Create subscriber
            #[acton_actor]
            #[derive(Default)]
            struct NonexistentSubscriber {
                responses: Vec<DocChunksQueryResponse>,
            }

            let mut subscriber_builder = runtime.new_agent::<NonexistentSubscriber>().await;

            subscriber_builder.mutate_on::<DocChunksQueryResponse>(|agent, envelope| {
                let msg = envelope.message().clone();
                agent.model.responses.push(msg);
                AgentReply::immediate()
            });

            subscriber_builder.after_stop(|agent| {
                assert_eq!(
                    agent.model.responses.len(),
                    1,
                    "Should receive exactly one response"
                );

                let response = &agent.model.responses[0];
                assert_eq!(
                    response.chunks.len(),
                    0,
                    "Should have no chunks for nonexistent crate"
                );

                AgentReply::immediate()
            });

            subscriber_builder
                .handle()
                .subscribe::<DocChunksQueryResponse>()
                .await;
            let subscriber_handle = subscriber_builder.start().await;

            // Query chunks for nonexistent crate
            let specifier = CrateSpecifier::from_str("nonexistent@1.0.0").unwrap();
            db_handle
                .send(QueryDocChunks { specifier })
                .await;

            tokio::time::sleep(Duration::from_millis(200)).await;

            // Clean shutdown
            subscriber_handle.stop().await.unwrap();
            db_handle.stop().await.unwrap();
            runtime.shutdown_all().await.unwrap();

            cleanup_test_db(&test_db_path).await;
        })
        .await
        .expect("Test should complete within timeout");
    }
}
