use clap::{Parser, Subcommand};
use skript::runtime::engine::Engine;
use skript::runtime::storage::{InMemoryStateStore, InMemoryTaskQueue};
use skript::runtime::redis_storage::{RedisStateStore, RedisTaskQueue};
use skript::actions::builtin::{LogAction, AssignAction};
use skript::nodes::common::{StartDefinition, EndDefinition};
use skript::nodes::flow::{IfDefinition, ForkDefinition, JoinDefinition, IterationDefinition, LoopDefinition};
use skript::compiler::core::Compiler;
use skript::compiler::loader::load_workflow_from_yaml;
use std::sync::Arc;
use std::collections::HashMap;
use std::path::PathBuf;
use anyhow::Result;
use tracing::{info, error};
use tracing_subscriber;
use std::fs;

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Run a workflow locally in memory (Standalone Mode)
    Run {
        /// Path to the workflow YAML file
        #[arg(long, short)]
        file: PathBuf,

        /// Initial variables (key=value)
        #[arg(long, short = 'D', value_parser = parse_key_val)]
        vars: Vec<(String, serde_json::Value)>,
    },

    /// Start a worker node connecting to Redis (Distributed Mode)
    Worker {
        /// Redis connection URL
        #[arg(long, default_value = "redis://127.0.0.1:6379/0")]
        redis: String,

        /// Worker Name (for logging)
        #[arg(long, default_value = "worker")]
        name: String,

        /// Directory containing workflow YAML files to preload
        #[arg(long)]
        workflows: Option<PathBuf>,
    },

    /// Submit a workflow to Redis for workers to execute (Client Mode)
    Submit {
        /// Path to the workflow YAML file
        #[arg(long, short)]
        file: PathBuf,

        /// Redis connection URL
        #[arg(long, default_value = "redis://127.0.0.1:6379/0")]
        redis: String,

        /// Initial variables (key=value)
        #[arg(long, short = 'D', value_parser = parse_key_val)]
        vars: Vec<(String, serde_json::Value)>,
    },
    /// Run automated benchmark
    Bench,
}

fn parse_key_val(s: &str) -> Result<(String, serde_json::Value), String> {
    let pos = s.find('=').ok_or_else(|| format!("invalid KEY=value: no `=` found in `{}`", s))?;
    let key = s[..pos].to_string();
    let val_str = &s[pos + 1..];
    // Try parsing as JSON, otherwise treat as string
    let val = serde_json::from_str(val_str).unwrap_or_else(|_| serde_json::Value::String(val_str.to_string()));
    Ok((key, val))
}

fn register_standard_components(engine: &mut Engine) {
    engine.register_node(Box::new(StartDefinition));
    engine.register_node(Box::new(EndDefinition));
    engine.register_node(Box::new(IfDefinition));
    engine.register_node(Box::new(ForkDefinition));
    engine.register_node(Box::new(JoinDefinition));
    engine.register_node(Box::new(IterationDefinition));
    engine.register_node(Box::new(LoopDefinition));

    engine.register_function(Arc::new(LogAction));
    engine.register_function(Arc::new(AssignAction));
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();
    let cli = Cli::parse();

    match cli.command {
        Commands::Bench => {
            use skript::benchmark::BenchmarkRunner;
            let runner = BenchmarkRunner::new();
            runner.auto_tune().await?;
        }
        Commands::Run { file, vars } => {
            info!("Running in Standalone Memory Mode");
            let mut engine = Engine::new(); // Defaults to Memory
            register_standard_components(&mut engine);

            let workflow = load_workflow_from_yaml(file.to_str().unwrap())?;
            let workflow_id = workflow.id.clone();
            
            let mut compiler = Compiler::new();
            let blueprint = compiler.compile(workflow)?;
            engine.register_blueprint(blueprint);

            let initial_vars: HashMap<_, _> = vars.into_iter().collect();
            let instance_id = engine.start_workflow(&workflow_id, initial_vars).await?;
            
            info!("Workflow started: {}", instance_id);
            engine.run_worker().await;
            info!("Workflow finished.");
        }

        Commands::Worker { redis, name, workflows } => {
            info!("[{}] Starting Worker... Redis: {}", name, redis);
            
            let client = redis::Client::open(redis).expect("Invalid Redis URL");
            let store = Arc::new(RedisStateStore::new(client.clone()));
            let queue = Arc::new(RedisTaskQueue::new(client, "skript:distributed:tasks".to_string()));

            let mut engine = Engine::new_with_storage(store, queue);
            register_standard_components(&mut engine);

            if let Some(dir) = workflows {
                info!("Loading workflows from: {:?}", dir);
                if let Ok(entries) = fs::read_dir(dir) {
                    let mut compiler = Compiler::new();
                    for entry in entries.flatten() {
                        let path = entry.path();
                        if let Some(ext) = path.extension().and_then(|s| s.to_str()) {
                            if ext == "yaml" || ext == "yml" {
                                match load_workflow_from_yaml(path.to_str().unwrap()) {
                                    Ok(wf) => {
                                        info!("Loaded workflow: {}", wf.id);
                                        match compiler.compile(wf) {
                                            Ok(bp) => engine.register_blueprint(bp),
                                            Err(e) => error!("Failed to compile {}: {}", path.display(), e),
                                        }
                                    },
                                    Err(e) => error!("Failed to load {}: {}", path.display(), e),
                                }
                            }
                        }
                    }
                }
            }

            info!("Worker ready.");
            engine.run_worker().await;
        }

        Commands::Submit { file, redis, vars } => {
            info!("Submitting to Redis: {}", redis);
            
            let client = redis::Client::open(redis).expect("Invalid Redis URL");
            let store = Arc::new(RedisStateStore::new(client.clone()));
            let queue = Arc::new(RedisTaskQueue::new(client, "skript:distributed:tasks".to_string()));

            let mut engine = Engine::new_with_storage(store, queue);
            register_standard_components(&mut engine);

            let workflow = load_workflow_from_yaml(file.to_str().unwrap())?;
            let workflow_id = workflow.id.clone();
            
            let mut compiler = Compiler::new();
            let blueprint = compiler.compile(workflow)?;
            
            // In a real system, we would push this Blueprint to Redis so workers can fetch it.
            // For now, we just register it locally to allow 'start_workflow' validation to pass.
            engine.register_blueprint(blueprint);

            let initial_vars: HashMap<_, _> = vars.into_iter().collect();
            let instance_id = engine.start_workflow(&workflow_id, initial_vars).await?;
            
            info!("Workflow submitted successfully! Instance ID: {}", instance_id);
        }
    }

    Ok(())
}
