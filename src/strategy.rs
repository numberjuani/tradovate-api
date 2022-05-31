#![allow(dead_code)]
use serde_json::{json, Value};
use std::{cmp::Ordering, sync::Arc};
pub type StrategyRWL = Arc<tokio::sync::RwLock<Strategy>>;
use crate::{
    api_client::ResourceType,
    credentials::USERNAME,
    md_socket::MarketDataRWL,
    models::{
        AcctCommChannelRWL, DOMSummary, OrderStatus, OrderUpdateMessage, Position, SimpleQuote,
    },
    reporting::{program_log, ReportRWL},
    settings::{
        ACCOUNT_NUMBER, DOM_THRESHOLD, LARGE_TRADE, ONE_SECOND, SINGLE_LEG_COMMISSION,
        TRADE_CLOSE_TRIGGER, TRADING_SYMBOL, TS_PRESSURE_THRESHOLD,
    },
    time_and_sales::{OrderAction, TimeAndSalesItem, TimeAndSalesPressure},
};

#[derive(Debug, Clone)]
pub struct MarketPosition {
    pub account_id: i64,
    pub contract_id: i64,
    pub net_pos: i64,
    pub entry_trigger: String,
    pub entry_fill: Option<OrderStatus>,
    pub exit_trigger: String,
    pub exit_fill: Option<OrderStatus>,
    pub realized_pnl: f64,
}
impl MarketPosition {
    pub fn new_with_trigger(entry_trigger: String) -> Self {
        Self {
            account_id: 0,
            contract_id: 0,
            net_pos: 0,
            entry_trigger,
            entry_fill: None,
            exit_trigger: String::new(),
            exit_fill: None,
            realized_pnl: 0.0,
        }
    }
    pub fn update_from_order_update(self, update: &OrderUpdateMessage) -> Self {
        Self {
            account_id: update.data.entity.account_id,
            contract_id: update.data.entity.contract_id,
            net_pos: if update.data.entity.action == OrderAction::Buy {
                update.data.entity.cum_qty
            } else {
                -update.data.entity.cum_qty
            },
            entry_fill: Some(update.data.entity.to_owned()),
            exit_fill: None,
            realized_pnl: 0.0,
            ..self
        }
    }
    pub fn from_reversal(update: &OrderUpdateMessage) -> Self {
        Self {
            account_id: update.data.entity.account_id,
            contract_id: update.data.entity.contract_id,
            net_pos: if update.data.entity.action == OrderAction::Buy {
                update.data.entity.cum_qty / 2
            } else {
                -update.data.entity.cum_qty / 2
            },
            entry_fill: Some(update.data.entity.to_owned()),
            exit_fill: None,
            realized_pnl: 0.0,
            entry_trigger: String::new(),
            exit_trigger: String::new(),
        }
    }
    pub fn close(
        self,
        update: &OrderUpdateMessage,
        big_point_value: f64,
        single_leg_commission: f64,
    ) -> Self {
        let difference = update.data.entity.avg_px - self.entry_fill.as_ref().unwrap().avg_px;
        Self {
            realized_pnl: (self.net_pos as f64 * (big_point_value * difference))
                - (2.0 * single_leg_commission) * (self.net_pos.abs() as f64),
            exit_fill: Some(update.data.entity.to_owned()),
            ..self
        }
    }
    pub fn create_closing_ticket(&self, request_id: i64) -> String {
        let action = match self.net_pos.cmp(&0) {
            Ordering::Greater => OrderAction::Sell,
            Ordering::Less => OrderAction::Buy,
            Ordering::Equal => OrderAction::Unknown,
        };
        let order_ticket = create_market_order_ticket(action, self.net_pos.abs(), self.account_id);
        create_order_request(request_id, order_ticket)
    }
    pub fn unrealized_pnl(&self, quote: &SimpleQuote, point_value: f64) -> f64 {
        match self.net_pos.cmp(&0) {
            Ordering::Less => {
                self.net_pos.abs() as f64
                    * point_value
                    * (self.entry_fill.as_ref().unwrap().avg_px - quote.ask)
            }
            Ordering::Equal => 0.0,
            Ordering::Greater => {
                self.net_pos as f64
                    * point_value
                    * (quote.bid - self.entry_fill.as_ref().unwrap().avg_px)
            }
        }
    }
    pub fn matches_account_position(&self, account_position: &Position) -> bool {
        self.net_pos == account_position.net_pos
            && self.account_id == account_position.account_id
            && self.contract_id == account_position.contract_id
    }
}

#[derive(Debug)]
pub struct Strategy {
    pub dom_extreme: (String, DOMSummary),
    pub ts_pressure_extreme: TimeAndSalesPressure,
    pub largest_single_item: Option<TimeAndSalesItem>,
    pub sent_entry_orders: bool,
    pub sent_exit_orders: bool,
    pub status: StrategyStatus,
    pub positions: Vec<MarketPosition>,
    pub last_report_instant: tokio::time::Instant,
}
impl Strategy {
    pub fn is_opposite(&self, action: OrderAction) -> bool {
        match action {
            OrderAction::Buy => self.net_pos() < 0,
            OrderAction::Sell => self.net_pos() > 0,
            OrderAction::Unknown => false,
        }
    }
    pub fn net_pos(&self) -> i64 {
        if let Some(position) = self.positions.last() {
            position.net_pos
        } else {
            0
        }
    }
    pub fn has_open_position(&self) -> bool {
        if let Some(position) = self.positions.last() {
            position.exit_fill.is_none()
        } else {
            false
        }
    }
    pub fn new() -> Self {
        Self {
            dom_extreme: (String::new(), DOMSummary::new()),
            ts_pressure_extreme: TimeAndSalesPressure::new(),
            status: StrategyStatus::Unaware,
            sent_entry_orders: false,
            sent_exit_orders: false,
            positions: Vec::new(),
            last_report_instant: tokio::time::Instant::now(),
            largest_single_item: None,
        }
    }
    pub fn total_pnl(&self) -> f64 {
        self.positions
            .iter()
            .fold(0.0, |acc, x| acc + x.realized_pnl)
    }
    pub fn open_pnl(&self, quote: SimpleQuote, point_value: f64) -> Option<f64> {
        if let Some(position) = self.positions.last() {
            if position.exit_fill.is_none() {
                Some(position.unrealized_pnl(&quote, point_value))
            } else {
                None
            }
        } else {
            None
        }
    }
    pub fn summary(&self, quote: SimpleQuote, point_value: f64) -> String {
        format!(
            "Total Session Pnl {}\n Open Pnl {:#?}",
            self.total_pnl(),
            self.open_pnl(quote, point_value)
        )
    }
}
impl Default for Strategy {
    fn default() -> Self {
        Self::new()
    }
}
pub async fn calculate(
    market_data_rwl: MarketDataRWL,
    strategy_rwl: StrategyRWL,
    account_data_rwl: AcctCommChannelRWL,
    report_rwl: ReportRWL,
) {
    let signal = generate_signal(market_data_rwl.read().await.clone(), strategy_rwl.clone()).await;
    if let Ok(strategy) = strategy_rwl.try_read() {
        match strategy.status {
            StrategyStatus::AwaitingTrades => {
                if let Ok(market_data) = market_data_rwl.try_read() {
                    drop(strategy);
                    if !strategy_rwl.read().await.sent_entry_orders && !signal.0.is_unknown() {
                        send_orders(
                            signal,
                            1,
                            account_data_rwl.clone(),
                            strategy_rwl.clone(),
                            LegType::Entry,
                        )
                        .await
                    }
                    drop(market_data)
                }
            }
            StrategyStatus::SentEntryOrders => {
                if let Ok(account_data) = account_data_rwl.try_read() {
                    if let Some(order_update) = &account_data.order_update {
                        if !account_data.order_id_messages.is_empty()
                            && order_update.is_filled()
                            && order_update.id()
                                == account_data.order_id_messages.last().unwrap().data.order_id
                        {
                            drop(strategy);
                            if let Ok(mut write) = strategy_rwl.try_write() {
                                let index = write.positions.len() - 1;
                                write.positions[index] = write.positions[index]
                                    .clone()
                                    .update_from_order_update(order_update);
                                write.status = StrategyStatus::InATrade;
                                program_log(
                                    &format!("Strategy Status -> {:#?}", StrategyStatus::InATrade),
                                    ResourceType::Trading,
                                    report_rwl.clone(),
                                )
                                .await;
                                drop(write);
                            }
                        }
                    }
                    drop(account_data);
                }
            }
            StrategyStatus::InATrade => {
                if let Ok(market_data) = market_data_rwl.try_read() {
                    drop(strategy);
                    let strategy = strategy_rwl.read().await;
                    if strategy.is_opposite(signal.0) {
                        drop(strategy);
                        send_orders(
                            signal,
                            1,
                            account_data_rwl.clone(),
                            strategy_rwl.clone(),
                            LegType::Exit,
                        )
                        .await;
                        program_log(
                            &format!("Strategy Status -> {:#?}", strategy_rwl.read().await.status),
                            ResourceType::Trading,
                            report_rwl.clone(),
                        )
                        .await;
                        return
                    } 
                    let position = strategy.positions.last().unwrap().clone();
                    let contract = market_data
                        .contract_ids
                        .iter()
                        .find(|contract| contract.id == position.contract_id)
                        .unwrap();
                    if let Some(quote) =
                        &market_data.time_and_sales.get_quote(contract.historical_id)
                    {
                        let pnl = position.unrealized_pnl(quote, contract.big_point_value);
                        if pnl < -TRADE_CLOSE_TRIGGER && !strategy.sent_exit_orders {
                            drop(strategy);
                            if let Ok(mut write) = strategy_rwl.try_write() {
                                if let Ok(mut data) = account_data_rwl.try_write() {
                                    program_log(
                                        &format!("Unrealized Pnl {}", pnl),
                                        ResourceType::Trading,
                                        report_rwl.clone(),
                                    )
                                    .await;
                                    let request_id = (data.sent_requests.len() + 2) as i64;
                                    let request = position.create_closing_ticket(request_id);
                                    if !write.sent_exit_orders {
                                        data.unsent_requests.push(request);
                                    }
                                    write.sent_exit_orders = true;
                                    let index = write.positions.len()-1;
                                    write.positions[index].exit_trigger = format!("Unrealized Pnl = {pnl}");
                                    write.status = StrategyStatus::SentExitOrders;
                                    drop(write);
                                    program_log(
                                        &format!(
                                            "Strategy Status -> {:#?}",
                                            StrategyStatus::SentExitOrders
                                        ),
                                        ResourceType::Trading,
                                        report_rwl.clone(),
                                    )
                                    .await;
                                    drop(data);
                                }
                            };
                        };
                    };
                    drop(market_data);
                };
            }
            StrategyStatus::SentExitOrders => {
                if let Ok(mut data) = account_data_rwl.try_write() {
                    if let Some(order_update) = data.order_update.clone() {
                        if !data.order_id_messages.is_empty()
                            && order_update.is_filled() 
                            && order_update.id() == data.order_id_messages.last().unwrap().data.order_id
                        {
                            drop(strategy);
                            if let Ok(mut write) = strategy_rwl.try_write() {
                                let index = write.positions.len() - 1;
                                if write.positions[index].entry_fill.as_ref().unwrap()
                                    != &order_update.data.entity
                                {
                                    data.order_update = None;
                                    write.status = StrategyStatus::AwaitingTrades;
                                    let index = write.positions.len()-1;
                                    program_log(
                                        &format!(
                                            "Strategy Status -> {:#?}",
                                            StrategyStatus::AwaitingTrades
                                        ),
                                        ResourceType::Trading,
                                        report_rwl.clone(),
                                    )
                                    .await;
                                    write.sent_entry_orders = false;
                                    write.sent_exit_orders = false;
                                    let big_point_value = market_data_rwl
                                        .read()
                                        .await
                                        .contract_ids
                                        .iter()
                                        .find(|contract| {
                                            contract.id
                                                == write.positions.last().unwrap().contract_id
                                        })
                                        .unwrap()
                                        .big_point_value;
                                    let closed_position = write.positions[index].clone().close(
                                        &order_update,
                                        big_point_value,
                                        SINGLE_LEG_COMMISSION,
                                    );
                                    program_log(
                                        &format!(
                                            "Trade Realized Pnl {}",
                                            closed_position.realized_pnl
                                        ),
                                        ResourceType::Trading,
                                        report_rwl.clone(),
                                    )
                                    .await;
                                    write.positions[index] = closed_position;
                                }
                                drop(write);
                                drop(data);
                                return;
                            }
                        }
                    }
                    drop(data);
                }
            }
            StrategyStatus::Unaware => {
                drop(strategy);
                let mut write = strategy_rwl.write().await;
                if let Some(user_data) = &account_data_rwl.read().await.user_data {
                    if !user_data.is_flat() {
                        let market_position = user_data
                            .data
                            .positions
                            .last()
                            .unwrap()
                            .to_market_position();
                        program_log(
                            &format!("Account has position {:#?}", market_position),
                            ResourceType::Trading,
                            report_rwl.clone(),
                        )
                        .await;
                        write.positions.push(market_position);
                        program_log(
                            &format!("Strategy Status -> {:#?}", StrategyStatus::InATrade),
                            ResourceType::Trading,
                            report_rwl.clone(),
                        )
                        .await;
                        write.status = StrategyStatus::InATrade;
                    } else {
                        program_log(
                            &"Account is flat".to_string(),
                            ResourceType::Trading,
                            report_rwl.clone(),
                        )
                        .await;
                        program_log(
                            &format!("Strategy Status -> {:#?}", StrategyStatus::AwaitingTrades),
                            ResourceType::Trading,
                            report_rwl.clone(),
                        )
                        .await;
                        write.status = StrategyStatus::AwaitingTrades
                    }
                };
                drop(write);
            }
        }
    }
}

pub fn create_market_order_ticket(order_action: OrderAction, qty: i64, acct_id: i64) -> Value {
    json!({
        "action": order_action,
        "symbol": TRADING_SYMBOL,
        "orderQty": qty,
        "orderType": "Market",
        "accountSpec": USERNAME,
        "accountId": acct_id,
        "isAutomated": true
    })
}

pub fn create_order_request(request_id: i64, order_ticket: Value) -> String {
    format!("order/placeorder\n{}\n\n{}", request_id, order_ticket)
}

#[derive(Debug)]
pub enum StrategyStatus {
    Unaware,
    AwaitingTrades,
    SentEntryOrders,
    InATrade,
    SentExitOrders,
}

async fn generate_signal(
    market_data: crate::socket_processing::MarketData,
    strategy_rwl: StrategyRWL,
) -> (OrderAction, String) {
    if let Ok(mut strategy) = strategy_rwl.try_write() {
        let size_trigger = if let Some(item) = &strategy.largest_single_item {
            std::cmp::max(item.qty, LARGE_TRADE)
        } else {
            LARGE_TRADE
        };
        let now = chrono::Utc::now().timestamp_millis() - ONE_SECOND;
        if let Some(large_item) = market_data
            .time_and_sales
            .items
            .iter()
            .rev()
            .find(|item| item.qty >= size_trigger && item.timestamp >= now)
        {
            if large_item.action != OrderAction::Unknown {
                return (large_item.action, format!("Large Trade {}", serde_json::to_string_pretty(large_item).unwrap()));
            }
        }
        let mut dom_summary = DOMSummary::new();
        for dom in &market_data.market_depth {
            dom_summary = dom_summary.sum(dom.combine());
        }
        let ts_pressure = market_data.time_and_sales.get_pressure();
        let ts_pressure_threshold =
            if TS_PRESSURE_THRESHOLD > strategy.ts_pressure_extreme.net.abs() {
                TS_PRESSURE_THRESHOLD
            } else {
                strategy.ts_pressure_extreme.net.abs()
            };
        let buy_signal = ts_pressure.net > ts_pressure_threshold && dom_summary.net > DOM_THRESHOLD;
        let sell_signal =
            ts_pressure.net < -ts_pressure_threshold && dom_summary.net < -DOM_THRESHOLD;
        if ts_pressure.net.abs() > strategy.ts_pressure_extreme.net.abs() {
            strategy.ts_pressure_extreme = ts_pressure.clone()
        }
        if dom_summary.net.abs() > strategy.dom_extreme.1.net.abs() {
            strategy.dom_extreme = (dom_summary.get_time(), dom_summary.clone())
        }
        let signal_string = format!("{} \n{}", serde_json::to_string_pretty(&ts_pressure).unwrap(), serde_json::to_string_pretty(&dom_summary).unwrap());
        drop(strategy);
        if buy_signal {
            (
                OrderAction::Buy,
                signal_string
            )
        } else if sell_signal {
            (
                OrderAction::Sell,
                signal_string
            )
        } else {
            (OrderAction::Unknown, String::new())
        }
    } else {
        (OrderAction::Unknown, String::new())
    }
}

pub async fn send_orders(
    action: (OrderAction, String),
    amount: i64,
    account_data_rwl: AcctCommChannelRWL,
    strategy_rwl: StrategyRWL,
    leg_type: LegType,
) {
    if let Ok(mut open) = account_data_rwl.try_write() {
        if let Some(user_data) = &open.user_data {
            if let Ok(mut write) = strategy_rwl.try_write() {
                let ticket = create_market_order_ticket(
                    action.0,
                    amount,
                    user_data
                        .data
                        .accounts
                        .iter()
                        .find(|acct| acct.name == ACCOUNT_NUMBER)
                        .unwrap()
                        .id,
                );
                let id = (open.sent_requests.len() + 1) as i64;
                let request = create_order_request(id, ticket);
                match leg_type {
                    LegType::Entry => {
                        if !write.sent_entry_orders {
                            open.unsent_requests.push(request);
                            write.sent_entry_orders = true;
                            write.status = StrategyStatus::SentEntryOrders;
                            write
                                .positions
                                .push(MarketPosition::new_with_trigger(action.1))
                        }
                    }
                    LegType::Exit => {
                        if !write.sent_exit_orders {
                            open.unsent_requests.push(request);
                            write.status = StrategyStatus::SentExitOrders;
                            let index = write.positions.len() - 1;
                            write.sent_exit_orders = true;
                            write.positions[index].exit_trigger = action.1;
                        }
                    }
                }
                drop(write);
            }
        }
        drop(open);
        return;
    }
}


pub enum LegType {
    Entry,
    Exit,
}
