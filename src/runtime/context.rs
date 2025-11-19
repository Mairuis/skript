use std::sync::Arc;
use serde_json::Value;
use uuid::Uuid;
use crate::runtime::storage::StateStore;
use anyhow::Result;

/// 运行时上下文 (Runtime Context)
/// 包含工作流实例的所有动态状态，现在委托给 StateStore
#[derive(Clone)] // Context should be cheap to clone (just Arcs)
pub struct Context {
    pub instance_id: Uuid,
    pub workflow_id: String,
    pub store: Arc<dyn StateStore>,
}

impl Context {
    pub fn new(instance_id: Uuid, workflow_id: String, store: Arc<dyn StateStore>) -> Self {
        Self {
            instance_id,
            workflow_id,
            store,
        }
    }

    pub async fn get_var(&self, key: &str) -> Option<Value> {
        match self.store.get_var(self.instance_id, key).await {
            Ok(v) => v,
            Err(e) => {
                // In a real production system we might want to log this error
                eprintln!("Error getting var {}: {}", key, e);
                None
            }
        }
    }

    pub async fn set_var(&self, key: &str, value: Value) {
        if let Err(e) = self.store.set_var(self.instance_id, key, value).await {
             eprintln!("Error setting var {}: {}", key, e);
        }
    }
    
    pub async fn get_all_vars(&self) -> Result<std::collections::HashMap<String, Value>> {
        self.store.get_all_vars(self.instance_id).await
    }
    
    pub async fn decrement_join_count(&self, node_index: usize, initial_count: usize) -> Result<usize> {
        self.store.decrement_join_count(self.instance_id, node_index, initial_count).await
    }
}
