use crate::runtime::node::{Node, NodeDefinition};
use crate::runtime::context::Context;
use crate::runtime::syscall::Syscall;
use crate::runtime::task::Task;
use async_trait::async_trait;
use serde_json::Value;
use anyhow::{Result, anyhow};

#[derive(Debug)]
pub struct StartNode {
    next: Option<usize>,
}

pub struct StartDefinition;

impl NodeDefinition for StartDefinition {
    fn name(&self) -> &str { "start" }
    fn validate(&self, _params: &Value) -> Result<()> { Ok(()) }
    fn prepare(&self, params: Value) -> Result<Box<dyn Node>> {
        let next = params.get("next").and_then(|v| v.as_u64()).map(|i| i as usize);
        Ok(Box::new(StartNode { next }))
    }
}

#[async_trait]
impl Node for StartNode {
    async fn execute(&self, _ctx: &Context, _task: &Task, syscall: &mut dyn Syscall) -> Result<()> {
        if let Some(target) = self.next {
            syscall.jump(target);
        }
        Ok(())
    }
}

#[derive(Debug)]
pub struct EndNode;

pub struct EndDefinition;

impl NodeDefinition for EndDefinition {
    fn name(&self) -> &str { "end" }
    fn validate(&self, _params: &Value) -> Result<()> { Ok(()) }
    fn prepare(&self, _params: Value) -> Result<Box<dyn Node>> {
        Ok(Box::new(EndNode))
    }
}

#[async_trait]
impl Node for EndNode {
    async fn execute(&self, _ctx: &Context, _task: &Task, syscall: &mut dyn Syscall) -> Result<()> {
        syscall.terminate(); 
        Ok(())
    }
}