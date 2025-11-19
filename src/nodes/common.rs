use crate::runtime::node::{Node, NodeDefinition};
use crate::runtime::context::Context;
use crate::runtime::syscall::Syscall;
use crate::runtime::task::Task;
use async_trait::async_trait;
use serde_json::Value;
use anyhow::Result;
use tracing::{info, warn};

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
pub struct EndNode {
    output_var: String,
}

pub struct EndDefinition;

impl NodeDefinition for EndDefinition {
    fn name(&self) -> &str { "end" }
    fn validate(&self, _params: &Value) -> Result<()> { Ok(()) }
    fn prepare(&self, params: Value) -> Result<Box<dyn Node>> {
        let output_var = params.get("output").and_then(|v| v.as_str()).unwrap_or("").to_string();
        Ok(Box::new(EndNode { output_var }))
    }
}

#[async_trait]

impl Node for EndNode {

    async fn execute(&self, ctx: &Context, _task: &Task, syscall: &mut dyn Syscall) -> Result<()> {

        if !self.output_var.is_empty() {

            if let Some(val) = ctx.get_var(&self.output_var).await {

                info!("Workflow Output: {:?}", val);

                ctx.set_var("_WORKFLOW_OUTPUT", val).await;

            } else {

                warn!("End node configured to output '{}' but variable not found", self.output_var);

            }

        }

        syscall.terminate(); 

        Ok(())

    }

}
