use skript::compiler::expander::Expander;
use skript::dsl::builder::WorkflowBuilder;
use skript::dsl::NodeType;
use skript::dsl::Node;

#[test]
fn test_expand_parallel_node() {
    // 1. Build Workflow with Parallel Node
    // Start -> Parallel(p1) [Branch1: A, Branch2: B->C] -> End
    let branch1 = vec![
        Node { id: "A".to_string(), kind: NodeType::Function { name: "log".to_string(), params: Default::default(), output: None } }
    ];
    
    let branch2 = vec![
        Node { id: "B".to_string(), kind: NodeType::Function { name: "log".to_string(), params: Default::default(), output: None } },
        Node { id: "C".to_string(), kind: NodeType::Function { name: "log".to_string(), params: Default::default(), output: None } }
    ];

    let workflow = WorkflowBuilder::new("parallel-expand-test")
        .start("start")
        .parallel("p1", vec![branch1, branch2])
        .end("end", "")
        .connect("start", "p1")
        .connect("p1", "end")
        .build();

    // 2. Expand
    let expander = Expander::new();
    let expanded_workflow = expander.expand(workflow).expect("Expansion failed");

    // 3. Assertions
    
    // Check Nodes Count: 
    // Original: Start, Parallel, End (3)
    // New: Start, End, A, B, C, p1_fork, p1_join (7)
    assert_eq!(expanded_workflow.nodes.len(), 7);

    // Check p1_fork existence
    let fork_node = expanded_workflow.nodes.iter().find(|n| n.id == "p1_fork").expect("Fork node not found");
    if let NodeType::Fork { branch_start_ids, join_id } = &fork_node.kind {
        assert_eq!(join_id, "p1_join");
        assert_eq!(branch_start_ids.len(), 2);
        assert!(branch_start_ids.contains(&"A".to_string()));
        assert!(branch_start_ids.contains(&"B".to_string()));
    } else {
        panic!("p1_fork is not a Fork node");
    }

    // Check Edges
    // 1. Start -> p1_fork (Redirected)
    assert!(expanded_workflow.edges.iter().any(|e| e.source == "start" && e.target == "p1_fork"));
    
    // 2. p1_join -> End (Redirected)
    assert!(expanded_workflow.edges.iter().any(|e| e.source == "p1_join" && e.target == "end"));

    // 3. Branch 2 Internal: B -> C (Auto generated)
    assert!(expanded_workflow.edges.iter().any(|e| e.source == "B" && e.target == "C"));

    // 4. Branch Tails -> Join: A -> p1_join, C -> p1_join
    assert!(expanded_workflow.edges.iter().any(|e| e.source == "A" && e.target == "p1_join"));
    assert!(expanded_workflow.edges.iter().any(|e| e.source == "C" && e.target == "p1_join"));
}