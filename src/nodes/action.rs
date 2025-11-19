use crate::runtime::node::{Node, NodeDefinition};
use crate::runtime::context::Context;
use crate::runtime::syscall::Syscall;
use crate::runtime::task::Task;
use crate::actions::ActionHandler;
use async_trait::async_trait;
use serde_json::Value;
use anyhow::Result;
use std::sync::Arc;

/// 将 ActionHandler 包装为 Node
#[derive(Debug)]
pub struct ActionNode {
    handler: Arc<dyn ActionHandler>,
    params: Value,
    output: Option<String>,
    next: Option<usize>,
}

#[async_trait]
impl Node for ActionNode {
    async fn execute(&self, ctx: &Context, _task: &Task, syscall: &mut dyn Syscall) -> Result<()> {
        // 1. Resolve Variables in Params
        let mut resolved_params = self.params.clone();
        if let Some(obj) = resolved_params.as_object_mut() {
            for (_, v) in obj.iter_mut() {
                if let Some(s) = v.as_str() {
                    if s.starts_with("${") && s.ends_with("}") {
                        let var_name = &s[2..s.len()-1];
                        if let Some(val) = ctx.get_var(var_name) {
                            *v = val;
                        }
                    }
                }
            }
        }

        // 2. Execute Logic
        let result = self.handler.execute(resolved_params, ctx).await?;

        // 3. Write Output
        if let Some(out_key) = &self.output {
            ctx.set_var(out_key, result);
        }

        // 4. Jump Next
        if let Some(target) = self.next {
            syscall.jump(target);
        }

        Ok(())
    }
}

/// 对应的 Definition
pub struct ActionNodeDefinition {
    pub handler: Arc<dyn ActionHandler>,
}

impl NodeDefinition for ActionNodeDefinition {
    fn name(&self) -> &str {
        self.handler.name()
    }

    fn validate(&self, params: &Value) -> Result<()> {
        self.handler.validate(params)
    }

    fn prepare(&self, params: Value) -> Result<Box<dyn Node>> {
        // Extract System Params
        let next = params.get("next").and_then(|v| v.as_u64()).map(|i| i as usize);
        let output = params.get("output").and_then(|v| v.as_str()).map(|s| s.to_string());
        
        // The rest are user params
        // Note: We might want to remove "next" and "output" from params before passing to Node?
        // Or just let Node keep them. ActionHandler usually ignores unknown params.
        
        Ok(Box::new(ActionNode {
            handler: self.handler.clone(),
            params,
            output,
            next,
        }))
    }
}
