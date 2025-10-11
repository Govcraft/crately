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
    CrateDownloaded, CrateListResponse, CrateProcessingComplete, CrateProcessingFailed,
    CrateQueryResponse, CrateSummary, DatabaseError, DatabaseReady, DocumentationChunked,
    DocumentationExtracted, DocumentationVectorized, ListCrates, PersistCodeSample,
    PersistCrate, PersistDocChunk, PersistEmbedding, QueryCrate, QuerySimilarDocs,
    SimilarDocsResponse,
};
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

        // Subscribe to pipeline events for state tracking
        builder.handle().subscribe::<CrateDownloaded>().await;
        builder.handle().subscribe::<DocumentationExtracted>().await;
        builder.handle().subscribe::<DocumentationChunked>().await;
        builder.handle().subscribe::<DocumentationVectorized>().await;
        builder.handle().subscribe::<CrateProcessingComplete>().await;
        builder.handle().subscribe::<CrateProcessingFailed>().await;

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
                        debug!(
                            "Updated crate status to downloaded: {}@{}",
                            name, version
                        );
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
                        debug!(
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

        // Handle DocumentationChunked - update status and record chunking
        builder.act_on::<DocumentationChunked>(|agent, envelope| {
            let msg = envelope.message().clone();
            let db = agent.model.db.clone();
            let broker = agent.broker().clone();

            AgentReply::from_async(async move {
                let name = msg.specifier.name().to_string();
                let version = msg.specifier.version().to_string();
                let chunk_count = msg.chunk_count;

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
                        debug!(
                            "Updated crate status to chunked: {}@{} ({} chunks)",
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

        // Handle DocumentationVectorized - update status and record vectorization
        builder.act_on::<DocumentationVectorized>(|agent, envelope| {
            let msg = envelope.message().clone();
            let db = agent.model.db.clone();
            let broker = agent.broker().clone();

            AgentReply::from_async(async move {
                let name = msg.specifier.name().to_string();
                let version = msg.specifier.version().to_string();
                let vector_count = msg.vector_count;
                let embedding_model = msg.embedding_model.clone();

                let update_query = r#"
                    UPDATE crate SET
                        status = 'vectorized',
                        status_updated_at = time::now(),
                        vectorization_completed_at = time::now()
                    WHERE name = $name AND version = $version
                "#;

                match db
                    .query(update_query)
                    .bind(("name", name.clone()))
                    .bind(("version", version.clone()))
                    .await
                {
                    Ok(_) => {
                        debug!(
                            "Updated crate status to vectorized: {}@{} ({} vectors, model: {})",
                            name, version, vector_count, embedding_model
                        );
                    }
                    Err(e) => {
                        error!("Failed to update crate vectorization status: {}", e);
                        broker
                            .broadcast(DatabaseError {
                                operation: format!(
                                    "update vectorization status for {}@{}",
                                    name, version
                                ),
                                error: e.to_string(),
                            })
                            .await;
                    }
                }
            })
        });

        // Handle CrateProcessingComplete - update final status
        builder.act_on::<CrateProcessingComplete>(|agent, envelope| {
            let msg = envelope.message().clone();
            let db = agent.model.db.clone();
            let broker = agent.broker().clone();

            AgentReply::from_async(async move {
                let name = msg.specifier.name().to_string();
                let version = msg.specifier.version().to_string();
                let total_duration_ms = msg.total_duration_ms;

                let update_query = r#"
                    UPDATE crate SET
                        status = 'complete',
                        status_updated_at = time::now(),
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
                        debug!(
                            "Updated crate status to complete: {}@{} (total duration: {}ms)",
                            name, version, total_duration_ms
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
                        debug!(
                            "Updated crate status to failed: {}@{} (stage: {}, error: {})",
                            name, version, stage, error
                        );
                    }
                    Err(e) => {
                        error!("Failed to record crate processing failure: {}", e);
                        broker
                            .broadcast(DatabaseError {
                                operation: format!(
                                    "record failure for {}@{}",
                                    name, version
                                ),
                                error: e.to_string(),
                            })
                            .await;
                    }
                }
            })
        });

        // Handle PersistDocChunk - insert documentation chunk
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
                        let crate_id: Result<Option<serde_json::Value>, _> = response.take(0);

                        match crate_id {
                            Ok(Some(crate_record)) => {
                                if let Some(id) = crate_record.get("id") {
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
                                        .bind(("start_line", msg.metadata.start_line.map(|n| n as i64)))
                                        .bind(("end_line", msg.metadata.end_line.map(|n| n as i64)))
                                        .bind(("parent_module", msg.metadata.parent_module.clone()))
                                        .bind(("item_type", msg.metadata.item_type.clone()))
                                        .bind(("item_name", msg.metadata.item_name.clone()))
                                        .await
                                    {
                                        Ok(_) => {
                                            debug!("Persisted doc chunk {} for {}@{}", msg.chunk_id, name, version);
                                        }
                                        Err(e) => {
                                            error!("Failed to persist doc chunk: {}", e);
                                            broker
                                                .broadcast(DatabaseError {
                                                    operation: format!("persist doc chunk {}", msg.chunk_id),
                                                    error: e.to_string(),
                                                })
                                                .await;
                                        }
                                    }
                                } else {
                                    error!("Crate record missing id field");
                                }
                            }
                            Ok(None) => {
                                error!(
                                    "Crate not found for chunk: {}@{}",
                                    name, version
                                );
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
                    Ok(mut crate_response) => {
                        let crate_id: Result<Option<serde_json::Value>, _> = crate_response.take(0);

                        if let Ok(Some(crate_record)) = crate_id {
                            // Get chunk ID
                            let chunk_query = "SELECT id FROM doc_chunk WHERE chunk_id = $chunk_id";

                            match db
                                .query(chunk_query)
                                .bind(("chunk_id", msg.chunk_id.clone()))
                                .await
                            {
                                Ok(mut chunk_response) => {
                                    let chunk_id: Result<Option<serde_json::Value>, _> = chunk_response.take(0);

                                    match chunk_id {
                                        Ok(Some(chunk_record)) => {
                                            if let (Some(crate_id_val), Some(chunk_id_val)) =
                                                (crate_record.get("id"), chunk_record.get("id"))
                                            {
                                                let vector_dim = msg.vector.len();

                                                let insert_query = r#"
                                                    CREATE embedding CONTENT {
                                                        chunk_id: $chunk_id,
                                                        crate_id: $crate_id,
                                                        vector: $vector,
                                                        vector_dimension: $vector_dimension,
                                                        model_name: $model_name,
                                                        model_version: $model_version,
                                                        content_hash: crypto::md5(string::concat($chunk_id, $model_name))
                                                    }
                                                "#;

                                                match db
                                                    .query(insert_query)
                                                    .bind(("chunk_id", chunk_id_val.clone()))
                                                    .bind(("crate_id", crate_id_val.clone()))
                                                    .bind(("vector", msg.vector.clone()))
                                                    .bind(("vector_dimension", vector_dim as i64))
                                                    .bind(("model_name", msg.model_name.clone()))
                                                    .bind(("model_version", msg.model_version.clone()))
                                                    .await
                                                {
                                                    Ok(_) => {
                                                        debug!(
                                                            "Persisted embedding for chunk {} (dim: {}, model: {})",
                                                            msg.chunk_id, vector_dim, msg.model_name
                                                        );

                                                        // Update chunk's vectorized flag
                                                        let _ = db
                                                            .query("UPDATE doc_chunk SET vectorized = true, vectorized_at = time::now() WHERE chunk_id = $chunk_id")
                                                            .bind(("chunk_id", msg.chunk_id.clone()))
                                                            .await;
                                                    }
                                                    Err(e) => {
                                                        error!("Failed to persist embedding: {}", e);
                                                        broker
                                                            .broadcast(DatabaseError {
                                                                operation: format!("persist embedding for {}", msg.chunk_id),
                                                                error: e.to_string(),
                                                            })
                                                            .await;
                                                    }
                                                }
                                            }
                                        }
                                        Ok(None) => {
                                            error!("Chunk not found: {}", msg.chunk_id);
                                            broker
                                                .broadcast(DatabaseError {
                                                    operation: format!("persist embedding for {}", msg.chunk_id),
                                                    error: format!("Chunk {} not found in database", msg.chunk_id),
                                                })
                                                .await;
                                        }
                                        Err(e) => {
                                            error!("Failed to parse chunk lookup result: {}", e);
                                            broker
                                                .broadcast(DatabaseError {
                                                    operation: format!("persist embedding for {}", msg.chunk_id),
                                                    error: e.to_string(),
                                                })
                                                .await;
                                        }
                                    }
                                }
                                Err(e) => {
                                    error!("Failed to lookup chunk for embedding: {}", e);
                                    broker
                                        .broadcast(DatabaseError {
                                            operation: format!("persist embedding for {}", msg.chunk_id),
                                            error: e.to_string(),
                                        })
                                        .await;
                                }
                            }
                        } else {
                            error!(
                                "Crate not found for embedding: {}@{}",
                                name, version
                            );
                            broker
                                .broadcast(DatabaseError {
                                    operation: format!("persist embedding for {}", msg.chunk_id),
                                    error: format!(
                                        "Crate {}@{} not found in database",
                                        name, version
                                    ),
                                })
                                .await;
                        }
                    }
                    Err(e) => {
                        error!("Failed to lookup crate for embedding persistence: {}", e);
                        broker
                            .broadcast(DatabaseError {
                                operation: format!("persist embedding for {}", msg.chunk_id),
                                error: e.to_string(),
                            })
                            .await;
                    }
                }
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
                        let crate_id: Result<Option<serde_json::Value>, _> = response.take(0);

                        match crate_id {
                            Ok(Some(crate_record)) => {
                                if let Some(id) = crate_record.get("id") {
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
                                                msg.sample_index,
                                                name,
                                                version,
                                                msg.sample_type
                                            );
                                        }
                                        Err(e) => {
                                            error!("Failed to persist code sample: {}", e);
                                            broker
                                                .broadcast(DatabaseError {
                                                    operation: format!(
                                                        "persist code sample {} for {}@{}",
                                                        msg.sample_index,
                                                        name,
                                                        version
                                                    ),
                                                    error: e.to_string(),
                                                })
                                                .await;
                                        }
                                    }
                                } else {
                                    error!("Crate record missing id field");
                                }
                            }
                            Ok(None) => {
                                error!(
                                    "Crate not found for code sample: {}@{}",
                                    name, version
                                );
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
                                operation: format!(
                                    "persist code sample for {}@{}",
                                    name, version
                                ),
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

        // Add after_start hook to create schema and broadcast ready event
        builder.after_start(|agent| {
            let db = agent.model.db.clone();
            let broker = agent.broker().clone();

            AgentReply::from_async(async move {
                info!("Initializing database schema...");

                // Execute schema definition
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
                        ASSERT $value IN ['pending', 'downloading', 'downloaded', 'extracting',
                                          'extracted', 'compiling_docs', 'docs_compiled',
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
        error_responses: Vec<DatabaseError>,
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
            assert!(
                stop_result.is_ok(),
                "DatabaseActor should stop gracefully"
            );

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
                assert_eq!(agent.model.query_responses.len(), 1, "Should receive exactly one query response");

                let response = &agent.model.query_responses[0];
                assert!(response.specifier.is_some(), "Should find the crate");

                let spec = response.specifier.as_ref().unwrap();
                assert_eq!(spec.name(), "serde", "Crate name should match");
                assert_eq!(spec.version().to_string(), "1.0.0", "Version should match");
                assert_eq!(response.features, vec!["derive"], "Features should match");
                assert_eq!(response.status, "pending", "Status should be pending");
                assert!(!response.created_at.is_empty(), "Should have created_at timestamp");

                AgentReply::immediate()
            });

            // Subscribe to broadcast messages before starting
            subscriber_builder.handle().subscribe::<CrateQueryResponse>().await;
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
            assert_eq!(agent.model.query_responses.len(), 1, "Should receive exactly one query response");

            let response = &agent.model.query_responses[0];
            assert!(response.specifier.is_some(), "Should find the latest version");

            let spec = response.specifier.as_ref().unwrap();
            assert_eq!(spec.name(), "tokio", "Crate name should match");
            assert_eq!(spec.version().to_string(), "1.36.0", "Should return latest version");
            assert_eq!(response.features, vec!["full"], "Features should match latest version");
            assert_eq!(response.status, "pending", "Status should be pending");
            assert!(!response.created_at.is_empty(), "Should have created_at timestamp");

            AgentReply::immediate()
        });

            // Subscribe to broadcast messages before starting
            subscriber_builder.handle().subscribe::<CrateQueryResponse>().await;
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
            assert_eq!(agent.model.query_responses.len(), 1, "Should receive exactly one query response");

            let response = &agent.model.query_responses[0];
            assert!(response.specifier.is_none(), "Should not find non-existent crate");
            assert_eq!(response.status, "not_found", "Status should be not_found");
            assert!(response.features.is_empty(), "Features should be empty");
            assert!(response.created_at.is_empty(), "created_at should be empty");

            AgentReply::immediate()
        });

        // Subscribe to broadcast messages before starting
        subscriber_builder.handle().subscribe::<CrateQueryResponse>().await;
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
            assert_eq!(agent.model.list_responses.len(), 1, "Should receive exactly one list response");

            let response = &agent.model.list_responses[0];
            assert_eq!(response.crates.len(), 2, "Should list 2 crates");
            assert_eq!(response.total_count, 2, "Total count should be 2");

            // Verify crate summaries contain expected data
            let crate_names: Vec<&str> = response.crates.iter().map(|c| c.name.as_str()).collect();
            assert!(crate_names.contains(&"serde"), "Should contain serde");
            assert!(crate_names.contains(&"tokio"), "Should contain tokio");

            AgentReply::immediate()
        });

        // Subscribe to broadcast messages before starting
        subscriber_builder.handle().subscribe::<CrateListResponse>().await;
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
            assert_eq!(agent.model.list_responses.len(), 2, "Should receive two list responses (2 pages)");

            // First page should have 2 crates
            let first_page = &agent.model.list_responses[0];
            assert_eq!(first_page.crates.len(), 2, "First page should have 2 crates");
            assert_eq!(first_page.total_count, 5, "Total count should be 5");

            // Second page should have 2 crates
            let second_page = &agent.model.list_responses[1];
            assert_eq!(second_page.crates.len(), 2, "Second page should have 2 crates");
            assert_eq!(second_page.total_count, 5, "Total count should be 5");

            AgentReply::immediate()
        });

        // Subscribe to broadcast messages before starting
        subscriber_builder.handle().subscribe::<CrateListResponse>().await;
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
            assert_eq!(agent.model.list_responses.len(), 1, "Should receive exactly one list response");

            let response = &agent.model.list_responses[0];
            assert!(response.crates.is_empty(), "Should return empty crates list");
            assert_eq!(response.total_count, 0, "Total count should be 0");

            AgentReply::immediate()
        });

        // Subscribe to broadcast messages before starting
        subscriber_builder.handle().subscribe::<CrateListResponse>().await;
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
}
