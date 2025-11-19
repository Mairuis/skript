use clap::{Parser, Subcommand};
use skript::compiler::{loader, core::Compiler};
use skript::runtime::engine::Engine;
use skript::nodes::common::{StartDefinition, EndDefinition};
use skript::nodes::flow::{IfDefinition, ForkDefinition, JoinDefinition, IterationDefinition, LoopDefinition};
use skript::actions::builtin::{LogAction, AssignAction};
use skript::actions::http::HttpAction;
use std::sync::Arc;
use std::path::PathBuf;
use std::collections::HashMap;
use tracing::info;

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Run a workflow file
    Run {
        /// Path to the workflow YAML file
        file: PathBuf,
    },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize logging
    tracing_subscriber::fmt::init();

    let cli = Cli::parse();

    match &cli.command {
        Commands::Run { file } => {
            info!("Loading workflow from: {:?}", file);

            // 1. Load DSL
            let workflow = loader::load_workflow_from_yaml(&file.to_string_lossy())?;
            info!("Loaded workflow: {}", workflow.id);

            // 2. Compile
            let mut compiler = Compiler::new();
            let blueprint = compiler.compile(workflow)?;
            info!("Compiled blueprint with {} nodes.", blueprint.nodes.len());

            // 3. Setup Engine
            let mut engine = Engine::new();
            
            // Register Standard Nodes
            engine.register_node(Box::new(StartDefinition));
            engine.register_node(Box::new(EndDefinition));
            engine.register_node(Box::new(IfDefinition));
            engine.register_node(Box::new(ForkDefinition));
            engine.register_node(Box::new(JoinDefinition));
            engine.register_node(Box::new(IterationDefinition));
            engine.register_node(Box::new(LoopDefinition));

            // Register Actions
            engine.register_function(Arc::new(LogAction));
            engine.register_function(Arc::new(AssignAction));
            engine.register_function(Arc::new(HttpAction::new()));
            
            engine.register_blueprint(blueprint.clone());

            // 4. Start Execution
            let instance_id = engine.start_workflow(&blueprint.id, HashMap::new()).await?;
            info!("Started instance: {}", instance_id);

            // 5. Run Worker
            // In CLI mode, we want to run until completion.
            // Since our engine runs indefinitely, we might need a signal to stop.
            // For now, we run and wait for Ctrl+C or just let it run.
            // A better way for CLI is to wait until the workflow status is "Completed".
            // But our Engine doesn't expose status polling yet.
            
            info!("Running... (Press Ctrl+C to stop)");
            engine.run_worker().await;
        }
    }

    Ok(())
}
