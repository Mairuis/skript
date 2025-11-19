use async_trait::async_trait;
use serde_json::{Value, json};
use crate::actions::{FunctionHandler, ExecutionMode};
use crate::runtime::context::Context;
use anyhow::Result;
use std::fmt::Debug;
use evalexpr::{eval_with_context, HashMapContext, ContextWithMutableVariables, DefaultNumericTypes};
use tracing::{info, error};

#[derive(Debug)]
pub struct LogAction;

#[async_trait]
impl FunctionHandler for LogAction {
    fn name(&self) -> &str {
        "log"
    }

    fn execution_mode(&self) -> ExecutionMode {
        ExecutionMode::Sync
    }

    fn validate(&self, _params: &Value) -> Result<()> {
        Ok(())
    }

    async fn execute(&self, params: Value, _ctx: &Context) -> Result<Value> {
        if let Some(msg) = params.get("msg").and_then(|v| v.as_str()) {
            info!("[LOG] {}", msg);
        } else {
            info!("[LOG] {:?}", params);
        }
        Ok(Value::Null)
    }
}

#[derive(Debug)]
pub struct AssignAction;

#[async_trait]
impl FunctionHandler for AssignAction {
    fn name(&self) -> &str {
        "assign"
    }

    fn execution_mode(&self) -> ExecutionMode {
        ExecutionMode::Sync
    }

    fn validate(&self, _params: &Value) -> Result<()> {
        Ok(())
    }

    async fn execute(&self, params: Value, ctx: &Context) -> Result<Value> {
        // 1. Handle "assignments" list
        if let Some(list) = params.get("assignments").and_then(|v| v.as_array()) {
            for item in list {
                if let (Some(k), Some(v)) = (item.get("key").and_then(|s| s.as_str()), item.get("value")) {
                    ctx.set_var(k, v.clone()).await;
                }
            }
        }

        // 2. Handle "expression"
        if let Some(expr) = params.get("expression").and_then(|v| v.as_str()) {
            // Simple parsing for "var = expr"
            let (target_var, rhs) = if let Some((left, right)) = expr.split_once('=') {
                (Some(left.trim()), right.trim())
            } else {
                (None, expr)
            };

            // Build context for evalexpr
            let mut eval_ctx = HashMapContext::<DefaultNumericTypes>::new();
            let all_vars = ctx.get_all_vars().await?;
            
            for (k, v) in all_vars {
                let ev = match v {
                    Value::String(s) => Some(evalexpr::Value::String(s)),
                    Value::Number(n) => {
                         if let Some(i) = n.as_i64() { Some(evalexpr::Value::Int(i)) }
                         else if let Some(f) = n.as_f64() { Some(evalexpr::Value::Float(f)) }
                         else { None }
                    },
                    Value::Bool(b) => Some(evalexpr::Value::Boolean(b)),
                    _ => None,
                };
                if let Some(ev) = ev {
                    let _ = eval_ctx.set_value(k, ev);
                }
            }

            // Evaluate
            match eval_with_context(rhs, &eval_ctx) {
                Ok(result) => {
                    let json_val = match result {
                        evalexpr::Value::String(s) => Some(Value::String(s)),
                        evalexpr::Value::Int(i) => Some(json!(i)),
                        evalexpr::Value::Float(f) => Some(json!(f)),
                        evalexpr::Value::Boolean(b) => Some(Value::Bool(b)),
                        _ => None,
                    };

                    if let Some(jv) = json_val {
                         // If it was an assignment, set the var
                         if let Some(var_name) = target_var {
                             ctx.set_var(var_name, jv).await;
                         } else {
                             // If just expression, maybe return it? 
                             // But we prioritize "value" param for return.
                             // We could return this if "value" is missing.
                             if params.get("value").is_none() {
                                 return Ok(jv);
                             }
                         }
                    }
                },
                Err(e) => error!("Expression evaluation failed: {} -> {}", rhs, e),
            }
        }

        // 3. Handle "value"
        if let Some(val) = params.get("value") {
            Ok(val.clone())
        } else {
            Ok(Value::Null)
        }
    }
}
