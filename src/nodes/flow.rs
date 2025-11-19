use crate::runtime::node::{Node, NodeDefinition};
use crate::runtime::context::Context;
use crate::runtime::syscall::Syscall;
use crate::runtime::task::Task;
use async_trait::async_trait;
use serde_json::{Value, json};
use anyhow::{Result, anyhow};
use evalexpr::{build_operator_tree, Node as EvalNode, ContextWithMutableVariables, HashMapContext, DefaultNumericTypes};
use std::sync::atomic::{AtomicUsize, Ordering};

// --- ITERATION NODE ---

#[derive(Debug)]
pub struct IterationNode {
    collection_var: String,
    item_var: String,
    body_target: Option<usize>,
    next_target: Option<usize>,
}

pub struct IterationDefinition;

impl NodeDefinition for IterationDefinition {
    fn name(&self) -> &str { "iteration" }
    fn validate(&self, _params: &Value) -> Result<()> { Ok(()) }
    fn prepare(&self, params: Value) -> Result<Box<dyn Node>> {
        let collection_var = params.get("collection").and_then(|v| v.as_str())
             .map(|s| s.replace("${", "").replace("}", ""))
             .ok_or(anyhow!("Missing collection"))?.to_string();
        let item_var = params.get("item_var").and_then(|v| v.as_str()).ok_or(anyhow!("Missing item_var"))?.to_string();
        
        let body = params.get("body").and_then(|v| v.as_u64()).map(|i| i as usize);
        let next = params.get("next").and_then(|v| v.as_u64()).map(|i| i as usize);

        Ok(Box::new(IterationNode {
            collection_var,
            item_var,
            body_target: body,
            next_target: next,
        }))
    }
}

#[async_trait]
impl Node for IterationNode {
    async fn execute(&self, ctx: &Context, task: &Task, syscall: &mut dyn Syscall) -> Result<()> {
        let iter_idx_key = format!("__iter_idx_{}", task.node_index);
        
        let current_idx = ctx.get_var(&iter_idx_key).and_then(|v| v.as_u64()).unwrap_or(0) as usize;
        let collection = ctx.get_var(&self.collection_var);
        
        if let Some(Value::Array(arr)) = collection {
            if current_idx < arr.len() {
                ctx.set_var(&self.item_var, arr[current_idx].clone());
                ctx.set_var(&iter_idx_key, json!(current_idx + 1));
                
                if let Some(target) = self.body_target {
                    syscall.jump(target);
                }
            } else {
                if let Some(target) = self.next_target {
                    syscall.jump(target);
                }
            }
        } else {
             if let Some(target) = self.next_target {
                 syscall.jump(target);
             }
        }
        
        Ok(())
    }
}

// --- LOOP NODE ---

#[derive(Debug)]
pub struct LoopNode {
    condition: EvalNode,
    raw_cond: String,
    body_target: Option<usize>,
    next_target: Option<usize>,
}

pub struct LoopDefinition;

impl NodeDefinition for LoopDefinition {
    fn name(&self) -> &str { "loop" }
    fn validate(&self, _params: &Value) -> Result<()> { Ok(()) }
    fn prepare(&self, params: Value) -> Result<Box<dyn Node>> {
        let cond_str = params.get("condition").and_then(|v| v.as_str())
            .map(|s| s.replace("${", "").replace("}", ""))
            .ok_or(anyhow!("Missing condition"))?;
            
        let compiled = build_operator_tree(&cond_str)?;
        
        let body = params.get("body").and_then(|v| v.as_u64()).map(|i| i as usize);
        let next = params.get("next").and_then(|v| v.as_u64()).map(|i| i as usize);

        Ok(Box::new(LoopNode {
            condition: compiled,
            raw_cond: cond_str,
            body_target: body,
            next_target: next,
        }))
    }
}

#[async_trait]
impl Node for LoopNode {
    async fn execute(&self, ctx: &Context, _task: &Task, syscall: &mut dyn Syscall) -> Result<()> {
        // Evaluate condition (similar to IfNode)
        let mut eval_ctx = HashMapContext::<DefaultNumericTypes>::new();
        for r in ctx.variables.iter() {
            let (k, v) = (r.key(), r.value());
            let eval_val = match v {
                Value::String(s) => Some(evalexpr::Value::String(s.clone())),
                Value::Number(n) => {
                    if let Some(i) = n.as_i64() { Some(evalexpr::Value::Int(i)) }
                    else if let Some(f) = n.as_f64() { Some(evalexpr::Value::Float(f)) }
                    else { None }
                },
                Value::Bool(b) => Some(evalexpr::Value::Boolean(*b)),
                _ => None, 
            };
            if let Some(ev) = eval_val {
                let _ = eval_ctx.set_value(k.clone(), ev);
            }
        }

        let result = self.condition.eval_boolean_with_context(&eval_ctx)
            .unwrap_or_else(|e| {
                eprintln!("Eval failed for loop '{}': {}", self.raw_cond, e);
                false
            });

        if result {
            if let Some(target) = self.body_target {
                syscall.jump(target);
            }
        } else {
            if let Some(target) = self.next_target {
                syscall.jump(target);
            }
        }
        Ok(())
    }
}

// --- IF NODE ---

#[derive(Debug)]
struct IfBranch {
    condition: EvalNode, // Pre-compiled AST
    target: usize,
    raw_cond: String,
}

#[derive(Debug)]
pub struct IfNode {
    branches: Vec<IfBranch>,
    else_next: Option<usize>,
}

pub struct IfDefinition;

impl NodeDefinition for IfDefinition {
    fn name(&self) -> &str { "if" }
    fn validate(&self, _params: &Value) -> Result<()> { Ok(()) }
    
    fn prepare(&self, params: Value) -> Result<Box<dyn Node>> {
        let mut branches = Vec::new();
        if let Some(arr) = params.get("branches").and_then(|v| v.as_array()) {
            for b in arr {
                let cond_str = b.get("condition").and_then(|v| v.as_str()).ok_or(anyhow!("Missing condition"))?;
                let target = b.get("target").and_then(|v| v.as_u64()).ok_or(anyhow!("Missing target"))? as usize;
                
                let clean_cond = cond_str.replace("${", "").replace("}", "");
                let compiled = build_operator_tree(&clean_cond)?;
                
                branches.push(IfBranch {
                    condition: compiled,
                    target,
                    raw_cond: clean_cond,
                });
            }
        }
        
        let else_next = params.get("else_next").and_then(|v| v.as_u64()).map(|i| i as usize);
        
        Ok(Box::new(IfNode { branches, else_next }))
    }
}

#[async_trait]
impl Node for IfNode {
    async fn execute(&self, ctx: &Context, _task: &Task, syscall: &mut dyn Syscall) -> Result<()> {
        let mut eval_ctx = HashMapContext::<DefaultNumericTypes>::new();
        for r in ctx.variables.iter() {
            let (k, v) = (r.key(), r.value());
            let eval_val = match v {
                Value::String(s) => Some(evalexpr::Value::String(s.clone())),
                Value::Number(n) => {
                    if let Some(i) = n.as_i64() { Some(evalexpr::Value::Int(i)) }
                    else if let Some(f) = n.as_f64() { Some(evalexpr::Value::Float(f)) }
                    else { None }
                },
                Value::Bool(b) => Some(evalexpr::Value::Boolean(*b)),
                _ => None, 
            };
            if let Some(ev) = eval_val {
                let _ = eval_ctx.set_value(k.clone(), ev);
            }
        }

        let mut matched = false;
        for branch in &self.branches {
            let result = branch.condition.eval_boolean_with_context(&eval_ctx)
                .unwrap_or_else(|e| {
                    eprintln!("Eval failed for '{}': {}", branch.raw_cond, e);
                    false
                });
            
            if result {
                syscall.jump(branch.target);
                matched = true;
                break;
            }
        }
        
        if !matched {
            if let Some(idx) = self.else_next {
                syscall.jump(idx);
            }
        }
        Ok(())
    }
}

// --- FORK NODE ---

#[derive(Debug)]
pub struct ForkNode {
    targets: Vec<usize>,
}

pub struct ForkDefinition;

impl NodeDefinition for ForkDefinition {
    fn name(&self) -> &str { "fork" }
    fn validate(&self, _params: &Value) -> Result<()> { Ok(()) }
    fn prepare(&self, params: Value) -> Result<Box<dyn Node>> {
        let mut targets = Vec::new();
        if let Some(arr) = params.get("targets").and_then(|v| v.as_array()) {
            for t in arr {
                if let Some(idx) = t.as_u64() {
                    targets.push(idx as usize);
                }
            }
        }
        Ok(Box::new(ForkNode { targets }))
    }
}

#[async_trait]
impl Node for ForkNode {
    async fn execute(&self, _ctx: &Context, _task: &Task, syscall: &mut dyn Syscall) -> Result<()> {
        syscall.fork(self.targets.clone());
        Ok(())
    }
}

// --- JOIN NODE ---

#[derive(Debug)]
pub struct JoinNode {
    next: Option<usize>,
    expect_count: usize,
}

pub struct JoinDefinition;

impl NodeDefinition for JoinDefinition {
    fn name(&self) -> &str { "join" }
    fn validate(&self, _params: &Value) -> Result<()> { Ok(()) }
    fn prepare(&self, params: Value) -> Result<Box<dyn Node>> {
        let next = params.get("next").and_then(|v| v.as_u64()).map(|i| i as usize);
        let expect_count = params.get("expect_count").and_then(|v| v.as_u64()).ok_or(anyhow!("Missing expect_count"))? as usize;
        Ok(Box::new(JoinNode { next, expect_count }))
    }
}

#[async_trait]
impl Node for JoinNode {
    async fn execute(&self, ctx: &Context, task: &Task, syscall: &mut dyn Syscall) -> Result<()> {
        let should_proceed = {
            let counter = ctx.pending_joins
                .entry(task.node_index)
                .or_insert_with(|| AtomicUsize::new(self.expect_count));
            
            let prev = counter.fetch_sub(1, Ordering::SeqCst);
            prev == 1
        };

        if should_proceed {
            ctx.pending_joins.remove(&task.node_index);
            if let Some(target) = self.next {
                syscall.jump(target);
            }
        } else {
            syscall.wait();
        }
        Ok(())
    }
}