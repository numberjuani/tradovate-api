mod time_and_sales;
use crate::{
    acct_socket::{incoming_acct_socket, outgoing_acct_socket},
    api_client::{Protocol, ResourceType, TradovateAPIClient, Server},
    md_socket::{
        process_market_data_messages, process_outgoing_md_messages, MarketDataIncoming,
        MarketDataRequest,
    },
    models::AcctCommsChannel,
    others::get_duration_until_open,
    reporting::program_log,
    settings::{REPORT_TO_EMAIL, SOCKET_INTERVAL_SPEED, TIMEZONE, TRADING_SYMBOL},
    strategy::calculate,
};
use core::time;
use futures_util::StreamExt;
use md_socket::{APIClientRWL, MarketDataIncomingRWL, MarketDataOutGoingRWL, MarketDataRWL};
use models::AcctCommChannelRWL;
use reporting::ReportRWL;
use std::sync::Arc;
use strategy::StrategyRWL;
use tokio::signal::unix::{signal, SignalKind};
use tokio_stream::wrappers::{IntervalStream, SignalStream};
use tokio_tungstenite::connect_async;
mod acct_socket;
mod api_client;
mod credentials;
mod md_socket;
mod models;
mod others;
mod reporting;
mod settings;
mod socket_processing;
mod strategy;
use crate::md_socket::MarketData;
#[tokio::main]
async fn main() {
    use MarketData::*;
    let strategy_rwl = Arc::new(tokio::sync::RwLock::new(strategy::Strategy::new()));
    let report_rwl = Arc::new(tokio::sync::RwLock::new(String::new()));
    let mut fast_attempts = 0;
    loop {
        if let Ok(client) = TradovateAPIClient::new(Server::Demo, "MyApp", "0.2").get_auth().await {
            let md_url = client.url(ResourceType::MarketData, Protocol::Wss);
            let account_url = client.url(ResourceType::Trading, Protocol::Wss);
            let api_client_rwl = Arc::new(tokio::sync::RwLock::new(client.clone()));
            let market_data = Arc::new(tokio::sync::RwLock::new(
                socket_processing::MarketData::new(vec![
                    client.get_contract_info("ESM2").await.unwrap(),
                    client.get_contract_info("NQM2").await.unwrap(),
                ]),
            ));
            let md_requests = vec![
                MarketDataRequest::new(Chart, "ESM2"),
                MarketDataRequest::new(Chart, "NQM2"),
                MarketDataRequest::new(DepthOfMarket, "ESM2"),
                MarketDataRequest::new(DepthOfMarket, "NQM2"),
            ];
            let acct_requests = vec![
                client.ws_auth_msg(ResourceType::Trading).await,
                client.get_user_sync_request(),
            ];
            let market_data_outgoing_rwl = Arc::new(tokio::sync::RwLock::new(
                md_socket::MarketDataOutgoing::new(md_requests),
            ));
            let market_data_incoming_rwl =
                Arc::new(tokio::sync::RwLock::new(MarketDataIncoming::new()));
            let acct_comms_channel = Arc::new(tokio::sync::RwLock::new(AcctCommsChannel::new(
                acct_requests,
            )));
            let time_to_wait = get_duration_until_open();
            if time_to_wait != time::Duration::ZERO {
                program_log(
                    &format!(
                        "Waiting {} seconds for market to open",
                        time_to_wait.as_secs()
                    ),
                    ResourceType::Any,
                    report_rwl.clone(),
                )
                .await;
            } else {
                program_log(
                    "Market is open connecting now...",
                    ResourceType::Any,
                    report_rwl.clone(),
                )
                .await
            };
            tokio::time::sleep(time_to_wait).await;
            let start = tokio::time::Instant::now();
            if connect_and_trade(
                &md_url,
                &account_url,
                market_data_incoming_rwl.clone(),
                api_client_rwl.clone(),
                market_data_outgoing_rwl.clone(),
                market_data.clone(),
                acct_comms_channel.clone(),
                strategy_rwl.clone(),
                report_rwl.clone(),
            )
            .await
            {
                break;
            }
            if start.elapsed().as_secs() < 60 {
                fast_attempts += 1
            } else {
                fast_attempts = 0
            }
            if fast_attempts >= 3 {
                reporting::send_txt_message("phone-number", "Max connections have been attempted")
                    .await
                    .unwrap();
                program_log(
                    "Max connections have been attempted",
                    ResourceType::Any,
                    report_rwl.clone(),
                )
                .await;
                break;
            }
            program_log(
                "Program finished and restarting",
                ResourceType::MarketData,
                report_rwl.clone(),
            )
            .await;
        } else {program_log("Could not obtain auth", ResourceType::Any, report_rwl.clone()).await}
    }
    program_log("User Aborted", ResourceType::MarketData, report_rwl.clone()).await;
}

pub async fn connect_and_trade(
    md_url: &str,
    account_url: &str,
    market_data_incoming_rwl: MarketDataIncomingRWL,
    api_client_rwl: APIClientRWL,
    market_data_outgoing_rwl: MarketDataOutGoingRWL,
    market_data: MarketDataRWL,
    acct_comms_channel: AcctCommChannelRWL,
    strategy_rwl: StrategyRWL,
    report_rwl: ReportRWL,
) -> bool {
    program_log(
        &format!("connecting to {} {}", md_url, account_url),
        ResourceType::Any,
        report_rwl.clone(),
    )
    .await;
    if let Ok((stream, response)) = connect_async(md_url).await {
        if let Ok((trading_stream, trading_response)) = connect_async(account_url).await {
            program_log(
                &format!("Websocket connection status: {:#?}", response.status()),
                ResourceType::MarketData,
                report_rwl.clone(),
            )
            .await;
            program_log(
                &format!(
                    "Websocket connection status: {:#?}",
                    trading_response.status()
                ),
                ResourceType::MarketData,
                report_rwl.clone(),
            )
            .await;
            let (account_write, account_read) = trading_stream.split();
            let (md_write, md_read) = stream.split();
            let md_read_future = md_read.for_each(|msg| async {
                if process_market_data_messages(
                    market_data_incoming_rwl.clone(),
                    market_data_outgoing_rwl.clone(),
                    market_data.clone(),
                    msg,
                    ResourceType::MarketData,
                    report_rwl.clone(),
                )
                .await
                {
                    return;
                }
            });
            let account_read_future = account_read.for_each(|msg| async {
                if incoming_acct_socket(
                    acct_comms_channel.clone(),
                    ResourceType::Trading,
                    msg,    
                    report_rwl.clone(),
                )
                .await
                {
                    return;
                }
            });
            let interval =
                tokio::time::interval(time::Duration::from_micros(SOCKET_INTERVAL_SPEED));
            let account_write_future =
                IntervalStream::new(interval).fold(account_write, |pmd_write, _| async {
                    outgoing_acct_socket(
                        acct_comms_channel.clone(),
                        pmd_write,
                        ResourceType::Trading,
                        report_rwl.clone(),
                    )
                    .await
                });
            let interval =
                tokio::time::interval(time::Duration::from_micros(SOCKET_INTERVAL_SPEED));
            let md_write_future =
                IntervalStream::new(interval).fold(md_write, |pmd_write, _| async {
                    process_outgoing_md_messages(
                        market_data_incoming_rwl.clone(),
                        market_data_outgoing_rwl.clone(),
                        api_client_rwl.clone(),
                        pmd_write,
                        ResourceType::MarketData,
                        report_rwl.clone(),
                    )
                    .await
                });
            let interval =
                tokio::time::interval(time::Duration::from_micros(SOCKET_INTERVAL_SPEED));
            let strategy_calculations = IntervalStream::new(interval).for_each(|_| async {
                calculate(
                    market_data.clone(),
                    strategy_rwl.clone(),
                    acct_comms_channel.clone(),
                    report_rwl.clone(),
                )
                .await
            });
            let start = tokio::time::Instant::now() + time::Duration::from_secs(60 * 30);
            let interval_at = tokio::time::interval_at(start, time::Duration::from_secs(60 * 120));
            let periodic_reporting = IntervalStream::new(interval_at).for_each(|_| async {
                if let Ok(strategy) = strategy_rwl.try_read() {
                    if let Ok(market_data) = market_data.try_read() {
                        let body = &format!("{:#?}\n{}", strategy, market_data.summarize_ts());
                        reporting::send_email(REPORT_TO_EMAIL, body, "Periodic Report").await;
                    }
                }
            });
            let start = tokio::time::Instant::now() + time::Duration::from_secs(60);
            let interval_at = tokio::time::interval_at(start, time::Duration::from_secs(60));
            let log_file_print = IntervalStream::new(interval_at).for_each(|_| async {
                reporting::print_contents(report_rwl.clone())
            });
            let interval = tokio::time::interval(time::Duration::from_secs(60));
            let access_token_renewal_future = IntervalStream::new(interval).for_each(|_| async {
                let now = chrono::Utc::now().timestamp_millis();
                if let Ok(api_client) = api_client_rwl.try_read() {
                    if now > (api_client.token_expiration_time - 240000) {
                        if let Ok(response) = api_client.renew_access_token().await {
                            drop(api_client);
                            let new_expiration_time = chrono::DateTime::parse_from_rfc3339(
                                response["expirationTime"].as_str().unwrap(),
                            )
                            .unwrap();
                            let pst_expiry = new_expiration_time
                                .with_timezone(&TIMEZONE)
                                .format("%v %T")
                                .to_string();
                            api_client_rwl.write().await.token_expiration_time =
                                new_expiration_time.timestamp_millis();
                            others::delete_file(settings::AUTH_FILENAME);
                            let report =
                                format!("Renewed Access token. Now expiriring at {}", pst_expiry);
                            program_log(&report, ResourceType::Any, report_rwl.clone()).await;
                            reporting::send_txt_message("yourphonenumber", &report)
                                .await
                                .unwrap();
                        }
                    }
                }
            });
            let program_close_future = SignalStream::new(signal(SignalKind::interrupt()).unwrap())
                .for_each(|_| async {
                    program_log("Received Ctrl+C", ResourceType::Any, report_rwl.clone()).await;
                    let mut open = market_data_outgoing_rwl.write().await;
                    open.socket_close = true;
                    open.terminated = true;
                    let mut other_open = acct_comms_channel.write().await;
                    other_open.socket_close = true;
                    other_open.terminated = true;
                    let strat = strategy_rwl.read().await;
                    program_log(
                        &format!("{:#?}", strat),
                        ResourceType::MarketData,
                        report_rwl.clone(),
                    )
                    .await;
                    drop(other_open);
                    drop(open);
                    if let Ok(market_data_open) = market_data.try_read() {
                        if let Some(contract) = market_data_open
                            .contract_ids
                            .iter()
                            .find(|contract| contract.symbol == TRADING_SYMBOL)
                        {
                            if let Some(quote) =
                                market_data_open.time_and_sales.get_quote(contract.id)
                            {
                                program_log(
                                    &strat.summary(quote, contract.big_point_value),
                                    ResourceType::MarketData,
                                    report_rwl.clone(),
                                )
                                .await;
                                program_log(
                                    &market_data_open.summarize_ts(),
                                    ResourceType::MarketData,
                                    report_rwl.clone(),
                                )
                                .await;
                            }
                        }
                    }
                    drop(strat);
                    reporting::print_contents(report_rwl.clone());
                    tokio::time::sleep(time::Duration::from_millis(2000)).await;
                });
            tokio::select! {
                () = md_read_future => false,
                () = account_read_future => false,
                _ = md_write_future => true,
                _ = account_write_future => true,
                _ = program_close_future => true,
                _ = periodic_reporting => false,
                _ = access_token_renewal_future => false,
                _ = log_file_print => false,
                () = strategy_calculations => false,
            };
        } else {
            program_log(
                "Could not connect to account socket",
                ResourceType::Trading,
                report_rwl,
            )
            .await
        };
    } else {
        program_log(
            "Could not connect to market data socket",
            ResourceType::MarketData,
            report_rwl,
        )
        .await
    };
    market_data_outgoing_rwl.read().await.terminated
}
