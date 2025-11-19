use crate::runtime::blueprint::{Blueprint, BlueprintNode};
use crate::actions::ExecutionMode;
use std::collections::{HashMap, HashSet};
use anyhow::Result;
use serde_json::{json, Value};

pub struct Optimizer;

impl Optimizer {
    pub fn new() -> Self {
        Self
    }

    pub fn optimize(&self, blueprint: Blueprint, lookup_mode: impl Fn(&str) -> Option<ExecutionMode>) -> Result<Blueprint> {
        let nodes = blueprint.nodes;
        let n_count = nodes.len();
        
        // 1. Build Graph Info (Adjacency & In-Degree)
        // We need to parse "next", "targets", "branches" from params to find edges.
        let mut adj: Vec<Vec<usize>> = vec![Vec::new(); n_count];
        let mut in_degree: Vec<usize> = vec![0; n_count];

        for (u, node) in nodes.iter().enumerate() {
            let targets = extract_targets(node);
            for &v in &targets {
                if v < n_count {
                    adj[u].push(v);
                    in_degree[v] += 1;
                }
            }
        }

        // 2. Identify Fusion Chains
        // chain_map: starting_node_index -> List of nodes in the chain
        // merged: set of nodes that are merged into a chain (excluding the head)
        let mut chains: HashMap<usize, Vec<usize>> = HashMap::new();
        let mut merged: HashSet<usize> = HashSet::new();

        // Iterate topologically or just linear scan? 
        // Linear scan is fine if we just look for local pairs.
        // We want to find maximal chains. 
        // A node is a chain head if it is Sync and (not merged).
        
        for i in 0..n_count {
            if merged.contains(&i) {
                continue;
            }

            if is_sync(&nodes[i], &lookup_mode) {
                // Start a chain?
                // Only if it's not already part of a chain? (covered by !merged)
                
                let mut current_chain = vec![i];
                let mut curr = i;

                loop {
                    // Look ahead
                    // Condition for A -> B fusion:
                    // 1. A has exactly 1 outgoing edge to B.
                    // 2. B has exactly 1 incoming edge (from A).
                    // 3. B is Sync.
                    // 4. B is not A (no self loop).
                    
                    if adj[curr].len() != 1 {
                        break;
                    }
                    let next = adj[curr][0];
                    
                    if next == curr { break; } // Loop protection
                    
                    if in_degree[next] == 1 && is_sync(&nodes[next], &lookup_mode) {
                        // Fuse!
                        current_chain.push(next);
                        merged.insert(next);
                        curr = next;
                    } else {
                        break;
                    }
                }
                
                if current_chain.len() > 1 {
                    chains.insert(i, current_chain);
                }
            }
        }

        // 3. Construct New Nodes & Remap
        let mut new_nodes: Vec<BlueprintNode> = Vec::new();
        let mut old_to_new: HashMap<usize, usize> = HashMap::new();

        for i in 0..n_count {
            if merged.contains(&i) {
                continue; // Skip merged nodes
            }

            if let Some(chain) = chains.get(&i) {
                // Create Fused Node
                let head_node = &nodes[chain[0]];
                let tail_node_idx = *chain.last().unwrap();
                let tail_node = &nodes[tail_node_idx];
                
                // Extract ops from all nodes in chain
                let mut ops = Vec::new();
                for &idx in chain {
                    let node = &nodes[idx];
                    ops.push(json!({
                        "kind": node.kind,
                        "params": node.params
                    }));
                }
                
                // The "next" of the fused node is the "next" of the tail node
                // But we need to preserve the structure expected by FusedNode runtime
                // The runtime FusedNode expects "ops" and "next".
                // extract_targets(tail_node) gives us the next index (if any).
                // But wait, BlueprintNode stores "next" in params usually.
                // We should copy the control flow params from the tail node to the fused node.
                
                // Let's grab the "next" field from tail node's params if it exists.
                let next_val = tail_node.params.get("next").cloned();
                
                let fused_params = json!({
                    "ops": ops,
                    "next": next_val 
                });
                
                let new_idx = new_nodes.len();
                new_nodes.push(BlueprintNode {
                    kind: "fused".to_string(),
                    params: fused_params,
                });
                
                // Map all nodes in chain to this new node
                // Actually, only the head needs to be mapped for incoming edges from outside.
                // Internal edges are gone.
                old_to_new.insert(i, new_idx);
                
            } else {
                // Keep as is
                let new_idx = new_nodes.len();
                new_nodes.push(nodes[i].clone());
                old_to_new.insert(i, new_idx);
            }
        }

        // 4. Remap Targets
        // Update all indices in params to point to new_nodes indices
        for node in &mut new_nodes {
             remap_node_targets(node, &old_to_new);
        }
        
        // Remap start_index
        let new_start_index = *old_to_new.get(&blueprint.start_index).unwrap_or(&blueprint.start_index); // Fallback should not happen if valid

        Ok(Blueprint {
            id: blueprint.id,
            name: blueprint.name,
            nodes: new_nodes,
            start_index: new_start_index,
        })
    }
}

fn is_sync(node: &BlueprintNode, lookup: &impl Fn(&str) -> Option<ExecutionMode>) -> bool {
    lookup(&node.kind) == Some(ExecutionMode::Sync)
}

// Helper to extract all outgoing node indices from a node's params
fn extract_targets(node: &BlueprintNode) -> Vec<usize> {
    let mut targets = Vec::new();
    
    // Common patterns
    if let Some(next) = node.params.get("next").and_then(|v| v.as_u64()) {
        targets.push(next as usize);
    }
    
    if let Some(ts) = node.params.get("targets").and_then(|v| v.as_array()) {
        for t in ts {
            if let Some(idx) = t.as_u64() {
                targets.push(idx as usize);
            }
        }
    }
    
    if let Some(join) = node.params.get("join_target").and_then(|v| v.as_u64()) {
        targets.push(join as usize);
    }

    if let Some(branches) = node.params.get("branches").and_then(|v| v.as_array()) {
        for b in branches {
             if let Some(t) = b.get("target").and_then(|v| v.as_u64()) {
                 targets.push(t as usize);
             }
        }
    }
    
    if let Some(else_next) = node.params.get("else_next").and_then(|v| v.as_u64()) {
        targets.push(else_next as usize);
    }

    // Loop/Iteration
    if let Some(body) = node.params.get("body").and_then(|v| v.as_u64()) {
        targets.push(body as usize);
    }
    
    targets
}

fn remap_node_targets(node: &mut BlueprintNode, map: &HashMap<usize, usize>) {
    // Helper to remap a Value containing an index
    let remap_val = |v: &mut Value| {
        if let Some(idx) = v.as_u64() {
            if let Some(&new_idx) = map.get(&(idx as usize)) {
                *v = json!(new_idx);
            }
        }
    };

    // Remap known fields
    if let Some(next) = node.params.get_mut("next") {
        remap_val(next);
    }
    
    if let Some(ts) = node.params.get_mut("targets").and_then(|v| v.as_array_mut()) {
        for t in ts {
            remap_val(t);
        }
    }

    if let Some(join) = node.params.get_mut("join_target") {
        remap_val(join);
    }

    if let Some(branches) = node.params.get_mut("branches").and_then(|v| v.as_array_mut()) {
        for b in branches {
             if let Some(t) = b.get_mut("target") {
                 remap_val(t);
             }
        }
    }
    
    if let Some(else_next) = node.params.get_mut("else_next") {
        remap_val(else_next);
    }

    if let Some(body) = node.params.get_mut("body") {
        remap_val(body);
    }
}
