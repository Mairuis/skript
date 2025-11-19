use crate::dsl::{Workflow, Node, NodeType, Edge};
use crate::runtime::blueprint::{Blueprint, BlueprintNode, NodeIndex};
use crate::compiler::expander::Expander;
use std::collections::HashMap;
use anyhow::{Result, anyhow};
use serde_json::json;

pub struct Compiler {
    id_map: HashMap<String, NodeIndex>,
}

impl Compiler {
    pub fn new() -> Self {
        Self {
            id_map: HashMap::new(),
        }
    }

    pub fn compile(&mut self, raw_workflow: Workflow) -> Result<Blueprint> {
        // 0. Pass 0: Expand
        let expander = Expander::new();
        let workflow = expander.expand(raw_workflow)?;

        // 1. Pass 1: Indexing
        for (idx, node) in workflow.nodes.iter().enumerate() {
            if self.id_map.insert(node.id.clone(), idx).is_some() {
                return Err(anyhow!("Duplicate node ID: {}", node.id));
            }
        }

        // 2. Pass 2: Transform
        let mut blueprint_nodes = Vec::with_capacity(workflow.nodes.len());
        
        let mut adjacency: HashMap<String, Vec<&Edge>> = HashMap::new();
        for edge in &workflow.edges {
            adjacency.entry(edge.source.clone()).or_default().push(edge);
        }

        for node in &workflow.nodes {
            let bp_node = self.transform_node(node, &adjacency)?;
            blueprint_nodes.push(bp_node);
        }
        
        // 3. Start Node
        let start_node_id = workflow.nodes.iter()
            .find(|n| matches!(n.kind, NodeType::Start))
            .map(|n| n.id.clone())
            .ok_or_else(|| anyhow!("Start node not found"))?;
            
        let start_index = *self.id_map.get(&start_node_id).unwrap();

        Ok(Blueprint {
            id: workflow.id,
            name: workflow.name,
            nodes: blueprint_nodes,
            start_index,
        })
    }

    fn transform_node(&self, node: &Node, adjacency: &HashMap<String, Vec<&Edge>>) -> Result<BlueprintNode> {
        let edges = adjacency.get(&node.id).map(|v| v.as_slice()).unwrap_or(&[]);
        
        match &node.kind {
            NodeType::Start => {
                let next = edges.first().map(|e| self.resolve_target(&e.target)).transpose()?;
                Ok(BlueprintNode {
                    kind: "start".to_string(),
                    params: json!({ "next": next }),
                })
            }
            NodeType::End { output } => Ok(BlueprintNode {
                kind: "end".to_string(),
                params: json!({ "output": output }),
            }),
            NodeType::Function { name, params, output } => {
                let next = edges.first().map(|e| self.resolve_target(&e.target)).transpose()?;
                
                // Combine user params with system params
                let mut full_params = serde_json::to_value(params)?;
                if let Some(obj) = full_params.as_object_mut() {
                    if let Some(n) = next {
                         obj.insert("next".to_string(), json!(n));
                    }
                    if let Some(o) = output {
                        obj.insert("output".to_string(), json!(o));
                    }
                }
                
                Ok(BlueprintNode {
                    kind: name.clone(),
                    params: full_params,
                })
            }
            NodeType::Assign { assignments, expression } => {
                let next = edges.first().map(|e| self.resolve_target(&e.target)).transpose()?;
                
                let mut full_params = json!({
                    "assignments": assignments,
                    "expression": expression
                });
                
                if let Some(obj) = full_params.as_object_mut() {
                    if let Some(n) = next {
                         obj.insert("next".to_string(), json!(n));
                    }
                }

                Ok(BlueprintNode {
                    kind: "assign".to_string(),
                    params: full_params,
                })
            }
            NodeType::Iteration { collection, item_var } => {
                let mut next = None;
                let mut body = None;

                for edge in edges {
                     let target_idx = self.resolve_target(&edge.target)?;
                     if edge.branch_type.as_deref() == Some("body") {
                         if body.is_some() {
                             return Err(anyhow!("Multiple body branches for iteration node {}", node.id));
                         }
                         body = Some(target_idx);
                     } else {
                         if next.is_some() {
                             return Err(anyhow!("Multiple next branches for iteration node {}", node.id));
                         }
                         next = Some(target_idx);
                     }
                }

                Ok(BlueprintNode {
                    kind: "iteration".to_string(),
                    params: json!({
                        "collection": collection,
                        "item_var": item_var,
                        "next": next,
                        "body": body
                    }),
                })
            }
            NodeType::Loop { condition } => {
                let mut next = None;
                let mut body = None;

                for edge in edges {
                     let target_idx = self.resolve_target(&edge.target)?;
                     if edge.branch_type.as_deref() == Some("body") {
                         body = Some(target_idx);
                     } else {
                         next = Some(target_idx);
                     }
                }
                
                Ok(BlueprintNode {
                    kind: "loop".to_string(),
                    params: json!({
                        "condition": condition,
                        "body": body,
                        "next": next
                    }),
                })
            }
            NodeType::If { branches: defined_branches } => {
                 let mut compiled_branches = Vec::new();
                 let mut else_next = None;

                 for edge in edges {
                     let target_idx = self.resolve_target(&edge.target)?;
                     
                     if let Some(cond) = &edge.condition {
                         // Edge defines condition
                         compiled_branches.push(json!({
                             "condition": cond,
                             "target": target_idx
                         }));
                     } else if let Some(idx) = edge.branch_index {
                         // Edge refers to index in defined_branches
                         if idx < defined_branches.len() {
                             if let Some(cond) = defined_branches[idx].get("condition") {
                                 compiled_branches.push(json!({
                                     "condition": cond,
                                     "target": target_idx
                                 }));
                             } else {
                                 return Err(anyhow!("Branch {} for node {} has no condition", idx, node.id));
                             }
                         } else {
                             return Err(anyhow!("Branch index {} out of bounds for node {}", idx, node.id));
                         }
                     } else if edge.branch_type.as_deref() == Some("else") {
                         if else_next.is_some() {
                             return Err(anyhow!("Multiple else branches found for node {}", node.id));
                         }
                         else_next = Some(target_idx);
                     } else {
                         // Fallback: treat as else if no condition/index? Or error?
                         // If ambiguous, treat as else if not set.
                         if else_next.is_some() {
                              return Err(anyhow!("Multiple else/default branches found for node {}", node.id));
                         }
                         else_next = Some(target_idx);
                     }
                 }
                 
                 Ok(BlueprintNode {
                     kind: "if".to_string(),
                     params: json!({
                         "branches": compiled_branches,
                         "else_next": else_next
                     }),
                 })
            }
            NodeType::Parallel { .. } => {
                Err(anyhow!("Parallel node '{}' should have been expanded", node.id))
            }
            NodeType::Fork { branch_start_ids, join_id } => {
                let mut targets = Vec::new();
                for id in branch_start_ids {
                    targets.push(self.resolve_target(id)?);
                }
                let join_target = self.resolve_target(join_id)?;
                
                Ok(BlueprintNode {
                    kind: "fork".to_string(),
                    params: json!({
                        "targets": targets,
                        "join_target": join_target
                    }),
                })
            }
            NodeType::Join { expect_count } => {
                 let next = edges.first().map(|e| self.resolve_target(&e.target)).transpose()?;
                 Ok(BlueprintNode {
                     kind: "join".to_string(),
                     params: json!({
                         "next": next,
                         "expect_count": expect_count
                     }),
                 })
            }
        }
    }
    
    fn resolve_target(&self, target_id: &str) -> Result<NodeIndex> {
        self.id_map.get(target_id)
            .cloned()
            .ok_or_else(|| anyhow!("Target node not found: {}", target_id))
    }
}
