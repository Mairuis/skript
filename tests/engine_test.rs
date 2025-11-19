use skript::dsl::builder::WorkflowBuilder;
use skript::compiler::core::Compiler;
use skript::runtime::engine::Engine;
use skript::actions::builtin::{LogAction, AssignAction};
use skript::nodes::common::{StartDefinition, EndDefinition};
use skript::nodes::flow::{IfDefinition, ForkDefinition, JoinDefinition};
use skript::dsl::Node;
use skript::dsl::NodeType;
use std::collections::HashMap;
use std::time::Duration;
use std::sync::Arc;
use serde_json::json;

#[tokio::test]
async fn test_engine_linear_execution() {
    // 1. Define Workflow
    let workflow = WorkflowBuilder::new("engine-test-linear")
        .start("start")
        .function("step1", "log")
            .param("msg", "Engine is running!")
            .build()
        .function("step2", "assign")
            .param("value", "success_value")
            .output("result_var")
            .build()
        .end("end", "")
        .connect("start", "step1")
        .connect("step1", "step2")
        .connect("step2", "end")
        .build();

    // 2. Compile
    let mut compiler = Compiler::new();
    let blueprint = compiler.compile(workflow).expect("Compilation failed");

    // 3. Setup Engine
    let mut engine = Engine::new();
    // Register Standard Nodes
    engine.register_node(Box::new(StartDefinition));
    engine.register_node(Box::new(EndDefinition));
    engine.register_node(Box::new(IfDefinition));
    engine.register_node(Box::new(ForkDefinition));
    engine.register_node(Box::new(JoinDefinition));

    // Register Actions
    engine.register_function(Arc::new(LogAction));
    engine.register_function(Arc::new(AssignAction));
    
    engine.register_blueprint(blueprint);

    // 4. Start Workflow
    let instance_id = engine.start_workflow("engine-test-linear", HashMap::new())
        .await
        .expect("Failed to start workflow");

    // 5. Run Engine (with timeout)
    tokio::select! {
        _ = engine.run_worker() => {}
        _ = tokio::time::sleep(Duration::from_millis(100)) => {}
    }

    // 6. Verify State
    let result = engine.get_instance_var(instance_id, "result_var");
    assert_eq!(result, Some(json!("success_value")));
}

#[tokio::test]
async fn test_engine_if_branching() {
    let workflow = WorkflowBuilder::new("engine-test-if")
        .start("start")
        .function("init_x", "assign")
            .param("value", 20)
            .output("x")
            .build()
        .if_node("check_x")
        .function("branch_big", "assign")
            .param("value", "big_path")
            .output("path_result")
            .build()
        .function("branch_small", "assign")
            .param("value", "small_path")
            .output("path_result")
            .build()
        .end("end", "")
        .connect("start", "init_x")
        .connect("init_x", "check_x")
        .connect_if("check_x", "branch_big", "x > 10")
        .connect_else("check_x", "branch_small")
        .connect("branch_big", "end")
        .connect("branch_small", "end")
        .build();

    let mut compiler = Compiler::new();
    let blueprint = compiler.compile(workflow).expect("Compilation failed");

    let mut engine = Engine::new();
    engine.register_node(Box::new(StartDefinition));
    engine.register_node(Box::new(EndDefinition));
    engine.register_node(Box::new(IfDefinition));
    engine.register_node(Box::new(ForkDefinition));
    engine.register_node(Box::new(JoinDefinition));
    engine.register_function(Arc::new(LogAction));
    engine.register_function(Arc::new(AssignAction));
    engine.register_blueprint(blueprint);

    let instance_id = engine.start_workflow("engine-test-if", HashMap::new())
        .await
        .expect("Failed to start workflow");

    tokio::select! {
        _ = engine.run_worker() => {}
        _ = tokio::time::sleep(Duration::from_millis(100)) => {}
    }

    let result = engine.get_instance_var(instance_id, "path_result");
    assert_eq!(result, Some(json!("big_path")));
    
    let x_val = engine.get_instance_var(instance_id, "x");
    assert_eq!(x_val, Some(json!(20)));
}

#[tokio::test]
async fn test_engine_parallel_join() {
    let branch1 = vec![
        Node { 
            id: "B1".to_string(), 
            kind: NodeType::Function { 
                name: "assign".to_string(), 
                params: HashMap::from([("value".to_string(), json!(true))]), 
                output: Some("b1".to_string()) 
            } 
        }
    ];
    
    let branch2 = vec![
        Node { 
            id: "B2".to_string(), 
            kind: NodeType::Function { 
                name: "assign".to_string(), 
                params: HashMap::from([("value".to_string(), json!(true))]), 
                output: Some("b2".to_string()) 
            } 
        }
    ];

    let workflow = WorkflowBuilder::new("engine-test-parallel")
        .start("start")
        .parallel("p1", vec![branch1, branch2])
        .function("after_join", "assign")
            .param("value", "done")
            .output("final_status")
            .build()
        .end("end", "")
        .connect("start", "p1")
        .connect("p1", "after_join")
        .connect("after_join", "end")
        .build();

    let mut compiler = Compiler::new();
    let blueprint = compiler.compile(workflow).expect("Compilation failed");

    let mut engine = Engine::new();
    engine.register_node(Box::new(StartDefinition));
    engine.register_node(Box::new(EndDefinition));
    engine.register_node(Box::new(IfDefinition));
    engine.register_node(Box::new(ForkDefinition));
    engine.register_node(Box::new(JoinDefinition));
    engine.register_function(Arc::new(AssignAction));
    engine.register_blueprint(blueprint);

    let instance_id = engine.start_workflow("engine-test-parallel", HashMap::new())
        .await
        .expect("Failed to start workflow");

    tokio::select! {
        _ = engine.run_worker() => {}
        _ = tokio::time::sleep(Duration::from_millis(200)) => {}
    }

    let b1 = engine.get_instance_var(instance_id, "b1");
    let b2 = engine.get_instance_var(instance_id, "b2");
    let status = engine.get_instance_var(instance_id, "final_status");

    assert_eq!(b1, Some(json!(true)), "Branch 1 should execute");
    assert_eq!(b2, Some(json!(true)), "Branch 2 should execute");
    assert_eq!(status, Some(json!("done")), "Flow should pass join and reach end");
}
