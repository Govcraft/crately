//! Simple utility to query the crately database
//!
//! Run with: cargo run --example query_db

use surrealdb::engine::local::RocksDb;
use surrealdb::sql::Thing;
use surrealdb::Surreal;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Get database path
    let xdg_dirs = xdg::BaseDirectories::with_prefix("crately");
    let db_path = xdg_dirs
        .create_data_directory("db")
        .expect("Failed to get database path");

    println!("Connecting to database: {}", db_path.display());

    // Connect to database
    let db = Surreal::new::<RocksDb>(db_path).await?;

    // Use namespace and database
    db.use_ns("crately").use_db("production").await?;

    println!("\n=== Querying all crates ===\n");

    // Query all crates - use a more specific struct instead of serde_json::Value
    #[derive(Debug, serde::Deserialize)]
    struct CrateRecord {
        id: Thing,
        name: String,
        version: String,
        features: Vec<String>,
        status: String,
        created_at: Option<String>,
    }

    let mut result = db.query("SELECT * FROM crate").await?;
    let crates: Vec<CrateRecord> = result.take(0)?;

    if crates.is_empty() {
        println!("No crates found in database.");
    } else {
        println!("Found {} crate(s):\n", crates.len());

        for crate_record in &crates {
            println!("  {}@{}", crate_record.name, crate_record.version);
            println!("    ID: {}", crate_record.id);
            println!("    Status: {}", crate_record.status);
            if !crate_record.features.is_empty() {
                println!("    Features: {}", crate_record.features.join(", "));
            }
            if let Some(created_at) = &crate_record.created_at {
                println!("    Created: {}", created_at);
            }
            println!();
        }
    }

    // Display total
    println!("Total crates: {}", crates.len());

    println!("\n=== Querying embeddings ===\n");

    // Query all embeddings - use a specific struct
    #[derive(Debug, serde::Deserialize)]
    struct EmbeddingRecord {
        id: Thing,
        chunk_id: String,
        crate_id: Thing,
        vector_dimension: i32,
        model_name: String,
        model_version: String,
        created_at: Option<String>,
    }

    let mut result = db
        .query("SELECT id, chunk_id, crate_id, vector_dimension, model_name, model_version, created_at FROM embedding")
        .await?;
    let embeddings: Vec<EmbeddingRecord> = result.take(0)?;

    if embeddings.is_empty() {
        println!("No embeddings found in database.");
    } else {
        println!("Found {} embedding(s):\n", embeddings.len());

        for embedding in &embeddings {
            println!("  Embedding ID: {}", embedding.id);
            println!("    Chunk: {}", embedding.chunk_id);
            println!("    Crate: {}", embedding.crate_id);
            println!("    Model: {} (v{})", embedding.model_name, embedding.model_version);
            println!("    Dimensions: {}", embedding.vector_dimension);
            if let Some(created_at) = &embedding.created_at {
                println!("    Created: {}", created_at);
            }
            println!();
        }

        // Display statistics
        println!("Total embeddings: {}", embeddings.len());

        // Count by model
        use std::collections::HashMap;
        let mut model_counts: HashMap<String, usize> = HashMap::new();
        for embedding in &embeddings {
            let model_key = format!("{} v{}", embedding.model_name, embedding.model_version);
            *model_counts.entry(model_key).or_insert(0) += 1;
        }

        if !model_counts.is_empty() {
            println!("\nEmbeddings by model:");
            for (model, count) in model_counts {
                println!("  {}: {}", model, count);
            }
        }
    }

    Ok(())
}
