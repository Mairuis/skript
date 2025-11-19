use async_trait::async_trait;
use serde_json::Value;
use crate::runtime::context::Context;
use anyhow::Result;
use std::fmt::Debug;

pub mod builtin;
pub mod http;

/// 插件接口：所有功能节点必须实现此 Trait
#[async_trait]
pub trait ActionHandler: Send + Sync + Debug {
    fn name(&self) -> &str;
    fn validate(&self, params: &Value) -> Result<()>;
    async fn execute(&self, params: Value, ctx: &Context) -> Result<Value>;
}