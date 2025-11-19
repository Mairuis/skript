pub mod actions;

use crate::runtime::engine::Engine;
use crate::runtime::context::Context;
use crate::nodes::common::{StartDefinition, EndDefinition};
use crate::nodes::flow::{ForkDefinition, JoinDefinition};
use crate::actions::builtin::AssignAction;
use crate::compiler::core::{Compiler, CompilerConfig};
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
    no_jit: bool,
}

impl BenchmarkRunner {
    pub fn new(no_jit: bool) -> Self {
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
            no_jit,
        }
    }

    async fn run_once(&self, branch_count: usize, _fib_n: u64) -> Result<(Duration, f64)> {
        // 1. Build Workflow
        let mut branches = Vec::with_capacity(branch_count);
        for i in 0..branch_count {
            let mut branch_nodes = Vec::new();
            let branch_prefix = format!("b{}_", i);

            // First node to initialize a variable
            branch_nodes.push(Node {
                id: format!("{}assign_0", branch_prefix),
                kind: NodeType::Function {
                    name: "assign".to_string(),
                    params: HashMap::from([
                        ("expression".to_string(), json!(format!("{}_temp_0 = 1", branch_prefix)))
                    ]),
                    output: None,
                },
            });

            // 9 more consecutive assign nodes
            for j in 1..10 {
                branch_nodes.push(Node {
                    id: format!("{}assign_{}", branch_prefix, j),
                    kind: NodeType::Function {
                        name: "assign".to_string(),
                        params: HashMap::from([
                            ("expression".to_string(), json!(format!("{}_temp_{} = {}_temp_{} + 1", branch_prefix, j, branch_prefix, j-1)))
                        ]),
                        output: None,
                    },
                });
            }

            // The last node in the chain will set the 'finished' variable
            branch_nodes.push(Node {
                id: format!("{}assign_final", branch_prefix),
                kind: NodeType::Function {
                    name: "assign".to_string(),
                    params: HashMap::from([
                        ("assignments".to_string(), json!([
                            {
                                "key": format!("finished_branch_{}", i),
                                "value": true
                            }
                        ]))
                    ]),
                    output: None,
                },
            });
            
            branches.push(Branch {
                nodes: branch_nodes
            });
        }

        let workflow_id = format!("bench_chain_{}", branch_count);
        let mut nodes_vec = vec![
            Node { id: "start".to_string(), kind: NodeType::Start },
            Node { 
                id: "par".to_string(), 
                kind: NodeType::Parallel { branches } 
            },
            // We need a final join node after the parallel section
            Node {
                 id: "final_join".to_string(),
                 kind: NodeType::Join { expect_count: branch_count }
            },
            Node { id: "end".to_string(), kind: NodeType::End { output: "overall_finished".to_string() } }
        ];

        // Add edges within the branches if not implicitly handled by Parallel expander
        // The expander will handle linear connections within branches.

        let mut edges_vec = vec![
            Edge { source: "start".to_string(), target: "par".to_string(), condition: None, branch_type: None, branch_index: None },
            Edge { source: "par".to_string(), target: "final_join".to_string(), condition: None, branch_type: None, branch_index: None },
            Edge { source: "final_join".to_string(), target: "end".to_string(), condition: None, branch_type: None, branch_index: None },
        ];
        
        let workflow = Workflow {
            id: workflow_id.clone(),
            name: "Benchmark Chained Assign".to_string(),
            variables: HashMap::new(),
            nodes: nodes_vec,
            edges: edges_vec,
        };

        // 2. Compile
        let config = CompilerConfig { enable_fusion: !self.no_jit };
        let mut compiler = Compiler::new_with_config(config);
        let blueprint = compiler.compile(workflow)?;
        self.engine.register_blueprint(blueprint.clone());

        // 3. Run
        let instance_id = self.engine.start_workflow(&blueprint.id, HashMap::new()).await?;
        
        let start = Instant::now();
        
        // Poll for completion - now we need to check all branch finished flags
        loop {
            tokio::time::sleep(Duration::from_micros(100)).await;
            let mut all_finished = true;
            for i in 0..branch_count {
                let var_name = format!("finished_branch_{}", i);
                if self.engine.get_instance_var(instance_id, &var_name).await != Some(json!(true)) {
                    all_finished = false;
                    break;
                }
            }

            if all_finished {
                 break;
            }
            if start.elapsed().as_secs() > 60 {
                anyhow::bail!("Benchmark timeout");
            }
        }

        let duration = start.elapsed();
        let total_ops_per_branch = 10 + 1; // 10 assign + 1 final assign
        let total_simulated_tasks = branch_count * total_ops_per_branch;
        let tps = total_simulated_tasks as f64 / duration.as_secs_f64();
        
        Ok((duration, tps))
    }

    pub async fn auto_tune(&self) -> Result<()> {
        let cpu_count = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(1);
        let worker_count = cpu_count * 2;
        
        println!("==================================================================");
        println!("🚀 SKRIPT EXTREME STRESS BENCHMARK");
        println!("==================================================================");
        println!("CPU Cores: {}", cpu_count);
        println!("Workers:   {}", worker_count);
        println!("JIT:       {}", if self.no_jit { "DISABLED" } else { "ENABLED" });
        println!("Mode:      Chained Assign Tasks (10 per branch) [High CPU Load]");
        println!("Strategy:  Auto-Ramp (2x) -> Sustain Test (10s)");
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
        let _fib_n = 25; // Not used in this benchmark mode
        let mut last_avg_tps = 0.0;
        let mut peak_tps = 0.0;
        let mut optimal_load = 0;

        // 1. Ramp-up Phase
        loop {
            print!("Ramping: {:6} branches | Samples: ", current_branches);
            
            // Take 3 samples
            let mut tps_sum = 0.0;
            for _ in 0..3 {
                match self.run_once(current_branches, _fib_n).await {
                    Ok((_, tps)) => {
                        tps_sum += tps;
                        print!(".");
                    }
                    Err(e) => {
                        println!("Error: {}", e);
                        break;
                    }
                }
            }
            let avg_tps = tps_sum / 3.0;
            println!(" | TPS: {:>8.2}", avg_tps);

            if avg_tps > peak_tps {
                peak_tps = avg_tps;
                optimal_load = current_branches;
            }

            // Ramp up strategy (Aggressive 2x)
            if avg_tps > last_avg_tps * 0.98 { 
                last_avg_tps = avg_tps;
                current_branches = current_branches * 2;
            } else {
                println!("⚠️  Saturation detected at {} branches.", current_branches);
                break;
            }

            if current_branches > 200_000 {
                println!("🛑 Safety cap reached.");
                break;
            }
        }

        println!("------------------------------------------------------------------");
        println!("🔥 SUSTAINED LOAD TEST (10s)");
        println!("------------------------------------------------------------------");
        println!("Target Load: {} concurrent branches", optimal_load);
        
        let start_sustain = Instant::now();
        let mut total_tasks_processed = 0;
        let mut iterations = 0;

        while start_sustain.elapsed().as_secs() < 10 {
            iterations += 1;
            match self.run_once(optimal_load, _fib_n).await {
                Ok(_) => {
                    total_tasks_processed += optimal_load;
                    if iterations % 5 == 0 {
                        print!("."); 
                        use std::io::Write;
                        std::io::stdout().flush().unwrap();
                    }
                }
                Err(e) => println!("Sustain error: {}", e),
            }
        }
        println!();

        let sustain_duration = start_sustain.elapsed();
        let sustained_tps = total_tasks_processed as f64 / sustain_duration.as_secs_f64();

        println!("==================================================================");
        println!("🏆 FINAL RESULTS");
        println!("==================================================================");
        println!("Peak TPS (Burst):   {:.2}", peak_tps);
        println!("Sustained TPS:      {:.2}", sustained_tps);
        println!("Optimal Load:       {}", optimal_load);
        println!("Total Branches:     {}", total_tasks_processed);
        println!("Total Assign Ops:   {}", total_tasks_processed * (10 + 1));
        println!("==================================================================");

        for h in handles { h.abort(); }
        Ok(())
    }
}