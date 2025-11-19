use std::process::{Command, Child};
use std::time::Duration;
use std::sync::Arc;
use skript::runtime::redis_storage::{RedisStateStore, RedisTaskQueue};
use skript::runtime::engine::Engine;
use skript::compiler::core::Compiler;
use skript::compiler::loader::load_workflow_from_yaml;
use skript::nodes::common::{StartDefinition, EndDefinition};
use skript::nodes::flow::{IfDefinition, ForkDefinition, JoinDefinition};
use skript::actions::builtin::AssignAction;
use std::collections::HashMap;
use serde_json::Value;

fn build_worker_binary() {
    println!("Building distributed_worker example...");
    let status = Command::new("cargo")
        .args(&["build", "--example", "distributed_worker"]) 
        .status()
        .expect("Failed to build worker example");
    assert!(status.success(), "Build failed");
}

fn spawn_worker(redis_url: &str, workflow_path: &str, name: &str) -> Child {
    let bin_path = "./target/debug/examples/distributed_worker";
    Command::new(bin_path)
        .arg("--redis")
        .arg(redis_url)
        .arg("--workflow")
        .arg(workflow_path)
        .arg("--name")
        .arg(name)
        .spawn()
        .expect("Failed to start worker process")
}

use skript::actions::FunctionHandler;
use skript::runtime::context::Context;
use anyhow::Result;
use async_trait::async_trait;

// Dummy handler for client-side validation
#[derive(Debug)]
struct DummySysInfo;
#[async_trait]
impl FunctionHandler for DummySysInfo {
    fn name(&self) -> &str { "sys_info" }
    fn validate(&self, _params: &Value) -> Result<()> { Ok(()) }
    async fn execute(&self, _params: Value, _ctx: &Context) -> Result<Value> { Ok(Value::Null) }
}

#[tokio::test]
#[ignore]
async fn test_multi_process_execution() {
    // 0. Config
    let redis_host = "localhost";
    let redis_port = 6379;
    let redis_pwd = "difyai123456";
    let redis_db = 6;
    let redis_url = format!("redis://:{}@{}:{}/{}", redis_pwd, redis_host, redis_port, redis_db);
    
    let workflow_path = "tests/fixtures/distributed_flow.yaml";
    
    // 1. Build Binary
    build_worker_binary();

    // 2. Clean Redis
    let client = redis::Client::open(redis_url.clone()).unwrap();
    let mut conn = client.get_connection().unwrap();
    let _: () = redis::cmd("FLUSHDB").query(&mut conn).unwrap();
    println!("Redis flushed.");

    // 3. Prepare Initial Task manually (Simulating an API trigger)
    // We need to load the workflow to get the start index
    let workflow_def = load_workflow_from_yaml(workflow_path).unwrap();
    let mut compiler = Compiler::new();
    let blueprint = compiler.compile(workflow_def).unwrap();
    let start_index = blueprint.start_index;
    let workflow_id = blueprint.id.clone();

    let store = Arc::new(RedisStateStore::new(client.clone()));
    let queue = Arc::new(RedisTaskQueue::new(client.clone(), "skript:distributed:tasks".to_string()));
    
    let mut engine = Engine::new_with_storage(store.clone(), queue.clone());
    
    // Register definitions needed for start_workflow check
    engine.register_node(Box::new(StartDefinition));
    engine.register_node(Box::new(EndDefinition));
    engine.register_node(Box::new(IfDefinition));
    engine.register_node(Box::new(ForkDefinition));
    engine.register_node(Box::new(JoinDefinition));
    engine.register_function(Arc::new(AssignAction));
    engine.register_function(Arc::new(DummySysInfo)); // Register Dummy
    
    engine.register_blueprint(blueprint); // Register here!

    println!("Starting workflow: {}", workflow_id);
    let instance_id = engine.start_workflow(&workflow_id, HashMap::new())
        .await
        .expect("Failed to trigger workflow");

    // 4. Spawn 2 Worker Processes
    let mut worker1 = spawn_worker(&redis_url, workflow_path, "worker-1");
    let mut worker2 = spawn_worker(&redis_url, workflow_path, "worker-2");

    println!("Workers spawned. Waiting for execution...");

    // 5. Wait Loop (Poll Redis for result)
    let mut success = false;
    for _ in 0..20 { // Wait up to 10 seconds
        tokio::time::sleep(Duration::from_millis(500)).await;
        
        // Check if both variables are set
        let info1 = engine.get_instance_var(instance_id, "worker_info_1").await;
        let info2 = engine.get_instance_var(instance_id, "worker_info_2").await;
        
        if info1.is_some() && info2.is_some() {
            println!("Execution finished!");
            
            let v1 = info1.unwrap();
            let v2 = info2.unwrap();
            println!("Worker 1 Info: {:?}", v1);
            println!("Worker 2 Info: {:?}", v2);
            
            let pid1 = v1.get("pid").and_then(|v| v.as_u64()).unwrap();
            let pid2 = v2.get("pid").and_then(|v| v.as_u64()).unwrap();
            
            println!("PID 1: {}, PID 2: {}", pid1, pid2);
            
            // Verify they are executed by our workers (PIDs match child processes)
            let w1_pid = worker1.id() as u64;
            let w2_pid = worker2.id() as u64;
            let test_runner_pid = std::process::id() as u64;
            
            println!("Test Runner PID: {}", test_runner_pid);
            println!("Spawned Workers: Worker1={}, Worker2={}", w1_pid, w2_pid);

            // Assertion 1: Tasks must NOT be executed by the test runner (local engine)
            assert_ne!(pid1, test_runner_pid, "Task 1 was executed by local runner, not worker!");
            assert_ne!(pid2, test_runner_pid, "Task 2 was executed by local runner, not worker!");

            // Assertion 2: Tasks MUST be executed by one of the spawned workers
            assert!(pid1 == w1_pid || pid1 == w2_pid, "PID 1 {} matches neither W1 {} nor W2 {}", pid1, w1_pid, w2_pid);
            assert!(pid2 == w1_pid || pid2 == w2_pid, "PID 2 {} matches neither W1 {} nor W2 {}", pid2, w1_pid, w2_pid);
            
            // Optional: Assert they are different if we want to prove parallelism
            if pid1 != pid2 {
                println!("\n✅ SUCCESS: Parallel execution confirmed on different processes!");
                println!("   Branch 1 -> Worker (PID {})", pid1);
                println!("   Branch 2 -> Worker (PID {})", pid2);
            } else {
                println!("\n⚠️  WARNING: One worker executed both branches (Race Condition).");
                println!("   Worker (PID {}) handled both.", pid1);
            }

            success = true;
            break;
        }
    }

    // 6. Cleanup
    let _ = worker1.kill();
    let _ = worker2.kill();

    assert!(success, "Workflow execution timed out or failed");
}
