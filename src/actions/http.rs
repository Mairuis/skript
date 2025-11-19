use async_trait::async_trait;
use serde_json::{Value, json};
use crate::actions::FunctionHandler;
use crate::runtime::context::Context;
use anyhow::{Result, anyhow};
use std::fmt::Debug;
use reqwest::Client;

#[derive(Debug)]
pub struct HttpAction {
    client: Client,
}

impl HttpAction {
    pub fn new() -> Self {
        Self {
            client: Client::new(),
        }
    }
}

#[async_trait]
impl FunctionHandler for HttpAction {
    fn name(&self) -> &str {
        "http"
    }

    fn validate(&self, params: &Value) -> Result<()> {
        if params.get("url").is_none() {
            return Err(anyhow!("Missing required parameter: url"));
        }
        Ok(())
    }

    async fn execute(&self, params: Value, _ctx: &Context) -> Result<Value> {
        let url = params.get("url").and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("Invalid url"))?;
        
        let method_str = params.get("method").and_then(|v| v.as_str()).unwrap_or("GET");
        let method = method_str.parse::<reqwest::Method>()
            .map_err(|_| anyhow!("Invalid HTTP method: {}", method_str))?;

        let mut builder = self.client.request(method, url);

        // Handle Body (JSON)
        if let Some(body) = params.get("body") {
            builder = builder.json(body);
        }

        // Handle Headers
        if let Some(headers) = params.get("headers").and_then(|v| v.as_object()) {
            for (k, v) in headers {
                if let Some(v_str) = v.as_str() {
                    builder = builder.header(k, v_str);
                }
            }
        }

        let response = builder.send().await?;
        let status = response.status().as_u16();
        
        // Parse JSON response if possible, else text
        // We return a wrapper object { status: 200, data: ... }
        let data = match response.json::<Value>().await {
            Ok(json) => json,
            Err(_) => Value::Null, // Or handle text body
        };

        Ok(json!({
            "status": status,
            "data": data
        }))
    }
}
