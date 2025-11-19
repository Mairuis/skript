use std::sync::Arc;
use dashmap::DashMap;
use uuid::Uuid;
use anyhow::{Result, anyhow};
use crate::runtime::blueprint::{Blueprint, NodeIndex};
use crate::runtime::context::Context;
use crate::runtime::task::Task;
use crate::runtime::node::{Node, NodeDefinition};
use crate::runtime::syscall::Syscall;
use crate::runtime::storage::{StateStore, TaskQueue, InMemoryStateStore, InMemoryTaskQueue};
use crate::actions::FunctionHandler;
use crate::nodes::function::FunctionNodeDefinition;
use std::collections::HashMap;
use serde_json::Value;

pub struct Engine {
    // Raw Blueprints (Config)
    blueprints: DashMap<String, Arc<Blueprint>>,
    // Instantiated Nodes (JIT Cache)
    executable_cache: DashMap<String, Arc<Vec<Box<dyn Node>>>>,
    
    // Storage Abstractions
    store: Arc<dyn StateStore>,
    task_queue: Arc<dyn TaskQueue>,
    
    // Registry for Node Factories
    node_registry: HashMap<String, Box<dyn NodeDefinition>>,
}

use tokio::time::timeout;
use std::time::Duration;
use tracing::{info, error, warn};

struct EngineSyscall {
    task: Task,
    pending_tasks: Vec<Task>,
}

impl Syscall for EngineSyscall {
    fn jump(&mut self, target: NodeIndex) {
        let new_task = Task {
            instance_id: self.task.instance_id,
            workflow_id: self.task.workflow_id.clone(),
            token_id: self.task.token_id,
            node_index: target,
            flow_id: self.task.flow_id,
        };
        self.pending_tasks.push(new_task);
    }

    fn fork(&mut self, targets: Vec<NodeIndex>) {
        for target in targets {
            let new_task = Task {
                instance_id: self.task.instance_id,
                workflow_id: self.task.workflow_id.clone(),
                token_id: Uuid::new_v4(),
                node_index: target,
                flow_id: self.task.flow_id,
            };
            self.pending_tasks.push(new_task);
        }
    }

    fn wait(&mut self) {
        // Do nothing
    }

    fn terminate(&mut self) {
        // Do nothing
    }
}

impl Engine {
    pub fn new() -> Self {
        // Default to In-Memory implementation
        let store = Arc::new(InMemoryStateStore::new());
        let task_queue = Arc::new(InMemoryTaskQueue::new());
        Self::new_with_storage(store, task_queue)
    }

    pub fn new_with_storage(store: Arc<dyn StateStore>, task_queue: Arc<dyn TaskQueue>) -> Self {
        let mut engine = Self {
            blueprints: DashMap::new(),
            executable_cache: DashMap::new(),
            store,
            task_queue,
            node_registry: HashMap::new(),
        };
        
        // Register internal FusedNode handler
        engine.register_node(Box::new(crate::nodes::fused::FusedNodeDefinition));
        
        engine
    }

    pub fn register_blueprint(&self, blueprint: Blueprint) {
        let id = blueprint.id.clone();
        self.blueprints.insert(id.clone(), Arc::new(blueprint));
        self.executable_cache.remove(&id);
    }

    pub fn register_node(&mut self, definition: Box<dyn NodeDefinition>) {
        self.node_registry.insert(definition.name().to_string(), definition);
    }

    pub fn register_function(&mut self, handler: Arc<dyn FunctionHandler>) {
        let def = FunctionNodeDefinition { handler };
        self.register_node(Box::new(def));
    }

    fn prepare_blueprint(&self, blueprint_id: &str) -> Result<Arc<Vec<Box<dyn Node>>>> {
        if let Some(nodes) = self.executable_cache.get(blueprint_id) {
            return Ok(nodes.clone());
        }

        let blueprint = self.blueprints.get(blueprint_id)
            .ok_or_else(|| anyhow!("Blueprint not found: {}", blueprint_id))?;

        let mut nodes = Vec::with_capacity(blueprint.nodes.len());
        for bp_node in &blueprint.nodes {
            let def = self.node_registry.get(&bp_node.kind)
                .ok_or_else(|| anyhow!("Node definition not found: {}", bp_node.kind))?;
            
            let node_instance = def.prepare(bp_node.params.clone())?;
            nodes.push(node_instance);
        }

        let arc_nodes = Arc::new(nodes);
        self.executable_cache.insert(blueprint_id.to_string(), arc_nodes.clone());
        Ok(arc_nodes)
    }

    pub async fn start_workflow(&self, blueprint_id: &str, initial_vars: HashMap<String, Value>) -> Result<Uuid> {
        let _ = self.prepare_blueprint(blueprint_id)?;
        let blueprint_meta = self.blueprints.get(blueprint_id).unwrap(); 

        let instance_id = Uuid::new_v4();
        
        // 1. Initialize State
        self.store.init_instance(instance_id, initial_vars).await?;

        // 2. Push Initial Task
        let task = Task {
            instance_id,
            workflow_id: blueprint_id.to_string(),
            token_id: Uuid::new_v4(),
            node_index: blueprint_meta.start_index,
            flow_id: Uuid::new_v4(),
        };

        self.task_queue.push(task).await
            .map_err(|e| anyhow!("Failed to send initial task: {}", e))?;

        Ok(instance_id)
    }

    pub async fn run_worker(&self) {
        info!("Worker started.");

        loop {
            match self.task_queue.pop().await {
                Ok(Some(task)) => {
                    let workflow_id = &task.workflow_id;
                    
                    // Create Ephemeral Context
                    let context = Context::new(
                        task.instance_id,
                        workflow_id.clone(),
                        self.store.clone()
                    );

                    let nodes = if let Some(n) = self.executable_cache.get(workflow_id) {
                        n.clone()
                    } else {
                        if let Ok(n) = self.prepare_blueprint(workflow_id) {
                            n
                        } else {
                            error!(workflow_id = %workflow_id, "Failed to prepare blueprint");
                            continue;
                        }
                    };

                    if task.node_index >= nodes.len() {
                        error!(node_index = task.node_index, "Node index out of bounds");
                        continue;
                    }

                    let node = &nodes[task.node_index];
                    
                    let mut syscall = EngineSyscall {
                        task: task.clone(),
                        pending_tasks: Vec::new(),
                    };

                    // Global timeout configuration (hardcoded for now)
                    let timeout_duration = Duration::from_secs(60);

                    match timeout(timeout_duration, node.execute(&context, &task, &mut syscall)).await {
                        Ok(Ok(())) => {
                            // Flush pending tasks
                            for new_task in syscall.pending_tasks {
                                if let Err(e) = self.task_queue.push(new_task).await {
                                    error!("Failed to schedule task: {}", e);
                                }
                            }
                        }
                        Ok(Err(e)) => {
                            error!(instance_id = %task.instance_id, node_index = task.node_index, error = ?e, "Task failed");
                        }
                        Err(_) => {
                            error!(instance_id = %task.instance_id, node_index = task.node_index, "Task timed out after {:?}", timeout_duration);
                        }
                    }
                }
                Ok(None) => {
                    // Queue closed or empty? If empty and using mpsc, it waits. 
                    // If pop() returns None it implies channel closed.
                    warn!("Task queue returned None (closed?), worker stopping.");
                    break;
                }
                Err(e) => {
                    error!("Error popping from task queue: {}", e);
                    tokio::time::sleep(Duration::from_millis(100)).await;
                }
            }
        }
    }

    pub async fn get_instance_var(&self, instance_id: Uuid, key: &str) -> Option<Value> {
        match self.store.get_var(instance_id, key).await {
            Ok(v) => v,
            Err(e) => {
                error!("Failed to get instance var: {}", e);
                None
            }
        }
    }
}
