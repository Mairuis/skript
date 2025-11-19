use anyhow::{Result, Context as AnyhowContext};
use std::fs;
use crate::dsl::Workflow;

pub fn load_workflow_from_yaml(file_path: &str) -> Result<Workflow> {
    let yaml_content = fs::read_to_string(file_path)
        .with_context(|| format!("Failed to read YAML file from {}", file_path))?;

    let workflow: Workflow = serde_yaml::from_str(&yaml_content)
        .with_context(|| format!("Failed to deserialize YAML content from {}", file_path))?;

    Ok(workflow)
}
