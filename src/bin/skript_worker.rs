use clap::Parser;
use skript::runtime::engine::Engine;
use skript::runtime::redis_storage::{RedisStateStore, RedisTaskQueue};
use skript::actions::builtin::{LogAction, AssignAction};
use skript::nodes::common::{StartDefinition, EndDefinition};
use skript::nodes::flow::{IfDefinition, ForkDefinition, JoinDefinition};
use skript::compiler::core::Compiler;
use skript::compiler::loader::load_workflow_from_yaml;
use skript::actions::FunctionHandler;
use skript::runtime::context::Context;
use std::sync::Arc;
use std::process;
use anyhow::Result;
use async_trait::async_trait;
use serde_json::{Value, json};

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Redis connection URL
    #[arg(long, default_value = "redis://127.0.0.1:6379/0")]
    redis: String,

    /// Path to the workflow YAML file to load
    #[arg(long)]
    workflow: String,
    
    /// Worker Name (for logging)
    #[arg(long, default_value = "worker")]
    name: String,
}

// --- Special Debug Action to leak Process Info ---
#[derive(Debug)]
struct SysInfoAction;

#[async_trait]
impl FunctionHandler for SysInfoAction {
    fn name(&self) -> &str { "sys_info" }
    fn validate(&self, _params: &Value) -> Result<()> { Ok(()) }
    async fn execute(&self, _params: Value, _ctx: &Context) -> Result<Value> {
        let pid = process::id();
        Ok(json!({
            "pid": pid
        }))
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();
    println!("[{}] Starting... Redis: {}, Workflow: {}", args.name, args.redis, args.workflow);

    // 1. Setup Storage
    let client = redis::Client::open(args.redis.clone()).expect("Invalid Redis URL");
    let store = Arc::new(RedisStateStore::new(client.clone()));
    let queue = Arc::new(RedisTaskQueue::new(client, "skript:distributed:tasks".to_string()));

    let mut engine = Engine::new_with_storage(store, queue);

    // 2. Register Standard Nodes
    engine.register_node(Box::new(StartDefinition));
    engine.register_node(Box::new(EndDefinition));
    engine.register_node(Box::new(IfDefinition));
    engine.register_node(Box::new(ForkDefinition));
    engine.register_node(Box::new(JoinDefinition));
    engine.register_function(Arc::new(LogAction));
    engine.register_function(Arc::new(AssignAction));
    
    // 3. Register SysInfo Action
    engine.register_function(Arc::new(SysInfoAction));

    // 4. Load & Compile Workflow
    let workflow = load_workflow_from_yaml(&args.workflow).expect("Failed to load workflow");
    let mut compiler = Compiler::new();
    let blueprint = compiler.compile(workflow).expect("Failed to compile workflow");
    engine.register_blueprint(blueprint);

    println!("[{}] Ready. Waiting for tasks...", args.name);

    // 5. Run Loop
    engine.run_worker().await;

    Ok(())
}
