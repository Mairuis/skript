use skript::compiler::loader;
use skript::dsl::builder::WorkflowBuilder;
use std::fs;

#[test]
fn test_load_simple_yaml_workflow() {
    let yaml_content = r#"
id: "test-yaml-flow"
name: "YAML Test Workflow"
variables:
  env: "dev"
nodes:
  - id: "start"
    type: "Start"
  - id: "action_node"
    type: "Function"
    name: "log"
    params:
      msg: "Hello from YAML"
    output: "log_result"
  - id: "end"
    type: "End"
edges:
  - source: "start"
    target: "action_node"
  - source: "action_node"
    target: "end"
"#;

    let temp_dir = tempfile::tempdir().expect("Failed to create temp dir");
    let file_path = temp_dir.path().join("test_workflow.yaml");
    fs::write(&file_path, yaml_content).expect("Failed to write temp file");

    let loaded_workflow = loader::load_workflow_from_yaml(&file_path.to_string_lossy())
        .expect("Failed to load workflow from YAML");

    let expected_workflow = WorkflowBuilder::new("test-yaml-flow")
        .name("YAML Test Workflow")
        .var("env", "dev")
        .start("start")
        .function("action_node", "log")
            .param("msg", "Hello from YAML")
            .output("log_result")
            .build()
        .end("end", "")
        .connect("start", "action_node")
        .connect("action_node", "end")
        .build();

    assert_eq!(loaded_workflow, expected_workflow);

    // Cleanup
    temp_dir.close().expect("Failed to close temp dir");
}