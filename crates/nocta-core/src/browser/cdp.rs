use crate::{CoreError, CoreResult};
use futures::{SinkExt, StreamExt};
use serde_json::{json, Value};
use std::{collections::VecDeque, time::Duration};
use tokio::{net::TcpStream, sync::Mutex};
use tokio_tungstenite::{connect_async, tungstenite::Message, MaybeTlsStream, WebSocketStream};
use tokio_util::sync::CancellationToken;

type Socket = WebSocketStream<MaybeTlsStream<TcpStream>>;
struct Connection {
    socket: Socket,
    serial: u32,
    events: VecDeque<Value>,
}
pub struct Cdp {
    connection: Mutex<Connection>,
}

impl Cdp {
    pub async fn connect(url: &str, cancel: &CancellationToken) -> CoreResult<Self> {
        let socket = tokio::select! {
            biased;
            _ = cancel.cancelled() => return Err(CoreError::Cancelled),
            result = tokio::time::timeout(Duration::from_secs(10), connect_async(url)) => result.map_err(|_| CoreError::Timeout)?.map_err(|_| CoreError::Browser("Cannot connect to the managed browser.".into()))?.0,
        };
        Ok(Self {
            connection: Mutex::new(Connection {
                socket,
                serial: 0,
                events: VecDeque::new(),
            }),
        })
    }
    pub async fn call(
        &self,
        method: &str,
        params: Value,
        session: Option<&str>,
        cancel: &CancellationToken,
    ) -> CoreResult<Value> {
        self.call_timeout(method, params, session, cancel, Duration::from_secs(15))
            .await
    }
    pub async fn call_timeout(
        &self,
        method: &str,
        params: Value,
        session: Option<&str>,
        cancel: &CancellationToken,
        timeout: Duration,
    ) -> CoreResult<Value> {
        let operation = async {
            let mut connection = self.connection.lock().await;
            connection.serial += 1;
            let id = connection.serial;
            let mut request = json!({"id": id, "method": method, "params": params});
            if let Some(session) = session {
                request["sessionId"] = session.into();
            }
            connection
                .socket
                .send(Message::Text(request.to_string().into()))
                .await
                .map_err(|_| CoreError::Browser("The managed browser disconnected.".into()))?;
            while let Some(message) = connection.socket.next().await {
                let message = message
                    .map_err(|_| CoreError::Browser("The managed browser disconnected.".into()))?;
                if let Message::Text(text) = message {
                    let value: Value = serde_json::from_str(&text)
                        .map_err(|_| CoreError::Browser("Invalid browser response.".into()))?;
                    if value.get("method").is_some() {
                        if connection.events.len() >= 256 {
                            connection.events.pop_front();
                        }
                        connection.events.push_back(value.clone());
                    }
                    if value["id"].as_u64() == Some(id as u64) {
                        if value.get("error").is_some() {
                            return Err(CoreError::Browser(format!(
                                "Browser command {method} failed."
                            )));
                        }
                        return Ok(value["result"].clone());
                    }
                }
            }
            Err(CoreError::Browser(
                "The managed browser disconnected.".into(),
            ))
        };
        tokio::select! { biased; _ = cancel.cancelled() => Err(CoreError::Cancelled), result = tokio::time::timeout(timeout, operation) => result.map_err(|_| CoreError::Timeout)? }
    }
    pub async fn take_events(&self) -> Vec<Value> {
        self.connection.lock().await.events.drain(..).collect()
    }
}
