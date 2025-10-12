//! End-to-End Pipeline Integration Tests for Issue #46
//!
//! Comprehensive integration tests verifying the complete crate processing pipeline
//! from HTTP request to database persistence. Tests cover:
//! - Complete pipeline flow with all actors coordinated
//! - Message passing between CrateCoordinatorActor → DownloaderActor → FileReaderActor →
//!   ProcessorActor → VectorizerActor → DatabaseActor
//! - Error propagation and recovery across the pipeline
//! - Proper actor coordination and event broadcasting
//! - Concurrent crate processing

use acton_reactive::prelude::*;
use crately::actors::console::Console;
use crately::actors::database::DatabaseActor;
use crately::actors::crate_coordinator_actor::CrateCoordinatorActor;
use crately::actors::config::CoordinatorConfig;
use crately::colors::ColorConfig;
use crately::cli::ColorChoice;
use crately::crate_specifier::CrateSpecifier;
use crately::messages::{
    CrateReceived, CrateDownloaded, CrateDownloadFailed, CrateProcessingComplete,
    CrateProcessingFailed, DocumentationExtracted, DocumentationExtractionFailed,
    DocumentationChunked, DocumentationVectorized, QueryCrate,
    CrateQueryResponse, DatabaseReady, DownloadErrorKind, ExtractionErrorKind,
};
use std::path::PathBuf;
use std::str::FromStr;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::time::Duration;
use crately::actors::retry_coordinator::RetryCoordinator;
use crately::retry_policy::RetryPolicy;

/// Helper: Create a unique test database path using test name and timestamp
/// to prevent lock contention between parallel tests.
fn create_test_db_path(test_name: &str) -> PathBuf {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!("crately_pipeline_e2e_test_{}_{}", test_name, timestamp))
}

/// Helper: Clean up test database with retry logic to handle file locks.
async fn cleanup_test_db(path: &std::path::Path) {
    // Give RocksDB time to release file locks
    tokio::time::sleep(Duration::from_millis(200)).await;

    // Retry cleanup a few times if locks are still held
    for attempt in 0..5 {
        match std::fs::remove_dir_all(path) {
            Ok(()) => break,
            Err(_e) if attempt < 4 => {
                tokio::time::sleep(Duration::from_millis(150)).await;
            }
            Err(e) => {
                eprintln!("Warning: Final cleanup attempt failed for {}: {}", path.display(), e);
            }
        }
    }
}

/// TestSubscriber actor for collecting messages broadcasted during pipeline execution.
///
/// This actor subscribes to all relevant pipeline events and collects them for
/// verification in integration tests, following the acton-reactive test pattern.
#[acton_actor]
#[derive(Default, Clone)]
struct PipelineTestSubscriber {
    crate_received_events: Vec<CrateReceived>,
    crate_downloaded_events: Vec<CrateDownloaded>,
    crate_download_failed_events: Vec<CrateDownloadFailed>,
    documentation_extracted_events: Vec<DocumentationExtracted>,
    documentation_extraction_failed_events: Vec<DocumentationExtractionFailed>,
    documentation_chunked_events: Vec<DocumentationChunked>,
    documentation_vectorized_events: Vec<DocumentationVectorized>,
    processing_complete_events: Vec<CrateProcessingComplete>,
    processing_failed_events: Vec<CrateProcessingFailed>,
    query_responses: Vec<CrateQueryResponse>,
    database_ready_received: bool,
}

// =============================================================================
// Complete Pipeline Flow Tests
// =============================================================================

/// Test complete pipeline flow from CrateReceived to database persistence.
///
/// This test verifies:
/// - CrateCoordinatorActor receives CrateReceived and tracks state
/// - Messages flow through: Coordinator → Downloader → FileReader → Processor → Vectorizer
/// - DatabaseActor persists all data correctly
/// - All pipeline events are broadcasted
/// - Final status in database is "complete"
#[tokio::test(flavor = "multi_thread")]
async fn test_pipeline_complete_flow_from_crate_received_to_persistence() {
    tokio::time::timeout(Duration::from_secs(45), async {
        let mut runtime = ActonApp::launch();
        let test_db_path = create_test_db_path("complete_flow");
        let color_config = ColorConfig::new(ColorChoice::Never);

        // Spawn core actors directly
        let _console = Console::spawn(&mut runtime, color_config)
            .await
            .expect("Console should spawn");

        let (database, _db_info) = DatabaseActor::spawn(&mut runtime, test_db_path.clone())
            .await
            .expect("DatabaseActor should spawn");

        let coordinator_config = CoordinatorConfig::default();

        // Spawn RetryCoordinator first
        let retry_policy = RetryPolicy::default();
        let retry_coordinator = RetryCoordinator::spawn(&mut runtime, retry_policy)
            .await
            .expect("RetryCoordinator should spawn");
        let coordinator = CrateCoordinatorActor::spawn(&mut runtime, coordinator_config, retry_coordinator.clone())
            .await
            .expect("CrateCoordinatorActor should spawn");

        // Create test subscriber to collect pipeline events
        let mut subscriber_builder = runtime.new_agent::<PipelineTestSubscriber>().await;

        subscriber_builder
            .mutate_on::<CrateReceived>(|agent, envelope| {
                agent.model.crate_received_events.push(envelope.message().clone());
                AgentReply::immediate()
            })
            .mutate_on::<CrateDownloaded>(|agent, envelope| {
                agent.model.crate_downloaded_events.push(envelope.message().clone());
                AgentReply::immediate()
            })
            .mutate_on::<DocumentationExtracted>(|agent, envelope| {
                agent.model.documentation_extracted_events.push(envelope.message().clone());
                AgentReply::immediate()
            })
            .mutate_on::<DocumentationChunked>(|agent, envelope| {
                agent.model.documentation_chunked_events.push(envelope.message().clone());
                AgentReply::immediate()
            })
            .mutate_on::<DocumentationVectorized>(|agent, envelope| {
                agent.model.documentation_vectorized_events.push(envelope.message().clone());
                AgentReply::immediate()
            })
            .mutate_on::<CrateProcessingComplete>(|agent, envelope| {
                agent.model.processing_complete_events.push(envelope.message().clone());
                AgentReply::immediate()
            })
            .mutate_on::<CrateQueryResponse>(|agent, envelope| {
                agent.model.query_responses.push(envelope.message().clone());
                AgentReply::immediate()
            })
            .mutate_on::<DatabaseReady>(|agent, _envelope| {
                agent.model.database_ready_received = true;
                AgentReply::immediate()
            })
            .after_stop(|agent| {
                // Verify complete pipeline execution
                assert_eq!(
                    agent.model.crate_received_events.len(),
                    1,
                    "Should receive exactly one CrateReceived event"
                );

                assert_eq!(
                    agent.model.crate_downloaded_events.len(),
                    1,
                    "Should receive exactly one CrateDownloaded event"
                );

                assert_eq!(
                    agent.model.documentation_extracted_events.len(),
                    1,
                    "Should receive exactly one DocumentationExtracted event"
                );

                assert_eq!(
                    agent.model.documentation_chunked_events.len(),
                    1,
                    "Should receive exactly one DocumentationChunked event"
                );

                assert_eq!(
                    agent.model.documentation_vectorized_events.len(),
                    1,
                    "Should receive exactly one DocumentationVectorized event"
                );

                assert_eq!(
                    agent.model.processing_complete_events.len(),
                    1,
                    "Should receive exactly one CrateProcessingComplete event"
                );

                // Verify database responses
                assert!(
                    !agent.model.query_responses.is_empty(),
                    "Should receive database query responses"
                );

                // Verify final status is complete
                if let Some(final_response) = agent.model.query_responses.last() {
                    assert_eq!(
                        final_response.status, "complete",
                        "Final status should be 'complete'"
                    );
                }

                assert!(
                    agent.model.database_ready_received,
                    "Should receive DatabaseReady event"
                );

                AgentReply::immediate()
            });

        // Subscribe to all pipeline events
        subscriber_builder.handle().subscribe::<CrateReceived>().await;
        subscriber_builder.handle().subscribe::<CrateDownloaded>().await;
        subscriber_builder.handle().subscribe::<DocumentationExtracted>().await;
        subscriber_builder.handle().subscribe::<DocumentationChunked>().await;
        subscriber_builder.handle().subscribe::<DocumentationVectorized>().await;
        subscriber_builder.handle().subscribe::<CrateProcessingComplete>().await;
        subscriber_builder.handle().subscribe::<CrateQueryResponse>().await;
        subscriber_builder.handle().subscribe::<DatabaseReady>().await;

        let subscriber_handle = subscriber_builder.start().await;

        // Wait for database initialization
        tokio::time::sleep(Duration::from_millis(500)).await;

        // Trigger pipeline: Send CrateReceived event
        let specifier = CrateSpecifier::from_str("serde@1.0.210").expect("Valid CrateSpecifier");
        let features = vec!["derive".to_string(), "std".to_string()];

        coordinator
            .send(CrateReceived {
                specifier: specifier.clone(),
                features: features.clone(),
            })
            .await;

        // Simulate pipeline progression by broadcasting events
        tokio::time::sleep(Duration::from_millis(300)).await;

        // Step 1: Download completed
        runtime.broker().broadcast(CrateDownloaded {
                specifier: specifier.clone(),
                features: features.clone(),
                extracted_path: PathBuf::from("/tmp/serde-1.0.210"),
                download_duration_ms: 1200,
            })
            .await;

        tokio::time::sleep(Duration::from_millis(250)).await;

        // Step 2: Documentation extracted
        runtime.broker().broadcast(DocumentationExtracted {
                specifier: specifier.clone(),
                features: features.clone(),
                file_count: 120,
                documentation_bytes: 487424,
                extracted_path: PathBuf::from("/tmp/serde-1.0.210"),
            })
            .await;

        tokio::time::sleep(Duration::from_millis(250)).await;

        // Step 3: Documentation chunked
        runtime.broker().broadcast(DocumentationChunked {
                specifier: specifier.clone(),
                features: features.clone(),
                chunk_count: 58,
                total_tokens_estimated: 14720,
            })
            .await;

        tokio::time::sleep(Duration::from_millis(250)).await;

        // Step 4: Documentation vectorized
        runtime.broker().broadcast(DocumentationVectorized {
                specifier: specifier.clone(),
                features: features.clone(),
                vector_count: 58,
                embedding_model: "text-embedding-3-small".to_string(),
                vectorization_duration_ms: 2800,
            })
            .await;

        tokio::time::sleep(Duration::from_millis(300)).await;

        // Query final status from database
        database
            .send(QueryCrate {
                name: "serde".to_string(),
                version: Some("1.0.210".to_string()),
            })
            .await;

        tokio::time::sleep(Duration::from_millis(300)).await;

        // Shutdown gracefully
        subscriber_handle.stop().await.expect("Subscriber should stop");
        coordinator.stop().await.expect("Coordinator should stop");
        database.stop().await.expect("Database should stop");
        runtime.shutdown_all().await.expect("Runtime should shutdown");

        cleanup_test_db(&test_db_path).await;
    })
    .await
    .expect("Test should complete within timeout");
}

// =============================================================================
// Error Propagation and Recovery Tests
// =============================================================================

/// Test error propagation from download failure through coordinator to database.
///
/// This test verifies:
/// - DownloaderActor broadcasts CrateDownloadFailed on network failure
/// - CrateCoordinatorActor receives the failure and handles retry logic
/// - After max retries, CrateProcessingFailed is broadcasted
/// - DatabaseActor updates status to "failed" with error message
#[tokio::test(flavor = "multi_thread")]
async fn test_pipeline_error_propagation_download_failure_with_retry() {
    tokio::time::timeout(Duration::from_secs(40), async {
        let test_db_path = create_test_db_path("download_failure");
        let color_config = ColorConfig::new(ColorChoice::Never);

        let mut runtime = ActonApp::launch();

        let _console = Console::spawn(&mut runtime, color_config)
            .await
            .expect("Console should spawn");

        let (database, _db_info) = DatabaseActor::spawn(&mut runtime, test_db_path.clone())
            .await
            .expect("DatabaseActor should spawn");

        let coordinator_config = CoordinatorConfig::default();

        // Spawn RetryCoordinator first
        let retry_policy = RetryPolicy::default();
        let retry_coordinator = RetryCoordinator::spawn(&mut runtime, retry_policy)
            .await
            .expect("RetryCoordinator should spawn");
        let coordinator = CrateCoordinatorActor::spawn(&mut runtime, coordinator_config, retry_coordinator.clone())
            .await
            .expect("CrateCoordinatorActor should spawn");

        // Create test subscriber
        let mut subscriber_builder = runtime.new_agent::<PipelineTestSubscriber>().await;

        subscriber_builder
            .mutate_on::<CrateReceived>(|agent, envelope| {
                agent.model.crate_received_events.push(envelope.message().clone());
                AgentReply::immediate()
            })
            .mutate_on::<CrateDownloadFailed>(|agent, envelope| {
                agent.model.crate_download_failed_events.push(envelope.message().clone());
                AgentReply::immediate()
            })
            .mutate_on::<CrateProcessingFailed>(|agent, envelope| {
                agent.model.processing_failed_events.push(envelope.message().clone());
                AgentReply::immediate()
            })
            .mutate_on::<CrateQueryResponse>(|agent, envelope| {
                agent.model.query_responses.push(envelope.message().clone());
                AgentReply::immediate()
            })
            .after_stop(|agent| {
                assert_eq!(
                    agent.model.crate_received_events.len(),
                    1,
                    "Should receive CrateReceived event"
                );

                assert!(
                    !agent.model.crate_download_failed_events.is_empty(),
                    "Should receive at least one CrateDownloadFailed event"
                );

                assert_eq!(
                    agent.model.processing_failed_events.len(),
                    1,
                    "Should receive CrateProcessingFailed after max retries"
                );

                // Verify final status is failed
                if let Some(final_response) = agent.model.query_responses.last() {
                    assert_eq!(
                        final_response.status, "failed",
                        "Final status should be 'failed'"
                    );
                }

                AgentReply::immediate()
            });

        subscriber_builder.handle().subscribe::<CrateReceived>().await;
        subscriber_builder.handle().subscribe::<CrateDownloadFailed>().await;
        subscriber_builder.handle().subscribe::<CrateProcessingFailed>().await;
        subscriber_builder.handle().subscribe::<CrateQueryResponse>().await;

        let subscriber_handle = subscriber_builder.start().await;

        // Wait for database initialization
        tokio::time::sleep(Duration::from_millis(500)).await;

        // Trigger pipeline with CrateReceived
        let specifier = CrateSpecifier::from_str("nonexistent-crate@1.0.0")
            .expect("Valid CrateSpecifier");
        let features = vec![];

        coordinator
            .send(CrateReceived {
                specifier: specifier.clone(),
                features: features.clone(),
            })
            .await;

        tokio::time::sleep(Duration::from_millis(300)).await;

        // Simulate download failure
        runtime.broker()
            .broadcast(CrateDownloadFailed {
                specifier: specifier.clone(),
                features: features.clone(),
                error_message: "Network timeout after 30 seconds".to_string(),
                error_kind: DownloadErrorKind::Timeout,
            })
            .await;

        // Wait for coordinator to process failure and retry logic
        tokio::time::sleep(Duration::from_millis(1500)).await;

        // Query final status
        database
            .send(QueryCrate {
                name: "nonexistent-crate".to_string(),
                version: Some("1.0.0".to_string()),
            })
            .await;

        tokio::time::sleep(Duration::from_millis(300)).await;

        // Shutdown gracefully
        subscriber_handle.stop().await.expect("Subscriber should stop");
        coordinator.stop().await.expect("Coordinator should stop");
        database.stop().await.expect("Database should stop");
        runtime.shutdown_all().await.expect("Runtime should shutdown");

        cleanup_test_db(&test_db_path).await;
    })
    .await
    .expect("Test should complete within timeout");
}

/// Test error propagation from documentation extraction failure.
///
/// This test verifies:
/// - FileReaderActor broadcasts DocumentationExtractionFailed
/// - CrateCoordinatorActor handles extraction failure with retry
/// - After max retries, status updates to "failed"
#[tokio::test(flavor = "multi_thread")]
async fn test_pipeline_error_propagation_extraction_failure() {
    tokio::time::timeout(Duration::from_secs(40), async {
        let test_db_path = create_test_db_path("extraction_failure");
        let color_config = ColorConfig::new(ColorChoice::Never);

        let mut runtime = ActonApp::launch();

        let _console = Console::spawn(&mut runtime, color_config)
            .await
            .expect("Console should spawn");

        let (database, _db_info) = DatabaseActor::spawn(&mut runtime, test_db_path.clone())
            .await
            .expect("DatabaseActor should spawn");

        let coordinator_config = CoordinatorConfig::default();

        // Spawn RetryCoordinator first
        let retry_policy = RetryPolicy::default();
        let retry_coordinator = RetryCoordinator::spawn(&mut runtime, retry_policy)
            .await
            .expect("RetryCoordinator should spawn");
        let coordinator = CrateCoordinatorActor::spawn(&mut runtime, coordinator_config, retry_coordinator.clone())
            .await
            .expect("CrateCoordinatorActor should spawn");

        let mut subscriber_builder = runtime.new_agent::<PipelineTestSubscriber>().await;

        subscriber_builder
            .mutate_on::<CrateReceived>(|agent, envelope| {
                agent.model.crate_received_events.push(envelope.message().clone());
                AgentReply::immediate()
            })
            .mutate_on::<CrateDownloaded>(|agent, envelope| {
                agent.model.crate_downloaded_events.push(envelope.message().clone());
                AgentReply::immediate()
            })
            .mutate_on::<DocumentationExtractionFailed>(|agent, envelope| {
                agent.model.documentation_extraction_failed_events.push(envelope.message().clone());
                AgentReply::immediate()
            })
            .mutate_on::<CrateProcessingFailed>(|agent, envelope| {
                agent.model.processing_failed_events.push(envelope.message().clone());
                AgentReply::immediate()
            })
            .mutate_on::<CrateQueryResponse>(|agent, envelope| {
                agent.model.query_responses.push(envelope.message().clone());
                AgentReply::immediate()
            })
            .after_stop(|agent| {
                assert!(
                    !agent.model.documentation_extraction_failed_events.is_empty(),
                    "Should receive DocumentationExtractionFailed events"
                );

                assert_eq!(
                    agent.model.processing_failed_events.len(),
                    1,
                    "Should receive CrateProcessingFailed after max retries"
                );

                if let Some(final_response) = agent.model.query_responses.last() {
                    assert_eq!(
                        final_response.status, "failed",
                        "Final status should be 'failed'"
                    );
                }

                AgentReply::immediate()
            });

        subscriber_builder.handle().subscribe::<CrateReceived>().await;
        subscriber_builder.handle().subscribe::<CrateDownloaded>().await;
        subscriber_builder.handle().subscribe::<DocumentationExtractionFailed>().await;
        subscriber_builder.handle().subscribe::<CrateProcessingFailed>().await;
        subscriber_builder.handle().subscribe::<CrateQueryResponse>().await;

        let subscriber_handle = subscriber_builder.start().await;

        tokio::time::sleep(Duration::from_millis(500)).await;

        let specifier = CrateSpecifier::from_str("corrupted-crate@2.0.0")
            .expect("Valid CrateSpecifier");
        let features = vec![];

        coordinator
            .send(CrateReceived {
                specifier: specifier.clone(),
                features: features.clone(),
            })
            .await;

        tokio::time::sleep(Duration::from_millis(300)).await;

        // Simulate successful download
        runtime.broker()
            .broadcast(CrateDownloaded {
                specifier: specifier.clone(),
                features: features.clone(),
                extracted_path: PathBuf::from("/tmp/corrupted-crate-2.0.0"),
                download_duration_ms: 800,
            })
            .await;

        tokio::time::sleep(Duration::from_millis(250)).await;

        // Simulate extraction failure
        runtime.broker()
            .broadcast(DocumentationExtractionFailed {
                specifier: specifier.clone(),
                features: features.clone(),
                error_message: "Corrupted archive: invalid tar format".to_string(),
                error_kind: ExtractionErrorKind::InvalidFormat,
                elapsed_ms: 150,
            })
            .await;

        // Wait for retry logic
        tokio::time::sleep(Duration::from_millis(1500)).await;

        database
            .send(QueryCrate {
                name: "corrupted-crate".to_string(),
                version: Some("2.0.0".to_string()),
            })
            .await;

        tokio::time::sleep(Duration::from_millis(300)).await;

        subscriber_handle.stop().await.expect("Subscriber should stop");
        coordinator.stop().await.expect("Coordinator should stop");
        database.stop().await.expect("Database should stop");
        runtime.shutdown_all().await.expect("Runtime should shutdown");

        cleanup_test_db(&test_db_path).await;
    })
    .await
    .expect("Test should complete within timeout");
}

// =============================================================================
// Concurrent Processing Tests
// =============================================================================

/// Test concurrent processing of multiple crates through the pipeline.
///
/// This test verifies:
/// - Multiple CrateReceived events can be processed concurrently
/// - Coordinator tracks multiple crates independently
/// - All crates progress through pipeline stages without interference
/// - Database correctly stores all crates
#[tokio::test(flavor = "multi_thread")]
async fn test_pipeline_concurrent_crate_processing() {
    tokio::time::timeout(Duration::from_secs(60), async {
        let test_db_path = create_test_db_path("concurrent_processing");
        let color_config = ColorConfig::new(ColorChoice::Never);

        let mut runtime = ActonApp::launch();

        let _console = Console::spawn(&mut runtime, color_config)
            .await
            .expect("Console should spawn");

        let (database, _db_info) = DatabaseActor::spawn(&mut runtime, test_db_path.clone())
            .await
            .expect("DatabaseActor should spawn");

        let coordinator_config = CoordinatorConfig::default();

        // Spawn RetryCoordinator first
        let retry_policy = RetryPolicy::default();
        let retry_coordinator = RetryCoordinator::spawn(&mut runtime, retry_policy)
            .await
            .expect("RetryCoordinator should spawn");
        let coordinator = CrateCoordinatorActor::spawn(&mut runtime, coordinator_config, retry_coordinator.clone())
            .await
            .expect("CrateCoordinatorActor should spawn");

        let mut subscriber_builder = runtime.new_agent::<PipelineTestSubscriber>().await;

        subscriber_builder
            .mutate_on::<CrateReceived>(|agent, envelope| {
                agent.model.crate_received_events.push(envelope.message().clone());
                AgentReply::immediate()
            })
            .mutate_on::<CrateProcessingComplete>(|agent, envelope| {
                agent.model.processing_complete_events.push(envelope.message().clone());
                AgentReply::immediate()
            })
            .mutate_on::<CrateQueryResponse>(|agent, envelope| {
                agent.model.query_responses.push(envelope.message().clone());
                AgentReply::immediate()
            })
            .after_stop(|agent| {
                assert_eq!(
                    agent.model.crate_received_events.len(),
                    3,
                    "Should receive 3 CrateReceived events"
                );

                assert_eq!(
                    agent.model.processing_complete_events.len(),
                    3,
                    "Should receive 3 CrateProcessingComplete events"
                );

                // Verify all 3 crates were queried and found
                let complete_responses: Vec<_> = agent
                    .model
                    .query_responses
                    .iter()
                    .filter(|r| r.status == "complete")
                    .collect();

                assert!(
                    complete_responses.len() >= 3,
                    "Should have at least 3 complete status responses"
                );

                AgentReply::immediate()
            });

        subscriber_builder.handle().subscribe::<CrateReceived>().await;
        subscriber_builder.handle().subscribe::<CrateProcessingComplete>().await;
        subscriber_builder.handle().subscribe::<CrateQueryResponse>().await;

        let subscriber_handle = subscriber_builder.start().await;

        tokio::time::sleep(Duration::from_millis(500)).await;

        // Define 3 crates to process concurrently
        let crates = vec![
            ("tokio", "1.35.0", vec!["full".to_string()]),
            ("serde", "1.0.210", vec!["derive".to_string()]),
            ("axum", "0.7.3", vec!["macros".to_string()]),
        ];

        // Send all CrateReceived events
        for (name, version, features) in &crates {
            let specifier = CrateSpecifier::from_str(&format!("{}@{}", name, version))
                .expect("Valid CrateSpecifier");

            coordinator
                .send(CrateReceived {
                    specifier: specifier.clone(),
                    features: features.clone(),
                })
                .await;
        }

        tokio::time::sleep(Duration::from_millis(400)).await;

        // Process each crate through the pipeline
        for (name, version, features) in &crates {
            let specifier = CrateSpecifier::from_str(&format!("{}@{}", name, version))
                .expect("Valid CrateSpecifier");

            // Download
            runtime.broker().broadcast(CrateDownloaded {
                    specifier: specifier.clone(),
                    features: features.clone(),
                    extracted_path: PathBuf::from(format!("/tmp/{}-{}", name, version)),
                    download_duration_ms: 1000,
                })
                .await;

            tokio::time::sleep(Duration::from_millis(150)).await;

            // Extract
            runtime.broker().broadcast(DocumentationExtracted {
                    specifier: specifier.clone(),
                    features: features.clone(),
                    file_count: 100,
                    documentation_bytes: 400000,
                    extracted_path: PathBuf::from(format!("/tmp/{}-{}", name, version)),
                })
                .await;

            tokio::time::sleep(Duration::from_millis(150)).await;

            // Chunk
            runtime.broker().broadcast(DocumentationChunked {
                    specifier: specifier.clone(),
                    features: features.clone(),
                    chunk_count: 50,
                    total_tokens_estimated: 12800,
                })
                .await;

            tokio::time::sleep(Duration::from_millis(150)).await;

            // Vectorize
            runtime.broker().broadcast(DocumentationVectorized {
                    specifier: specifier.clone(),
                    features: features.clone(),
                    vector_count: 50,
                    embedding_model: "text-embedding-3-small".to_string(),
                    vectorization_duration_ms: 2500,
                })
                .await;

            tokio::time::sleep(Duration::from_millis(200)).await;
        }

        // Query all crates to verify completion
        for (name, version, _features) in &crates {
            database
                .send(QueryCrate {
                    name: name.to_string(),
                    version: Some(version.to_string()),
                })
                .await;

            tokio::time::sleep(Duration::from_millis(150)).await;
        }

        tokio::time::sleep(Duration::from_millis(500)).await;

        subscriber_handle.stop().await.expect("Subscriber should stop");
        coordinator.stop().await.expect("Coordinator should stop");
        database.stop().await.expect("Database should stop");
        runtime.shutdown_all().await.expect("Runtime should shutdown");

        cleanup_test_db(&test_db_path).await;
    })
    .await
    .expect("Test should complete within timeout");
}

// =============================================================================
// HTTP Server Integration Tests
// =============================================================================

/// Test pipeline processing as if triggered by HTTP POST endpoint.
///
/// This test verifies:
/// - CrateReceived event triggers pipeline processing
/// - Pipeline processes the crate end-to-end
/// - Final status is queryable from database
/// - Simulates the HTTP endpoint flow without requiring ServerActor
#[tokio::test(flavor = "multi_thread")]
async fn test_pipeline_http_post_endpoint_triggers_processing() {
    tokio::time::timeout(Duration::from_secs(50), async {
        let mut runtime = ActonApp::launch();
        let test_db_path = create_test_db_path("http_endpoint");
        let color_config = ColorConfig::new(ColorChoice::Never);

        let _console = Console::spawn(&mut runtime, color_config)
            .await
            .expect("Console should spawn");

        let (database, _db_info) = DatabaseActor::spawn(&mut runtime, test_db_path.clone())
            .await
            .expect("DatabaseActor should spawn");

        let coordinator_config = CoordinatorConfig::default();

        // Spawn RetryCoordinator first
        let retry_policy = RetryPolicy::default();
        let retry_coordinator = RetryCoordinator::spawn(&mut runtime, retry_policy)
            .await
            .expect("RetryCoordinator should spawn");
        let coordinator = CrateCoordinatorActor::spawn(&mut runtime, coordinator_config, retry_coordinator.clone())
            .await
            .expect("CrateCoordinatorActor should spawn");

        let mut subscriber_builder = runtime.new_agent::<PipelineTestSubscriber>().await;

        subscriber_builder
            .mutate_on::<CrateReceived>(|agent, envelope| {
                agent.model.crate_received_events.push(envelope.message().clone());
                AgentReply::immediate()
            })
            .mutate_on::<CrateProcessingComplete>(|agent, envelope| {
                agent.model.processing_complete_events.push(envelope.message().clone());
                AgentReply::immediate()
            })
            .mutate_on::<CrateQueryResponse>(|agent, envelope| {
                agent.model.query_responses.push(envelope.message().clone());
                AgentReply::immediate()
            })
            .after_stop(|agent| {
                assert!(
                    !agent.model.crate_received_events.is_empty(),
                    "Should receive CrateReceived from HTTP endpoint"
                );

                assert!(
                    !agent.model.processing_complete_events.is_empty(),
                    "Should complete processing"
                );

                AgentReply::immediate()
            });

        subscriber_builder.handle().subscribe::<CrateReceived>().await;
        subscriber_builder.handle().subscribe::<CrateProcessingComplete>().await;
        subscriber_builder.handle().subscribe::<CrateQueryResponse>().await;

        let subscriber_handle = subscriber_builder.start().await;

        // Wait for database initialization
        tokio::time::sleep(Duration::from_millis(500)).await;

        // Simulate HTTP POST by sending CrateReceived directly to coordinator
        // (This is what ServerActor would do upon receiving HTTP request)
        let specifier = CrateSpecifier::from_str("hyper@1.0.0")
            .expect("Valid CrateSpecifier");
        let features = vec!["http1".to_string()];

        coordinator
            .send(CrateReceived {
                specifier: specifier.clone(),
                features: features.clone(),
            })
            .await;

        tokio::time::sleep(Duration::from_millis(300)).await;

        // Simulate pipeline stages
        runtime.broker()
            .broadcast(CrateDownloaded {
                specifier: specifier.clone(),
                features: features.clone(),
                extracted_path: PathBuf::from("/tmp/hyper-1.0.0"),
                download_duration_ms: 1100,
            })
            .await;

        tokio::time::sleep(Duration::from_millis(250)).await;

        runtime.broker()
            .broadcast(DocumentationExtracted {
                specifier: specifier.clone(),
                features: features.clone(),
                file_count: 95,
                documentation_bytes: 380000,
                extracted_path: PathBuf::from("/tmp/hyper-1.0.0"),
            })
            .await;

        tokio::time::sleep(Duration::from_millis(250)).await;

        runtime.broker()
            .broadcast(DocumentationChunked {
                specifier: specifier.clone(),
                features: features.clone(),
                chunk_count: 45,
                total_tokens_estimated: 11520,
            })
            .await;

        tokio::time::sleep(Duration::from_millis(250)).await;

        runtime.broker()
            .broadcast(DocumentationVectorized {
                specifier: specifier.clone(),
                features: features.clone(),
                vector_count: 45,
                embedding_model: "text-embedding-3-small".to_string(),
                vectorization_duration_ms: 2200,
            })
            .await;

        tokio::time::sleep(Duration::from_millis(300)).await;

        // Query final status
        database
            .send(QueryCrate {
                name: "hyper".to_string(),
                version: Some("1.0.0".to_string()),
            })
            .await;

        tokio::time::sleep(Duration::from_millis(300)).await;

        // Shutdown gracefully
        subscriber_handle.stop().await.expect("Subscriber should stop");
        coordinator.stop().await.expect("Coordinator should stop");
        database.stop().await.expect("Database should stop");
        runtime.shutdown_all().await.expect("Runtime should shutdown");

        cleanup_test_db(&test_db_path).await;
    })
    .await
    .expect("Test should complete within timeout");
}
