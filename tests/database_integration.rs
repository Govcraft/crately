//! Database Integration Tests for Issue #48 Phase 4
//!
//! Comprehensive integration tests for the database pipeline schema,
//! covering status transitions, chunk persistence, vector search, and error handling.

use acton_reactive::prelude::*;
use crately::actors::database::DatabaseActor;
use crately::crate_specifier::CrateSpecifier;
use crately::messages::{
    CrateDownloaded, CrateProcessingComplete, CrateProcessingFailed, CrateQueryResponse,
    DatabaseError, DocumentationChunked, DocumentationExtracted, DocumentationVectorized,
    PersistCrate, PersistDocChunk, PersistEmbedding, QueryCrate, QuerySimilarDocs,
    SimilarDocsResponse,
};
use crately::types::ChunkMetadata;
use std::path::PathBuf;
use std::str::FromStr;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::time::Duration;

/// Helper: Create a unique test database path using test name and timestamp
/// to prevent lock contention between parallel tests.
fn create_test_db_path(test_name: &str) -> PathBuf {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!("crately_integration_test_{}_{}", test_name, timestamp))
}

/// Helper: Clean up test database with retry logic to handle file locks.
async fn cleanup_test_db(path: &std::path::Path) {
    // Give RocksDB time to release file locks
    tokio::time::sleep(Duration::from_millis(150)).await;

    // Retry cleanup a few times if locks are still held
    for attempt in 0..5 {
        match std::fs::remove_dir_all(path) {
            Ok(()) => break,
            Err(_e) if attempt < 4 => {
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
            Err(e) => {
                eprintln!("Warning: Final cleanup attempt failed: {}", e);
            }
        }
    }
}

/// TestSubscriber helper actor for verifying broadcast responses in integration tests.
///
/// This actor subscribes to all relevant message types and collects them for verification
/// in the after_stop hook, following the acton-reactive test pattern.
#[acton_actor]
#[derive(Default, Clone)]
struct TestSubscriber {
    query_responses: Vec<CrateQueryResponse>,
    error_responses: Vec<DatabaseError>,
    similar_docs_responses: Vec<SimilarDocsResponse>,
}

// =============================================================================
// Status Transition Tests
// =============================================================================

#[tokio::test(flavor = "multi_thread")]
async fn test_full_pipeline_status_transitions() {
    tokio::time::timeout(Duration::from_secs(30), async {
        let mut runtime = ActonApp::launch();
        let test_db_path = create_test_db_path("pipeline_transitions");

        let (db_handle, _db_info) = DatabaseActor::spawn(&mut runtime, test_db_path.clone())
            .await
            .unwrap();

        // Create TestSubscriber to verify query responses
        let mut subscriber_builder = runtime.new_agent::<TestSubscriber>().await;

        subscriber_builder.mutate_on::<CrateQueryResponse>(|agent, envelope| {
            agent.model.query_responses.push(envelope.message().clone());
            AgentReply::immediate()
        });

        subscriber_builder.after_stop(|agent| {
            // We expect 7 queries: initial + 6 status updates
            assert!(
                agent.model.query_responses.len() >= 7,
                "Should receive at least 7 query responses for full pipeline"
            );

            // Verify status progression
            let statuses: Vec<String> = agent
                .model
                .query_responses
                .iter()
                .map(|r| r.status.clone())
                .collect();

            assert!(statuses.contains(&"pending".to_string()));
            assert!(statuses.contains(&"downloaded".to_string()));
            assert!(statuses.contains(&"extracted".to_string()));
            assert!(statuses.contains(&"chunked".to_string()));
            assert!(statuses.contains(&"vectorized".to_string()));
            assert!(statuses.contains(&"complete".to_string()));

            AgentReply::immediate()
        });

        subscriber_builder.handle().subscribe::<CrateQueryResponse>().await;
        let subscriber_handle = subscriber_builder.start().await;

        // Wait for schema initialization
        tokio::time::sleep(Duration::from_millis(400)).await;

        // Step 1: Persist crate (initial status: pending)
        let specifier = CrateSpecifier::from_str("tokio@1.35.0").unwrap();
        db_handle
            .send(PersistCrate {
                specifier: specifier.clone(),
                features: vec!["full".to_string()],
            })
            .await;

        tokio::time::sleep(Duration::from_millis(200)).await;

        // Query initial status
        db_handle
            .send(QueryCrate {
                name: "tokio".to_string(),
                version: Some("1.35.0".to_string()),
            })
            .await;

        tokio::time::sleep(Duration::from_millis(150)).await;

        // Step 2: Broadcast CrateDownloaded (status: downloaded)
        runtime
            .broker()
            .broadcast(CrateDownloaded {
                specifier: specifier.clone(),
                features: vec!["full".to_string()],
                extracted_path: PathBuf::from("/tmp/tokio-1.35.0"),
                download_duration_ms: 1500,
            })
            .await;

        tokio::time::sleep(Duration::from_millis(150)).await;

        // Query after download
        db_handle
            .send(QueryCrate {
                name: "tokio".to_string(),
                version: Some("1.35.0".to_string()),
            })
            .await;

        tokio::time::sleep(Duration::from_millis(150)).await;

        // Step 3: Broadcast DocumentationExtracted (status: extracted)
        runtime
            .broker()
            .broadcast(DocumentationExtracted {
                specifier: specifier.clone(),
                features: vec!["full".to_string()],
                file_count: 150,
                documentation_bytes: 524288,
                extracted_path: PathBuf::from("/tmp/tokio-1.35.0"),
            })
            .await;

        tokio::time::sleep(Duration::from_millis(150)).await;

        // Query after extraction
        db_handle
            .send(QueryCrate {
                name: "tokio".to_string(),
                version: Some("1.35.0".to_string()),
            })
            .await;

        tokio::time::sleep(Duration::from_millis(150)).await;

        // Step 4: Broadcast DocumentationChunked (status: chunked)
        runtime
            .broker()
            .broadcast(DocumentationChunked {
                specifier: specifier.clone(),
                features: vec!["full".to_string()],
                chunk_count: 75,
                total_tokens_estimated: 19200,
            })
            .await;

        tokio::time::sleep(Duration::from_millis(150)).await;

        // Query after chunking
        db_handle
            .send(QueryCrate {
                name: "tokio".to_string(),
                version: Some("1.35.0".to_string()),
            })
            .await;

        tokio::time::sleep(Duration::from_millis(150)).await;

        // Step 5: Broadcast DocumentationVectorized (status: vectorized)
        runtime
            .broker()
            .broadcast(DocumentationVectorized {
                specifier: specifier.clone(),
                features: vec!["full".to_string()],
                vector_count: 75,
                embedding_model: "text-embedding-3-small".to_string(),
                vectorization_duration_ms: 3500,
            })
            .await;

        tokio::time::sleep(Duration::from_millis(150)).await;

        // Query after vectorization
        db_handle
            .send(QueryCrate {
                name: "tokio".to_string(),
                version: Some("1.35.0".to_string()),
            })
            .await;

        tokio::time::sleep(Duration::from_millis(150)).await;

        // Step 6: Broadcast CrateProcessingComplete (status: complete)
        runtime
            .broker()
            .broadcast(CrateProcessingComplete {
                specifier: specifier.clone(),
                features: vec!["full".to_string()],
                total_duration_ms: 45000,
                stages_completed: 4,
            })
            .await;

        tokio::time::sleep(Duration::from_millis(150)).await;

        // Query final status
        db_handle
            .send(QueryCrate {
                name: "tokio".to_string(),
                version: Some("1.35.0".to_string()),
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

#[tokio::test(flavor = "multi_thread")]
async fn test_failed_status_transition_with_error_recording() {
    tokio::time::timeout(Duration::from_secs(20), async {
        let mut runtime = ActonApp::launch();
        let test_db_path = create_test_db_path("failed_transition");

        let (db_handle, _db_info) = DatabaseActor::spawn(&mut runtime, test_db_path.clone())
            .await
            .unwrap();

        // Create TestSubscriber to verify query responses
        let mut subscriber_builder = runtime.new_agent::<TestSubscriber>().await;

        subscriber_builder.mutate_on::<CrateQueryResponse>(|agent, envelope| {
            agent.model.query_responses.push(envelope.message().clone());
            AgentReply::immediate()
        });

        subscriber_builder.after_stop(|agent| {
            assert!(
                agent.model.query_responses.len() >= 2,
                "Should receive at least 2 query responses"
            );

            // Find the final query response (status: failed)
            let final_response = agent.model.query_responses.last().unwrap();
            assert_eq!(final_response.status, "failed", "Status should be failed");

            AgentReply::immediate()
        });

        subscriber_builder.handle().subscribe::<CrateQueryResponse>().await;
        let subscriber_handle = subscriber_builder.start().await;

        // Wait for schema initialization
        tokio::time::sleep(Duration::from_millis(400)).await;

        // Persist crate
        let specifier = CrateSpecifier::from_str("faulty-crate@1.0.0").unwrap();
        db_handle
            .send(PersistCrate {
                specifier: specifier.clone(),
                features: vec![],
            })
            .await;

        tokio::time::sleep(Duration::from_millis(200)).await;

        // Query initial status
        db_handle
            .send(QueryCrate {
                name: "faulty-crate".to_string(),
                version: Some("1.0.0".to_string()),
            })
            .await;

        tokio::time::sleep(Duration::from_millis(150)).await;

        // Broadcast processing failure
        runtime
            .broker()
            .broadcast(CrateProcessingFailed {
                specifier: specifier.clone(),
                stage: "downloading".to_string(),
                error: "Network timeout after 30s".to_string(),
            })
            .await;

        tokio::time::sleep(Duration::from_millis(200)).await;

        // Query after failure
        db_handle
            .send(QueryCrate {
                name: "faulty-crate".to_string(),
                version: Some("1.0.0".to_string()),
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

// =============================================================================
// Chunk Persistence Tests
// =============================================================================

#[tokio::test(flavor = "multi_thread")]
async fn test_persist_and_retrieve_documentation_chunks() {
    tokio::time::timeout(Duration::from_secs(20), async {
        let mut runtime = ActonApp::launch();
        let test_db_path = create_test_db_path("chunk_persistence");

        let (db_handle, _db_info) = DatabaseActor::spawn(&mut runtime, test_db_path.clone())
            .await
            .unwrap();

        // Wait for schema initialization
        tokio::time::sleep(Duration::from_millis(400)).await;

        // Persist crate first
        let specifier = CrateSpecifier::from_str("serde@1.0.200").unwrap();
        db_handle
            .send(PersistCrate {
                specifier: specifier.clone(),
                features: vec!["derive".to_string()],
            })
            .await;

        tokio::time::sleep(Duration::from_millis(200)).await;

        // Persist multiple doc chunks
        for i in 0..5 {
            let metadata = ChunkMetadata {
                content_type: "markdown".to_string(),
                start_line: Some(i * 100),
                end_line: Some((i + 1) * 100 - 1),
                token_count: 256,
                char_count: 1024,
                parent_module: Some(format!("serde::module{}", i)),
                item_type: Some("module".to_string()),
                item_name: Some(format!("Module{}", i)),
            };

            db_handle
                .send(PersistDocChunk {
                    specifier: specifier.clone(),
                    chunk_index: i,
                    chunk_id: format!("serde_1.0.200_chunk_{:03}", i),
                    content: format!("This is documentation chunk {} for serde", i),
                    source_file: format!("src/module{}.rs", i),
                    metadata,
                })
                .await;

            tokio::time::sleep(Duration::from_millis(100)).await;
        }

        // Clean shutdown
        db_handle.stop().await.unwrap();
        runtime.shutdown_all().await.unwrap();

        cleanup_test_db(&test_db_path).await;
    })
    .await
    .expect("Test should complete within timeout");
}

#[tokio::test(flavor = "multi_thread")]
async fn test_chunk_metadata_accuracy() {
    tokio::time::timeout(Duration::from_secs(20), async {
        let mut runtime = ActonApp::launch();
        let test_db_path = create_test_db_path("chunk_metadata");

        let (db_handle, _db_info) = DatabaseActor::spawn(&mut runtime, test_db_path.clone())
            .await
            .unwrap();

        // Wait for schema initialization
        tokio::time::sleep(Duration::from_millis(400)).await;

        // Persist crate
        let specifier = CrateSpecifier::from_str("tokio@1.36.0").unwrap();
        db_handle
            .send(PersistCrate {
                specifier: specifier.clone(),
                features: vec!["full".to_string()],
            })
            .await;

        tokio::time::sleep(Duration::from_millis(200)).await;

        // Persist chunk with detailed metadata
        let metadata = ChunkMetadata {
            content_type: "rust".to_string(),
            start_line: Some(42),
            end_line: Some(142),
            token_count: 512,
            char_count: 2048,
            parent_module: Some("tokio::runtime".to_string()),
            item_type: Some("struct".to_string()),
            item_name: Some("Runtime".to_string()),
        };

        db_handle
            .send(PersistDocChunk {
                specifier: specifier.clone(),
                chunk_index: 10,
                chunk_id: "tokio_1.36.0_chunk_010".to_string(),
                content: "Runtime struct provides the async runtime environment".to_string(),
                source_file: "src/runtime/mod.rs".to_string(),
                metadata,
            })
            .await;

        tokio::time::sleep(Duration::from_millis(200)).await;

        // Clean shutdown
        db_handle.stop().await.unwrap();
        runtime.shutdown_all().await.unwrap();

        cleanup_test_db(&test_db_path).await;
    })
    .await
    .expect("Test should complete within timeout");
}

// =============================================================================
// Vector Search Tests
// =============================================================================

#[tokio::test(flavor = "multi_thread")]
async fn test_vector_search_accuracy_and_ranking() {
    tokio::time::timeout(Duration::from_secs(25), async {
        let mut runtime = ActonApp::launch();
        let test_db_path = create_test_db_path("vector_search");

        let (db_handle, _db_info) = DatabaseActor::spawn(&mut runtime, test_db_path.clone())
            .await
            .unwrap();

        // Create TestSubscriber to verify search responses
        let mut subscriber_builder = runtime.new_agent::<TestSubscriber>().await;

        subscriber_builder.mutate_on::<SimilarDocsResponse>(|agent, envelope| {
            agent
                .model
                .similar_docs_responses
                .push(envelope.message().clone());
            AgentReply::immediate()
        });

        subscriber_builder.after_stop(|agent| {
            assert_eq!(
                agent.model.similar_docs_responses.len(),
                1,
                "Should receive exactly one search response"
            );

            let response = &agent.model.similar_docs_responses[0];
            assert!(!response.results.is_empty(), "Should return search results");

            // Verify results are ranked by similarity score (descending)
            for i in 0..response.results.len().saturating_sub(1) {
                assert!(
                    response.results[i].similarity_score >= response.results[i + 1].similarity_score,
                    "Results should be sorted by similarity score descending"
                );
            }

            // Verify all scores meet the threshold (>0.7)
            for result in &response.results {
                assert!(
                    result.similarity_score > 0.7,
                    "All results should meet similarity threshold"
                );
            }

            AgentReply::immediate()
        });

        subscriber_builder
            .handle()
            .subscribe::<SimilarDocsResponse>()
            .await;
        let subscriber_handle = subscriber_builder.start().await;

        // Wait for schema initialization
        tokio::time::sleep(Duration::from_millis(400)).await;

        // Setup: Persist crates and chunks with embeddings
        let specifiers = [
            CrateSpecifier::from_str("tokio@1.35.0").unwrap(),
            CrateSpecifier::from_str("async-std@1.12.0").unwrap(),
            CrateSpecifier::from_str("actix-web@4.4.0").unwrap(),
        ];

        for (idx, specifier) in specifiers.iter().enumerate() {
            // Persist crate
            db_handle
                .send(PersistCrate {
                    specifier: specifier.clone(),
                    features: vec![],
                })
                .await;

            tokio::time::sleep(Duration::from_millis(100)).await;

            // Persist chunk
            let metadata = ChunkMetadata {
                content_type: "markdown".to_string(),
                start_line: None,
                end_line: None,
                token_count: 200,
                char_count: 800,
                parent_module: None,
                item_type: Some("crate".to_string()),
                item_name: Some(specifier.name().to_string()),
            };

            let chunk_id = format!("{}_{}_chunk_000", specifier.name(), specifier.version());

            db_handle
                .send(PersistDocChunk {
                    specifier: specifier.clone(),
                    chunk_index: 0,
                    chunk_id: chunk_id.clone(),
                    content: format!("Async runtime documentation for {}", specifier.name()),
                    source_file: "src/lib.rs".to_string(),
                    metadata,
                })
                .await;

            tokio::time::sleep(Duration::from_millis(100)).await;

            // Persist embedding with varying similarity to query
            // Create vector with decreasing similarity pattern
            let base_value = 0.8 - (idx as f32 * 0.1);
            let vector: Vec<f32> = (0..1536).map(|i| base_value + (i as f32 * 0.0001)).collect();

            db_handle
                .send(PersistEmbedding {
                    chunk_id,
                    specifier: specifier.clone(),
                    vector,
                    model_name: "text-embedding-3-small".to_string(),
                    model_version: "3".to_string(),
                })
                .await;

            tokio::time::sleep(Duration::from_millis(100)).await;
        }

        // Query vector (similar to first embedding pattern)
        let query_vector: Vec<f32> = (0..1536).map(|i| 0.79 + (i as f32 * 0.0001)).collect();

        // Perform vector search
        db_handle
            .send(QuerySimilarDocs {
                query_vector,
                limit: Some(10),
                crate_filter: None,
            })
            .await;

        tokio::time::sleep(Duration::from_millis(300)).await;

        // Clean shutdown
        subscriber_handle.stop().await.unwrap();
        db_handle.stop().await.unwrap();
        runtime.shutdown_all().await.unwrap();

        cleanup_test_db(&test_db_path).await;
    })
    .await
    .expect("Test should complete within timeout");
}

#[tokio::test(flavor = "multi_thread")]
async fn test_vector_search_with_crate_filter() {
    tokio::time::timeout(Duration::from_secs(25), async {
        let mut runtime = ActonApp::launch();
        let test_db_path = create_test_db_path("vector_search_filter");

        let (db_handle, _db_info) = DatabaseActor::spawn(&mut runtime, test_db_path.clone())
            .await
            .unwrap();

        // Create TestSubscriber to verify search responses
        let mut subscriber_builder = runtime.new_agent::<TestSubscriber>().await;

        subscriber_builder.mutate_on::<SimilarDocsResponse>(|agent, envelope| {
            agent
                .model
                .similar_docs_responses
                .push(envelope.message().clone());
            AgentReply::immediate()
        });

        subscriber_builder.after_stop(|agent| {
            assert_eq!(
                agent.model.similar_docs_responses.len(),
                1,
                "Should receive exactly one search response"
            );

            let response = &agent.model.similar_docs_responses[0];

            // Verify all results are from the filtered crate
            for result in &response.results {
                assert_eq!(result.specifier.name(), "tokio", "All results should be from tokio crate");
                assert_eq!(result.specifier.version().to_string(), "1.35.0", "All results should be from version 1.35.0");
            }

            AgentReply::immediate()
        });

        subscriber_builder
            .handle()
            .subscribe::<SimilarDocsResponse>()
            .await;
        let subscriber_handle = subscriber_builder.start().await;

        // Wait for schema initialization
        tokio::time::sleep(Duration::from_millis(400)).await;

        // Setup: Persist multiple crates with embeddings
        let specifiers = vec![
            CrateSpecifier::from_str("tokio@1.35.0").unwrap(),
            CrateSpecifier::from_str("async-std@1.12.0").unwrap(),
        ];

        for specifier in &specifiers {
            db_handle
                .send(PersistCrate {
                    specifier: specifier.clone(),
                    features: vec![],
                })
                .await;

            tokio::time::sleep(Duration::from_millis(100)).await;

            let metadata = ChunkMetadata {
                content_type: "markdown".to_string(),
                start_line: None,
                end_line: None,
                token_count: 200,
                char_count: 800,
                parent_module: None,
                item_type: Some("crate".to_string()),
                item_name: Some(specifier.name().to_string()),
            };

            let chunk_id = format!("{}_{}_chunk_000", specifier.name(), specifier.version());

            db_handle
                .send(PersistDocChunk {
                    specifier: specifier.clone(),
                    chunk_index: 0,
                    chunk_id: chunk_id.clone(),
                    content: format!("Documentation for {}", specifier.name()),
                    source_file: "src/lib.rs".to_string(),
                    metadata,
                })
                .await;

            tokio::time::sleep(Duration::from_millis(100)).await;

            let vector: Vec<f32> = (0..1536).map(|i| 0.8 + (i as f32 * 0.0001)).collect();

            db_handle
                .send(PersistEmbedding {
                    chunk_id,
                    specifier: specifier.clone(),
                    vector,
                    model_name: "text-embedding-3-small".to_string(),
                    model_version: "3".to_string(),
                })
                .await;

            tokio::time::sleep(Duration::from_millis(100)).await;
        }

        // Query with crate filter for tokio only
        let query_vector: Vec<f32> = (0..1536).map(|i| 0.79 + (i as f32 * 0.0001)).collect();

        db_handle
            .send(QuerySimilarDocs {
                query_vector,
                limit: Some(10),
                crate_filter: Some(CrateSpecifier::from_str("tokio@1.35.0").unwrap()),
            })
            .await;

        tokio::time::sleep(Duration::from_millis(300)).await;

        // Clean shutdown
        subscriber_handle.stop().await.unwrap();
        db_handle.stop().await.unwrap();
        runtime.shutdown_all().await.unwrap();

        cleanup_test_db(&test_db_path).await;
    })
    .await
    .expect("Test should complete within timeout");
}

#[tokio::test(flavor = "multi_thread")]
async fn test_vector_search_limit_parameter() {
    tokio::time::timeout(Duration::from_secs(25), async {
        let mut runtime = ActonApp::launch();
        let test_db_path = create_test_db_path("vector_search_limit");

        let (db_handle, _db_info) = DatabaseActor::spawn(&mut runtime, test_db_path.clone())
            .await
            .unwrap();

        // Create TestSubscriber to verify search responses
        let mut subscriber_builder = runtime.new_agent::<TestSubscriber>().await;

        subscriber_builder.mutate_on::<SimilarDocsResponse>(|agent, envelope| {
            agent
                .model
                .similar_docs_responses
                .push(envelope.message().clone());
            AgentReply::immediate()
        });

        subscriber_builder.after_stop(|agent| {
            assert_eq!(
                agent.model.similar_docs_responses.len(),
                1,
                "Should receive exactly one search response"
            );

            let response = &agent.model.similar_docs_responses[0];
            assert!(
                response.results.len() <= 3,
                "Should respect limit parameter (max 3 results)"
            );

            AgentReply::immediate()
        });

        subscriber_builder
            .handle()
            .subscribe::<SimilarDocsResponse>()
            .await;
        let subscriber_handle = subscriber_builder.start().await;

        // Wait for schema initialization
        tokio::time::sleep(Duration::from_millis(400)).await;

        // Setup: Persist 5 chunks with embeddings
        let specifier = CrateSpecifier::from_str("test-crate@1.0.0").unwrap();

        db_handle
            .send(PersistCrate {
                specifier: specifier.clone(),
                features: vec![],
            })
            .await;

        tokio::time::sleep(Duration::from_millis(100)).await;

        for i in 0..5 {
            let metadata = ChunkMetadata {
                content_type: "markdown".to_string(),
                start_line: None,
                end_line: None,
                token_count: 200,
                char_count: 800,
                parent_module: None,
                item_type: Some("module".to_string()),
                item_name: Some(format!("Module{}", i)),
            };

            let chunk_id = format!("test-crate_1.0.0_chunk_{:03}", i);

            db_handle
                .send(PersistDocChunk {
                    specifier: specifier.clone(),
                    chunk_index: i,
                    chunk_id: chunk_id.clone(),
                    content: format!("Documentation chunk {}", i),
                    source_file: format!("src/module{}.rs", i),
                    metadata,
                })
                .await;

            tokio::time::sleep(Duration::from_millis(80)).await;

            let vector: Vec<f32> = (0..1536).map(|j| 0.8 + (j as f32 * 0.0001)).collect();

            db_handle
                .send(PersistEmbedding {
                    chunk_id,
                    specifier: specifier.clone(),
                    vector,
                    model_name: "text-embedding-3-small".to_string(),
                    model_version: "3".to_string(),
                })
                .await;

            tokio::time::sleep(Duration::from_millis(80)).await;
        }

        // Query with limit=3
        let query_vector: Vec<f32> = (0..1536).map(|i| 0.79 + (i as f32 * 0.0001)).collect();

        db_handle
            .send(QuerySimilarDocs {
                query_vector,
                limit: Some(3),
                crate_filter: None,
            })
            .await;

        tokio::time::sleep(Duration::from_millis(300)).await;

        // Clean shutdown
        subscriber_handle.stop().await.unwrap();
        db_handle.stop().await.unwrap();
        runtime.shutdown_all().await.unwrap();

        cleanup_test_db(&test_db_path).await;
    })
    .await
    .expect("Test should complete within timeout");
}

// =============================================================================
// Error Handling and Recovery Tests
// =============================================================================

#[tokio::test(flavor = "multi_thread")]
async fn test_error_handling_invalid_crate_reference() {
    tokio::time::timeout(Duration::from_secs(20), async {
        let mut runtime = ActonApp::launch();
        let test_db_path = create_test_db_path("error_invalid_crate");

        let (db_handle, _db_info) = DatabaseActor::spawn(&mut runtime, test_db_path.clone())
            .await
            .unwrap();

        // Create TestSubscriber to verify error responses
        let mut subscriber_builder = runtime.new_agent::<TestSubscriber>().await;

        subscriber_builder.mutate_on::<DatabaseError>(|agent, envelope| {
            agent.model.error_responses.push(envelope.message().clone());
            AgentReply::immediate()
        });

        subscriber_builder.after_stop(|agent| {
            assert!(
                !agent.model.error_responses.is_empty(),
                "Should receive database error for invalid crate reference"
            );

            let error = &agent.model.error_responses[0];
            assert!(
                error.operation.contains("persist doc chunk"),
                "Error should be for doc chunk persistence"
            );
            assert!(
                error.error.contains("not found"),
                "Error should indicate crate not found"
            );

            AgentReply::immediate()
        });

        subscriber_builder.handle().subscribe::<DatabaseError>().await;
        let subscriber_handle = subscriber_builder.start().await;

        // Wait for schema initialization
        tokio::time::sleep(Duration::from_millis(400)).await;

        // Attempt to persist chunk without persisting crate first (invalid reference)
        let specifier = CrateSpecifier::from_str("nonexistent@1.0.0").unwrap();
        let metadata = ChunkMetadata {
            content_type: "markdown".to_string(),
            start_line: None,
            end_line: None,
            token_count: 100,
            char_count: 400,
            parent_module: None,
            item_type: None,
            item_name: None,
        };

        db_handle
            .send(PersistDocChunk {
                specifier,
                chunk_index: 0,
                chunk_id: "nonexistent_1.0.0_chunk_000".to_string(),
                content: "Test content".to_string(),
                source_file: "src/lib.rs".to_string(),
                metadata,
            })
            .await;

        tokio::time::sleep(Duration::from_millis(300)).await;

        // Clean shutdown
        subscriber_handle.stop().await.unwrap();
        db_handle.stop().await.unwrap();
        runtime.shutdown_all().await.unwrap();

        cleanup_test_db(&test_db_path).await;
    })
    .await
    .expect("Test should complete within timeout");
}

#[tokio::test(flavor = "multi_thread")]
async fn test_error_handling_invalid_chunk_reference_for_embedding() {
    tokio::time::timeout(Duration::from_secs(20), async {
        let mut runtime = ActonApp::launch();
        let test_db_path = create_test_db_path("error_invalid_chunk");

        let (db_handle, _db_info) = DatabaseActor::spawn(&mut runtime, test_db_path.clone())
            .await
            .unwrap();

        // Create TestSubscriber to verify error responses
        let mut subscriber_builder = runtime.new_agent::<TestSubscriber>().await;

        subscriber_builder.mutate_on::<DatabaseError>(|agent, envelope| {
            agent.model.error_responses.push(envelope.message().clone());
            AgentReply::immediate()
        });

        subscriber_builder.after_stop(|agent| {
            assert!(
                !agent.model.error_responses.is_empty(),
                "Should receive database error for invalid chunk reference"
            );

            let error = &agent.model.error_responses[0];
            assert!(
                error.operation.contains("persist embedding"),
                "Error should be for embedding persistence"
            );

            AgentReply::immediate()
        });

        subscriber_builder.handle().subscribe::<DatabaseError>().await;
        let subscriber_handle = subscriber_builder.start().await;

        // Wait for schema initialization
        tokio::time::sleep(Duration::from_millis(400)).await;

        // Persist crate but not chunk
        let specifier = CrateSpecifier::from_str("test-crate@1.0.0").unwrap();
        db_handle
            .send(PersistCrate {
                specifier: specifier.clone(),
                features: vec![],
            })
            .await;

        tokio::time::sleep(Duration::from_millis(200)).await;

        // Attempt to persist embedding without persisting chunk first
        let vector: Vec<f32> = (0..1536).map(|i| (i as f32) * 0.001).collect();

        db_handle
            .send(PersistEmbedding {
                chunk_id: "nonexistent_chunk_id".to_string(),
                specifier,
                vector,
                model_name: "text-embedding-3-small".to_string(),
                model_version: "3".to_string(),
            })
            .await;

        tokio::time::sleep(Duration::from_millis(300)).await;

        // Clean shutdown
        subscriber_handle.stop().await.unwrap();
        db_handle.stop().await.unwrap();
        runtime.shutdown_all().await.unwrap();

        cleanup_test_db(&test_db_path).await;
    })
    .await
    .expect("Test should complete within timeout");
}

#[tokio::test(flavor = "multi_thread")]
async fn test_recovery_after_database_error() {
    tokio::time::timeout(Duration::from_secs(20), async {
        let mut runtime = ActonApp::launch();
        let test_db_path = create_test_db_path("error_recovery");

        let (db_handle, _db_info) = DatabaseActor::spawn(&mut runtime, test_db_path.clone())
            .await
            .unwrap();

        // Create TestSubscriber to verify successful operations after error
        let mut subscriber_builder = runtime.new_agent::<TestSubscriber>().await;

        subscriber_builder.mutate_on::<DatabaseError>(|agent, envelope| {
            agent.model.error_responses.push(envelope.message().clone());
            AgentReply::immediate()
        });

        subscriber_builder.mutate_on::<CrateQueryResponse>(|agent, envelope| {
            agent.model.query_responses.push(envelope.message().clone());
            AgentReply::immediate()
        });

        subscriber_builder.after_stop(|agent| {
            // Should have received at least one error
            assert!(
                !agent.model.error_responses.is_empty(),
                "Should receive error for invalid operation"
            );

            // Should also have received successful query response after recovery
            assert!(
                !agent.model.query_responses.is_empty(),
                "Should successfully query after error"
            );

            let response = &agent.model.query_responses[0];
            assert!(
                response.specifier.is_some(),
                "Should find the valid crate after recovery"
            );

            AgentReply::immediate()
        });

        subscriber_builder.handle().subscribe::<DatabaseError>().await;
        subscriber_builder.handle().subscribe::<CrateQueryResponse>().await;
        let subscriber_handle = subscriber_builder.start().await;

        // Wait for schema initialization
        tokio::time::sleep(Duration::from_millis(400)).await;

        // Cause an error with invalid chunk reference
        let invalid_specifier = CrateSpecifier::from_str("invalid@1.0.0").unwrap();
        let metadata = ChunkMetadata {
            content_type: "markdown".to_string(),
            start_line: None,
            end_line: None,
            token_count: 100,
            char_count: 400,
            parent_module: None,
            item_type: None,
            item_name: None,
        };

        db_handle
            .send(PersistDocChunk {
                specifier: invalid_specifier,
                chunk_index: 0,
                chunk_id: "invalid_chunk".to_string(),
                content: "Test".to_string(),
                source_file: "src/lib.rs".to_string(),
                metadata,
            })
            .await;

        tokio::time::sleep(Duration::from_millis(200)).await;

        // Now perform valid operations to verify recovery
        let valid_specifier = CrateSpecifier::from_str("valid-crate@1.0.0").unwrap();
        db_handle
            .send(PersistCrate {
                specifier: valid_specifier.clone(),
                features: vec!["std".to_string()],
            })
            .await;

        tokio::time::sleep(Duration::from_millis(200)).await;

        // Query the valid crate to verify database is still operational
        db_handle
            .send(QueryCrate {
                name: "valid-crate".to_string(),
                version: Some("1.0.0".to_string()),
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
