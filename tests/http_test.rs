use skript::dsl::builder::WorkflowBuilder;
use skript::compiler::core::Compiler;
use skript::runtime::engine::Engine;
use skript::actions::http::HttpAction;
use skript::nodes::common::{StartDefinition, EndDefinition};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

#[tokio::test]
#[ignore]
async fn test_http_action() {
    // Start -> Http(Get httpbin) -> End
    let workflow = WorkflowBuilder::new("http-test")
        .start("start")
        .action("req", "http")
            .param("url", "https://httpbin.org/get")
            .param("method", "GET")
            .output("resp")
            .build()
        .end("end")
        .connect("start", "req")
        .connect("req", "end")
        .build();

    let mut compiler = Compiler::new();
    let blueprint = compiler.compile(workflow).expect("Compilation failed");

    let mut engine = Engine::new();
    engine.register_node(Box::new(StartDefinition));
    engine.register_node(Box::new(EndDefinition));
    engine.register_action(Arc::new(HttpAction::new()));
    
    engine.register_blueprint(blueprint);

    let instance_id = engine.start_workflow("http-test", HashMap::new())
        .await
        .expect("Failed to start workflow");

    // Wait for network
    tokio::select! {
        _ = engine.run_worker() => {}
        _ = tokio::time::sleep(Duration::from_secs(5)) => {}
    }

    let resp = engine.get_instance_var(instance_id, "resp");
    assert!(resp.is_some(), "Should have response");
    
    let val = resp.unwrap();
    assert_eq!(val["status"], 200);
    // httpbin returns { "url": "..." } in data
    assert!(val["data"]["url"].as_str().unwrap().contains("httpbin.org"));
}
