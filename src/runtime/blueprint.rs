use serde::{Serialize, Deserialize};
use serde_json::Value;

pub type NodeIndex = usize;

/// 编译后的蓝图 (中间表示，可序列化)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Blueprint {
    pub id: String,
    pub name: String,
    pub nodes: Vec<BlueprintNode>,
    pub start_index: NodeIndex,
}

/// 蓝图节点配置
/// 这是一个通用的数据容器，用于在该节点被加载时传递给 NodeDefinition::prepare
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlueprintNode {
    /// 节点类型名称 (e.g. "log", "if", "fork")
    pub kind: String, 
    /// 配置参数 (包含编译器计算出的跳转目标索引，如 "next": 1)
    pub params: Value,
}
