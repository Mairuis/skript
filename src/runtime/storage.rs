use async_trait::async_trait;
use serde_json::Value;
use uuid::Uuid;
use crate::runtime::task::Task;
use anyhow::Result;
use dashmap::DashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use tokio::sync::mpsc;

// --- Interfaces ---

#[async_trait]
pub trait TaskQueue: Send + Sync {
    async fn push(&self, task: Task) -> Result<()>;
    async fn pop(&self) -> Result<Option<Task>>;
}

#[async_trait]
pub trait StateStore: Send + Sync {
    async fn get_var(&self, instance_id: Uuid, key: &str) -> Result<Option<Value>>;
    async fn set_var(&self, instance_id: Uuid, key: &str, value: Value) -> Result<()>;
    async fn init_instance(&self, instance_id: Uuid, initial_vars: std::collections::HashMap<String, Value>) -> Result<()>;
    /// Used for iterating all variables (e.g. for expression evaluation)
    /// Note: This might be expensive in remote implementations.
    async fn get_all_vars(&self, instance_id: Uuid) -> Result<std::collections::HashMap<String, Value>>;
    
    /// Atomically decrement a join counter.
    /// Returns the NEW value after decrement.
    async fn decrement_join_count(&self, instance_id: Uuid, node_index: usize, initial_count: usize) -> Result<usize>;
}

// --- In-Memory Implementations ---

pub struct InMemoryTaskQueue {
    sender: mpsc::Sender<Task>,
    receiver: tokio::sync::Mutex<mpsc::Receiver<Task>>,
}

impl InMemoryTaskQueue {
    pub fn new(capacity: usize) -> Self {
        let (tx, rx) = mpsc::channel(capacity);
        Self {
            sender: tx,
            receiver: tokio::sync::Mutex::new(rx),
        }
    }
}

#[async_trait]
impl TaskQueue for InMemoryTaskQueue {
    async fn push(&self, task: Task) -> Result<()> {
        self.sender.send(task).await.map_err(|e| anyhow::anyhow!("Task channel closed: {}", e))
    }

    async fn pop(&self) -> Result<Option<Task>> {
        let mut rx = self.receiver.lock().await;
        Ok(rx.recv().await)
    }
}

pub struct InMemoryStateStore {
    // Map<InstanceID, Map<VarKey, Value>>
    vars: DashMap<Uuid, DashMap<String, Value>>,
    // Map<InstanceID, Map<NodeIndex, AtomicCounter>>
    joins: DashMap<Uuid, DashMap<usize, Arc<AtomicUsize>>>,
}

impl InMemoryStateStore {
    pub fn new() -> Self {
        Self {
            vars: DashMap::new(),
            joins: DashMap::new(),
        }
    }
}

#[async_trait]
impl StateStore for InMemoryStateStore {
    async fn get_var(&self, instance_id: Uuid, key: &str) -> Result<Option<Value>> {
        if let Some(inst_vars) = self.vars.get(&instance_id) {
            Ok(inst_vars.get(key).map(|v| v.value().clone()))
        } else {
            Ok(None)
        }
    }

    async fn set_var(&self, instance_id: Uuid, key: &str, value: Value) -> Result<()> {
        // Ensure instance entry exists
        let inst_vars = self.vars.entry(instance_id).or_insert_with(DashMap::new);
        inst_vars.insert(key.to_string(), value);
        Ok(())
    }

    async fn init_instance(&self, instance_id: Uuid, initial_vars: std::collections::HashMap<String, Value>) -> Result<()> {
        let instance_vars = DashMap::new();
        for (k, v) in initial_vars {
            instance_vars.insert(k, v);
        }
        self.vars.insert(instance_id, instance_vars);
        Ok(())
    }
    
    async fn get_all_vars(&self, instance_id: Uuid) -> Result<std::collections::HashMap<String, Value>> {
        if let Some(inst_vars) = self.vars.get(&instance_id) {
            let mut map = std::collections::HashMap::new();
            for item in inst_vars.iter() {
                map.insert(item.key().clone(), item.value().clone());
            }
            Ok(map)
        } else {
            Ok(std::collections::HashMap::new())
        }
    }

    async fn decrement_join_count(&self, instance_id: Uuid, node_index: usize, initial_count: usize) -> Result<usize> {
        let inst_joins = self.joins.entry(instance_id).or_insert_with(DashMap::new);
        
        // 1. Get the Arc and release the map lock immediately by cloning
        let counter_arc = inst_joins.entry(node_index)
            .or_insert_with(|| Arc::new(AtomicUsize::new(initial_count)))
            .value()
            .clone();

        // 2. Operate on the AtomicUsize (no map lock held here)
        let prev = counter_arc.fetch_sub(1, Ordering::SeqCst);
        let new_val = prev - 1;
        
        // 3. If zero, cleanup (locks map again, which is now safe)
        if new_val == 0 {
             inst_joins.remove(&node_index);
        }
        
        Ok(new_val)
    }
}
