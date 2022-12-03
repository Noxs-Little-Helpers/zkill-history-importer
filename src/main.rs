mod models;

use models::{
    config,
};
use log::{error, info, warn, LevelFilter, debug};

use std::{
    env,
    fs,
};
use std::future::Future;
use std::ops::RangeInclusive;
use chrono::TimeZone;
use log4rs::{
    append::{
        console::{
            ConsoleAppender,
            Target,
        },
        file::FileAppender,
        rolling_file::{
            policy::{
                compound::CompoundPolicy,
                compound::roll::fixed_window::FixedWindowRoller,
                compound::trigger::size::SizeTrigger,
            },
            RollingFileAppender,
        },
    },
    encode::{pattern::PatternEncoder, json::JsonEncoder},
    config::{Appender, Config, Logger, Root},
    filter::threshold::ThresholdFilter,
};
use mongodb::bson::{Bson, doc, Document};
use mongodb::{bson, Database};
use crate::models::api;

#[tokio::main]
async fn main() {
    let app_config: config::AppConfig = load_config();
    config_logging(&app_config.logging);
    info!("zkill-history-importer started");

    let start_date = match &app_config.start_date {
        Some(value) => {
            match chrono::Utc.datetime_from_str(&value, "%Y-%m-%d") {
                Ok(value) => value,
                Err(error) => panic!("Cannot parse start date string. [{0}]", error)
            }
        }
        None => { chrono::Utc::now() }
    };
    let day_list = get_list_days(start_date, 0..=app_config.num_days);
    let all_ids = get_ids_for_day(&day_list, &app_config.api_config.zkill_history_url).await;
    println!("{:?}", all_ids);
    upload_to_database(all_ids, &app_config).await;
}

async fn upload_to_database(killmails: Vec<(u64, String)>, app_config: &config::AppConfig) {
    let client: mongodb::Client = match connect_to_db(&app_config.database.conn_string).await {
        Ok(client) => {
            client
        }
        Err(error) => {
            error!("Database: Unable to create database client");
            panic!("Database: Unable to create database client. Dont know how to proceed so panicking");
        }
    };
    let database = client.database(&app_config.database.database_name);
    let collection = database.collection(&app_config.database.collection_name);
    {
        info!("Database: Attempting to connect");
        let mut test_ping_successful = false;
        loop {
            match ping_db(&database).await {
                Ok(document) => { test_ping_successful = true }
                Err(error) => {
                    error!("Database: Unable to ping. Reattempting... [{0:?}]", error);
                }
            }
            if (test_ping_successful) {
                info!("Database: Connection established");
                break;
            }
        }
    }
    for (id, hash) in killmails {
        let exists_already = is_in_collection(&id, &collection).await;
        // if !exists_already {
        let (ccp_value, zkb_value) = match get_kill_details(&id, &hash, &app_config.api_config.zkill_details_url, &app_config.api_config.zkill_details_url).await {
            Ok(output) => { output }
            Err(message) => {
                error!("{0}", &message);
                panic!("{0}", &message);
            }
        };
        // write_to_db(ccp_value, zkb_value, &collection).await;
        // }
        break;
    }
}

async fn is_in_collection(id: &u64, collection: &mongodb::Collection<Bson>) -> bool {
    return false;
}

async fn write_to_db(ccp_value: Bson, zkb_value: Bson, collection: &mongodb::Collection<Bson>) -> Result<mongodb::results::InsertOneResult, mongodb::error::Error> {
    return collection.insert_one(ccp_value, None).await;
}

async fn get_kill_details(id: &u64, hash: &String, zkill_details_url: &String, ccp_details_url: &String) -> Result<(Bson, Bson), String> {
    let zkill_api_url = format!("{0}{1}/", zkill_details_url, id);
    println!("{}", &zkill_api_url);
    let zkill_response = match reqwest::get(&zkill_api_url).await {
        Ok(response) => {
            match response.text().await {
                Ok(result) => {
                    if result.is_empty() {
                        return Err(format!("zkillboard details: Empty repose"));
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
    let ccp_api_url = format!("{0}/{1}/{2}/?datasource=tranquility", ccp_details_url, id, hash);
    let ccp_response = match reqwest::get(&ccp_api_url).await {
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
    let mut zkill_response_value: serde_json::Value = match serde_json::from_str(&zkill_response) {
        Ok(value) => { value }
        Err(error) => {
            return Err(format!("zKillboard details: Response could not be parsed by serde_json [{0}] [{1:?}]", &zkill_response, error));
        }
    };
    let mut ccp_response_value: serde_json::Value = match serde_json::from_str(&ccp_response) {
        Ok(value) => { value }
        Err(error) => {
            return Err(format!("CCP details: Response could not be parsed by serde_json [{0}] [{1:?}]", &ccp_response, error));
        }
    };
    let zkill_bson_doc = match bson::to_bson(&zkill_response_value) {
        Ok(value) => { value }
        Err(error) => {
            return Err(format!("zKillboard details: Response could not be parsed by bson [{0}] [{1:?}]", &zkill_response_value, error));
        }
    };
    let cpp_bson_doc = match bson::to_bson(&ccp_response_value) {
        Ok(value) => { value }
        Err(error) => {
            return Err(format!("CCP details: Response could not be parsed by bson [{0}] [{1:?}]", &ccp_response, error));
        }
    };
    return Ok((cpp_bson_doc, zkill_bson_doc));
}

async fn get_ids_for_day(day_list: &Vec<chrono::DateTime<chrono::Utc>>, base_url: &String) -> Vec<(u64, String)> {
    let mut output = Vec::new();
    for day in day_list {
        let api_formatted_date = day.format("%Y%m%d").to_string();
        let api_url = format!("{0}{1}.json", base_url, api_formatted_date);
        let mut resp = match reqwest::get(&api_url).await {
            Ok(response) => {
                match response.json::<std::collections::HashMap<u64, String>>().await {
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
            println!("{0}{1}", &id,&hash);
            output.push((id, hash));
        }
    }
    return output;
}

fn get_list_days(start_date: chrono::DateTime<chrono::Utc>, day_range: RangeInclusive<u64>) -> Vec<chrono::DateTime<chrono::Utc>> {
    let mut output_list = Vec::new();
    for num_days in day_range {
        let mut date_time = match start_date.checked_sub_days(chrono::Days::new(num_days)) {
            Some(result) => { result }
            None => panic!("Cannot determine days to import")
        };
        output_list.push(date_time);
    }
    return output_list;
}

async fn ping_db(database: &Database) -> Result<Document, mongodb::error::Error> {
    return database
        .run_command(doc! {"ping": 1}, None).await;
}

async fn connect_to_db(connect_addr: &String) -> mongodb::error::Result<mongodb::Client> {
    let mut client_options = mongodb::options::ClientOptions::parse(connect_addr).await?;
    return mongodb::Client::with_options(client_options);
}

fn load_config() -> config::AppConfig {
    let args: Vec<String> = env::args().collect();
    let config_loc = match args.get(1) {
        Some(loc) => {
            loc
        }
        None => {
            panic!("Config file not specified in first argument");
        }
    };
    let contents = fs::read_to_string(config_loc)
        .expect(&format!("Cannot open config file {0}", config_loc));
    let model: config::AppConfig = serde_json::from_str(&contents).unwrap();
    return model;
}

fn config_logging(logging_config: &config::LoggingConfig) {
    let mut active_log = String::new();
    {
        active_log.push_str(&logging_config.dir);
        active_log.push_str("/");
        active_log.push_str(&logging_config.active_file);
    }
    let mut archive_patter = String::new();
    {
        archive_patter.push_str(&logging_config.dir);
        archive_patter.push_str("/");
        archive_patter.push_str(&logging_config.archive_pattern);
    }
    let log_to_stdout = ConsoleAppender::builder().target(Target::Stdout)
        .build();
    // Build a file logger.
    let log_to_file = RollingFileAppender::builder()
        .encoder(Box::new(JsonEncoder::new()))
        .build(&active_log,
               Box::new(CompoundPolicy::new(
                   Box::new(SizeTrigger::new(10 * 1024 * 1024)),
                   Box::new(FixedWindowRoller::builder().build(&archive_patter, 10).unwrap()),
               )))
        .unwrap();

    // Log Debug level output to file where debug is the default level
    // and the programmatically specified level to stderr.
    let config = Config::builder()
        .appender(Appender::builder().build("log_to_file", Box::new(log_to_file)))
        .appender(
            Appender::builder()
                .filter(Box::new(ThresholdFilter::new(log::LevelFilter::Info)))
                .build("log_to_stdout", Box::new(log_to_stdout)),
        )
        .build(
            Root::builder()
                .appender("log_to_file")
                .appender("log_to_stdout")
                .build(LevelFilter::Debug),
        )
        .unwrap();

    log4rs::init_config(config).unwrap();
}