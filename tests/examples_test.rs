use skript::compiler::core::Compiler;
use skript::runtime::engine::Engine;
use skript::actions::builtin::{LogAction, AssignAction};
use skript::nodes::common::{StartDefinition, EndDefinition};
use skript::nodes::flow::{IfDefinition, ForkDefinition, JoinDefinition, IterationDefinition, LoopDefinition};
use skript::compiler::loader::load_workflow_from_yaml;
use std::sync::Arc;
use std::collections::HashMap;
use std::time::Duration;
use std::path::Path;

async fn run_example(file_name: &str) {
    let path = Path::new("dsl_examples").join(file_name);
    println!("Running example: {:?}", path);
    
    let workflow = load_workflow_from_yaml(path.to_str().unwrap()).expect("Failed to load workflow");
    let workflow_id = workflow.id.clone();
    
    let mut compiler = Compiler::new();
    let blueprint = compiler.compile(workflow).expect("Compilation failed");

    let mut engine = Engine::new();
    engine.register_node(Box::new(StartDefinition));
    engine.register_node(Box::new(EndDefinition));
    engine.register_node(Box::new(IfDefinition));
    engine.register_node(Box::new(ForkDefinition));
    engine.register_node(Box::new(JoinDefinition));
    engine.register_node(Box::new(IterationDefinition));
    engine.register_node(Box::new(LoopDefinition));

    engine.register_function(Arc::new(LogAction));
    engine.register_function(Arc::new(AssignAction));
    
    // Mock other actions used in examples
    // "http_request", "inventory_service", "email_service", "sms_service", "db_update"
    // For now, we map them to LogAction or a MockAction to prevent failure.
    // Engine needs exact name matching.
    
    // We can register generic MockAction that prints params.
    #[derive(Debug)]
    struct MockAction(String);
    use async_trait::async_trait;
    use skript::actions::FunctionHandler;
    use skript::runtime::context::Context;
    use serde_json::Value;
    use anyhow::Result;

    #[async_trait]
    impl FunctionHandler for MockAction {
        fn name(&self) -> &str { &self.0 }
        fn validate(&self, _params: &Value) -> Result<()> { Ok(()) }
        async fn execute(&self, params: Value, _ctx: &Context) -> Result<Value> {
            println!("[MOCK] Action: {}, Params: {:?}", self.0, params);
            // Return dummy object for checks like user_profile.is_vip
            // Check complex_flow requirements
            if self.0 == "http_request" {
                 return Ok(serde_json::json!({
                     "is_vip": true,
                     "email": "test@test.com",
                     "phone": "123"
                 }));
            }
            if self.0 == "inventory_service" {
                return Ok(serde_json::json!({ "ok": true }));
            }
            Ok(Value::Null)
        }
    }

    engine.register_function(Arc::new(MockAction("http_request".to_string())));
    engine.register_function(Arc::new(MockAction("inventory_service".to_string())));
    engine.register_function(Arc::new(MockAction("email_service".to_string())));
    engine.register_function(Arc::new(MockAction("sms_service".to_string())));
    engine.register_function(Arc::new(MockAction("db_update".to_string())));
    engine.register_function(Arc::new(MockAction("js_eval".to_string())));
    engine.register_function(Arc::new(MockAction("run_workflow".to_string())));
    engine.register_function(Arc::new(MockAction("sleep".to_string())));

    engine.register_blueprint(blueprint);

    let _instance_id = engine.start_workflow(&workflow_id, HashMap::new())
        .await
        .expect("Failed to start workflow");

    // Run for a bit
    tokio::select! {
        _ = engine.run_worker() => {}
        _ = tokio::time::sleep(Duration::from_millis(500)) => {}
    }
    
    println!("Example {} finished.", file_name);
}

#[tokio::test]
async fn test_example_assign_node() {
    run_example("assign_node.yaml").await;
}

#[tokio::test]
async fn test_example_complex_flow() {
    run_example("complex_flow.yaml").await;
}

#[tokio::test]
async fn test_example_function_node() {
    run_example("function_node.yaml").await;
}

#[tokio::test]
async fn test_example_if_node() {
    run_example("if_node.yaml").await;
}

#[tokio::test]
async fn test_example_iteration_node() {
    run_example("iteration_node.yaml").await;
}

#[tokio::test]
async fn test_example_loop_node() {
    // Loop node might use 'loop' type or 'iteration'?
    // Let's check loop_node.yaml content if it fails.
    // Assuming it uses standard nodes.
    run_example("loop_node.yaml").await;
}

#[tokio::test]
async fn test_example_simple_parallel() {
    run_example("simple_parallel.yaml").await;
}
