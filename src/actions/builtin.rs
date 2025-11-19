use async_trait::async_trait;
use serde_json::Value;
use crate::actions::ActionHandler;
use crate::runtime::context::Context;
use anyhow::Result;
use std::fmt::Debug;

#[derive(Debug)]
pub struct LogAction;

#[async_trait]
impl ActionHandler for LogAction {
    fn name(&self) -> &str {
        "log"
    }

    fn validate(&self, _params: &Value) -> Result<()> {
        // 简单校验：确保有 msg 字段？这里先略过
        Ok(())
    }

    async fn execute(&self, params: Value, _ctx: &Context) -> Result<Value> {
        if let Some(msg) = params.get("msg").and_then(|v| v.as_str()) {
            println!("[LOG] {}", msg);
        } else {
            println!("[LOG] {:?}", params);
        }
        Ok(Value::Null)
    }
}

#[derive(Debug)]
pub struct AssignAction;

#[async_trait]
impl ActionHandler for AssignAction {
    fn name(&self) -> &str {
        "assign"
    }

    fn validate(&self, _params: &Value) -> Result<()> {
        Ok(())
    }

    async fn execute(&self, params: Value, _ctx: &Context) -> Result<Value> {
        // 这里 Assign 比较特殊，它通常不返回值，而是直接修改 Context?
        // 但根据我们的架构，Action 尽量只返回 Value，由 Engine 负责写入 output_var。
        // 不过，如果 AssignAction 想支持 "value" 参数直接返回，那就可以。
        
        if let Some(val) = params.get("value") {
            Ok(val.clone())
        } else {
            Ok(Value::Null)
        }
    }
}
