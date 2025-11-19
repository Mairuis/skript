use skript::dsl::builder::WorkflowBuilder;
use skript::dsl::NodeType;
use serde_json::json;

#[test]
fn test_build_linear_workflow() {
    let workflow = WorkflowBuilder::new("linear-flow")
        .var("env", "prod")
        .start("start")
        .function("http_get", "http_request")
            .param("url", "https://api.example.com")
            .param("method", "GET")
            .output("response")
            .build()
        .end("end", "")
        .connect("start", "http_get")
        .connect("http_get", "end")
        .build();

    assert_eq!(workflow.id, "linear-flow");
    assert_eq!(workflow.variables.get("env"), Some(&json!("prod")));
    assert_eq!(workflow.nodes.len(), 3);
    assert_eq!(workflow.edges.len(), 2);

    // 检查 Action 节点
    if let Some(node) = workflow.nodes.iter().find(|n| n.id == "http_get") {
        if let NodeType::Function { name, params, output } = &node.kind {
            assert_eq!(name, "http_request");
            assert_eq!(params.get("url"), Some(&json!("https://api.example.com")));
            assert_eq!(*output, Some("response".to_string())); // 修正: Dereference output
        } else {
            panic!("Node type mismatch");
        }
    } else {
        panic!("Node not found");
    }
}

#[test]
fn test_build_branching_workflow() {
    let workflow = WorkflowBuilder::new("branch-flow")
        .start("start")
        .if_node("check_condition")
        .function("branch_a", "log")
            .param("msg", "A")
            .build()
        .function("branch_b", "log")
            .param("msg", "B")
            .build()
        .end("end", "")
        // 连接
        .connect("start", "check_condition")
        .connect_if("check_condition", "branch_a", "${x} > 10")
        .connect_else("check_condition", "branch_b")
        .connect("branch_a", "end")
        .connect("branch_b", "end")
        .build();

    assert_eq!(workflow.edges.len(), 5);
    
    // 检查条件边
    let if_edge = workflow.edges.iter()
        .find(|e| e.source == "check_condition" && e.target == "branch_a")
        .unwrap();
    assert_eq!(if_edge.condition, Some("${x} > 10".to_string()));

    // 检查 Else 边
    let else_edge = workflow.edges.iter()
        .find(|e| e.source == "check_condition" && e.target == "branch_b")
        .unwrap();
    assert_eq!(else_edge.branch_type, Some("else".to_string()));
}
