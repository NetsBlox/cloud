use super::super::error;
use super::super::Client;

use futures_util::SinkExt;
use reqwest::{Method, RequestBuilder, Response};
use tokio::net::TcpStream;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::{connect_async, MaybeTlsStream, WebSocketStream};

use serde_json::{json, Value};

pub struct MessageChannel {
    pub id: String,
    pub stream: WebSocketStream<MaybeTlsStream<TcpStream>>,
}

impl MessageChannel {
    // TODO: do we need a method for sending other types?
    // TODO: sending a generic struct (implementing Deserialize)
    pub async fn send_json(
        &mut self,
        addr: &str,
        r#type: &str,
        data: Value,
    ) -> Result<(), error::Error> {
        let msg = json!({
            "type": "message",
            "dstId": addr,
            "msgType": r#type,
            "content": data
        });
        let msg_text = serde_json::to_string(&msg).unwrap();
        self.stream
            .send(Message::Text(msg_text))
            .await
            .map_err(|e| error::Error::WebSocketError(e.to_string()))?;

        Ok(())
    }
}

impl MessageChannel {
    pub async fn ws_connect(
        url: &str,
    ) -> Result<WebSocketStream<MaybeTlsStream<TcpStream>>, error::Error> {
        let (ws_stream, _) = connect_async(url)
            .await
            .map_err(|e| error::Error::RequestError(e.to_string()))?;
        Ok(ws_stream)
    }
}

impl Client {
    pub fn request(&self, method: Method, path: &str) -> RequestBuilder {
        let client = reqwest::Client::new();
        let empty = "".to_owned();
        let token = self.cfg.token.as_ref().unwrap_or(&empty);
        client
            .request(method, format!("{}{}", self.cfg.url, path))
            .header("Cookie", format!("netsblox={}", token))
    }
}

pub fn get_token(resp: &Response) -> Option<String> {
    Some(
        resp.cookies()
            .find(|cookie| cookie.name() == "netsblox")
            .ok_or("No cookie received.")
            .unwrap()
            .value()
            .to_owned(),
    )
}
