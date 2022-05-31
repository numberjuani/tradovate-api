use chrono::DateTime;
use chrono::NaiveDateTime;
use chrono::Utc;
use itertools::Itertools;
use serde::Deserialize;
use serde::Serialize;

use crate::models::SimpleQuote;
use crate::settings::ONE_SECOND;
use crate::settings::TIMEZONE;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Copy,Eq, PartialOrd, Ord)]
pub enum OrderAction {
    Buy,
    Sell,
    Unknown,
}
impl OrderAction {
    pub fn is_unknown(&self) -> bool {
        self == &OrderAction::Unknown
    }
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TimeAndSalesItem {
    pub historical_id: i64,
    pub action: OrderAction,
    pub qty: i64,
    pub price: f64,
    pub bid: f64,
    pub ask: f64,
    pub timestamp: i64,
    pub receipt_delay: i64,
}
impl TimeAndSalesItem {
    pub fn get_normal_time(&self) -> String {
        DateTime::<Utc>::from_utc(NaiveDateTime::from_timestamp(self.timestamp / 1000, 0), Utc)
            .with_timezone(&TIMEZONE)
            .format("%D %T %Z")
            .to_string()
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct TimeAndSalesPressure {
    pub bids_per_second: f64,
    pub asks_per_second: f64,
    pub net: f64,
    pub time: String,
    pub price: f64,
}
impl TimeAndSalesPressure {
    pub fn new() -> Self {
        Self {
            bids_per_second: 0.0,
            asks_per_second: 0.0,
            net: 0.0,
            time: String::new(),
            price: 0.0,
        }
    }
}
impl Default for TimeAndSalesPressure {
    fn default() -> Self {
        Self::new()
    }
}
#[derive(Debug, Clone)]
pub struct TimeAndSalesCollection {
    pub items: Vec<TimeAndSalesItem>,
}
impl TimeAndSalesCollection {
    pub fn new() -> Self {
        Self { items: Vec::new() }
    }
    pub fn get_pressure(&self) -> TimeAndSalesPressure {
        let now = chrono::Local::now();
        let ts_items = self
            .items
            .iter()
            .rev()
            .filter(|ts_item| ts_item.timestamp >= now.timestamp_millis() - ONE_SECOND)
            .collect_vec();
        let mut asks_per_second = 0.0;
        let mut bids_per_second = 0.0;
        for item in &ts_items {
            match item.action {
                OrderAction::Sell => bids_per_second += item.qty as f64,
                OrderAction::Buy => asks_per_second += item.qty as f64,
                OrderAction::Unknown => todo!(),
            }
        }
        let price = if let Some(last) = ts_items.last() {
            last.price
        } else {0.0};
        TimeAndSalesPressure {
            bids_per_second,
            asks_per_second,
            net: asks_per_second - bids_per_second,
            time: now.with_timezone(&TIMEZONE).format("%F %T").to_string(),
            price,
        }
    }
    pub fn get_quote(&self, historical_id: i64) -> Option<SimpleQuote> {
        if let Some(last_item) = self
            .items
            .iter()
            .rev()
            .find(|item| item.historical_id == historical_id)
        {
            return Some(SimpleQuote {
                bid: last_item.bid,
                ask: last_item.ask,
            });
        }
        None
    }
    pub fn largest_buy(&self) -> Option<TimeAndSalesItem> {
        self.items
            .iter()
            .filter(|item| item.action == OrderAction::Buy)
            .max_by_key(|item| item.qty)
            .cloned()
    }
    pub fn largest_sell(&self) -> Option<TimeAndSalesItem> {
        self.items
            .iter()
            .filter(|item| item.action == OrderAction::Sell)
            .max_by_key(|item| item.qty)
            .cloned()
    }
}

impl Default for TimeAndSalesCollection {
    fn default() -> Self {
        Self::new()
    }
}
