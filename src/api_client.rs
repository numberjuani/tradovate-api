use serde_json::{json, Value};

use crate::{
    credentials::{CID, PASSWORD, SECRET, USERNAME},
    models::{ContractID, ProductInfo},
    others::{create_json_file, delete_file, open_json},
    settings::{AUTH_FILENAME, DEMO_TRADING_URL, LIVE_MARKET_DATA_URL, LIVE_TRADING_URL},
};

#[derive(Debug, Clone)]
pub enum Server {
    Live,
    Demo,
}
#[derive(Debug, Clone, Copy)]
pub enum ResourceType {
    Trading,
    MarketData,
    Any,
}
pub enum Protocol {
    Https,
    Wss,
}
impl Protocol {
    pub fn add_prefix(&self, base_url: &str) -> String {
        match self {
            Protocol::Https => format!("https://{}", base_url),
            Protocol::Wss => url::Url::parse(&format!("wss://{}/v1/websocket", base_url))
                .unwrap()
                .to_string(),
        }
    }
}
#[derive(Debug, Clone)]
pub struct TradovateAPIClient {
    pub server_type: Server,
    pub app_id: String,
    pub app_version: String,
    pub device_id: String,
    pub access_token: String,
    pub token_expiration_time: i64,
    pub user_id: i64,
}
impl TradovateAPIClient {
    pub fn new(server_type: Server, app_id: &str, app_version: &str) -> Self {
        Self {
            app_id: app_id.to_string(),
            app_version: app_version.to_string(),
            device_id: machine_uid::get().unwrap(),
            access_token: String::new(),
            token_expiration_time: 0,
            server_type,
            user_id: 0,
        }
    }
    pub async fn get_auth(self) -> Result<Self, reqwest::Error> {
        match open_json(AUTH_FILENAME) {
            Ok(auth_file) => {
                if chrono::Utc::now()
                    >= chrono::DateTime::parse_from_rfc3339(
                        auth_file["expirationTime"].as_str().unwrap(),
                    )
                    .unwrap()
                {
                    delete_file(AUTH_FILENAME);
                    match self.request_token().await {
                        Ok((t1, t2, t3)) => Ok(Self {
                            access_token: t1,
                            token_expiration_time: t2,
                            user_id: t3,
                            ..self
                        }),
                        Err(e) => {
                            Err(e)
                        }
                    }
                } else {
                    Ok(Self {
                        access_token: auth_file["accessToken"].as_str().unwrap().to_string(),
                        token_expiration_time: chrono::DateTime::parse_from_rfc3339(
                            auth_file["expirationTime"].as_str().unwrap(),
                        )
                        .unwrap()
                        .timestamp_millis(),
                        user_id: auth_file["userId"].as_i64().unwrap(),
                        ..self
                    })
                }
            }
            Err(_) => match self.request_token().await {
                Ok((t1, t2, t3)) => Ok(Self {
                    access_token: t1,
                    token_expiration_time: t2,
                    user_id: t3,
                    ..self
                }),
                Err(e) => Err(e),
            },
        }
    }
    pub async fn request_token(&self) -> Result<(String, i64, i64), reqwest::Error> {
        let body = json!({
            "name":       USERNAME,
            "password":   PASSWORD,
            "appId":      self.app_id,
            "appVersion": self.app_version,
            "cid":        CID,
            "sec":        SECRET,
            "deviceId":   self.device_id
        });
        let url = format!(
            "{}/v1/auth/accesstokenrequest",
            self.url(ResourceType::Trading, Protocol::Https)
        );
        let request = reqwest::Client::new()
            .post(url)
            .header("Content-Type", "application/json")
            .header("Accept", "application/json")
            .body(body.to_string());
        let response = request.send().await?;
        let response_body = response.json::<Value>().await?;
        create_json_file(AUTH_FILENAME, &response_body);
        let exp_timestamp =
            chrono::DateTime::parse_from_rfc3339(response_body["expirationTime"].as_str().unwrap())
                .unwrap()
                .timestamp_millis();
        Ok((
            response_body["accessToken"].as_str().unwrap().to_string(),
            exp_timestamp,
            response_body["userId"].as_i64().unwrap(),
        ))
    }
    pub async fn get_products_list(&self) -> Result<Value, reqwest::Error> {
        let response = reqwest::Client::new()
            .get(format!(
                "{}/v1/contract/deps",
                self.url(ResourceType::Trading, Protocol::Https)
            ))
            .bearer_auth(&self.access_token)
            .header("Content-Type", "application/json")
            .header("Accept", "application/json")
            .send()
            .await?;
        let response_body = response.json::<Value>().await?;
        create_json_file("Contracts List.json", &response_body);
        Ok(response_body)
    }
    pub fn url(&self, resource_type: ResourceType, protocol: Protocol) -> String {
        match self.server_type {
            Server::Live => match resource_type {
                ResourceType::Trading => protocol.add_prefix(LIVE_TRADING_URL),
                ResourceType::MarketData => protocol.add_prefix(LIVE_MARKET_DATA_URL),
                ResourceType::Any => todo!(),
            },
            Server::Demo => match resource_type {
                ResourceType::Trading => protocol.add_prefix(DEMO_TRADING_URL),
                ResourceType::MarketData => protocol.add_prefix(LIVE_MARKET_DATA_URL),
                ResourceType::Any => todo!(),
            },
        }
    }
    pub async fn ws_auth_msg(&self, resource_type: ResourceType) -> String {
        match resource_type {
            ResourceType::Trading => format!("authorize\n1\n\n{}", self.access_token),
            ResourceType::MarketData => format!("authorize\n1\n\n{}", self.access_token),
            ResourceType::Any => todo!(),
        }
    }
    pub fn get_user_sync_request(&self) -> String {
        let body = json!({
            "users": [self.user_id],
        });
        format!("user/syncrequest\n2\n\n{}", body)
    }
    async fn get_contract_id(&self, symbol: &str) -> Result<ContractID, reqwest::Error> {
        let query = vec![("name", symbol)];
        Ok(reqwest::Client::new()
            .get(&format!(
                "{}/contract/find",
                self.url(ResourceType::Trading, Protocol::Https)
            ))
            .header("accept", "application/json")
            .bearer_auth(self.access_token.clone())
            .query(&query)
            .send()
            .await?
            .json::<ContractID>()
            .await?)
    }
    async fn get_product_info(&self, symbol: &str) -> Result<ProductInfo, reqwest::Error> {
        let query = vec![("name", symbol)];
        let url = &format!(
            "{}/v1/product/find",
            self.url(ResourceType::Trading, Protocol::Https)
        );
        let request = reqwest::Client::new()
            .get(url)
            .header("Accept", "application/json")
            .bearer_auth(self.access_token.clone())
            .query(&query);
        let response = request.send().await?;
        let body = response.json::<ProductInfo>().await?;
        Ok(body)
    }
    pub async fn get_contract_info(&self, symbol: &str) -> Result<ContractID, reqwest::Error> {
        let mut contract_id = self.get_contract_id(symbol).await?;
        let product_info = self.get_product_info(&symbol[0..symbol.len() - 2]).await?;
        contract_id.big_point_value = product_info.value_per_point;
        Ok(contract_id)
    }
    pub async fn renew_access_token(&self) -> Result<Value, reqwest::Error> {
        let url = &format!(
            "{}/v1/auth/renewaccesstoken",
            self.url(ResourceType::Trading, Protocol::Https)
        );
        let request = reqwest::Client::new()
            .get(url)
            .header("Accept", "application/json")
            .bearer_auth(self.access_token.clone());
        let response = request.send().await?;
        let body = response.json::<Value>().await?;
        Ok(body)
    }
    pub fn token_is_valid(&self) -> bool {
        chrono::Utc::now().timestamp_millis() - self.token_expiration_time > 120000
    }
}
