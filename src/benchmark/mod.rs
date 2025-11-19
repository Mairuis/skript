pub mod actions;

use crate::runtime::engine::Engine;
use crate::runtime::context::Context;
use crate::nodes::common::{StartDefinition, EndDefinition};
use crate::nodes::flow::{ForkDefinition, JoinDefinition};
use crate::actions::builtin::AssignAction;
use crate::compiler::core::Compiler;
use crate::dsl::{Workflow, Node, NodeType, Edge, Branch};
use crate::benchmark::actions::{FibonacciAction, SleepAction};
use std::sync::Arc;
use std::collections::HashMap;
use std::time::{Duration, Instant};
use serde_json::json;
use tracing::{info, warn};
use anyhow::Result;

pub struct BenchmarkRunner {
    engine: Arc<Engine>,
}

impl BenchmarkRunner {
    pub fn new() -> Self {
        let mut engine = Engine::new();
        engine.register_node(Box::new(StartDefinition));
        engine.register_node(Box::new(EndDefinition));
        engine.register_node(Box::new(ForkDefinition));
        engine.register_node(Box::new(JoinDefinition));
        engine.register_function(Arc::new(AssignAction));
        engine.register_function(Arc::new(FibonacciAction));
        engine.register_function(Arc::new(SleepAction));
        
        Self {
            engine: Arc::new(engine),
        }
    }

    async fn run_once(&self, branch_count: usize, fib_n: u64) -> Result<(Duration, f64)> {
        // 1. Build Workflow
        let mut branches = Vec::with_capacity(branch_count);
        for i in 0..branch_count {
            branches.push(Branch {
                nodes: vec![
                    Node {
                        id: format!("task_{}", i),
                        kind: NodeType::Function { 
                            name: "fib".to_string(), 
                            params: HashMap::from([
                                ("n".to_string(), json!(fib_n))
                            ]), 
                            output: None 
                        }
                    }
                ]
            });
        }

        let workflow_id = format!("bench_{}_{}", branch_count, fib_n);
        let workflow = Workflow {
            id: workflow_id.clone(),
            name: "Benchmark".to_string(),
            variables: HashMap::new(),
            nodes: vec![
                Node { id: "start".to_string(), kind: NodeType::Start },
                Node { 
                    id: "par".to_string(), 
                    kind: NodeType::Parallel { branches } 
                },
                Node {
                     id: "set_done".to_string(),
                     kind: NodeType::Assign {
                         assignments: vec![
                             HashMap::from([
                                 ("key".to_string(), json!("finished")),
                                 ("value".to_string(), json!(true))
                             ])
                         ],
                         expression: None
                     }
                },
                Node { id: "end".to_string(), kind: NodeType::End { output: "finished".to_string() } }
            ],
            edges: vec![
                Edge { source: "start".to_string(), target: "par".to_string(), condition: None, branch_type: None, branch_index: None },
                Edge { source: "par".to_string(), target: "set_done".to_string(), condition: None, branch_type: None, branch_index: None },
                Edge { source: "set_done".to_string(), target: "end".to_string(), condition: None, branch_type: None, branch_index: None },
            ]
        };

        // 2. Compile
        let mut compiler = Compiler::new();
        let blueprint = compiler.compile(workflow)?;
        self.engine.register_blueprint(blueprint.clone());

        // 3. Run
        let instance_id = self.engine.start_workflow(&blueprint.id, HashMap::new()).await?;
        
        let start = Instant::now();
        
        // Poll for completion
        loop {
            tokio::time::sleep(Duration::from_micros(100)).await;
            if let Some(val) = self.engine.get_instance_var(instance_id, "finished").await {
                 if val == json!(true) {
                     break;
                 }
            }
            if start.elapsed().as_secs() > 60 {
                anyhow::bail!("Benchmark timeout");
            }
        }

        let duration = start.elapsed();
        let tps = branch_count as f64 / duration.as_secs_f64();
        
        Ok((duration, tps))
    }

    pub async fn auto_tune(&self) -> Result<()> {
        let cpu_count = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(1);
        let worker_count = cpu_count * 2;
        
        println!("==================================================================");
        println!("🚀 SKRIPT AUTO-TUNING BENCHMARK");
        println!("==================================================================");
        println!("CPU Cores: {}", cpu_count);
        println!("Workers:   {}", worker_count);
        println!("Mode:      Fibonacci(20) [CPU Bound + Scheduling Stress]");
        println!("------------------------------------------------------------------");

        // Start workers
        let mut handles = Vec::new();
        for _ in 0..worker_count {
            let e = self.engine.clone();
            handles.push(tokio::spawn(async move {
                e.run_worker().await;
            }));
        }

        let mut current_branches = 100;
        let fib_n = 20;
        let mut last_tps = 0.0;
        let mut max_tps = 0.0;
        let mut max_config = 0;

        loop {
            print!("Testing with {:5} concurrent tasks... ", current_branches);
            match self.run_once(current_branches, fib_n).await {
                Ok((duration, tps)) => {
                    println!("Done in {:>6.3}s | TPS: {:>8.2}", duration.as_secs_f64(), tps);
                    
                    if tps > max_tps {
                        max_tps = tps;
                        max_config = current_branches;
                    }

                    // Ramp up strategy
                    if tps > last_tps * 0.95 { // Still growing or plateaued
                        last_tps = tps;
                        // Aggressive increase
                        current_branches = (current_branches as f64 * 1.5) as usize;
                    } else {
                        // Performance degraded significanty
                        println!("⚠️  Performance degraded at {} tasks. Stopping ramp-up.", current_branches);
                        break;
                    }

                    // Cap
                    if current_branches > 100_000 {
                        println!("🛑 Reached safety cap (100k tasks).");
                        break;
                    }
                }
                Err(e) => {
                    println!("FAILED: {}", e);
                    break;
                }
            }
        }

        println!("------------------------------------------------------------------");
        println!("🏆 PEAK PERFORMANCE");
        println!("------------------------------------------------------------------");
        println!("Max TPS:       {:.2} tasks/sec", max_tps);
        println!("Optimal Load:  {} concurrent tasks", max_config);
        println!("==================================================================");

        // Cleanup
        for h in handles {
            h.abort();
        }

        Ok(())
    }
}