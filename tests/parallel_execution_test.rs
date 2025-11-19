use skript::runtime::{engine::Engine, context::Context};
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
async fn test_parallel_execution_performance() -> Result<()> {
    // 1. Setup Engine
    let mut engine = Engine::new();
    engine.register_node(Box::new(StartDefinition));
    engine.register_node(Box::new(EndDefinition));
    engine.register_node(Box::new(ForkDefinition));
    engine.register_node(Box::new(JoinDefinition));
    engine.register_function(Arc::new(AssignAction));
    
    let sleep_ms = 300;
    engine.register_function(Arc::new(SleepAction { duration_ms: sleep_ms })); 

    // 2. Construct Workflow
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
        id: "parallel_perf_test".to_string(),
        name: "Parallel Performance Test".to_string(),
        variables: HashMap::new(),
        nodes: vec![
            Node { id: "start".to_string(), kind: NodeType::Start },
            Node { 
                id: "par".to_string(), 
                kind: NodeType::Parallel { 
                    branches: vec![
                        create_sleep_branch("1"), 
                        create_sleep_branch("2"), 
                        create_sleep_branch("3"),
                        create_sleep_branch("4"),
                    ] 
                } 
            },
            // Add a flag setting node to know when we are done
            Node {
                 id: "set_done".to_string(),
                 kind: NodeType::Assign {
                     assignments: vec![
                         HashMap::from([
                             ("key".to_string(), json!("finished")),
                             ("value".to_string(), json!(true))
                         ])
                     ],
                     expression: None
                 }
            },
            Node { id: "end".to_string(), kind: NodeType::End { output: "finished".to_string() } }
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
    let cpu_count = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(1);
    let worker_count = std::cmp::max(4, cpu_count * 2);
    
    println!("Testing with {} workers...", worker_count);

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
    
    // Max wait: 1000ms.
    // Serial execution (4 * 300ms = 1200ms) would fail this check.
    // Parallel execution (300ms + overhead) should pass easily.
    for _ in 0..10 { 
        tokio::time::sleep(Duration::from_millis(100)).await;
        if let Some(val) = engine.get_instance_var(instance_id, "finished").await {
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

    assert!(finished, "Workflow did not finish in time. Duration so far: {}ms. Expected < 1000ms.", duration.as_millis());
    println!("Parallel execution finished in {}ms", duration.as_millis());

    Ok(())
}
