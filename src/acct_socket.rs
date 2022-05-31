use futures_util::SinkExt;
use serde_json::Value;
use tokio_tungstenite::tungstenite::{Error, Message};

use crate::{
    api_client::ResourceType,
    md_socket::WriteSocket,
    models::{AcctCommChannelRWL, OrderIDMessage, OrderUpdateMessage, UserDataMessage},
    reporting::{program_log, send_email, ReportRWL},
    settings::REPORT_TO_EMAIL,
};

pub async fn incoming_acct_socket(
    acct_comms_channel: AcctCommChannelRWL,
    resource_type: ResourceType,
    msg: Result<Message, Error>,
    report_rwl: ReportRWL,
) -> bool {
    if let Ok(message) = msg {
        match message {
            Message::Text(mut message) => {
                if let Some(first_char) = message.chars().next() {
                    match first_char {
                        'o' => {
                            program_log(
                                "Opening Socket Connection...",
                                resource_type,
                                report_rwl.clone(),
                            )
                            .await;
                            match acct_comms_channel.try_write() {
                                Ok(mut open) => {
                                    open.connection_established = true;
                                }
                                Err(_) => {
                                    program_log(
                                        "could not open rwlock for opening frame",
                                        resource_type,
                                        report_rwl.clone(),
                                    )
                                    .await
                                }
                            }
                        }
                        'h' => {}
                        'a' => {
                            message.remove(0);
                            let as_value: Result<Value, serde_json::Error> =
                                serde_json::from_str(&message);
                            match as_value {
                                Ok(as_value) => {
                                    if let Some(array) = as_value.as_array() {
                                        for json in array {
                                            if let Some(obj) = json.as_object() {
                                                if obj.keys().len() == 2 {
                                                    if obj.contains_key("i")
                                                        && obj.contains_key("s")
                                                    {
                                                        if let Some(i_value) = obj["i"].as_i64() {
                                                            if let Some(s_value) = obj["s"].as_i64()
                                                            {
                                                                if i_value == 1 && s_value == 200 {
                                                                    match acct_comms_channel.try_write() {
                                                                        Ok(mut open) => {
                                                                            open.is_authorized = true;
                                                                            program_log(
                                                                                "Succesfully authorized",
                                                                                resource_type,report_rwl.clone()).await;
                                                                        }
                                                                        Err(_) => program_log(
                                                                            "could not open rwlock for auth",
                                                                            resource_type,report_rwl.clone()).await,
                                                                    }
                                                                }
                                                            }
                                                        }
                                                    } else {
                                                        let parsed: Result<
                                                            OrderUpdateMessage,
                                                            serde_json::Error,
                                                        > = serde_json::from_value(json.clone());
                                                        if let Ok(order_update) = parsed {
                                                            match acct_comms_channel.try_write() {
                                                                Ok(mut open) => {
                                                                    open.order_update = Some(order_update);
                                                                    program_log("Succesfully received order update",resource_type,report_rwl.clone()).await;
                                                                }
                                                                Err(_) => program_log(
                                                                    "could not write to rwlock for order update message",
                                                                    resource_type,report_rwl.clone()).await,
                                                            }
                                                        }
                                                    }
                                                } else {
                                                    let parsed: Result<
                                                        OrderIDMessage,
                                                        serde_json::Error,
                                                    > = serde_json::from_value(json.clone());
                                                    if let Ok(order_id) = parsed {
                                                        match acct_comms_channel.try_write() {
                                                            Ok(mut open) => {
                                                                program_log(
                                                                    &format!(
                                                                        "Succesfully received order id {:#?}",
                                                                        order_id
                                                                    ),
                                                                    resource_type,report_rwl.clone()).await;
                                                                open.order_id_messages.push(order_id);
                                                                drop(open);
                                                                return false
                                                            }
                                                            Err(_) => program_log(
                                                                "could not write to rwlock for order id message",
                                                                resource_type,report_rwl.clone()).await,
                                                        }
                                                    }
                                                    let parsed: Result<
                                                        UserDataMessage,
                                                        serde_json::Error,
                                                    > = serde_json::from_value(json.clone());
                                                    if let Ok(user_data) = parsed {
                                                        match acct_comms_channel.try_write() {
                                                            Ok(mut open) => {
                                                                program_log(
                                                                    &format!(
                                                                        "Succesfully received user data\n{:#?}",
                                                                        user_data.data.positions
                                                                    ),
                                                                    resource_type,report_rwl.clone()).await;
                                                                open.user_data = Some(user_data);
                                                                drop(open);
                                                                return false
                                                            }
                                                            Err(_) => program_log(
                                                                "could not write to rwlock for user data message",
                                                                resource_type,report_rwl.clone()).await,
                                                        }
                                                    }
                                                    program_log(
                                                        &format!("unrecognized message: {}", json),
                                                        resource_type,
                                                        report_rwl.clone(),
                                                    )
                                                    .await
                                                }
                                            }
                                        }
                                    }
                                }
                                Err(e) => {
                                    program_log(
                                        &format!(
                                            "Error {} creating json value from message:  {:#?}",
                                            e, message
                                        ),
                                        resource_type,
                                        report_rwl.clone(),
                                    )
                                    .await
                                }
                            }
                        }
                        'c' => {}
                        _ => {}
                    }
                }
            }
            Message::Close(msg) => {
                acct_comms_channel.write().await.received_closing_frame = true;
                program_log(
                    &format!("Received closing frame {:#?}", msg),
                    resource_type,
                    report_rwl.clone(),
                )
                .await;
                send_email(
                    REPORT_TO_EMAIL,
                    "Trading Connection was closed",
                    "Account Socket Closed",
                )
                .await;
                return true;
            }
            _ => return false,
        }
    }
    return false;
}

pub async fn outgoing_acct_socket(
    comms_channel: AcctCommChannelRWL,
    mut pwrite: WriteSocket,
    resource_type: ResourceType,
    report_rwl: ReportRWL,
) -> WriteSocket {
    if let Ok(mut open) = comms_channel.try_write() {
        if open.received_closing_frame {
            return pwrite;
        }
        if open.last_heart_beat_instant.elapsed().as_millis() >= 2500
            && !open.terminated
            && !open.received_closing_frame
        {
            match pwrite.send(Message::Text("[]".to_string())).await {
                Ok(_) => {
                    open.last_heart_beat_instant = tokio::time::Instant::now();
                }
                Err(e) => {
                    program_log(
                        &format!("could not send hearbeat frame {}", e),
                        resource_type,
                        report_rwl.clone(),
                    )
                    .await;
                    return pwrite;
                }
            }
        }
        if open.connection_established && !open.is_authorized && !open.unsent_requests.is_empty() {
            match pwrite
                .send(Message::Text(open.unsent_requests[0].clone()))
                .await
            {
                Ok(_) => {
                    let item = open.unsent_requests.swap_remove(0);
                    open.sent_requests.push(item);
                }
                Err(e) => {
                    program_log(
                        &format!("could not send auth request {}", e),
                        resource_type,
                        report_rwl.clone(),
                    )
                    .await
                }
            }
        }
        if open.is_authorized && !open.unsent_requests.is_empty() {
            match pwrite
                .send(Message::Text(open.unsent_requests[0].clone()))
                .await
            {
                Ok(_) => {
                    let item = open.unsent_requests.swap_remove(0);
                    open.sent_requests.push(item);
                }
                Err(e) => {
                    program_log(
                        &format!("could not send auth request {}", e),
                        resource_type,
                        report_rwl.clone(),
                    )
                    .await
                }
            }
        }
        if open.socket_close {
            open.socket_close = false;
            match pwrite.send(Message::Close(None)).await {
                Ok(_) => {
                    program_log(
                        "succesfully sent closing message",
                        resource_type,
                        report_rwl.clone(),
                    )
                    .await
                }
                Err(_) => {
                    program_log(
                        "error sending closing message",
                        resource_type,
                        report_rwl.clone(),
                    )
                    .await
                }
            }
            return pwrite;
        }
    }
    pwrite
}
