pub mod builder;

use std::collections::HashMap;
use serde::{Serialize, Deserialize};
use serde_json::Value;

/// 原始 DSL 定义的 Workflow
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Workflow {
    pub id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub variables: HashMap<String, Value>,
    #[serde(default)]
    pub nodes: Vec<Node>,
    #[serde(default)]
    pub edges: Vec<Edge>,
}

/// DSL 中的节点类型
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type")]
pub enum NodeType {
    Start,
    End {
        #[serde(default)]
        output: String,
    },
    #[serde(alias = "Function")]
    Function {
        name: String,
        #[serde(default)]
        params: HashMap<String, Value>,
        output: Option<String>,
    },
    Assign {
        #[serde(default)]
        assignments: Vec<HashMap<String, Value>>,
        expression: Option<String>,
    },
    If {
        #[serde(default)]
        branches: Vec<HashMap<String, String>>, // [{condition: "..."}]
    },
    Parallel {
        branches: Vec<Branch>, // 嵌套子图
    },
    Iteration {
        collection: String,
        item_var: String,
    },
    Loop {
        condition: String,
    },
    
    // --- 内部 IR 节点 (由 Expander 生成，不应在 YAML 中直接使用) ---
    Fork {
        branch_start_ids: Vec<String>,
        join_id: String,
    },
    Join {
        expect_count: usize,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Branch {
    pub nodes: Vec<Node>,
}

/// DSL 中的节点
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Node {
    pub id: String,
    #[serde(flatten)]
    pub kind: NodeType,
}

/// DSL 中的边
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Edge {
    pub source: String,
    pub target: String,
    pub condition: Option<String>,
    pub branch_type: Option<String>, // "else", "body" 等
    pub branch_index: Option<usize>,
}