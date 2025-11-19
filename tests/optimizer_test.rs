use skript::dsl::builder::WorkflowBuilder;
use skript::compiler::core::Compiler;
use skript::runtime::engine::Engine;
use skript::actions::builtin::{LogAction, AssignAction};
use serde_json::json;
use std::sync::Arc;
use std::time::Instant;
use uuid::Uuid;

#[test]
fn test_optimizer_fusion() {
    // Create a workflow with a chain of Sync nodes that should be fused.
    // Start -> Assign1 -> Assign2 -> Log1 -> End
    
    // 1. Build Workflow
    let workflow = WorkflowBuilder::new("fusion-test")
        .start("start")
        
        // Node 1: Sync (Assign)
        .function("step1", "assign")
            .param("expression", "a = 1")
            .build()
            
        // Node 2: Sync (Assign)
        .function("step2", "assign")
            .param("expression", "b = 2")
            .build()
            
        // Node 3: Sync (Log)
        .function("step3", "log")
            .param("msg", "Finished")
            .build()
            
        .end("end", "")
        
        // Connections (Linear Chain)
        .connect("start", "step1")
        .connect("step1", "step2")
        .connect("step2", "step3")
        .connect("step3", "end")
        .build();

    // 2. Compile
    let mut compiler = Compiler::new();
    let blueprint = compiler.compile(workflow.clone()).expect("Compilation failed");

    // 3. Verification
    // Original nodes: Start, Assign, Assign, Log, End (5 nodes)
    // Expected: Start, Fused(Assign, Assign, Log), End (3 nodes)
    // Note: "start" usually points to next. 
    // Let's see how many nodes are in blueprint.
    
    println!("Compiled Blueprint Nodes:");
    for (i, node) in blueprint.nodes.iter().enumerate() {
        println!("[{}] Kind: {}", i, node.kind);
    }

    // Check for fusion
    let fused_nodes_count = blueprint.nodes.iter()
        .filter(|n| n.kind == "fused")
        .count();
        
    assert!(fused_nodes_count > 0, "Optimizer should have produced at least one fused node");
    
    // Ensure chain is reduced.
    // Start(0) -> Fused(1) -> End(2)
    // Note: Start node in 'transform_node' is generated. 
    // The logic handles Start -> next.
    
    // If Start is not Sync, it won't be fused into the chain.
    // Assign, Assign, Log are Sync.
    // So we expect: Start, FusedNode, End.
    assert!(blueprint.nodes.len() < 5, "Node count should be reduced by fusion");
}

#[tokio::test]
async fn test_fusion_runtime_execution() {
    // Integration test: Run the fused workflow and ensure it produces correct results.
    
    let workflow = WorkflowBuilder::new("fusion-exec-test")
        .start("start")
        
        .function("init_a", "assign")
            .param("expression", "a = 10")
            .build()
            
        .function("calc_b", "assign")
            .param("expression", "b = a * 2")
            .build()
            
        .function("calc_c", "assign")
            .param("expression", "c = b + 5")
            .build()
            
        .end("end", "")
        
        .connect("start", "init_a")
        .connect("init_a", "calc_b")
        .connect("calc_b", "calc_c")
        .connect("calc_c", "end")
        .build();
        
    let mut compiler = Compiler::new();
    let blueprint = compiler.compile(workflow).unwrap();
    
    let engine = Engine::new();
    // Register handlers
    // Note: The Engine needs to know how to execute "fused" nodes.
    // We need to register the FusedNode definition in the Engine?
    // Wait, FusedNode is a runtime node type, but Engine needs a NodeDefinition for it?
    // Yes, Engine.register_node needs to be called for "fused".
    // BUT, FusedNode is special. It contains other nodes.
    // We haven't implemented FusedNodeDefinition yet!
    
    // TODO: We need to implement FusedNodeDefinition and register it.
    // Let's finish this test code first, then fix the missing piece.
}
