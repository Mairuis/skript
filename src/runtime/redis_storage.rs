use async_trait::async_trait;
use serde_json::Value;
use uuid::Uuid;
use crate::runtime::task::Task;
use crate::runtime::storage::{StateStore, TaskQueue};
use anyhow::Result;
use redis::AsyncCommands;
use std::collections::HashMap;

pub struct RedisTaskQueue {
    client: redis::Client,
    queue_key: String,
}

impl RedisTaskQueue {
    pub fn new(client: redis::Client, queue_key: String) -> Self {
        Self {
            client,
            queue_key,
        }
    }
}

#[async_trait]
impl TaskQueue for RedisTaskQueue {
    async fn push(&self, task: Task) -> Result<()> {
        let mut conn = self.client.get_multiplexed_async_connection().await?;
        let serialized = serde_json::to_string(&task)?;
        let _: () = conn.lpush(&self.queue_key, serialized).await?;
        Ok(())
    }

    async fn pop(&self) -> Result<Option<Task>> {
        let mut conn = self.client.get_multiplexed_async_connection().await?;
        // BRPOP returns (key, value). We use timeout 0 for infinite block?
        // Or maybe better to use a reasonable timeout to allow shutdown/checking?
        // Let's use 1 second timeout for now to stay responsive.
        let result: Option<(String, String)> = conn.brpop(&self.queue_key, 1.0).await?;
        
        if let Some((_, task_json)) = result {
             let task = serde_json::from_str(&task_json)?;
             Ok(Some(task))
        } else {
             Ok(None)
        }
    }
}

pub struct RedisStateStore {
    client: redis::Client,
}

impl RedisStateStore {
    pub fn new(client: redis::Client) -> Self {
        Self { client }
    }

    fn var_key(&self, instance_id: Uuid) -> String {
        format!("skript:inst:{}:vars", instance_id)
    }
    
    fn join_key(&self, instance_id: Uuid) -> String {
        format!("skript:inst:{}:joins", instance_id)
    }
}

#[async_trait]
impl StateStore for RedisStateStore {
    async fn get_var(&self, instance_id: Uuid, key: &str) -> Result<Option<Value>> {
        let mut conn = self.client.get_multiplexed_async_connection().await?;
        let val_str: Option<String> = conn.hget(self.var_key(instance_id), key).await?;
        
        if let Some(s) = val_str {
            let val: Value = serde_json::from_str(&s)?;
            Ok(Some(val))
        } else {
            Ok(None)
        }
    }

    async fn set_var(&self, instance_id: Uuid, key: &str, value: Value) -> Result<()> {
        let mut conn = self.client.get_multiplexed_async_connection().await?;
        let val_str = serde_json::to_string(&value)?;
        let _: () = conn.hset(self.var_key(instance_id), key, val_str).await?;
        Ok(())
    }

    async fn init_instance(&self, instance_id: Uuid, initial_vars: HashMap<String, Value>) -> Result<()> {
        let mut conn = self.client.get_multiplexed_async_connection().await?;
        let key = self.var_key(instance_id);
        
        // Optim: Use pipeline or just loop? Pipeline is better but hset_multiple might work if we flatten.
        // HSET accepts multiple pairs.
        if !initial_vars.is_empty() {
            let mut items = Vec::new();
            for (k, v) in initial_vars {
                 let v_str = serde_json::to_string(&v)?;
                 items.push((k, v_str));
            }
            let _: () = conn.hset_multiple(key, &items).await?;
        }
        Ok(())
    }
    
    async fn get_all_vars(&self, instance_id: Uuid) -> Result<HashMap<String, Value>> {
        let mut conn = self.client.get_multiplexed_async_connection().await?;
        let raw_map: HashMap<String, String> = conn.hgetall(self.var_key(instance_id)).await?;
        
        let mut result = HashMap::new();
        for (k, v_str) in raw_map {
            if let Ok(v) = serde_json::from_str(&v_str) {
                result.insert(k, v);
            }
        }
        Ok(result)
    }

    async fn decrement_join_count(&self, instance_id: Uuid, node_index: usize, initial_count: usize) -> Result<usize> {
        // LUA SCRIPT for atomicity
        // ARGV[1] = initial_count
        // KEYS[1] = join_key (Hash)
        // ARGV[2] = node_index (Field)
        
        let script = redis::Script::new(r#"
            local key = KEYS[1]
            local field = ARGV[2]
            local init = tonumber(ARGV[1])
            
            -- Check if exists
            local current = redis.call("HGET", key, field)
            if current == false then
                -- Initialize if not exists. 
                -- Wait, if not exists, we assume it's the first thread arriving?
                -- But if we just HINCRBY -1, it starts from 0 -> -1?
                -- We need to initialize to 'initial_count' first?
                
                -- Logic:
                -- If not exists, set to initial_count - 1.
                -- If exists, decrement.
                
                local val = init - 1
                if val == 0 then
                    return 0 -- Already done? Should not happen if init >= 1
                else
                    redis.call("HSET", key, field, val)
                    return val
                end
            else
                local val = tonumber(current) - 1
                if val <= 0 then
                    redis.call("HDEL", key, field)
                    return 0
                else
                    redis.call("HSET", key, field, val)
                    return val
                end
            end
        "#);
        
        let mut conn = self.client.get_multiplexed_async_connection().await?;
        let key = self.join_key(instance_id);
        
        let new_val: usize = script
            .key(key)
            .arg(initial_count)
            .arg(node_index)
            .invoke_async(&mut conn)
            .await?;
            
        Ok(new_val)
    }
}
