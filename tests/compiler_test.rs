use skript::compiler::core::Compiler;
use skript::dsl::builder::WorkflowBuilder;

#[test]
fn test_compile_linear_workflow() {
    // 1. Build DSL
    let workflow = WorkflowBuilder::new("linear-compile-test")
        .start("start")
        .function("step1", "log")
            .param("msg", "hello")
            .build()
        .end("end")
        .connect("start", "step1")
        .connect("step1", "end")
        .build();

    // 2. Compile
    let mut compiler = Compiler::new();
    let blueprint = compiler.compile(workflow).expect("Compilation failed");

    // 3. Assert Blueprint Structure
    assert_eq!(blueprint.id, "linear-compile-test");
    assert_eq!(blueprint.nodes.len(), 3);
    
    // Verify Start Node
    let start_node = &blueprint.nodes[blueprint.start_index];
    assert_eq!(start_node.kind, "start");
    // The next node should be index 1
    assert_eq!(start_node.params.get("next").unwrap().as_u64(), Some(1));

    // Verify Action Node (Index 1)
    let action_node = &blueprint.nodes[1];
    assert_eq!(action_node.kind, "log");
    assert_eq!(action_node.params.get("next").unwrap().as_u64(), Some(2)); // Points to End
    assert_eq!(action_node.params.get("msg").unwrap().as_str(), Some("hello"));

     // Verify End Node (Index 2)
    let end_node = &blueprint.nodes[2];
    assert_eq!(end_node.kind, "end");
}