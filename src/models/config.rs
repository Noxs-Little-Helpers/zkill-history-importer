use serde::{Deserialize, Serialize};

#[derive(Debug)]
#[derive(Serialize, Deserialize)]
#[derive(Clone)]
pub struct AppConfig {
    pub num_days: u64,
    pub start_date: Option<String>,
    pub schedule_config: Option<ScheduleConfig>,
    pub api_config: ApiConfig,
    pub database_config: DatabaseConfig,
    pub logging_config: LoggingConfig,
}

#[derive(Debug)]
#[derive(Serialize, Deserialize)]
#[derive(Clone)]
pub struct ScheduleConfig {
    pub hours_to_wait: u64,
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