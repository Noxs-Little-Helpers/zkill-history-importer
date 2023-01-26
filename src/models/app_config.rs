use serde::{Deserialize, Serialize};

#[derive(Debug)]
#[derive(Serialize, Deserialize)]
#[derive(Clone)]
pub struct AppConfig {
    pub num_days: u64,
    pub start_date: Option<StartDateConfig>,
    pub scheduling: Option<ScheduleConfig>,
    pub api: ApiConfig,
    pub database: DatabaseConfig,
    pub logging: Option<LoggingConfig>,
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
pub struct StartDateConfig {
    pub year: i32,
    pub month: u32,
    pub day: u32,
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
    pub logging_level: String,
}