pub mod builder;

use std::collections::HashMap;
use serde::{Serialize, Deserialize};
use serde_json::Value;

/// 原始 DSL 定义的 Workflow
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Workflow {
    pub id: String,
    pub name: String,
    pub variables: HashMap<String, Value>,
    pub nodes: Vec<Node>,
    pub edges: Vec<Edge>,
}

/// DSL 中的节点类型
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type")]
pub enum NodeType {
    Start,
    End,
    Action {
        name: String,
        params: HashMap<String, Value>,
        output: Option<String>,
    },
    If, // 分支逻辑在 Edge 上
    Parallel {
        branches: Vec<Branch>, // 嵌套子图
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
    // Branch 不需要 edges，因为它是一条简单的链或者子图？
    // 让我们假设 Branch 内部是一个完整的子图片段，或者简单的节点列表（默认线性连接）
    // 现在的 DSL 设计 branches: - nodes: [...] 
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
}