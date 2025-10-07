use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct CrateResponse {
    pub name: String,
    pub version: String,
    pub features: Vec<String>,
    pub message: String,
}
