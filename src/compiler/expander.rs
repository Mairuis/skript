use crate::dsl::{Workflow, Node, NodeType, Edge, Branch};
use anyhow::{Result, anyhow};

pub struct Expander {
    // 可以添加状态，如生成的 ID 计数器
}

impl Expander {
    pub fn new() -> Self {
        Self {}
    }

    pub fn expand(&self, workflow: Workflow) -> Result<Workflow> {
        let mut new_nodes = Vec::new();
        let mut new_edges = workflow.edges.clone();
        
        // 待处理的边重定向：Map<OldTarget, NewTarget>
        // 当 Parallel 节点被替换为 Fork 时，指向 Parallel 的边需要改指 Fork
        // 当 Parallel 节点有 next 时，Join 节点需要指向 next
        // 但这里的 DSL 结构中，next 是通过 Edge 定义的。
        
        // 我们采用一种简单的策略：
        // 1. 遍历所有节点。
        // 2. 如果是 Parallel，生成一堆新节点和边，替换掉它。
        // 3. 修正指向 Parallel 的边。

        for node in workflow.nodes {
            if let NodeType::Parallel { branches } = node.kind {
                self.expand_parallel(node.id, branches, &mut new_nodes, &mut new_edges)?;
            } else {
                new_nodes.push(node);
            }
        }

        Ok(Workflow {
            nodes: new_nodes,
            edges: new_edges,
            ..workflow
        })
    }

    fn expand_parallel(
        &self,
        parallel_id: String,
        branches: Vec<Branch>,
        new_nodes: &mut Vec<Node>,
        new_edges: &mut Vec<Edge>,
    ) -> Result<()> {
        let fork_id = format!("{}_fork", parallel_id);
        let join_id = format!("{}_join", parallel_id);

        let mut branch_start_ids = Vec::new();

        for branch in branches {
            if branch.nodes.is_empty() {
                continue;
            }

            // 1. 提取分支内节点
            let branch_len = branch.nodes.len();
            for (i, node) in branch.nodes.iter().enumerate() {
                new_nodes.push(node.clone());
                
                // 2. 自动生成分支内部的连接边 (线性连接)
                if i < branch_len - 1 {
                    let next_node = &branch.nodes[i+1];
                    new_edges.push(Edge {
                        source: node.id.clone(),
                        target: next_node.id.clone(),
                        condition: None,
                        branch_type: None,
                        branch_index: None,
                    });
                }
            }

            // 记录分支头尾
            let head_id = branch.nodes[0].id.clone();
            let tail_id = branch.nodes[branch_len - 1].id.clone();

            branch_start_ids.push(head_id);

            // 3. 连接 分支尾 -> Join
            new_edges.push(Edge {
                source: tail_id,
                target: join_id.clone(),
                condition: None,
                branch_type: None,
                branch_index: None,
            });
        }

        // 4. 创建 Fork 节点
        new_nodes.push(Node {
            id: fork_id.clone(),
            kind: NodeType::Fork {
                branch_start_ids: branch_start_ids.clone(),
                join_id: join_id.clone(),
            },
        });

        // 5. 创建 Join 节点
        new_nodes.push(Node {
            id: join_id.clone(),
            kind: NodeType::Join {
                expect_count: branch_start_ids.len(),
            },
        });

        // 6. 修正外部边：指向 Parallel 的 -> 指向 Fork
        for edge in new_edges.iter_mut() {
            if edge.target == parallel_id {
                edge.target = fork_id.clone();
            }
            // 如果 Parallel 指向外部 (source == parallel_id)，则改为 Join 指向外部
            if edge.source == parallel_id {
                edge.source = join_id.clone();
            }
        }

        Ok(())
    }
}
