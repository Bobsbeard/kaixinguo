//! Sync connectors. The `Connector` enum isolates everything service-specific.
//!
//! Bingqilin API contract — v0.1 assumption (spec §9), to be confirmed
//! against the real API once documented:
//!   POST {base}/api/v1/sync
//!   Authorization: Bearer <token>
//!   Body:     { "ops": [ SyncOp, ... ] }
//!   Response: { "results": [ { "id": <op id>, "status": "ok" | "error",
//!                              "remote_id": <string, optional>,
//!                              "error": <string, optional> } ] }
//!
//! For development, a server URL of "mock" or "mock://..." selects the
//! mock connector, which acknowledges every op and fabricates remote ids.

use serde::Deserialize;

use super::SyncOp;
use crate::error::AppError;

pub struct OpResult {
    pub op_id: String,
    /// Ok(Some(remote_id)) on success; Err(message) on per-op failure.
    pub outcome: Result<Option<String>, String>,
}

pub enum Connector {
    Mock,
    Bingqilin {
        base_url: String,
        token: String,
        client: reqwest::Client,
    },
}

#[derive(Debug, Deserialize)]
struct SyncResponse {
    results: Vec<SyncResultRow>,
}

#[derive(Debug, Deserialize)]
struct SyncResultRow {
    id: String,
    status: String,
    remote_id: Option<String>,
    error: Option<String>,
}

impl Connector {
    pub fn from_settings(server_url: &str, token: &str) -> Self {
        let url = server_url.trim().trim_end_matches('/');
        if url.is_empty() || url == "mock" || url.starts_with("mock://") {
            Connector::Mock
        } else {
            Connector::Bingqilin {
                base_url: url.to_string(),
                token: token.to_string(),
                client: reqwest::Client::new(),
            }
        }
    }

    pub async fn push(&self, ops: &[SyncOp]) -> Result<Vec<OpResult>, AppError> {
        match self {
            Connector::Mock => Ok(ops
                .iter()
                .map(|o| OpResult {
                    op_id: o.id.clone(),
                    outcome: Ok(Some(format!("mock-{}", &o.id[..8.min(o.id.len())]))),
                })
                .collect()),
            Connector::Bingqilin {
                base_url,
                token,
                client,
            } => {
                let resp = client
                    .post(format!("{base_url}/api/v1/sync"))
                    .bearer_auth(token)
                    .json(&serde_json::json!({ "ops": ops }))
                    .send()
                    .await
                    .map_err(|e| AppError::Network(e.to_string()))?;
                if !resp.status().is_success() {
                    return Err(AppError::Network(format!(
                        "Bingqilin returned HTTP {}",
                        resp.status()
                    )));
                }
                let wire: SyncResponse = resp
                    .json()
                    .await
                    .map_err(|e| AppError::Network(format!("bad sync response: {e}")))?;
                Ok(wire
                    .results
                    .into_iter()
                    .map(|r| OpResult {
                        op_id: r.id,
                        outcome: match r.status.as_str() {
                            "ok" => Ok(r.remote_id),
                            _ => Err(r
                                .error
                                .unwrap_or_else(|| format!("status {}", r.status))),
                        },
                    })
                    .collect())
            }
        }
    }
}
