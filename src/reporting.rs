use std::{sync::Arc, io::Write};
use lettre::{AsyncSmtpTransport, AsyncTransport, Tokio1Executor};
use serde_json::{Value, json};
pub type ReportRWL = Arc<tokio::sync::RwLock<String>>;
use crate::{
    api_client::ResourceType,
    credentials::{ASID, EMAIL_PASSWORD, EMAIL_USERNAME, TWILIO_AT, TWILIO_NUMBER},
    settings::{TWILIO_URL, TIMEZONE, LOG_FILENAME, DEBUG_MODE},
};

pub async fn program_log(item: &str, resource_type: ResourceType, report_rwl:ReportRWL) {
    let time = chrono::Utc::now().with_timezone(&TIMEZONE).to_rfc2822();
    let report_line = format!("{} - {:#?} - {}\n", time, resource_type, item);
    if DEBUG_MODE {println!("{}",report_line)};
    let mut write = report_rwl.write().await;
    write.push_str(&report_line);
    drop(write);
}

pub async fn send_txt_message(number: &str, contents: &str) -> Result<Value, reqwest::Error> {
    if DEBUG_MODE {return Ok(json!({"debug":true}))}
    let encoded_str = format!(
        "From={}&To={}&Body={}",
        urlencoding::encode(TWILIO_NUMBER),
        urlencoding::encode(number),
        urlencoding::encode(contents)
    );
    let url = format!("{}/Accounts/{}/Messages.json", TWILIO_URL, ASID);
    Ok(reqwest::Client::new()
        .post(url)
        .header("content-Type", "application/x-www-form-urlencoded")
        .basic_auth(ASID, Some(TWILIO_AT))
        .body(encoded_str)
        .send()
        .await?
        .json::<Value>()
        .await?)
}

pub async fn send_email(recipient_email: &str, email_body: &str, subject: &str) {
    let to = format!("Trader <{}>", recipient_email);
    let time = chrono::Utc::now();
    let pst_timestamp: String = time
        .with_timezone(&chrono_tz::US::Pacific)
        .format("%F %T")
        .to_string();
    let stamped = format!("{pst_timestamp} PST\n{email_body}");
    let email = lettre::Message::builder()
        .from("Tradovate App <your@email.com>".parse().unwrap())
        .to(to.parse().unwrap())
        .subject(subject)
        .body(stamped.to_string())
        .unwrap();
    let creds = lettre::transport::smtp::authentication::Credentials::new(
        EMAIL_USERNAME.to_string(),
        EMAIL_PASSWORD.to_string(),
    );
    let mailer: AsyncSmtpTransport<Tokio1Executor> =
        AsyncSmtpTransport::<Tokio1Executor>::starttls_relay("mail.smtp.com")
            .unwrap()
            .credentials(creds)
            .build();

    if !DEBUG_MODE {
        if let Err(e) = mailer.send(email).await {
            eprint!("{}",e)
        }
    } 
}

pub fn print_contents(report_rwl:ReportRWL) {
    if let Ok(mut write) = report_rwl.try_write() {
        match std::fs::OpenOptions::new()
            .append(true)
            .open(LOG_FILENAME)
        {
            Ok(mut file) => {
                file.write_all(write.as_bytes()).unwrap();
                write.clear();
            }
            Err(_) => {
                let mut newfile =
                    std::fs::File::create(LOG_FILENAME).unwrap();
                newfile.write_all(write.as_bytes()).unwrap();
            }
        }
    }
}