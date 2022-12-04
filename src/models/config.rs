use serde::{Deserialize, Serialize};

#[derive(Debug)]
#[derive(Serialize, Deserialize)]
#[derive(Clone)]
pub struct AppConfig {
    pub start_date: Option<String>,
    pub num_days: u64,
    pub api_config: ApiConfig,
    pub database: DatabaseConfig,
    pub logging: LoggingConfig,
}

#[derive(Debug)]
#[derive(Serialize, Deserialize)]
#[derive(Clone)]
pub struct ApiConfig {
    pub zkill_history_url: String,
    pub zkill_details_url: String,
    pub ccp_details_url: String,
    pub user_agent: String,
}

#[derive(Debug)]
#[derive(Serialize, Deserialize)]
#[derive(Clone)]
pub struct DatabaseConfig {
    pub conn_string: String,
    pub database_name: String,
    pub collection_name: String,
}

#[derive(Debug)]
#[derive(Serialize, Deserialize)]
#[derive(Clone)]
pub struct LoggingConfig {
    pub dir: String,
    pub active_file: String,
    pub archive_pattern: String,
}