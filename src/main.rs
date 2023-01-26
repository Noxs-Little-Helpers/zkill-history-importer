mod models;

use models::{
    app_config,
};
use tracing::{error, info, warn, debug, trace, Level};
use tracing_subscriber::FmtSubscriber;

use std::{
    env,
    str::FromStr
};
use std::ops::RangeInclusive;
use mongodb::bson::{Bson, doc, Document};
use mongodb::{bson, Database};
use std::time::{SystemTime};
use chrono::TimeZone;

#[tokio::main]
async fn main() {
    let app_config: &app_config::AppConfig = &load_config_from_file();
    config_logging(match &app_config.logging {
        None => { Level::INFO }
        Some(logging_config) => {
            match Level::from_str(&logging_config.logging_level.as_str()) {
                Err(e) => {
                    panic!("Invalid logging level supplied in config. Supplied value [{0}] Error [{1:?}]", &logging_config.logging_level, e);
                }
                Ok(level) => { level }
            }
        }
    });
    info!("zkill-history-importer started");

    let http_client = reqwest::Client::builder().user_agent(&app_config.api.user_agent).build().unwrap();
    let mut num_iterations: u64 = 0;
    loop {
        let execution_start_time = SystemTime::now();
        let date_to_start_pulling_from = match &app_config.start_date {
            Some(value) => {
                if app_config.scheduling.is_some() {
                    panic!("Config specifies start date while also specifying schedule config.");
                }
                match chrono::Utc.with_ymd_and_hms(value.year, value.month, value.day, 0, 0, 0) {
                    chrono::LocalResult::Single(value) => value,
                    chrono::LocalResult::None => panic!("Cannot parse start date. Are you sure you entered year month and day correctly?"),
                    chrono::LocalResult::Ambiguous(_,_) => panic!("Cannot parse start date. Result is ambiguous"),
                }
            }
            None => { chrono::Utc::now() }
        };
        info!("Pull Start Date [{0}]", &date_to_start_pulling_from);
        let day_list = get_encompassing_days(date_to_start_pulling_from, 0..=app_config.num_days);

        let all_ids = get_killmail_for_days(&day_list, &app_config.api.zkill_history_url, &http_client).await;

        task_upload_to_database_if_missing(&all_ids, &app_config, &http_client).await;

        num_iterations += 1;
        let end_time = SystemTime::now();
        match end_time.duration_since(execution_start_time) {
            Ok(time_elapsed) => { info!("Completed task. Time elapsed [{0:?}]", time_elapsed); }
            Err(_) => { info!("Completed task. Error calculating time elapsed"); }
        };
        match &app_config.scheduling {
            Some(some) => {
                info!("Sleeping for [{0}] hours. Iteration# [{1}]", &some.hours_to_wait, num_iterations);
                tokio::time::sleep(tokio::time::Duration::from_secs(&some.hours_to_wait * 60)).await;
            }
            None => {
                info!("Stopping program");
                break;
            }
        };
    }
}

#[tracing::instrument]
async fn task_upload_to_database_if_missing(killmails: &Vec<(i64, String)>, app_config: &app_config::AppConfig, http_client: &reqwest::Client) {
    let client: mongodb::Client = match get_database_client(&app_config.database.conn_string).await {
        Ok(client) => {
            client
        }
        Err(error) => {
            panic!("Database: Unable to create database client. {0:?}", error);
        }
    };
    let database = client.database(&app_config.database.database_name);
    let collection = database.collection(&app_config.database.collection_name);
    {
        info!("Database: Attempting to connect");
        let mut test_ping_successful = false;
        loop {
            match confirm_database_connection(&database).await {
                Ok(_) => { test_ping_successful = true }
                Err(error) => {
                    error!("Database: Unable to ping. Reattempting... [{0:?}]", error);
                }
            }
            if test_ping_successful {
                info!("Database: Connection established");
                break;
            }
        }
    }
    info!("Found [{0}] kills Going through each kill in list to find missing", killmails.len());
    let mut num_uploaded = 0;
    for (id, hash) in killmails {
        let exists_already = is_in_collection(&id, &collection).await;
        if !exists_already {
            info!("Killmail not present in database. Pulling data from api ID[{0}]", id);
            let mut num_api_attempts: u64 = 0;
            let killmail = loop {
                match get_kill_details(&id, &hash, &app_config.api.zkill_details_url, &app_config.api.ccp_details_url, http_client).await {
                    Ok(output) => { break output; }
                    Err(message) => {
                        // if num_api_attempts > 10 {
                        // continue 'km_loop;
                        // } else {
                        num_api_attempts += 1;
                        error!("Got error getting api info. Sleeping 1 second before trying again. Attempt number [{0}] Error [{1}]", &num_api_attempts, &message);
                        tokio::time::sleep(tokio::time::Duration::from_millis(1000)).await;
                        continue;
                        // }
                    }
                };
            };
            loop {
                match write_bson_to_database(&killmail, &collection).await {
                    Ok(inserted_id) => {
                        info!("Database: Document inserted");
                        debug!("Database: Document inserted ID: [{0}]", inserted_id.inserted_id);
                        num_uploaded += 1;
                        break;
                    }
                    Err(error) => {
                        error!("Database: Got error attempting to write to database message[{0}] [{1:?}]", &killmail, &error);
                    }
                };
            }
            tokio::time::sleep(tokio::time::Duration::from_millis(1000)).await;
        } else {
            info!("Killmail present in database skipping. ID[{0}]", id);
        }
    }
    info!("Task completed. Uploaded {0} documents", num_uploaded);
}

async fn is_in_collection(id: &i64, collection: &mongodb::Collection<Bson>) -> bool {
    return match collection.find_one(doc! {"killmail_id": id}, None).await {
        Ok(result) => {
            match result {
                None => { false }
                Some(_) => { true }
            }
        }
        Err(_) => { false }
    };
}

async fn get_kill_details(id: &i64, hash: &String, zkill_details_url: &String, ccp_details_url: &String, http_client: &reqwest::Client) -> Result<Document, String> {
    let zkill_api_url = format!("{0}{1}/", zkill_details_url, id);
    info!("Making request to {}", &zkill_api_url);
    let zkill_response = match http_client.execute(http_client.get(&zkill_api_url).build().unwrap()).await {
        Ok(response) => {
            match response.text().await {
                Ok(result) => {
                    if result.is_empty() {
                        return Err("zkillboard details: Empty response".to_string());
                    } else {
                        result
                    }
                }
                Err(error) => {
                    return Err(format!("zkillboard details: Could not parse response from api got [{0}]", &error));
                }
            }
        }
        Err(error) => {
            return Err(format!("zKillboard details: Got error sending api request. Url: {0} Error:{1}", &zkill_details_url, &error));
        }
    };
    let zkill_response_value: serde_json::Value = match serde_json::from_str(&zkill_response) {
        Ok(value) => { value }
        Err(error) => {
            return Err(format!("zKillboard details: Response could not be parsed by serde_json [{0}] [{1:?}]", &zkill_response, error));
        }
    };
    let zkill_bson_doc = match bson::to_document(&zkill_response_value.as_array().unwrap()[0]) {
        Ok(value) => { value }
        Err(error) => {
            return Err(format!("zKillboard details: Response could not be parsed by bson [{0}] [{1:?}]", &zkill_response_value, error));
        }
    };
    let ccp_api_url = format!("{0}{1}/{2}/?datasource=tranquility", ccp_details_url, id, hash);
    info!("Making request to {}", &ccp_api_url);
    let ccp_response = match http_client.execute(http_client.get(&ccp_api_url).build().unwrap()).await {
        Ok(response) => {
            match response.text().await {
                Ok(result) => {
                    if result.is_empty() {
                        return Err(format!("CCP details: Empty response"));
                    } else {
                        result
                    }
                }
                Err(error) => {
                    return Err(format!("CCP details: Could not parse response from api got [{0}]", &error));
                }
            }
        }
        Err(error) => {
            return Err(format!("CCP details: Got error sending api request. Url: {0} Error:{1}", &ccp_api_url, &error));
        }
    };
    let ccp_response_value: serde_json::Value = match serde_json::from_str(&ccp_response) {
        Ok(value) => { value }
        Err(error) => {
            return Err(format!("CCP details: Response could not be parsed by serde_json [{0}] [{1:?}]", &ccp_response, error));
        }
    };
    let mut cpp_bson_doc = match bson::to_document(&ccp_response_value) {
        Ok(value) => { value }
        Err(error) => {
            return Err(format!("CCP details: Response could not be parsed by bson [{0}] [{1:?}]", &ccp_response, error));
        }
    };
    cpp_bson_doc.extend(zkill_bson_doc);
    return Ok(cpp_bson_doc);
}

#[tracing::instrument]
async fn get_killmail_for_days(day_list: &Vec<chrono::DateTime<chrono::Utc>>, base_url: &String, http_client: &reqwest::Client) -> Vec<(i64, String)> {
    info!("Getting zkill history for each day in the list. Num Days [{0}]", day_list.len());
    let mut output = Vec::new();
    for day in day_list {
        let api_formatted_date = day.format("%Y%m%d").to_string();
        let api_url = format!("{0}{1}.json", base_url, api_formatted_date);
        let mut resp = match http_client.execute(http_client.get(&api_url).build().unwrap()).await {
            Ok(response) => {
                match response.json::<std::collections::HashMap<i64, String>>().await {
                    Ok(result) => {
                        result
                    }
                    Err(error) => {
                        error!("Could not parse response from api got [{0}]", &error);
                        continue;
                    }
                }
            }
            Err(error) => {
                error!("Got error sending api request. Url: {0} Error:{1}",&api_url, &error);
                continue;
            }
        };

        for (id, hash) in resp.drain() {
            output.push((id, hash));
        }
    }
    return output;
}

fn get_encompassing_days(start_date: chrono::DateTime<chrono::Utc>, day_range: RangeInclusive<u64>) -> Vec<chrono::DateTime<chrono::Utc>> {
    let mut output_list = Vec::new();
    for num_days in day_range {
        let date_time = match start_date.checked_sub_days(chrono::Days::new(num_days)) {
            Some(result) => { result }
            None => panic!("Cannot determine days to import")
        };
        output_list.push(date_time);
    }
    return output_list;
}

async fn write_bson_to_database(ccp_value: &bson::document::Document, collection: &mongodb::Collection<Bson>) -> Result<mongodb::results::InsertOneResult, mongodb::error::Error> {
    return collection.insert_one(bson::to_bson(ccp_value).unwrap(), None).await;
}

async fn confirm_database_connection(database: &Database) -> Result<Document, mongodb::error::Error> {
    return database
        .run_command(doc! {"ping": 1}, None).await;
}

async fn get_database_client(connect_addr: &String) -> mongodb::error::Result<mongodb::Client> {
    let client_options = mongodb::options::ClientOptions::parse(connect_addr).await?;
    return mongodb::Client::with_options(client_options);
}

fn load_config_from_file() -> app_config::AppConfig {
    let args: Vec<String> = env::args().collect();
    let config_loc = match args.get(1) {
        Some(loc) => {
            loc
        }
        None => {
            panic!("Config file not specified in first argument");
        }
    };

    let imported_config = config::Config::builder()
        .add_source(config::File::with_name(config_loc))
        .add_source(config::Environment::with_prefix("NLH"))
        .build()
        .unwrap();
    return imported_config
        .try_deserialize::<app_config::AppConfig>()
        .unwrap();
}
fn config_logging(logging_level: Level) {
    let subscriber = FmtSubscriber::builder()
        .with_max_level(logging_level)
        .finish();
    tracing::subscriber::set_global_default(subscriber)
        .expect("Setting default logging subscriber failed");
}