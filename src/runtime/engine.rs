use std::sync::Arc;
use dashmap::DashMap;
use tokio::sync::mpsc;
use uuid::Uuid;
use anyhow::{Result, anyhow};
use crate::runtime::blueprint::{Blueprint, NodeIndex};
use crate::runtime::context::Context;
use crate::runtime::task::Task;
use crate::runtime::node::{Node, NodeDefinition};
use crate::runtime::syscall::Syscall;
use crate::actions::FunctionHandler;
use crate::nodes::function::FunctionNodeDefinition;
use std::collections::HashMap;
use serde_json::Value;

pub struct Engine {
    // Raw Blueprints (Config)
    blueprints: DashMap<String, Arc<Blueprint>>,
    // Instantiated Nodes (JIT Cache)
    executable_cache: DashMap<String, Arc<Vec<Box<dyn Node>>>>,
    
    instances: DashMap<Uuid, Arc<Context>>,
    
    // Registry for Node Factories
    node_registry: HashMap<String, Box<dyn NodeDefinition>>,
    
    task_sender: mpsc::Sender<Task>,
    task_receiver: Option<mpsc::Receiver<Task>>, 
}

struct EngineSyscall<'a> {
    task: Task,
    sender: &'a mpsc::Sender<Task>,
}

impl<'a> Syscall for EngineSyscall<'a> {
    fn jump(&mut self, target: NodeIndex) {
        let new_task = Task {
            instance_id: self.task.instance_id,
            token_id: self.task.token_id,
            node_index: target,
            flow_id: self.task.flow_id,
        };
        let _ = self.sender.try_send(new_task); 
    }

    fn fork(&mut self, targets: Vec<NodeIndex>) {
        for target in targets {
            let new_task = Task {
                instance_id: self.task.instance_id,
                token_id: Uuid::new_v4(),
                node_index: target,
                flow_id: self.task.flow_id,
            };
            let _ = self.sender.try_send(new_task);
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
        let (tx, rx) = mpsc::channel(100); 
        Self {
            blueprints: DashMap::new(),
            executable_cache: DashMap::new(),
            instances: DashMap::new(),
            node_registry: HashMap::new(),
            task_sender: tx,
            task_receiver: Some(rx),
        }
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
        let context_vars = DashMap::new();
        for (k, v) in initial_vars {
            context_vars.insert(k, v);
        }
        
        let context = Arc::new(Context::new(instance_id, blueprint_id.to_string(), context_vars));
        self.instances.insert(instance_id, context);

        let task = Task {
            instance_id,
            token_id: Uuid::new_v4(),
            node_index: blueprint_meta.start_index,
            flow_id: Uuid::new_v4(),
        };

        self.task_sender.send(task).await
            .map_err(|e| anyhow!("Failed to send initial task: {}", e))?;

        Ok(instance_id)
    }

    pub async fn run_worker(&mut self) {
        let mut rx = self.task_receiver.take().expect("Worker already started");
        println!("Worker started.");

        while let Some(task) = rx.recv().await {
            let instance = if let Some(i) = self.instances.get(&task.instance_id) {
                i.clone()
            } else {
                continue;
            };

            let nodes = if let Some(n) = self.executable_cache.get(&instance.workflow_id) {
                n.clone()
            } else {
                if let Ok(n) = self.prepare_blueprint(&instance.workflow_id) {
                    n
                } else {
                    eprintln!("Failed to prepare blueprint for instance");
                    continue;
                }
            };

            let node = &nodes[task.node_index];
            
            let mut syscall = EngineSyscall {
                task: task.clone(),
                sender: &self.task_sender,
            };

            if let Err(e) = node.execute(&instance, &task, &mut syscall).await {
                eprintln!("Task failed: {:?}", e);
            }
        }
    }

    pub fn get_instance_var(&self, instance_id: Uuid, key: &str) -> Option<Value> {
        self.instances.get(&instance_id).and_then(|ctx| ctx.get_var(key))
    }
}
