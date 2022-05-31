#![allow(dead_code)]
use std::sync::Arc;

use crate::{
    api_client::{ResourceType, TradovateAPIClient},
    models::WebsocketResponse,
    reporting::{send_email, program_log, ReportRWL},
    settings::REPORT_TO_EMAIL,
    socket_processing::{self, process_message},
};

use chrono::Duration;
use futures_util::{stream::SplitSink, SinkExt};
use serde_json::{json, Value};
use tokio::net::TcpStream;
use tokio_tungstenite::{
    tungstenite::{Error, Message},
    MaybeTlsStream, WebSocketStream,
};
pub type WriteSocket = SplitSink<WebSocketStream<MaybeTlsStream<TcpStream>>, Message>;
pub type MarketDataIncomingRWL = Arc<tokio::sync::RwLock<MarketDataIncoming>>;
pub type MarketDataOutGoingRWL = Arc<tokio::sync::RwLock<MarketDataOutgoing>>;
pub type MarketDataRWL = Arc<tokio::sync::RwLock<socket_processing::MarketData>>;
pub type APIClientRWL = Arc<tokio::sync::RwLock<TradovateAPIClient>>;
#[derive(Debug, Clone)]
pub struct MarketDataOutgoing {
    pub outgoing_messages: Vec<String>,
    pub last_heart_beat_instant: tokio::time::Instant,
    pub socket_close: bool,
    pub terminated: bool,
    pub data_requests: Vec<MarketDataRequest>,
    pub sent_requests: Vec<String>,
    pub received_closing_frame:bool,
}
impl MarketDataOutgoing {
    pub fn new(data_requests: Vec<MarketDataRequest>) -> Self {
        Self {
            outgoing_messages: Vec::new(),
            last_heart_beat_instant: tokio::time::Instant::now(),
            socket_close: false,
            terminated: false,
            data_requests,
            sent_requests: Vec::new(),
            received_closing_frame: false,
        }
    }
}
#[derive(Debug, Clone)]
pub struct MarketDataIncoming {
    pub connection_established: bool,
    pub is_authorized: bool,
    pub request_responses: Vec<WebsocketResponse>,
}
impl MarketDataIncoming {
    pub fn new() -> Self {
        Self {
            connection_established: false,
            request_responses: Vec::new(),
            is_authorized: false,
        }
    }
}
impl Default for MarketDataIncoming {
    fn default() -> Self {
        Self::new()
    }
}
pub async fn process_market_data_messages(
    market_data_incoming_rwl: MarketDataIncomingRWL,
    market_data_outgoing_rwl: MarketDataOutGoingRWL,
    market_data: MarketDataRWL,
    message: Result<Message, Error>,
    resource_type: ResourceType,
    report_rwl:ReportRWL
) -> bool {
    if let Ok(msg) = message {
        match msg {
            Message::Text(mut textmsg) => {
                if let Some(first_char) = textmsg.chars().next() {
                    match first_char {
                        'o' => {
                            program_log("Opening Socket Connection...", resource_type,report_rwl.clone()).await;
                            let mut open = market_data_incoming_rwl.write().await;
                            open.connection_established = true;
                            drop(open);
                            return false
                        }
                        'a' => {
                            textmsg.remove(0);
                            process_message(
                                textmsg,
                                market_data_incoming_rwl.clone(),
                                market_data_outgoing_rwl.clone(),
                                market_data.clone(),
                                resource_type,
                                report_rwl.clone()
                            )
                            .await;
                            return false
                        }
                        'h' => {
                            return false
                        }
                        a => {
                            program_log(
                                &format!("Unexpected response token received: {:#?}", a),
                                resource_type,report_rwl.clone()).await;
                            return false
                        }
                    }
                } else {return false}
            }
            Message::Close(mgs) => {
                market_data_outgoing_rwl.write().await.received_closing_frame = true;
                program_log(&format!("Received closing frame {:#?}", mgs), resource_type,report_rwl.clone()).await;
                send_email(
                    REPORT_TO_EMAIL,
                    "Market Data Connection was closed",
                    "MD Socket Closed",
                )
                .await;
                return true
                
            }
            a => {
                program_log(
                    &format!("Unexpected response token received: {:#?}", a),
                    resource_type,report_rwl.clone()).await;
                return false
            }
        }
    } 
    return false
}

pub async fn process_outgoing_md_messages(
    market_data_incoming_rwl: MarketDataIncomingRWL,
    market_data_outgoing_rwl: MarketDataOutGoingRWL,
    api_client_rwl: APIClientRWL,
    mut pwrite: WriteSocket,
    resource_type: ResourceType,
    report_rwl:ReportRWL
) -> WriteSocket {
    let mut write = market_data_outgoing_rwl.write().await;
    if write.received_closing_frame {return pwrite}
    if write.last_heart_beat_instant.elapsed().as_millis() >= 2500 && !write.terminated && !write.received_closing_frame {
        match pwrite.send(Message::Text("[]".to_string())).await {
            Ok(_) => {
                write.last_heart_beat_instant = tokio::time::Instant::now();
            }
            Err(e) => {
                program_log(
                    &format!("could not send hearbeat frame {}", e),
                    resource_type,report_rwl.clone()).await;
                return pwrite;
            }
        }
    }
    if !write.outgoing_messages.is_empty() && !write.terminated {
        for message in &write.outgoing_messages {
            if let Err(e) = pwrite.send(Message::Text(message.to_string())).await {
                program_log(
                    &format!("could not send message.Error {}", e),
                    resource_type,report_rwl.clone()).await;
            }
        }
        write.outgoing_messages.clear();
    }
    if let Ok(read) = market_data_incoming_rwl.try_read() {
        if read.connection_established && write.sent_requests.is_empty() {
            if let Ok(api_client) = api_client_rwl.try_read() {
                let auth = api_client.ws_auth_msg(resource_type).await;
                match pwrite.send(Message::Text(auth.clone())).await {
                    Ok(_) => write.sent_requests.push(auth),
                    Err(e) => program_log(
                        &format!("could not send hearbeat frame {}", e),
                        resource_type,report_rwl.clone()).await,
                }
            }
        }
        if read.is_authorized && write.sent_requests.len() < (write.data_requests.len() + 2) {
            for n in 0..write.data_requests.len() {
                if write.data_requests[n].is_unsent() {
                    let request_string = write.data_requests[n].subscribe((n + 2) as i32);
                    match pwrite.send(Message::Text(request_string.clone())).await {
                        Ok(_) => {
                            program_log(
                                &format!(
                                    "{} succesfully sent data request message",
                                    write.data_requests[n].summarize()
                                ),
                                resource_type,report_rwl.clone()).await;
                            write.data_requests[n].status = RequestStatus::Sent;
                            write.sent_requests.push(request_string)
                        }
                        Err(e) => program_log(
                            &format!(
                                "{} error sending data request message Error: {}",
                                write.data_requests[n].summarize(),
                                e
                            ),
                            resource_type,report_rwl.clone()).await,
                    }
                }
            }
        }
    }
    // AFTER USER HITS CTRL C
    if write.socket_close {
        write.socket_close = false;
        for n in 0..write.data_requests.len() {
            if write.data_requests[n].is_sent() {
                match pwrite
                    .send(Message::Text(
                        write.data_requests[n].unsubscribe((n + 2) as i32),
                    ))
                    .await
                {
                    Ok(_) => {
                        program_log(
                            &format!(
                                "{} succesfully sent data unsubscribe request message",
                                write.data_requests[n].summarize()
                            ),
                            resource_type,report_rwl.clone()).await;
                        write.data_requests[n].status = RequestStatus::Canceled;
                    }
                    Err(e) => program_log(
                        &format!(
                            "{} error sending data unsubscribe request message Error: {}",
                            write.data_requests[n].summarize(),
                            e
                        ),
                        resource_type,report_rwl.clone()).await,
                }
            }
        }
        match pwrite.send(Message::Close(None)).await {
            Ok(_) => program_log("succesfully sent closing message", resource_type,report_rwl.clone()).await,
            Err(_) => program_log("error sending closing message", resource_type,report_rwl.clone()).await,
        }
        drop(write);
        return pwrite;
    }
    drop(write);
    pwrite
}

#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub enum MarketData {
    DepthOfMarket,
    Quote,
    Histogram,
    Chart,
}

#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub enum RequestStatus {
    Sent,
    Unsent,
    Canceled,
}
#[derive(Debug, Clone, PartialEq, PartialOrd)]
pub struct MarketDataRequest {
    pub data_type: MarketData,
    pub symbol: String,
    pub status: RequestStatus,
    pub contract_id: i64,
    pub historical_id: i64,
}
impl MarketDataRequest {
    pub fn new(data_type: MarketData, symbol: &str) -> Self {
        Self {
            data_type,
            symbol: symbol.to_string(),
            status: RequestStatus::Unsent,
            contract_id: 0,
            historical_id: 0,
        }
    }
    pub fn subscribe(&self, request_id: i32) -> String {
        use MarketData::*;
        let endpoint = match self.data_type {
            DepthOfMarket => "md/subscribeDOM",
            Quote => "md/subscribeQuote",
            Histogram => "md/subscribeHistogram",
            Chart => "md/getChart",
        };
        let request_body = if self.data_type != Chart {
            json!({
                "symbol": self.symbol
            })
        } else {
            get_tick_chart_request_body(&self.symbol)
        };
        format!("{}\n{}\n\n{}", endpoint, request_id, request_body)
    }
    pub fn unsubscribe(&self, request_id: i32) -> String {
        let endpoint = match self.data_type {
            MarketData::DepthOfMarket => "md/unsubscribeDOM",
            MarketData::Quote => "md/unsubscribeQuote",
            MarketData::Histogram => "md/unsubscribeHistogram",
            MarketData::Chart => "md/cancelChart",
        };
        let request_body = if self.data_type != MarketData::Chart {
            json!({
                "symbol": self.symbol
            })
        } else {
            json!({
                "subscriptionId": self.historical_id
            })
        };
        format!("{}\n{}\n\n{}", endpoint, request_id, request_body)
    }
    pub fn is_unsent(&self) -> bool {
        self.status == RequestStatus::Unsent
    }
    pub fn is_sent(&self) -> bool {
        self.status == RequestStatus::Sent
    }
    pub fn summarize(&self) -> String {
        format!("{} {:?}", self.symbol, self.data_type)
    }
}

pub fn get_tick_chart_request_body(symbol: &str) -> Value {
    let time_stamp = chrono::Utc::now() - Duration::minutes(1);
    let formatted = time_stamp.to_rfc3339();
    json!({
      "symbol": symbol,
      "chartDescription": {
        "underlyingType": "Tick",
        "elementSize": 1,
        "elementSizeUnit": "UnderlyingUnits"
      },
      "timeRange": {
          "asFarAsTimestamp": formatted,
      }
    })
}
