use uuid::Uuid;
use crate::runtime::blueprint::NodeIndex;

#[derive(Debug, Clone)]
pub struct Task {
    pub instance_id: Uuid,
    pub token_id: Uuid,
    pub node_index: NodeIndex,
    /// 用于追踪 Fork/Join 的血缘关系
    /// 当 Fork 时，子 Token 继承父 Token 的 flow_id (或者生成新的 flow_id 并在 Join 时检查?)
    /// 简单策略：Fork 时产生新的 flow_id 给一组分支，Join 时等待该 flow_id 下的所有分支完成。
    /// 或者：使用 Token 的 Parent 关系。
    /// 我们暂时保留 flow_id，用于标识“这一批并行任务”。
    pub flow_id: Uuid, 
}
