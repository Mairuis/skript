use skript::runtime::engine::Engine;
use skript::runtime::redis_storage::{RedisStateStore, RedisTaskQueue};
use skript::actions::builtin::AssignAction;
use skript::nodes::common::{StartDefinition, EndDefinition};
use skript::nodes::flow::{IfDefinition, ForkDefinition, JoinDefinition};
use skript::compiler::core::Compiler;
use skript::dsl::builder::WorkflowBuilder;
use std::sync::Arc;
use std::collections::HashMap;
use std::time::Duration;
use serde_json::json;
use redis::Client;

// Helper to get redis client from env config provided by user
fn get_redis_client() -> Client {
    let host = "localhost";
    let port = 6379;
    let password = "difyai123456";
    let db = 6;
    
    // Format: redis://:password@host:port/db
    let url = format!("redis://:{}@{}:{}/{}", password, host, port, db);
    redis::Client::open(url).expect("Invalid Redis URL")
}

#[tokio::test]
#[ignore] // Ignored by default, run explicitly if redis is available
async fn test_redis_distributed_execution() {
    // 1. Setup Redis & Clean DB
    let client = get_redis_client();
    let mut conn = client.get_multiplexed_async_connection().await.expect("Failed to connect to Redis");
    let _: () = redis::cmd("FLUSHDB").query_async(&mut conn).await.expect("Failed to flush db");

    println!("Connected to Redis and flushed DB 6.");

    // 2. Setup Components
    let store = Arc::new(RedisStateStore::new(client.clone()));
    // Use a unique queue name for this test
    let queue = Arc::new(RedisTaskQueue::new(client.clone(), "skript:test:queue".to_string()));

    let mut engine = Engine::new_with_storage(store.clone(), queue.clone());
    
    // Register definitions
    engine.register_node(Box::new(StartDefinition));
    engine.register_node(Box::new(EndDefinition));
    engine.register_node(Box::new(IfDefinition));
    engine.register_node(Box::new(ForkDefinition));
    engine.register_node(Box::new(JoinDefinition));
    engine.register_function(Arc::new(AssignAction));

    // 3. Create a Workflow
    // Start -> Assign(a=1) -> Assign(b=a+1) -> End
    let workflow = WorkflowBuilder::new("redis-test-flow")
        .start("start")
        .function("step1", "assign")
            .param("value", 1)
            .output("a")
            .build()
        .function("step2", "assign")
            .param("expression", "b = a + 10")
            .build()
        .end("end", "b")
        .connect("start", "step1")
        .connect("step1", "step2")
        .connect("step2", "end")
        .build();

    let mut compiler = Compiler::new();
    let blueprint = compiler.compile(workflow).expect("Compilation failed");
    engine.register_blueprint(blueprint);

    // 4. Start "Worker" in background (Simulate a separate process)
    // We clone engine components to move into the task
    let store_for_worker = store.clone();
    let queue_for_worker = queue.clone();
    let mut worker_engine = Engine::new_with_storage(store_for_worker, queue_for_worker);
    
    // Re-register definitions for worker engine (since registry is not shared in this simulation setup, 
    // though Engine struct clones hold references, the registry is per Engine instance)
    worker_engine.register_node(Box::new(StartDefinition));
    worker_engine.register_node(Box::new(EndDefinition));
    worker_engine.register_node(Box::new(IfDefinition));
    worker_engine.register_node(Box::new(ForkDefinition));
    worker_engine.register_node(Box::new(JoinDefinition));
    worker_engine.register_function(Arc::new(AssignAction));
    
    let mut worker_compiler = Compiler::new();
    worker_engine.register_blueprint(worker_compiler.compile(
        WorkflowBuilder::new("redis-test-flow")
        .start("start")
        .function("step1", "assign").param("value", 1).output("a").build()
        .function("step2", "assign").param("expression", "b = a + 10").build()
        .end("end", "b")
        .connect("start", "step1").connect("step1", "step2").connect("step2", "end")
        .build()
    ).unwrap());

    let worker_handle = tokio::spawn(async move {
        println!("Worker started...");
        // Run for a limited time to process tasks
        let _ = tokio::time::timeout(Duration::from_secs(5), worker_engine.run_worker()).await;
        println!("Worker stopped.");
    });

    // 5. Submit Workflow
    println!("Submitting workflow...");
    let instance_id = engine.start_workflow("redis-test-flow", HashMap::new())
        .await
        .expect("Failed to start workflow");
    
    println!("Workflow started: {}", instance_id);

    // 6. Wait for execution
    worker_handle.await.expect("Worker thread panicked");

    // 7. Verify Result from Redis
    // The variable "b" should be 1 + 10 = 11.
    // Note: Engine::get_instance_var uses the configured store (Redis)
    let val = engine.get_instance_var(instance_id, "b").await;
    
    println!("Result from Redis: {:?}", val);
    assert_eq!(val, Some(json!(11)));
    
    // Verify Workflow Output var
    let output = engine.get_instance_var(instance_id, "_WORKFLOW_OUTPUT").await;
    assert_eq!(output, Some(json!(11)));
}
