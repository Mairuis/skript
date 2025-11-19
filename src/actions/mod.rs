use async_trait::async_trait;
use serde_json::Value;
use crate::runtime::context::Context;
use anyhow::Result;
use std::fmt::Debug;

pub mod builtin;
pub mod http;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExecutionMode {
    /// Execute normally via async task scheduling (Network I/O, heavy compute)
    Async,
    /// Execute immediately in the current thread (Simple logic, math, fast operations)
    /// Candidates for Node Fusion.
    Sync,
}

/// 插件接口：所有功能节点必须实现此 Trait
#[async_trait]
pub trait FunctionHandler: Send + Sync + Debug {
    fn name(&self) -> &str;
    /// execution mode of the handler
    fn execution_mode(&self) -> ExecutionMode {
        ExecutionMode::Async
    }
    fn validate(&self, params: &Value) -> Result<()>;
    async fn execute(&self, params: Value, ctx: &Context) -> Result<Value>;
}