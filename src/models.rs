use crate::settings::TIMEZONE;
use crate::strategy::MarketPosition;
use crate::time_and_sales::{self, OrderAction, TimeAndSalesItem};
use std::cmp::Ordering;
use std::sync::Arc;

pub type AcctCommChannelRWL = Arc<tokio::sync::RwLock<AcctCommsChannel>>;

use rayon::iter::IndexedParallelIterator;
use rayon::iter::IntoParallelRefIterator;
use rayon::iter::ParallelIterator;
use serde::Deserialize;
use serde::Serialize;
use serde_json::Value;

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all(deserialize = "camelCase"))]
pub struct ContractID {
    pub contract_maturity_id: i64,
    pub id: i64,
    #[serde(rename = "name")]
    pub symbol: String,
    pub provider_tick_size: f64,
    pub status: String,
    #[serde(default = "to_be_calculated_int")]
    pub historical_id: i64,
    #[serde(default = "to_be_calculated_float")]
    pub big_point_value: f64,
}
#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all(deserialize = "camelCase"))]
pub struct ProductInfo {
    pub id: i64,
    #[serde(rename = "name")]
    pub symbol: String,
    pub currency_id: i64,
    pub product_type: String,
    pub description: String,
    pub exchange_id: i64,
    pub contract_group_id: i64,
    pub risk_discount_contract_group_id: i64,
    pub status: String,
    pub months: String,
    #[serde(default = "to_be_calculated_bool")]
    pub is_secured: bool,
    pub value_per_point: f64,
    pub price_format_type: String,
    pub price_format: i64,
    pub tick_size: f64,
}
pub fn to_be_calculated_bool() -> bool {
    false
}

#[derive(Debug)]
pub struct AcctCommsChannel {
    pub connection_established: bool,
    pub last_heart_beat_instant: tokio::time::Instant,
    pub is_authorized: bool,
    pub socket_close: bool,
    pub terminated: bool,
    pub user_data: Option<UserDataMessage>,
    pub unsent_requests: Vec<String>,
    pub sent_requests: Vec<String>,
    pub order_update: Option<OrderUpdateMessage>,
    pub order_id_messages: Vec<OrderIDMessage>,
    pub received_closing_frame:bool,
}
impl AcctCommsChannel {
    pub fn new(requests: Vec<String>) -> Self {
        Self {
            last_heart_beat_instant: tokio::time::Instant::now(),
            is_authorized: false,
            socket_close: false,
            terminated: false,
            user_data: None,
            connection_established: false,
            unsent_requests: requests,
            sent_requests: Vec::new(),
            order_update: None,
            order_id_messages: Vec::new(),
            received_closing_frame: false,
        }
    }
}

// BEGIN STRUCTS FOR QUOTE MESSAGE
#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all(deserialize = "camelCase"))]
pub struct QuoteMessage {
    #[serde(rename = "d")]
    pub data: D,
    #[serde(rename = "e")]
    pub event: String,
}
#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all(deserialize = "camelCase"))]
pub struct D {
    pub quotes: Vec<Quote>,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all(deserialize = "camelCase"))]
pub struct Quote {
    pub contract_id: i64,
    pub entries: Entries,
    pub id: i64,
    pub timestamp: String,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all(deserialize = "camelCase"))]
pub struct Entries {
    #[serde(rename = "Bid")]
    pub bid: Bid,
    #[serde(rename = "HighPrice")]
    pub high_price: HighPrice,
    #[serde(rename = "LowPrice")]
    pub low_price: LowPrice,
    #[serde(rename = "Offer")]
    pub offer: Offer,
    #[serde(rename = "OpenInterest")]
    pub open_interest: OpenInterest,
    #[serde(rename = "OpeningPrice")]
    pub opening_price: OpeningPrice,
    #[serde(rename = "SettlementPrice")]
    pub settlement_price: SettlementPrice,
    #[serde(rename = "TotalTradeVolume")]
    pub total_trade_volume: TotalTradeVolume,
    #[serde(rename = "Trade")]
    pub trade: Trade,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all(deserialize = "camelCase"))]
pub struct Bid {
    pub price: f64,
    pub size: i64,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all(deserialize = "camelCase"))]
pub struct HighPrice {
    pub price: f64,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all(deserialize = "camelCase"))]
pub struct LowPrice {
    pub price: f64,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all(deserialize = "camelCase"))]
pub struct Offer {
    pub price: f64,
    pub size: i64,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all(deserialize = "camelCase"))]
pub struct OpenInterest {
    pub size: i64,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all(deserialize = "camelCase"))]
pub struct OpeningPrice {
    pub price: f64,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all(deserialize = "camelCase"))]
pub struct SettlementPrice {
    pub price: f64,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all(deserialize = "camelCase"))]
pub struct TotalTradeVolume {
    pub size: i64,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all(deserialize = "camelCase"))]
pub struct Trade {
    pub price: f64,
}
// END QUOTE MESSAGE
//BEGIN STRUCTS FOR DOM
#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all(deserialize = "camelCase"))]
pub struct DOMMessage {
    #[serde(rename = "d")]
    pub data: DOMData,
    #[serde(rename = "e")]
    pub event: String,
}
impl DOMMessage {
    pub fn combine(&self) -> DOMSummary {
        let mut target = Vec::new();
        self.data
            .doms
            .par_iter()
            .map(|dom| dom.get_summary())
            .collect_into_vec(&mut target);
        let mut total = DOMSummary::new();
        for summary in target {
            total = total.sum(summary);
        }
        total
    }
}
#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all(deserialize = "camelCase"))]
pub struct DOMData {
    pub doms: Vec<Dom>,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all(deserialize = "camelCase"))]
pub struct Dom {
    pub bids: Vec<Bid>,
    pub contract_id: i64,
    pub offers: Vec<Offer>,
    pub timestamp: String,
}
impl Dom {
    pub fn get_summary(&self) -> DOMSummary {
        let asks: i64 = self.offers.iter().map(|offer| offer.size).sum();
        let bids: i64 = self.bids.iter().map(|bid| bid.size).sum();
        let total = asks + bids;
        let percent_buys = 100.0 * (bids as f64 / total as f64);
        let percent_sells = 100.0 * (asks as f64 / total as f64);
        let timestamp = match chrono::DateTime::parse_from_rfc3339(&self.timestamp) {
            Ok(ts) => ts.timestamp_millis(),
            Err(_) => chrono::Utc::now().timestamp_millis()  
        };
        DOMSummary {
            percent_sells,
            percent_buys,
            total_orders: total,
            contract_ids: vec![self.contract_id],
            net: percent_sells - percent_buys,
            timestamp,
            total_bid: bids,
            total_ask: asks,
        }
    }
}
#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DOMSummary {
    pub contract_ids: Vec<i64>,
    pub total_bid: i64,
    pub total_ask: i64,
    pub percent_sells: f64,
    pub percent_buys: f64,
    pub total_orders: i64,
    pub timestamp: i64,
    pub net: f64,
}
impl DOMSummary {
    pub fn new() -> Self {
        Self {
            contract_ids: Vec::new(),
            percent_sells: 0.0,
            percent_buys: 0.0,
            total_orders: 0,
            timestamp: 0,
            net: 0.0,
            total_bid: 0,
            total_ask: 0,
        }
    }
    pub fn sum(self, other: DOMSummary) -> Self {
        let mut contained = false;
        for id in &other.contract_ids {
            contained = self.contract_ids.contains(id);
        }
        if !contained {
            let mut all = self.contract_ids;
            all.extend(other.contract_ids);
            let new_total = self.total_orders + other.total_orders;
            let new_bids = self.total_bid + other.total_bid;
            let new_asks = self.total_ask + other.total_ask;
            let percent_sells = 100.0 * (new_asks as f64 / new_total as f64);
            let percent_buys = 100.0 * (new_bids as f64 / new_total as f64);
            let timestamp = if self.timestamp.ne(&0) && other.timestamp.ne(&0) {
                std::cmp::min(self.timestamp, other.timestamp)
            } else {
                std::cmp::max(self.timestamp,other.timestamp)
            };
            Self {
                contract_ids: all,
                percent_sells,
                percent_buys,
                total_orders: self.total_orders + other.total_orders,
                net: percent_sells - percent_buys,
                timestamp,
                total_bid: self.total_bid + other.total_bid,
                total_ask: self.total_ask + other.total_ask,
            }
        } else {
            Self { ..self }
        }
    }
    pub fn get_time(&self) -> String {
        chrono::DateTime::<chrono::Utc>::from_utc(chrono::NaiveDateTime::from_timestamp(self.timestamp/1000, 0), chrono::Utc)
            .with_timezone(&TIMEZONE)
            .format("%D %T %Z")
            .to_string()
    }
}
#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WebsocketResponse {
    #[serde(rename = "s")]
    pub status: i64,
    #[serde(rename = "i")]
    pub request_id: i64,
    #[serde(rename = "d")]
    pub data: RequestData,
}
#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RequestData {
    pub mode: String,
    #[serde(rename = "subscriptionId")]
    #[serde(default = "to_be_calculated_int")]
    pub contract_id: i64,
    #[serde(rename = "historicalId")]
    #[serde(default = "to_be_calculated_int")]
    pub historical_id: i64,
    #[serde(rename = "realtimeId")]
    #[serde(default = "to_be_calculated_int")]
    pub realtime_id: i64,
}
// TICK CHART DATA MESSAGES
#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all(deserialize = "camelCase"))]
pub struct TickChart {
    #[serde(rename = "d")]
    pub data: ChartData,
    #[serde(rename = "e")]
    pub event: String,
}
impl TickChart {
    pub fn get_all_ts_items(&self) -> Vec<TimeAndSalesItem> {
        let mut vec_of_vec = Vec::new();
        self.data
            .charts
            .par_iter()
            .map(|chart| chart.get_ts_items())
            .collect_into_vec(&mut vec_of_vec);
        let mut output_vec: Vec<TimeAndSalesItem> = Vec::new();
        for vec in vec_of_vec {
            output_vec.extend(vec);
        }
        output_vec
    }
}
#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all(deserialize = "camelCase"))]
pub struct ChartData {
    pub charts: Vec<Chart>,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all(deserialize = "camelCase"))]
#[serde(default)]
pub struct Chart {
    #[serde(default = "to_be_calculated_string")]
    pub symbol: String,
    #[serde(default = "to_be_calculated_int")]
    pub contract_id: i64,
    #[serde(rename = "bp")]
    pub base_price: i64,
    #[serde(rename = "bt")]
    pub base_timestamp: i64,
    #[serde(rename = "id")]
    pub historical_id: i64,
    #[serde(rename = "s")]
    pub packet_data_source: String,
    #[serde(rename = "td")]
    pub trade_date: i64, // YYYYMMDD
    #[serde(rename = "tks")]
    pub ticks: Vec<Tick>,
    #[serde(rename = "ts")]
    pub tick_size: f64,
    pub eoh: bool,
}
impl Chart {
    pub fn get_ts_items(&self) -> Vec<TimeAndSalesItem> {
        let mut output_vec = Vec::new();
        let base_price = self.base_price as f64 * self.tick_size;
        self.ticks
            .par_iter()
            .map(|tick| {
                tick.to_ts_item(
                    base_price,
                    self.tick_size,
                    self.base_timestamp,
                    self.historical_id,
                )
            })
            .collect_into_vec(&mut output_vec);
        output_vec
    }
}
pub fn to_be_calculated_string() -> String {
    String::new()
}
#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all(deserialize = "camelCase"))]
pub struct Tick {
    #[serde(rename = "a")]
    #[serde(default = "to_be_calculated_int")]
    pub ask_relative_price: i64,
    #[serde(rename = "as")]
    #[serde(default = "to_be_calculated_int")]
    pub ask_size: i64,
    #[serde(rename = "b")]
    #[serde(default = "to_be_calculated_int")]
    pub bid_relative_price: i64,
    #[serde(rename = "bs")]
    #[serde(default = "to_be_calculated_int")]
    pub bid_size: i64,
    #[serde(rename = "id")]
    pub tick_id: i64,
    #[serde(rename = "p")]
    pub relative_price: i64, // Actual tick price is packet.bp + tick.p
    #[serde(rename = "s")]
    pub tick_volume: i64,
    #[serde(rename = "t")]
    pub relative_timestamp: i64, // Actual tick timestamp is packet.bt + tick.t
}
impl Tick {
    pub fn to_ts_item(
        &self,
        base_price: f64,
        tick_size: f64,
        base_timestamp: i64,
        historical_id: i64,
    ) -> TimeAndSalesItem {
        let price = base_price + (tick_size * self.relative_price as f64);
        let bid = base_price + (tick_size * self.bid_relative_price as f64);
        let ask = base_price + (tick_size * self.ask_relative_price as f64);
        let mid_price = 0.5 * (ask + bid);
        let timestamp = base_timestamp + self.relative_timestamp;
        let delay = chrono::Utc::now().timestamp_millis() - timestamp;
        let action = if price > mid_price {
            OrderAction::Buy
        } else if price < mid_price {
            OrderAction::Sell
        } else {
            OrderAction::Unknown
        };
        TimeAndSalesItem {
            historical_id,
            action,
            qty: self.tick_volume,
            price,
            timestamp,
            receipt_delay: delay,
            bid,
            ask,
        }
    }
}
pub fn to_be_calculated_int() -> i64 {
    0
}

// BEGIN USER DATA MESSAGES

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all(deserialize = "camelCase"))]
pub struct UserDataMessage {
    #[serde(rename = "d")]
    pub data: UserData,
    #[serde(rename = "i")]
    pub request_id: i64,
    #[serde(rename = "s")]
    pub request_status: i64,
}
impl UserDataMessage {
    pub fn is_flat(&self) -> bool {
        match self.data.positions.is_empty() {
            true => true,
            false => {
                for position in &self.data.positions {
                    if position.net_pos != 0 {
                        return false;
                    }
                }
                true
            }
        }
    }
}
#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all(deserialize = "camelCase"))]
pub struct UserData {
    pub account_risk_statuses: Vec<AccountRiskStatus>,
    pub accounts: Vec<Account>,
    pub cash_balances: Vec<CashBalance>,
    pub command_reports: Vec<Value>,
    pub commands: Vec<Value>,
    pub contract_groups: Vec<ContractGroup>,
    pub contract_maturities: Vec<Value>,
    pub contracts: Vec<Value>,
    pub currencies: Vec<Currency>,
    pub exchanges: Vec<Exchange>,
    pub execution_reports: Vec<Value>,
    pub fill_pairs: Vec<Value>,
    pub fills: Vec<Value>,
    pub margin_snapshots: Vec<MarginSnapshot>,
    pub order_strategies: Vec<Value>,
    pub order_strategy_links: Vec<Value>,
    pub order_strategy_types: Vec<Value>,
    pub order_versions: Vec<Value>,
    pub orders: Vec<Value>,
    pub positions: Vec<Position>,
    pub products: Vec<Value>,
    pub properties: Vec<Value>,
    pub spread_definitions: Vec<Value>,
    pub user_account_auto_liqs: Vec<UserAccountAutoLiq>,
    pub user_plugins: Vec<UserPlugin>,
    pub user_properties: Vec<UserProperty>,
    pub user_read_statuses: Vec<UserReadStatuse>,
    pub users: Vec<User>,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all(deserialize = "camelCase"))]
pub struct AccountRiskStatus {
    pub id: i64,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all(deserialize = "camelCase"))]
pub struct Account {
    pub account_type: String,
    pub active: bool,
    pub archived: bool,
    pub auto_liq_profile_id: i64,
    pub clearing_house_id: i64,
    pub id: i64,
    pub legal_status: String,
    pub margin_account_type: String,
    pub name: String,
    pub nickname: String,
    pub risk_category_id: i64,
    pub timestamp: String,
    pub user_id: i64,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all(deserialize = "camelCase"))]
pub struct CashBalance {
    pub account_id: i64,
    pub amount: f64,
    pub archived: bool,
    pub currency_id: i64,
    pub id: i64,
    pub realized_pn_l: f64,
    pub timestamp: String,
    pub trade_date: TradeDate,
    pub week_realized_pn_l: f64,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all(deserialize = "camelCase"))]
pub struct TradeDate {
    pub day: i64,
    pub month: i64,
    pub year: i64,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all(deserialize = "camelCase"))]
pub struct ContractGroup {
    pub id: i64,
    pub name: String,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all(deserialize = "camelCase"))]
pub struct Currency {
    pub id: i64,
    pub name: String,
    pub symbol: String,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all(deserialize = "camelCase"))]
pub struct Exchange {
    pub cftc_reporting: bool,
    pub complex: String,
    pub free_market_data: bool,
    pub id: i64,
    pub is_secured_default: bool,
    pub market_type: String,
    pub name: String,
    pub span: Option<String>,
    pub time_zone: String,
    pub tradingview_groups: Option<String>,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all(deserialize = "camelCase"))]
pub struct MarginSnapshot {
    pub auto_liq_level: f64,
    pub full_initial_margin: f64,
    pub id: i64,
    pub initial_margin: f64,
    pub liq_only_level: f64,
    pub maintenance_margin: f64,
    pub risk_time_period_id: i64,
    pub timestamp: String,
    pub total_used_margin: f64,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all(deserialize = "camelCase"))]
pub struct UserAccountAutoLiq {
    pub id: i64,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all(deserialize = "camelCase"))]
pub struct UserPlugin {
    pub approval: bool,
    pub archived: bool,
    pub autorenewal: Option<bool>,
    pub credit_card_id: Option<i64>,
    pub credit_card_transaction_id: Option<i64>,
    pub entitlement_id: i64,
    pub expiration_date: Option<ExpirationDate>,
    pub id: i64,
    pub paid_amount: f64,
    pub plan_categories: String,
    pub plan_price: f64,
    pub plugin_name: String,
    pub start_date: StartDate,
    pub timestamp: String,
    pub user_id: i64,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all(deserialize = "camelCase"))]
pub struct ExpirationDate {
    pub day: i64,
    pub month: i64,
    pub year: i64,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all(deserialize = "camelCase"))]
pub struct StartDate {
    pub day: i64,
    pub month: i64,
    pub year: i64,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all(deserialize = "camelCase"))]
pub struct UserProperty {
    pub id: i64,
    pub property_id: i64,
    pub user_id: i64,
    pub value: String,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all(deserialize = "camelCase"))]
pub struct UserReadStatuse {
    pub id: i64,
    pub news_story_id: i64,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all(deserialize = "camelCase"))]
pub struct User {
    pub creation_timestamp: String,
    pub email: String,
    pub id: i64,
    pub name: String,
    pub professional: bool,
    pub status: String,
    pub timestamp: String,
    pub two_factor_auth: bool,
    pub user_type: String,
}

// ORDER UPDATE MESSAGE

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all(deserialize = "camelCase"))]
pub struct OrderUpdateMessage {
    #[serde(rename = "d")]
    pub data: OrderUpdate,
    #[serde(rename = "e")]
    pub event: String,
}
impl OrderUpdateMessage {
    pub fn is_filled(&self) -> bool {
        self.data.entity.ord_status == "Filled"
    }
    pub fn id(&self) -> i64 {
        self.data.entity.order_id
    }
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all(deserialize = "camelCase"))]
pub struct OrderUpdate {
    pub entity: OrderStatus,
    pub entity_type: String,
    pub event_type: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all(deserialize = "camelCase"))]
pub struct OrderStatus {
    pub account_id: i64,
    pub action: time_and_sales::OrderAction,
    pub avg_px: f64,
    pub command_id: i64,
    pub contract_id: i64,
    pub cum_qty: i64,
    pub exec_type: String,
    pub external_cl_ord_id: String,
    pub id: i64,
    pub last_px: f64,
    pub last_qty: i64,
    pub name: String,
    pub ord_status: String,
    pub order_id: i64,
    pub timestamp: String,
    #[serde(default = "to_be_calculated_ts")]
    pub parsing_ts: String,
}
impl OrderStatus {
    pub fn new(avg_px: f64, action: OrderAction) -> Self {
        Self {
            account_id: 0,
            action,
            avg_px,
            command_id: 0,
            contract_id: 0,
            cum_qty: 0,
            exec_type: String::new(),
            external_cl_ord_id: String::new(),
            id: 0,
            last_px: 0.0,
            last_qty: 0,
            name: String::new(),
            ord_status: "Filled".to_string(),
            order_id: 0,
            timestamp: String::new(),
            parsing_ts: to_be_calculated_ts(),
        }
    }
}
fn to_be_calculated_ts() -> String {
    chrono::Utc::now()
        .with_timezone(&TIMEZONE)
        .format("%F %T")
        .to_string()
}
#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all(deserialize = "camelCase"))]
pub struct Position {
    pub account_id: i64,
    pub archived: bool,
    pub bought: i64,
    pub bought_value: f64,
    pub contract_id: i64,
    pub id: i64,
    pub net_pos: i64,
    #[serde(default = "to_be_calculated_float")]
    pub net_price: f64,
    pub prev_pos: i64,
    pub sold: i64,
    pub sold_value: f64,
    pub timestamp: String,
    pub trade_date: TradeDate,
}
impl Position {
    pub fn to_market_position(&self) -> MarketPosition {
        let action = match self.net_pos.cmp(&0) {
            Ordering::Greater => OrderAction::Buy,
            Ordering::Less => OrderAction::Buy,
            Ordering::Equal => OrderAction::Unknown,
        };
        MarketPosition {
            account_id: self.account_id,
            contract_id: self.contract_id,
            net_pos: self.net_pos,
            entry_fill: Some(OrderStatus::new(self.net_price, action)),
            exit_fill: None,
            realized_pnl: 0.0,
            entry_trigger: String::new(),
            exit_trigger: String::new(),
        }
    }
}
pub fn to_be_calculated_float() -> f64 {
    0.0
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all(deserialize = "camelCase"))]
pub struct OrderIDMessage {
    #[serde(rename = "d")]
    pub data: OrderID,
    #[serde(rename = "i")]
    pub request_id: i64,
    #[serde(rename = "s")]
    pub request_status: i64,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all(deserialize = "camelCase"))]
pub struct OrderID {
    pub order_id: i64,
}

#[derive(Debug)]
pub struct SimpleQuote {
    pub bid: f64,
    pub ask: f64,
}
