use crate::errors::AppError;
use base64::Engine;
use serde::Deserialize;
use serde_json::json;

#[derive(Debug, Clone)]
pub struct AccountData {
    pub address: String,
    pub owner: String,
    pub lamports: u64,
    pub data: Vec<u8>,
    pub slot: u64,
}

#[derive(Debug, Clone)]
pub struct RpcClient {
    endpoint: String,
    http: reqwest::Client,
}

impl RpcClient {
    pub fn new(endpoint: impl Into<String>) -> Self {
        Self {
            endpoint: endpoint.into(),
            http: reqwest::Client::new(),
        }
    }

    pub async fn get_multiple_accounts(
        &self,
        addresses: &[String],
    ) -> Result<Vec<AccountData>, AppError> {
        if addresses.is_empty() {
            return Ok(Vec::new());
        }

        let body = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "getMultipleAccounts",
            "params": [
                addresses,
                {
                    "encoding": "base64",
                    "commitment": "confirmed"
                }
            ]
        });

        let response = self
            .http
            .post(&self.endpoint)
            .json(&body)
            .send()
            .await
            .map_err(|error| AppError::Rpc(format!("failed to send getMultipleAccounts: {error}")))?;

        let status = response.status();
        if !status.is_success() {
            return Err(AppError::Rpc(format!("getMultipleAccounts returned HTTP {status}")));
        }

        let parsed: RpcResponse = response
            .json()
            .await
            .map_err(|error| AppError::Rpc(format!("failed to parse getMultipleAccounts JSON: {error}")))?;

        parse_get_multiple_accounts_response(addresses, parsed)
    }
}

pub fn parse_get_multiple_accounts_response(
    addresses: &[String],
    response: RpcResponse,
) -> Result<Vec<AccountData>, AppError> {
    if let Some(error) = response.error {
        return Err(AppError::Rpc(format!(
            "json-rpc error {}: {}",
            error.code, error.message
        )));
    }

    let result = response
        .result
        .ok_or_else(|| AppError::Rpc("json-rpc response is missing result".to_string()))?;

    if result.value.len() != addresses.len() {
        return Err(AppError::Rpc(format!(
            "account count mismatch: requested {}, received {}",
            addresses.len(),
            result.value.len()
        )));
    }

    let slot = result.context.slot;
    result
        .value
        .into_iter()
        .zip(addresses.iter())
        .map(|(account, address)| decode_account(address, account, slot))
        .collect()
}

fn decode_account(
    address: &str,
    account: Option<RpcAccount>,
    slot: u64,
) -> Result<AccountData, AppError> {
    let account = account.ok_or_else(|| AppError::Rpc(format!("account {address} was null")))?;
    let encoded = account
        .data
        .first()
        .ok_or_else(|| AppError::Rpc(format!("account {address} has no data field")))?;
    let data = base64::engine::general_purpose::STANDARD
        .decode(encoded)
        .map_err(|error| AppError::Rpc(format!("failed to decode base64 for {address}: {error}")))?;

    Ok(AccountData {
        address: address.to_string(),
        owner: account.owner,
        lamports: account.lamports,
        data,
        slot,
    })
}

#[derive(Debug, Deserialize)]
pub struct RpcResponse {
    pub result: Option<RpcResult>,
    pub error: Option<RpcError>,
}

#[derive(Debug, Deserialize)]
pub struct RpcError {
    pub code: i64,
    pub message: String,
}

#[derive(Debug, Deserialize)]
pub struct RpcResult {
    pub context: RpcContext,
    pub value: Vec<Option<RpcAccount>>,
}

#[derive(Debug, Deserialize)]
pub struct RpcContext {
    pub slot: u64,
}

#[derive(Debug, Deserialize)]
pub struct RpcAccount {
    pub lamports: u64,
    pub owner: String,
    pub data: Vec<String>,
}
