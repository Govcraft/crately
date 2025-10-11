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

    Ok(())
}
