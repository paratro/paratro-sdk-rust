//! A tiny scripted HTTP/1.1 server on loopback that stands in for the gateway.
//!
//! * `POST /api/v1/auth/token` is answered automatically with `token-<n>` where
//!   `n` counts auth calls, so a refreshed token is distinguishable.
//! * Every other request pops the next scripted `(status, body)` reply.
//! * Every request (auth included) is recorded for assertions.
//!
//! No real gateway, no network, no credentials.
#![allow(dead_code)]

use std::collections::{BTreeMap, VecDeque};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use paratro_sdk::{Config, MpcClient};
use serde_json::{json, Value};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::task::JoinHandle;

pub const AUTH_PATH: &str = "/api/v1/auth/token";

#[derive(Debug, Clone)]
pub struct Call {
    pub method: String,
    /// Path including the query string, exactly as sent.
    pub path: String,
    /// Header names lower-cased.
    pub headers: BTreeMap<String, String>,
    pub body: Vec<u8>,
}

impl Call {
    pub fn body_str(&self) -> &str {
        std::str::from_utf8(&self.body).expect("utf-8 body")
    }

    pub fn json(&self) -> Value {
        serde_json::from_slice(&self.body).expect("json body")
    }

    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .get(&name.to_ascii_lowercase())
            .map(String::as_str)
    }
}

pub struct FakeGateway {
    pub client: MpcClient,
    pub base_url: String,
    calls: Arc<Mutex<Vec<Call>>>,
    worker: JoinHandle<()>,
}

impl Drop for FakeGateway {
    fn drop(&mut self) {
        self.worker.abort();
    }
}

impl FakeGateway {
    /// Starts the server with the given scripted replies for non-auth requests.
    pub async fn start(replies: Vec<(u16, Value)>) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base_url = format!("http://{}", listener.local_addr().unwrap());
        let calls: Arc<Mutex<Vec<Call>>> = Arc::new(Mutex::new(Vec::new()));
        let recorded = calls.clone();

        let worker = tokio::spawn(async move {
            let mut replies = VecDeque::from(replies);
            let mut auth_calls = 0usize;
            loop {
                let (mut stream, _) = listener.accept().await.unwrap();
                let call = read_request(&mut stream).await;
                let (status, body) = if call.path == AUTH_PATH {
                    auth_calls += 1;
                    (
                        200,
                        json!({
                            "token": format!("token-{auth_calls}"),
                            "expires_in": 900,
                            "token_type": "Bearer",
                            "client": {
                                "client_id": "fixture",
                                "client_name": "fixture",
                                "status": "ACTIVE",
                                "subscription_tier": "",
                                "max_wallets": 1
                            }
                        }),
                    )
                } else {
                    replies.pop_front().unwrap_or((
                        500,
                        json!({"code":"internal_error","type":"api_error","message":"unmocked request in test fixture"}),
                    ))
                };
                recorded.lock().unwrap().push(call);
                let body = body.to_string();
                let response = format!(
                    "HTTP/1.1 {status} Fixture\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
                stream.write_all(response.as_bytes()).await.unwrap();
                let _ = stream.shutdown().await;
            }
        });

        let client =
            MpcClient::new("fixture-key", "fixture-secret", Config::new(&base_url)).unwrap();
        Self {
            client,
            base_url,
            calls,
            worker,
        }
    }

    /// All recorded requests, in order (auth included).
    pub fn calls(&self) -> Vec<Call> {
        self.calls.lock().unwrap().clone()
    }

    /// Recorded requests excluding `POST /api/v1/auth/token`.
    pub fn api_calls(&self) -> Vec<Call> {
        self.calls()
            .into_iter()
            .filter(|c| c.path != AUTH_PATH)
            .collect()
    }

    /// Recorded `POST /api/v1/auth/token` requests.
    pub fn auth_calls(&self) -> Vec<Call> {
        self.calls()
            .into_iter()
            .filter(|c| c.path == AUTH_PATH)
            .collect()
    }

    /// The single API request a test expects to have been made.
    pub fn only_api_call(&self) -> Call {
        let calls = self.api_calls();
        assert_eq!(
            calls.len(),
            1,
            "expected exactly one API call, got {calls:?}"
        );
        calls.into_iter().next().unwrap()
    }
}

async fn read_request(stream: &mut tokio::net::TcpStream) -> Call {
    let mut bytes = Vec::new();
    let header_end = loop {
        let mut chunk = [0u8; 4096];
        let n = tokio::time::timeout(Duration::from_secs(5), stream.read(&mut chunk))
            .await
            .expect("request header timeout")
            .expect("request read");
        assert!(n > 0, "connection closed before headers were complete");
        bytes.extend_from_slice(&chunk[..n]);
        assert!(bytes.len() < 1 << 20, "oversized fixture request");
        if let Some(at) = bytes.windows(4).position(|w| w == b"\r\n\r\n") {
            break at + 4;
        }
    };
    let header_text = String::from_utf8(bytes[..header_end].to_vec()).unwrap();
    let mut lines = header_text.split("\r\n");
    let mut request_line = lines.next().unwrap().split_whitespace();
    let method = request_line.next().unwrap().to_owned();
    let path = request_line.next().unwrap().to_owned();
    let headers: BTreeMap<String, String> = lines
        .filter_map(|line| line.split_once(':'))
        .map(|(k, v)| (k.trim().to_ascii_lowercase(), v.trim().to_owned()))
        .collect();
    let length: usize = headers
        .get("content-length")
        .map_or(0, |n| n.parse().unwrap());
    while bytes.len() < header_end + length {
        let mut chunk = [0u8; 4096];
        let n = tokio::time::timeout(Duration::from_secs(5), stream.read(&mut chunk))
            .await
            .expect("request body timeout")
            .expect("request read");
        assert!(n > 0, "connection closed before body was complete");
        bytes.extend_from_slice(&chunk[..n]);
    }
    Call {
        method,
        path,
        headers,
        body: bytes[header_end..header_end + length].to_vec(),
    }
}

/// `common.ErrorBody` as the gateway writes it.
pub fn error_body(code: &str, error_type: &str, message: &str) -> Value {
    json!({"code": code, "type": error_type, "message": message})
}
