use async_trait::async_trait;
use serde_json::Value;
use crate::runtime::context::Context;
use crate::runtime::syscall::Syscall;
use crate::runtime::task::Task;
use anyhow::Result;
use std::fmt::Debug;

/// 运行时节点接口
#[async_trait]
pub trait Node: Send + Sync + Debug {
    /// 运行时执行
    async fn execute(&self, ctx: &Context, task: &Task, syscall: &mut dyn Syscall) -> Result<()>;
}

/// 节点工厂/定义接口
pub trait NodeDefinition: Send + Sync {
    fn name(&self) -> &str;
    fn validate(&self, params: &Value) -> Result<()>;
    fn prepare(&self, params: Value) -> Result<Box<dyn Node>>;
}