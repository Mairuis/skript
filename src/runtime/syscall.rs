use crate::runtime::blueprint::NodeIndex;

/// 系统调用接口
/// Node 通过此接口控制 Engine 的调度
pub trait Syscall: Send + Sync {
    /// 跳转到下一个节点
    fn jump(&mut self, target: NodeIndex);
    
    /// 分叉：产生多个并行分支
    fn fork(&mut self, targets: Vec<NodeIndex>);
    
    /// 挂起当前任务 (不产生新任务，等待被唤醒或丢弃)
    fn wait(&mut self);
    
    /// 结束当前分支
    fn terminate(&mut self);
}