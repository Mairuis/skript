use async_trait::async_trait;
use crate::runtime::node::{Node, NodeDefinition};
use crate::runtime::context::Context;
use crate::runtime::syscall::Syscall;
use crate::runtime::task::Task;
use crate::runtime::blueprint::NodeIndex;
use crate::actions::FunctionHandler;
use crate::actions::builtin::{AssignAction, LogAction};
use anyhow::{Result, anyhow};
use serde_json::Value;
use std::fmt::Debug;
use std::sync::Arc;

/// A lightweight executable operation for fused nodes.
/// Unlike `Node`, it doesn't interact with Syscall or Task, just Context.
#[async_trait]
pub trait ExecutableOp: Send + Sync + Debug {
    async fn execute_op(&self, ctx: &Context) -> Result<()>;
}

// Wrapper to adapt FunctionHandler to ExecutableOp
#[derive(Debug)]
struct FunctionOp {
    handler: Arc<dyn FunctionHandler>,
    params: Value,
    output: Option<String>,
}

#[async_trait]
impl ExecutableOp for FunctionOp {
    async fn execute_op(&self, ctx: &Context) -> Result<()> {
        let result = self.handler.execute(self.params.clone(), ctx).await?;
        
        if let Some(var_name) = &self.output {
            ctx.set_var(var_name, result).await;
        }
        Ok(())
    }
}

#[derive(Debug)]
pub struct FusedNode {
    pub ops: Vec<Box<dyn ExecutableOp>>,
    pub next_index: Option<NodeIndex>,
}

#[async_trait]
impl Node for FusedNode {
    async fn execute(&self, ctx: &Context, _task: &Task, syscall: &mut dyn Syscall) -> Result<()> {
        // Execute all sub-operations sequentially without context switching or scheduling
        for op in &self.ops {
            op.execute_op(ctx).await?;
        }

        // After all ops are done, jump to the next node if it exists
        if let Some(target) = self.next_index {
            syscall.jump(target);
        }
        
        Ok(())
    }
}

pub struct FusedNodeDefinition;

impl NodeDefinition for FusedNodeDefinition {
    fn name(&self) -> &str {
        "fused"
    }

    fn validate(&self, _params: &Value) -> Result<()> {
        Ok(())
    }

    fn prepare(&self, params: Value) -> Result<Box<dyn Node>> {
        let ops_json = params.get("ops").and_then(|v| v.as_array())
            .ok_or_else(|| anyhow!("FusedNode missing 'ops' param"))?;
            
        let mut ops: Vec<Box<dyn ExecutableOp>> = Vec::new();
        
        for op_def in ops_json {
            let kind = op_def.get("kind").and_then(|v| v.as_str())
                .ok_or_else(|| anyhow!("Fused op missing kind"))?;
            let op_params = op_def.get("params").cloned().unwrap_or(Value::Null);
            
            let output = op_params.get("output").and_then(|v| v.as_str()).map(|s| s.to_string());
            
            // Hardcoded registry for fusion candidates
            let handler: Arc<dyn FunctionHandler> = match kind {
                "log" => Arc::new(LogAction),
                "assign" => Arc::new(AssignAction),
                _ => return Err(anyhow!("Unsupported fused op kind: {}", kind)),
            };
            
            ops.push(Box::new(FunctionOp {
                handler,
                params: op_params,
                output,
            }));
        }
        
        let next_index = params.get("next").and_then(|v| v.as_u64()).map(|i| i as usize);

        Ok(Box::new(FusedNode {
            ops,
            next_index,
        }))
    }
}
