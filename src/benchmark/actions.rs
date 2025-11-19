use async_trait::async_trait;
use serde_json::{json, Value};
use crate::runtime::context::Context;
use crate::actions::{FunctionHandler, ExecutionMode};
use anyhow::Result;
use std::time::Duration;

#[derive(Debug)]
pub struct FibonacciAction;

impl FibonacciAction {
    fn fib(n: u64) -> u64 {
        if n <= 1 { n } else { Self::fib(n - 1) + Self::fib(n - 2) }
    }
}

#[async_trait]
impl FunctionHandler for FibonacciAction {
    fn name(&self) -> &str { "fib" }

    fn execution_mode(&self) -> ExecutionMode {
        ExecutionMode::Sync
    }

    fn validate(&self, _params: &Value) -> Result<()> { Ok(()) }
    async fn execute(&self, params: Value, _ctx: &Context) -> Result<Value> {
        let n = params.get("n").and_then(|v| v.as_u64()).unwrap_or(10);
        let result = Self::fib(n);
        Ok(json!({ "result": result }))
    }
}

#[derive(Debug)]
pub struct SleepAction;

#[async_trait]
impl FunctionHandler for SleepAction {
    fn name(&self) -> &str { "sleep" }

    fn execution_mode(&self) -> ExecutionMode {
        ExecutionMode::Async
    }

    fn validate(&self, _params: &Value) -> Result<()> { Ok(())
    }
    async fn execute(&self, params: Value, _ctx: &Context) -> Result<Value> {
        let ms = params.get("ms").and_then(|v| v.as_u64()).unwrap_or(10);
        tokio::time::sleep(Duration::from_millis(ms)).await;
        Ok(json!({ "slept": true }))
    }
}

