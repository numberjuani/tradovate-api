//LIVE SERVERS
pub const LIVE_TRADING_URL: &str = ""; //"live.tradovateapi.com"
pub const LIVE_MARKET_DATA_URL: &str = "md.tradovateapi.com";
// DEMO SERVERS
pub const DEMO_TRADING_URL: &str = "demo.tradovateapi.com";
//pub const DEMO_MARKET_DATA_URL:&str = "md-demo.tradovateapi.com";
// AUTH FILENAME
pub const AUTH_FILENAME: &str = "tradovate_auth.json";
pub const LOG_FILENAME: &str = "tradovate-algo.txt";
pub const TWILIO_URL: &str = "https://api.twilio.com/2010-04-01";

// TRADE THRESHOLDS
pub const DOM_THRESHOLD: f64 = 35.0;
pub const TS_PRESSURE_THRESHOLD: f64 = 150.0;
pub const LARGE_TRADE: i64 = 400;

pub const TRADING_SYMBOL: &str = "ESH2";
pub const ACCOUNT_NUMBER: &str = "your account num";

pub const TRADE_CLOSE_TRIGGER: f64 = 500.0;

pub const SOCKET_INTERVAL_SPEED: u64 = 1;
pub const SINGLE_LEG_COMMISSION: f64 = 2.05;

pub const TIMEZONE: chrono_tz::Tz = chrono_tz::US::Pacific;
pub const REPORT_TO_EMAIL: &str = "the email for peridic reporting";
pub const ONE_SECOND:i64 = 1000;

pub const DEBUG_MODE: bool = false;