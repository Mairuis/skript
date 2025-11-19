use std::sync::Arc;
use std::sync::atomic::AtomicUsize;
use dashmap::DashMap;
use serde_json::Value;
use uuid::Uuid;
use crate::runtime::blueprint::NodeIndex;

/// 运行时上下文 (Runtime Context)
/// 包含工作流实例的所有动态状态
#[derive(Debug)]
pub struct Context {
    pub instance_id: Uuid,
    pub workflow_id: String,
    /// 变量存储：支持并发读写
    pub variables: Arc<DashMap<String, Value>>,
    /// Join 节点计数器：NodeIndex -> 剩余等待数
    pub pending_joins: Arc<DashMap<NodeIndex, AtomicUsize>>,
}

impl Context {
    pub fn new(instance_id: Uuid, workflow_id: String, initial_vars: DashMap<String, Value>) -> Self {
        Self {
            instance_id,
            workflow_id,
            variables: Arc::new(initial_vars),
            pending_joins: Arc::new(DashMap::new()),
        }
    }

    pub fn get_var(&self, key: &str) -> Option<Value> {
        self.variables.get(key).map(|v| v.clone())
    }

    pub fn set_var(&self, key: &str, value: Value) {
        self.variables.insert(key.to_string(), value);
    }
}
