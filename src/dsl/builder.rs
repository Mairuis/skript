use crate::dsl::{Workflow, Node, Edge, NodeType, Branch};
use std::collections::HashMap;
use serde_json::Value;

pub struct WorkflowBuilder {
    id: String,
    name: String,
    variables: HashMap<String, Value>,
    pub nodes: Vec<Node>, // Made public for manual manipulation in tests if needed
    edges: Vec<Edge>,
}

impl WorkflowBuilder {
    pub fn new(id: &str) -> Self {
        Self {
            id: id.to_string(),
            name: id.to_string(),
            variables: HashMap::new(),
            nodes: Vec::new(),
            edges: Vec::new(),
        }
    }

    pub fn name(mut self, name: &str) -> Self {
        self.name = name.to_string();
        self
    }

    pub fn var(mut self, key: &str, value: impl Into<Value>) -> Self {
        self.variables.insert(key.to_string(), value.into());
        self
    }

    pub fn start(mut self, id: &str) -> Self {
        self.nodes.push(Node {
            id: id.to_string(),
            kind: NodeType::Start,
        });
        self
    }

    pub fn end(mut self, id: &str) -> Self {
        self.nodes.push(Node {
            id: id.to_string(),
            kind: NodeType::End,
        });
        self
    }

    pub fn function(self, id: &str, function_name: &str) -> FunctionBuilder {
        FunctionBuilder {
            workflow_builder: self,
            id: id.to_string(),
            function_name: function_name.to_string(),
            params: HashMap::new(),
            output: None,
        }
    }

    pub fn if_node(mut self, id: &str) -> Self {
        self.nodes.push(Node {
            id: id.to_string(),
            kind: NodeType::If { branches: Vec::new() },
        });
        self
    }

    /// 添加并行块
    pub fn parallel(mut self, id: &str, branches: Vec<Vec<Node>>) -> Self {
        let branches_structs = branches.into_iter()
            .map(|nodes| Branch { nodes })
            .collect();
            
        self.nodes.push(Node {
            id: id.to_string(),
            kind: NodeType::Parallel {
                branches: branches_structs,
            },
        });
        self
    }

    pub fn connect(mut self, source: &str, target: &str) -> Self {
        self.edges.push(Edge {
            source: source.to_string(),
            target: target.to_string(),
            condition: None,
            branch_type: None,
            branch_index: None,
        });
        self
    }

    pub fn connect_if(mut self, source: &str, target: &str, condition: &str) -> Self {
        self.edges.push(Edge {
            source: source.to_string(),
            target: target.to_string(),
            condition: Some(condition.to_string()),
            branch_type: None,
            branch_index: None,
        });
        self
    }

    pub fn connect_else(mut self, source: &str, target: &str) -> Self {
        self.edges.push(Edge {
            source: source.to_string(),
            target: target.to_string(),
            condition: None,
            branch_type: Some("else".to_string()),
            branch_index: None,
        });
        self
    }

    pub fn build(self) -> Workflow {
        Workflow {
            id: self.id,
            name: self.name,
            variables: self.variables,
            nodes: self.nodes,
            edges: self.edges,
        }
    }
}

pub struct FunctionBuilder {
    workflow_builder: WorkflowBuilder,
    id: String,
    function_name: String,
    params: HashMap<String, Value>,
    output: Option<String>,
}

impl FunctionBuilder {
    pub fn param(mut self, key: &str, value: impl Into<Value>) -> Self {
        self.params.insert(key.to_string(), value.into());
        self
    }

    pub fn output(mut self, var_name: &str) -> Self {
        self.output = Some(var_name.to_string());
        self
    }

    pub fn build(mut self) -> WorkflowBuilder {
        self.workflow_builder.nodes.push(Node {
            id: self.id,
            kind: NodeType::Function {
                name: self.function_name,
                params: self.params,
                output: self.output,
            },
        });
        self.workflow_builder
    }
}
