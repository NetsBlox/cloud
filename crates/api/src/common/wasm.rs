use super::super::error;
use super::super::Client;
use reqwest::{Method, RequestBuilder, Response};

use derive_more::{Deref, From};
use into_jsvalue_derive::IntoJsValue;
use serde::{Deserialize, Serialize};
use serde_json::json;
use tsify::Tsify;
use wasm_bindgen::{prelude::*, JsValue};
use web_sys::WebSocket;

#[derive(Serialize, Deserialize, Tsify, IntoJsValue, Deref, From)]
#[tsify(into_wasm_abi, from_wasm_abi)]
#[repr(transparent)]
#[serde(transparent)]
pub struct Vec<T: Into<JsValue> + Serialize>(std::vec::Vec<T>);

fn test() {
    let test: Vec<String> = vec![String::new()].into();
    for item in test {}
}

impl<T: Into<JsValue> + Serialize> IntoIterator for Vec<T> {
    type Item = T;
    type IntoIter = std::vec::IntoIter<T>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

impl<'a, T: Into<JsValue> + Serialize> IntoIterator for &'a Vec<T> {
    type Item = &'a T;
    type IntoIter = std::slice::Iter<'a, T>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.iter()
    }
}

impl<'a, T: Into<JsValue> + Serialize> IntoIterator for &'a mut Vec<T> {
    type Item = &'a mut T;
    type IntoIter = std::slice::IterMut<'a, T>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.iter_mut()
    }
}

#[wasm_bindgen(getter_with_clone)]
pub struct MessageChannel {
    pub id: String,
    pub stream: WebSocket,
}

#[wasm_bindgen]
impl MessageChannel {
    // TODO: do we need a method for sending other types?
    // TODO: sending a generic struct (implementing Deserialize)
    pub async fn send_json(
        &mut self,
        addr: &str,
        r#type: &str,
        data: &str,
    ) -> Result<(), error::Error> {
        let msg = json!({
            "type": "message",
            "dstId": addr,
            "msgType": r#type,
            "content": data
        });
        let msg_text = serde_json::to_string(&msg).unwrap();
        self.stream
            .send_with_str(&msg_text)
            .map_err(|e| error::Error::WebSocketError(e.as_string().unwrap_throw()))?;

        Ok(())
    }
}

impl MessageChannel {
    pub async fn ws_connect(url: &str) -> Result<WebSocket, error::Error> {
        Ok(WebSocket::new(url)
            .map_err(|e| error::Error::WebSocketError(e.as_string().unwrap_throw()))?)
    }
}

impl Client {
    pub fn request(&self, method: Method, path: &str) -> RequestBuilder {
        let client = reqwest::Client::new();
        client
            .request(method, format!("{}{}", self.cfg.url, path))
            .fetch_credentials_include()
    }
}

pub fn get_token(resp: &Response) -> Option<String> {
    None
}
