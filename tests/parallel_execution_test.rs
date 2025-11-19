use skript::runtime::{engine::Engine, context::Context, syscall::Syscall};
use skript::actions::FunctionHandler;
use skript::dsl::{Workflow, Node, NodeType, Edge, Branch};
use skript::compiler::core::Compiler;
use skript::nodes::common::{StartDefinition, EndDefinition};
use skript::nodes::flow::{ForkDefinition, JoinDefinition};
use skript::actions::builtin::AssignAction;
use async_trait::async_trait;
use serde_json::{json, Value};
use std::sync::Arc;
use std::collections::HashMap;
use std::time::{Duration, Instant};
use anyhow::Result;

#[derive(Debug)]
struct SleepAction {
    duration_ms: u64,
}

#[async_trait]
impl FunctionHandler for SleepAction {
    fn name(&self) -> &str { "sleep" }
    fn validate(&self, _params: &Value) -> Result<()> { Ok(()) }
    async fn execute(&self, _params: Value, _ctx: &Context) -> Result<Value> {
        tokio::time::sleep(Duration::from_millis(self.duration_ms)).await;
        Ok(json!({ "slept": true }))
    }
}

#[tokio::test]
async fn test_parallel_execution() -> Result<()> {
    // 1. Setup Engine
    let mut engine = Engine::new();
    engine.register_node(Box::new(StartDefinition));
    engine.register_node(Box::new(EndDefinition));
    engine.register_node(Box::new(ForkDefinition));
    engine.register_node(Box::new(JoinDefinition));
    engine.register_function(Arc::new(AssignAction));
    
    // Register Sleep Action (500ms)
    engine.register_function(Arc::new(SleepAction { duration_ms: 500 })); 

    // 2. Construct Workflow: Start -> Fork(3x Sleep) -> Join -> Assign(done=true) -> End
    
    fn create_sleep_branch(id_suffix: &str) -> Branch {
        Branch {
            nodes: vec![
                Node {
                    id: format!("sleep_{}", id_suffix),
                    kind: NodeType::Function { 
                        name: "sleep".to_string(), 
                        params: HashMap::new(), 
                        output: None 
                    }
                }
            ]
        }
    }

    let workflow = Workflow {
        id: "parallel_test".to_string(),
        name: "Parallel Test".to_string(),
        variables: HashMap::new(),
        nodes: vec![
            Node { id: "start".to_string(), kind: NodeType::Start },
            Node { 
                id: "par".to_string(), 
                kind: NodeType::Parallel { 
                    branches: vec![
                        create_sleep_branch("1"), 
                        create_sleep_branch("2"), 
                        create_sleep_branch("3")
                    ] 
                } 
            },
            Node {
                 id: "set_done".to_string(),
                 kind: NodeType::Assign {
                     // assignments: [{"key": "done", "value": true}]
                     assignments: vec![
                         HashMap::from([
                             ("key".to_string(), json!("done")),
                             ("value".to_string(), json!(true))
                         ])
                     ],
                     expression: None
                 }
            },
            Node { id: "end".to_string(), kind: NodeType::End { output: "done".to_string() } }
        ],
        edges: vec![
            Edge { source: "start".to_string(), target: "par".to_string(), condition: None, branch_type: None, branch_index: None },
            Edge { source: "par".to_string(), target: "set_done".to_string(), condition: None, branch_type: None, branch_index: None },
            Edge { source: "set_done".to_string(), target: "end".to_string(), condition: None, branch_type: None, branch_index: None },
        ]
    };

    // 3. Compile & Register
    let mut compiler = Compiler::new();
    let blueprint = compiler.compile(workflow)?;
    engine.register_blueprint(blueprint.clone());

    let engine = Arc::new(engine);
    
    // 4. Start Instance
    let instance_id = engine.start_workflow(&blueprint.id, HashMap::new()).await?;
    
    // 5. Spawn Parallel Workers
    // We spawn 4 workers to ensure 3 branches can run simultaneously
    let worker_count = 4;
    let mut handles = Vec::new();
    
    for _ in 0..worker_count {
        let e = engine.clone();
        handles.push(tokio::spawn(async move {
            e.run_worker().await;
        }));
    }

    // 6. Poll for completion
    let start = Instant::now();
    let mut finished = false;
    
    // We expect it to finish in ~500ms + overhead. 
    // If it was serial, it would be 1500ms+.
    // We poll for up to 1.2 seconds.
    for _ in 0..12 { 
        tokio::time::sleep(Duration::from_millis(100)).await;
        if let Some(val) = engine.get_instance_var(instance_id, "done").await {
            if val == json!(true) {
                finished = true;
                break;
            }
        }
    }
    
    let duration = start.elapsed();
    
    // Abort workers
    for h in handles {
        h.abort();
    }

    assert!(finished, "Workflow did not finish within expected time (1.2s). Parallelism might be broken or slow.");
    
    println!("Execution took: {}ms", duration.as_millis());
    
    // Verify Parallelism
    // Serial execution would be at least 1500ms.
    // Parallel execution should be around 500-600ms.
    assert!(duration.as_millis() < 1100, "Execution took {}ms, which suggests serial execution (expected < 1100ms)", duration.as_millis());

    Ok(())
}
