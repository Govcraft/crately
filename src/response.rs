use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct CrateResponse {
    pub name: String,
    pub version: String,
    pub features: Vec<String>,
    pub message: String,
}
