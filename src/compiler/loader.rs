use anyhow::{Result, Context as AnyhowContext};
use std::fs;
use crate::dsl::Workflow;

pub fn load_workflow_from_yaml(file_path: &str) -> Result<Workflow> {
    let yaml_content = fs::read_to_string(file_path)
        .with_context(|| format!("Failed to read YAML file from {}", file_path))?;

    // Try parsing as generic Value first to check for "workflow" wrapper
    let value: serde_yaml::Value = serde_yaml::from_str(&yaml_content)
        .with_context(|| format!("Failed to parse YAML from {}", file_path))?;

    let mut final_value = if let Some(inner) = value.get("workflow") {
        inner.clone()
    } else {
        value.clone()
    };

    // Merge 'nodes' and 'edges' from root if missing in workflow object
    if let Some(map) = final_value.as_mapping_mut() {
        let nodes_key = serde_yaml::Value::String("nodes".to_string());
        if !map.contains_key(&nodes_key) {
            if let Some(nodes) = value.get("nodes") {
                map.insert(nodes_key, nodes.clone());
            }
        }

        let edges_key = serde_yaml::Value::String("edges".to_string());
        if !map.contains_key(&edges_key) {
            if let Some(edges) = value.get("edges") {
                map.insert(edges_key, edges.clone());
            }
        }
    }

    let workflow: Workflow = serde_yaml::from_value(final_value)
        .with_context(|| format!("Failed to deserialize Workflow structure from {}", file_path))?;

    Ok(workflow)
}
