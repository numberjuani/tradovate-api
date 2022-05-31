use crate::settings::{TIMEZONE};
use chrono::{Datelike, NaiveTime};
use serde_json::Value;


pub fn open_json(filename: &str) -> Result<Value, std::io::Error> {
    let file = std::fs::File::open(filename)?;
    let reader = std::io::BufReader::new(file);
    let data_file: Value = serde_json::from_reader(reader)?;
    Ok(data_file)
}

pub fn create_json_file<T: serde::Serialize>(filename: &str, contents: &T) {
    serde_json::to_writer(&std::fs::File::create(filename).unwrap(), contents).unwrap();
}

pub fn delete_file(filename: &str) {
    std::fs::remove_file(filename).unwrap()
}

pub fn get_duration_until_open() -> std::time::Duration {
    let now = chrono::Utc::now().with_timezone(&TIMEZONE);
    let market_opening_time = NaiveTime::from_hms(15, 0, 0);
    let market_closing_time = NaiveTime::from_hms(14, 0, 0);
    let market_is_closed = now.time() >= market_closing_time && now.time() < market_opening_time;
    let hours_min = if market_is_closed {(market_opening_time - now.time()).to_std().unwrap()} else {std::time::Duration::ZERO};
    let absolute = match now.weekday() {
        chrono::Weekday::Fri => {
            hours_min+std::time::Duration::from_secs(60*60*24*2)
        },
        chrono::Weekday::Sat => {
            hours_min+std::time::Duration::from_secs(60*60*24)
        },
        chrono::Weekday::Sun => {
            hours_min
        },
        _ => {
            hours_min
        }
    };
    if absolute != std::time::Duration::ZERO {absolute - std::time::Duration::from_secs(30)} else {absolute}
}