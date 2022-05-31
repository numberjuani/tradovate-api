use crate::{
    api_client::ResourceType,
    md_socket::{MarketDataIncomingRWL, MarketDataOutGoingRWL, MarketDataRWL},
    models::{ContractID, DOMMessage, QuoteMessage, TickChart, WebsocketResponse},
    time_and_sales::TimeAndSalesCollection, reporting::{program_log, ReportRWL},
};
use serde_json::Value;

#[derive(Debug,Clone)]
pub struct MarketData {
    pub quote: Option<QuoteMessage>,
    pub market_depth: Vec<DOMMessage>,
    pub time_and_sales: TimeAndSalesCollection,
    pub contract_ids: Vec<ContractID>,
}
impl MarketData {
    pub fn new(contract_ids: Vec<ContractID>) -> Self {
        Self {
            quote: None,
            market_depth: Vec::new(),
            time_and_sales: TimeAndSalesCollection::new(),
            contract_ids,
        }
    }
    pub fn summarize_ts(&self) -> String {
        let mut output = String::new();
        if let Some(largest_buy) = self.time_and_sales.largest_buy() {
            if let Some(contract) = self
                .contract_ids
                .iter()
                .find(|contract| contract.historical_id == largest_buy.historical_id)
            {
                output.push_str(&format!(
                    "{}-{} Largest Buy {:#?} \n",
                    contract.symbol,
                    largest_buy.get_normal_time(),
                    largest_buy
                ));
            }
        }
        if let Some(largest_sell) = self.time_and_sales.largest_sell() {
            if let Some(contract) = self
                .contract_ids
                .iter()
                .find(|contract| contract.historical_id == largest_sell.historical_id)
            {
                output.push_str(&format!(
                    "{}-{} Largest Sell {:#?}",
                    contract.symbol,
                    largest_sell.get_normal_time(),
                    largest_sell
                ));
            }
        }
        output
    }
}
pub async fn process_message(
    message: String,
    market_data_incoming_rwl: MarketDataIncomingRWL,
    market_data_outgoing_rwl: MarketDataOutGoingRWL,
    market_data_rwl: MarketDataRWL,
    resource_type: ResourceType,
    report_rwl:ReportRWL
) {
    let message: Result<Value, serde_json::Error> = serde_json::from_str(&message);
    match &message {
        Ok(json) => match json {
            Value::Array(array) => {
                for json in array {
                    if let Value::Object(obj) = json {
                        if obj.contains_key("s") {
                            if obj.keys().len() == 2 {
                                if let Some(s_value) = obj["s"].as_i64() {
                                    if let Some(i_value) = obj["i"].as_i64() {
                                        if i_value == 1 && s_value == 200 {
                                            let mut open = market_data_incoming_rwl.write().await;
                                            open.is_authorized = true;
                                            drop(open);
                                            program_log("Succesfully authorized", resource_type,report_rwl.clone()).await;
                                            return;
                                        }
                                    }
                                }
                            } else {
                                let parsed: Result<WebsocketResponse, serde_json::Error> =
                                    serde_json::from_value(json.clone());
                                if let Ok(ws_response) = parsed {
                                    if ws_response.data.historical_id > 0 {
                                        let mut market_data_open = market_data_rwl.write().await;
                                        let outgoing_open = market_data_outgoing_rwl.read().await;
                                        let symbol = &outgoing_open.data_requests
                                            [(ws_response.request_id - 2) as usize]
                                            .symbol;
                                        for n in 0..market_data_open.contract_ids.len() {
                                            if market_data_open.contract_ids[n].symbol == *symbol {
                                                market_data_open.contract_ids[n].historical_id =
                                                    ws_response.data.historical_id;
                                            }
                                        }
                                        drop(market_data_open);
                                        drop(outgoing_open);
                                    }
                                    let mut open = market_data_incoming_rwl.write().await;
                                    open.request_responses.push(ws_response);
                                    drop(open);
                                    return;
                                }
                            }
                        } else if let Some(e_contens) = obj["e"].as_str() {
                            if matches!(e_contens, "md" | "chart") {
                                if let Some(obj) = obj["d"].as_object() {
                                    if let Some(key) = obj.keys().next() {
                                        match key.as_ref() {
                                            "charts" => {
                                                let parsed: Result<
                                                    TickChart,
                                                    serde_json::Error,
                                                > = serde_json::from_value(json.clone());
                                                match parsed {
                                                    Ok(tick_chart) => {
                                                        let mut ts_items =
                                                            tick_chart.get_all_ts_items();
                                                        ts_items
                                                            .sort_by_key(|item| item.timestamp);
                                                        let mut open =
                                                            market_data_rwl.write().await;
                                                        open.time_and_sales
                                                            .items
                                                            .extend(ts_items);
                                                        drop(open);
                                                        return;
                                                    }

                                                    Err(e) => program_log(
                                                        &format!(
                                                            "error parsing tick chart: {}",
                                                            e
                                                        ),
                                                        resource_type,report_rwl.clone()).await,
                                                }
                                            }
                                            "quotes" => {
                                                let parsed: Result<
                                                    QuoteMessage,
                                                    serde_json::Error,
                                                > = serde_json::from_value(json.clone());
                                                match parsed {
                                                    Ok(qt_msg) => {
                                                        let mut open =
                                                            market_data_rwl.write().await;
                                                        open.quote = Some(qt_msg);
                                                        drop(open);
                                                        return;
                                                    }

                                                    Err(e) => program_log(
                                                        &format!("error parsing quote {}", e),
                                                        resource_type,report_rwl.clone()).await,
                                                }
                                            }
                                            "doms" => {
                                                let parsed: Result<
                                                    DOMMessage,
                                                    serde_json::Error,
                                                > = serde_json::from_value(json.clone());
                                                match parsed {
                                                    Ok(dom_msg) => {
                                                        let mut open =
                                                            market_data_rwl.write().await;
                                                        if let Some(position) = open
                                                            .market_depth
                                                            .iter()
                                                            .position(|dom| {
                                                                dom.data.doms[0].contract_id
                                                                    == dom_msg.data.doms[0]
                                                                        .contract_id
                                                            })
                                                        {
                                                            open.market_depth
                                                                .swap_remove(position);
                                                        }
                                                        open.market_depth.push(dom_msg);
                                                        drop(open);
                                                        return;
                                                    }
                                                    Err(e) => program_log(
                                                        &format!("error parsing dom {}", e),
                                                        resource_type,report_rwl.clone()).await,
                                                }
                                            }
                                            _ => program_log(
                                                &format!("unrecognized message: {}", json),
                                                resource_type,report_rwl.clone()).await,
                                        }
                                    }
                                }

                                program_log(
                                    &format!("unrecognized message: {}", json),
                                    resource_type,report_rwl.clone()).await
                            }
                        }
                    }
                }
            }
            a => {
                program_log(
                    &format!("unrecognized message: {}", a),
                    resource_type,report_rwl.clone()).await
            }
        },
        Err(e) => program_log(
            &format!(
                "could not parse message to json. Error:{:#?} \nmessage {:#?}",
                e, message
            ),
            resource_type,report_rwl.clone()).await,
    }
}
